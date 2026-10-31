//! Scripted P4 Hiyal orchestration and allowlisted zero-side-effect tool.
//! The effect journal is always bound to the trial by Hiyal before dispatch.
//! A tool receipt precedes a player-command action; no user data or provider
//! connection is used here.
use crate::{ACTORS,ActionKind,GameAction,GameObservation,GameWorld,PLAYER,PERIOD,WORLD_KIND};
use hiyal::{
    core::{AgentId, World},
    effects::{EffectResult,ModelRequest,Message,Role,Sampling,Usage},
    experiment::{EffectPolicy,Step,Trial},
    runtime::{Backend,BillingDisposition,BoxResult,EffectSpec,Receipt},
    scheduler::Scheduling,
    storage::Durability,
};
use serde_json::json;
use std::{
    collections::{BTreeMap,BTreeSet},
    path::{Path,PathBuf},
    sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering}},
};

#[derive(Default,Clone)]
pub struct GamePolicy {
    /// Test-only interrupted-after-receipt injection, never configured for live.
    pub fail_once_after_receipt: Option<Arc<AtomicBool>>,
    pub fail_role_once: Option<(AgentId,Arc<AtomicBool>)>,
}
impl GamePolicy {
    fn specification(obs:&GameObservation)->Result<Vec<EffectSpec>,String>{
        if obs.synthetic_receipts && (1..=3).contains(&obs.actor) {
            let count=if obs.actor<=2 {3} else {2};
            return Ok((0..count).map(|index| EffectSpec::Model(ModelRequest{
                provider:"p4-synthetic-offline".into(),
                model:"scripted-p4-receipt-v1".into(),
                resolved_model_version:Some("synthetic-no-network-v1".into()),
                messages:vec![Message{
                    role:Role::User,
                    content:format!("q{} actor{} {} receipt{} v{}",
                        obs.quarter,obs.actor,obs.phase,index,obs.expected_world_version),
                    tool_call_id:None,
                }],
                sampling:Sampling{
                    temperature:Some(0.0),top_p:None,seed:Some(0),
                    max_output_tokens:Some(1),
                }, tool_schemas:vec![],streaming:false,response_format:None,
            })).collect());
        }
        if obs.action.kind!=ActionKind::PlayerCommand || obs.actor!=PLAYER.0 as u8 {
            return Ok(vec![]);
        }
        let body=&obs.action.payload;
        let id=body["command_id"].as_str().ok_or("player command id missing")?;
        let operation_id=format!("game-{id}-v{}",obs.expected_world_version);
        Ok(vec![EffectSpec::Tool {
            name:"player_command".into(),
            input:json!({
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
    fn requests(&self,actor:AgentId,obs:&GameObservation)->Result<Vec<EffectSpec>,String>{
        if actor.0!=u64::from(obs.actor) {return Err("observation actor mismatch".into());}
        Self::specification(obs)
    }
    fn decide(&self,actor:AgentId,obs:&GameObservation,receipts:&[Receipt])
        ->Result<GameAction,String>
    {
        if actor.0!=u64::from(obs.actor){return Err("forged minister id".into());}
        let expected=Self::specification(obs)?;
        if expected.len()!=receipts.len() {return Err("effect receipt count mismatch".into());}
        for (spec,receipt) in expected.iter().zip(receipts) {
            if receipt.spec!=*spec || receipt.billing!=BillingDisposition::NotCharged {
                return Err("incorrect request or billing disposition".into());
            }
            let valid=match (&receipt.result,spec) {
                (EffectResult::Tool{output},EffectSpec::Tool{input,..})=>output==input,
                (EffectResult::Model{content,effective_model,..},EffectSpec::Model(m))=>
                    m.provider=="p4-synthetic-offline"
                    && content=="synthetic:ok"
                    && effective_model==&m.model,
                _=>false,
            };
            if !valid {return Err("untrusted or changed effect receipt".into());}
        }
        if actor==PLAYER && expected.len()==1
            && self.fail_once_after_receipt.as_ref()
                .is_some_and(|flag|flag.swap(false,Ordering::SeqCst))
        {
            panic!("P4 TEST FAILPOINT after receipt fsync; before world action");
        }
        if self.fail_role_once.as_ref()
            .is_some_and(|(id,flag)|*id==actor && flag.swap(false,Ordering::SeqCst)) {
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
    fn permits_tools(&self)->bool{true}
    fn unsupported_is_pre_dispatch(&self)->bool{true}
    fn backend_id(&self)->&'static str{"p4-offline-allowlist"}
    fn billing_disposition(&self,_:&EffectSpec,_:&EffectResult)->BillingDisposition {
        BillingDisposition::NotCharged
    }
    fn resolve<'a>(&'a self,request:&'a EffectSpec)->BoxResult<'a>{
        Box::pin(async move {
            if let EffectSpec::Model(model)=request {
                if model.provider!="p4-synthetic-offline"
                    || model.model!="scripted-p4-receipt-v1"
                    || model.streaming
                    || model.messages.len()!=1
                {return Ok(EffectResult::UnsupportedCapability{
                    capability:"all real models disabled in P4".into(),
                });}
                return Ok(EffectResult::Model{
                    content:"synthetic:ok".into(),effective_model:model.model.clone(),
                    usage:Some(Usage{input_tokens:Some(0),output_tokens:Some(0),
                        billed_cost:Some("0".into()),currency:Some("USD".into())}),
                });
            }
            let EffectSpec::Tool{name,input}=request else {
                return Ok(EffectResult::UnsupportedCapability{
                    capability:"P4 only accepts allowlisted effects".into()
                });
            };
            if name!="player_command" || input["schema_version"]!=1
                || input["actor"]!=0
                || input["command_body"]["schema_version"]!=1
            {return Ok(EffectResult::UnsupportedCapability{
                capability:"unapproved P4 effect".into()
            });}
            let operation=input["operation_id"].as_str()
                .ok_or("missing stable operation id")?;
            let command=input["command_body"]["command_id"].as_str()
                .ok_or("missing command id")?;
            let version=input["expected_world_version"].as_str()
                .ok_or("missing expected version")?;
            if operation!=format!("game-{command}-v{version}")
                || input["command_body"]["expected_world_version"]!=version
                || serde_json::to_vec(input).map_err(|e|e.to_string())?.len()>12_288
            {return Err("invalid or oversized player effect".into());}
            myth_contracts::parse_command(
                &input["command_body"].to_string()).map_err(|e|format!("{e:?}"))?;
            Ok(EffectResult::Tool{output:input.clone()})
        })
    }
}
pub type GameTrial=Trial<GameWorld,GamePolicy>;
static PAUSE_CACHE: OnceLock<Mutex<BTreeMap<PathBuf,BTreeSet<AgentId>>>>=OnceLock::new();
fn pause_cache()->&'static Mutex<BTreeMap<PathBuf,BTreeSet<AgentId>>>{
    PAUSE_CACHE.get_or_init(||Mutex::new(BTreeMap::new()))
}
fn invalidate_pauses(path:&Path){
    if let Ok(mut map)=pause_cache().lock(){map.remove(path);}
}
pub fn create(dir:&Path,name:&str,seed:u64)->Result<GameTrial,String>{
    invalidate_pauses(dir);
    let mut trial=Trial::create(
        dir,name,seed,WORLD_KIND,GameWorld::initial("p4-pilot")?,
        GamePolicy::default(),Arc::new(PlayerCommandTool),ACTORS.to_vec(),
        Scheduling::RoundRobin,
    )?;
    trial.store.set_durability(Durability::EveryEvent)?;
    Ok(trial)
}
pub fn create_stress(dir:&Path,name:&str,seed:u64)->Result<GameTrial,String>{
    invalidate_pauses(dir);
    let mut trial=Trial::create(
        dir,name,seed,WORLD_KIND,GameWorld::stress("p4-pilot")?,
        GamePolicy::default(),Arc::new(PlayerCommandTool),ACTORS.to_vec(),
        Scheduling::RoundRobin,
    )?;
    trial.store.set_durability(Durability::EveryEvent)?;
    Ok(trial)
}
pub fn resume(dir:&Path,replay_only:bool)->Result<GameTrial,String>{
    invalidate_pauses(dir);
    let mut trial=Trial::resume(
        dir,WORLD_KIND,GamePolicy::default(),Arc::new(PlayerCommandTool),replay_only
    )?;
    trial.store.set_durability(Durability::EveryEvent)?;
    trial.store.world().verify()?;
    Ok(trial)
}
fn enable(trial:&mut GameTrial,allowed:&[AgentId])->Result<(),String>{
    // Hiyal deliberately keeps its live scheduler private. Reconstruct the
    // actual persisted pause set through its verified read-only API only once
    // per process/dir, then cache updates after successful durable controls.
    // Invalidate on create/resume so recovery never trusts an old local cache.
    let dir=trial.store.dir.clone();
    let mut cache=pause_cache().lock().map_err(|_|"pause cache poisoned")?;
    if !cache.contains_key(&dir) {
        let verified=hiyal::storage::Store::<GameWorld,GameAction>
            ::inspect_read_only(&dir)?;
        cache.insert(dir.clone(),verified.schedule.paused);
    }
    for actor in ACTORS {
        let paused=cache.get(&dir).is_some_and(|x|x.contains(&actor));
        let should_pause=!allowed.contains(&actor);
        if paused!=should_pause {
            let outcome=if should_pause {trial.pause_actor(actor)}
                else {trial.resume_actor(actor)};
            if let Err(error)=outcome {
                cache.remove(&dir);
                return Err(error);
            }
            let current=cache.get_mut(&dir).ok_or("missing cached schedule")?;
            if should_pause {current.insert(actor);} else {current.remove(&actor);}
        }
    }
    Ok(())
}

fn check_step(step:Step<GameWorld>)->Result<(),String>{
    match step {
        Step::Accepted(_)=>Ok(()),
        Step::Rejected{reason,actor}=>Err(format!("actor {} rejected: {reason}",actor.0)),
        Step::NoEligible=>Err("no eligible actor".into()),
    }
}
/// One explicitly chosen logical action or three frozen minister decisions.
/// Scheduling changes are Hiyal durable control records, *not* calendar time.
pub async fn advance_slice(trial:&mut GameTrial)->Result<bool,String>{
    if trial.store.world().is_closed(){return Ok(false);}
    let phase=trial.store.world().phase.clone();
    match phase.as_str() {
        "quarter_open"|"amendment_window"=>{
            enable(trial,&[PLAYER])?;
            check_step(trial.step().await?)?;
        }
        "resolved"=>{
            enable(trial,&[PERIOD])?;
            check_step(trial.step().await?)?;
            trial.checkpoint()?;
        }
        "final_positions_frozen" if trial.store.world().final_votes_this_meeting()==3 =>{
            enable(trial,&[PLAYER])?;
            check_step(trial.step().await?)?;
        }
        "first_positions_frozen"|"final_positions_frozen"=>{
            enable(trial,&crate::ACTORS[1..4])?;
            let steps=trial.run_round(3).await?;
            if steps.len()>3 {return Err("more than three frozen actions".into());}
            // On a recovered frozen plan, previously committed actors must
            // not be replayed; Hiyal returns only the unfinished suffix.
            for step in steps {check_step(step)?;}
        }
        _=>return Err("unsupported or missing cabinet phase".into()),
    }
    trial.store.world().verify()?;
    Ok(true)
}
/// Performance-only sequential cabinet path: measures each complete Hiyal
/// role step (including EveryEvent fsync), rather than dividing an opaque
/// three-actor frozen batch by three. Recovery acceptance still separately
/// verifies the production frozen run_round path, including crash suffixes.
pub async fn benchmark_sequential_cabinet(trial:&mut GameTrial)
    ->Result<Vec<u128>,String>
{
    let phase=trial.store.world().phase.as_str();
    if phase!="first_positions_frozen" && phase!="final_positions_frozen" {
        return Err("not inside a cabinet phase".into());
    }
    enable(trial,&crate::ACTORS[1..4])?;
    let mut times=vec![];
    for _ in 0..3 {
        let begin=std::time::Instant::now();
        check_step(trial.step().await?)?;
        times.push(begin.elapsed().as_micros());
    }
    trial.store.world().verify()?;
    Ok(times)
}
pub async fn run_until_closed(trial:&mut GameTrial)->Result<(),String>{
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
pub fn inspect(dir:&Path)->Result<(String,String,u64),String>{
    use hiyal::storage::Store;
    let validated=Store::<GameWorld,GameAction>::inspect_read_only(dir)?;
    validated.world.verify()?;
    Ok((validated.world.private_state_sha256,validated.chain_head,validated.sequence))
}

/// Never confuse replay-only (no new tool or model dispatch) with the
/// operational resume mode (which can admit new offline player tool calls).
pub fn final_private_hash(dir:&Path)->Result<String,String>{
    Ok(inspect(dir)?.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hiyal::runtime::EffectJournal;
    use std::time::{SystemTime,UNIX_EPOCH};
    fn temp(name:&str)->std::path::PathBuf {
        let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("p4-{name}-{}-{nonce}",std::process::id()))
    }
    #[tokio::test]
    async fn six_quarters_save_resume_matches_full_validation_without_network(){
        let path=temp("replay");
        let expected;
        {
            let mut trial=create(&path,"scripted-pilot",43).unwrap();
            advance_slice(&mut trial).await.unwrap();
            advance_slice(&mut trial).await.unwrap(); // frozen first round
            assert_eq!(trial.store.world().phase,"amendment_window");
            trial.checkpoint().unwrap();
            let prior=inspect(&path).unwrap();
            assert!(!prior.0.is_empty());
        }
        {
            let mut resumed=resume(&path,false).unwrap();
            run_until_closed(&mut resumed).await.unwrap();
            assert_eq!(resumed.store.world().quarter,6);
            assert!(resumed.store.world().is_closed());
            assert_eq!(resumed.store.world().actions.iter()
                .filter(|a|a.kind==ActionKind::MinisterAction).count(),36);
            expected=resumed.store.world().private_state_sha256.clone();
        }
        let (restored,chain,_sequence)=inspect(&path).unwrap();
        assert_eq!(restored,expected);
        assert_eq!(chain.len(),64);
        let strict=resume(&path,true).unwrap();
        assert_eq!(strict.store.world().private_state_sha256,expected);
        drop(strict);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn interrupted_frozen_round_replays_only_pending_ministers(){
        let path=temp("frozen-recovery");
        let mut trial=create(&path,"frozen-round",47).unwrap();
        enable(&mut trial,&[PLAYER]).unwrap();
        check_step(trial.step().await.unwrap()).unwrap();
        assert_eq!(trial.store.world().phase,"first_positions_frozen");
        enable(&mut trial,&crate::ACTORS[1..4]).unwrap();
        trial.policy.fail_role_once=Some((crate::EDUCATION,Arc::new(AtomicBool::new(true))));
        let task=tokio::spawn(async move {trial.run_round(3).await});
        assert!(task.await.unwrap_err().is_panic());
        assert!(path.join("pending-round.json").is_file());
        let mut restored=resume(&path,false).unwrap();
        let remaining=restored.run_round(3).await.unwrap();
        assert!(!remaining.is_empty());
        for event in remaining {check_step(event).unwrap();}
        assert_eq!(restored.store.world().phase,"amendment_window");
        assert_eq!(restored.store.world().actions.iter()
            .filter(|x|x.kind==ActionKind::MinisterAction).count(),3);
        run_until_closed(&mut restored).await.unwrap();
        let hash=restored.store.world().private_state_sha256.clone();
        drop(restored);
        assert_eq!(inspect(&path).unwrap().0,hash);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn one_writer_lock_survives_and_releases_on_close(){
        let path=temp("writer");
        let trial=create(&path,"writer-lease",48).unwrap();
        assert!(resume(&path,false).is_err(),"second process-style writer must be denied");
        drop(trial);
        let reopened=resume(&path,false).unwrap();
        assert_eq!(reopened.store.world().actions.len(),0);
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
    struct UncertainOfflineFailure;
    impl Backend for UncertainOfflineFailure {
        fn resolve<'a>(&'a self,_:&'a EffectSpec)->BoxResult<'a> {
            Box::pin(async move {Ok(EffectResult::Failed{
                category:"test-uncertain".into(),
                message:"synthetic provider outcome unknown".into()
            })})
        }
    }
    #[tokio::test]
    async fn unknown_synthetic_charge_survives_restart_without_auto_refund(){
        let path=temp("unknown-bill");
        let mut trial=create_stress(&path,"unknown-charge",49).unwrap();
        enable(&mut trial,&[PLAYER]).unwrap();
        check_step(trial.step().await.unwrap()).unwrap();
        enable(&mut trial,&crate::ACTORS[1..4]).unwrap();
        trial.backend=Arc::new(UncertainOfflineFailure);
        let result=trial.run_round(1).await.unwrap();
        assert_eq!(result.len(),1);
        assert!(matches!(&result[0],Step::Rejected{..}));
        let unknown=trial.effects.receipts().values().filter(|receipt|
            receipt.billing==BillingDisposition::Unknown).count();
        assert_eq!(unknown,3);
        drop(trial);
        let reopened=resume(&path,false).unwrap();
        assert_eq!(reopened.effects.receipts().values().filter(|receipt|
            receipt.billing==BillingDisposition::Unknown).count(),3);
        drop(reopened);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn typed_tool_is_allowlisted_and_never_billed(){
        let path=temp("tool");
        let trial=create(&path,"tool-receipt",44).unwrap();
        let obs=trial.store.world().observe(PLAYER).unwrap();
        let spec=GamePolicy::specification(&obs).unwrap();
        assert_eq!(spec.len(),1);
        let backend=PlayerCommandTool;
        let result=backend.resolve(&spec[0]).await.unwrap();
        assert!(matches!(result,EffectResult::Tool{..}));
        assert_eq!(backend.billing_disposition(&spec[0],&result),
            BillingDisposition::NotCharged);
        let disallowed=EffectSpec::Tool{name:"shell".into(),input:json!({})};
        assert!(matches!(backend.resolve(&disallowed).await.unwrap(),
            EffectResult::UnsupportedCapability{..}));
        drop(trial);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn stress_runs_all_36_minister_actions_and_96_fake_model_receipts(){
        let path=temp("stress");
        let mut trial=create_stress(&path,"stress-pilot",46).unwrap();
        run_until_closed(&mut trial).await.unwrap();
        assert_eq!(trial.store.world().stress_cohorts.len(),1024);
        assert_eq!(trial.store.world().actions.iter().filter(|x|
            x.kind==ActionKind::MinisterAction).count(),36);
        let synthetic=trial.effects.receipts().values().filter(|x|
            matches!(x.spec,EffectSpec::Model(_))).count();
        assert_eq!(synthetic,96);
        assert!(trial.effects.receipts().values().all(|x|
            x.billing==BillingDisposition::NotCharged));
        let hash=trial.store.world().private_state_sha256.clone();
        drop(trial);
        assert_eq!(inspect(&path).unwrap().0,hash);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[tokio::test]
    async fn receipt_survives_panic_before_world_commit_and_is_reused(){
        let path=temp("receipt-panic");
        let mut trial=create(&path,"receipt-recovery",45).unwrap();
        enable(&mut trial,&[PLAYER]).unwrap();
        trial.policy.fail_once_after_receipt=Some(Arc::new(AtomicBool::new(true)));
        let task=tokio::spawn(async move {trial.step().await});
        assert!(task.await.unwrap_err().is_panic());
        let receipts=EffectJournal::inspect_read_only(
            &path,&hiyal::core::TrialId::new("receipt-recovery",45).0).unwrap();
        assert_eq!(receipts,1);
        let mut resumed=resume(&path,true).unwrap();
        assert_eq!(resumed.store.world().actions.len(),0);
        check_step(resumed.step().await.unwrap()).unwrap();
        assert_eq!(resumed.store.world().actions.len(),1);
        drop(resumed);
        assert_eq!(EffectJournal::inspect_read_only(
            &path,&hiyal::core::TrialId::new("receipt-recovery",45).0).unwrap(),1);
        std::fs::remove_dir_all(path).unwrap();
    }
}
