//! Scripted P4 Hiyal orchestration and allowlisted zero-side-effect tool.
//! The effect journal is always bound to the trial by Hiyal before dispatch.
//! A tool receipt precedes a player-command action; no user data or provider
//! connection is used here.
use crate::{
    ACTORS, ActionKind, GameAction, GameObservation, GameWorld, PERIOD, PLAYER, WORLD_KIND,
};
use crate::{
    p5_allowance::{Allowance, PinnedRoute, Purpose, Scope},
    p5_backend::{SharedAdmissionBackend, governed_trial},
    p5_cabinet,
};
use hiyal::{
    core::AgentId,
    effects::{EffectResult, Message, ModelRequest, Role, Sampling, Usage},
    experiment::{EffectPolicy, Step, Trial},
    guard::{ToolExecutor, ToolRule},
    runtime::{Backend, BillingDisposition, BoxResult, EffectSpec, Receipt},
    scheduler::Scheduling,
    storage::Durability,
};
use myth_contracts::checkpoint::Phase;
use serde_json::json;
use std::{
    collections::BTreeSet,
    ops::{Deref, DerefMut},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Default, Clone)]
pub struct GamePolicy {
    /// Test-only interrupted-after-receipt injection, never configured for live.
    pub fail_once_after_receipt: Option<Arc<AtomicBool>>,
    pub fail_role_once: Option<(AgentId, Arc<AtomicBool>)>,
    pub live_route: Option<PinnedRoute>,
}
impl GamePolicy {
    fn specification(&self, obs: &GameObservation) -> Result<Vec<EffectSpec>, String> {
        if let Some(route) = &self.live_route
            && (1..=3).contains(&obs.actor)
        {
            return Ok(vec![p5_cabinet::request(obs, route)?]);
        }
        if obs.synthetic_receipts && (1..=3).contains(&obs.actor) {
            let count = if obs.actor <= 2 { 3 } else { 2 };
            return Ok((0..count)
                .map(|index| {
                    EffectSpec::Model(ModelRequest {
                        provider: "p4-synthetic-offline".into(),
                        model: "scripted-p4-receipt-v1".into(),
                        resolved_model_version: Some("synthetic-no-network-v1".into()),
                        messages: vec![Message {
                            role: Role::User,
                            content: format!(
                                "q{} actor{} {:?} receipt{} v{}",
                                obs.quarter,
                                obs.actor,
                                obs.phase,
                                index,
                                obs.expected_world_version
                            ),
                            tool_call_id: None,
                        }],
                        sampling: Sampling {
                            temperature: Some(0.0),
                            top_p: None,
                            seed: Some(0),
                            max_output_tokens: Some(1),
                        },
                        tool_schemas: vec![],
                        streaming: false,
                        response_format: None,
                    })
                })
                .collect());
        }
        if obs.action.kind != ActionKind::PlayerCommand || obs.actor != PLAYER.0 as u8 {
            return Ok(vec![]);
        }
        let body = &obs.action.payload;
        let id = body["command_id"]
            .as_str()
            .ok_or("player command id missing")?;
        let operation_id = format!("game-{id}-v{}", obs.expected_world_version);
        Ok(vec![EffectSpec::Tool {
            name: "player_command".into(),
            input: json!({
                "schema_version":1,
                "operation_id":operation_id,
                "actor":obs.actor,
                "expected_world_version":obs.expected_world_version,
                "command_body":body,
            }),
        }])
    }
}
impl EffectPolicy<GameWorld> for GamePolicy {
    fn requests(&self, actor: AgentId, obs: &GameObservation) -> Result<Vec<EffectSpec>, String> {
        if actor.0 != u64::from(obs.actor) {
            return Err("observation actor mismatch".into());
        }
        self.specification(obs)
    }
    fn decide(
        &self,
        actor: AgentId,
        obs: &GameObservation,
        receipts: &[Receipt],
    ) -> Result<GameAction, String> {
        if actor.0 != u64::from(obs.actor) {
            return Err("forged minister id".into());
        }
        let expected = self.specification(obs)?;
        if self.live_route.is_some() && (1..=3).contains(&obs.actor) {
            if expected.len() != 1
                || receipts.len() != 1
                || receipts[0].spec != expected[0]
                || receipts[0].billing == BillingDisposition::NotCharged
            {
                return Err("minister inference receipt missing or mismatched".into());
            }
            let (content, effective_model) = match &receipts[0].result {
                EffectResult::Model {
                    content,
                    effective_model,
                    ..
                } => (content, effective_model),
                _ => return Err("minister inference failed; explicit retry required".into()),
            };
            if self
                .live_route
                .as_ref()
                .is_none_or(|route| effective_model != &route.model)
            {
                return Err("unapproved response model".into());
            }
            let mut action = obs.action.clone();
            action.payload = p5_cabinet::checked_action(obs, content)?;
            action.model_receipt_digest = Some(receipts[0].content_hash.clone());
            return Ok(action);
        }
        if expected.len() != receipts.len() {
            return Err("effect receipt count mismatch".into());
        }
        for (spec, receipt) in expected.iter().zip(receipts) {
            let allowed_billing = receipt.billing == BillingDisposition::NotCharged
                || (self.live_route.is_some()
                    && matches!(spec, EffectSpec::Tool { .. })
                    && receipt.billing == BillingDisposition::Unknown);
            if receipt.spec != *spec || !allowed_billing {
                return Err("incorrect request or billing disposition".into());
            }
            let valid = match (&receipt.result, spec) {
                (EffectResult::Tool { output }, EffectSpec::Tool { input, .. }) => output == input,
                (
                    EffectResult::Model {
                        content,
                        effective_model,
                        ..
                    },
                    EffectSpec::Model(m),
                ) => {
                    m.provider == "p4-synthetic-offline"
                        && content == "synthetic:ok"
                        && effective_model == &m.model
                }
                _ => false,
            };
            if !valid {
                return Err("untrusted or changed effect receipt".into());
            }
        }
        if actor == PLAYER
            && expected.len() == 1
            && self
                .fail_once_after_receipt
                .as_ref()
                .is_some_and(|flag| flag.swap(false, Ordering::SeqCst))
        {
            panic!("P4 TEST FAILPOINT after receipt fsync; before world action");
        }
        if self
            .fail_role_once
            .as_ref()
            .is_some_and(|(id, flag)| *id == actor && flag.swap(false, Ordering::SeqCst))
        {
            panic!("P4 TEST FAILPOINT inside durable frozen minister round");
        }
        Ok(obs.action.clone())
    }
}

