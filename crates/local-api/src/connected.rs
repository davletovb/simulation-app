//! P6 offline connected campaign owner. No LLM routing occurs in this module.
//! A durable command acceptance record is written before HTTP 202. Only one
//! task can advance a given Hiyal trial; no duplicated command ID can apply.
use crate::real_news::NewsSource;
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
use myth_contracts::RealItemWire;
use myth_domain as d;
use myth_domain::PolicyFamily;
use myth_domain::reducer::{ELECTION_HELD_ABOVE_PPM, ELECTION_MIN_SUPPORT_PPM};
use myth_hiyal_adapter::{
    ActionKind, GameWorld, headline_text,
    orchestrator::{
        GameTrial, advance_slice, advance_unmodified_quarter, create_campaign, inspect, resume,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path as FsPath, PathBuf},
    sync::OnceLock,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex as AsyncMutex, broadcast};

// A full quarter is about eight commands, so this holds the longest game (MAX_TERMS terms) with room to spare.
const MAX_COMMANDS: usize = 1024;
const FROZEN_NEWS: &str = include_str!("../../../data/england-school-pilot-v1/news-bundle.json");
const FROZEN_COUNTRY: &str =
    include_str!("../../../data/england-school-pilot-v1/country-pack.json");
const FROZEN_VALUES: &str = include_str!("../../../data/england-school-pilot-v1/values.json");

/// Removes uncommitted fork staging on *all* error paths. A published fork
/// remains durable even if the following directory fsync fails.
struct ForkStageGuard {
    path: PathBuf,
    active: bool,
}
impl Drop for ForkStageGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

