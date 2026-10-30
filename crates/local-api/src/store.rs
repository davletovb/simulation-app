//! Bounded local *recording* storage, not a Hiyal replay journal.
//! Only authored pilot choices are persisted; no minister-private effort,
//! provider credentials, LLM outputs, or authoritative world state.
use myth_contracts::Allocation as WireAllocation;
use myth_domain::{self as d, ContractError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

const MAX_RECORD_BYTES: usize = 65_536;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedChoice {
    pub quarter: u8,
    pub meeting_id: String,
    pub allocation: WireAllocation,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptedRecord {
    pub schema_version: u16,
    pub record_id: String,
    pub choices: Vec<RecordedChoice>,
}
impl ScriptedRecord {
    pub fn validate(&self) -> Result<(), StoreError> {
        if self.schema_version != d::SCHEMA_VERSION || !valid_filename(&self.record_id) {
            return Err(StoreError::InvalidRecord);
        }
        if self.choices.len() > usize::from(d::SIX_QUARTERS) {
            return Err(StoreError::InvalidRecord);
        }
        let mut previous = 0_u8;
        for choice in &self.choices {
            d::validate_identifier(&choice.meeting_id).map_err(StoreError::Domain)?;
            d::Quarter::new(choice.quarter).map_err(StoreError::Domain)?;
            if choice.quarter <= previous {
                return Err(StoreError::InvalidRecord);
            }
            previous = choice.quarter;
            let allocation = d::Allocation::try_from(choice.allocation.clone())
                .map_err(|_| StoreError::InvalidRecord)?;
            if allocation.total() > u32::from(d::PILOT_CREDITS) {
                return Err(StoreError::InvalidRecord);
            }
        }
        Ok(())
    }
}
fn valid_filename(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredRecord {
    pub payload: ScriptedRecord,
    pub sha256: String,
}
#[derive(Debug)]
pub enum StoreError {
    InvalidRecord,
    Missing,
    Corrupt,
    Domain(ContractError),
    Io(std::io::Error),
}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StoreError {}
impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
fn hash(payload: &ScriptedRecord) -> Result<String, StoreError> {
    let bytes = serde_json::to_vec(payload).map_err(|_| StoreError::Corrupt)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// Scoped to a single filesystem directory. Serialize local writers; use a
/// same-directory temporary file, fsync file then rename and fsync directory.
/// No process-crossing lock or multi-instance recovery guarantee in P1.
pub struct SaveStore {
    root: PathBuf,
    lock: Mutex<()>,
}
impl SaveStore {
    pub fn new(root: PathBuf) -> Result<Self, StoreError> {
        fs::create_dir_all(&root)?;
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(StoreError::InvalidRecord);
        }
        Ok(Self {
            root,
            lock: Mutex::new(()),
        })
    }
    fn path(&self, id: &str) -> Result<PathBuf, StoreError> {
        if !valid_filename(id) {
            return Err(StoreError::InvalidRecord);
        }
        Ok(self.root.join(format!("{id}.json")))
    }
    pub fn put(&self, payload: ScriptedRecord) -> Result<StoredRecord, StoreError> {
        payload.validate()?;
        let stored = StoredRecord {
            sha256: hash(&payload)?,
            payload,
        };
        let bytes = serde_json::to_vec(&stored).map_err(|_| StoreError::Corrupt)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(StoreError::InvalidRecord);
        }
        let path = self.path(&stored.payload.record_id)?;
        let _guard = self.lock.lock().map_err(|_| StoreError::Corrupt)?;
        // Uniqueness across fast saves, PID reuse and container PID 1 restarts.
        // Always create_new and retry rather than deleting unknown crash orphans.
        let mut opened = None;
        for _ in 0..8 {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| StoreError::Corrupt)?
                .as_nanos();
            let nonce = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate =
                path.with_extension(format!("tmp-{}-{timestamp}-{nonce}", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => {
                    opened = Some((candidate, file));
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(StoreError::Io(err)),
            }
        }
        let (tmp, mut file) = opened.ok_or(StoreError::Corrupt)?;
        let write = (|| -> Result<(), StoreError> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&tmp, &path)?;
            Ok(())
        })();
        if write.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        write?;
        // Rename has already replaced the visible record. A subsequent
        // directory-sync error cannot honestly be reported as a failed save.
        // On platforms without directory fsync, the process cannot promise
        // crash durability; P4/P7 need separately tested recovery semantics.
        #[cfg(unix)]
        match File::open(&self.root).and_then(|directory| directory.sync_all()) {
            Ok(()) => {}
            Err(error) => eprintln!(
                "Warning: saved choice record was replaced, but directory durability is unconfirmed: {error}"
            ),
        }
        Ok(stored)
    }
    pub fn get(&self, id: &str) -> Result<StoredRecord, StoreError> {
        let path = self.path(id)?;
        let _guard = self.lock.lock().map_err(|_| StoreError::Corrupt)?;
        let mut f = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(StoreError::Missing),
            Err(e) => return Err(StoreError::Io(e)),
        };
        let mut bytes = Vec::new();
        Read::take(&mut f, (MAX_RECORD_BYTES + 1) as u64).read_to_end(&mut bytes)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(StoreError::Corrupt);
        }
        let record: StoredRecord =
            serde_json::from_slice(&bytes).map_err(|_| StoreError::Corrupt)?;
        record.payload.validate().map_err(|_| StoreError::Corrupt)?;
        if record.payload.record_id != id || record.sha256 != hash(&record.payload)? {
            return Err(StoreError::Corrupt);
        }
        Ok(record)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("myth-p1d-{}-{stamp}", std::process::id()))
    }
    fn record(id: &str) -> ScriptedRecord {
        ScriptedRecord {
            schema_version: 1,
            record_id: id.into(),
            choices: vec![RecordedChoice {
                quarter: 1,
                meeting_id: "m1".into(),
                allocation: WireAllocation {
                    staff: 50,
                    repairs: 40,
                    access: 10,
                },
            }],
        }
    }

    #[test]
    fn save_restore_integrity_and_tamper_detection() {
        let root = temp();
        let store = SaveStore::new(root.clone()).unwrap();
        let original = store.put(record("test-1")).unwrap();
        assert_eq!(store.get("test-1").unwrap(), original);
        // Verify persisted records survive reopening the storage owner.
        let restarted = SaveStore::new(root.clone()).unwrap();
        assert_eq!(restarted.get("test-1").unwrap(), original);
        let mut corrupted = serde_json::to_value(&original).unwrap();
        corrupted["payload"]["choices"][0]["allocation"]["staff"] = 49.into();
        fs::write(
            store.path("test-1").unwrap(),
            serde_json::to_vec(&corrupted).unwrap(),
        )
        .unwrap();
        assert!(matches!(store.get("test-1"), Err(StoreError::Corrupt)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_path_traversal_overspend_and_duplicate_quarters() {
        let root = temp();
        let store = SaveStore::new(root.clone()).unwrap();
        assert!(matches!(
            store.put(record("../escape")),
            Err(StoreError::InvalidRecord)
        ));
        assert!(matches!(
            store.get("bad/name"),
            Err(StoreError::InvalidRecord)
        ));
        assert!(matches!(
            store.put(record("Pilot1")),
            Err(StoreError::InvalidRecord)
        ));
        let mut bad = record("safe");
        bad.choices[0].allocation.staff = 99;
        assert!(matches!(store.put(bad), Err(StoreError::InvalidRecord)));
        let mut dup = record("safe");
        dup.choices.push(dup.choices[0].clone());
        assert!(matches!(store.put(dup), Err(StoreError::InvalidRecord)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_crash_temporary_file_cannot_block_new_save() {
        let root = temp();
        let store = SaveStore::new(root.clone()).unwrap();
        let stale = store
            .path("pilot")
            .unwrap()
            .with_extension(format!("tmp-{}", std::process::id()));
        fs::write(&stale, b"stale interrupted write").unwrap();
        let saved = store.put(record("pilot")).unwrap();
        assert_eq!(store.get("pilot").unwrap(), saved);
        assert!(
            stale.exists(),
            "do not delete temp files from unknown owners"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn overwritten_record_has_exactly_one_authoritative_file() {
        let root = temp();
        let store = SaveStore::new(root.clone()).unwrap();
        let a = store.put(record("one")).unwrap();
        let mut b = record("one");
        b.choices[0].allocation.access = 9;
        let updated = store.put(b).unwrap();
        assert_ne!(a.sha256, updated.sha256);
        assert_eq!(store.get("one").unwrap(), updated);
        assert_eq!(fs::read_dir(root.clone()).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
