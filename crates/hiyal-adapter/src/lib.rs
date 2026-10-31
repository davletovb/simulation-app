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
const MAX_ACCEPTED_ACTIONS: usize = 512;
const MAX_ACTION_BYTES: usize = 12_288;
const PACK: &str = include_str!("../../../data/england-school-pilot-v1/country-pack.json");
const SCENARIO: &str = include_str!("../../../data/england-school-pilot-v1/authored.json");
const MANIFEST: &str = include_str!("../../../data/england-school-pilot-v1/manifest.json");

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// A deliberate, migration-controlled behavior contract, not raw source text.
/// Bump when P3 rules, parsing, adapters, scenario selection or model policy change.
pub const RULES_VERSION: &str = "p4-england-education-rules-v1";
type FrozenSource = Result<(String, String, [u32; 6]), String>;
static FROZEN: OnceLock<FrozenSource> = OnceLock::new();
fn frozen() -> Result<&'static (String, String, [u32; 6]), String> {
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
            let cells = scenario["population"]["variants"][0]["weights_ppm"]
                .as_array()
                .ok_or("missing authored population")?;
            if cells.len() != 6 {
                return Err("invalid cohort count".into());
            }
            let mut weights = [0u32; 6];
            for (i, value) in cells.iter().enumerate() {
                weights[i] =
                    u32::try_from(value.as_u64().ok_or("invalid cohort weight")?).map_err(error)?;
            }
            Ok((pack.into(), scenario_hash.into(), weights))
        })
        .as_ref()
        .map_err(Clone::clone)
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
    pub weights_ppm: [u32; 6],
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
        let (expected_pack, expected_scenario, weights) = frozen()?;
        let mut world = Self {
            schema_version: 1,
            campaign_id: campaign_id.into(),
            pack_id: "england-school-delivery-v1".into(),
            pack_sha256: expected_pack.clone(),
            scenario_sha256: expected_scenario.clone(),
            rules_identity: RULES_VERSION.into(),
            weights_ppm: *weights,
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
        let (pack, scenario, weights) = frozen()?;
        if &self.pack_sha256 != pack
            || &self.scenario_sha256 != scenario
            || self.weights_ppm != *weights
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
        let mut campaign = Campaign::with_rules(
            &self.campaign_id,
            &self.pack_id,
            d::CohortWeights::new(self.weights_ppm).map_err(error)?,
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
            let mut begin = Campaign::with_rules(
                &self.campaign_id,
                &self.pack_id,
                d::CohortWeights::new(self.weights_ppm).map_err(error)?,
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
        if !myth_contracts::valid_public_id(command_id)
        {
            return Err("invalid connected command ID".into());
        }
        let kind = body["kind"].as_str().ok_or("missing typed command kind")?;
        let valid_phase = match (self.phase, kind) {
            (Phase::QuarterOpen, "propose") => true,
            (Phase::AmendmentWindow, "amend") => true,
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
            if self.phase != Phase::Resolved {
                return Err("period actor not eligible".into());
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
