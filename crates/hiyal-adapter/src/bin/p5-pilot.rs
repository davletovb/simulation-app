//! Manually authorized P5 Experiment A pilot launcher. This binary is not
//! called by CI or the local web API; no live inference starts by default.
//! The 24 normal minister slots are four six-call cabinet meetings. Failure
//! pauses the run, without automatic paid retry or imputed minister support.
use hiyal::runtime::Backend;
use myth_hiyal_adapter::{
    ActionKind,
    orchestrator::{advance_slice, create_live, inspect, resume_live},
    p5_allowance::{Allowance, PinnedRoute},
    p5_openrouter::OpenRouterBackend,
};
use serde_json::json;
use std::{
    env,
    path::Path,
    sync::{Arc, Mutex},
};

fn required(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("missing required operator setting {key}"))
}
fn pinned_route() -> Result<PinnedRoute, String> {
    let route = PinnedRoute {
        model: required("MYTH_P5_MODEL")?,
        provider: required("MYTH_P5_PROVIDER")?,
        input_micro_usd_per_million: required("MYTH_P5_INPUT_MICRO_PER_M")?
            .parse()
            .map_err(|_| "invalid input price")?,
        output_micro_usd_per_million: required("MYTH_P5_OUTPUT_MICRO_PER_M")?
            .parse()
            .map_err(|_| "invalid output price")?,
        reasoning_disabled: true,
        reasoning_parameter_supported: env::var("MYTH_P5_REASONING_PARAMETER_SUPPORTED")
            .ok()
            .as_deref()
            == Some("yes"),
        response_model: env::var("MYTH_P5_RESPONSE_MODEL").ok(),
    };
    route.validate()?;
    // Exact provider-echo model and reasoning support are operator-reviewed
    // route capabilities, frozen in the durable trial route identity.
    if required("MYTH_P5_ROUTE_PRICE_VERIFIED")? != "yes" {
        return Err("operator must separately verify route, schema and prices".into());
    }
    Ok(route)
}
async fn run(trial_dir: &Path, ledger_dir: &Path, installation: &str) -> Result<(), String> {
    if required("MYTH_P5_MANUAL_AUTHORIZATION")? != "AUTHORIZE_34_CALL_EXPERIMENT_A" {
        return Err("manual cost authorization absent; no model request sent".into());
    }
    // All settings checked before attempting to provision the allowance.
    let route = pinned_route()?;
    let provider: Arc<dyn Backend> =
        Arc::new(OpenRouterBackend::from_env(route.clone())?.with_custody_dir(trial_dir));
    let allowance = Arc::new(Mutex::new(Allowance::open(ledger_dir, installation)?));
    let mut trial = if trial_dir.exists() {
        resume_live(trial_dir, false, route, allowance.clone(), provider)?
    } else {
        create_live(
            trial_dir,
            "p5-experiment-a",
            2027,
            "p5-experiment-a",
            route,
            allowance.clone(),
            provider,
        )?
    };
    if trial.store.world().quarter > 4 {
        return Err("four-quarter pilot completed; no further paid attempts permitted".into());
    }
    for _ in 0..50 {
        if trial.store.world().quarter > 4 {
            break;
        }
        if let Err(e) = advance_slice(&mut trial).await {
            return Err(format!(
                "pilot paused at quarter {}; inspect saved completion custody and reconcile charges; Hiyal does not yet support retrying the same failed effect key: {e}",
                trial.store.world().quarter,
            ));
        }
    }
    if trial.store.world().quarter != 5 {
        return Err("bounded pilot did not reach Q5; evaluate partial failure counts".into());
    }
    trial.checkpoint()?;
    let modeled: Vec<_> = trial
        .store
        .world()
        .actions
        .iter()
        .filter(|a| a.kind == ActionKind::MinisterAction)
        .collect();
    let valid = modeled.len();
    let returned_support = modeled
        .iter()
        .filter(|a| a.payload["position"] == "support")
        .count();
    let state_hash = trial.store.world().private_state_sha256.clone();
    let attempted = allowance
        .lock()
        .map_err(|_| "allowance poisoned")?
        .calls()
        .len();
    let reserved = allowance
        .lock()
        .map_err(|_| "allowance poisoned")?
        .remaining_micro_usd()?;
    let receipt_count = trial.effects.receipt_count();
    drop(trial);
    let replay_hash = inspect(trial_dir)?.0;
    if state_hash != replay_hash {
        return Err("strict offline replay failed after live cabinet".into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "experiment":"p5-experiment-a",
            "status":"recorded_pending_behavior_review",
            "planned_action_slots":24,
            "valid_returned_actions":valid,
            "invalid_or_missing_slots":24_usize.saturating_sub(valid),
            "returned_support_votes":returned_support,
            "model_effect_and_player_receipts":receipt_count,
            "installation_total_reservations":attempted,
            "shared_remaining_micro_usd":reserved.to_string(),
            "authoritative_state_sha256":state_hash,
            "strict_replay_sha256":replay_hash,
            "model_behavior_pass_claim":false,
            "model_provider_capability_certified":false
        }))
        .map_err(|_| "could not serialize pilot report")?
    );
    Ok(())
}
fn recovered_trial(
    trial_dir:&Path,ledger_dir:&Path,installation:&str,
)->Result<(myth_hiyal_adapter::orchestrator::GameTrial,PinnedRoute),String>{
    use myth_hiyal_adapter::orchestrator::{resume_live,PlayerCommandTool};
    let bytes=std::fs::read(trial_dir.join("p5-route.json"))
        .map_err(|_|"missing immutable trial route")?;
    let route:PinnedRoute=serde_json::from_slice(&bytes)
        .map_err(|_|"invalid immutable trial route")?;
    let ledger=Arc::new(Mutex::new(Allowance::open(ledger_dir,installation)?));
    let trial=resume_live(trial_dir,true,route.clone(),ledger,Arc::new(PlayerCommandTool))?;
    Ok((trial,route))
}
fn seat_id(value:&str)->Result<hiyal::core::AgentId,String>{
    let seat=match value {"finance"=>1,"education"=>2,"infrastructure"=>3,
        _=>return Err("expected finance|education|infrastructure seat".into())};
    Ok(hiyal::core::AgentId(seat))
}
async fn entry() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        // Reconciliation never calls OpenRouter. An operator must compare
        // the provider's actual invoice before recording a verified charge.
        Some("verify-charge") if args.len()==5 => {
            if env::var("MYTH_P5_VERIFIED_PROVIDER_INVOICE").ok().as_deref()!=Some("yes") {
                return Err("provider invoice verification authorization absent".into());
            }
            let amount=args[4].parse::<u64>().map_err(|_| "invalid micro-USD charge")?;
            let mut ledger=Allowance::open(Path::new(&args[1]),&args[2])?;
            ledger.verify_charge(&args[3],amount)?;
            println!("{}",json!({"operation":"verified_charge_recorded",
                "call_id":args[3],"amount_micro_usd":amount.to_string(),
                "remaining_micro_usd":ledger.remaining_micro_usd()?.to_string()}));
            Ok(())
        }
        Some("authorize-retry") if args.len()==5 => {
            if env::var("MYTH_P5_RETRY_AUTHORIZATION").ok().as_deref()!=Some("AUTHORIZE_FAILED_SEAT") {
                return Err("explicit manual retry approval missing".into());
            }
            let (trial,_route)=recovered_trial(Path::new(&args[1]),Path::new(&args[2]),&args[3])?;
            let actor=seat_id(&args[4])?;
            myth_hiyal_adapter::orchestrator::authorize_failed_minister_retry(&trial,actor)?;
            println!("{}",json!({"operation":"explicit_retry_authorized",
                "actor":actor.0,"paid_dispatch_performed":false}));
            Ok(())
        }
        Some("certify-saved") if args.len()==5 => {
            if env::var("MYTH_P5_CERTIFY_SAVED").ok().as_deref()!=Some("VERIFY_SAVED_COMPLETION") {
                return Err("manual saved-completion certification approval missing".into());
            }
            let (trial,route)=recovered_trial(Path::new(&args[1]),Path::new(&args[2]),&args[3])?;
            let actor=seat_id(&args[4])?;
            let client=OpenRouterBackend::from_env(route)?.with_custody_dir(Path::new(&args[1]));
            myth_hiyal_adapter::orchestrator::certify_failed_minister_saved_completion(
                &trial,actor,&client).await?;
            println!("{}",json!({"operation":"saved_paid_completion_certified",
                "actor":actor.0,"paid_post_performed":false}));
            Ok(())
        }
        Some("inspect") if args.len()==2 => {
            let (hash,chain,sequence)=inspect(Path::new(&args[1]))?;
            println!("{}",json!({
                "mode":"offline_read_only",
                "model_network_calls":0,
                "state_sha256":hash,
                "chain_sha256":chain,
                "journal_events":sequence.to_string()
            }));
            Ok(())
        }
        Some("pilot") if args.len()==4 => {
            run(Path::new(&args[1]),Path::new(&args[2]),&args[3]).await
        }
        _ => Err("usage: p5-pilot inspect <trial-dir> | pilot <trial-dir> <ledger-dir> <installation-id> | authorize-retry <trial-dir> <ledger-dir> <installation-id> <seat> | certify-saved <trial-dir> <ledger-dir> <installation-id> <seat> | verify-charge <ledger-dir> <installation-id> <call-id> <micro-usd>".into()),
    }
}
#[tokio::main]
async fn main() {
    if let Err(e) = entry().await {
        eprintln!("P5 pilot not completed: {e}");
        std::process::exit(2);
    }
}