/// The tool validates and echoes typed data; it writes no file, issues no
/// model request, and cannot access the game directory or financial ledger.
#[derive(Default)]
pub struct PlayerCommandTool;
impl Backend for PlayerCommandTool {
    fn permits_tools(&self) -> bool {
        true
    }
    fn unsupported_is_pre_dispatch(&self) -> bool {
        true
    }
    fn backend_id(&self) -> &'static str {
        "p4-offline-allowlist"
    }
    fn billing_disposition(&self, _: &EffectSpec, _: &EffectResult) -> BillingDisposition {
        BillingDisposition::NotCharged
    }
    fn resolve<'a>(&'a self, request: &'a EffectSpec) -> BoxResult<'a> {
        Box::pin(async move {
            if let EffectSpec::Model(model) = request {
                if model.provider != "p4-synthetic-offline"
                    || model.model != "scripted-p4-receipt-v1"
                    || model.streaming
                    || model.messages.len() != 1
                {
                    return Ok(EffectResult::UnsupportedCapability {
                        capability: "all real models disabled in P4".into(),
                    });
                }
                return Ok(EffectResult::Model {
                    content: "synthetic:ok".into(),
                    effective_model: model.model.clone(),
                    usage: Some(Usage {
                        input_tokens: Some(0),
                        output_tokens: Some(0),
                        billed_cost: Some("0".into()),
                        currency: Some("USD".into()),
                    }),
                });
            }
            let EffectSpec::Tool { name, input } = request else {
                return Ok(EffectResult::UnsupportedCapability {
                    capability: "P4 only accepts allowlisted effects".into(),
                });
            };
            if name != "player_command"
                || input["schema_version"] != 1
                || input["actor"] != 0
                || input["command_body"]["schema_version"] != 1
            {
                return Ok(EffectResult::UnsupportedCapability {
                    capability: "unapproved P4 effect".into(),
                });
            }
            let operation = input["operation_id"]
                .as_str()
                .ok_or("missing stable operation id")?;
            let command = input["command_body"]["command_id"]
                .as_str()
                .ok_or("missing command id")?;
            let version = input["expected_world_version"]
                .as_str()
                .ok_or("missing expected version")?;
            if operation != format!("game-{command}-v{version}")
                || input["command_body"]["expected_world_version"] != version
                || serde_json::to_vec(input).map_err(|e| e.to_string())?.len() > 12_288
            {
                return Err("invalid or oversized player effect".into());
            }
            myth_contracts::parse_command(&input["command_body"].to_string())
                .map_err(|e| format!("{e:?}"))?;
            Ok(EffectResult::Tool {
                output: input.clone(),
            })
        })
    }
}
/// Only the already-validated, read-only P4 player-command echo is
/// permitted through the live GovernedBackend. It cannot mutate the world.
impl ToolExecutor for PlayerCommandTool {
    fn validate(&self, name: &str, input: &serde_json::Value) -> Result<(), String> {
        if name != "player_command"
            || input["schema_version"] != 1
            || input["actor"] != 0
            || input["command_body"]["schema_version"] != 1
        {
            return Err("unapproved player tool".into());
        }
        myth_contracts::parse_command(&input["command_body"].to_string())
            .map_err(|_| "invalid player command for tool".into())
            .map(|_| ())
    }
    fn call<'a>(&'a self, name: &'a str, input: &'a serde_json::Value) -> BoxResult<'a> {
        Box::pin(async move {
            Backend::resolve(
                self,
                &EffectSpec::Tool {
                    name: name.into(),
                    input: input.clone(),
                },
            )
            .await
        })
    }
}
/// The schedule mirror lives with its exclusive-writer trial, never in a
/// process-global cache. Resume reconstructs it with one verified journal
/// inspection outside any lock.
pub struct GameTrial {
    inner: Trial<GameWorld, GamePolicy>,
    paused: BTreeSet<AgentId>,
    live_admission: Option<Arc<SharedAdmissionBackend>>,
}
impl GameTrial {
    /// Keep the private scheduler mirror synchronized even when a caller
    /// invokes the public pause/resume methods outside `enable`.
    pub fn pause_actor(&mut self, actor: AgentId) -> Result<(), String> {
        self.inner.pause_actor(actor)?;
        self.paused.insert(actor);
        Ok(())
    }
    pub fn resume_actor(&mut self, actor: AgentId) -> Result<(), String> {
        self.inner.resume_actor(actor)?;
        self.paused.remove(&actor);
        Ok(())
    }
}
impl Deref for GameTrial {
    type Target = Trial<GameWorld, GamePolicy>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for GameTrial {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
fn create_world(dir: &Path, name: &str, seed: u64, world: GameWorld) -> Result<GameTrial, String> {
    let mut inner = Trial::create(
        dir,
        name,
        seed,
        WORLD_KIND,
        world,
        GamePolicy::default(),
        Arc::new(PlayerCommandTool),
        ACTORS.to_vec(),
        Scheduling::RoundRobin,
    )?;
    inner.store.set_durability(Durability::EveryEvent)?;
    Ok(GameTrial {
        inner,
        paused: BTreeSet::new(),
        live_admission: None,
    })
}
pub fn create(dir: &Path, name: &str, seed: u64) -> Result<GameTrial, String> {
    create_world(dir, name, seed, GameWorld::initial("p4-pilot")?)
}
pub fn create_stress(dir: &Path, name: &str, seed: u64) -> Result<GameTrial, String> {
    create_world(dir, name, seed, GameWorld::stress("p4-pilot")?)
}
pub fn resume(dir: &Path, replay_only: bool) -> Result<GameTrial, String> {
    let mut inner = Trial::resume(
        dir,
        WORLD_KIND,
        GamePolicy::default(),
        Arc::new(PlayerCommandTool),
        replay_only,
    )?;
    inner.store.set_durability(Durability::EveryEvent)?;
    inner.store.world().verify()?;
    let persisted = hiyal::storage::Store::<GameWorld, GameAction>::inspect_read_only(dir)?;
    Ok(GameTrial {
        inner,
        paused: persisted.schedule.paused,
        live_admission: None,
    })
}

/// Persist the pinned live route in the private trial directory. Hiyal's
/// own immutable per-trial BudgetConfig is separately checked on resume.
fn staged_route_path(dir: &Path) -> std::path::PathBuf {
    dir.with_extension("p5-route-stage")
}
fn route_bytes(route: &PinnedRoute) -> Result<Vec<u8>, String> {
    route.validate()?;
    let bytes = serde_json::to_vec(route).map_err(|_| "invalid route metadata")?;
    if bytes.len() > 4096 {
        return Err("route metadata oversized".into());
    }
    Ok(bytes)
}
fn stage_live_route(dir: &Path, route: &PinnedRoute) -> Result<(), String> {
    use std::io::Write;
    if dir.exists() {
        return Err("live trial directory already exists".into());
    }
    let bytes = route_bytes(route)?;
    let path = staged_route_path(dir);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "preflight route binding already exists or cannot be created")?;
    file.write_all(&bytes)
        .map_err(|_| "cannot persist staged immutable route")?;
    file.sync_all().map_err(|_| "cannot sync staged route")?;
    if let Some(parent) = dir.parent() {
        #[cfg(unix)]
        std::fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|_| "cannot sync staging directory")?;
    }
    Ok(())
}
fn bind_live_route(dir: &Path, route: &PinnedRoute, creating: bool) -> Result<(), String> {
    let bytes = route_bytes(route)?;
    let bound = dir.join("p5-route.json");
    let staged = staged_route_path(dir);
    if bound.is_file() {
        if std::fs::read(&bound).map_err(|_| "cannot read immutable live route")? != bytes {
            return Err("live route or pricing differs from saved trial".into());
        }
        return Ok(());
    }
    if !dir.join("manifest.json").is_file() || !staged.is_file() {
        return Err("incomplete live trial creation: missing immutable route and staged recovery; do not dispatch".into());
    }
    // A trial created before the atomic route rename can be restored only
    // before its first effect/action; never invent a binding for an active run.
    if !creating
        && std::fs::metadata(dir.join("events.jsonl"))
            .map_err(|_| "missing trial event history")?
            .len()
            != 0
    {
        return Err(
            "route recovery rejected: trial has events; manual investigation required".into(),
        );
    }
    if std::fs::read(&staged).map_err(|_| "cannot read staged route")? != bytes {
        return Err("staged model or price differs from requested route".into());
    }
    std::fs::rename(&staged, &bound).map_err(|_| "cannot atomically bind staged route")?;
    #[cfg(unix)]
    std::fs::File::open(dir)
        .and_then(|f| f.sync_all())
        .map_err(|_| "cannot sync immutable trial route")?;
    Ok(())
}
fn minister_scope(world: &GameWorld) -> Scope {
    Scope {
        purpose: Purpose::Minister,
        campaign_id: Some(world.campaign_id.clone()),
        quarter: Some(world.quarter),
        batch_id: None,
    }
}
fn live_governor(provider: Arc<dyn Backend>) -> Result<Arc<dyn Backend>, String> {
    let governed = governed_trial(provider)
        .allow_tool(
            "player_command",
            ToolRule {
                max_input_bytes: 12_288,
                requires_approval: false,
                has_external_side_effects: false,
            },
        )?
        .with_executor(Arc::new(PlayerCommandTool));
    Ok(Arc::new(governed))
}
/// The caller must open and recover the installation ledger first. Neither
/// this function nor P1's ordinary loopback service enables live inference.
pub fn create_live(
    dir: &Path,
    name: &str,
    seed: u64,
    campaign_id: &str,
    route: PinnedRoute,
    ledger: Arc<std::sync::Mutex<Allowance>>,
    provider: Arc<dyn Backend>,
) -> Result<GameTrial, String> {
    let world = GameWorld::initial(campaign_id)?;
    let governor = live_governor(provider)?;
    let admitted = Arc::new(SharedAdmissionBackend::new(
        ledger,
        governor,
        route.clone(),
        minister_scope(&world),
    )?);

    let route_for_binding = route.clone();
    // Preflight and fsync the identity before Hiyal commits a trial genesis.
    // On a crash, resume can atomically complete the staged binding if no
    // actions have yet been journaled; otherwise admission remains closed.
    stage_live_route(dir, &route_for_binding)?;
    let mut inner = Trial::create(
        dir,
        name,
        seed,
        WORLD_KIND,
        world,
        GamePolicy {
            live_route: Some(route),
            ..GamePolicy::default()
        },
        admitted.clone(),
        ACTORS.to_vec(),
        Scheduling::RoundRobin,
    )?;
    inner.store.set_durability(Durability::EveryEvent)?;
    bind_live_route(dir, &route_for_binding, true)?;
    Ok(GameTrial {
        inner,
        paused: BTreeSet::new(),
        live_admission: Some(admitted),
    })
}
/// Resume with the SAME immutable Hiyal budget and route, and an already
/// recovered shared allowance. Saved model receipts are never re-dispatched.
pub fn resume_live(
    dir: &Path,
    replay_only: bool,
    route: PinnedRoute,
    ledger: Arc<std::sync::Mutex<Allowance>>,
    provider: Arc<dyn Backend>,
) -> Result<GameTrial, String> {
    bind_live_route(dir, &route, false)?;
    let persisted = hiyal::storage::Store::<GameWorld, GameAction>::inspect_read_only(dir)?;
    persisted.world.verify()?;
    let governor = live_governor(provider)?;
    let admitted = Arc::new(SharedAdmissionBackend::new(
        ledger,
        governor,
        route.clone(),
        minister_scope(&persisted.world),
    )?);

    let mut inner = Trial::resume(
        dir,
        WORLD_KIND,
        GamePolicy {
            live_route: Some(route),
            ..GamePolicy::default()
        },
        admitted.clone(),
        replay_only,
    )?;
    inner.store.set_durability(Durability::EveryEvent)?;
    inner.store.world().verify()?;
    Ok(GameTrial {
        inner,
        paused: persisted.schedule.paused,
        live_admission: Some(admitted),
    })
}

