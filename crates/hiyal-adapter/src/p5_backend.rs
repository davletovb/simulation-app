//! Game-owned admission adapter. Hiyal's per-trial GovernedBackend is
//! deliberately OUTSIDE this wrapper. The game ledger is the authority
//! across all Hiyal trials and non-trial assistant/authoring calls.
use crate::p5_allowance::{Allowance, PinnedRoute, Purpose, Scope};
use hiyal::{
    effects::{EffectCallKey, EffectResult, ModelRequest},
    guard::{BudgetConfig, GovernedBackend},
    runtime::{Backend, BillingDisposition, BoxResult, EffectSpec},
};
use serde_json::to_vec;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

pub const MAX_TRIAL_EFFECTS: u64 = 96;
pub fn fixed_trial_budget() -> BudgetConfig {
    BudgetConfig {
        maximum_micro_usd: 1_500_000,
        prompt_micro_usd_per_token: 1,
        output_micro_usd_per_token: 5,
        max_input_bytes: 20_480,
        max_output_tokens: 800,
    }
}
/// Never construct a budgeted trial with a process-shared GovernedBackend:
/// recovery of one trial would otherwise overwrite another trial's cap.
pub fn governed_trial(backend: Arc<dyn Backend>) -> GovernedBackend {
    GovernedBackend::new(backend, MAX_TRIAL_EFFECTS).with_budget(fixed_trial_budget())
}
pub struct SharedAdmissionBackend {
    ledger: Arc<Mutex<Allowance>>,
    inner: Arc<dyn Backend>,
    route: PinnedRoute,
    scope: Mutex<Scope>,
}
impl SharedAdmissionBackend {
    pub fn new(
        ledger: Arc<Mutex<Allowance>>, inner: Arc<dyn Backend>,
        route: PinnedRoute, scope: Scope,
    ) -> Result<Self, String> {
        route.validate()?;
        scope.validate_for_backend()?;
        if inner.permits_tools() {
            return Err("tool-capable backend forbidden behind model admission".into());
        }
        Ok(Self { ledger, inner, route, scope: Mutex::new(scope) })
    }
    pub fn set_scope(&self, scope: Scope) -> Result<(), String> {
        scope.validate_for_backend()?;
        *self.scope.lock().map_err(|_| "scope lock poisoned")? = scope;
        Ok(())
    }
    fn call_id(key: &EffectCallKey) -> Result<String, String> {
        let hash = Sha256::digest(to_vec(key).map_err(|e| e.to_string())?);
        Ok(format!("effect-{hash:x}"))
    }
    fn admitted(&self, call_id: &str, model: &ModelRequest) -> Result<(), String> {
        let mut locked = self.ledger.lock().map_err(|_| "shared allowance poisoned")?;
        let result = locked.reserve(call_id, self.scope.lock().map_err(|_| "scope lock poisoned")?.clone(), self.route.clone(), model)?;
        if !result.fresh {
            return Err("existing attempt requires receipt reconciliation; redispatch forbidden".into());
        }
        locked.mark_dispatched(call_id)
    }
    fn settled(&self, call_id: &str, result: &Result<EffectResult, String>) -> Result<(), String> {
        let (input, output) = match result {
            Ok(EffectResult::Model { usage: Some(u), .. }) => (u.input_tokens, u.output_tokens),
            _ => (None, None),
        };
        self.ledger.lock().map_err(|_| "shared allowance poisoned")?
            .record(call_id, input, output, None)
    }
    /// Explicitly charged, stable non-trial attempt. No Hiyal trial budget
    /// exists for this call; it still shares the authoritative local cap.
    pub async fn invoke_outside(
        &self, call_id: &str, request: &ModelRequest,
    ) -> Result<EffectResult, String> {
        if matches!(self.scope.lock().map_err(|_| "scope lock poisoned")?.purpose, Purpose::Minister) {
            return Err("minister must be invoked through a trial".into());
        }
        self.admitted(call_id, request)?;
        let spec = EffectSpec::Model(request.clone());
        let response = self.inner.resolve(&spec).await;
        self.settled(call_id, &response)?;
        Self::checked_response(response, &self.route)
    }
    fn checked_response(
        response: Result<EffectResult, String>, route: &PinnedRoute,
    ) -> Result<EffectResult, String> {
        let result = response?;
        match &result {
            EffectResult::Model { effective_model, .. } if effective_model == &route.model => {
                Ok(result)
            }
            EffectResult::Model { .. } => Err("provider returned an unapproved model identity".into()),
            EffectResult::UnsupportedCapability { .. } =>
                Err("remote capability not certified; potential charge reserved".into()),
            EffectResult::Failed { category, .. } => Err(format!("provider failed: {category}")),
            _ => Err("unexpected non-model provider result".into()),
        }
    }
}
impl Backend for SharedAdmissionBackend {
    fn backend_id(&self) -> &'static str { "myth-openrouter-admission" }
    fn billing_disposition(&self, _: &EffectSpec, result: &EffectResult) -> BillingDisposition {
        if matches!(result, EffectResult::Model { .. }) {
            BillingDisposition::Charged
        } else {
            BillingDisposition::Unknown
        }
    }
    // No provider response from this adapter can claim it was not dispatched.
    fn unsupported_is_pre_dispatch(&self) -> bool { false }
    fn resolve<'a>(&'a self, _: &'a EffectSpec) -> BoxResult<'a> {
        Box::pin(async { Err("all trial effects require a stable Hiyal call key".into()) })
    }
    fn resolve_keyed<'a>(
        &'a self, key: &'a EffectCallKey, spec: &'a EffectSpec,
    ) -> BoxResult<'a> {
        Box::pin(async move {
            let EffectSpec::Model(model) = spec else {
                return Err("tool effects forbidden in model admission".into());
            };
            if spec.key(&key.trial_id, key.logical_step, key.agent_id, key.call_index)? != *key {
                return Err("effect identity differs from key".into());
            }
            let id = Self::call_id(key)?;
            self.admitted(&id, model)?;
            let response = self.inner.resolve(spec).await;
            self.settled(&id, &response)?;
            Self::checked_response(response, &self.route)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::p5_allowance::INITIAL_MICRO_USD;
    use hiyal::effects::{Message, Role, Sampling, Usage};
    use std::{sync::atomic::{AtomicUsize,Ordering},time::{SystemTime,UNIX_EPOCH}};
    struct Fake(AtomicUsize);
    impl Backend for Fake {
        fn resolve<'a>(&'a self, spec: &'a EffectSpec) -> BoxResult<'a> {
            Box::pin(async move {
                self.0.fetch_add(1, Ordering::SeqCst);
                let EffectSpec::Model(m) = spec else { unreachable!() };
                Ok(EffectResult::Model {
                    content:"{}".into(), effective_model:m.model.clone(),
                    usage: Some(Usage{input_tokens:None,output_tokens:None,billed_cost:None,currency:None})
                })
            })
        }
    }
    fn route() -> PinnedRoute { PinnedRoute {
        model:"test/model".into(),provider:"provider".into(),
        input_micro_usd_per_million:1_000_000,output_micro_usd_per_million:5_000_000,
        reasoning_disabled:true
    }}
    fn req() -> ModelRequest { ModelRequest {
        provider:"openrouter".into(), model:"test/model".into(), resolved_model_version:Some("pin".into()),
        messages:vec![Message{role:Role::User,content:"safe".into(),tool_call_id:None}],
        sampling:Sampling{temperature:Some(0.0),top_p:None,seed:None,max_output_tokens:Some(400)},
        tool_schemas:vec![],streaming:false,
        response_format:Some(serde_json::json!({"type":"json_schema","json_schema":{"strict":true,"name":"test","schema":{"type":"object"}}}))
    }}
    #[tokio::test]
    async fn keyed_and_outside_calls_share_one_cap_without_double_dispatch() {
        let nonce=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!("myth-admit-{}-{nonce}",std::process::id()));
        let ledger=Arc::new(Mutex::new(Allowance::open(&path,"test").unwrap()));
        let fake=Arc::new(Fake(AtomicUsize::new(0)));
        let campaign=SharedAdmissionBackend::new(ledger.clone(),fake.clone(),route(),Scope {
            purpose:Purpose::Minister,campaign_id:Some("a".into()),quarter:Some(1),batch_id:None,
        }).unwrap();
        let spec=EffectSpec::Model(req());
        let key=spec.key("trial",1,hiyal::core::AgentId(1),0).unwrap();
        assert!(campaign.resolve_keyed(&key,&spec).await.is_ok());
        assert!(campaign.resolve_keyed(&key,&spec).await.is_err());
        let helper=SharedAdmissionBackend::new(ledger.clone(),fake.clone(),route(),Scope {
            purpose:Purpose::Assistant,campaign_id:None,quarter:None,batch_id:None,
        }).unwrap();
        assert!(helper.invoke_outside("outside-a",&req()).await.is_ok());
        assert_eq!(fake.0.load(Ordering::SeqCst),2);
        let remaining=ledger.lock().unwrap().remaining_micro_usd().unwrap();
        assert!(remaining<INITIAL_MICRO_USD);
        drop(campaign); drop(helper); drop(ledger);
        let recovered=Allowance::open(&path,"test").unwrap();
        assert_eq!(remaining,recovered.remaining_micro_usd().unwrap());
        std::fs::remove_dir_all(path).unwrap();
    }
}