const MAX_CACHED_CAMPAIGNS: usize = 128;
const MAX_COMMAND_BYTES: usize = 1024 * 1024;
use myth_contracts::valid_public_id as valid_id;
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
    FinishContinuation,
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
    /// After a won election, start the next term. Never implied by a generic advance.
    BeginTerm,
    /// Pay to soften an announced bad headline that lands in `headline_quarter` (decision 0010).
    Respond {
        headline_quarter: u8,
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
            Self::ContinueQuarter => Some(json!({"kind":"defer"})),
            Self::FinishContinuation => None,
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
            Self::BeginTerm => Some(json!({"kind":"begin_term"})),
            Self::Respond { headline_quarter } => Some(json!({
                "kind": "respond",
                "headline_quarter": headline_quarter,
            })),
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
    /// Credits spent answering headlines (decision 0010).
    pub relief_spent: u16,
}
/// Only public, explicitly recorded player decisions are projected. The
/// complete private P3 world, latent support, actor beliefs and efforts are
/// not serialized into this browser-facing report.
///
/// `quarter` is the quarter in which the decision was made. `outcome` is
/// `ratified` or `deferred` for a board meeting, or `continued` when the player
/// kept the existing programme without a meeting; a continuation reports no
/// votes (`final_votes == 0`) and the allocation it carries forward.
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
/// A headline from the news calendar, announced a quarter ahead (decision 0010). Surprises are never listed here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicUpcoming {
    /// The quarter it lands in, at the end of that quarter.
    pub quarter: u8,
    pub id: String,
    pub title: String,
    pub summary: String,
    /// `invented` today; real-world items will use another value (they are labelled, never mixed in unmarked).
    pub source: String,
    /// `bad` or `good`.
    pub tone: String,
    /// What it would do to each group's support if it landed now, in ppm (negative takes support away).
    pub effect_ppm: [i32; 6],
    /// The same, weighted by each group's share of the voters: the effect on overall support.
    pub overall_ppm: i32,
    /// `serious` (takes two points or more of overall support), `minor`, or `welcome` for good news.
    pub severity: String,
    /// What a response would cost now, in credits. Absent for good news and once a response has been paid.
    pub relief_cost: Option<u16>,
    /// How much of every loss a response takes off, in percent.
    pub relief_cut_pct: u8,
    /// Credits already paid to answer it, if they were.
    pub paid: Option<u16>,
    /// Set when the story also stops one kind of work (decision 0011).
    pub disruption: Option<PublicDisruption>,
    /// Set when inspectors will also count the promises that are still unkept (decision 0012).
    pub scrutiny: Option<PublicScrutiny>,
    /// Set when it is a windfall: the credits that arrive when it lands (decision 0012).
    pub grant_credits: Option<u16>,
    /// Set when the headline is real news: where it came from and which rule turned it into numbers (decision 0013).
    pub real: Option<PublicRealSource>,
}
/// Where a real headline came from, and why it does what it does.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicRealSource {
    /// Who published it, for example `GOV.UK, Department for Education`.
    pub publisher: String,
    /// `YYYY-MM-DD`.
    pub published: String,
    pub url: String,
    /// The keyword rule that matched, by id and in words.
    pub rule_id: String,
    pub rule_label: String,
}
/// A real headline imported for a term, whether or not it changes anything (decision 0013).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicRealNote {
    pub term: u8,
    pub title: String,
    pub summary: String,
    pub url: String,
    pub published: String,
    /// The rule that matched. Absent when nothing matched and the headline only informs.
    pub rule_label: Option<String>,
    /// The quarter it lands in, if it took one. Absent when it informs only, or no quarter of its tone was free.
    pub quarter: Option<u8>,
}
/// What an inspection would count and take.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicScrutiny {
    /// The kinds of work counted: any of `staffing`, `repairs`, `access`.
    pub families: Vec<String>,
    /// Units promised and still undelivered in those kinds of work when the inspectors arrive, as things stand, in the
    /// order staffing, repairs, access (zero for a kind that is not counted).
    pub owed_units: [u16; 3],
    /// The part of the loss that is the count, per group (already counted in `effect_ppm`).
    pub effect_ppm: [i32; 6],
}
/// What a disruption would do to the work the player has promised.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicDisruption {
    /// `staffing`, `repairs` or `access`.
    pub family: String,
    /// How much of its usual capacity that kind of work keeps, in percent, and for how many quarters.
    pub keep_pct: u32,
    pub quarters: u8,
    /// Units that would be delivered after the term's election instead of before it.
    pub delayed_units: u16,
    /// The support those units would have earned by the election, per group, taken away (already counted in `effect_ppm`).
    pub effect_ppm: [i32; 6],
}
/// A headline that has landed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicLanded {
    pub quarter: u8,
    pub id: String,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub tone: String,
    /// What it did to each group's support, after any response.
    pub effect_ppm: [i32; 6],
    pub overall_ppm: i32,
    /// Nobody was told in advance.
    pub surprise: bool,
    /// A response softened it.
    pub responded: bool,
    /// Set when the story also stopped one kind of work. `keep_pct` is what was applied, after any response.
    pub disruption: Option<PublicLandedDisruption>,
    /// Set when inspectors counted the promises that were still unkept.
    pub scrutiny: Option<PublicLandedScrutiny>,
    /// Set when it was a windfall: the credits that arrived.
    pub grant_credits: Option<u16>,
    /// Set when the headline was real news.
    pub real: Option<PublicRealSource>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicLandedScrutiny {
    pub families: Vec<String>,
    /// Units promised and not yet delivered that the inspectors counted, in the order staffing, repairs, access.
    pub owed_units: [u16; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicLandedDisruption {
    pub family: String,
    pub keep_pct: u32,
    pub quarters: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicCalendar {
    pub upcoming: Vec<PublicUpcoming>,
    /// Every headline that has landed, oldest first.
    pub recent: Vec<PublicLanded>,
    /// Every real headline imported so far, oldest first, including those that only inform.
    pub real_news: Vec<PublicRealNote>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewsBundle {
    schema_version: u16,
    cutoff_utc: String,
    items: Vec<PublicNews>,
    label: String,
}
/// Canonical Gregorian dates only; lexical cutoff checks require this first.
fn strict_iso_date(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        date[..4].parse::<u32>(),
        date[5..7].parse::<u32>(),
        date[8..10].parse::<u32>(),
    ) else {
        return false;
    };
    if year == 0 {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}
fn parse_news_bundle(raw: &str) -> Result<(String, Vec<PublicNews>), String> {
    let bundle: NewsBundle = serde_json::from_str(raw).map_err(|_| "invalid news bundle")?;
    let cutoff = bundle
        .cutoff_utc
        .get(..10)
        .ok_or("missing news cutoff date")?;
    if bundle.schema_version != 1
        || bundle.label.is_empty()
        || bundle.items.len() > 12
        || bundle.cutoff_utc.len() != 20
        || !bundle.cutoff_utc.ends_with('Z')
        || !strict_iso_date(cutoff)
    {
        return Err("news bundle/country cutoff mismatch".into());
    }
    for item in &bundle.items {
        let real = item.kind == "real_source_update";
        let fiction = item.kind == "fictional_simulation_news";
        if !(real || fiction)
            || !strict_iso_date(&item.date)
            || item.title.is_empty()
            || item.summary.is_empty()
            || (real
                && (item.date.as_str() > cutoff
                    || !item
                        .url
                        .as_deref()
                        .is_some_and(|url| url.starts_with("https://"))))
            || (fiction && item.url.is_some())
        {
            return Err("unsafe or unclassified archived news".into());
        }
    }
    Ok((bundle.cutoff_utc, bundle.items))
}
static VALIDATED_NEWS: OnceLock<Result<(String, Vec<PublicNews>), String>> = OnceLock::new();
fn frozen_news(cutoff: &str) -> Result<Vec<PublicNews>, String> {
    let (expected, news) = VALIDATED_NEWS
        .get_or_init(|| parse_news_bundle(FROZEN_NEWS))
        .as_ref()
        .map_err(Clone::clone)?;
    if expected != cutoff {
        return Err("news cutoff differs from campaign pack".into());
    }
    Ok(news.clone())
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
fn parse_source_indicators() -> Result<(String, Vec<PublicIndicator>), String> {
    let pack: SourcePack =
        serde_json::from_str(FROZEN_COUNTRY).map_err(|_| "cannot parse frozen country source")?;
    let values: SourceValues =
        serde_json::from_str(FROZEN_VALUES).map_err(|_| "cannot parse frozen source values")?;
    if pack.schema_version != 1
        || values.schema_version != 1
        || pack.cutoff_utc != values.cutoff_utc
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
    Ok((pack.cutoff_utc, indicators))
}
static VALIDATED_INDICATORS: OnceLock<Result<(String, Vec<PublicIndicator>), String>> =
    OnceLock::new();
fn source_indicators(cutoff: &str) -> Result<Vec<PublicIndicator>, String> {
    let (expected, rows) = VALIDATED_INDICATORS
        .get_or_init(parse_source_indicators)
        .as_ref()
        .map_err(Clone::clone)?;
    if expected != cutoff {
        return Err("evidence cutoff differs from campaign pack".into());
    }
    Ok(rows.clone())
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
/// The six groups, in the order used for every per-group array in the snapshot.
const GROUP_IDS: [&str; 6] = [
    "urban_low",
    "urban_middle",
    "urban_high",
    "rural_low",
    "rural_middle",
    "rural_high",
];
/// How the public feels, group by group. Every array follows `group_ids`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicSupport {
    pub group_ids: Vec<String>,
    /// Support as of the last resolved quarter, in parts per million (500,000 is 50%).
    pub group_ppm: [u32; 6],
    /// What is already queued to land: settled support minus current support. At most one quarter
    /// of delivery is ever pending, so this is next quarter's change of mood.
    pub pending_delta_ppm: [i32; 6],
    /// Each group's share of the voters, in parts per million. Sums to 1,000,000.
    pub weight_ppm: [u32; 6],
}
/// What an election returns. `projected` while a term is running (every queued response lands, nothing new
/// is delivered); `won` or `lost` once the term has ended and the verdict is recorded.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicElection {
    pub status: String,
    /// The term this election ends (1-based). There is no fixed number of terms: a win starts another.
    pub term: u8,
    /// True only for the election at the technical ceiling (the longest tenure the engine records): winning it
    /// ends the game instead of starting another term.
    pub ceiling: bool,
    pub settled_group_ppm: [u32; 6],
    pub weighted_support_ppm: u32,
    pub blocs_held_ppm: u32,
    pub min_support_ppm: u32,
    pub held_above_ppm: u32,
}
impl PublicElection {
    fn bars(status: &str, term: u8, settled: [u32; 6], weighted: u32, held: u32) -> Self {
        Self {
            status: status.into(),
            term,
            ceiling: term == d::MAX_TERMS,
            settled_group_ppm: settled,
            weighted_support_ppm: weighted,
            blocs_held_ppm: held,
            min_support_ppm: ELECTION_MIN_SUPPORT_PPM,
            held_above_ppm: ELECTION_HELD_ABOVE_PPM,
        }
    }
}
/// An election already held, kept so the screen can say how the first term went.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicPastElection {
    pub term: u8,
    pub quarter: u8,
    pub won: bool,
    pub weighted_support_ppm: u32,
    pub blocs_held_ppm: u32,
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
    pub continuation_pending: bool,
    pub proposal: Option<Value>,
    pub votes: Vec<PublicVote>,
    pub treasury: PublicTreasury,
    pub decisions: Vec<PublicDecision>,
    pub delivery_history: Vec<PublicDelivery>,
    pub support: PublicSupport,
    pub election: PublicElection,
    /// Elections already held, oldest first. Empty during the first term.
    pub past_elections: Vec<PublicPastElection>,
    pub term: u8,
    pub term_quarters: u8,
    pub news: Vec<PublicNews>,
    pub calendar: PublicCalendar,
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
    // Each journaled action belongs to the quarter counted from the explicit
    // PERIOD `advance_quarter` commands. It is never inferred from the most
    // recent proposal: a quarter without a meeting has no proposal.
    let mut quarter = 1u8;
    // A funded meeting was opened in `quarter`. The journal records two kinds
    // of `defer`: one after a vote (a real deferral) and, from Q2, one at
    // QuarterOpen before any proposal, which is the explicit "keep the existing
    // programme" choice (see `GameWorld::continuation_pending`).
    let mut meeting_opened = false;
    // The most recently ratified allocation: what a continuation carries forward.
    let mut programme: Option<Value> = None;
    for action in &world.actions {
        if action.kind == ActionKind::PlayerCommand {
            let body = &action.payload["command"];
            match body["kind"].as_str() {
                Some("propose") => {
                    meeting_opened = true;
                    proposal = Some(body["allocation"].clone());
                    outcome = None;
                    votes.clear();
                }
                Some("amend") => proposal = Some(body["allocation"].clone()),
                Some(kind @ ("ratify" | "defer")) => {
                    let ratified = kind == "ratify";
                    let continued = !ratified && !meeting_opened;
                    let label = if ratified {
                        "ratified"
                    } else if continued {
                        "continued"
                    } else {
                        "deferred"
                    };
                    // A continuation holds no meeting: it has no votes of its
                    // own, and the earlier meeting's votes must not be reported
                    // as support for it.
                    let (allocation, final_support, final_votes) = if continued {
                        (programme.clone(), 0, 0)
                    } else {
                        let relevant: Vec<_> =
                            votes.iter().filter(|v| v.stage == "final").collect();
                        (
                            proposal.clone(),
                            relevant
                                .iter()
                                .filter(|v| v.position == "support")
                                .count()
                                .try_into()
                                .map_err(|_| "too many public final votes")?,
                            relevant
                                .len()
                                .try_into()
                                .map_err(|_| "too many final votes")?,
                        )
                    };
                    if ratified {
                        programme = proposal.clone();
                    }
                    outcome = Some(label.to_owned());
                    decisions.push(PublicDecision {
                        quarter,
                        outcome: label.to_owned(),
                        allocation,
                        final_support,
                        final_votes,
                    });
                }
                // The clock stops at the end of a term: the election is held in that same quarter, and
                // only an explicit `begin_term` opens the next one.
                Some("advance_quarter") => {
                    if !quarter.is_multiple_of(d::TERM_QUARTERS) {
                        quarter = quarter.saturating_add(1);
                    }
                    meeting_opened = false;
                }
                Some("begin_term") => {
                    quarter = quarter.saturating_add(1);
                    meeting_opened = false;
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
    if decisions.len() > usize::from(d::MAX_QUARTERS) {
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
    if delivery_history.len() > usize::from(d::MAX_QUARTERS)
        || delivery_history
            .iter()
            .enumerate()
            .any(|(i, report)| usize::from(report.quarter) != i + 1)
    {
        return Err("invalid public quarterly report sequence".into());
    }
    let current: [u32; 6] = (*campaign.world().support()).map(|cell| cell.value());
    let election = campaign
        .election_projection()
        .map_err(|reason| format!("election projection failed: {reason:?}"))?;
    let mut pending_delta_ppm = [0_i32; 6];
    for (delta, (settled, now)) in pending_delta_ppm
        .iter_mut()
        .zip(election.group_support_ppm.iter().zip(current.iter()))
    {
        *delta = i32::try_from(i64::from(*settled) - i64::from(*now))
            .map_err(|_| "pending support change out of range".to_string())?;
    }
    // Once a term has ended the verdict is the one recorded on election night, not a projection.
    let quarter_now =
        d::Quarter::new(world.quarter).map_err(|e| format!("invalid quarter: {e:?}"))?;
    let held_now = campaign
        .elections()
        .last()
        .filter(|record| record.quarter == quarter_now);
    let status = match held_now {
        Some(record) if record.won => "won",
        Some(_) => "lost",
        None if world.is_closed() => return Err("campaign closed without an election".into()),
        None => "projected",
    };
    let past_elections: Vec<PublicPastElection> = campaign
        .elections()
        .iter()
        .map(|record| PublicPastElection {
            term: record.term,
            quarter: record.quarter.value(),
            won: record.won,
            weighted_support_ppm: record.projection.weighted_support_ppm,
            blocs_held_ppm: record.projection.blocs_held_ppm,
        })
        .collect();
    let support = PublicSupport {
        group_ids: GROUP_IDS.iter().map(|id| (*id).to_owned()).collect(),
        group_ppm: current,
        pending_delta_ppm,
        weight_ppm: *campaign.world().cohort_weights().parts_ppm(),
    };
    let election = PublicElection::bars(
        status,
        quarter_now.term(),
        election.group_support_ppm,
        election.weighted_support_ppm,
        election.blocs_held_ppm,
    );
    let calendar = public_calendar(&campaign)?;
    let fiscal = PublicTreasury {
        available_credits: treasury.available(),
        encumbered_staff: treasury.encumbered(PolicyFamily::Staffing),
        encumbered_repairs: treasury.encumbered(PolicyFamily::Repairs),
        encumbered_access: treasury.encumbered(PolicyFamily::Access),
        service_spent: treasury.service_spent(),
        audit_spent: treasury.audit_spent(),
        relief_spent: treasury.relief_spent(),
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
        continuation_pending:world.continuation_pending(),
        proposal,
        votes,
        treasury:fiscal,
        decisions,
        delivery_history,
        support,
        election,
        past_elections,
        term: quarter_now.term(),
        term_quarters: d::TERM_QUARTERS,
        news: frozen_news(&world.source_cutoff_utc)?,
        calendar,
        comparison: PublicComparison {
            mode: "primary_campaign".into(),
            source_campaign_id: None,
            source_cursor: None,
            inherited_shock_stream: false,
            post_fork_actor_actions_fixed: false,
        },
        evidence,
        outcome,
        assumption:"Synthetic education-delivery rules; scripted cabinet, not live ministers or empirical predictions".into(),
        mode:"offline_scripted",
    })
}
/// The kinds of work an inspection counts, by name.
fn counted_families(families: [bool; 3]) -> Vec<String> {
    ["staffing", "repairs", "access"]
        .into_iter()
        .zip(families)
        .filter(|(_, counted)| *counted)
        .map(|(name, _)| name.to_owned())
        .collect()
}
/// Who publishes the real headlines, as the screen says it.
const REAL_PUBLISHER: &str = "GOV.UK, Department for Education";
fn family_name(family: PolicyFamily) -> &'static str {
    match family {
        PolicyFamily::Staffing => "staffing",
        PolicyFamily::Repairs => "repairs",
        PolicyFamily::Access => "access",
    }
}
/// What the news calendar shows (decision 0010): headlines announced a quarter ahead with their price, and every headline
/// that has landed. Words come from the frozen scenario file; the engine only knows numbers.
fn public_calendar(campaign: &myth_domain::reducer::Campaign) -> Result<PublicCalendar, String> {
    let weights = *campaign.world().cohort_weights().parts_ppm();
    let overall = |shock: &[i32; 6]| -> Result<i32, String> {
        let weighted: i64 = shock
            .iter()
            .zip(weights.iter())
            .map(|(s, w)| i64::from(*s) * i64::from(*w))
            .sum();
        i32::try_from(weighted / 1_000_000).map_err(|_| "headline effect out of range".to_string())
    };
    let words = |s: &d::news::Scheduled| match s.real {
        Some(slot) => campaign
            .real_items(s.quarter.term())
            .get(usize::from(slot.item))
            .map(|item| (item.title.clone(), item.summary.clone()))
            .ok_or_else(|| "real headline text missing".to_string()),
        None => headline_text(s.tone, s.index).ok_or_else(|| "headline text missing".to_string()),
    };
    let real_source = |s: &d::news::Scheduled| -> Option<PublicRealSource> {
        let slot = s.real?;
        let item = campaign
            .real_items(s.quarter.term())
            .get(usize::from(slot.item))?;
        let rule = item.rule.as_ref()?;
        Some(PublicRealSource {
            publisher: REAL_PUBLISHER.into(),
            published: item.published.clone(),
            url: item.url.clone(),
            rule_id: rule.id.clone(),
            rule_label: rule.label.clone(),
        })
    };
    let headline_id = |s: &d::news::Scheduled| -> String {
        match s.real {
            Some(slot) => campaign
                .real_items(s.quarter.term())
                .get(usize::from(slot.item))
                .and_then(|item| item.rule.as_ref())
                .map_or_else(
                    || "real_news".to_owned(),
                    |rule| format!("real_{}", rule.id),
                ),
            None => s.headline().id.to_owned(),
        }
    };
    let tone = |t: d::news::Tone| match t {
        d::news::Tone::Bad => "bad".to_string(),
        d::news::Tone::Good => "good".to_string(),
    };
    let mut upcoming = vec![];
    for up in campaign.upcoming_headlines() {
        let (title, summary) = words(&up.scheduled)?;
        let overall_ppm = overall(&up.expected)?;
        upcoming.push(PublicUpcoming {
            quarter: up.scheduled.quarter.value(),
            id: headline_id(&up.scheduled),
            title,
            summary,
            source: if up.scheduled.real.is_some() {
                "real"
            } else {
                "invented"
            }
            .into(),
            tone: tone(up.scheduled.tone),
            effect_ppm: up.expected,
            overall_ppm,
            severity: if up.scheduled.tone == d::news::Tone::Good {
                "welcome"
            } else if overall_ppm <= -20_000 {
                "serious"
            } else {
                "minor"
            }
            .into(),
            relief_cost: up.relief_cost,
            relief_cut_pct: u8::try_from(d::news::RELIEF_CUT_PCT)
                .map_err(|_| "cut out of range")?,
            paid: up.answered,
            disruption: up.delivery.map(|delivery| PublicDisruption {
                family: family_name(delivery.family).into(),
                keep_pct: delivery.keep_pct,
                quarters: delivery.quarters,
                delayed_units: delivery.delayed_units,
                effect_ppm: delivery.effect,
            }),
            scrutiny: up.scrutiny.map(|count| PublicScrutiny {
                families: counted_families(count.families),
                owed_units: count.owed,
                effect_ppm: count.effect,
            }),
            grant_credits: (up.grant > 0).then_some(up.grant),
            real: real_source(&up.scheduled),
        });
    }
    let mut recent = vec![];
    for landed in campaign.landed_headlines() {
        let (title, summary) = words(&landed.scheduled)?;
        recent.push(PublicLanded {
            quarter: landed.scheduled.quarter.value(),
            id: headline_id(&landed.scheduled),
            title,
            summary,
            source: if landed.scheduled.real.is_some() {
                "real"
            } else {
                "invented"
            }
            .into(),
            tone: tone(landed.scheduled.tone),
            effect_ppm: landed.shock,
            overall_ppm: overall(&landed.shock)?,
            surprise: landed.scheduled.surprise,
            responded: landed.responded,
            disruption: landed
                .scheduled
                .headline()
                .disrupts
                .filter(|_| landed.scheduled.tone == d::news::Tone::Bad)
                .map(|cut| PublicLandedDisruption {
                    family: family_name(cut.family).into(),
                    keep_pct: if landed.responded {
                        d::news::softened_keep(cut.keep_pct)
                    } else {
                        cut.keep_pct
                    },
                    quarters: cut.quarters,
                }),
            scrutiny: landed
                .scheduled
                .headline()
                .scrutiny
                .filter(|_| landed.scheduled.tone == d::news::Tone::Bad)
                .map(|count| PublicLandedScrutiny {
                    families: counted_families(count.families),
                    owed_units: landed.owed,
                }),
            grant_credits: (landed.granted > 0).then_some(landed.granted),
            real: real_source(&landed.scheduled),
        });
    }
    let mut real_news = vec![];
    for (term, items) in campaign.real_batches() {
        for (i, item) in items.iter().enumerate() {
            real_news.push(PublicRealNote {
                term: *term,
                title: item.title.clone(),
                summary: item.summary.clone(),
                url: item.url.clone(),
                published: item.published.clone(),
                rule_label: item.rule.as_ref().map(|r| r.label.clone()),
                quarter: campaign.real_quarter(*term, i).map(|q| q.value()),
            });
        }
    }
    Ok(PublicCalendar {
        upcoming,
        recent,
        real_news,
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
    /// Where this campaign's real headlines come from, if anywhere (decision 0013).
    news: NewsSource,
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
    news: NewsSource,
    owners: Mutex<BTreeMap<String, Arc<Owner>>>,
    loading: AsyncMutex<()>,
    _writer_lock: Arc<File>,
}
impl Connected {
    /// A service with no real news: the game plays on invented headlines alone. This is what every test uses.
    pub fn new(root: PathBuf) -> Result<Arc<Self>, String> {
        Self::with_news(root, NewsSource::Off)
    }
    /// A service that freezes real headlines into each campaign at the start of every term (decision 0013).
    pub fn with_news(root: PathBuf, news: NewsSource) -> Result<Arc<Self>, String> {
        // Fail closed before opening a campaign if the immutable source archive is invalid.
        let cutoff = "2026-10-29T23:59:59Z";
        frozen_news(cutoff)?;
        source_indicators(cutoff)?;
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
        // With the process-wide writer lock held, interrupted staging from
        // a killed previous owner cannot be live. Remove only our bounded
        // hidden fork-stage directories, never published campaign journals.
        for entry in fs::read_dir(&root).map_err(|_| "cannot scan private campaign root")? {
            let entry = entry.map_err(|_| "cannot scan fork staging")?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name.starts_with('.')
                && name.ends_with(".fork-stage")
                && entry
                    .file_type()
                    .map_err(|_| "cannot inspect fork staging")?
                    .is_dir()
            {
                fs::remove_dir_all(entry.path())
                    .map_err(|_| "cannot clean interrupted fork staging")?;
            }
        }
        Ok(Arc::new(Self {
            root,
            news,
            owners: Mutex::new(BTreeMap::new()),
            loading: AsyncMutex::new(()),
            _writer_lock,
        }))
    }
    /// Reads never create a campaign. Admission may create a new explicit
    /// campaign; only one loading task runs at a time, and filesystem replay
    /// never executes under the global owner registry mutex or on Tokio.
    async fn owner(self: &Arc<Self>, id: &str, create: bool) -> Result<Arc<Owner>, String> {
        if !valid_id(id) {
            return Err("invalid campaign id".into());
        }
        if let Some(held) = self
            .owners
            .lock()
            .map_err(|_| "owner registry poisoned")?
            .get(id)
        {
            return Ok(held.clone());
        }
        if !create && !self.root.join(id).is_dir() {
            return Err("campaign not found".into());
        }
        let _loading = self.loading.lock().await;
        if let Some(held) = self
            .owners
            .lock()
            .map_err(|_| "owner registry poisoned")?
            .get(id)
        {
            return Ok(held.clone());
        }
        if self
            .owners
            .lock()
            .map_err(|_| "owner registry poisoned")?
            .len()
            >= MAX_CACHED_CAMPAIGNS
        {
            return Err("campaign owner cache is full".into());
        }
        let path = self.root.join(id);
        let log_path = self.root.join(format!("{id}.commands.json"));
        let key = id.to_owned();
        let id_for_load = key.clone();
        let (trial, log) = tokio::task::spawn_blocking(move || {
            if !path.exists() && log_path.exists() {
                return Err("command history exists but Hiyal trial is missing".into());
            }
            if !create && !path.is_dir() {
                return Err("campaign not found".into());
            }
            let trial = if path.exists() {
                resume(&path, false)?
            } else {
                create_campaign(&path, &id_for_load, 43, &id_for_load)?
            };
            if trial.store.world().campaign_id != id_for_load {
                let provenance = read_fork_meta(&path)?.ok_or("missing fork provenance")?;
                if provenance.branch_id != id_for_load
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
            let log = read_log(&log_path, &id_for_load)?;
            Ok::<_, String>((trial, log))
        })
        .await
        .map_err(|_| "campaign restoration task failed")??;
        let (sender, _) = broadcast::channel(64);
        let owner = Arc::new(Owner {
            engine: AsyncMutex::new(Engine {
                trial,
                log,
                poisoned: false,
            }),
            news: self.news.clone(),
            inflight: AtomicBool::new(false),
            sender,
            log_path: self.root.join(format!("{key}.commands.json")),
            id: key.clone(),
            _writer_lock: self._writer_lock.clone(),
        });
        let recover = owner.engine.try_lock().is_ok_and(|g| {
            g.log
                .records
                .iter()
                .any(|r| r.status == OperationStatus::Accepted)
        });
        self.owners
            .lock()
            .map_err(|_| "owner registry poisoned")?
            .insert(key, owner.clone());
        if recover {
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
        // The real headlines for a term that is about to begin, fetched with the lock released (decision 0013).
        let mut prefetched: Option<(String, Vec<RealItemWire>)> = None;
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
            if owner.news.is_on() && engine.log.records[pos].kind == CommandKind::BeginTerm {
                let command_id = engine.log.records[pos].command_id.clone();
                if prefetched.as_ref().is_none_or(|(id, _)| *id != command_id) {
                    let told = told_links(&engine);
                    drop(engine);
                    let items = owner.news.batch(&told).await;
                    prefetched = Some((command_id, items));
                    continue;
                }
            }
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
                    Ok(()) if engine.log.records[pos].kind == CommandKind::FinishContinuation => {
                        advance_unmodified_quarter(&mut engine.trial).await
                    }
                    Ok(()) => advance_slice(&mut engine.trial).await.map(|_| ()),
                    Err(error) => Err(error),
                };
                engine.trial.clear_player_command();
                // A new term's real headlines go into the record right after it begins, before the first plan.
                if applied.is_ok()
                    && engine.log.records[pos].kind == CommandKind::BeginTerm
                    && let Some((id, items)) = prefetched.take()
                    && id == command_id
                    && let Err(reason) = freeze_news(&mut engine, items).await
                {
                    eprintln!("Real news could not be recorded for this term: {reason}");
                }
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
                        continuation_pending: false,
                        proposal: None,
                        votes: vec![],
                        treasury: PublicTreasury {
                            available_credits: 0,
                            encumbered_staff: 0,
                            encumbered_repairs: 0,
                            encumbered_access: 0,
                            service_spent: 0,
                            audit_spent: 0,
                            relief_spent: 0,
                        },
                        decisions: vec![],
                        delivery_history: vec![],
                        support: PublicSupport {
                            group_ids: GROUP_IDS.iter().map(|id| (*id).to_owned()).collect(),
                            group_ppm: [0; 6],
                            pending_delta_ppm: [0; 6],
                            weight_ppm: [0; 6],
                        },
                        election: PublicElection::bars("projected", 1, [0; 6], 0, 0),
                        past_elections: vec![],
                        term: 1,
                        term_quarters: d::TERM_QUARTERS,
                        news: vec![],
                        calendar: PublicCalendar {
                            upcoming: vec![],
                            recent: vec![],
                            real_news: vec![],
                        },
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
/// The links of every real story this campaign has already told, so none is told twice.
fn told_links(engine: &Engine) -> BTreeSet<String> {
    engine
        .trial
        .store
        .world()
        .fold()
        .map(|campaign| {
            campaign
                .real_batches()
                .values()
                .flatten()
                .map(|item| item.url.clone())
                .collect()
        })
        .unwrap_or_default()
}
/// Freeze a batch of real headlines into the campaign's record, as the server's own command at the start of a term (decision
/// 0013). Nothing happens, and nothing is journaled, if the game would not accept it (a plan has been made, or this term already
/// has its batch): real news that arrives late is simply not used.
async fn freeze_news(engine: &mut Engine, items: Vec<RealItemWire>) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let world = engine.trial.store.world();
    let campaign = world.fold()?;
    let wanted: Vec<d::news::RealItem> = items.iter().cloned().map(Into::into).collect();
    if !campaign.accepts_news_import(&wanted) {
        return Ok(());
    }
    let term = (world.quarter - 1) / d::TERM_QUARTERS + 1;
    let command_id = format!("news-t{term}");
    engine
        .trial
        .select_player_command(&command_id, json!({"kind": "import_news", "items": items}))?;
    let applied = advance_slice(&mut engine.trial).await;
    engine.trial.clear_player_command();
    applied.map(|_| ())
}
/// Campaign creation is an explicit POST, never a side effect of GET.
async fn create(
    State(service): State<Arc<Connected>>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<Committed>), Failure> {
    if !valid_id(&id) {
        return Err(status(StatusCode::BAD_REQUEST, "invalid campaign id"));
    }
    let exists = service.root.join(&id).is_dir();
    let owner = service
        .owner(&id, true)
        .await
        .map_err(|_| status(StatusCode::CONFLICT, "campaign could not be created"))?;
    // The first term's real headlines are fetched before anything is locked, then frozen in before the first plan.
    if !exists && owner.news.is_on() {
        let items = owner.news.batch(&BTreeSet::new()).await;
        let mut engine = owner.engine.lock().await;
        if let Err(reason) = freeze_news(&mut engine, items).await {
            eprintln!("Real news could not be recorded for this campaign: {reason}");
        }
    }
    let engine = owner.engine.lock().await;
    let view = capture_for(&engine.trial, &owner.id)
        .map_err(|_| status(StatusCode::CONFLICT, "verification failed"))?;
    Ok((
        if exists {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        Json(Committed {
            snapshot: view,
            operation: None,
        }),
    ))
}
async fn snapshot(
    State(service): State<Arc<Connected>>,
    Path(id): Path<String>,
) -> Result<Json<Committed>, Failure> {
    let owner = service
        .owner(&id, false)
        .await
        .map_err(|_| status(StatusCode::NOT_FOUND, "campaign unavailable"))?;
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
        .owner(&id, false)
        .await
        .map_err(|_| status(StatusCode::NOT_FOUND, "campaign unavailable"))?;
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
        .owner(&id, true)
        .await
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
    if request.kind == CommandKind::AdvanceSlice
        && engine.trial.store.world().phase == myth_contracts::checkpoint::Phase::ElectionNight
    {
        // The scripted player would otherwise begin the next term on the person's behalf.
        return Err(status(
            StatusCode::CONFLICT,
            "election night needs an explicit begin_term",
        ));
    }
    if matches!(
        request.kind,
        CommandKind::ContinueQuarter | CommandKind::FinishContinuation
    ) && engine.trial.store.world().phase != myth_contracts::checkpoint::Phase::QuarterOpen
    {
        return Err(status(
            StatusCode::CONFLICT,
            "cannot continue an active cabinet meeting",
        ));
    }
    if request.kind == CommandKind::FinishContinuation
        && !engine.trial.store.world().continuation_pending()
    {
        return Err(status(
            StatusCode::CONFLICT,
            "no journaled continuation choice",
        ));
    }
    if request.kind == CommandKind::ContinueQuarter
        && (engine.trial.store.world().quarter < 2
            || engine.trial.store.world().continuation_pending())
    {
        return Err(status(
            StatusCode::CONFLICT,
            "cannot repeat continuation choice",
        ));
    }
    if let CommandKind::Respond { headline_quarter } = request.kind {
        // Refuse here, with a reason, anything the engine would only fail later: no such announced headline, one already
        // answered, or not enough credits.
        let campaign = engine
            .trial
            .store
            .world()
            .fold()
            .map_err(|_| status(StatusCode::CONFLICT, "campaign state unavailable"))?;
        let quote = campaign
            .upcoming_headlines()
            .into_iter()
            .find(|u| u.scheduled.quarter.value() == headline_quarter)
            .and_then(|u| u.relief_cost)
            .ok_or_else(|| {
                status(
                    StatusCode::CONFLICT,
                    "no announced headline to answer for that quarter",
                )
            })?;
        if quote > campaign.world().treasury().available() {
            return Err(status(
                StatusCode::CONFLICT,
                "not enough credits to answer that headline",
            ));
        }
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
        .owner(&id, false)
        .await
        .map_err(|_| status(StatusCode::NOT_FOUND, "campaign unavailable"))?;
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
                // An unverified boundary is a notification, NOT a cursor-0
                // commit. Never feed it through monotonic state reduction.
                let frame = if event.snapshot.phase == "IntegrityFailure" {
                    Event::default()
                        .event("integrity_failure")
                        .json_data(event)
                        .ok()?
                } else {
                    Event::default()
                        .event("committed")
                        .id(id)
                        .json_data(event)
                        .ok()?
                };
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
        .owner(&id, false)
        .await
        .map_err(|_| status(StatusCode::NOT_FOUND, "source unavailable"))?;
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
    let source = engine.trial.store.dir.clone();
    // capture_for already strictly verified the journal against the frozen
    // committed checkpoint at this boundary.
    let source_hash = engine.trial.store.world().private_state_sha256.clone();
    let meta = ForkMeta {
        schema_version: 1,
        branch_id: request.branch_id.clone(),
        source_campaign_id: engine.trial.store.world().campaign_id.clone(),
        source_cursor: boundary.cursor,
        source_state_sha256: source_hash,
    };
    // Verified quiescent cursor/hash were captured under the campaign lock.
    // The immutable copy and strict replay run outside Tokio and outside
    // the live campaign lock; any concurrent append causes a digest/cursor
    // mismatch and the uncommitted staging directory is cleaned up.
    let root = service.root.clone();
    drop(engine);
    tokio::task::spawn_blocking(move || -> Result<_, Failure> {
        let dst = root.join(&request.branch_id);
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
        let stage = root.join(format!(".{}.fork-stage", request.branch_id));
        fs::create_dir(&stage).map_err(|_| {
            status(
                StatusCode::CONFLICT,
                "fork staging exists or cannot be created",
            )
        })?;
        let mut cleanup = ForkStageGuard {
            path: stage.clone(),
            active: true,
        };
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
        if clone_hash.0 != meta.source_state_sha256
            || clone_hash.2.to_string() != meta.source_cursor
        {
            return Err(status(StatusCode::CONFLICT, "fork hash or cursor mismatch"));
        }
        fs::rename(&stage, &dst)
            .map_err(|_| status(StatusCode::CONFLICT, "fork publication failed"))?;
        cleanup.active = false;
        #[cfg(unix)]
        File::open(&root).and_then(|f| f.sync_all()).map_err(|_| {
            status(
                StatusCode::INTERNAL_SERVER_ERROR,
                "cannot sync branch directory",
            )
        })?;
        Ok((StatusCode::CREATED, Json(fork_receipt(&meta))))
    })
    .await
    .map_err(|_| status(StatusCode::INTERNAL_SERVER_ERROR, "fork task failed"))?
}
pub fn routes() -> Router<Arc<Connected>> {
    Router::new()
        .route("/api/campaigns/{id}", post(create))
        .route("/api/campaigns/{id}/snapshot", get(snapshot))
        .route("/api/campaigns/{id}/commands", post(command))
        .route("/api/campaigns/{id}/commands/{command_id}", get(operation))
        .route("/api/campaigns/{id}/events", get(stream_events))
        .route("/api/campaigns/{id}/forks", post(fork))
}
#[cfg(test)]
mod tests {
    use super::*;
    use myth_hiyal_adapter::orchestrator::run_until_term_end;
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
    #[test]
    fn uncommitted_fork_stage_is_cleaned_on_error_or_restart() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let stage = dir.join(".fork-test.fork-stage");
        fs::create_dir(&stage).unwrap();
        let guard = ForkStageGuard {
            path: stage.clone(),
            active: true,
        };
        fs::write(stage.join("events.jsonl"), b"partial").unwrap();
        drop(guard);
        assert!(!stage.exists());
        // Simulated SIGKILL: no Drop handler ran, but exclusive ownership on
        // restart may remove only the unpublished staging directory.
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join("events.jsonl"), b"partial").unwrap();
        drop(service);
        let resumed = Connected::new(dir.clone()).unwrap();
        assert!(!stage.exists());
        drop(resumed);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn unknown_campaign_reads_do_not_create_state() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        match snapshot(State(service.clone()), Path("missing".into())).await {
            Err((code, _)) => assert_eq!(code, StatusCode::NOT_FOUND),
            Ok(_) => panic!("unknown read unexpectedly created a campaign"),
        }
        assert!(!dir.join("missing").exists());
        let (code, Json(created)) = create(State(service.clone()), Path("missing".into()))
            .await
            .unwrap();
        assert_eq!(code, StatusCode::CREATED);
        assert_eq!(created.snapshot.cursor, "0");
        assert!(dir.join("missing").is_dir());
        drop(service);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn durable_idempotency_and_scoped_recovery() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let first = service.owner("alpha", true).await.unwrap();
        let second = service.owner("beta", true).await.unwrap();
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
        let owner = recovered.owner("alpha", true).await.unwrap();
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
        assert!(recovered.owner("../bad", true).await.is_err());
        drop(owner);
        drop(recovered);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn initial_and_fallback_join_never_skip_a_concurrent_commit() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let owner = service.owner("race", true).await.unwrap();
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
        let original = service.owner("source", true).await.unwrap();
        let branch = service.owner("alternate", true).await.unwrap();
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
                    .owner("alternate", true)
                    .await
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
        let owner = service.owner("interrupted", true).await.unwrap();
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
        let recovered = restarted.owner("interrupted", true).await.unwrap();
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
        let owner = service.owner("accepted-crash", true).await.unwrap();
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
        let writer = restarted.owner("accepted-crash", true).await.unwrap();
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
        let owner = service.owner("metrics", true).await.unwrap();
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
        let reopened = recovered.owner("metrics", true).await.unwrap();
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
        let owner = service.owner("mid-round", true).await.unwrap();
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
        let live = restored.owner("mid-round", true).await.unwrap();
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
        let alpha = service.owner("alpha", true).await.unwrap();
        let beta = service.owner("beta", true).await.unwrap();
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
        let owner = service.owner("lost-ack", true).await.unwrap();
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
        let reopened = restarted.owner("lost-ack", true).await.unwrap();
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
        let owner = service.owner(campaign, true).await.unwrap();
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
        let opening = service.owner("report", true).await.unwrap();
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
        assert_eq!(later.comparison.mode, "primary_campaign");
        assert!(!later.comparison.post_fork_actor_actions_fixed);
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
    fn p7_news_cutoff_rejects_noncanonical_and_impossible_dates() {
        for bad in [
            "1 November 2026",
            "2026-10-30T12:00:00Z",
            "2026-02-29",
            "2026-13-01",
            "2026-10-00",
            "2026-10-32",
            "2026-2-09",
        ] {
            assert!(!strict_iso_date(bad), "accepted non-ISO date {bad}");
        }
        assert!(strict_iso_date("2024-02-29"));
        assert!(parse_news_bundle(&FROZEN_NEWS.replace("2026-08-17", "1 November 2026")).is_err());
        assert!(parse_news_bundle(&FROZEN_NEWS.replace("2026-08-17", "2026-11-01")).is_err());
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
        assert_eq!(
            CommandKind::ContinueQuarter.body(&world).unwrap(),
            Some(json!({"kind":"defer"}))
        );
        assert_eq!(CommandKind::FinishContinuation.body(&world).unwrap(), None);
    }
    fn assert_decision(
        decision: &PublicDecision,
        quarter: u8,
        outcome: &str,
        final_votes: u8,
        allocation: Option<Value>,
    ) {
        assert_eq!(decision.quarter, quarter);
        assert_eq!(decision.outcome, outcome);
        assert_eq!(decision.final_votes, final_votes);
        if final_votes == 0 {
            assert_eq!(decision.final_support, 0);
        }
        assert_eq!(decision.allocation, allocation);
    }
    /// The journal stores a continuation and a real deferral identically
    /// (`{"kind":"defer"}`) and only a proposal names its meeting. The public
    /// projection must still report the true quarter and the right label.
    #[tokio::test]
    async fn p7_public_decisions_distinguish_continuation_from_defer_with_the_true_quarter() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let id = "decision-labels";
        let mut serial = 0usize;
        let mut step = |kind| {
            serial += 1;
            (kind, serial)
        };
        let programme = Some(json!({"staff":50,"repairs":40,"access":10}));
        let nothing = Some(json!({"staff":0,"repairs":0,"access":0}));

        // Q1: a funded meeting, ratified.
        let (kind, n) = step(CommandKind::Propose {
            allocation: WireAllocation {
                staff: 50,
                repairs: 40,
                access: 10,
            },
        });
        accepted_step(&service, id, kind, n).await;
        for _ in 0..3 {
            let (kind, n) = step(CommandKind::AdvanceSlice);
            accepted_step(&service, id, kind, n).await;
        }
        let (kind, n) = step(CommandKind::Decide { ratify: true });
        let view = accepted_step(&service, id, kind, n).await;
        assert_eq!((view.quarter, view.decisions.len()), (1, 1));
        assert_decision(&view.decisions[0], 1, "ratified", 3, programme.clone());
        let (kind, n) = step(CommandKind::AdvanceSlice);
        assert_eq!(accepted_step(&service, id, kind, n).await.quarter, 2);

        // Q2: keep the programme. It is not a deferral and held no vote.
        let (kind, n) = step(CommandKind::ContinueQuarter);
        let view = accepted_step(&service, id, kind, n).await;
        assert!(view.continuation_pending);
        assert_eq!((view.quarter, view.decisions.len()), (2, 2));
        assert_decision(&view.decisions[1], 2, "continued", 0, programme.clone());
        assert_eq!(
            view.outcome, None,
            "a continuation is not a meeting outcome"
        );
        let (kind, n) = step(CommandKind::FinishContinuation);
        assert_eq!(accepted_step(&service, id, kind, n).await.quarter, 3);

        // Q3: a real meeting, deferred after the board has voted.
        let (kind, n) = step(CommandKind::Propose {
            allocation: WireAllocation {
                staff: 0,
                repairs: 0,
                access: 0,
            },
        });
        accepted_step(&service, id, kind, n).await;
        for _ in 0..3 {
            let (kind, n) = step(CommandKind::AdvanceSlice);
            accepted_step(&service, id, kind, n).await;
        }
        let (kind, n) = step(CommandKind::Decide { ratify: false });
        let view = accepted_step(&service, id, kind, n).await;
        assert_eq!((view.quarter, view.decisions.len()), (3, 3));
        assert_decision(&view.decisions[2], 3, "deferred", 3, nothing);
        let (kind, n) = step(CommandKind::AdvanceSlice);
        assert_eq!(accepted_step(&service, id, kind, n).await.quarter, 4);

        // Q4: continuing again carries forward the ratified programme, not the
        // proposal that was just deferred.
        let (kind, n) = step(CommandKind::ContinueQuarter);
        let view = accepted_step(&service, id, kind, n).await;
        assert_eq!((view.quarter, view.decisions.len()), (4, 4));
        assert_decision(&view.decisions[3], 4, "continued", 0, programme);

        // Every label's quarter matches the world's own quarter when it was made.
        for (i, decision) in view.decisions.iter().enumerate() {
            assert_eq!(usize::from(decision.quarter), i + 1);
        }
        drop(service);
        fs::remove_dir_all(dir).unwrap();
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
            let view = service.owner(id, false).await.unwrap();
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
                if quarter == 3 {
                    let (kind, n) = step(CommandKind::OrderAudit {
                        family: myth_contracts::Family::Staffing,
                        inspected_quarter: 2,
                    });
                    let checked = accepted_step(&service, id, kind, n).await;
                    let owner = service.owner(id, false).await.unwrap();
                    let audit_count = owner
                        .engine
                        .lock()
                        .await
                        .trial
                        .store
                        .world()
                        .fold()
                        .unwrap()
                        .player_audits()
                        .len();
                    assert_eq!(audit_count, 1);
                    assert!(
                        !serde_json::to_string(&checked)
                            .unwrap()
                            .contains("inspected_cumulative")
                    );
                    drop(owner);
                }
                let (kind, n) = step(CommandKind::ContinueQuarter);
                let chosen = accepted_step(&service, id, kind, n).await;
                assert!(chosen.continuation_pending);
                assert_eq!(chosen.delivery_history.len(), quarter as usize - 1);
                let owner = service.owner(id, false).await.unwrap();
                let engine = owner.engine.lock().await;
                let world = engine.trial.store.world();
                assert!(world.continuation_pending());
                assert!(
                    world
                        .selected_player_command(
                            "sneaky",
                            json!({
                                "kind":"propose", "meeting_id": format!("q{}", world.quarter),
                                "allocation":{"staff":0,"repairs":0,"access":0}
                            })
                        )
                        .is_err()
                );
                inspect(&engine.trial.store.dir).unwrap();
                drop(engine);
                drop(owner);
                let (kind, n) = step(CommandKind::FinishContinuation);
                let next = accepted_step(&service, id, kind, n).await;
                assert!(!next.continuation_pending);
                assert_eq!(next.delivery_history.len(), quarter as usize);
            }
            let owner = service.owner(id, false).await.unwrap();
            let snapshot = capture_for(&owner.engine.lock().await.trial, id).unwrap();
            // Six quarters are reported and the first term's election has been held: either the game is
            // over (a loss) or it waits on election night for the second term.
            assert!(snapshot.closed || snapshot.phase == "ElectionNight");
            assert_ne!(snapshot.election.status, "projected");
            assert_eq!(snapshot.delivery_history.len(), 6);
            // Q1 was ratified; Q2-Q6 each kept the programme, which is not a deferral.
            let labels: Vec<_> = snapshot
                .decisions
                .iter()
                .map(|d| (d.quarter, d.outcome.as_str()))
                .collect();
            assert_eq!(
                labels,
                [
                    (1, "ratified"),
                    (2, "continued"),
                    (3, "continued"),
                    (4, "continued"),
                    (5, "continued"),
                    (6, "continued"),
                ]
            );
            assert!(
                snapshot.decisions[1..]
                    .iter()
                    .all(|d| d.final_votes == 0 && d.allocation == snapshot.decisions[0].allocation)
            );
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
    async fn p7_two_scripted_terms_report_exactly_twelve_quarters_and_replay() {
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        // The calendar of every campaign is fixed by its name (decisions 0010 to 0012). This name was chosen so that the
        // scripted player wins both terms; another name might meet an inspection or a bad run of news and lose one.
        let owner = service.owner("six-report-1", true).await.unwrap();
        let final_view;
        let saved_hash;
        {
            let mut engine = owner.engine.lock().await;
            run_until_term_end(&mut engine.trial).await.unwrap();
            // The scripted player begins the next term; only the HTTP API refuses a generic step there.
            advance_slice(&mut engine.trial).await.unwrap();
            run_until_term_end(&mut engine.trial).await.unwrap();
            final_view = capture_for(&engine.trial, "six-report-1").unwrap();
            // Two terms won, so the game is not over: it waits on election night for a third.
            assert!(!final_view.closed);
            assert_eq!(final_view.phase, "ElectionNight");
            assert_eq!(final_view.quarter, 12);
            assert_eq!(final_view.past_elections.len(), 2);
            assert_eq!(final_view.delivery_history.len(), 12);
            assert_eq!(
                final_view
                    .delivery_history
                    .iter()
                    .map(|r| r.quarter)
                    .collect::<Vec<_>>(),
                (1..=12).collect::<Vec<_>>()
            );
            assert_eq!(final_view.news.len(), 3);
            assert_eq!(final_view.comparison.mode, "primary_campaign");
            let t = &final_view.treasury;
            // Two allocations, and any windfalls the news brought on the way (decision 0012).
            let windfalls: u32 = final_view
                .calendar
                .recent
                .iter()
                .map(|h| u32::from(h.grant_credits.unwrap_or(0)))
                .sum();
            assert_eq!(
                u32::from(t.available_credits)
                    + u32::from(t.encumbered_staff)
                    + u32::from(t.encumbered_repairs)
                    + u32::from(t.encumbered_access)
                    + u32::from(t.service_spent)
                    + u32::from(t.audit_spent)
                    + u32::from(t.relief_spent),
                200 + windfalls
            );
            saved_hash = inspect(&engine.trial.store.dir).unwrap().0;
        }
        drop(owner);
        drop(service);
        assert_eq!(inspect(&dir.join("six-report-1")).unwrap().0, saved_hash);
        let reopened = Connected::new(dir.clone()).unwrap();
        let restored = reopened.owner("six-report-1", false).await.unwrap();
        let view = capture_for(&restored.engine.lock().await.trial, "six-report-1").unwrap();
        assert_eq!(view.cursor, final_view.cursor);
        assert_eq!(view.view_version, final_view.view_version);
        assert_eq!(view.delivery_history.len(), 12);
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
                .owner("decision", true)
                .await
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
                    .owner("decision", true)
                    .await
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
    /// One plan, proposed and approved the way a player does it: propose, hear reactions, lock, final votes, approve, end the
    /// quarter. Returns the view after the quarter has ended.
    async fn play_plan(
        service: &Arc<Connected>,
        campaign: &str,
        allocation: WireAllocation,
        serial: &mut usize,
    ) -> PublicView {
        *serial += 1;
        let mut view = accepted_step(
            service,
            campaign,
            CommandKind::Propose { allocation },
            *serial,
        )
        .await;
        for kind in [
            CommandKind::AdvanceSlice,
            CommandKind::AdvanceSlice,
            CommandKind::AdvanceSlice,
            CommandKind::Decide { ratify: true },
            CommandKind::AdvanceSlice,
        ] {
            *serial += 1;
            view = accepted_step(service, campaign, kind, *serial).await;
        }
        view
    }
    /// Keep the programme through one quarter (no meeting) and end it.
    async fn keep_quarter(
        service: &Arc<Connected>,
        campaign: &str,
        serial: &mut usize,
    ) -> PublicView {
        *serial += 1;
        accepted_step(service, campaign, CommandKind::ContinueQuarter, *serial).await;
        *serial += 1;
        accepted_step(service, campaign, CommandKind::FinishContinuation, *serial).await
    }
    /// Writes the hand-written test feed (not a capture of any real feed) where a service can read it.
    fn feed_file(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("myth-feed-{}-{name}.xml", std::process::id()));
        fs::write(&path, crate::real_news::tests::FIXTURE).unwrap();
        path
    }
    #[tokio::test]
    async fn real_headlines_are_frozen_into_each_term_and_match_the_independent_replica() {
        // Chosen with tools/support-model/news_model.py: "rn112". The test feed has two stories a rule matches at the start (a
        // teacher pay award, good, and a RAAC survey, bad) and two that match nothing; term 1 cannot carry bad news, so only the
        // pay award takes a quarter (the first good one, 2). At the start of term 2 the stories already told are skipped, so the
        // strike ballot is the only one left, and it takes the first bad quarter (10). Every number is the replica's, to the ppm.
        let id = "rn112";
        let dir = root();
        let feed = feed_file("frozen");
        let service = Connected::with_news(
            dir.clone(),
            crate::real_news::NewsSource::File(feed.clone()),
        )
        .unwrap();
        let created = create(State(service.clone()), Path(id.to_owned()))
            .await
            .unwrap()
            .1
            .0
            .snapshot;
        let notes: Vec<_> = created
            .calendar
            .real_news
            .iter()
            .map(|n| (n.term, n.title.as_str(), n.rule_label.as_deref(), n.quarter))
            .collect();
        assert_eq!(
            notes,
            [
                (
                    1,
                    "Teacher pay award confirmed for next year",
                    Some("Teacher pay and recruitment improve"),
                    Some(2)
                ),
                (
                    1,
                    "RAAC survey finds more unsafe buildings",
                    Some("School buildings in trouble"),
                    None
                ),
                (1, "New guidance published for school governors", None, None),
                (1, "Consultation opens on the curriculum", None, None),
            ]
        );
        assert_eq!(created.calendar.real_news[0].published, "2026-11-02");
        assert_eq!(
            created.calendar.real_news[0].summary,
            "Schools will be funded for the award & given notice."
        );
        // Nothing is invented about where it came from: links outside GOV.UK never get in.
        assert!(
            created
                .calendar
                .real_news
                .iter()
                .all(|n| n.url.starts_with("https://www.gov.uk/"))
        );
        let mut serial = 0_usize;
        let balanced = WireAllocation {
            staff: 50,
            repairs: 40,
            access: 10,
        };
        let mut view = play_plan(&service, id, balanced, &mut serial).await;
        for _ in 2..=6 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        assert_eq!(view.phase, "ElectionNight");
        let pay = view
            .calendar
            .recent
            .iter()
            .find(|h| h.quarter == 2)
            .unwrap();
        assert_eq!(
            (
                pay.id.as_str(),
                pay.source.as_str(),
                pay.tone.as_str(),
                pay.surprise
            ),
            ("real_teacher_pay_good", "real", "good", false)
        );
        assert_eq!(pay.effect_ppm, [12_000; 6]);
        let source = pay
            .real
            .as_ref()
            .expect("a real headline says where it came from");
        assert_eq!(
            (
                source.publisher.as_str(),
                source.published.as_str(),
                source.rule_id.as_str()
            ),
            (
                "GOV.UK, Department for Education",
                "2026-11-02",
                "teacher_pay_good"
            )
        );
        assert_eq!(
            source.url,
            "https://www.gov.uk/government/news/teacher-pay-award-confirmed"
        );
        assert!(
            view.calendar
                .recent
                .iter()
                .filter(|h| h.real.is_some())
                .count()
                == 1
        );
        assert_eq!(
            view.support.group_ppm,
            [602_120, 613_600, 484_280, 603_536, 620_080, 495_824]
        );
        assert_eq!(
            view.election.settled_group_ppm,
            [603_800, 624_000, 484_200, 611_240, 637_600, 503_960]
        );
        assert_eq!(view.election.weighted_support_ppm, 582_184);
        // Beginning term 2 fetches again and freezes the new term's batch: only the strike ballot is left to tell.
        serial += 1;
        let second = accepted_step(&service, id, CommandKind::BeginTerm, serial).await;
        assert_eq!((second.quarter, second.term), (7, 2));
        let notes: Vec<_> = second
            .calendar
            .real_news
            .iter()
            .map(|n| (n.term, n.title.as_str(), n.quarter))
            .collect();
        assert_eq!(notes.len(), 5);
        assert_eq!(
            notes[4],
            (2, "Strike ballot opens among teachers", Some(10))
        );
        let reserve = WireAllocation {
            staff: 50,
            repairs: 35,
            access: 0,
        };
        let view = play_plan(&service, id, reserve, &mut serial).await;
        assert_eq!(view.treasury.available_credits, 15);
        let mut view = view;
        for _ in 8..=8 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        assert_eq!(view.quarter, 9);
        // A quarter ahead the real headline is announced, quoted and priced like any other bad news, and says it is real.
        let strike = view
            .calendar
            .upcoming
            .iter()
            .find(|h| h.quarter == 10)
            .expect("announced");
        assert_eq!(
            (
                strike.id.as_str(),
                strike.source.as_str(),
                strike.tone.as_str()
            ),
            ("real_teacher_supply_bad", "real", "bad")
        );
        assert_eq!(strike.title, "Strike ballot opens among teachers");
        assert_eq!(
            strike.effect_ppm,
            [-27_260, -29_300, -2_500, -14_374, -16_010, -1_646]
        );
        assert_eq!(strike.overall_ppm, -19_382);
        assert_eq!(strike.severity, "minor");
        assert_eq!(strike.relief_cost, Some(20));
        assert!(
            strike.disruption.is_none()
                && strike.scrutiny.is_none()
                && strike.grant_credits.is_none()
        );
        let source = strike.real.as_ref().unwrap();
        assert_eq!(
            (
                source.rule_id.as_str(),
                source.rule_label.as_str(),
                source.published.as_str()
            ),
            (
                "teacher_supply_bad",
                "Teacher supply under pressure",
                "2026-10-31"
            )
        );
        // The reserve is 15 credits and the answer costs 20: the game refuses, as it does for any headline.
        let refused = command(
            State(service.clone()),
            Path(id.to_owned()),
            Json(Command {
                command_id: "answer-too-dear".into(),
                expected_world_version: view.view_version.clone(),
                kind: CommandKind::Respond {
                    headline_quarter: 10,
                },
            }),
        )
        .await;
        assert_eq!(refused.unwrap_err().0, StatusCode::CONFLICT);
        view = keep_quarter(&service, id, &mut serial).await;
        assert_eq!(view.quarter, 10);
        let strike = view
            .calendar
            .upcoming
            .iter()
            .find(|h| h.quarter == 10)
            .unwrap();
        assert_eq!(
            strike.effect_ppm,
            [-33_236, -35_180, -2_500, -16_440, -18_182, -2_003]
        );
        assert_eq!(
            (
                strike.overall_ppm,
                strike.severity.as_str(),
                strike.relief_cost
            ),
            (-23_154, "serious", Some(24))
        );
        for _ in 10..=12 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        assert_eq!(
            (view.phase.as_str(), view.election.status.as_str()),
            ("ElectionNight", "won")
        );
        let landed = view
            .calendar
            .recent
            .iter()
            .find(|h| h.quarter == 10)
            .unwrap();
        assert_eq!(
            landed.effect_ppm,
            [-33_236, -35_180, -2_500, -16_440, -18_182, -2_003]
        );
        assert!(landed.real.is_some() && !landed.responded);
        assert_eq!(
            view.election.settled_group_ppm,
            [664_764, 701_820, 477_500, 637_060, 689_818, 497_838]
        );
        assert_eq!(view.election.weighted_support_ppm, 627_142);
        // The headlines are in the record, not fetched again: reopening the same save with real news switched off shows
        // exactly the same ones, and so does a service that could not reach any feed.
        let before = serde_json::to_value(&view.calendar).unwrap();
        drop(service);
        let reopened = Connected::new(dir.clone()).unwrap();
        let owner = reopened.owner(id, false).await.unwrap();
        let again = capture_for(&owner.engine.lock().await.trial, id).unwrap();
        assert_eq!(serde_json::to_value(&again.calendar).unwrap(), before);
        assert_eq!(again.view_version, view.view_version);
        drop(owner);
        drop(reopened);
        fs::remove_file(feed).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn a_feed_that_cannot_be_read_gives_a_campaign_with_no_real_news_and_nothing_breaks() {
        let dir = root();
        let missing = std::env::temp_dir().join("myth-feed-does-not-exist.xml");
        let service =
            Connected::with_news(dir.clone(), crate::real_news::NewsSource::File(missing)).unwrap();
        let created = create(State(service.clone()), Path("down".to_owned()))
            .await
            .unwrap()
            .1
            .0
            .snapshot;
        assert!(created.calendar.real_news.is_empty());
        let mut serial = 0_usize;
        let view = play_plan(
            &service,
            "down",
            WireAllocation {
                staff: 50,
                repairs: 40,
                access: 10,
            },
            &mut serial,
        )
        .await;
        assert_eq!(view.quarter, 2);
        assert!(
            view.calendar.real_news.is_empty()
                && view.calendar.recent.iter().all(|h| h.real.is_none())
        );
        drop(service);
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn inspections_and_windfalls_match_the_independent_replica() {
        // Chosen with tools/support-model/news_model.py: in term 1 an education grant lands in quarter 4 (15 credits); in
        // term 2 an inspection of building promises (quarter 10), a spot check on every promise (quarter 11) and a charity
        // donation (quarter 12) are all announced a quarter ahead, and nothing disrupts work. Every number is the replica's.
        let id = "sc156";
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let mut serial = 0_usize;
        let balanced = WireAllocation {
            staff: 50,
            repairs: 40,
            access: 10,
        };
        let mut view = play_plan(&service, id, balanced, &mut serial).await;
        for _ in 2..=6 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        assert_eq!(view.phase, "ElectionNight");
        assert_eq!(view.election.status, "won");
        assert_eq!(
            view.election.settled_group_ppm,
            [586_800, 617_000, 487_200, 594_240, 630_600, 506_960]
        );
        assert_eq!(view.election.weighted_support_ppm, 574_584);
        // The grant arrived in quarter 4 and nothing has used it: all 100 credits of the plan are spent, so only it is left.
        let landed: Vec<_> = view
            .calendar
            .recent
            .iter()
            .map(|h| (h.quarter, h.id.as_str(), h.grant_credits))
            .collect();
        assert_eq!(
            landed,
            [
                (3, "inspectorate_praise", None),
                (4, "education_grant", Some(15))
            ]
        );
        let grant = &view.calendar.recent[1];
        assert_eq!(grant.effect_ppm, [0; 6], "a windfall is credits, not mood");
        assert!(grant.scrutiny.is_none() && grant.disruption.is_none());
        assert_eq!(
            (view.treasury.available_credits, view.treasury.service_spent),
            (15, 100)
        );
        // Beginning term 2 adds a fresh allocation to it.
        serial += 1;
        let second = accepted_step(&service, id, CommandKind::BeginTerm, serial).await;
        assert_eq!((second.quarter, second.term), (7, 2));
        assert_eq!(second.treasury.available_credits, 115);
        // A plan that keeps 30 credits back to answer with.
        let plan = WireAllocation {
            staff: 50,
            repairs: 35,
            access: 0,
        };
        let view = play_plan(&service, id, plan, &mut serial).await;
        assert_eq!((view.quarter, view.treasury.available_credits), (8, 30));
        // Quarter 8's cost-of-living story lands; quarter 9 is open.
        let view = keep_quarter(&service, id, &mut serial).await;
        assert_eq!(view.quarter, 9);
        // A quarter ahead of the inspection of building promises, the card says what it will count and what that costs:
        // 15 repair units still promised after quarter 10's delivery.
        let audit = view
            .calendar
            .upcoming
            .iter()
            .find(|h| h.quarter == 10)
            .expect("announced");
        assert_eq!(
            (audit.id.as_str(), audit.tone.as_str()),
            ("building_promises_audit", "bad")
        );
        let count = audit.scrutiny.as_ref().expect("an inspection");
        assert_eq!(count.families, ["repairs"]);
        assert_eq!(count.owed_units, [0, 15, 0]);
        assert_eq!(count.effect_ppm, [0, -6_000, -600, -5_070, -12_300, -7_530]);
        assert_eq!(
            audit.effect_ppm,
            [-5_560, -14_702, -1_600, -14_036, -26_724, -9_865]
        );
        assert_eq!(audit.overall_ppm, -10_115);
        assert_eq!(audit.severity, "minor");
        assert_eq!(audit.relief_cost, Some(11));
        assert!(audit.disruption.is_none() && audit.grant_credits.is_none());
        // Leave it. Quarter 10 opens; the spot check on every promise is now announced too.
        let view = keep_quarter(&service, id, &mut serial).await;
        assert_eq!(view.quarter, 10);
        let audit = view
            .calendar
            .upcoming
            .iter()
            .find(|h| h.quarter == 10)
            .unwrap();
        assert_eq!(
            audit.effect_ppm,
            [-6_751, -15_854, -1_600, -14_715, -27_530, -9_030],
            "the quote moves with where support stands"
        );
        let spot = view
            .calendar
            .upcoming
            .iter()
            .find(|h| h.quarter == 11)
            .expect("announced");
        assert_eq!(spot.id, "spot_check_on_every_promise");
        let count = spot.scrutiny.as_ref().unwrap();
        assert_eq!(count.families, ["staffing", "repairs", "access"]);
        assert_eq!(count.owed_units, [2, 5, 0]);
        assert_eq!(
            count.effect_ppm,
            [-1_040, -3_200, 0, -2_162, -4_660, -1_558]
        );
        assert_eq!(
            spot.effect_ppm,
            [-9_479, -15_518, -1_250, -10_200, -17_352, -2_808]
        );
        assert_eq!(spot.overall_ppm, -9_947);
        assert_eq!(spot.relief_cost, Some(10));
        // Good news cannot be answered.
        let refused = command(
            State(service.clone()),
            Path(id.to_owned()),
            Json(Command {
                command_id: "respond-to-good-news".into(),
                expected_world_version: view.view_version.clone(),
                kind: CommandKind::Respond {
                    headline_quarter: 12,
                },
            }),
        )
        .await;
        assert_eq!(refused.unwrap_err().0, StatusCode::CONFLICT);
        // Answer the spot check, not the audit.
        serial += 1;
        let answered = accepted_step(
            &service,
            id,
            CommandKind::Respond {
                headline_quarter: 11,
            },
            serial,
        )
        .await;
        assert_eq!(answered.treasury.relief_spent, 10);
        assert_eq!(answered.treasury.available_credits, 20);
        let mut view = answered;
        for _ in 10..=11 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        assert_eq!(view.quarter, 12);
        let landed: Vec<_> = view
            .calendar
            .recent
            .iter()
            .filter(|h| h.quarter >= 8)
            .map(|h| (h.quarter, h.id.as_str(), h.responded))
            .collect();
        assert_eq!(
            landed,
            [
                (8, "cost_of_living", false),
                (10, "building_promises_audit", false),
                (11, "spot_check_on_every_promise", true)
            ]
        );
        let at = |q: u8| {
            view.calendar
                .recent
                .iter()
                .find(|h| h.quarter == q)
                .unwrap()
        };
        // The unanswered audit took the story and the count, as quoted.
        assert_eq!(
            at(10).effect_ppm,
            [-6_751, -15_854, -1_600, -14_715, -27_530, -9_030]
        );
        assert_eq!(at(10).scrutiny.as_ref().unwrap().owed_units, [0, 15, 0]);
        // The answered spot check took 60% less of both.
        assert_eq!(
            at(11).effect_ppm,
            [-4_084, -6_468, -500, -3_988, -6_764, -1_123]
        );
        assert_eq!(at(11).scrutiny.as_ref().unwrap().owed_units, [2, 5, 0]);
        // The charity donation arrives in the last quarter, and the second election is held on what is left.
        view = keep_quarter(&service, id, &mut serial).await;
        assert_eq!(view.phase, "ElectionNight");
        assert_eq!(view.election.status, "won");
        assert_eq!(
            view.election.settled_group_ppm,
            [605_375, 661_958, 465_900, 590_775, 658_810, 484_347]
        );
        assert_eq!(view.election.weighted_support_ppm, 589_998);
        let donation = view
            .calendar
            .recent
            .iter()
            .find(|h| h.quarter == 12)
            .unwrap();
        assert_eq!(
            (donation.id.as_str(), donation.grant_credits),
            ("charity_donation", Some(10))
        );
        // Every credit is in exactly one account: 100 + 100 allocated, 15 + 10 from windfalls.
        let t = &view.treasury;
        assert_eq!(
            (t.available_credits, t.service_spent, t.relief_spent),
            (30, 185, 10)
        );
        assert_eq!(
            u32::from(t.available_credits)
                + u32::from(t.encumbered_staff)
                + u32::from(t.encumbered_repairs)
                + u32::from(t.encumbered_access)
                + u32::from(t.service_spent)
                + u32::from(t.audit_spent)
                + u32::from(t.relief_spent),
            225
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn election_night_is_published_and_matches_the_independent_replica() {
        // Every campaign has its own news calendar, fixed by its name (decisions 0010 to 0012). "dz4827" was chosen with
        // tools/support-model/news_model.py for a second term that has an announced flood stopping Repairs in quarter 11,
        // no other disruption, no inspection and no windfall. Every number below is that independent replica's, to the ppm.
        let id = "dz4827";
        let dir = root();
        let service = Connected::new(dir.clone()).unwrap();
        let mut serial = 0_usize;
        let balanced = WireAllocation {
            staff: 50,
            repairs: 40,
            access: 10,
        };
        let mut view = play_plan(&service, id, balanced, &mut serial).await;
        // Q2 is open and the campaign is running: the election is only a projection.
        assert_eq!(view.quarter, 2);
        assert!(!view.closed);
        assert_eq!(view.election.status, "projected");
        assert_eq!(view.support.group_ppm, [500_000; 6]);
        // Q1's access work has been delivered and its response is queued: that is the "mood to come".
        assert!(view.support.pending_delta_ppm.iter().any(|d| *d != 0));
        // Then keep the programme for the other five quarters.
        for _ in 2..=6 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        // The first term is won, so the game waits on election night for the player to begin the second.
        assert!(!view.closed);
        assert_eq!(view.phase, "ElectionNight");
        assert_eq!((view.quarter, view.term), (6, 1));
        assert!(!view.election.ceiling);
        assert_eq!(view.election.status, "won");
        assert_eq!(view.election.term, 1);
        // The first term brings good news only: two headlines, announced a quarter ahead.
        assert_eq!(
            view.election.settled_group_ppm,
            [591_800, 612_000, 472_200, 624_240, 650_600, 516_960]
        );
        assert_eq!(view.election.weighted_support_ppm, 575_184);
        assert_eq!(view.election.blocs_held_ppm, 780_000);
        // Before the election the last quarter's response has not landed yet.
        assert_eq!(
            view.support.group_ppm,
            [590_120, 601_600, 472_280, 616_536, 633_080, 508_824]
        );
        let landed: Vec<_> = view
            .calendar
            .recent
            .iter()
            .map(|h| {
                (
                    h.quarter,
                    h.id.as_str(),
                    h.tone.as_str(),
                    h.surprise,
                    h.responded,
                )
            })
            .collect();
        assert_eq!(
            landed,
            [
                (3, "volunteer_surge", "good", true, false),
                (5, "attendance_rises", "good", false, false)
            ]
        );
        assert!(
            view.calendar
                .recent
                .iter()
                .all(|h| h.source == "invented" && !h.title.is_empty() && h.disruption.is_none())
        );
        assert!(
            view.calendar.upcoming.is_empty(),
            "nothing is announced on election night"
        );
        for i in 0..6 {
            assert_eq!(
                i64::from(view.support.pending_delta_ppm[i]),
                i64::from(view.election.settled_group_ppm[i])
                    - i64::from(view.support.group_ppm[i])
            );
        }
        assert_eq!(
            view.support.weight_ppm,
            [260_000, 320_000, 220_000, 70_000, 80_000, 50_000]
        );
        assert_eq!(view.support.weight_ppm.iter().sum::<u32>(), 1_000_000);
        assert_eq!(view.support.group_ids.len(), 6);
        assert_eq!(view.support.group_ids[2], "urban_high");
        assert_eq!(view.election.min_support_ppm, ELECTION_MIN_SUPPORT_PPM);
        assert_eq!(view.past_elections.len(), 1);
        assert!(view.past_elections[0].won);
        assert_eq!(view.past_elections[0].weighted_support_ppm, 575_184);
        // What the browser receives.
        let wire = serde_json::to_value(&view).unwrap();
        assert_eq!(wire["election"]["status"], "won");
        assert_eq!(wire["support"]["group_ids"][5], "rural_high");
        assert!(
            wire.get("terms").is_none(),
            "the number of terms is not announced: it is not fixed"
        );
        assert_eq!(wire["election"]["ceiling"], false);
        // A generic step must never begin the next term on the person's behalf, and neither may the other quarter commands.
        for (n, kind) in [
            CommandKind::AdvanceSlice,
            CommandKind::ContinueQuarter,
            CommandKind::Decide { ratify: true },
        ]
        .into_iter()
        .enumerate()
        {
            let refused = command(
                State(service.clone()),
                Path(id.to_owned()),
                Json(Command {
                    command_id: format!("refused-{n}"),
                    expected_world_version: view.view_version.clone(),
                    kind,
                }),
            )
            .await;
            assert_eq!(refused.unwrap_err().0, StatusCode::CONFLICT);
        }
        // Begin the second term: a fresh allocation is granted on top of what was left, and the earlier result is kept.
        let before = view.treasury.available_credits;
        serial += 1;
        let second = accepted_step(&service, id, CommandKind::BeginTerm, serial).await;
        assert_eq!((second.quarter, second.term), (7, 2));
        assert_eq!(second.phase, "QuarterOpen");
        assert_eq!(second.treasury.available_credits, before + 100);
        assert_eq!(second.election.status, "projected");
        assert_eq!(second.election.term, 2);
        assert_eq!(second.past_elections.len(), 1);
        // Nothing is rewritten at the boundary: the last quarter's queued response lands during the second term's first quarter.
        assert_eq!(second.support.group_ppm, view.support.group_ppm);
        assert_eq!(
            second.support.pending_delta_ppm,
            view.support.pending_delta_ppm
        );
        // The term's first bad headline is announced already, a mood story with a price. Leave it.
        let up = &second.calendar.upcoming;
        assert_eq!(up.len(), 1, "{up:?}");
        assert_eq!(
            (up[0].quarter, up[0].id.as_str(), up[0].tone.as_str()),
            (8, "funding_row", "bad")
        );
        assert!(up[0].disruption.is_none());
        assert_eq!(up[0].relief_cost, Some(18));
        // Plan with a reserve: 50/35/0 still wins the cabinet and keeps 15 credits back to answer a headline with. (Fewer than 50
        // for Staffing would let the School Staff Association slow it down, which the replica does not model.)
        let reserve = WireAllocation {
            staff: 50,
            repairs: 35,
            access: 0,
        };
        let view = play_plan(&service, id, reserve, &mut serial).await;
        assert_eq!(view.quarter, 8);
        assert_eq!(view.treasury.available_credits, 15);
        let mut view = view;
        for _ in 8..=9 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        assert_eq!(view.quarter, 10);
        // A quarter ahead of a flood that stops Repairs for two quarters, the screen says what it would do to this plan: ten
        // repair units a quarter from quarter 9 were promised, so 15 of those still to come would land after the election.
        let flood = view
            .calendar
            .upcoming
            .iter()
            .find(|h| h.quarter == 11)
            .expect("announced");
        assert_eq!(
            (flood.id.as_str(), flood.tone.as_str()),
            ("flooding_halts_repairs", "bad")
        );
        let cut = flood.disruption.as_ref().expect("a disruption");
        assert_eq!(
            (
                cut.family.as_str(),
                cut.keep_pct,
                cut.quarters,
                cut.delayed_units
            ),
            ("repairs", 0, 2, 15)
        );
        // The mood the story takes plus what the delayed work would have earned, taken away: tools/support-model/news_model.py.
        assert_eq!(
            cut.effect_ppm,
            [1_200, -12_000, -1_200, -10_140, -24_600, -15_060]
        );
        assert_eq!(
            flood.effect_ppm,
            [-3_532, -16_960, -1_700, -26_346, -42_264, -16_560]
        );
        assert_eq!(flood.overall_ppm, -12_772);
        assert_eq!(flood.severity, "minor");
        assert_eq!(flood.relief_cost, Some(13));
        // Answer it: 13 credits, and the stoppage becomes a slowdown (60% less cut).
        let money_before = view.treasury.available_credits;
        serial += 1;
        let answered = accepted_step(
            &service,
            id,
            CommandKind::Respond {
                headline_quarter: 11,
            },
            serial,
        )
        .await;
        assert_eq!(answered.treasury.relief_spent, 13);
        assert_eq!(answered.treasury.available_credits, money_before - 13);
        let paid = answered
            .calendar
            .upcoming
            .iter()
            .find(|h| h.quarter == 11)
            .unwrap();
        assert_eq!((paid.paid, paid.relief_cost), (Some(13), None));
        // Once answered, the card shows what the answer bought: a 60% slowdown instead of a stoppage, and only 3 of the 25
        // units still to come arriving after the election instead of 15.
        let bought = paid.disruption.as_ref().unwrap();
        assert_eq!((bought.keep_pct, bought.delayed_units), (60, 3));
        assert!(
            paid.overall_ppm > flood.overall_ppm,
            "an answered headline is expected to cost less"
        );
        assert_eq!(
            paid.effect_ppm,
            [-1_652, -4_384, -440, -8_510, -11_985, -3_612]
        );
        assert_eq!(paid.overall_ppm, -3_664);
        // Answering twice, a quarter with nothing announced, and a quarter that does not exist are all refused.
        for (n, quarter) in [(1_u8, 11_u8), (2, 12), (3, 30)] {
            let refused = command(
                State(service.clone()),
                Path(id.to_owned()),
                Json(Command {
                    command_id: format!("respond-refused-{n}"),
                    expected_world_version: answered.view_version.clone(),
                    kind: CommandKind::Respond {
                        headline_quarter: quarter,
                    },
                }),
            )
            .await;
            assert_eq!(
                refused.unwrap_err().0,
                StatusCode::CONFLICT,
                "quarter {quarter}"
            );
        }
        let mut view = answered;
        for _ in 10..=12 {
            view = keep_quarter(&service, id, &mut serial).await;
        }
        // The second election is won, so the game goes on: term 3 is waiting.
        assert!(!view.closed);
        assert_eq!(view.phase, "ElectionNight");
        assert_eq!((view.quarter, view.term), (12, 2));
        assert_eq!(view.election.status, "won");
        assert_eq!(
            view.election.settled_group_ppm,
            [648_262, 680_358, 450_060, 616_966, 662_205, 489_996]
        );
        assert_eq!(view.election.weighted_support_ppm, 605_939);
        let landed: Vec<_> = view
            .calendar
            .recent
            .iter()
            .filter(|h| h.quarter >= 7)
            .map(|h| (h.quarter, h.id.as_str(), h.responded))
            .collect();
        assert_eq!(
            landed,
            [
                (8, "funding_row", false),
                (9, "attendance_rises", false),
                (10, "rural_bus_cuts", false),
                (11, "flooding_halts_repairs", true)
            ]
        );
        // The answered flood took 60% less, and slowed Repairs to 60% for two quarters instead of stopping it.
        let flooded = view
            .calendar
            .recent
            .iter()
            .find(|h| h.quarter == 11)
            .unwrap();
        assert_eq!(
            flooded.effect_ppm,
            [-2_118, -2_342, -200, -5_158, -6_255, -600]
        );
        let cut = flooded.disruption.as_ref().unwrap();
        assert_eq!(
            (cut.family.as_str(), cut.keep_pct, cut.quarters),
            ("repairs", 60, 2)
        );
        // The delivery is as the replica predicts: 32 of the 35 repair units arrived (10, 10, then 6 and 6 in the slowdown).
        let t = &view.treasury;
        assert_eq!(t.service_spent, 100 + 50 + 32);
        assert_eq!(
            (t.encumbered_repairs, t.relief_spent, t.available_credits),
            (3, 13, 2)
        );
        // Every credit is still in exactly one account, answers included.
        assert_eq!(
            u32::from(t.available_credits)
                + u32::from(t.encumbered_staff)
                + u32::from(t.encumbered_repairs)
                + u32::from(t.encumbered_access)
                + u32::from(t.service_spent)
                + u32::from(t.audit_spent)
                + u32::from(t.relief_spent),
            200
        );
        // And term 3 can begin, with both wins on record.
        serial += 1;
        let third = accepted_step(&service, id, CommandKind::BeginTerm, serial).await;
        assert_eq!((third.quarter, third.term), (13, 3));
        assert_eq!(third.past_elections.len(), 2);
        assert!(third.past_elections.iter().all(|e| e.won));
        fs::remove_dir_all(dir).unwrap();
    }
}