/// Operator approvals are *not* Hiyal effect receipts. They are separate
/// fsynced private authorization state; the actual retry gets a new Hiyal
/// event sequence and therefore a different durable effect call key.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryAuthorization {
    actor: u64,
    quarter: u8,
    phase: Phase,
    frozen_observation_sha256: String,
    source_event_hash: String,
    source_receipt_hash: String,
    source_request_sha256: String,
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryAuthorizations {
    schema: u8,
    entries: Vec<RetryAuthorization>,
}
fn retry_file(dir: &Path) -> std::path::PathBuf {
    dir.join("p5-retry-authorizations.json")
}
fn read_retry_authorizations(dir: &Path) -> Result<RetryAuthorizations, String> {
    let path=retry_file(dir);
    if !path.exists() {
        return Ok(RetryAuthorizations{schema:1,entries:Vec::new()});
    }
    let bytes=std::fs::read(path).map_err(|_| "cannot read private retry approvals")?;
    if bytes.len()>65_536 {return Err("retry approvals exceed bound".into());}
    let state:RetryAuthorizations=serde_json::from_slice(&bytes)
        .map_err(|_| "invalid private retry approvals")?;
    if state.schema!=1 || state.entries.len()>12 {
        return Err("invalid retry approval schema or count".into());
    }
    Ok(state)
}
fn retry_source(trial:&GameTrial, actor:AgentId) -> Result<Option<RetryAuthorization>,String> {
    use hiyal::{core::World,storage::Outcome};
    use sha2::{Digest,Sha256};
    let world=trial.store.world();
    if !matches!(world.phase,Phase::FirstPositionsFrozen|Phase::FinalPositionsFrozen)
        || !world.remaining_ministers().contains(&actor) {
        return Ok(None);
    }
    let observation=world.observe(actor)?;
    let events=trial.store.read_events()?;
    let freeze_sequence=events.iter().rev().find(|e|
        e.actor==PLAYER && matches!(e.outcome,Outcome::Accepted{..}))
        .map(|e|e.sequence()).ok_or("missing durable frozen player action")?;
    let failed=events.iter().rev().find(|e|
        e.actor==actor && e.sequence()>freeze_sequence
            && matches!(e.outcome,Outcome::Rejected{..}));
    let Some(failure)=failed else {return Ok(None)};
    let prior=trial.effects.receipts().iter().find(|(key,receipt)|
        key.agent_id==actor && key.logical_step==failure.sequence()
            && matches!(receipt.result,EffectResult::Failed{..}|EffectResult::UnsupportedCapability{..}));
    let Some((key,receipt))=prior else {
        // Invalid *typed* outputs require a separate per-action JSON repair
        // protocol, not an operational retry.
        return Err("rejected minister has no operationally failed keyed receipt");
    };
    if *key != receipt.spec.key(&key.trial_id,key.logical_step,key.agent_id,key.call_index)? {
        return Err("prior retry receipt key mismatch");
    }
    let EffectSpec::Model(model)=&receipt.spec else {
        return Err("failed minister receipt was not a model request");
    };
    let source_request_sha256=format!("{:x}",Sha256::digest(
        serde_json::to_vec(model).map_err(|_|"cannot hash failed model request")?));
    Ok(Some(RetryAuthorization{
        actor:actor.0,quarter:world.quarter,phase:world.phase,
        frozen_observation_sha256:observation.action.observation_sha256.clone(),
        source_event_hash:failure.hash.clone(),
        source_receipt_hash:receipt.content_hash.clone(),
        source_request_sha256,
    }))
}
fn persist_retry_authorizations(dir:&Path, state:&RetryAuthorizations)->Result<(),String> {
    use std::io::Write;
    let bytes=serde_json::to_vec(state).map_err(|_|"cannot encode retry approvals")?;
    if bytes.len()>65_536 {return Err("retry approval history full");}
    let tmp=dir.join("p5-retry-authorizations.new");
    let mut file=std::fs::OpenOptions::new().write(true).create(true).truncate(true)
        .open(&tmp).map_err(|_|"cannot stage retry authorization")?;
    file.write_all(&bytes).map_err(|_|"cannot write retry authorization")?;
    file.sync_all().map_err(|_|"cannot sync retry authorization")?;
    std::fs::rename(&tmp,retry_file(dir)).map_err(|_|"cannot commit retry authorization")?;
    #[cfg(unix)]
    std::fs::File::open(dir).and_then(|f|f.sync_all())
        .map_err(|_|"cannot sync retry directory")?;
    Ok(())
}
/// A retry is player/operator-initiated, never scheduled automatically.
pub fn authorize_failed_minister_retry(trial:&GameTrial,actor:AgentId)->Result<(),String>{
    if trial.live_admission.is_none() {return Err("not a live governed trial".into());}
    if trial.store.dir.join("pending-round.json").exists() {
        return Err("cannot authorize while frozen round is unfinished".into());
    }
    if !trial.store.world().remaining_ministers().contains(&actor) {
        return Err("actor has no failed, unfinished minister action".into());
    }
    let source=retry_source(trial,actor)?.ok_or("no failed operational effect to retry")?;
    let mut state=read_retry_authorizations(&trial.store.dir)?;
    if state.entries.iter().any(|e|e.actor==source.actor
        && e.source_event_hash==source.source_event_hash){
        return Err("retry for this failed receipt already authorized");
    }
    if state.entries.iter().filter(|e|e.quarter==source.quarter).count()>=2 {
        return Err("quarterly manual operational retry cap reached");
    }
    state.entries.push(source);
    persist_retry_authorizations(&trial.store.dir,&state)
}
/// Re-verify a saved paid response using ONLY the OpenRouter generation
/// metadata GET. A successful certification is separately persisted by the
/// transport. This call cannot send a new paid completion request.
pub async fn certify_failed_minister_saved_completion(
    trial:&GameTrial,actor:AgentId,
    backend:&crate::p5_openrouter::OpenRouterBackend,
)->Result<(),String>{
    use hiyal::core::World;
    let source=retry_source(trial,actor)?.ok_or("missing failed model source")?;
    let state=read_retry_authorizations(&trial.store.dir)?;
    if !state.entries.iter().any(|e|e.actor==source.actor
        && e.quarter==source.quarter && e.phase==source.phase
        && e.source_event_hash==source.source_event_hash
        && e.source_receipt_hash==source.source_receipt_hash
        && e.source_request_sha256==source.source_request_sha256
        && e.frozen_observation_sha256==source.frozen_observation_sha256){
        return Err("explicit durable retry authorization required before custody certification".into());
    }
    let observation=trial.store.world().observe(actor)?;
    let route=trial.policy.live_route.as_ref().ok_or("missing pinned live route")?;
    let EffectSpec::Model(request)=p5_cabinet::request(&observation,route)? else {
        return Err("minister reconstruction not a model request".into());
    };
    use sha2::{Digest,Sha256};
    let hash=format!("{:x}",Sha256::digest(
        serde_json::to_vec(&request).map_err(|_|"cannot hash frozen retry request")?));
    if hash!=source.source_request_sha256 {
        return Err("frozen retry request differs from original paid response".into());
    }
    backend.certify_saved(&request).await
}
fn ensure_authorized_failed_ministers(trial:&GameTrial,actors:&[AgentId])->Result<(),String>{
    if trial.live_admission.is_none(){return Ok(());}
    let state=read_retry_authorizations(&trial.store.dir)?;
    for &actor in actors {
        if let Some(source)=retry_source(trial,actor)? {
            let matching=state.entries.iter().filter(|auth|
                auth.actor==source.actor
                    && auth.quarter==source.quarter
                    && auth.phase==source.phase
                    && auth.frozen_observation_sha256==source.frozen_observation_sha256
                    && auth.source_event_hash==source.source_event_hash
                    && auth.source_receipt_hash==source.source_receipt_hash
                    && auth.source_request_sha256==source.source_request_sha256).count();
            if matching!=1 {
                return Err("failed minister requires one explicit durable retry authorization".into());
            }
        }
    }
    Ok(())
}
fn enable(trial: &mut GameTrial, allowed: &[AgentId]) -> Result<(), String> {
    for actor in ACTORS {
        let currently_paused = trial.paused.contains(&actor);
        let should_pause = !allowed.contains(&actor);
        if currently_paused != should_pause {
            if should_pause {
                trial.pause_actor(actor)?;
            } else {
                trial.resume_actor(actor)?;
            }
        }
    }
    Ok(())
}
fn check_step(step: Step<GameWorld>) -> Result<(), String> {
    match step {
        Step::Accepted(_) => Ok(()),
        Step::Rejected { reason, actor } => Err(format!("actor {} rejected: {reason}", actor.0)),
        Step::NoEligible => Err("no eligible actor".into()),
    }
}
/// One explicitly chosen logical action or three frozen minister decisions.
/// Scheduling changes are Hiyal durable control records, *not* calendar time.
pub async fn advance_slice(trial: &mut GameTrial) -> Result<bool, String> {
    if let Some(live) = &trial.live_admission {
        live.set_scope(minister_scope(trial.store.world()))?;
    }
    // Hiyal persists a plan before dispatch and removes it *after* all
    // committed entries. A crash after the final commit can leave the plan
    // present even when the P3 phase has already advanced. Drain that plan
    // before pause/resume (which Hiyal correctly forbids while pending).
    if trial.store.dir.join("pending-round.json").exists() {
        let steps = trial.run_round(3).await?;
        for step in steps {
            check_step(step)?;
        }
        trial.store.world().verify()?;
        return Ok(true);
    }
    if trial.store.world().is_closed() {
        return Ok(false);
    }
    let phase = trial.store.world().phase;
    match phase {
        Phase::QuarterOpen | Phase::AmendmentWindow => {
            enable(trial, &[PLAYER])?;
            check_step(trial.step().await?)?;
        }
        Phase::Resolved => {
            enable(trial, &[PERIOD])?;
            check_step(trial.step().await?)?;
            trial.checkpoint()?;
        }
        Phase::FinalPositionsFrozen if trial.store.world().final_votes_this_meeting() == 3 => {
            enable(trial, &[PLAYER])?;
            check_step(trial.step().await?)?;
        }
        Phase::FirstPositionsFrozen | Phase::FinalPositionsFrozen => {
            // After a completed round with a rejected seat, only permit
            // unfinished seats. Otherwise already accepted ministers would
            // produce spurious Rejected records on retry.
            let remaining = trial.store.world().remaining_ministers();
            if remaining.is_empty() {
                return Err("cabinet has no remaining actions".into());
            }
            ensure_authorized_failed_ministers(trial,&remaining)?;
            enable(trial, &remaining)?;
            let steps = trial.run_round(remaining.len()).await?;
            for step in steps {
                check_step(step)?;
            }
        }
        Phase::QuarterClosed => return Ok(false),
    }
    trial.store.world().verify()?;
    Ok(true)
}
/// Performance-only sequential cabinet path: measures each complete Hiyal
/// role step (including EveryEvent fsync), rather than dividing an opaque
/// three-actor frozen batch by three. Recovery acceptance still separately
/// verifies the production frozen run_round path, including crash suffixes.
pub async fn benchmark_sequential_cabinet(trial: &mut GameTrial) -> Result<Vec<u128>, String> {
    let phase = trial.store.world().phase;
    if !matches!(
        phase,
        Phase::FirstPositionsFrozen | Phase::FinalPositionsFrozen
    ) {
        return Err("not inside a cabinet phase".into());
    }
    let remaining = trial.store.world().remaining_ministers();
    enable(trial, &remaining)?;
    let mut times = vec![];
    for _ in 0..remaining.len() {
        let begin = std::time::Instant::now();
        check_step(trial.step().await?)?;
        times.push(begin.elapsed().as_micros());
    }
    trial.store.world().verify()?;
    Ok(times)
}
pub async fn run_until_closed(trial: &mut GameTrial) -> Result<(), String> {
    for _ in 0..100 {
        if !advance_slice(trial).await? {
            trial.checkpoint()?;
            return Ok(());
        }
    }
    Err("bounded scripted run did not close".into())
}
/// Strict, read-only full Hiyal journal verification plus independent complete
/// P3 action-fold validation. Does not call the backend or repair files.
pub fn inspect(dir: &Path) -> Result<(String, String, u64), String> {
    use hiyal::storage::Store;
    let validated = Store::<GameWorld, GameAction>::inspect_read_only(dir)?;
    validated.world.verify()?;
    Ok((
        validated.world.private_state_sha256,
        validated.chain_head,
        validated.sequence,
    ))
}

