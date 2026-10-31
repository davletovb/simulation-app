//! P5 shared, installation-owned allowance. This is not Hiyal's per-trial
//! budget and never attempts to infer provider billing from token usage.
//! The ledger is the authority; projections and trial receipts are not.
use hiyal::{effects::ModelRequest, guard::BudgetConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const INITIAL_MICRO_USD: u64 = 10_000_000;
const MAX_RECORDS: usize = 2_048;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const QUARTER_MICRO_USD: u64 = 250_000;
const CAMPAIGN_MICRO_USD: u64 = 1_500_000;
const BATCH_MICRO_USD: u64 = 3_000_000;

fn valid_id(id: &str) -> bool {
    (1..=100).contains(&id.len())
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b':' | b'/'))
}
fn reject(message: &str) -> String {
    message.to_owned()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    Minister,
    Assistant,
    Authoring,
    Evaluation,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub purpose: Purpose,
    pub campaign_id: Option<String>,
    pub quarter: Option<u8>,
    pub batch_id: Option<String>,
}
impl Scope {
    fn validate(&self) -> Result<(), String> {
        if self.campaign_id.as_deref().is_some_and(|x| !valid_id(x))
            || self.batch_id.as_deref().is_some_and(|x| !valid_id(x))
            || self.quarter.is_some_and(|q| !(1..=6).contains(&q))
            || self.quarter.is_some() != self.campaign_id.is_some()
            || (self.purpose == Purpose::Minister && self.campaign_id.is_none())
            || (self.purpose == Purpose::Evaluation && self.batch_id.is_none())
        {
            return Err(reject("invalid call scope"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedRoute {
    pub model: String,
    pub provider: String,
    pub input_micro_usd_per_million: u64,
    pub output_micro_usd_per_million: u64,
    pub reasoning_disabled: bool,
}
impl PinnedRoute {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_id(&self.model) || !valid_id(&self.provider)
            || self.input_micro_usd_per_million == 0
            || self.input_micro_usd_per_million > 1_000_000
            || self.output_micro_usd_per_million == 0
            || self.output_micro_usd_per_million > 5_000_000
            || !self.reasoning_disabled
        {
            return Err(reject("unapproved, unpriced or unbounded model route"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Reserved,
    Dispatched,
    Recorded,
    Verified,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub call_id: String,
    pub reservation_id: String,
    pub scope: Scope,
    pub epoch_id: u64,
    pub request_sha256: String,
    pub route: PinnedRoute,
    pub maximum_micro_usd: u64,
    pub status: Status,
    pub reported_input_tokens: Option<u64>,
    pub reported_output_tokens: Option<u64>,
    pub estimated_charge_micro_usd: Option<u64>,
    pub verified_charge_micro_usd: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema_version: u16,
    installation_id: String,
    application_allowance_id: String,
    epoch_id: u64,
    epoch_funded_micro_usd: u64,
    calls: Vec<Call>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    payload: State,
    sha256: String,
}
pub struct Admission {
    pub call: Call,
    /// Reopening a call ID is never permission to dispatch it again.
    pub fresh: bool,
}
/// One owner holds an OS file lock for the complete ledger lifetime.
/// The lock file is independent of the atomically renamed data file.
pub struct Allowance {
    dir: PathBuf,
    _lock: File,
    state: State,
    poisoned: bool,
}
fn lock_write_anchor(lock: &mut File, installation: &str) -> Result<(), String> {
    lock.write_all(format!("myth-p5-allowance-v1:{installation}").as_bytes())
        .map_err(|e| e.to_string())?;
    lock.sync_all().map_err(|e| e.to_string())
}
impl Allowance {
    pub fn open(dir: &Path, installation: &str) -> Result<Self, String> {
        if !valid_id(installation) {
            return Err(reject("invalid installation id"));
        }
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let mut lock = OpenOptions::new().read(true).write(true).create(true)
            .truncate(false).open(dir.join("application-allowance.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock().map_err(|_| reject("allowance already owned by a process"))?;
        let data = dir.join("application-allowance.json");
        let state = if data.exists() {
            if fs::read(dir.join("application-allowance.lock")).map_err(|e| e.to_string())?
                != format!("myth-p5-allowance-v1:{installation}").as_bytes()
            {
                return Err(reject("missing or mismatched installation anchor"));
            }
            let bytes = fs::read(&data).map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_FILE_BYTES {
                return Err(reject("allowance file too large"));
            }
            let value: Envelope = serde_json::from_slice(&bytes)
                .map_err(|_| reject("allowance recovery failed"))?;
            let computed = hash(&serde_json::to_vec(&value.payload).map_err(|e| e.to_string())?);
            if computed != value.sha256 || value.payload.schema_version != 1
                || value.payload.installation_id != installation
                || value.payload.application_allowance_id != format!("installation:{installation}")
                || value.payload.epoch_id == 0
                || value.payload.epoch_funded_micro_usd > INITIAL_MICRO_USD
                || value.payload.calls.len() > MAX_RECORDS
            {
                return Err(reject("allowance recovery failed integrity or identity checks"));
            }
            let mut ids = std::collections::BTreeSet::new();
            let mut reservations = std::collections::BTreeSet::new();
            for call in &value.payload.calls {
                call.scope.validate()?;
                call.route.validate()?;
                if !valid_id(&call.call_id) || !valid_id(&call.reservation_id)
                    || !ids.insert(&call.call_id) || !reservations.insert(&call.reservation_id)
                    || call.epoch_id == 0 || call.epoch_id > value.payload.epoch_id
                    || call.maximum_micro_usd == 0
                    || call.maximum_micro_usd > INITIAL_MICRO_USD
                    || call.request_sha256.len() != 64
                    || (call.status == Status::Verified) != call.verified_charge_micro_usd.is_some()
                    || call.verified_charge_micro_usd.is_some_and(|n| n > call.maximum_micro_usd)
                {
                    return Err(reject("invalid durable allowance entry"));
                }
            }
            value.payload
        } else {
            if lock.metadata().map_err(|e| e.to_string())?.len() != 0 {
                return Err(reject("missing previously initialized allowance ledger"));
            }
            // Never silently initialize a previously used installation: caller
            // must deliberately provision a fresh empty directory.
            if fs::read_dir(dir).map_err(|e| e.to_string())?
                .any(|x| x.is_ok_and(|v| v.file_name().to_string_lossy() != "application-allowance.lock"))
            {
                return Err(reject("missing ledger in nonempty allowance directory"));
            }
            State {
                schema_version: 1,
                installation_id: installation.into(),
                application_allowance_id: format!("installation:{installation}"),
                epoch_id: 1,
                epoch_funded_micro_usd: INITIAL_MICRO_USD,
                calls: Vec::new(),
            }
        };
        let mut ledger = Self { dir: dir.into(), _lock: lock, state, poisoned: false };
        ledger.check_recovery()?;
        if !data.exists() {
            // Persist the installation marker before the ledger; a crash in
            // between makes subsequent opens fail closed, not re-fund.
            lock_write_anchor(&mut ledger._lock, installation)?;
            ledger.persist()?;
        }
        Ok(ledger)
    }

    fn chargeable_in_current_epoch(&self, call: &Call) -> bool {
        call.epoch_id == self.state.epoch_id || call.status != Status::Verified
    }
    fn held(&self) -> Result<u64, String> {
        self.state.calls.iter().filter(|c| self.chargeable_in_current_epoch(c))
            .try_fold(0u64, |sum, c| sum.checked_add(c.maximum_micro_usd)
                .ok_or_else(|| reject("allowance overflow")))
    }
    fn check_recovery(&self) -> Result<(), String> {
        if self.held()? > self.state.epoch_funded_micro_usd {
            return Err(reject("recovered allowance exceeded funded amount"));
        }
        for call in &self.state.calls {
            let matches = self.state.calls.iter().filter(|x| {
                x.scope.campaign_id == call.scope.campaign_id && x.scope.campaign_id.is_some()
                    && x.scope.quarter == call.scope.quarter
            });
            let sum = matches.try_fold(0u64, |a, x| a.checked_add(x.maximum_micro_usd))
                .ok_or("quarter overflow")?;
            if call.scope.quarter.is_some() && sum > QUARTER_MICRO_USD {
                return Err(reject("recovered quarter exceeds cap"));
            }
        }
        Ok(())
    }
    pub fn remaining_micro_usd(&self) -> Result<u64, String> {
        self.state.epoch_funded_micro_usd.checked_sub(self.held()?)
            .ok_or_else(|| reject("allowance exhausted"))
    }
    pub fn epoch_id(&self) -> u64 {
        self.state.epoch_id
    }
    pub fn calls(&self) -> &[Call] {
        &self.state.calls
    }
    fn persist(&mut self) -> Result<(), String> {
        if self.poisoned {
            return Err(reject("ledger previously failed to persist"));
        }
        let attempt = (|| -> Result<(), String> {
            let body = serde_json::to_vec(&self.state).map_err(|e| e.to_string())?;
            let wrapper = serde_json::to_vec(&Envelope {
                payload: serde_json::from_slice(&body).map_err(|e| e.to_string())?,
                sha256: hash(&body),
            }).map_err(|e| e.to_string())?;
            if wrapper.len() as u64 > MAX_FILE_BYTES {
                return Err(reject("ledger capacity exceeded"));
            }
            let temp = self.dir.join("application-allowance.new");
            let mut f = OpenOptions::new().write(true).create(true).truncate(true)
                .open(&temp).map_err(|e| e.to_string())?;
            f.write_all(&wrapper).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
            fs::rename(temp, self.dir.join("application-allowance.json"))
                .map_err(|e| e.to_string())?;
            #[cfg(unix)]
            File::open(&self.dir).and_then(|d| d.sync_all()).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if attempt.is_err() {
            self.poisoned = true;
        }
        attempt
    }
    fn ensure_available(&self) -> Result<(), String> {
        if self.poisoned { Err(reject("allowance persistence failed")) } else { Ok(()) }
    }
    /// Durable before network dispatch. A repeated call_id is read-only, even
    /// if the previous attempt crashed while reserved or dispatched.
    pub fn reserve(
        &mut self, call_id: &str, scope: Scope, route: PinnedRoute,
        request: &ModelRequest,
    ) -> Result<Admission, String> {
        self.ensure_available()?;
        scope.validate()?;
        route.validate()?;
        if !valid_id(call_id) || request.provider != "openrouter"
            || request.model != route.model || request.streaming
            || !request.tool_schemas.is_empty()
            || request.response_format.as_ref().is_none_or(|v| v["type"] != "json_schema"
                || v["json_schema"]["strict"] != true)
            || request.sampling.max_output_tokens.is_none()
        {
            return Err(reject("unapproved OpenRouter structured request"));
        }
        let assistant = scope.purpose != Purpose::Minister;
        let max_output = if assistant { 800 } else { 400 };
        // Conservative tokenizer-independent guard: bytes count as tokens.
        // Hiyal independently reserves with framing overhead.
        let max_input = if assistant { 5_000 } else { 3_000 };
        if request.sampling.max_output_tokens.is_none_or(|x| x == 0 || x > max_output)
            || serde_json::to_vec(request).map_err(|e| e.to_string())?.len() > max_input
        {
            return Err(reject("declared request input/output cap exceeded"));
        }
        let config = BudgetConfig {
            maximum_micro_usd: CAMPAIGN_MICRO_USD,
            prompt_micro_usd_per_token: 1,
            output_micro_usd_per_token: 5,
            max_input_bytes: if assistant { 20_480 } else { 12_288 },
            max_output_tokens: max_output,
        };
        let maximum = config.reserve_amount(request)?;
        let digest = hash(&serde_json::to_vec(request).map_err(|e| e.to_string())?);
        if let Some(old) = self.state.calls.iter().find(|x| x.call_id == call_id) {
            if old.scope != scope || old.route != route || old.request_sha256 != digest
                || old.maximum_micro_usd != maximum
            {
                return Err(reject("call ID reused with changed request"));
            }
            return Ok(Admission { call: old.clone(), fresh: false });
        }
        if self.state.calls.len() >= MAX_RECORDS {
            return Err(reject("call journal full"));
        }
        if self.remaining_micro_usd()? < maximum {
            return Err(reject("shared application allowance exhausted"));
        }
        if let Some(campaign) = &scope.campaign_id {
            let previous: Vec<&Call> = self.state.calls.iter()
                .filter(|c| c.scope.campaign_id.as_ref() == Some(campaign)).collect();
            if previous.len() >= 96 {
                return Err(reject("campaign call cap exhausted"));
            }
            let used = previous.iter().try_fold(0u64, |n, c| n.checked_add(c.maximum_micro_usd))
                .ok_or("campaign overflow")?;
            if used.checked_add(maximum).is_none_or(|n| n > CAMPAIGN_MICRO_USD) {
                return Err(reject("campaign allowance exhausted"));
            }
            if let Some(q) = scope.quarter {
                let within: Vec<&&Call> = previous.iter().filter(|c| c.scope.quarter == Some(q)).collect();
                if within.len() >= 16 || (scope.purpose == Purpose::Minister
                    && within.iter().filter(|c| c.scope.purpose == Purpose::Minister).count() >= 14)
                {
                    return Err(reject("quarter call cap exhausted"));
                }
                let used = within.iter().try_fold(0u64, |n, c| n.checked_add(c.maximum_micro_usd))
                    .ok_or("quarter overflow")?;
                if used.checked_add(maximum).is_none_or(|n| n > QUARTER_MICRO_USD) {
                    return Err(reject("quarter allowance exhausted"));
                }
            }
        }
        if let Some(batch) = &scope.batch_id {
            let used = self.state.calls.iter().filter(|c| c.scope.batch_id.as_ref() == Some(batch))
                .try_fold(0u64, |n, c| n.checked_add(c.maximum_micro_usd))
                .ok_or("batch overflow")?;
            if used.checked_add(maximum).is_none_or(|n| n > BATCH_MICRO_USD) {
                return Err(reject("evaluation batch exhausted"));
            }
        }
        let call = Call {
            call_id: call_id.into(),
            reservation_id: format!("r-{}-{}", self.state.epoch_id, self.state.calls.len() + 1),
            scope,
            epoch_id: self.state.epoch_id,
            request_sha256: digest,
            route,
            maximum_micro_usd: maximum,
            status: Status::Reserved,
            reported_input_tokens: None,
            reported_output_tokens: None,
            estimated_charge_micro_usd: None,
            verified_charge_micro_usd: None,
        };
        self.state.calls.push(call.clone());
        self.persist()?;
        Ok(Admission { call, fresh: true })
    }
    fn entry(&mut self, call_id: &str) -> Result<&mut Call, String> {
        self.state.calls.iter_mut().find(|c| c.call_id == call_id)
            .ok_or_else(|| reject("unknown call ID"))
    }
    /// Write a durable dispatch marker before sending bytes. Subsequent opens
    /// must not redispatch a marked or previously reserved attempt.
    pub fn mark_dispatched(&mut self, call_id: &str) -> Result<(), String> {
        self.ensure_available()?;
        let entry = self.entry(call_id)?;
        if entry.status != Status::Reserved { return Err(reject("attempt already dispatched")); }
        entry.status = Status::Dispatched;
        self.persist()
    }
    /// A response without a verified invoice remains an unresolved charge.
    /// No reported token count ever refunds an upper-bound reservation.
    pub fn record(&mut self, call_id: &str, input: Option<u64>, output: Option<u64>,
        estimate_micro_usd: Option<u64>) -> Result<(), String> {
        self.ensure_available()?;
        let entry = self.entry(call_id)?;
        if entry.status != Status::Dispatched { return Err(reject("receipt already recorded or no dispatch")); }
        entry.status = Status::Recorded;
        entry.reported_input_tokens = input;
        entry.reported_output_tokens = output;
        entry.estimated_charge_micro_usd = estimate_micro_usd;
        self.persist()
    }
    /// An operator-verified invoice only changes liability across *future*
    /// explicit epochs; it never refunds the original Hiyal trial cap.
    pub fn verify_charge(&mut self, call_id: &str, amount: u64) -> Result<(), String> {
        self.ensure_available()?;
        let entry = self.entry(call_id)?;
        if entry.status != Status::Recorded || amount > entry.maximum_micro_usd {
            return Err(reject("unverified or excessive invoice"));
        }
        entry.verified_charge_micro_usd = Some(amount);
        entry.status = Status::Verified;
        self.persist()
    }
    /// Explicit funded reset; any unverified old attempt remains a liability.
    pub fn start_funded_epoch(&mut self, funded_micro_usd: u64) -> Result<u64, String> {
        self.ensure_available()?;
        if funded_micro_usd == 0 || funded_micro_usd > INITIAL_MICRO_USD {
            return Err(reject("invalid explicit funding"));
        }
        let liability = self.state.calls.iter().filter(|c| c.status != Status::Verified)
            .try_fold(0u64, |n, c| n.checked_add(c.maximum_micro_usd))
            .ok_or("liability overflow")?;
        if liability > funded_micro_usd {
            return Err(reject("new epoch cannot cover unresolved liabilities"));
        }
        self.state.epoch_id = self.state.epoch_id.checked_add(1).ok_or("epoch overflow")?;
        self.state.epoch_funded_micro_usd = funded_micro_usd;
        self.persist()?;
        Ok(self.state.epoch_id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use hiyal::effects::{Message, Role, Sampling};
    fn request() -> ModelRequest {
        ModelRequest {
            provider: "openrouter".into(), model: "test/model".into(),
            resolved_model_version: Some("fixed".into()),
            messages: vec![Message { role: Role::User, content: "typed role-visible view".into(), tool_call_id: None }],
            sampling: Sampling { temperature: Some(0.0), top_p: None, seed: Some(1), max_output_tokens: Some(400) },
            tool_schemas: vec![], streaming: false,
            response_format: Some(serde_json::json!({"type":"json_schema","json_schema":{"strict":true,"schema":{"type":"object"}}})),
        }
    }
    fn route() -> PinnedRoute {
        PinnedRoute { model: "test/model".into(), provider: "pinned-provider".into(),
            input_micro_usd_per_million: 1_000_000, output_micro_usd_per_million: 5_000_000,
            reasoning_disabled: true }
    }
    fn scope(purpose: Purpose, campaign: Option<&str>, quarter: Option<u8>) -> Scope {
        Scope { purpose, campaign_id: campaign.map(str::to_owned), quarter, batch_id: None }
    }
    fn temporary() -> PathBuf {
        let p = std::env::temp_dir().join(format!("myth-p5-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = fs::remove_dir_all(&p);
        p
    }
    #[test]
    fn campaigns_and_outside_calls_survive_both_opening_orders() {
        for reverse in [false, true] {
            let path = temporary().join(if reverse { "reverse" } else { "forward" });
            let mut ledger = Allowance::open(&path, "owner").unwrap();
            let cases = [
                ("c1", scope(Purpose::Minister, Some("campaign-a"), Some(1))),
                ("c2", scope(Purpose::Minister, Some("campaign-b"), Some(1))),
                ("asst", scope(Purpose::Assistant, None, None)),
                ("author", scope(Purpose::Authoring, None, None)),
            ];
            for (id, s) in cases {
                assert!(ledger.reserve(id, s, route(), &request()).unwrap().fresh);
                ledger.mark_dispatched(id).unwrap();
            }
            ledger.record("c1", Some(11), Some(9), Some(5)).unwrap();
            let prior = ledger.remaining_micro_usd().unwrap();
            assert!(Allowance::open(&path, "owner").is_err());
            drop(ledger);
            let mut restarted = Allowance::open(&path, "owner").unwrap();
            assert_eq!(restarted.remaining_micro_usd().unwrap(), prior);
            let entries = if reverse { ["c2", "c1"] } else { ["c1", "c2"] };
            for id in entries {
                let s = scope(Purpose::Minister, Some(if id == "c1" { "campaign-a" } else { "campaign-b" }), Some(1));
                assert!(!restarted.reserve(id, s, route(), &request()).unwrap().fresh);
                assert!(restarted.mark_dispatched(id).is_err());
            }
            restarted.verify_charge("c1", 5).unwrap();
            restarted.start_funded_epoch(INITIAL_MICRO_USD).unwrap();
            let unresolved = restarted.calls().iter().filter(|c| c.status != Status::Verified)
                .map(|c| c.maximum_micro_usd).sum::<u64>();
            assert_eq!(restarted.remaining_micro_usd().unwrap(), INITIAL_MICRO_USD - unresolved);
            drop(restarted);
            let recovered = Allowance::open(&path, "owner").unwrap();
            assert_eq!(recovered.remaining_micro_usd().unwrap(), INITIAL_MICRO_USD - unresolved);
            let _ = fs::remove_dir_all(&path);
        }
    }
    #[test]
    fn fails_closed_for_corruption_and_changed_retry() {
        let path = temporary();
        let mut ledger = Allowance::open(&path, "owner").unwrap();
        ledger.reserve("call", scope(Purpose::Minister, Some("one"), Some(1)), route(), &request()).unwrap();
        assert!(ledger.reserve("call", scope(Purpose::Minister, Some("two"), Some(1)), route(), &request()).is_err());
        let saved = fs::read(path.join("application-allowance.json")).unwrap();
        drop(ledger);
        fs::write(path.join("application-allowance.json"), b"{}").unwrap();
        assert!(Allowance::open(&path, "owner").is_err());
        fs::write(path.join("application-allowance.json"), saved).unwrap();
        assert!(Allowance::open(&path, "owner").is_ok());
        fs::remove_file(path.join("application-allowance.json")).unwrap();
        assert!(Allowance::open(&path, "owner").is_err());
        let _ = fs::remove_dir_all(&path);
    }
    #[test]
    fn pre_dispatch_rejections_and_uncertain_across_resets() {
        let path = temporary();
        let mut ledger = Allowance::open(&path, "owner").unwrap();
        let mut bad = request();
        bad.streaming = true;
        assert!(ledger.reserve("bad", scope(Purpose::Minister, Some("a"), Some(1)), route(), &bad).is_err());
        ledger.reserve("reserved", scope(Purpose::Minister, Some("a"), Some(1)), route(), &request()).unwrap();
        let held = ledger.calls()[0].maximum_micro_usd;
        ledger.start_funded_epoch(INITIAL_MICRO_USD).unwrap();
        assert_eq!(ledger.remaining_micro_usd().unwrap(), INITIAL_MICRO_USD - held);
        drop(ledger);
        let _ = fs::remove_dir_all(path);
    }
}
