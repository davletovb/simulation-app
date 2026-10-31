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
use myth_hiyal_adapter::{
    ActionKind, GameWorld,
    orchestrator::{GameTrial, advance_slice, create_campaign, inspect, resume},
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
    Propose { allocation: WireAllocation },
    Amend { allocation: WireAllocation },
    Decide { ratify: bool },
}
impl CommandKind {
    fn body(&self,world:&GameWorld)->Result<Option<Value>,String>{
        let meeting=format!("q{}",world.quarter);
        let proposal_version=||->Result<String,String>{
            Ok(world.fold()?.proposal_version()
                .ok_or("no frozen proposal version")?.to_decimal())
        };
        Ok(match self {
            Self::AdvanceSlice=>None,
            Self::Propose{allocation}=>Some(json!({
                "kind":"propose","meeting_id":meeting,"allocation":allocation,
            })),
            Self::Amend{allocation}=>Some(json!({
                "kind":"amend","meeting_id":meeting,
                "proposal_version":proposal_version()?,
                "allocation":allocation,
            })),
            Self::Decide{ratify:true}=>Some(json!({
                "kind":"ratify","meeting_id":meeting,
                "proposal_version":proposal_version()?,
            })),
            Self::Decide{ratify:false}=>Some(json!({"kind":"defer"})),
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicVote {
    ministry: String,
    stage: String,
    position: String,
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
    let mut votes = Vec::new();
    for action in &world.actions {
        if action.kind == ActionKind::PlayerCommand {
            match action.payload["command"]["kind"].as_str() {
                Some("propose") => {
                    proposal = Some(action.payload["command"]["allocation"].clone());
                    outcome = None;
                    votes.clear();
                }
                Some("ratify") => outcome = Some("ratified".into()),
                Some("defer") => outcome = Some("deferred".into()),
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
        outcome,
        assumption:"Synthetic education-delivery rules; scripted cabinet, not live ministers or empirical predictions".into(),
        mode:"offline_scripted",
    })
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
}
pub struct Connected {
    root: PathBuf,
    owners: Mutex<BTreeMap<String, Arc<Owner>>>,
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
        Ok(Arc::new(Self {
            root,
            owners: Mutex::new(BTreeMap::new()),
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
            return Err("campaign identity differs from path".into());
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
            let pending_round=engine.trial.store.dir.join("pending-round.json").exists();
            let result = if current != expected && !pending_round {
                Ok(())
            } else {
                let body=engine.log.records[pos].kind.body(engine.trial.store.world());
                let prepared=match body {
                    Ok(Some(payload))=>engine.trial.select_player_command(
                        &engine.log.records[pos].command_id,payload),
                    Ok(None)=>Ok(()),
                    Err(error)=>Err(error),
                };
                let applied=match prepared {
                    Ok(())=>advance_slice(&mut engine.trial).await.map(|_|()),
                    Err(error)=>Err(error),
                };
                engine.trial.clear_player_command();
                applied
            };
            let snap = capture(&engine.trial);
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
                    snapshot: capture(&engine.trial).unwrap_or(PublicView {
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
        if owner.engine.lock().await.log.records.iter()
            .any(|r|r.status==OperationStatus::Accepted) {
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
        snapshot: capture(&engine.trial)
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
    if let Some(body)=request.kind.body(engine.trial.store.world())
        .map_err(|_|status(StatusCode::CONFLICT,"command lacks a frozen proposal"))? {
        engine.trial.store.world().selected_player_command(&request.command_id,body)
            .map_err(|_|status(StatusCode::CONFLICT,"typed player action invalid at this phase"))?;
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
async fn stream_events(
    State(service): State<Arc<Connected>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Failure> {
    let owner = service
        .owner(&id)
        .map_err(|_| status(StatusCode::CONFLICT, "campaign unavailable"))?;
    // Register for live events BEFORE taking the one-boundary snapshot.
    let live = owner.sender.subscribe();
    let engine = owner.engine.lock().await;
    let state =
        capture(&engine.trial).map_err(|_| status(StatusCode::CONFLICT, "verification failed"))?;
    let operation = engine.log.records.last().cloned();
    drop(engine);
    let last = headers.get("last-event-id").and_then(|h| h.to_str().ok());
    let id_value = format!("{}:{}", state.campaign_id, state.cursor);
    // Always send a same-boundary snapshot. Duplicate cursor is harmless and
    // client-side reducer explicitly ignores older/duplicate versions.
    let first = Event::default()
        .event(if last == Some(id_value.as_str()) {
            "snapshot"
        } else {
            "snapshot_fallback"
        })
        .id(id_value)
        .json_data(Committed {
            snapshot: state,
            operation,
        })
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
pub fn routes() -> Router<Arc<Connected>> {
    Router::new()
        .route("/api/campaigns/{id}/snapshot", get(snapshot))
        .route("/api/campaigns/{id}/commands", post(command))
        .route("/api/campaigns/{id}/commands/{command_id}", get(operation))
        .route("/api/campaigns/{id}/events", get(stream_events))
}
#[cfg(test)]
mod tests {
    use super::*;
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
}