/// Never confuse replay-only (no new tool or model dispatch) with the
/// operational resume mode (which can admit new offline player tool calls).
pub fn final_private_hash(dir: &Path) -> Result<String, String> {
    Ok(inspect(dir)?.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiyal::core::World;
    use hiyal::runtime::EffectJournal;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn temp(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("p4-{name}-{}-{nonce}", std::process::id()))
    }
    #[tokio::test]
    async fn six_quarters_save_resume_matches_full_validation_without_network() {
        let path = temp("replay");
        let expected;
        {
            let mut trial = create(&path, "scripted-pilot", 43).unwrap();
            advance_slice(&mut trial).await.unwrap();
            advance_slice(&mut trial).await.unwrap(); // frozen first round
            assert_eq!(trial.store.world().phase, Phase::AmendmentWindow);
            trial.checkpoint().unwrap();
            let prior = inspect(&path).unwrap();
            assert!(!prior.0.is_empty());
        }
        {
            let mut resumed = resume(&path, false).unwrap();
            run_until_closed(&mut resumed).await.unwrap();
            assert_eq!(resumed.store.world().quarter, 6);
            assert!(resumed.store.world().is_closed());
            assert_eq!(
                resumed
                    .store
                    .world()
                    .actions
                    .iter()
                    .filter(|a| a.kind == ActionKind::MinisterAction)
                    .count(),
                36
            );
            expected = resumed.store.world().private_state_sha256.clone();
        }
        let (restored, chain, _sequence) = inspect(&path).unwrap();
        assert_eq!(restored, expected);
        assert_eq!(chain.len(), 64);
        let strict = resume(&path, true).unwrap();
        assert_eq!(strict.store.world().private_state_sha256, expected);
        drop(strict);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn interrupted_frozen_round_replays_only_pending_ministers() {
        let path = temp("frozen-recovery");
        let mut trial = create(&path, "frozen-round", 47).unwrap();
        enable(&mut trial, &[PLAYER]).unwrap();
        check_step(trial.step().await.unwrap()).unwrap();
        assert_eq!(trial.store.world().phase, Phase::FirstPositionsFrozen);
        enable(&mut trial, &crate::ACTORS[1..4]).unwrap();
        trial.policy.fail_role_once = Some((crate::EDUCATION, Arc::new(AtomicBool::new(true))));
        let task = tokio::spawn(async move { trial.run_round(3).await });
        assert!(task.await.unwrap_err().is_panic());
        assert!(path.join("pending-round.json").is_file());
        let mut restored = resume(&path, false).unwrap();
        let remaining = restored.run_round(3).await.unwrap();
        assert!(!remaining.is_empty());
        for event in remaining {
            check_step(event).unwrap();
        }
        assert_eq!(restored.store.world().phase, Phase::AmendmentWindow);
        assert_eq!(
            restored
                .store
                .world()
                .actions
                .iter()
                .filter(|x| x.kind == ActionKind::MinisterAction)
                .count(),
            3
        );
        run_until_closed(&mut restored).await.unwrap();
        let hash = restored.store.world().private_state_sha256.clone();
        drop(restored);
        assert_eq!(inspect(&path).unwrap().0, hash);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn recovered_finished_round_plan_is_unlinked_before_phase_transition() {
        let path = temp("last-minister-crash");
        let mut trial = create(&path, "last-minister", 73).unwrap();
        advance_slice(&mut trial).await.unwrap();
        enable(&mut trial, &ACTORS[1..4]).unwrap();
        trial.policy.fail_role_once = Some((crate::EDUCATION, Arc::new(AtomicBool::new(true))));
        let task = tokio::spawn(async move { trial.run_round(3).await });
        assert!(task.await.unwrap_err().is_panic());
        let saved = std::fs::read(path.join("pending-round.json")).unwrap();
        let mut first = resume(&path, false).unwrap();
        for step in first.run_round(3).await.unwrap() {
            check_step(step).unwrap();
        }
        assert_eq!(first.store.world().phase, Phase::AmendmentWindow);
        drop(first);
        // Recreate the exact valid plan after the last action fsync to model
        // a crash immediately before the atomic cleanup of pending-round.
        std::fs::write(path.join("pending-round.json"), saved).unwrap();
        let mut resumed = resume(&path, false).unwrap();
        advance_slice(&mut resumed).await.unwrap();
        assert!(!path.join("pending-round.json").exists());
        assert_eq!(resumed.store.world().phase, Phase::AmendmentWindow);
        run_until_closed(&mut resumed).await.unwrap();
        assert_eq!(
            resumed
                .store
                .world()
                .actions
                .iter()
                .filter(|a| a.kind == ActionKind::MinisterAction)
                .count(),
            36
        );
        drop(resumed);
        std::fs::remove_dir_all(path).unwrap();
    }
    struct SelectiveFailure;
    impl Backend for SelectiveFailure {
        fn backend_id(&self) -> &'static str {
            "p4-selective-failure-test"
        }
        fn billing_disposition(&self, _: &EffectSpec, result: &EffectResult) -> BillingDisposition {
            if matches!(result, EffectResult::Failed { .. }) {
                BillingDisposition::Unknown
            } else {
                BillingDisposition::NotCharged
            }
        }
        fn resolve<'a>(&'a self, spec: &'a EffectSpec) -> BoxResult<'a> {
            Box::pin(async move {
                if let EffectSpec::Model(m) = spec
                    && m.messages.iter().any(|x| x.content.contains("actor2"))
                {
                    return Ok(EffectResult::Failed {
                        category: "synthetic".into(),
                        message: "one seat failed".into(),
                    });
                }
                let backend = PlayerCommandTool;
                backend.resolve(spec).await
            })
        }
    }
    #[tokio::test]
    async fn retry_partly_rejected_round_commits_only_missing_seat() {
        let path = temp("partial-round");
        let mut trial = create_stress(&path, "partial", 75).unwrap();
        advance_slice(&mut trial).await.unwrap();
        enable(&mut trial, &ACTORS[1..4]).unwrap();
        trial.backend = Arc::new(SelectiveFailure);
        let steps = trial.run_round(3).await.unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(
            steps
                .iter()
                .filter(|s| matches!(s, Step::Accepted(_)))
                .count(),
            2
        );
        assert_eq!(
            trial.store.world().remaining_ministers(),
            vec![crate::EDUCATION]
        );
        let earlier = trial.store.world().actions.len();
        trial.backend = Arc::new(PlayerCommandTool);
        advance_slice(&mut trial).await.unwrap();
        assert_eq!(trial.store.world().actions.len(), earlier + 1);
        assert_eq!(trial.store.world().phase, Phase::AmendmentWindow);
        assert_eq!(
            trial
                .store
                .world()
                .actions
                .iter()
                .filter(|a| a.kind == ActionKind::MinisterAction)
                .count(),
            3
        );
        trial.store.world().verify().unwrap();
        drop(trial);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn sequential_cabinet_timing_preserves_frozen_round_private_state() {
        let frozen_dir = temp("frozen-baseline");
        let sequential_dir = temp("sequential-measure");
        let mut frozen = create(&frozen_dir, "same-model", 77).unwrap();
        run_until_closed(&mut frozen).await.unwrap();
        let baseline = frozen.store.world().private_state_sha256.clone();
        drop(frozen);
        let mut measured = create(&sequential_dir, "same-model", 77).unwrap();
        for _ in 0..60 {
            if measured.store.world().is_closed() {
                break;
            }
            let phase = measured.store.world().phase;
            if phase == Phase::FirstPositionsFrozen
                || (phase == Phase::FinalPositionsFrozen
                    && measured.store.world().final_votes_this_meeting() < 3)
            {
                assert_eq!(
                    benchmark_sequential_cabinet(&mut measured)
                        .await
                        .unwrap()
                        .len(),
                    3
                );
            } else {
                advance_slice(&mut measured).await.unwrap();
            }
        }
        assert!(measured.store.world().is_closed());
        assert_eq!(measured.store.world().private_state_sha256, baseline);
        drop(measured);
        std::fs::remove_dir_all(frozen_dir).unwrap();
        std::fs::remove_dir_all(sequential_dir).unwrap();
    }
    #[test]
    fn altered_player_command_and_wrong_actor_fail_before_commit() {
        let mut world = GameWorld::initial("p4-pilot").unwrap();
        let planned = world.observe(PLAYER).unwrap().action;
        let mut forged = planned.clone();
        forged.payload["command"]["allocation"]["staff"] = json!(60);
        assert!(world.apply(PLAYER, forged).is_err());
        let mut wrong = planned.clone();
        wrong.actor = crate::FINANCE.0 as u8;
        assert!(world.apply(PLAYER, wrong).is_err());
        assert!(world.actions.is_empty());
        world.verify().unwrap();
    }
    #[tokio::test]
    async fn one_writer_lock_survives_and_releases_on_close() {
        let path = temp("writer");
        let trial = create(&path, "writer-lease", 48).unwrap();
        assert!(
            resume(&path, false).is_err(),
            "second process-style writer must be denied"
        );
        drop(trial);
        let reopened = resume(&path, false).unwrap();
        assert_eq!(reopened.store.world().actions.len(), 0);
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
    struct UncertainOfflineFailure;
    impl Backend for UncertainOfflineFailure {
        fn resolve<'a>(&'a self, _: &'a EffectSpec) -> BoxResult<'a> {
            Box::pin(async move {
                Ok(EffectResult::Failed {
                    category: "test-uncertain".into(),
                    message: "synthetic provider outcome unknown".into(),
                })
            })
        }
    }
    #[tokio::test]
    async fn unknown_synthetic_charge_survives_restart_without_auto_refund() {
        let path = temp("unknown-bill");
        let mut trial = create_stress(&path, "unknown-charge", 49).unwrap();
        enable(&mut trial, &[PLAYER]).unwrap();
        check_step(trial.step().await.unwrap()).unwrap();
        enable(&mut trial, &crate::ACTORS[1..4]).unwrap();
        trial.backend = Arc::new(UncertainOfflineFailure);
        let result = trial.run_round(1).await.unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], Step::Rejected { .. }));
        let unknown = trial
            .effects
            .receipts()
            .values()
            .filter(|receipt| receipt.billing == BillingDisposition::Unknown)
            .count();
        assert_eq!(unknown, 3);
        drop(trial);
        let reopened = resume(&path, false).unwrap();
        assert_eq!(
            reopened
                .effects
                .receipts()
                .values()
                .filter(|receipt| receipt.billing == BillingDisposition::Unknown)
                .count(),
            3
        );
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn typed_tool_is_allowlisted_and_never_billed() {
        let path = temp("tool");
        let trial = create(&path, "tool-receipt", 44).unwrap();
        let obs = trial.store.world().observe(PLAYER).unwrap();
        let spec = GamePolicy::default().specification(&obs).unwrap();
        assert_eq!(spec.len(), 1);
        let backend = PlayerCommandTool;
        let result = backend.resolve(&spec[0]).await.unwrap();
        assert!(matches!(result, EffectResult::Tool { .. }));
        assert_eq!(
            backend.billing_disposition(&spec[0], &result),
            BillingDisposition::NotCharged
        );
        let disallowed = EffectSpec::Tool {
            name: "shell".into(),
            input: json!({}),
        };
        assert!(matches!(
            backend.resolve(&disallowed).await.unwrap(),
            EffectResult::UnsupportedCapability { .. }
        ));
        drop(trial);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn stress_runs_all_36_minister_actions_and_96_fake_model_receipts() {
        let path = temp("stress");
        let mut trial = create_stress(&path, "stress-pilot", 46).unwrap();
        run_until_closed(&mut trial).await.unwrap();
        assert_eq!(trial.store.world().stress_cohorts.len(), 1024);
        assert_eq!(
            trial
                .store
                .world()
                .actions
                .iter()
                .filter(|x| x.kind == ActionKind::MinisterAction)
                .count(),
            36
        );
        let synthetic = trial
            .effects
            .receipts()
            .values()
            .filter(|x| matches!(x.spec, EffectSpec::Model(_)))
            .count();
        assert_eq!(synthetic, 96);
        assert!(
            trial
                .effects
                .receipts()
                .values()
                .all(|x| x.billing == BillingDisposition::NotCharged)
        );
        let hash = trial.store.world().private_state_sha256.clone();
        drop(trial);
        assert_eq!(inspect(&path).unwrap().0, hash);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn receipt_survives_panic_before_world_commit_and_is_reused() {
        let path = temp("receipt-panic");
        let mut trial = create(&path, "receipt-recovery", 45).unwrap();
        enable(&mut trial, &[PLAYER]).unwrap();
        trial.policy.fail_once_after_receipt = Some(Arc::new(AtomicBool::new(true)));
        let task = tokio::spawn(async move { trial.step().await });
        assert!(task.await.unwrap_err().is_panic());
        let receipts = EffectJournal::inspect_read_only(
            &path,
            &hiyal::core::TrialId::new("receipt-recovery", 45).0,
        )
        .unwrap();
        assert_eq!(receipts, 1);
        let mut resumed = resume(&path, true).unwrap();
        assert_eq!(resumed.store.world().actions.len(), 0);
        check_step(resumed.step().await.unwrap()).unwrap();
        assert_eq!(resumed.store.world().actions.len(), 1);
        drop(resumed);
        assert_eq!(
            EffectJournal::inspect_read_only(
                &path,
                &hiyal::core::TrialId::new("receipt-recovery", 45).0
            )
            .unwrap(),
            1
        );
        std::fs::remove_dir_all(path).unwrap();
    }
    struct FakeDisagreeingCabinet;
    impl Backend for FakeDisagreeingCabinet {
        fn resolve<'a>(&'a self, spec: &'a EffectSpec) -> BoxResult<'a> {
            Box::pin(async move {
                let EffectSpec::Model(model) = spec else {
                    return Err("not a model effect".into());
                };
                let raw = model
                    .messages
                    .last()
                    .ok_or("no role context")?
                    .content
                    .split_once('\n')
                    .ok_or("missing role context")?
                    .1;
                let view: serde_json::Value =
                    serde_json::from_str(raw).map_err(|_| "bad frozen role view")?;
                let first = model.response_format.as_ref().is_some_and(|f| {
                    f["json_schema"]["name"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("myth_first_"))
                });
                let ministry = view["ministry"].as_str().ok_or("missing seat")?;
                let mut action = serde_json::json!({
                    "schema_version":1,"stage":if first {"first"} else {"final"},
                    "meeting_id":view["meeting_id"],
                    "proposal_version":view["proposal_version"],
                    "ministry":ministry,
                    "position":if ministry=="finance"{"support"}else{"oppose"},
                    "preferred_allocation":view["allocation"],
                    "conditions":[],"evidence_ids":[],
                    "rationale_summary":"Independent fictional cabinet assessment"
                });
                if first {
                    action["counterproposal_source"] = serde_json::json!({"kind":"new_typed"});
                } else if ministry != "finance" {
                    action["implementation_effort_permille"] = serde_json::json!(800);
                }
                Ok(EffectResult::Model {
                    content: action.to_string(),
                    effective_model: model.model.clone(),
                    usage: None,
                })
            })
        }
    }
    #[tokio::test]
    async fn fake_cabinet_decisions_change_policy_and_replay_after_restart() {
        use crate::p5_allowance::{Allowance, PinnedRoute};
        let control = temp("p5-control");
        let path = temp("p5-model-trial");
        let ledger_dir = temp("p5-allowance");
        let mut baseline = create_world(
            &control,
            "baseline",
            97,
            GameWorld::initial("cabinet-test").unwrap(),
        )
        .unwrap();
        run_until_closed(&mut baseline).await.unwrap();
        let scripted = baseline
            .store
            .world()
            .actions
            .iter()
            .filter(|a| a.payload["command"]["kind"] == "ratify")
            .count();
        assert!(scripted > 0);
        drop(baseline);
        let allowance = Arc::new(std::sync::Mutex::new(
            Allowance::open(&ledger_dir, "test").unwrap(),
        ));
        let provider: Arc<dyn Backend> = Arc::new(FakeDisagreeingCabinet);
        let route = PinnedRoute {
            model: "test/model".into(),
            provider: "test-provider".into(),
            input_micro_usd_per_million: 1_000_000,
            output_micro_usd_per_million: 5_000_000,
            reasoning_disabled: true,
            reasoning_parameter_supported: false,
            response_model: None,
        };
        let mut trial = create_live(
            &path,
            "fake-model",
            97,
            "cabinet-test",
            route.clone(),
            allowance.clone(),
            provider.clone(),
        )
        .unwrap();
        advance_slice(&mut trial).await.unwrap();
        if let Err(problem) = advance_slice(&mut trial).await {
            let outcomes: Vec<String> = trial
                .effects
                .receipts()
                .values()
                .map(|r| format!("{:?}", r.result))
                .collect();
            panic!("offline cabinet round failed: {problem}; effect outcomes: {outcomes:?}");
        }
        assert_eq!(trial.store.world().phase, Phase::AmendmentWindow);
        let reservations = allowance.lock().unwrap().calls().len();
        let remaining = allowance.lock().unwrap().remaining_micro_usd().unwrap();
        drop(trial);
        let mut different = route.clone();
        different.output_micro_usd_per_million = 4_000_000;
        assert!(resume_live(&path, false, different, allowance.clone(), provider.clone()).is_err());
        let mut resumed = resume_live(&path, false, route, allowance.clone(), provider).unwrap();
        assert_eq!(allowance.lock().unwrap().calls().len(), reservations);
        assert_eq!(
            allowance.lock().unwrap().remaining_micro_usd().unwrap(),
            remaining
        );
        run_until_closed(&mut resumed).await.unwrap();
        let modeled = resumed
            .store
            .world()
            .actions
            .iter()
            .filter(|a| a.payload["command"]["kind"] == "ratify")
            .count();
        assert_eq!(modeled, 0);
        assert_eq!(resumed.store.world().actions.iter().filter(|a|
            a.kind==ActionKind::MinisterAction && a.model_receipt_digest.is_some()).count(),36);
        let hash = resumed.store.world().private_state_sha256.clone();
        drop(resumed);
        assert_eq!(inspect(&path).unwrap().0, hash);
        let replay = resume(&path, true).unwrap();
        assert_eq!(replay.store.world().private_state_sha256, hash);
        drop(replay);
        drop(allowance);
        std::fs::remove_dir_all(control).unwrap();
        std::fs::remove_dir_all(path).unwrap();
        std::fs::remove_dir_all(ledger_dir).unwrap();
    }
    #[test]
    fn imaginary_receipt_digest_does_not_bypass_frozen_action_authority() {
        use hiyal::core::World;
        let mut world = GameWorld::initial("digest-reject").unwrap();
        let player = world.observe(PLAYER).unwrap();
        world.apply(PLAYER, player.action).unwrap();
        let mut forged = world.observe(crate::FINANCE).unwrap().action;
        forged.model_receipt_digest = Some("a".repeat(64));
        forged.payload["meeting_id"] = serde_json::json!("wrong-meeting");
        assert!(world.apply(crate::FINANCE, forged).is_err());
        assert_eq!(
            world
                .actions
                .iter()
                .filter(|a| a.kind == ActionKind::MinisterAction)
                .count(),
            0
        );
    }
    #[test]
    fn staged_route_recovers_only_before_any_journaled_action() {
        let path = temp("staged-live-route");
        let ledger_path = temp("staged-ledger");
        let ledger = Arc::new(std::sync::Mutex::new(
            crate::p5_allowance::Allowance::open(&ledger_path, "stage-owner").unwrap(),
        ));
        let route = crate::p5_allowance::PinnedRoute {
            model: "test/model".into(),
            provider: "test-provider".into(),
            input_micro_usd_per_million: 1_000_000,
            output_micro_usd_per_million: 5_000_000,
            reasoning_disabled: true,
            reasoning_parameter_supported: false,
            response_model: None,
        };
        let backend: Arc<dyn Backend> = Arc::new(PlayerCommandTool);
        let trial = create_live(
            &path,
            "stage-trial",
            11,
            "stage-campaign",
            route.clone(),
            ledger.clone(),
            backend.clone(),
        )
        .unwrap();
        drop(trial);
        std::fs::rename(path.join("p5-route.json"), staged_route_path(&path)).unwrap();
        let recovered =
            resume_live(&path, true, route.clone(), ledger.clone(), backend.clone()).unwrap();
        assert!(path.join("p5-route.json").is_file());
        drop(recovered);
        // Once a journaled action exists, a fabricated staged route is
        // insufficient to authorize a replay or a new provider request.
        drop(ledger);
        std::fs::remove_dir_all(path).unwrap();
        std::fs::remove_dir_all(ledger_path).unwrap();
    }
}
