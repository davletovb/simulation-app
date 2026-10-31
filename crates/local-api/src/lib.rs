//! P1 offline loopback adapter: scripted votes, pure Rust forecast, and
//! checksum-verified local *choice* recordings. No policy mutations or LLM.
#![forbid(unsafe_code)]
pub mod checkpoint_store;
pub mod connected;
pub mod store;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use myth_contracts::{
    Allocation as WireAllocation, ForecastLine, ForecastResponse, Ministry, PreviewCalculator,
    PreviewKind, ScriptedSeatVote, ScriptedVoteKind, ScriptedVotes, Vote,
};
use myth_domain::{self as d, preview::optimistic_preview};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use store::{SaveStore, ScriptedRecord, StoreError, StoredRecord};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Scripted,
    Recorded,
}
impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "scripted" => Ok(Self::Scripted),
            "recorded" => Ok(Self::Recorded),
            "live" => {
                Err("live inference is not implemented; refusing all network/model dispatch".into())
            }
            _ => Err("MYTH_MODE must be scripted or recorded".into()),
        }
    }
}

#[derive(Debug)]
pub struct Config {
    pub mode: Mode,
    pub bind: SocketAddr,
    pub save_dir: PathBuf,
}
impl Config {
    pub fn from_env() -> Result<Self, String> {
        let mode = Mode::parse(&std::env::var("MYTH_MODE").unwrap_or_else(|_| "scripted".into()))?;
        let bind: SocketAddr = std::env::var("MYTH_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8787".into())
            .parse()
            .map_err(|_| "MYTH_BIND must be a numeric loopback socket address")?;
        if !bind.ip().is_loopback() {
            return Err("MYTH_BIND must use a loopback IP address".into());
        }
        let save_dir = std::env::var_os("MYTH_SAVE_DIR")
            .map_or_else(|| PathBuf::from("local-data/saves"), PathBuf::from);
        Ok(Self {
            mode,
            bind,
            save_dir,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    mode: Mode,
    store: Arc<SaveStore>,
    connected: Arc<connected::Connected>,
}
impl AppState {
    pub fn new(mode: Mode, save_dir: PathBuf) -> Result<Self, StoreError> {
        Ok(Self {
            mode,
            connected: connected::Connected::new(save_dir.join("hiyal-campaigns"))
                .map_err(|_| StoreError::Corrupt)?,
            store: Arc::new(SaveStore::new(save_dir)?),
        })
    }
    pub fn save_root(&self) -> &std::path::Path {
        self.store.root()
    }
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}
#[derive(Debug)]
enum ApiError {
    Invalid,
    Missing,
    Corrupt,
    Storage,
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Invalid => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Invalid pilot record or allocation",
            ),
            Self::Missing => (StatusCode::NOT_FOUND, "not_found", "Recording not found"),
            Self::Corrupt => (
                StatusCode::CONFLICT,
                "integrity_failure",
                "Recorded content failed validation or checksum",
            ),
            Self::Storage => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_failure",
                "Local storage operation failed",
            ),
        };
        (status, Json(ErrorBody { code, message })).into_response()
    }
}
impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::Missing => Self::Missing,
            StoreError::Corrupt => Self::Corrupt,
            StoreError::InvalidRecord | StoreError::Domain(_) => Self::Invalid,
            StoreError::Io(_) => Self::Storage,
        }
    }
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    mode: Mode,
    schema_version: u16,
    network_calls: bool,
    authoritative_simulation: bool,
    connected_authoritative_simulation: bool,
}
async fn health(State(state): State<AppState>) -> Json<Health> {
    Json(Health {
        status: "ok",
        mode: state.mode,
        schema_version: d::SCHEMA_VERSION,
        network_calls: false,
        authoritative_simulation: false,
        connected_authoritative_simulation: true,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationRequest {
    pub allocation: WireAllocation,
}

fn forecast(req: AllocationRequest) -> Result<ForecastResponse, ApiError> {
    let a = d::Allocation::try_from(req.allocation).map_err(|_| ApiError::Invalid)?;
    let preview = optimistic_preview(a).map_err(|_| ApiError::Invalid)?;
    Ok(ForecastResponse {
        kind: PreviewKind::OptimisticUpperBound,
        calculator: PreviewCalculator::MythDomain,
        authoritative_simulation: false,
        conditional_access_bonus: preview.conditional_access_bonus,
        families: preview
            .families
            .into_iter()
            .map(|f| ForecastLine {
                family: f.family.into(),
                per_quarter: f.per_quarter,
                outstanding: f.outstanding,
            })
            .collect(),
    })
}
async fn preview(Json(req): Json<AllocationRequest>) -> Result<Json<ForecastResponse>, ApiError> {
    forecast(req).map(Json)
}

fn scripted_votes(req: AllocationRequest) -> Result<ScriptedVotes, ApiError> {
    let a = d::Allocation::try_from(req.allocation).map_err(|_| ApiError::Invalid)?;
    if a.total() > u32::from(d::PILOT_CREDITS) {
        return Err(ApiError::Invalid);
    }
    // P0 clean-belief baseline only; 500,000 ppm, no missed commitments.
    // An invalid package is not submitted to the scripted electorate.
    let support = [true, a.staff() >= 45, a.repairs() >= 35];
    let votes = [
        ScriptedSeatVote {
            ministry: Ministry::Finance,
            position: if support[0] {
                Vote::Support
            } else {
                Vote::Oppose
            },
        },
        ScriptedSeatVote {
            ministry: Ministry::Education,
            position: if support[1] {
                Vote::Support
            } else {
                Vote::Oppose
            },
        },
        ScriptedSeatVote {
            ministry: Ministry::Infrastructure,
            position: if support[2] {
                Vote::Support
            } else {
                Vote::Oppose
            },
        },
    ];
    let count = support.iter().filter(|v| **v).count() as u8;
    Ok(ScriptedVotes {
        kind: ScriptedVoteKind::CleanBeliefScriptedVotes,
        passes_support_threshold: count >= 2,
        institutional_ratification: false,
        votes,
        support_count: count,
        warning:
            "Illustrative clean-belief snapshot only; no meeting, ratification or treasury mutation"
                .into(),
    })
}
async fn cabinet(Json(req): Json<AllocationRequest>) -> Result<Json<ScriptedVotes>, ApiError> {
    scripted_votes(req).map(Json)
}
async fn put_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(record): Json<ScriptedRecord>,
) -> Result<Json<StoredRecord>, ApiError> {
    if id != record.record_id {
        return Err(ApiError::Invalid);
    }
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || store.put(record))
        .await
        .map_err(|_| ApiError::Storage)?
        .map(Json)
        .map_err(Into::into)
}
async fn get_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StoredRecord>, ApiError> {
    let store = state.store.clone();
    tokio::task::spawn_blocking(move || store.get(&id))
        .await
        .map_err(|_| ApiError::Storage)?
        .map(Json)
        .map_err(Into::into)
}

fn is_allowed_local_authority(host: &str) -> bool {
    let authority = host.trim();
    for base in ["localhost", "127.0.0.1", "[::1]"] {
        if authority == base {
            return true;
        }
        if let Some(rest) = authority.strip_prefix(base)
            && let Some(port) = rest.strip_prefix(':')
            && !port.is_empty()
            && port.bytes().all(|c| c.is_ascii_digit())
        {
            return true;
        }
    }
    false
}
async fn local_request_guard(req: axum::extract::Request, next: Next) -> Response {
    let headers: &HeaderMap = req.headers();
    let host_ok = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(is_allowed_local_authority);
    let origin_ok = headers
        .get(header::ORIGIN)
        .map(|v| {
            v.to_str().ok().is_some_and(|origin| {
                origin
                    .strip_prefix("http://")
                    .is_some_and(is_allowed_local_authority)
            })
        })
        .unwrap_or(true);
    if !host_ok || !origin_ok {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorBody {
                code: "nonlocal_origin",
                message: "Only loopback requests are supported",
            }),
        )
            .into_response();
    }
    // Non-browser tools can omit Origin; all modifying endpoints require a
    // JSON Content-Type (HTML forms and simple cross-site POSTs cannot write).
    if matches!(*req.method(), Method::POST | Method::PUT) {
        let json = headers
            .get(header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|s| s.to_ascii_lowercase().starts_with("application/json"));
        if !json {
            return (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(ErrorBody {
                    code: "json_required",
                    message: "application/json is required",
                }),
            )
                .into_response();
        }
    }
    next.run(req).await
}
/// Routes deliberately never accept Ratify/AdvanceQuarter as a state
/// transition: no world reducer or Hiyal journal exists in P1.
pub fn router(state: AppState) -> Router {
    let connected = state.connected.clone();
    Router::new()
        .merge(connected::routes().with_state(connected))
        .route("/api/health", get(health))
        .route("/api/preview", post(preview))
        .route("/api/scripted/meeting", post(cabinet))
        .route("/api/recordings/{id}", get(get_record).put(put_record))
        .layer(DefaultBodyLimit::max(65_536))
        .layer(middleware::from_fn(local_request_guard))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baseline_vote_fixtures_without_any_mutation() {
        let request = |staff, repairs, access| AllocationRequest {
            allocation: WireAllocation {
                staff,
                repairs,
                access,
            },
        };
        assert_eq!(
            scripted_votes(request(50, 40, 10)).unwrap().support_count,
            3
        );
        assert_eq!(
            scripted_votes(request(60, 30, 10)).unwrap().support_count,
            2
        );
        assert_eq!(
            scripted_votes(request(40, 30, 20)).unwrap().support_count,
            1
        );
        assert!(matches!(
            scripted_votes(request(60, 40, 10)),
            Err(ApiError::Invalid)
        ));
    }
    #[test]
    fn optimistic_preview_matches_domain_not_typescript() {
        let v = forecast(AllocationRequest {
            allocation: WireAllocation {
                staff: 0,
                repairs: 0,
                access: 25,
            },
        })
        .unwrap();
        assert_eq!(v.families[2].per_quarter, [4, 5, 5, 5, 5, 1]);
        assert!(!v.authoritative_simulation);
    }
    #[test]
    fn refuses_remote_listener_and_live_inference() {
        assert!(Mode::parse("live").is_err());
        assert_eq!(Mode::parse("recorded").unwrap(), Mode::Recorded);
        assert!(is_allowed_local_authority("127.0.0.1:8787"));
        assert!(is_allowed_local_authority("localhost:5173"));
        assert!(!is_allowed_local_authority("localhost.evil.example"));
        assert!(!is_allowed_local_authority("0.0.0.0:8787"));
        assert!(!is_allowed_local_authority("127.0.0.1:80.evil"));
    }
}
