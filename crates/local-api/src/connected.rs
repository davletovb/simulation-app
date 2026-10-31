//! P6 offline connected campaign owner. No LLM routing occurs in this module.
//! A durable command acceptance record is written before HTTP 202. Only one
//! task can advance a given Hiyal trial; no duplicated command ID can apply.
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use futures_util::stream;
use myth_contracts::Allocation as WireAllocation;
use myth_domain::PolicyFamily;
use myth_hiyal_adapter::{
    ActionKind, GameWorld,
    orchestrator::{
        GameTrial, advance_slice, advance_unmodified_quarter, create_campaign, inspect, resume,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path as FsPath, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex as AsyncMutex, broadcast};

const MAX_COMMANDS: usize = 512;
const FROZEN_NEWS: &str = include_str!("../../../data/england-school-pilot-v1/news-bundle.json");
const FROZEN_COUNTRY: &str =
    include_str!("../../../data/england-school-pilot-v1/country-pack.json");
const FROZEN_VALUES: &str = include_str!("../../../data/england-school-pilot-v1/values.json");
const MAX_COMMAND_BYTES: usize = 1024 * 1024;
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn status(code: StatusCode, message: &'static str) -> (StatusCode, Json<Value>) {
    (code, Json(json!({"error":message})))
}
type Failure = (StatusCode, Json<Value>);
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub command_id: String,
    pub expected_world_version: String,
    pub kind: CommandKind,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandKind {
    AdvanceSlice,
    ContinueQuarter,
    Propose {
        allocation: WireAllocation,
    },
    Amend {
        allocation: WireAllocation,
    },
    Decide {
        ratify: bool,
    },
    OrderAudit {
        family: myth_contracts::Family,
        inspected_quarter: u8,
    },
}
impl CommandKind {
    fn body(&self, world: &GameWorld) -> Result<Option<Value>, String> {
        let meeting = format!("q{}", world.quarter);
        let proposal_version = || -> Result<String, String> {
            Ok(world
                .fold()?
                .proposal_version()
                .ok_or("no frozen proposal version")?
                .to_decimal())
        };
        Ok(match self {
            Self::AdvanceSlice => None,
            Self::ContinueQuarter => None,
            Self::Propose { allocation } => Some(json!({
                "kind":"propose","meeting_id":meeting,"allocation":allocation,
            })),
            Self::Amend { allocation } => Some(json!({
                "kind":"amend","meeting_id":meeting,
                "proposal_version":proposal_version()?,
                "allocation":allocation,
            })),
            Self::Decide { ratify: true } => Some(json!({
                "kind":"ratify","meeting_id":meeting,
                "proposal_version":proposal_version()?,
            })),
            Self::Decide { ratify: false } => Some(json!({"kind":"defer"})),
            Self::OrderAudit {
                family,
                inspected_quarter,
            } => Some(json!({
                "kind": "order_audit",
                "family": family,
                "inspected_quarter": inspected_quarter,
            })),
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Accepted,
    Completed,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub campaign_id: String,
    pub command_id: String,
    pub request_sha256: String,
    pub expected_world_version: String,
    pub kind: CommandKind,
    pub status: OperationStatus,
    pub committed_cursor: Option<String>,
    pub message: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    schema_version: u16,
    campaign_id: String,
    records: Vec<Operation>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: Log,
    sha256: String,
}
fn read_log(path: &FsPath, id: &str) -> Result<Log, String> {
    if !path.exists() {
        return Ok(Log {
            schema_version: 1,
            campaign_id: id.into(),
            records: vec![],
        });
    }
    let bytes = fs::read(path).map_err(|_| "cannot read command log")?;
    if bytes.len() > MAX_COMMAND_BYTES {
        return Err("oversized command log".into());
    }
    let wrapped: Envelope =
        serde_json::from_slice(&bytes).map_err(|_| "invalid durable command log")?;
    let hash = digest(&serde_json::to_vec(&wrapped.payload).map_err(|_| "hash failed")?);
    if wrapped.sha256 != hash
        || wrapped.payload.schema_version != 1
        || wrapped.payload.campaign_id != id
        || wrapped.payload.records.len() > MAX_COMMANDS
        || wrapped.payload.records.iter().any(|r| {
            r.campaign_id != id
                || !valid_id(&r.command_id)
                || r.request_sha256.len() != 64
                || !r.request_sha256.bytes().all(|c| c.is_ascii_hexdigit())
        })
    {
        return Err("command log integrity or scope mismatch".into());
    }
    let mut ids = std::collections::BTreeSet::new();
    if wrapped
        .payload
        .records
        .iter()
        .any(|r| !ids.insert(&r.command_id))
    {
        return Err("duplicate durable command identifier".into());
    }
    Ok(wrapped.payload)
}
fn persist_log(path: &FsPath, log: &Log) -> Result<(), String> {
    let payload = serde_json::to_vec(log).map_err(|_| "cannot serialize command log")?;
    let bytes = serde_json::to_vec(&Envelope {
        payload: serde_json::from_slice(&payload).map_err(|_| "cannot validate log")?,
        sha256: digest(&payload),
    })
    .map_err(|_| "cannot serialize envelope")?;
    if bytes.len() > MAX_COMMAND_BYTES {
        return Err("command log capacity exhausted".into());
    }
    let temp = path.with_extension("commands-new");
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temp)
        .map_err(|_| "cannot stage command log")?;
    f.write_all(&bytes).map_err(|_| "cannot persist command")?;
    f.sync_all().map_err(|_| "cannot sync accepted command")?;
    fs::rename(&temp, path).map_err(|_| "cannot publish accepted command")?;
    #[cfg(unix)]
    File::open(path.parent().ok_or("missing command directory")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| "cannot sync command directory")?;
    Ok(())
}
/// A fork is a distinct local command scope, preserving the Hiyal genesis,
/// recorded effects and shock streams up to a verified decision boundary.
/// It is offline-only; inherited receipts must never be used to fund live API.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ForkMeta {
    schema_version: u16,
    branch_id: String,
    source_campaign_id: String,
    source_cursor: String,
    source_state_sha256: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkRequest {
    branch_id: String,
    expected_cursor: String,
    expected_world_version: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ForkReceipt {
    branch_id: String,
    source_campaign_id: String,
    source_cursor: String,
    source_state_sha256: String,
    shock_schedule: String,
}
fn fork_receipt(meta: &ForkMeta) -> ForkReceipt {
    ForkReceipt {
        branch_id: meta.branch_id.clone(),
        source_campaign_id: meta.source_campaign_id.clone(),
        source_cursor: meta.source_cursor.clone(),
        source_state_sha256: meta.source_state_sha256.clone(),
        shock_schedule: "exact Hiyal genesis and stream state inherited".into(),
    }
}
fn read_fork_meta(dir: &FsPath) -> Result<Option<ForkMeta>, String> {
    let file = dir.join("p6-fork-origin.json");
    if !file.exists() {
        return Ok(None);
    }
    let bytes = fs::read(file).map_err(|_| "cannot read fork provenance")?;
    if bytes.len() > 4096 {
        return Err("oversized fork provenance".into());
    }
    let meta: ForkMeta = serde_json::from_slice(&bytes).map_err(|_| "invalid fork provenance")?;
    if meta.schema_version != 1
        || !valid_id(&meta.branch_id)
        || !valid_id(&meta.source_campaign_id)
        || meta.branch_id == meta.source_campaign_id
        || meta.source_cursor.parse::<u64>().is_err()
        || meta.source_state_sha256.len() != 64
        || !meta
            .source_state_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err("unsafe fork provenance".into());
    }
    Ok(Some(meta))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicVote {
    ministry: String,
    stage: String,
    position: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicTreasury {
    pub available_credits: u16,
    pub encumbered_staff: u16,
    pub encumbered_repairs: u16,
    pub encumbered_access: u16,
    pub service_spent: u16,
    pub audit_spent: u16,
}
/// Only public, explicitly recorded player decisions are projected. The
/// complete private P3 world, latent support, actor beliefs and efforts are
/// not serialized into this browser-facing report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicDecision {
    pub quarter: u8,
    pub outcome: String,
    pub allocation: Option<Value>,
    pub final_support: u8,
    pub final_votes: u8,
}
/// Public P3 report only: reported activity and schedule, not private actual
/// delivery or hidden minister implementation effort.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicDelivery {
    pub quarter: u8,
    pub reported_cumulative: [u16; 3],
    pub due_cumulative: [u16; 3],
    pub weighted_support_ppm: u32,
    pub staff_slowdown_next: bool,
    pub access_bonus_next: bool,
}
/// Exact recorded-action replay is distinct from a new scripted fork.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicNews {
    pub date: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub url: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewsBundle {
    schema_version: u16,
    cutoff_utc: String,
    items: Vec<PublicNews>,
    label: String,
}
fn frozen_news(cutoff: &str) -> Result<Vec<PublicNews>, String> {
    let bundle: NewsBundle =
        serde_json::from_str(FROZEN_NEWS).map_err(|_| "invalid news bundle")?;
    if bundle.schema_version != 1
        || bundle.cutoff_utc != cutoff
        || bundle.label.is_empty()
        || bundle.items.len() > 12
    {
        return Err("news bundle/country cutoff mismatch".into());
    }
    for item in &bundle.items {
        let real = item.kind == "real_source_update";
        let fiction = item.kind == "fictional_simulation_news";
        if !(real || fiction)
            || item.title.is_empty()
            || item.summary.is_empty()
            || (real
                && (item.date.as_str() > cutoff.get(..10).unwrap_or("")
                    || !item
                        .url
                        .as_deref()
                        .is_some_and(|url| url.starts_with("https://"))))
            || (fiction && item.url.is_some())
        {
            return Err("unsafe or unclassified archived news".into());
        }
    }
    Ok(bundle.items)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicComparison {
    pub mode: String,
    pub source_campaign_id: Option<String>,
    pub source_cursor: Option<String>,
    pub inherited_shock_stream: bool,
    pub post_fork_actor_actions_fixed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicIndicator {
    pub variable_id: String,
    pub definition: String,
    pub scope: String,
    pub publisher: String,
    pub reference_period: String,
    pub display_value: String,
    pub unit: String,
    pub source_url: String,
    pub quality_flags: Vec<String>,
}
#[derive(Deserialize)]
struct SourcePack {
    schema_version: u16,
    cutoff_utc: String,
    evidence: Vec<SourceFact>,
}
#[derive(Deserialize)]
struct SourceFact {
    stable_variable_id: String,
    definition: String,
    geographic_scope: String,
    publisher: String,
    reference_period: String,
    source_url: String,
    normalized_unit: String,
}
#[derive(Deserialize)]
struct SourceValues {
    schema_version: u16,
    cutoff_utc: String,
    records: Vec<SourceValue>,
}
#[derive(Deserialize)]
struct SourceValue {
    stable_variable_id: String,
    raw_value: String,
    normalized_unit: String,
    quality_flags: Vec<String>,
}
fn source_indicators(cutoff: &str) -> Result<Vec<PublicIndicator>, String> {
    let pack: SourcePack =
        serde_json::from_str(FROZEN_COUNTRY).map_err(|_| "cannot parse frozen country source")?;
    let values: SourceValues =
        serde_json::from_str(FROZEN_VALUES).map_err(|_| "cannot parse frozen source values")?;
    if pack.schema_version != 1
        || values.schema_version != 1
        || pack.cutoff_utc != cutoff
        || values.cutoff_utc != cutoff
        || pack.evidence.len() != 8
        || values.records.len() != 8
    {
        return Err("country evidence contract mismatch".into());
    }
    let indexed: BTreeMap<&str, &SourceValue> = values
        .records
        .iter()
        .map(|entry| (entry.stable_variable_id.as_str(), entry))
        .collect();
    if indexed.len() != 8 {
        return Err("duplicate frozen source value IDs".into());
    }
    let mut indicators = Vec::new();
    for source in pack.evidence {
        let v = indexed
            .get(source.stable_variable_id.as_str())
            .ok_or("missing matching frozen source value")?;
        if source.normalized_unit != v.normalized_unit
            || !source.source_url.starts_with("https://")
            || source.definition.is_empty()
            || source.geographic_scope.is_empty()
            || v.raw_value.is_empty()
        {
            return Err("unverified frozen evidence metadata".into());
        }
        indicators.push(PublicIndicator {
            variable_id: source.stable_variable_id,
            definition: source.definition,
            scope: source.geographic_scope,
            publisher: source.publisher,
            reference_period: source.reference_period,
            display_value: v.raw_value.clone(),
            unit: v.normalized_unit.clone(),
            source_url: source.source_url,
            quality_flags: v.quality_flags.clone(),
        });
    }
    if indicators
        .iter()
        .map(|v| v.variable_id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != 8
    {
        return Err("duplicate frozen source IDs".into());
    }
    Ok(indicators)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicEvidence {
    pub case_pack: String,
    pub source_cutoff_utc: String,
    pub evidence_pack_sha256: String,
    pub scenario_sha256: String,
    pub attribution: String,
    pub scope: String,
    pub indicators: Vec<PublicIndicator>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicView {
    pub schema_version: u16,
    pub campaign_id: String,
    pub trial_id: String,
    pub cursor: String,
    pub view_version: String,
    pub quarter: u8,
    pub phase: String,
    pub closed: bool,
    pub proposal: Option<Value>,
    pub votes: Vec<PublicVote>,
    pub treasury: PublicTreasury,
    pub decisions: Vec<PublicDecision>,
    pub delivery_history: Vec<PublicDelivery>,
    pub news: Vec<PublicNews>,
    pub comparison: PublicComparison,
    pub evidence: PublicEvidence,
    pub outcome: Option<String>,
    pub assumption: String,
    pub mode: &'static str,
}
/// Important: caller owns exclusive mutable trial lock. Inspection validates
/// Hiyal's committed journal and captures cursor + public view at the SAME
/// boundary. Never read cursor after releasing this lock.
fn capture(trial: &GameTrial) -> Result<PublicView, String> {
    trial.store.world().verify()?;
    let (hash, _chain, sequence) = inspect(&trial.store.dir)?;
    let world: &GameWorld = trial.store.world();
    if world.private_state_sha256 != hash {
        return Err("replay/world boundary mismatch".into());
    }
    let mut proposal = None;
    let mut outcome = None;
    let mut votes: Vec<PublicVote> = Vec::new();
    let mut decisions = Vec::new();
    let mut last_quarter = 1u8;
    for action in &world.actions {
        if action.kind == ActionKind::PlayerCommand {
            let body = &action.payload["command"];
            match body["kind"].as_str() {
                Some("propose") => {
                    if let Some(q) = body["meeting_id"]
                        .as_str()
                        .and_then(|s| s.strip_prefix('q'))
                        .and_then(|s| s.parse::<u8>().ok())
                    {
                        last_quarter = q;
                    }
                    proposal = Some(body["allocation"].clone());
                    outcome = None;
                    votes.clear();
                }
                Some("amend") => proposal = Some(body["allocation"].clone()),
                Some("ratify" | "defer") => {
                    let kind = body["kind"].as_str().unwrap_or("defer");
                    outcome = Some(
                        if kind == "ratify" {
                            "ratified"
                        } else {
                            "deferred"
                        }
                        .into(),
                    );
                    let relevant: Vec<_> = votes.iter().filter(|v| v.stage == "final").collect();
                    decisions.push(PublicDecision {
                        quarter: last_quarter,
                        outcome: outcome.clone().unwrap_or_default(),
                        allocation: proposal.clone(),
                        final_support: relevant
                            .iter()
                            .filter(|v| v.position == "support")
                            .count()
                            .try_into()
                            .map_err(|_| "too many public final votes")?,
                        final_votes: relevant
                            .len()
                            .try_into()
                            .map_err(|_| "too many final votes")?,
                    });
                }
                _ => {}
            }
        } else if action.kind == ActionKind::MinisterAction
            && let (Some(ministry), Some(stage), Some(position)) = (
                action.payload["ministry"].as_str(),
                action.payload["stage"].as_str(),
                action.payload["position"].as_str(),
            )
        {
            votes.push(PublicVote {
                ministry: ministry.into(),
                stage: stage.into(),
                position: position.into(),
            });
        }
    }
    // The new quarter has no active proposal or current cabinet votes.
    // Do not display last quarter's ratified proposal as an active draft.
    if world.phase == myth_contracts::checkpoint::Phase::QuarterOpen {
        proposal = None;
        votes.clear();
        outcome = None;
    }
    if decisions.len() > 6 {
        return Err("unexpected number of public decisions".into());
    }
    let evidence = PublicEvidence {
        case_pack: world.pack_id.clone(),
        source_cutoff_utc: world.source_cutoff_utc.clone(),
        evidence_pack_sha256: world.pack_sha256.clone(),
        scenario_sha256: world.scenario_sha256.clone(),
        attribution: "Versioned synthetic case data and deterministic game rules, not current news"
            .into(),
        scope: "Fictional England school-delivery case; not a forecast of actual policy outcomes"
            .into(),
        indicators: source_indicators(&world.source_cutoff_utc)?,
    };
    let campaign = world.fold()?;
    let treasury = campaign.world().treasury();
    let delivery_history: Vec<PublicDelivery> = campaign
        .player_reports()
        .into_iter()
        .map(|report| PublicDelivery {
            quarter: report.quarter.value(),
            reported_cumulative: report.reported_cumulative,
            due_cumulative: report.due_cumulative,
            weighted_support_ppm: report.weighted_support_ppm,
            staff_slowdown_next: report.staff_slowdown_next,
            access_bonus_next: report.access_bonus_next,
        })
        .collect();
    if delivery_history.len() > 6
        || delivery_history
            .iter()
            .enumerate()
            .any(|(i, report)| usize::from(report.quarter) != i + 1)
    {
        return Err("invalid public quarterly report sequence".into());
    }
    let fiscal = PublicTreasury {
        available_credits: treasury.available(),
        encumbered_staff: treasury.encumbered(PolicyFamily::Staffing),
        encumbered_repairs: treasury.encumbered(PolicyFamily::Repairs),
        encumbered_access: treasury.encumbered(PolicyFamily::Access),
        service_spent: treasury.service_spent(),
        audit_spent: treasury.audit_spent(),
    };
    Ok(PublicView{
        schema_version:1,
        campaign_id:world.campaign_id.clone(),
        trial_id:world.campaign_id.clone(),
        cursor:sequence.to_string(),
        view_version:world.world_version.clone(),
        quarter:world.quarter,
        phase:format!("{:?}",world.phase),
        closed:world.is_closed(),
        proposal,
        votes,
        treasury:fiscal,
        decisions,
        delivery_history,
        news: frozen_news(&world.source_cutoff_utc)?,
        comparison: PublicComparison {
            mode: "exact_recorded_actions".into(),
            source_campaign_id: None,
            source_cursor: None,
            inherited_shock_stream: true,
            post_fork_actor_actions_fixed: true,
        },
        evidence,
        outcome,
        assumption:"Synthetic education-delivery rules; scripted cabinet, not live ministers or empirical predictions".into(),
        mode:"offline_scripted",
    })
}
/// Hiyal's original campaign/trial identity is preserved in the journal;
/// the API's logical branch ID is distinct and scopes commands/SSE.
fn capture_for(trial: &GameTrial, id: &str) -> Result<PublicView, String> {
    let mut view = capture(trial)?;
    view.campaign_id = id.into();
    view.trial_id = trial.store.genesis().trial_id.clone();
    if let Some(fork) = read_fork_meta(&trial.store.dir)? {
        if fork.branch_id != id {
            return Err("fork identity mismatch".into());
        }
        view.comparison = PublicComparison {
            mode: "fresh_scripted_branch".into(),
            source_campaign_id: Some(fork.source_campaign_id),
            source_cursor: Some(fork.source_cursor),
            inherited_shock_stream: true,
            post_fork_actor_actions_fixed: false,
        };
    }
    Ok(view)
}
#[derive(Clone, Serialize)]
pub struct Committed {
    pub snapshot: PublicView,
    pub operation: Option<Operation>,
}
struct Engine {
    trial: GameTrial,
    log: Log,
    poisoned: bool,
}
struct Owner {
    engine: AsyncMutex<Engine>,
    inflight: AtomicBool,
    sender: broadcast::Sender<Committed>,
    log_path: PathBuf,
    id: String,
    // Keep the process-wide OS lock alive while any background command
    // worker or campaign owner remains reachable.
    _writer_lock: Arc<File>,
}
pub struct Connected {
    root: PathBuf,
    owners: Mutex<BTreeMap<String, Arc<Owner>>>,
    _writer_lock: Arc<File>,
}
impl Connected {
    pub fn new(root: PathBuf) -> Result<Arc<Self>, String> {
        fs::create_dir_all(&root).map_err(|_| "cannot make private campaign root")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|_| "cannot protect campaign root")?;
        }
        // File::try_lock is stable on the project's pinned Rust 1.99 toolchain.
        // Its advisory OS lock remains held until the *last* owner/worker
        // disappears, and is automatically released after a process crash.
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(".p6-writer.lock"))
            .map_err(|_| "cannot create local single-writer lock")?;
        lock_file
            .try_lock()
            .map_err(|_| "another simulation API process already owns this save directory")?;
        let _writer_lock = Arc::new(lock_file);
        Ok(Arc::new(Self {
            root,
            owners: Mutex::new(BTreeMap::new()),
            _writer_lock,
        }))
    }
    fn owner(self: &Arc<Self>, id: &str) -> Result<Arc<Owner>, String> {
        if !valid_id(id) {
            return Err("invalid campaign id".into());
        }
        let mut map = self.owners.lock().map_err(|_| "owner registry poisoned")?;
        if let Some(held) = map.get(id) {
            return Ok(held.clone());
        }
        let path = self.root.join(id);
        if !path.exists() && self.root.join(format!("{id}.commands.json")).exists() {
            return Err("command history exists but Hiyal trial is missing".into());
        }
        let trial = if path.exists() {
            resume(&path, false)?
        } else {
            create_campaign(&path, id, 43, id)?
        };
        if trial.store.world().campaign_id != id {
            let provenance = read_fork_meta(&path)?.ok_or("missing fork provenance")?;
            if provenance.branch_id != id
                || provenance.source_campaign_id != trial.store.world().campaign_id
                || inspect(&path)?.2
                    < provenance
                        .source_cursor
                        .parse::<u64>()
                        .map_err(|_| "invalid fork source cursor")?
            {
                return Err("fork origin mismatch or truncated journal".into());
            }
        } else if read_fork_meta(&path)?.is_some() {
            return Err("fork provenance may not overwrite a primary campaign".into());
        }
        let log_path = self.root.join(format!("{id}.commands.json"));
        let log = read_log(&log_path, id)?;
        let (sender, _) = broadcast::channel(64);
        let owner = Arc::new(Owner {
            engine: AsyncMutex::new(Engine {
                trial,
                log,
                poisoned: false,
            }),
            inflight: AtomicBool::new(false),
            sender,
            log_path,
            id: id.to_owned(),
            _writer_lock: self._writer_lock.clone(),
        });
        map.insert(id.into(), owner.clone());
        if owner.engine.try_lock().is_ok_and(|g| {
            g.log
                .records
                .iter()
                .any(|r| r.status == OperationStatus::Accepted)
        }) {
            schedule(owner.clone());
        }
        Ok(owner)
    }
}
fn schedule(owner: Arc<Owner>) {
    if owner
        .inflight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    tokio::spawn(async move {
        loop {
            let mut engine = owner.engine.lock().await;
            if engine.poisoned {
                break;
            }
            let Some(pos) = engine
                .log
                .records
                .iter()
                .position(|r| r.status == OperationStatus::Accepted)
            else {
                break;
            };
            let expected = engine.log.records[pos].expected_world_version.clone();
            let current = engine.trial.store.world().world_version.clone();
            // Accepted before dispatch; recovering after the Hiyal journal
            // committed but before command-log completion must not step twice.
            let pending_round = engine.trial.store.dir.join("pending-round.json").exists();
            let result = if current != expected && !pending_round {
                Ok(())
            } else {
                let body = engine.log.records[pos]
                    .kind
                    .body(engine.trial.store.world());
                let command_id = engine.log.records[pos].command_id.clone();
                let prepared = match body {
                    Ok(Some(payload)) => engine.trial.select_player_command(&command_id, payload),
                    Ok(None) => Ok(()),
                    Err(error) => Err(error),
                };
                let applied = match prepared {
                    Ok(()) if engine.log.records[pos].kind == CommandKind::ContinueQuarter => {
                        advance_unmodified_quarter(&mut engine.trial).await
                    }
                    Ok(()) => advance_slice(&mut engine.trial).await.map(|_| ()),
                    Err(error) => Err(error),
                };
                engine.trial.clear_player_command();
                applied
            };
            let snap = capture_for(&engine.trial, &owner.id);
            let (completion, cursor) = match (result, snap) {
                (Ok(()), Ok(snap)) => (Ok(snap.clone()), Some(snap.cursor)),
                (Err(_), _) | (_, Err(_)) => (
                    Err("operation stopped: inspect durable Hiyal journal"),
                    None,
                ),
            };
            let old = engine.log.records[pos].clone();
            engine.log.records[pos].status = if completion.is_ok() {
                OperationStatus::Completed
            } else {
                OperationStatus::Failed
            };
            engine.log.records[pos].committed_cursor = cursor;
            if completion.is_err() {
                engine.log.records[pos].message = Some("Offline step failed; inspect trial".into());
            }
            if persist_log(&owner.log_path, &engine.log).is_err() {
                engine.poisoned = true;
                break;
            }
            if let Ok(snap) = completion {
                let _ = owner.sender.send(Committed {
                    snapshot: snap,
                    operation: Some(engine.log.records[pos].clone()),
                });
            } else {
                let _ = owner.sender.send(Committed {
                    snapshot: capture_for(&engine.trial, &owner.id).unwrap_or(PublicView {
                        schema_version: 1,
                        campaign_id: old.campaign_id.clone(),
                        trial_id: old.campaign_id.clone(),
                        cursor: "0".into(),
                        view_version: old.expected_world_version,
                        quarter: 0,
                        phase: "IntegrityFailure".into(),
                        closed: false,
                        proposal: None,
                        votes: vec![],
                        treasury: PublicTreasury {
                            available_credits: 0,
                            encumbered_staff: 0,
                            encumbered_repairs: 0,
                            encumbered_access: 0,
                            service_spent: 0,
                            audit_spent: 0,
                        },
                        decisions: vec![],
                        delivery_history: vec![],
                        news: vec![],
                        comparison: PublicComparison {
                            mode: "verification_failed".into(),
                            source_campaign_id: None,
                            source_cursor: None,
                            inherited_shock_stream: false,
                            post_fork_actor_actions_fixed: false,
                        },
                        evidence: PublicEvidence {
                            case_pack: "unavailable".into(),
                            source_cutoff_utc: "unavailable".into(),
                            evidence_pack_sha256: "unavailable".into(),
                            scenario_sha256: "unavailable".into(),
                            attribution: "Verification failed".into(),
                            scope: "No authenticated case evidence available".into(),
                            indicators: vec![],
                        },
                        outcome: None,
                        assumption: "Offline verification failed".into(),
                        mode: "offline_scripted",
                    }),
                    operation: Some(engine.log.records[pos].clone()),
                });
            }
        }
        owner.inflight.store(false, Ordering::Release);
        // Close the acceptance/worker-exit race: a command may have been
        // fsynced while the previous worker still held inflight=true.
        if owner
            .engine
            .lock()
            .await
            .log
            .records
            .iter()
            .any(|r| r.status == OperationStatus::Accepted)
        {
            schedule(owner);
        }
    });
}
async fn snapshot(
    State(service): State<Arc<Connected>>,
    Path(id): Path<String>,
) -> Result<Json<Committed>, Failure> {
    let owner = service
        .owner(&id)
        .map_err(|_| status(StatusCode::CONFLICT, "campaign unavailable"))?;
    let engine = owner.engine.lock().await;
    Ok(Json(Committed {
        snapshot: capture_for(&engine.trial, &owner.id)
            .map_err(|_| status(StatusCode::CONFLICT, "verification failed"))?,
        operation: None,
    }))
}
async fn operation(
    State(service): State<Arc<Connected>>,
    Path((id, command_id)): Path<(String, String)>,
) -> Result<Json<Operation>, Failure> {
    let owner = service
        .owner(&id)
        .map_err(|_| status(StatusCode::CONFLICT, "campaign unavailable"))?;
    let engine = owner.engine.lock().await;
    let found = engine
        .log
        .records
        .iter()
        .find(|r| r.command_id == command_id)
        .ok_or_else(|| status(StatusCode::NOT_FOUND, "command not found"))?;
    Ok(Json(found.clone()))
}
async fn command(
    State(service): State<Arc<Connected>>,
    Path(id): Path<String>,
    Json(request): Json<Command>,
) -> Result<(StatusCode, Json<Operation>), Failure> {
    if !valid_id(&request.command_id)
        || request.expected_world_version.is_empty()
        || request.expected_world_version.len() > 40
    {
        return Err(status(
            StatusCode::BAD_REQUEST,
            "invalid command id or version",
        ));
    }
    let owner = service
        .owner(&id)
        .map_err(|_| status(StatusCode::CONFLICT, "campaign unavailable"))?;
    let encoded = serde_json::to_vec(&request)
        .map_err(|_| status(StatusCode::BAD_REQUEST, "invalid command"))?;
    let hash = digest(&encoded);
    let mut engine = owner.engine.lock().await;
    if engine.poisoned {
        return Err(status(StatusCode::CONFLICT, "campaign writer unavailable"));
    }
    if let Some(old) = engine
        .log
        .records
        .iter()
        .find(|r| r.command_id == request.command_id)
    {
        if old.request_sha256 != hash {
            return Err(status(
                StatusCode::CONFLICT,
                "command id reused with changed content",
            ));
        }
        return Ok((StatusCode::OK, Json(old.clone())));
    }
    if engine.log.records.len() >= MAX_COMMANDS {
        return Err(status(StatusCode::CONFLICT, "command journal full"));
    }
    if engine
        .log
        .records
        .iter()
        .any(|r| r.status == OperationStatus::Accepted)
    {
        return Err(status(
            StatusCode::CONFLICT,
            "another command is awaiting execution",
        ));
    }
    if engine.trial.store.world().world_version != request.expected_world_version {
        return Err(status(StatusCode::CONFLICT, "stale expected world version"));
    }
    if engine.trial.store.world().is_closed() {
        return Err(status(StatusCode::CONFLICT, "campaign closed"));
    }
    if request.kind == CommandKind::ContinueQuarter
        && engine.trial.store.world().phase != myth_contracts::checkpoint::Phase::QuarterOpen
    {
        return Err(status(
            StatusCode::CONFLICT,
            "cannot continue an active cabinet meeting",
        ));
    }
    if let Some(body) = request
        .kind
        .body(engine.trial.store.world())
        .map_err(|_| status(StatusCode::CONFLICT, "command lacks a frozen proposal"))?
    {
        engine
            .trial
            .store
            .world()
            .selected_player_command(&request.command_id, body)
            .map_err(|_| {
                status(
                    StatusCode::CONFLICT,
                    "typed player action invalid at this phase",
                )
            })?;
    }
    let record = Operation {
        campaign_id: id,
        command_id: request.command_id,
        request_sha256: hash,
        expected_world_version: request.expected_world_version,
        kind: request.kind,
        status: OperationStatus::Accepted,
        committed_cursor: None,
        message: None,
    };
    engine.log.records.push(record.clone());
    // Must fsync BOTH acceptance and associated Hiyal genesis before 202.
    if persist_log(&owner.log_path, &engine.log).is_err() {
        engine.poisoned = true;
        return Err(status(
            StatusCode::INTERNAL_SERVER_ERROR,
            "durable acceptance uncertain",
        ));
    }
    drop(engine);
    schedule(owner);
    Ok((StatusCode::ACCEPTED, Json(record)))
}
/// Subscribe first, then capture the verified world and operation while
/// holding the one-writer lock. Any concurrent commit is either included
/// in this snapshot, or queued in the receiver as a subsequent frame.
async fn subscribed_boundary(
    owner: &Arc<Owner>,
) -> Result<(Committed, broadcast::Receiver<Committed>), String> {
    let receiver = owner.sender.subscribe();
    let engine = owner.engine.lock().await;
    let committed = Committed {
        snapshot: capture_for(&engine.trial, &owner.id)?,
        operation: engine.log.records.last().cloned(),
    };
    Ok((committed, receiver))
}
async fn stream_events(
    State(service): State<Arc<Connected>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Failure> {
    let owner = service
        .owner(&id)
        .map_err(|_| status(StatusCode::CONFLICT, "campaign unavailable"))?;
    // The same tested join is used for initial and Last-Event-ID fallback.
    let (committed, live) = subscribed_boundary(&owner)
        .await
        .map_err(|_| status(StatusCode::CONFLICT, "verification failed"))?;
    let last = headers.get("last-event-id").and_then(|h| h.to_str().ok());
    let id_value = format!(
        "{}:{}",
        committed.snapshot.campaign_id, committed.snapshot.cursor
    );
    // Always send a same-boundary snapshot. Duplicate cursor is harmless and
    // client-side reducer explicitly ignores older/duplicate versions.
    let first = Event::default()
        .event(if last == Some(id_value.as_str()) {
            "snapshot"
        } else {
            "snapshot_fallback"
        })
        .id(id_value)
        .json_data(committed)
        .map_err(|_| status(StatusCode::INTERNAL_SERVER_ERROR, "cannot encode snapshot"))?;
    let joined = stream::unfold((Some(first), live), |(initial, mut receiver)| async move {
        if let Some(e) = initial {
            return Some((Ok::<Event, Infallible>(e), (None, receiver)));
        }
        match receiver.recv().await {
            Ok(event) => {
                let id = format!("{}:{}", event.snapshot.campaign_id, event.snapshot.cursor);
                let frame = Event::default()
                    .event("committed")
                    .id(id)
                    .json_data(event)
                    .ok()?;
                Some((Ok(frame), (None, receiver)))
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {
                // A lagged subscriber must reconnect and obtain a fresh
                // validated fallback rather than silently skip a transition.
                None
            }
            Err(broadcast::error::RecvError::Closed) => None,
        }
    });
    Ok(Sse::new(joined).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}
async fn fork(
    State(service): State<Arc<Connected>>,
    Path(id): Path<String>,
    Json(request): Json<ForkRequest>,
) -> Result<(StatusCode, Json<ForkReceipt>), Failure> {
    use myth_contracts::checkpoint::Phase;
    if !valid_id(&request.branch_id) || request.branch_id == id || request.branch_id.len() + 5 > 64
    {
        return Err(status(
            StatusCode::BAD_REQUEST,
            "invalid fork branch identity",
        ));
    }
    let owner = service
        .owner(&id)
        .map_err(|_| status(StatusCode::CONFLICT, "source unavailable"))?;
    let engine = owner.engine.lock().await;
    if engine.poisoned
        || engine.trial.store.world().phase != Phase::Resolved
        || engine.trial.store.dir.join("pending-round.json").exists()
        || engine
            .log
            .records
            .iter()
            .any(|r| r.status == OperationStatus::Accepted)
    {
        return Err(status(
            StatusCode::CONFLICT,
            "not a verified quiescent decision boundary",
        ));
    }
    let boundary = capture_for(&engine.trial, &owner.id)
        .map_err(|_| status(StatusCode::CONFLICT, "source replay verification failed"))?;
    if boundary.cursor != request.expected_cursor
        || boundary.view_version != request.expected_world_version
    {
        return Err(status(StatusCode::CONFLICT, "fork boundary is stale"));
    }
    let source = &engine.trial.store.dir;
    let source_hash = inspect(source)
        .map_err(|_| status(StatusCode::CONFLICT, "source journal invalid"))?
        .0;
    let meta = ForkMeta {
        schema_version: 1,
        branch_id: request.branch_id.clone(),
        source_campaign_id: engine.trial.store.world().campaign_id.clone(),
        source_cursor: boundary.cursor,
        source_state_sha256: source_hash,
    };
    let dst = service.root.join(&request.branch_id);
    if dst.exists() {
        let existing = read_fork_meta(&dst)
            .map_err(|_| status(StatusCode::CONFLICT, "existing branch invalid"))?;
        if existing.as_ref().is_some_and(|v| {
            v.branch_id == meta.branch_id
                && v.source_campaign_id == meta.source_campaign_id
                && v.source_cursor == meta.source_cursor
                && v.source_state_sha256 == meta.source_state_sha256
        }) {
            return Ok((StatusCode::OK, Json(fork_receipt(&meta))));
        }
        return Err(status(
            StatusCode::CONFLICT,
            "branch already exists at another boundary",
        ));
    }
    // Do not copy unverified output, temp artifacts, locks or auth secrets.
    // Only the Hiyal manifest, exact journal and optional verified snapshot
    // and receipts are eligible for a bounded local offline fork.
    let stage = service
        .root
        .join(format!(".{}.fork-stage", request.branch_id));
    fs::create_dir(&stage).map_err(|_| {
        status(
            StatusCode::CONFLICT,
            "fork staging exists or cannot be created",
        )
    })?;
    for name in [
        "manifest.json",
        "events.jsonl",
        "effects.jsonl",
        "snapshot.json",
    ] {
        let original = source.join(name);
        if !original.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&original)
            .map_err(|_| status(StatusCode::CONFLICT, "fork source metadata unavailable"))?;
        if !metadata.file_type().is_file() || metadata.len() > 64 * 1024 * 1024 {
            return Err(status(
                StatusCode::CONFLICT,
                "unsafe source artifact for fork",
            ));
        }
        let target = stage.join(name);
        fs::copy(&original, &target).map_err(|_| {
            status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "cannot copy fork journal",
            )
        })?;
        File::open(&target)
            .and_then(|file| file.sync_all())
            .map_err(|_| {
                status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "cannot sync fork journal",
                )
            })?;
    }
    let raw = serde_json::to_vec(&meta).map_err(|_| {
        status(
            StatusCode::INTERNAL_SERVER_ERROR,
            "cannot encode fork metadata",
        )
    })?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(stage.join("p6-fork-origin.json"))
        .map_err(|_| {
            status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "cannot stage fork metadata",
            )
        })?;
    file.write_all(&raw)
        .and_then(|_| file.sync_all())
        .map_err(|_| {
            status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "cannot sync fork provenance",
            )
        })?;
    #[cfg(unix)]
    File::open(&stage).and_then(|f| f.sync_all()).map_err(|_| {
        status(
            StatusCode::INTERNAL_SERVER_ERROR,
            "cannot sync fork staging",
        )
    })?;
    // Verify the copied committed boundary before publishing it.
    let clone_hash = inspect(&stage).map_err(|_| {
        status(
            StatusCode::CONFLICT,
            "copied journal did not strictly replay",
        )
    })?;
    if clone_hash.0 != meta.source_state_sha256 || clone_hash.2.to_string() != meta.source_cursor {
        return Err(status(StatusCode::CONFLICT, "fork hash or cursor mismatch"));
    }
    fs::rename(&stage, &dst)
        .map_err(|_| status(StatusCode::CONFLICT, "fork publication failed"))?;
    #[cfg(unix)]
    File::open(&service.root)
        .and_then(|f| f.sync_all())
        .map_err(|_| {
            status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "cannot sync branch directory",
            )
        })?;
    Ok((StatusCode::CREATED, Json(fork_receipt(&meta))))
}
pub fn routes() -> Router<Arc<Connected>> {
    Router::new()
        .route("/api/campaigns/{id}/snapshot", get(snapshot))
        .route("/api/campaigns/{id}/commands", post(command))
        .route("/api/campaigns/{id}/commands/{command_id}", get(operation))
        .route("/api/campaigns/{id}/events", get(stream_events))
        .route("/api/campaigns/{id}/forks", post(fork))
}
#[cfg(test)]
mod tests {
    use super::*;
    use myth_hiyal_adapter::orchestrator::run_until_closed;
    fn root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "p6-connected-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn one_save_root_allows_only_one_os_writer() {
        let path = root();
        let first = Connected::new(path.clone()).unwrap();
        assert!(Connected::new(path.clone()).is_err());
        drop(first);
        let recovered = Connected::new(path.clone()).unwrap();
        drop(recovered);
        fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn durable_idempotency_and_scoped_recovery() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let first = service.owner("alpha").unwrap();
        let second = service.owner("beta").unwrap();
        let version = first
            .engine
            .lock()
            .await
            .trial
            .store
            .world()
            .world_version
            .clone();
        let request = Command {
            command_id: "command-one".into(),
            expected_world_version: version,
            kind: CommandKind::AdvanceSlice,
        };
        let (_, Json(accepted)) = command(
            State(service.clone()),
            Path("alpha".into()),
            Json(request.clone()),
        )
        .await
        .unwrap();
        assert_eq!(accepted.status, OperationStatus::Accepted);
        let (code, Json(retried)) = command(
            State(service.clone()),
            Path("alpha".into()),
            Json(request.clone()),
        )
        .await
        .unwrap();
        assert!(matches!(code, StatusCode::OK | StatusCode::ACCEPTED));
        assert_eq!(retried.request_sha256, accepted.request_sha256);
        let mut changed = request.clone();
        changed.expected_world_version = "forged".into();
        assert!(
            command(State(service.clone()), Path("alpha".into()), Json(changed))
                .await
                .is_err()
        );
        assert_eq!(
            second.engine.lock().await.trial.store.world().world_version,
            "0"
        );
        for _ in 0..80 {
            if first.engine.lock().await.log.records[0].status == OperationStatus::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert_eq!(
            first.engine.lock().await.log.records[0].status,
            OperationStatus::Completed
        );
        let cursor = capture(&first.engine.lock().await.trial).unwrap().cursor;
        drop(first);
        drop(second);
        drop(service);
        let recovered = Connected::new(dir.clone()).unwrap();
        let owner = recovered.owner("alpha").unwrap();
        assert_eq!(
            capture(&owner.engine.lock().await.trial).unwrap().cursor,
            cursor
        );
        let (status, Json(_)) = command(
            State(recovered.clone()),
            Path("alpha".into()),
            Json(request),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert!(recovered.owner("../bad").is_err());
        drop(owner);
        drop(recovered);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn initial_and_fallback_join_never_skip_a_concurrent_commit() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let owner = service.owner("race").unwrap();
        // Commit holds the writer lock when the subscriber attaches: the
        // subscriber registers immediately, and captures after the commit.
        let guard = owner.engine.lock().await;
        let joining = {
            let owner = owner.clone();
            tokio::spawn(async move { subscribed_boundary(&owner).await.unwrap() })
        };
        tokio::task::yield_now().await;
        drop(guard);
        let after = accepted_step(&service, "race", CommandKind::AdvanceSlice, 1).await;
        let (joined, mut live) = joining.await.unwrap();
        let joined_cursor = joined.snapshot.cursor.parse::<u64>().unwrap();
        let after_cursor = after.cursor.parse::<u64>().unwrap();
        assert!(joined_cursor <= after_cursor);
        if joined_cursor < after_cursor {
            // A commit after capture must be present in the subscribed feed.
            let update = tokio::time::timeout(Duration::from_secs(3), live.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(update.snapshot.cursor, after.cursor);
            assert_eq!(update.snapshot.view_version, after.view_version);
        } else {
            assert_eq!(joined.snapshot.view_version, after.view_version);
        }
        // Last-Event-ID fallback gets the *same* deliberately contended
        // subscribe -> lock -> capture race as the initial connection.
        let lock = owner.engine.lock().await;
        let reconnect = {
            let owner = owner.clone();
            tokio::spawn(async move { subscribed_boundary(&owner).await.unwrap() })
        };
        tokio::task::yield_now().await;
        drop(lock);
        let next = accepted_step(&service, "race", CommandKind::AdvanceSlice, 2).await;
        let (fallback, mut subsequent) = reconnect.await.unwrap();
        let fallback_cursor = fallback.snapshot.cursor.parse::<u64>().unwrap();
        let next_cursor = next.cursor.parse::<u64>().unwrap();
        assert!(fallback_cursor >= after_cursor);
        assert!(fallback_cursor <= next_cursor);
        if fallback_cursor < next_cursor {
            let event = tokio::time::timeout(Duration::from_secs(3), subsequent.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(event.snapshot.cursor, next.cursor);
            assert_eq!(event.snapshot.view_version, next.view_version);
        } else {
            assert_eq!(fallback.snapshot.view_version, next.view_version);
        }
        drop(live);
        drop(subsequent);
        drop(joined);
        drop(owner);
        drop(service);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn verified_decision_fork_inherits_shocks_but_diverges_independently() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let allocation = WireAllocation {
            staff: 50,
            repairs: 40,
            access: 10,
        };
        accepted_step(&service, "source", CommandKind::Propose { allocation }, 1).await;
        accepted_step(&service, "source", CommandKind::AdvanceSlice, 2).await;
        accepted_step(&service, "source", CommandKind::AdvanceSlice, 3).await;
        accepted_step(&service, "source", CommandKind::AdvanceSlice, 4).await;
        let resolved =
            accepted_step(&service, "source", CommandKind::Decide { ratify: true }, 5).await;
        assert_eq!(resolved.phase, "Resolved");
        let request = ForkRequest {
            branch_id: "alternate".into(),
            expected_cursor: resolved.cursor.clone(),
            expected_world_version: resolved.view_version.clone(),
        };
        let (created, Json(receipt)) = fork(
            State(service.clone()),
            Path("source".into()),
            Json(request.clone()),
        )
        .await
        .unwrap();
        assert_eq!(created, StatusCode::CREATED);
        assert_eq!(receipt.source_campaign_id, "source");
        let (duplicate, Json(again)) =
            fork(State(service.clone()), Path("source".into()), Json(request))
                .await
                .unwrap();
        assert_eq!(duplicate, StatusCode::OK);
        assert_eq!(again.source_state_sha256, receipt.source_state_sha256);
        let original = service.owner("source").unwrap();
        let branch = service.owner("alternate").unwrap();
        let before = inspect(&original.engine.lock().await.trial.store.dir).unwrap();
        assert_eq!(
            before,
            inspect(&branch.engine.lock().await.trial.store.dir).unwrap()
        );
        assert_eq!(
            capture_for(&branch.engine.lock().await.trial, "alternate")
                .unwrap()
                .campaign_id,
            "alternate"
        );
        // The cloned manifest carries the exact exogenous schedule while
        // the command and journal namespaces remain independent.
        assert_eq!(
            serde_json::to_value(
                &original
                    .engine
                    .lock()
                    .await
                    .trial
                    .store
                    .genesis()
                    .initial_streams
            )
            .unwrap(),
            serde_json::to_value(
                &branch
                    .engine
                    .lock()
                    .await
                    .trial
                    .store
                    .genesis()
                    .initial_streams
            )
            .unwrap()
        );
        let fork_advanced =
            accepted_step(&service, "alternate", CommandKind::AdvanceSlice, 1).await;
        assert_eq!(fork_advanced.quarter, 2);
        assert_eq!(
            inspect(&original.engine.lock().await.trial.store.dir).unwrap(),
            before
        );
        assert_ne!(
            inspect(&branch.engine.lock().await.trial.store.dir).unwrap(),
            before
        );
        drop(branch);
        drop(original);
        drop(service);
        let recovered = Connected::new(dir.clone()).unwrap();
        assert_eq!(
            capture_for(
                &recovered
                    .owner("alternate")
                    .unwrap()
                    .engine
                    .lock()
                    .await
                    .trial,
                "alternate"
            )
            .unwrap()
            .cursor,
            fork_advanced.cursor
        );
        drop(recovered);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn crash_after_hiyal_commit_before_command_status_rebuilds_once() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let owner = service.owner("interrupted").unwrap();
        let (original, next_cursor) = {
            let mut engine = owner.engine.lock().await;
            let world_version = engine.trial.store.world().world_version.clone();
            let command = Command {
                command_id: "durable-interrupted".into(),
                expected_world_version: world_version.clone(),
                kind: CommandKind::AdvanceSlice,
            };
            let encoded = serde_json::to_vec(&command).unwrap();
            engine.log.records.push(Operation {
                campaign_id: "interrupted".into(),
                command_id: command.command_id,
                request_sha256: digest(&encoded),
                expected_world_version: world_version,
                kind: command.kind,
                status: OperationStatus::Accepted,
                committed_cursor: None,
                message: None,
            });
            persist_log(&owner.log_path, &engine.log).unwrap();
            // Hiyal committed and synced, but the command's Completed status
            // has not been persisted. This models the crash boundary.
            advance_slice(&mut engine.trial).await.unwrap();
            let cursor = capture(&engine.trial).unwrap().cursor;
            (
                engine.trial.store.world().private_state_sha256.clone(),
                cursor,
            )
        };
        drop(owner);
        drop(service);
        let restarted = Connected::new(dir.clone()).unwrap();
        let recovered = restarted.owner("interrupted").unwrap();
        for _ in 0..120 {
            if recovered.engine.lock().await.log.records[0].status == OperationStatus::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let engine = recovered.engine.lock().await;
        assert_eq!(engine.log.records[0].status, OperationStatus::Completed);
        assert_eq!(
            engine.log.records[0].committed_cursor.as_deref(),
            Some(next_cursor.as_str())
        );
        assert_eq!(engine.trial.store.world().private_state_sha256, original);
        assert_eq!(capture(&engine.trial).unwrap().cursor, next_cursor);
        drop(engine);
        drop(recovered);
        drop(restarted);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn typed_wire_money_rejects_fractional_negative_and_huge_values() {
        let valid = serde_json::json!({
            "command_id":"whole-credits",
            "expected_world_version":"9007199254740993",
            "kind":{"kind":"propose",
                "allocation":{"staff":50,"repairs":40,"access":10}}
        });
        let parsed: Command = serde_json::from_value(valid.clone()).unwrap();
        assert_eq!(parsed.expected_world_version, "9007199254740993");
        assert_eq!(serde_json::to_value(&parsed).unwrap(), valid);
        for bad in [
            serde_json::json!(50.0),
            serde_json::json!(50.1),
            serde_json::json!(-1),
            serde_json::json!("50"),
            serde_json::json!(9007199254740993u64),
            serde_json::Value::Null,
        ] {
            let mut request = valid.clone();
            request["kind"]["allocation"]["staff"] = bad;
            assert!(serde_json::from_value::<Command>(request).is_err());
        }
    }
    #[tokio::test]
    async fn fsynced_acceptance_before_dispatch_recovers_once() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let owner = service.owner("accepted-crash").unwrap();
        let original = Command {
            command_id: "accepted-before-dispatch".into(),
            expected_world_version: "0".into(),
            kind: CommandKind::AdvanceSlice,
        };
        {
            let mut engine = owner.engine.lock().await;
            assert_eq!(capture(&engine.trial).unwrap().cursor, "0");
            engine.log.records.push(Operation {
                campaign_id: "accepted-crash".into(),
                command_id: original.command_id.clone(),
                request_sha256: digest(&serde_json::to_vec(&original).unwrap()),
                expected_world_version: original.expected_world_version.clone(),
                kind: original.kind.clone(),
                status: OperationStatus::Accepted,
                committed_cursor: None,
                message: None,
            });
            persist_log(&owner.log_path, &engine.log).unwrap();
        }
        drop(owner);
        drop(service);
        let restarted = Connected::new(dir.clone()).unwrap();
        let writer = restarted.owner("accepted-crash").unwrap();
        for _ in 0..120 {
            if writer.engine.lock().await.log.records[0].status == OperationStatus::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let engine = writer.engine.lock().await;
        assert_eq!(engine.log.records[0].status, OperationStatus::Completed);
        assert_eq!(engine.trial.store.world().actions.len(), 1);
        let cursor = capture(&engine.trial).unwrap().cursor;
        drop(engine);
        let (code, Json(reply)) = command(
            State(restarted.clone()),
            Path("accepted-crash".into()),
            Json(original),
        )
        .await
        .unwrap();
        assert_eq!(code, StatusCode::OK);
        assert_eq!(reply.status, OperationStatus::Completed);
        assert_eq!(
            capture(&writer.engine.lock().await.trial).unwrap().cursor,
            cursor
        );
        drop(writer);
        drop(restarted);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn p6_offline_metrics_without_model_calls() {
        let dir = root();
        let started = std::time::Instant::now();
        let service = Connected::new(dir.clone()).unwrap();
        let owner = service.owner("metrics").unwrap();
        let create_ms = started.elapsed().as_millis();
        let preview_started = std::time::Instant::now();
        let allocation = WireAllocation {
            staff: 50,
            repairs: 40,
            access: 10,
        };
        let forecast = crate::forecast(crate::AllocationRequest {
            allocation: allocation.clone(),
        })
        .unwrap();
        assert_eq!(forecast.families.len(), 3);
        let preview_ms = preview_started.elapsed().as_millis();
        let commit_started = std::time::Instant::now();
        let result =
            accepted_step(&service, "metrics", CommandKind::Propose { allocation }, 1).await;
        let commit_ms = commit_started.elapsed().as_millis();
        assert_eq!(result.phase, "FirstPositionsFrozen");
        let replay_started = std::time::Instant::now();
        let verified = inspect(&owner.engine.lock().await.trial.store.dir).unwrap();
        let full_verification_ms = replay_started.elapsed().as_millis();
        assert_eq!(verified.2.to_string(), result.cursor);
        drop(owner);
        drop(service);
        let restore_started = std::time::Instant::now();
        let recovered = Connected::new(dir.clone()).unwrap();
        let reopened = recovered.owner("metrics").unwrap();
        let saved = capture(&reopened.engine.lock().await.trial).unwrap();
        let restore_ms = restore_started.elapsed().as_millis();
        assert_eq!(saved.cursor, result.cursor);
        println!(
            "{}",
            serde_json::json!({
                "gate":"P6 offline performance (not live Experiment C-prime)",
                "create_ms":create_ms,
                "preview_ms":preview_ms,
                "command_accept_commit_wait_ms":commit_ms,
                "strict_full_verification_ms":full_verification_ms,
                "restore_and_snapshot_ms":restore_ms,
                "actual_model_calls":0,
                "model_usage_or_billing_measured":false,
                "local_external_infrastructure_cost_usd":0,
            })
        );
        drop(reopened);
        drop(recovered);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn mid_round_receipt_panic_restarts_without_duplicate_minister_actions() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        accepted_step(
            &service,
            "mid-round",
            CommandKind::Propose {
                allocation: WireAllocation {
                    staff: 50,
                    repairs: 40,
                    access: 10,
                },
            },
            1,
        )
        .await;
        let owner = service.owner("mid-round").unwrap();
        owner.engine.lock().await.trial.policy.fail_role_once = Some((
            myth_hiyal_adapter::EDUCATION,
            Arc::new(AtomicBool::new(true)),
        ));
        let version = owner
            .engine
            .lock()
            .await
            .trial
            .store
            .world()
            .world_version
            .clone();
        let request = Command {
            command_id: "failed-frozen-meeting".into(),
            expected_world_version: version,
            kind: CommandKind::AdvanceSlice,
        };
        let (_, Json(accepted)) = command(
            State(service.clone()),
            Path("mid-round".into()),
            Json(request),
        )
        .await
        .unwrap();
        assert_eq!(accepted.status, OperationStatus::Accepted);
        // The intentional Hiyal test panic occurs after some receipts have
        // been fsynced but before the complete frozen round commits.
        for _ in 0..120 {
            if owner
                .engine
                .lock()
                .await
                .trial
                .store
                .dir
                .join("pending-round.json")
                .exists()
                && owner.inflight.load(Ordering::Acquire)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(
            owner
                .engine
                .lock()
                .await
                .trial
                .store
                .dir
                .join("pending-round.json")
                .exists()
        );
        // Let the deliberately panicking background task release the OS
        // writer locks before reopening this directory in the test process.
        tokio::time::sleep(Duration::from_millis(100)).await;
        // Simulate process disposal; the new owner must recover journal and
        // the durable accepted command without duplicating any seat.
        drop(owner);
        drop(service);
        let restored = Connected::new(dir.clone()).unwrap();
        let live = restored.owner("mid-round").unwrap();
        for _ in 0..160 {
            if live.engine.lock().await.log.records.iter().any(|r| {
                r.command_id == "failed-frozen-meeting" && r.status == OperationStatus::Completed
            }) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let world = live.engine.lock().await;
        assert_eq!(
            world.log.records.last().unwrap().status,
            OperationStatus::Completed
        );
        assert_eq!(
            world
                .trial
                .store
                .world()
                .actions
                .iter()
                .filter(|a| a.kind == ActionKind::MinisterAction)
                .count(),
            3
        );
        assert_eq!(capture(&world.trial).unwrap().votes.len(), 3);
        drop(world);
        drop(live);
        drop(restored);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn distinct_campaign_sse_and_command_namespaces_never_cross() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let alpha = service.owner("alpha").unwrap();
        let beta = service.owner("beta").unwrap();
        let (_alpha_before, mut alpha_rx) = subscribed_boundary(&alpha).await.unwrap();
        let (_beta_before, mut beta_rx) = subscribed_boundary(&beta).await.unwrap();
        let committed = accepted_step(&service, "beta", CommandKind::AdvanceSlice, 1).await;
        let beta_event = tokio::time::timeout(Duration::from_secs(3), beta_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(beta_event.snapshot.campaign_id, "beta");
        assert_eq!(beta_event.snapshot.cursor, committed.cursor);
        assert!(matches!(
            alpha_rx.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
        assert_eq!(
            capture(&alpha.engine.lock().await.trial).unwrap().cursor,
            "0"
        );
        assert_eq!(
            capture(&beta.engine.lock().await.trial).unwrap().cursor,
            committed.cursor
        );
        drop(alpha_rx);
        drop(beta_rx);
        drop(alpha);
        drop(beta);
        drop(service);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn command_identity_persists_through_lost_ack_and_restart() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let owner = service.owner("lost-ack").unwrap();
        let version = owner
            .engine
            .lock()
            .await
            .trial
            .store
            .world()
            .world_version
            .clone();
        let request = Command {
            command_id: "stable-ack".into(),
            expected_world_version: version,
            kind: CommandKind::AdvanceSlice,
        };
        let (_, Json(first)) = command(
            State(service.clone()),
            Path("lost-ack".into()),
            Json(request.clone()),
        )
        .await
        .unwrap();
        let (status, Json(second)) = command(
            State(service.clone()),
            Path("lost-ack".into()),
            Json(request.clone()),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(first.request_sha256, second.request_sha256);
        for _ in 0..120 {
            if owner.engine.lock().await.log.records[0].status == OperationStatus::Completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let before = capture(&owner.engine.lock().await.trial).unwrap().cursor;
        drop(owner);
        drop(service);
        let restarted = Connected::new(dir.clone()).unwrap();
        let reopened = restarted.owner("lost-ack").unwrap();
        let (status, Json(replayed)) = command(
            State(restarted.clone()),
            Path("lost-ack".into()),
            Json(request.clone()),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(replayed.status, OperationStatus::Completed);
        assert_eq!(
            capture(&reopened.engine.lock().await.trial).unwrap().cursor,
            before
        );
        let mut conflict = request;
        conflict.kind = CommandKind::Decide { ratify: false };
        assert!(
            command(
                State(restarted.clone()),
                Path("lost-ack".into()),
                Json(conflict)
            )
            .await
            .is_err()
        );
        drop(reopened);
        drop(restarted);
        fs::remove_dir_all(dir).unwrap();
    }
    async fn accepted_step(
        service: &Arc<Connected>,
        campaign: &str,
        kind: CommandKind,
        serial: usize,
    ) -> PublicView {
        let owner = service.owner(campaign).unwrap();
        let version = owner
            .engine
            .lock()
            .await
            .trial
            .store
            .world()
            .world_version
            .clone();
        let request = Command {
            command_id: format!("step-{serial}"),
            expected_world_version: version,
            kind,
        };
        let (status, Json(op)) = command(
            State(service.clone()),
            Path(campaign.to_owned()),
            Json(request),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::ACCEPTED);
        for _ in 0..160 {
            let status = owner
                .engine
                .lock()
                .await
                .log
                .records
                .iter()
                .find(|r| r.command_id == op.command_id)
                .unwrap()
                .status
                .clone();
            if status == OperationStatus::Completed {
                break;
            }
            if status == OperationStatus::Failed {
                panic!("Hiyal rejected P6 typed player step {serial}");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert_eq!(
            owner
                .engine
                .lock()
                .await
                .log
                .records
                .iter()
                .find(|r| r.command_id == op.command_id)
                .unwrap()
                .status,
            OperationStatus::Completed
        );
        capture(&owner.engine.lock().await.trial).unwrap()
    }
    #[tokio::test]
    async fn public_evidence_and_late_report_exclude_private_role_fields() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let opening = service.owner("report").unwrap();
        let initial = capture(&opening.engine.lock().await.trial).unwrap();
        assert!(initial.decisions.is_empty());
        assert!(!initial.evidence.evidence_pack_sha256.is_empty());
        assert_eq!(initial.evidence.source_cutoff_utc, "2026-10-29T23:59:59Z");
        drop(opening);
        accepted_step(
            &service,
            "report",
            CommandKind::Propose {
                allocation: WireAllocation {
                    staff: 50,
                    repairs: 40,
                    access: 10,
                },
            },
            1,
        )
        .await;
        accepted_step(&service, "report", CommandKind::AdvanceSlice, 2).await;
        accepted_step(&service, "report", CommandKind::AdvanceSlice, 3).await;
        let final_view = accepted_step(&service, "report", CommandKind::AdvanceSlice, 4).await;
        assert_eq!(final_view.votes.len(), 6);
        let result =
            accepted_step(&service, "report", CommandKind::Decide { ratify: true }, 5).await;
        assert_eq!(result.decisions.len(), 1);
        assert_eq!(result.decisions[0].final_votes, 3);
        assert_eq!(result.decisions[0].outcome, "ratified");
        let later = accepted_step(&service, "report", CommandKind::AdvanceSlice, 6).await;
        assert_eq!(later.phase, "QuarterOpen");
        assert_eq!(later.decisions.len(), 1);
        assert_eq!(later.delivery_history.len(), 1);
        assert_eq!(later.delivery_history[0].quarter, 1);
        assert_eq!(later.evidence.indicators.len(), 8);
        assert!(
            later
                .evidence
                .indicators
                .iter()
                .any(|item| item.variable_id == "teacher_fte" && item.display_value == "466,372")
        );
        assert_eq!(later.news.len(), 3);
        assert_eq!(later.news[0].kind, "real_source_update");
        assert_eq!(later.news[2].kind, "fictional_simulation_news");
        assert_eq!(later.comparison.mode, "exact_recorded_actions");
        assert!(later.comparison.post_fork_actor_actions_fixed);
        assert!(later.proposal.is_none());
        assert!(later.votes.is_empty());
        let payload = serde_json::to_string(&later).unwrap();
        for secret in [
            "private_state_sha256",
            "implementation_effort_permille",
            "role_observation",
            "belief_ppm",
            "weights_ppm",
            "OPENROUTER_API_KEY",
        ] {
            assert!(!payload.contains(secret), "private field escaped: {secret}");
        }
        drop(service);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn p7_audit_command_preserves_typed_family_and_quarter() {
        let world = GameWorld::initial("audit-wire").unwrap();
        let body = CommandKind::OrderAudit {
            family: myth_contracts::Family::Repairs,
            inspected_quarter: 2,
        }
        .body(&world)
        .unwrap()
        .unwrap();
        assert_eq!(
            body,
            json!({"kind":"order_audit","family":"repairs","inspected_quarter":2})
        );
    }
    #[test]
    fn p7_quarter_continuation_is_an_explicit_typed_player_action() {
        let world = GameWorld::initial("continue-wire").unwrap();
        assert_eq!(CommandKind::ContinueQuarter.body(&world).unwrap(), None);
    }
    #[tokio::test]
    async fn p7_two_fundable_resolutions_remain_distinct_when_programmes_continue() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let mut outcomes = Vec::new();
        for (id, allocation) in [
            (
                "balanced-campaign",
                WireAllocation {
                    staff: 50,
                    repairs: 40,
                    access: 10,
                },
            ),
            (
                "staff-campaign",
                WireAllocation {
                    staff: 60,
                    repairs: 30,
                    access: 10,
                },
            ),
        ] {
            let mut serial = 0usize;
            let mut step = |kind| {
                serial += 1;
                (kind, serial)
            };
            let (kind, n) = step(CommandKind::Propose { allocation });
            accepted_step(&service, id, kind, n).await;
            for _ in 0..3 {
                let (kind, n) = step(CommandKind::AdvanceSlice);
                accepted_step(&service, id, kind, n).await;
            }
            let view = service.owner(id).unwrap();
            assert_eq!(
                capture_for(&view.engine.lock().await.trial, id)
                    .unwrap()
                    .phase,
                "FinalPositionsFrozen"
            );
            drop(view);
            let (kind, n) = step(CommandKind::Decide { ratify: true });
            let passed = accepted_step(&service, id, kind, n).await;
            assert_eq!(passed.outcome.as_deref(), Some("ratified"));
            let (kind, n) = step(CommandKind::AdvanceSlice);
            assert_eq!(accepted_step(&service, id, kind, n).await.quarter, 2);
            for quarter in 2..=6 {
                let (kind, n) = step(CommandKind::ContinueQuarter);
                let next = accepted_step(&service, id, kind, n).await;
                assert_eq!(next.delivery_history.len(), quarter as usize);
            }
            let owner = service.owner(id).unwrap();
            let snapshot = capture_for(&owner.engine.lock().await.trial, id).unwrap();
            assert!(snapshot.closed);
            assert_eq!(snapshot.delivery_history.len(), 6);
            outcomes.push(snapshot.delivery_history[5].reported_cumulative);
            drop(owner);
        }
        assert_ne!(
            outcomes[0], outcomes[1],
            "distinct authorised policy allocations must give distinguishable public histories"
        );
        drop(service);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn p7_complete_scripted_campaign_reports_exactly_six_quarters_and_replays() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let owner = service.owner("six-report").unwrap();
        let final_view;
        let saved_hash;
        {
            let mut engine = owner.engine.lock().await;
            run_until_closed(&mut engine.trial).await.unwrap();
            final_view = capture_for(&engine.trial, "six-report").unwrap();
            assert!(final_view.closed);
            assert_eq!(final_view.quarter, 6);
            assert_eq!(final_view.delivery_history.len(), 6);
            assert_eq!(
                final_view
                    .delivery_history
                    .iter()
                    .map(|r| r.quarter)
                    .collect::<Vec<_>>(),
                vec![1, 2, 3, 4, 5, 6]
            );
            assert_eq!(final_view.news.len(), 3);
            assert_eq!(final_view.comparison.mode, "exact_recorded_actions");
            let t = &final_view.treasury;
            assert_eq!(
                u32::from(t.available_credits)
                    + u32::from(t.encumbered_staff)
                    + u32::from(t.encumbered_repairs)
                    + u32::from(t.encumbered_access)
                    + u32::from(t.service_spent)
                    + u32::from(t.audit_spent),
                100
            );
            saved_hash = inspect(&engine.trial.store.dir).unwrap().0;
        }
        drop(owner);
        drop(service);
        assert_eq!(inspect(&dir.join("six-report")).unwrap().0, saved_hash);
        let reopened = Connected::new(dir.clone()).unwrap();
        let restored = reopened.owner("six-report").unwrap();
        let view = capture_for(&restored.engine.lock().await.trial, "six-report").unwrap();
        assert_eq!(view.cursor, final_view.cursor);
        assert_eq!(view.view_version, final_view.view_version);
        assert_eq!(view.delivery_history.len(), 6);
        assert_eq!(
            view.treasury.service_spent,
            final_view.treasury.service_spent
        );
        drop(restored);
        drop(reopened);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn one_connected_typed_decision_survives_strict_journal_restore() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let alloc = WireAllocation {
            staff: 50,
            repairs: 40,
            access: 10,
        };
        let v1 = accepted_step(
            &service,
            "decision",
            CommandKind::Propose {
                allocation: alloc.clone(),
            },
            1,
        )
        .await;
        assert_eq!(v1.phase, "FirstPositionsFrozen");
        assert_eq!(
            v1.proposal,
            Some(json!({"staff":50,"repairs":40,"access":10}))
        );
        let first = accepted_step(&service, "decision", CommandKind::AdvanceSlice, 2).await;
        assert_eq!(first.phase, "AmendmentWindow");
        assert_eq!(first.votes.len(), 3);
        let amended = accepted_step(
            &service,
            "decision",
            CommandKind::Amend { allocation: alloc },
            3,
        )
        .await;
        // Amendment and final-position freeze are separate committed actions.
        assert_eq!(amended.phase, "AmendmentWindow");
        let frozen = accepted_step(&service, "decision", CommandKind::AdvanceSlice, 4).await;
        assert_eq!(frozen.phase, "FinalPositionsFrozen");
        let final_votes = accepted_step(&service, "decision", CommandKind::AdvanceSlice, 5).await;
        assert_eq!(final_votes.votes.len(), 6);
        let result = accepted_step(
            &service,
            "decision",
            CommandKind::Decide { ratify: true },
            6,
        )
        .await;
        assert_eq!(result.phase, "Resolved");
        assert_eq!(result.outcome.as_deref(), Some("ratified"));
        let saved = accepted_step(&service, "decision", CommandKind::AdvanceSlice, 7).await;
        assert_eq!(saved.quarter, 2);
        let before = inspect(
            &service
                .owner("decision")
                .unwrap()
                .engine
                .lock()
                .await
                .trial
                .store
                .dir,
        )
        .unwrap();
        drop(service);
        let strict = inspect(&dir.join("decision")).unwrap();
        assert_eq!(strict, before);
        let recovered = Connected::new(dir.clone()).unwrap();
        assert_eq!(
            capture(
                &recovered
                    .owner("decision")
                    .unwrap()
                    .engine
                    .lock()
                    .await
                    .trial
            )
            .unwrap()
            .cursor,
            saved.cursor
        );
        drop(recovered);
        fs::remove_dir_all(dir).unwrap();
    }
}
