//! P4 private, single-world Hiyal adapter. Pure P3 rules remain independent.
//!
//! The private, versioned snapshot is an action-fold checkpoint: it retains
//! every admitted *typed logical action*, including private minister effort,
//! plus a digest of the reconstructed complete P3 campaign. It deliberately
//! does not embed free-text transcripts, LLM prompts, provider keys or UI state.
//! On untrusted resume Hiyal replays the committed journal; the reducer is
//! reconstituted from its private action fold and checked against the digest.
pub mod orchestrator;
pub mod p5_allowance;
pub mod p5_backend;
pub mod p5_cabinet;
pub mod p5_openrouter;
use hiyal::core::{AgentId, World};
use myth_contracts::{
    PublicPosition,
    checkpoint::{Phase, PrivateCheckpoint},
    parse_command, parse_minister_action,
};
use myth_domain as d;
use myth_domain::population::{Attribute, CHANNELS, Group, Kernel, Level, Population, Selector};
use myth_domain::{
    reducer::{Action, Campaign, Rules},
    scripted,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub const WORLD_KIND: &str = "myth.england-education-hiyal.v1";
pub const MODEL_KIND: &str = "p0-profile-only-scripted.v1";
pub const PLAYER: AgentId = AgentId(0);
pub const FINANCE: AgentId = AgentId(1);
pub const EDUCATION: AgentId = AgentId(2);
pub const INFRASTRUCTURE: AgentId = AgentId(3);
pub const PERIOD: AgentId = AgentId(4);
pub const ACTORS: [AgentId; 5] = [PLAYER, FINANCE, EDUCATION, INFRASTRUCTURE, PERIOD];
// A full term of meetings is about 60 actions, so this holds the longest game (MAX_TERMS terms) with room to spare.
const MAX_ACCEPTED_ACTIONS: usize = 1024;
const MAX_ACTION_BYTES: usize = 12_288;
const PACK: &str = include_str!("../../../data/england-school-pilot-v1/country-pack.json");
const SCENARIO: &str = include_str!("../../../data/england-school-pilot-v1/authored.json");
const MANIFEST: &str = include_str!("../../../data/england-school-pilot-v1/manifest.json");

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// The seed of a campaign's news calendar (decision 0010): the first eight bytes of a digest of its identity.
pub fn news_seed(campaign_id: &str) -> u64 {
    let digest = Sha256::digest(format!("myth-maker-news-calendar:{campaign_id}").as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    u64::from_be_bytes(bytes)
}
/// The words of a headline, from the frozen scenario file: `(title, summary)`. The engine only knows the numbers.
pub fn headline_text(tone: d::news::Tone, index: usize) -> Option<(String, String)> {
    static TEXT: OnceLock<Option<Value>> = OnceLock::new();
    let scenario = TEXT
        .get_or_init(|| serde_json::from_str::<Value>(SCENARIO).ok())
        .as_ref()?;
    let key = match tone {
        d::news::Tone::Bad => "bad",
        d::news::Tone::Good => "good",
    };
    let entry = scenario["news_calendar"][key].get(index)?;
    Some((
        entry["title"].as_str()?.to_owned(),
        entry["summary"].as_str()?.to_owned(),
    ))
}
/// The words of a trade-off story (decision 0014), from the frozen scenario file: `(title, summary)`.
pub fn tradeoff_text(index: usize) -> Option<(String, String)> {
    let scenario: Value = serde_json::from_str(SCENARIO).ok()?;
    let entry = scenario["news_calendar"]["mixed"].get(index)?;
    Some((
        entry["title"].as_str()?.to_owned(),
        entry["summary"].as_str()?.to_owned(),
    ))
}
/// A deliberate, migration-controlled behavior contract, not raw source text.
/// Bump when P3 rules, parsing, adapters, scenario selection or model policy change.
pub const RULES_VERSION: &str = "p4-england-education-rules-v11";
/// What the frozen files fix for every campaign: their digests and the population of voters the scenario describes.
struct Frozen {
    pack: String,
    scenario: String,
    population: Population,
}
type FrozenSource = Result<Frozen, String>;
static FROZEN: OnceLock<FrozenSource> = OnceLock::new();
fn frozen() -> Result<&'static Frozen, String> {
    FROZEN
        .get_or_init(|| {
            let meta: Value = serde_json::from_str(MANIFEST).map_err(error)?;
            let pack = meta["country_pack_sha256"]
                .as_str()
                .ok_or("missing P2 pack digest")?;
            let scenario_hash = meta["authored_sha256"]
                .as_str()
                .ok_or("missing P2 scenario digest")?;
            if sha(PACK.as_bytes()) != pack || sha(SCENARIO.as_bytes()) != scenario_hash {
                return Err("frozen P2 source hashes do not match manifest".into());
            }
            let scenario: Value = serde_json::from_str(SCENARIO).map_err(error)?;
            Ok(Frozen {
                pack: pack.into(),
                scenario: scenario_hash.into(),
                population: electorate(&scenario)?,
            })
        })
        .as_ref()
        .map_err(Clone::clone)
}
/// The population of voters the scenario describes (decision 0015): its attributes and groups and kernel from `electorate`, and
/// the share of each cell from the baseline of `population`, in the order `population.order` names.
fn electorate(scenario: &Value) -> Result<Population, String> {
    fn text(v: &Value, what: &str) -> Result<String, String> {
        v.as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("electorate: {what} must be text"))
    }
    fn list<'a>(v: &'a Value, what: &str) -> Result<&'a Vec<Value>, String> {
        v.as_array()
            .ok_or_else(|| format!("electorate: {what} must be a list"))
    }
    fn selector(v: &Value, attributes: &[Attribute]) -> Result<Selector, String> {
        if v.as_str() == Some("everyone") {
            return Ok(Selector::Everyone);
        }
        let object = v
            .as_object()
            .filter(|o| o.len() == 1)
            .ok_or("electorate: a selector is \"everyone\" or an object with one key")?;
        let (key, inner) = object.iter().next().ok_or("electorate: empty selector")?;
        match key.as_str() {
            "is" => {
                let attribute_id = text(&inner["attribute"], "selector attribute")?;
                let level_id = text(&inner["level"], "selector level")?;
                let attribute = attributes
                    .iter()
                    .position(|a| a.id == attribute_id)
                    .ok_or_else(|| format!("electorate: unknown attribute {attribute_id}"))?;
                let level = attributes[attribute]
                    .levels
                    .iter()
                    .position(|l| l.id == level_id)
                    .ok_or_else(|| format!("electorate: unknown level {level_id}"))?;
                Ok(Selector::Is { attribute, level })
            }
            "all" | "any" => {
                let parts = list(inner, "selector parts")?
                    .iter()
                    .map(|part| selector(part, attributes))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(if key == "all" {
                    Selector::All(parts)
                } else {
                    Selector::Any(parts)
                })
            }
            "not" => Ok(Selector::Not(Box::new(selector(inner, attributes)?))),
            other => Err(format!("electorate: unknown selector {other}")),
        }
    }
    fn channels(v: &Value, what: &str) -> Result<[i128; CHANNELS], String> {
        let numbers = list(v, what)?
            .iter()
            .map(|n| n.as_i64().map(i128::from))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| format!("electorate: {what} must be whole numbers"))?;
        <[i128; CHANNELS]>::try_from(numbers)
            .map_err(|_| format!("electorate: {what} needs {CHANNELS} numbers"))
    }
    let e = &scenario["electorate"];
    let attributes = list(&e["attributes"], "attributes")?
        .iter()
        .map(|a| {
            let levels = list(&a["levels"], "levels")?
                .iter()
                .map(|l| {
                    Ok(Level {
                        id: text(&l["id"], "level id")?,
                        label: text(&l["label"], "level label")?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Attribute {
                id: text(&a["id"], "attribute id")?,
                levels,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let declared = list(&e["groups"], "groups")?
        .iter()
        .map(|g| {
            Ok(Group {
                id: text(&g["id"], "group id")?,
                label: text(&g["label"], "group label")?,
                selector: selector(&g["select"], &attributes)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let kernel = &e["kernel"];
    let percent = attributes
        .iter()
        .map(|a| {
            a.levels
                .iter()
                .map(|l| channels(&kernel["percent"][&a.id][&l.id], "kernel percent"))
                .collect::<Result<Vec<_>, String>>()
        })
        .collect::<Result<Vec<_>, String>>()?;
    let kernel = Kernel {
        base_ppm: channels(&kernel["base_ppm"], "kernel base")?,
        percent,
    };
    let weights = list(
        &scenario["population"]["variants"][0]["weights_ppm"],
        "weights",
    )?
    .iter()
    .map(|w| w.as_u64().and_then(|w| u32::try_from(w).ok()))
    .collect::<Option<Vec<_>>>()
    .ok_or("electorate: cell weights must be whole numbers")?;
    let population = Population::new(attributes, weights, declared, kernel).map_err(error)?;
    // The invented news names groups: the scenario must declare each, or a headline would silently do nothing.
    d::news::check_decks(&population).map_err(error)?;
    let order = list(&scenario["population"]["order"], "population order")?;
    if order.len() != population.cells()
        || order
            .iter()
            .enumerate()
            .any(|(cell, id)| id.as_str() != population.cell_id(cell))
    {
        return Err("electorate: population.order does not name the cells in order".into());
    }
    Ok(population)
}
/// The population of voters every campaign has (decision 0015), read from the frozen scenario.
pub fn population() -> Result<&'static Population, String> {
    frozen().map(|f| &f.population)
}
fn encoded<T: Serialize>(x: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec(x).map_err(|e| e.to_string())
}
fn error<E: std::fmt::Debug>(e: E) -> String {
    format!("{e:?}")
}
fn actor_role(actor: AgentId) -> Option<d::Ministry> {
    match actor.0 {
        1 => Some(d::Ministry::Finance),
        2 => Some(d::Ministry::Education),
        3 => Some(d::Ministry::Infrastructure),
        _ => None,
    }
}
// Serialize the same typed vote enums as the validated P1 contracts.
fn wire_vote(value: d::Position) -> myth_contracts::Vote {
    match value {
        d::Position::Support => myth_contracts::Vote::Support,
        d::Position::Conditional => myth_contracts::Vote::Conditional,
        d::Position::Oppose => myth_contracts::Vote::Oppose,
    }
}
fn allocation_value(a: d::Allocation) -> Value {
    json!({"staff":a.staff(),"repairs":a.repairs(),"access":a.access()})
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    PlayerCommand,
    MinisterAction,
    FreezeFinal,
}

/// Private, replay-bound record, never publicly serialized in HTTP responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameAction {
    pub actor: u8,
    pub kind: ActionKind,
    pub payload: Value,
    /// Exact role observation digest at the moment the decision was made.
    pub observation_sha256: String,
    /// Versioned role-visible typed evidence supplied to the actor. Unlike
    /// a free-text transcript, this is the exact permitted action context.
    pub role_observation: Option<Value>,
    /// Added only after a checked charged model receipt; absent in P4 scripts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_receipt_digest: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameObservation {
    pub schema_version: u16,
    pub synthetic_receipts: bool,
    pub actor: u8,
    pub phase: Phase,
    pub quarter: u8,
    pub expected_world_version: String,
    pub action: GameAction,
}

/// Versioned fixed synthetic load records. They are not additional modeled
/// beneficiaries: the pure P3 society still has exactly six authored cells.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StressCohortRecord {
    pub index: u16,
    pub cell: u8,
    pub fictional_resource: u16,
}
/// All private state needed to *reconstruct* P3 exists in the bounded action
/// prefix and frozen genesis. No hidden in-memory-only minister effort survives
/// outside this fold; every persisted action is also a Hiyal journal event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameWorld {
    pub schema_version: u16,
    pub campaign_id: String,
    pub pack_id: String,
    pub pack_sha256: String,
    pub scenario_sha256: String,
    pub rules_identity: String,
    /// The share of each cell of the population, in ppm; as many entries as the scenario's population has cells.
    pub weights_ppm: Vec<u32>,
    pub source_cutoff_utc: String,
    pub synthetic_receipts: bool,
    pub stress_cohorts: Vec<StressCohortRecord>,
    pub actions: Vec<GameAction>,
    /// Authoritative private state hash; not a public role projection.
    pub private_state_sha256: String,
    /// Hiyal synchronous rounds freeze all three role observations at
    /// the phase-entry boundary, not after each actor's partial commit.
    pub frozen_role_anchor: Option<String>,
    pub world_version: String,
    pub quarter: u8,
    pub phase: Phase,
}

impl GameWorld {
    pub fn initial(campaign_id: &str) -> Result<Self, String> {
        let frozen = frozen()?;
        let mut world = Self {
            schema_version: 1,
            campaign_id: campaign_id.into(),
            pack_id: "england-school-delivery-v1".into(),
            pack_sha256: frozen.pack.clone(),
            scenario_sha256: frozen.scenario.clone(),
            rules_identity: RULES_VERSION.into(),
            weights_ppm: frozen.population.weights_ppm().to_vec(),
            source_cutoff_utc: "2026-10-29T23:59:59Z".into(),
            synthetic_receipts: false,
            stress_cohorts: vec![],
            actions: vec![],
            private_state_sha256: String::new(),
            frozen_role_anchor: None,
            world_version: "0".into(),
            quarter: 1,
            phase: Phase::QuarterOpen,
        };
        let campaign = world.fold()?;
        world.refresh(&campaign)?;
        Ok(world)
    }
    /// P0-04 stress has 1,024 synthetic *world records* while P3 continues
    /// to model only six authored social cells. All 96 synthetic model receipts
    /// are generated by the no-network test backend, never a real provider.
    pub fn stress(campaign_id: &str) -> Result<Self, String> {
        let mut world = Self::initial(campaign_id)?;
        world.synthetic_receipts = true;
        world.stress_cohorts = (0..1024_u16)
            .map(|index| StressCohortRecord {
                index,
                cell: (index % 6) as u8,
                fictional_resource: 10 + (index % 17),
            })
            .collect();
        let campaign = world.fold()?;
        world.refresh(&campaign)?;
        Ok(world)
    }
    fn rules(&self) -> Rules {
        Rules {
            q2_repair_cap: Some(8),
            // Every campaign has its own calendar, fixed by its identity: a replay and a what-if branch (which keeps the
            // original identity) see the same headlines.
            news_seed: Some(news_seed(&self.campaign_id)),
            ..Rules::default()
        }
    }
    fn apply_record(campaign: &mut Campaign, record: &GameAction) -> Result<(), String> {
        if encoded(record)?.len() > MAX_ACTION_BYTES {
            return Err("oversize game action".into());
        }
        let action = match record.kind {
            ActionKind::PlayerCommand => {
                if record.actor != PLAYER.0 as u8 && record.actor != PERIOD.0 as u8 {
                    return Err("non-player actor attempted player command".into());
                }
                let cmd = parse_command(&record.payload.to_string()).map_err(error)?;
                let advance = matches!(cmd.command(), d::PlayerCommand::AdvanceQuarter);
                if (record.actor == PERIOD.0 as u8) != advance {
                    return Err("period actor must exclusively advance time".into());
                }
                Action::Player(cmd)
            }
            ActionKind::MinisterAction => {
                let role = actor_role(AgentId(u64::from(record.actor)))
                    .ok_or("unapproved minister actor")?;
                let role_view = campaign.role_observation(role).map_err(error)?;
                let audited = serde_json::to_value(
                    myth_contracts::PublicRoleView::try_from(&role_view).map_err(error)?,
                )
                .map_err(error)?;
                if record.role_observation.as_ref() != Some(&audited) {
                    return Err("minister role observation changed since frozen decision".into());
                }
                let available = role_view.available_credits;
                let parsed =
                    parse_minister_action(&record.payload.to_string(), available).map_err(error)?;
                let expected_role = match &parsed.public {
                    PublicPosition::First(p) => p.ministry,
                    PublicPosition::Final(p) => p.ministry,
                };
                if role != expected_role {
                    return Err("forged ministry action".into());
                }
                match parsed.public {
                    PublicPosition::First(p) => Action::First(p),
                    PublicPosition::Final(p) => Action::Final(p, parsed.private_effort),
                }
            }
            ActionKind::FreezeFinal => {
                if record.actor != PLAYER.0 as u8 {
                    return Err("freeze requires player".into());
                }
                let id = record.payload["command_id"]
                    .as_str()
                    .ok_or("missing freeze command id")?;
                Action::FreezeFinal {
                    command_id: id.into(),
                }
            }
        };
        campaign.apply(action).map_err(error)?;
        Ok(())
    }
    pub fn fold(&self) -> Result<Campaign, String> {
        let frozen = frozen()?;
        if self.pack_sha256 != frozen.pack
            || self.scenario_sha256 != frozen.scenario
            || self.weights_ppm.as_slice() != frozen.population.weights_ppm()
        {
            return Err("P2 source or authored scenario binding changed".into());
        }
        if self.schema_version != 1
            || self.actions.len() > MAX_ACCEPTED_ACTIONS
            || self.pack_sha256.len() != 64
            || self.scenario_sha256.len() != 64
            || self.rules_identity != RULES_VERSION
            || self.source_cutoff_utc != "2026-10-29T23:59:59Z"
            || (self.synthetic_receipts && self.stress_cohorts.len() != 1024)
            || (!self.synthetic_receipts && !self.stress_cohorts.is_empty())
            || self.stress_cohorts.iter().enumerate().any(|(i, row)| {
                usize::from(row.index) != i
                    || row.cell != (i % 6) as u8
                    || row.fictional_resource != 10 + (i as u16 % 17)
            })
        {
            return Err("unsupported or oversized P4 world version".into());
        }
        let mut campaign = Campaign::with_population(
            &self.campaign_id,
            &self.pack_id,
            frozen.population.clone(),
            self.rules(),
        )
        .map_err(error)?;
        for record in &self.actions {
            Self::apply_record(&mut campaign, record)?;
        }
        Ok(campaign)
    }
    fn digest_campaign_with_actions(
        &self,
        campaign: &Campaign,
        actions: &[GameAction],
    ) -> Result<String, String> {
        let snapshot = PrivateCheckpoint::capture(campaign.world(), self.pack_sha256.clone())
            .map_err(error)?;
        // Replaying all privately authenticated actions reconstitutes every
        // private P3 field (meeting rounds, effort, due scopes, audits, beliefs).
        // These are not exposed via the P1 public DTO or web API.
        Ok(sha(&encoded(&(
            snapshot,
            actions,
            &self.scenario_sha256,
            &self.rules_identity,
            &self.weights_ppm,
            &self.stress_cohorts,
            self.synthetic_receipts,
            MODEL_KIND,
        ))?))
    }
    fn digest_campaign(&self, campaign: &Campaign) -> Result<String, String> {
        self.digest_campaign_with_actions(campaign, &self.actions)
    }
    fn phase_of(campaign: &Campaign) -> Phase {
        campaign.phase().into()
    }
    fn refresh(&mut self, campaign: &Campaign) -> Result<(), String> {
        self.private_state_sha256 = self.digest_campaign(campaign)?;
        self.world_version = campaign.world().world_version().to_decimal();
        self.quarter = campaign.world().quarter().value();
        self.phase = Self::phase_of(campaign);
        Ok(())
    }
    /// Full private fold check on a restored world, not trusting snapshot
    /// checksums alone. The complete Hiyal journal still must be verified.
    fn verified_campaign(&self) -> Result<Campaign, String> {
        let campaign = self.fold()?;
        let anchor = if matches!(
            self.phase,
            Phase::FirstPositionsFrozen | Phase::FinalPositionsFrozen
        ) {
            let entry = self
                .actions
                .iter()
                .rposition(|a| {
                    if self.phase == Phase::FirstPositionsFrozen {
                        a.kind == ActionKind::PlayerCommand
                            && a.payload["command"]["kind"] == "propose"
                    } else {
                        a.kind == ActionKind::FreezeFinal
                    }
                })
                .ok_or("frozen phase lacks boundary action")?;
            // Replay only the immutable prefix and hash only its actions;
            // never clone the entire 1,024-record world just to find the anchor.
            let mut begin = Campaign::with_population(
                &self.campaign_id,
                &self.pack_id,
                frozen()?.population.clone(),
                self.rules(),
            )
            .map_err(error)?;
            for a in &self.actions[..=entry] {
                Self::apply_record(&mut begin, a)?;
            }
            Some(self.digest_campaign_with_actions(&begin, &self.actions[..=entry])?)
        } else {
            None
        };
        if anchor != self.frozen_role_anchor {
            return Err("frozen role observation boundary mismatch".into());
        }
        if campaign.world().world_version().to_decimal() != self.world_version
            || campaign.world().quarter().value() != self.quarter
            || Self::phase_of(&campaign) != self.phase
            || self.digest_campaign(&campaign)? != self.private_state_sha256
        {
            return Err("private campaign fold does not match checkpoint".into());
        }
        Ok(campaign)
    }
    pub fn verify(&self) -> Result<(), String> {
        self.verified_campaign().map(|_| ())
    }
    /// P6 typed player intent, derived from the authoritative frozen world.
    /// This is validated again by the world reducer and the existing P3
    /// CommandEnvelope parser. It does not authorize minister actions.
    pub fn selected_player_command(
        &self,
        command_id: &str,
        body: Value,
    ) -> Result<GameAction, String> {
        if !myth_contracts::valid_public_id(command_id) {
            return Err("invalid connected command ID".into());
        }
        let kind = body["kind"].as_str().ok_or("missing typed command kind")?;
        let valid_phase = match (self.phase, kind) {
            (Phase::QuarterOpen, "defer") if self.quarter > 1 && !self.continuation_pending() => {
                true
            }
            (Phase::QuarterOpen, "propose" | "order_audit" | "respond")
                if !self.continuation_pending() =>
            {
                true
            }
            // The real headlines of a term are frozen into the record in its first quarter, before anything is planned
            // (decision 0013). The domain rules check it again, and that it happens only once.
            (Phase::QuarterOpen, "import_news")
                if self.quarter % d::TERM_QUARTERS == 1 && !self.continuation_pending() =>
            {
                true
            }
            (Phase::AmendmentWindow, "amend") => true,
            (Phase::ElectionNight, "begin_term") => true,
            (Phase::FinalPositionsFrozen, "ratify" | "defer")
                if self.final_votes_this_meeting() == 3 =>
            {
                true
            }
            _ => false,
        };
        if !valid_phase {
            return Err("player command not eligible at current phase".into());
        }
        if let Some(meeting) = body["meeting_id"].as_str()
            && meeting != self.meeting_id()
        {
            return Err("command meeting differs from active meeting".into());
        }
        let mut selected = self.planned(PLAYER, &self.verified_campaign()?)?;
        selected.kind = ActionKind::PlayerCommand;
        selected.payload = json!({
            "schema_version":1,
            "campaign_id":self.campaign_id,
            "command_id":command_id,
            "expected_world_version":self.world_version,
            "command":body,
        });
        parse_command(&selected.payload.to_string()).map_err(error)?;
        Ok(selected)
    }
    /// Only a recorded Q2+ Defer at QuarterOpen authorizes PERIOD to skip a meeting.
    pub fn continuation_pending(&self) -> bool {
        self.phase == Phase::QuarterOpen
            && self.actions.last().is_some_and(|record| {
                record.actor == PLAYER.0 as u8
                    && record.kind == ActionKind::PlayerCommand
                    && record.payload["command"]["kind"] == "defer"
            })
    }
    pub fn is_closed(&self) -> bool {
        self.phase == Phase::QuarterClosed
    }
    fn meeting_id(&self) -> String {
        format!("q{}", self.quarter)
    }
    fn current_meeting_actions(&self) -> impl Iterator<Item = &GameAction> {
        let boundary = self
            .actions
            .iter()
            .rposition(|a| {
                a.kind == ActionKind::PlayerCommand && a.payload["command"]["kind"] == "propose"
            })
            .map_or(0, |i| i + 1);
        self.actions[boundary..]
            .iter()
            .filter(|a| a.kind == ActionKind::MinisterAction)
    }
    pub fn minister_acted(&self, actor: AgentId) -> bool {
        let kind = if self.phase == Phase::FirstPositionsFrozen {
            "first"
        } else {
            "final"
        };
        self.current_meeting_actions()
            .any(|r| u64::from(r.actor) == actor.0 && r.payload["stage"] == kind)
    }
    pub fn remaining_ministers(&self) -> Vec<AgentId> {
        crate::ACTORS[1..4]
            .iter()
            .copied()
            .filter(|a| !self.minister_acted(*a))
            .collect()
    }
    pub fn final_votes_this_meeting(&self) -> usize {
        self.current_meeting_actions()
            .filter(|r| r.payload["stage"] == "final")
            .count()
    }
    fn planned(&self, actor: AgentId, campaign: &Campaign) -> Result<GameAction, String> {
        let meeting = self.meeting_id();
        let kind: ActionKind;
        let payload: Value;
        let mut role_observation = None;
        if actor == PLAYER {
            if self.continuation_pending() {
                return Err("continuation already chosen; only PERIOD may advance".into());
            }
            kind = ActionKind::PlayerCommand;
            if self.phase == Phase::AmendmentWindow {
                return Ok(GameAction {
                    actor: actor.0 as u8,
                    kind: ActionKind::FreezeFinal,
                    payload: json!({"command_id":format!("{meeting}-freeze")}),
                    observation_sha256: self.private_state_sha256.clone(),
                    role_observation: None,
                    model_receipt_digest: None,
                });
            }
            let body = match self.phase {
                Phase::QuarterOpen => json!({"kind":"propose","meeting_id":meeting,
                "allocation":if self.quarter==1 {
                    json!({"staff":50,"repairs":40,"access":10})
                } else {
                    json!({"staff":0,"repairs":0,"access":0})
                }}),
                Phase::ElectionNight => json!({"kind":"begin_term"}),
                Phase::FinalPositionsFrozen => {
                    if self.final_votes_this_meeting() != 3 {
                        return Err("must finish frozen final votes before player decision".into());
                    }
                    let supporting = self
                        .current_meeting_actions()
                        .filter(|r| {
                            r.payload["stage"] == "final" && r.payload["position"] == "support"
                        })
                        .count();
                    if supporting >= 2 {
                        json!({"kind":"ratify","meeting_id":meeting,
                            "proposal_version":campaign.proposal_version()
                                .ok_or("meeting lost version")?.to_decimal()})
                    } else {
                        json!({"kind":"defer"})
                    }
                }
                _ => return Err("player not eligible in this phase".into()),
            };
            payload = json!({"schema_version":1,"campaign_id":self.campaign_id,
                "command_id":format!("{meeting}-player-{}",self.world_version),
                "expected_world_version":self.world_version,"command":body});
        } else if actor == PERIOD {
            if self.phase != Phase::Resolved && !self.continuation_pending() {
                return Err(
                    "period actor requires resolved meeting or journaled continuation".into(),
                );
            }
            kind = ActionKind::PlayerCommand;
            payload = json!({"schema_version":1,"campaign_id":self.campaign_id,
                "command_id":format!("{meeting}-advance"),
                "expected_world_version":self.world_version,
                "command":{"kind":"advance_quarter"}});
        } else {
            let role = actor_role(actor).ok_or("unknown cabinet actor")?;
            if !matches!(
                self.phase,
                Phase::FirstPositionsFrozen | Phase::FinalPositionsFrozen
            ) {
                return Err("minister actor not eligible".into());
            }
            let first = self.phase == Phase::FirstPositionsFrozen;
            if self.minister_acted(actor) {
                return Err("minister already acted in frozen round".into());
            }
            let view = campaign.role_observation(role).map_err(error)?;
            role_observation = Some(
                serde_json::to_value(
                    myth_contracts::PublicRoleView::try_from(&view).map_err(error)?,
                )
                .map_err(error)?,
            );
            let decision = scripted::decide(&view).map_err(error)?;
            // Frozen round identity is a typed Phase, not free-form strings.
            let preference = if first {
                decision.first.preferred_allocation.allocation()
            } else {
                decision.final_position.preferred_allocation.allocation()
            };
            let position = if first {
                decision.first.position
            } else {
                decision.final_position.vote
            };
            kind = ActionKind::MinisterAction;
            payload = if first {
                let origin = match &decision.first.counterproposal_source {
                    d::CounterproposalOrigin::Menu(id) => json!({"kind":"menu","menu_id":id}),
                    d::CounterproposalOrigin::NewTyped => json!({"kind":"new_typed"}),
                };
                json!({"stage":"first","schema_version":1,"meeting_id":meeting,
                    "proposal_version":view.proposal_version.to_decimal(),
                    "ministry":myth_contracts::Ministry::from(role),"position":wire_vote(position),
                    "preferred_allocation":allocation_value(preference),
                    "counterproposal_source":origin,"conditions":[],"evidence_ids":[],
                    "rationale_summary":"Deterministic P0 profile decision"})
            } else {
                let mut output = json!({"stage":"final","schema_version":1,
                    "meeting_id":meeting,"proposal_version":view.proposal_version.to_decimal(),
                    "ministry":myth_contracts::Ministry::from(role),"position":wire_vote(position),
                    "preferred_allocation":allocation_value(preference),
                    "conditions":[],"evidence_ids":[],
                    "rationale_summary":"Deterministic P0 profile final vote"});
                if role != d::Ministry::Finance {
                    output["implementation_effort_permille"] = json!(1000);
                }
                output
            };
        }
        Ok(GameAction {
            actor: u8::try_from(actor.0).map_err(error)?,
            kind,
            payload,
            observation_sha256: if actor_role(actor).is_some() {
                self.frozen_role_anchor
                    .clone()
                    .ok_or("missing frozen role anchor")?
            } else {
                self.private_state_sha256.clone()
            },
            role_observation,
            model_receipt_digest: None,
        })
    }
}
impl World for GameWorld {
    type Action = GameAction;
    type Observation = GameObservation;
    fn observe(&self, actor: AgentId) -> Result<Self::Observation, String> {
        let campaign = self.verified_campaign()?;
        let action = self.planned(actor, &campaign)?;
        Ok(GameObservation {
            schema_version: 1,
            synthetic_receipts: self.synthetic_receipts,
            actor: u8::try_from(actor.0).map_err(error)?,
            phase: self.phase,
            quarter: self.quarter,
            expected_world_version: self.world_version.clone(),
            action,
        })
    }
    fn apply(&mut self, actor: AgentId, action: GameAction) -> Result<(), String> {
        // Hiyal takes an undo image before applying to the world. The local
        // path validates all rules against a temporary *Campaign*, not a
        // second full private GameWorld clone.
        let mut campaign = self.verified_campaign()?;
        if u64::from(action.actor) != actor.0 {
            return Err("actor mismatch".into());
        }
        let planned = self.planned(actor, &campaign)?;
        if action.model_receipt_digest.is_none() {
            if action != planned {
                if actor != PLAYER
                    || action.kind != ActionKind::PlayerCommand
                    || action.actor != planned.actor
                    || action.observation_sha256 != planned.observation_sha256
                    || action.role_observation.is_some()
                    || action.payload["schema_version"] != 1
                    || action.payload["campaign_id"] != self.campaign_id
                    || action.payload["expected_world_version"] != self.world_version
                {
                    return Err("action differs from frozen typed observation".into());
                }
                // The game issues only a typed, phase-eligible player command;
                // P3 will enforce budget, proposal version and funding rules.
                let id = action.payload["command_id"]
                    .as_str()
                    .ok_or("missing connected command id")?;
                let candidate =
                    self.selected_player_command(id, action.payload["command"].clone())?;
                if candidate != action {
                    return Err("changed connected player command".into());
                }
            }
        } else {
            let digest = action.model_receipt_digest.as_deref().unwrap_or_default();
            if action.kind != ActionKind::MinisterAction
                || actor_role(actor).is_none()
                || digest.len() != 64
                || !digest.bytes().all(|b| b.is_ascii_hexdigit())
                || action.actor != planned.actor
                || action.observation_sha256 != planned.observation_sha256
                || action.role_observation != planned.role_observation
            {
                return Err("model action lacked matching frozen role receipt".into());
            }
            // Defense in depth: a syntactically valid digest is not proof of
            // receipt authenticity. Enforce frozen role/stage/version and the
            // complete typed action contract again at the world boundary.
            // The Hiyal journal must be inspected separately to bind the
            // digest to an actual receipt; world logic never trusts the hash
            // alone to authorize an arbitrary minister payload.
            let frozen = GameObservation {
                schema_version: 1,
                synthetic_receipts: self.synthetic_receipts,
                actor: action.actor,
                phase: self.phase,
                quarter: self.quarter,
                expected_world_version: self.world_version.clone(),
                action: planned.clone(),
            };
            crate::p5_cabinet::checked_action(&frozen, &action.payload.to_string())?;
        }
        if self.actions.len() >= MAX_ACCEPTED_ACTIONS {
            return Err("action cap exceeded".into());
        }
        Self::apply_record(&mut campaign, &action)?;
        let before = self.phase;
        self.actions.push(action);
        if let Err(e) = self.refresh(&campaign) {
            self.actions.pop();
            return Err(e);
        }
        self.frozen_role_anchor = if matches!(
            self.phase,
            Phase::FirstPositionsFrozen | Phase::FinalPositionsFrozen
        ) {
            if self.phase != before {
                Some(self.private_state_sha256.clone())
            } else {
                self.frozen_role_anchor.clone()
            }
        } else {
            None
        };
        Ok(())
    }
}

#[cfg(test)]
mod p7_period_authorization_tests {
    use super::*;

    #[test]
    fn period_cannot_advance_an_unresolved_open_quarter_without_player_choice() {
        let world = GameWorld::initial("p7-world-guard").unwrap();
        assert_eq!(world.phase, Phase::QuarterOpen);
        assert!(!world.continuation_pending());
        assert!(world.observe(PERIOD).is_err());
        assert!(
            world
                .selected_player_command("skip-q1", json!({"kind":"defer"}))
                .is_err()
        );
    }
}

#[cfg(test)]
mod electorate_tests {
    use super::*;

    #[test]
    fn the_scenario_file_describes_exactly_the_populations_the_engine_defaults_to() {
        // The engine's built-in pilot (used by its own tests) and the population read from the frozen scenario are one
        // thing: attributes, cell shares, named groups and kernel. A change to either alone fails here.
        assert_eq!(population().unwrap(), &Population::pilot().unwrap());
    }

    #[test]
    fn a_new_world_carries_the_scenarios_cells_and_a_scenario_without_a_group_the_news_names_is_refused()
     {
        let world = GameWorld::initial("electorate-check").unwrap();
        assert_eq!(world.weights_ppm.len(), population().unwrap().cells());
        let mut scenario: Value = serde_json::from_str(SCENARIO).unwrap();
        let groups = scenario["electorate"]["groups"].as_array_mut().unwrap();
        groups.retain(|g| g["id"] != "middle_income");
        let refused = electorate(&scenario).unwrap_err();
        assert!(refused.contains("InvalidCondition"), "{refused}");
    }
}
