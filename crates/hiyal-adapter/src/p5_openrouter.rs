//! Explicitly opt-in OpenRouter model transport. The pinned Hiyal generic
//! HTTP backend cannot submit strict response_format schemas; we keep its
//! Hiyal Backend effect boundary but use an OpenRouter-only HTTP adapter.
//! The API key is never Debug-formatted, logged, serialized, or put in a receipt.
use crate::p5_allowance::{PinnedRoute, Purpose, validate_model_request};
use hiyal::{
    effects::{EffectResult, ModelRequest, Role, Usage},
    runtime::{Backend, BoxResult, EffectSpec},
};
use reqwest::{Client, redirect::Policy};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

const CHAT_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const GENERATION_URL: &str = "https://openrouter.ai/api/v1/generation";
const RESPONSE_LIMIT: usize = 65_536;

pub struct OpenRouterBackend {
    client: Client,
    secret: String,
    route: PinnedRoute,
    custody_dir: Option<PathBuf>,
}
impl OpenRouterBackend {
    /// Deliberately no model choice, secret or network call in constructors.
    /// A caller must explicitly choose a reviewed route and unlock admission.
    pub fn from_env(route: PinnedRoute) -> Result<Self, String> {
        route.validate()?;
        let secret = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| "OPENROUTER_API_KEY unavailable (live disabled)")?;
        if secret.trim().is_empty() || secret.contains(['\r', '\n']) {
            return Err("invalid OpenRouter credential".into());
        }
        if !route.reasoning_parameter_supported
            && std::env::var("MYTH_P5_NON_REASONING_ROUTE_VERIFIED")
                .ok()
                .as_deref()
                != Some("yes")
        {
            return Err("non-reasoning route must be independently certified before omitting reasoning control".into());
        }
        let client = Client::builder()
            .redirect(Policy::none())
            .no_proxy()
            .https_only(true)
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|_| "could not initialize pinned HTTP client")?;
        Ok(Self {
            client,
            secret,
            route,
            custody_dir: None,
        })
    }
    /// Private, fsynced custody of completed provider replies. Required
    /// before live POST dispatch; the completion stays available if the
    /// eventual generation-metadata lookup fails after a paid response.
    pub fn with_custody_dir(mut self, trial_dir: &Path) -> Self {
        self.custody_dir = Some(trial_dir.join("p5-completion-custody"));
        self
    }
    fn preserve_completion(&self, bytes: &[u8]) -> Result<(), String> {
        let dir = self
            .custody_dir
            .as_ref()
            .ok_or("private completion custody not configured")?;
        if !dir
            .parent()
            .is_some_and(|p| p.join("manifest.json").is_file())
        {
            return Err("trial not initialized for provider completion custody".into());
        }
        fs::create_dir_all(dir).map_err(|_| "cannot initialize private completion custody")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
                .map_err(|_| "cannot protect completion custody")?;
        }
        let digest = format!("{:x}", Sha256::digest(bytes));
        let target = dir.join(format!("{digest}.json"));
        if target.exists() {
            if fs::read(target).map_err(|_| "cannot recheck saved completion")? != bytes {
                return Err("saved completion hash collision or corruption".into());
            }
            return Ok(());
        }
        let temp = dir.join(format!("{digest}.tmp"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| "cannot create durable completion custody")?;
        file.write_all(bytes)
            .map_err(|_| "cannot write durable completion")?;
        file.sync_all()
            .map_err(|_| "cannot sync durable completion")?;
        fs::rename(&temp, &target).map_err(|_| "cannot publish durable completion")?;
        #[cfg(unix)]
        fs::File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| "cannot sync completion directory")?;
        Ok(())
    }
    fn request_body(&self, request: &ModelRequest) -> Result<Value, String> {
        validate_model_request(request, &self.route, &Purpose::Authoring)?;
        if request.provider != "openrouter"
            || request.model != self.route.model
            || request.resolved_model_version.as_deref() != Some(self.route.provider.as_str())
            || request.streaming
            || !request.tool_schemas.is_empty()
            || request
                .sampling
                .max_output_tokens
                .is_none_or(|n| n == 0 || n > 800)
            || !self.route.reasoning_disabled
        {
            return Err("unapproved OpenRouter request identity or limits".into());
        }
        let format = request
            .response_format
            .as_ref()
            .ok_or("structured output schema required")?;
        if format["type"] != "json_schema"
            || format["json_schema"]["strict"] != true
            || format["json_schema"]["schema"]["type"] != "object"
            || format["json_schema"]["name"]
                .as_str()
                .is_none_or(|n| n.is_empty() || n.len() > 64)
        {
            return Err("strict JSON schema unsupported or absent".into());
        }
        let messages = request
            .messages
            .iter()
            .map(|msg| {
                if msg.tool_call_id.is_some() || msg.content.len() > 8_192 {
                    return Err("tool messages or oversized context forbidden".into());
                }
                let role = match msg.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => return Err("unapproved tool role".into()),
                };
                Ok(json!({"role":role,"content":msg.content}))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if messages.len() != 2
            || request
                .messages
                .first()
                .is_none_or(|m| m.role != Role::System)
            || request.messages.get(1).is_none_or(|m| m.role != Role::User)
        {
            return Err("ministers require system and typed observation only".into());
        }
        let mut body = json!({
            "model": self.route.model,
            "messages": messages,
            "response_format": format,
            "stream": false,
            "max_tokens": request.sampling.max_output_tokens,
            "provider": {
                "order": [self.route.provider],
                "only": [self.route.provider],
                "allow_fallbacks": false,
                "require_parameters": true,
                "max_price": {
                    "prompt": self.route.input_micro_usd_per_million as f64 / 1_000_000.0,
                    "completion": self.route.output_micro_usd_per_million as f64 / 1_000_000.0
                }
            }
        });
        if self.route.reasoning_parameter_supported {
            body["reasoning"] = json!({"effort":"none"});
        }
        if let Some(t) = request.sampling.temperature {
            body["temperature"] = json!(t);
        }
        if let Some(p) = request.sampling.top_p {
            body["top_p"] = json!(p);
        }
        if let Some(seed) = request.sampling.seed {
            body["seed"] = json!(seed);
        }
        if serde_json::to_vec(&body)
            .map_err(|_| "could not serialize provider request")?
            .len()
            > 20_480
        {
            return Err("OpenRouter request envelope exceeds byte cap".into());
        }
        Ok(body)
    }
    async fn verify_generation(&self, id: &str) -> Result<(), String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("missing safe generation id".into());
        }
        let mut final_response = None;
        // Retry only the read-only generation-metadata GET, never the paid
        // completion POST. OpenRouter may publish metadata asynchronously.
        for n in 0..4 {
            let response = self
                .client
                .get(GENERATION_URL)
                .bearer_auth(&self.secret)
                .query(&[("id", id)])
                .send()
                .await;
            if let Ok(response) = response {
                if response.status().is_success() {
                    final_response = Some(response);
                    break;
                }
                if !(response.status().as_u16() == 404
                    || response.status().as_u16() == 429
                    || response.status().is_server_error())
                {
                    return Err("generation metadata returned terminal status".into());
                }
            }
            if n < 3 {
                tokio::time::sleep(Duration::from_millis(200 * (n + 1))).await;
            }
        }
        let response =
            final_response.ok_or("generation metadata not confirmed after bounded retry")?;
        let bytes = bounded_bytes(response).await?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| "invalid generation metadata")?;
        if value["data"]["model"] != self.route.echoed_model() {
            return Err("generation metadata reported unapproved resolved model".into());
        }
        let actual = value["data"]["provider_name"]
            .as_str()
            .ok_or("generation did not report actual provider")?;
        let normalize = |name: &str| {
            name.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        };
        if normalize(actual) != normalize(&self.route.provider) {
            return Err("unapproved actual generation provider".into());
        }
        Ok(())
    }
}
async fn bounded_bytes(mut response: reqwest::Response) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "provider response truncated")?
    {
        if body
            .len()
            .checked_add(chunk.len())
            .is_none_or(|size| size > RESPONSE_LIMIT)
        {
            return Err("provider response too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
/// Provider responses remain untrusted; a separate frozen role validator
/// converts their content into an authoritative typed minister action.
pub fn parse_completion(value: &Value, model: &str) -> Result<(EffectResult, String), String> {
    if value["model"] != model {
        return Err("OpenRouter returned different model".into());
    }
    let choice = value["choices"].get(0).ok_or("missing provider choice")?;
    if choice["finish_reason"] != "stop"
        || choice["message"]["tool_calls"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
        || choice["message"]["refusal"].as_str().is_some()
    {
        return Err("incomplete, refused, or unauthorized tool response".into());
    }
    let content = choice["message"]["content"]
        .as_str()
        .ok_or("provider omitted content")?;
    if content.len() > 8_192 {
        return Err("oversized minister completion".into());
    }
    let id = value["id"].as_str().ok_or("missing generation id")?;
    let input = value["usage"]["prompt_tokens"].as_u64();
    let output = value["usage"]["completion_tokens"].as_u64();
    let cost = value["usage"]["cost"]
        .as_f64()
        .filter(|c| c.is_finite() && *c >= 0.0)
        .map(|c| c.to_string());
    Ok((
        EffectResult::Model {
            content: content.into(),
            effective_model: model.into(),
            usage: if value["usage"].is_null() {
                None
            } else {
                Some(Usage {
                    input_tokens: input,
                    output_tokens: output,
                    billed_cost: cost,
                    currency: Some("USD".into()),
                })
            },
        },
        id.into(),
    ))
}
impl Backend for OpenRouterBackend {
    fn backend_id(&self) -> &'static str {
        "openrouter-strict-v1"
    }
    fn effective_provider(&self, _: &EffectSpec) -> String {
        format!("openrouter/{}", self.route.provider)
    }
    fn unsupported_is_pre_dispatch(&self) -> bool {
        false
    }
    fn resolve<'a>(&'a self, spec: &'a EffectSpec) -> BoxResult<'a> {
        Box::pin(async move {
            let EffectSpec::Model(request) = spec else {
                return Err("model transport cannot execute tools".into());
            };
            let body = self.request_body(request)?;
            if self.custody_dir.is_none() {
                return Err("private completion custody must be configured before POST".into());
            }
            // Do not send the key, request content or response in error strings.
            let response = self
                .client
                .post(CHAT_URL)
                .bearer_auth(&self.secret)
                .json(&body)
                .send()
                .await
                .map_err(|_| "OpenRouter transport failure; charge unknown")?;
            if !response.status().is_success() {
                return Err("OpenRouter HTTP failure; charge unknown".into());
            }
            let bytes = bounded_bytes(response).await?;
            // This fsync precedes all schema/identity/generation checks.
            // Never put raw model output in error text or a public log.
            self.preserve_completion(&bytes)?;
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "malformed provider response; charge unknown")?;
            let (output, generation) = parse_completion(&value, self.route.echoed_model())?;
            // The completion alone does not name the actual provider. Query
            // its generation metadata before admitting a typed minister vote.
            self.verify_generation(&generation).await?;
            // Return the immutable requested model identity after verifying
            // the exact approved echoed model and actual provider separately.
            let output = match output {
                EffectResult::Model { content, usage, .. } => EffectResult::Model {
                    content,
                    usage,
                    effective_model: self.route.model.clone(),
                },
                other => other,
            };
            Ok(output)
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fake_responses_reject_wrong_models_refusals_tools_and_partial_streams() {
        let base = json!({
            "id":"gen-test","model":"test/model",
            "choices":[{"finish_reason":"stop","message":{"content":"{}"}}],
            "usage":{"prompt_tokens":21,"completion_tokens":15}
        });
        assert!(parse_completion(&base, "test/model").is_ok());
        let mut without_usage = base.clone();
        without_usage.as_object_mut().unwrap().remove("usage");
        assert!(parse_completion(&without_usage, "test/model").is_ok());
        let mut bad = base.clone();
        bad["model"] = json!("redirected/model");
        assert!(parse_completion(&bad, "test/model").is_err());
        bad = base.clone();
        bad["choices"][0]["finish_reason"] = json!("length");
        assert!(parse_completion(&bad, "test/model").is_err());
        bad = base.clone();
        bad["choices"][0]["message"]["tool_calls"] = json!([{"name":"execute"}]);
        assert!(parse_completion(&bad, "test/model").is_err());
        bad = base.clone();
        bad["choices"][0]["message"]["refusal"] = json!("cannot");
        assert!(parse_completion(&bad, "test/model").is_err());
    }
}
