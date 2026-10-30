//! Private server-only checkpoint archive. Never attach these files to HTTP
//! responses. This is a validated checkpoint prototype, not Hiyal replay.
use myth_contracts::checkpoint::PrivateCheckpoint;
use myth_domain::world::WorldSnapshot;
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{
    fs::{self,File,OpenOptions},
    io::{Read,Write},
    path::PathBuf,
    sync::{Mutex,atomic::{AtomicU64,Ordering}},
    time::{SystemTime,UNIX_EPOCH},
};
const MAX_BYTES:usize=1_048_576;
static SERIAL:AtomicU64=AtomicU64::new(0);
#[derive(Debug)]
pub enum ArchiveError{Invalid,Missing,Corrupt,Io(std::io::Error)}
impl std::fmt::Display for ArchiveError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
        write!(f,"{self:?}")
    }
}
impl std::error::Error for ArchiveError {}
impl From<std::io::Error> for ArchiveError{
    fn from(e:std::io::Error)->Self{Self::Io(e)}
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive{
    checkpoint:PrivateCheckpoint,
    sha256:String,
}
fn content_hash(data:&PrivateCheckpoint)->Result<String,ArchiveError>{
    Ok(format!("{:x}",Sha256::digest(serde_json::to_vec(data)
        .map_err(|_|ArchiveError::Invalid)?)))
}
fn clean_id(id:&str)->bool{
    !id.is_empty()&&id.len()<=64&&id.bytes()
        .all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'_'||b==b'-')
}
pub struct PrivateArchiveStore{
    root:PathBuf,
    lock:Mutex<()>,
}
impl PrivateArchiveStore{
    pub fn new(root:PathBuf)->Result<Self,ArchiveError>{
        fs::create_dir_all(&root)?;
        let root=root.canonicalize()?;
        if !root.is_dir(){return Err(ArchiveError::Invalid)}
        Ok(Self{root,lock:Mutex::new(())})
    }
    fn path(&self,id:&str)->Result<PathBuf,ArchiveError>{
        if !clean_id(id){return Err(ArchiveError::Invalid)}
        Ok(self.root.join(format!("{id}.world.json")))
    }
    pub fn save(&self,id:&str,world:&WorldSnapshot,pack_sha256:&str)
        ->Result<String,ArchiveError>
    {
        let path=self.path(id)?;
        let checkpoint=PrivateCheckpoint::capture(world,pack_sha256.into())
            .map_err(|_|ArchiveError::Invalid)?;
        if checkpoint.campaign_id!=id{return Err(ArchiveError::Invalid)}
        let sha=content_hash(&checkpoint)?;
        let bytes=serde_json::to_vec(&Archive{checkpoint,sha256:sha.clone()})
            .map_err(|_|ArchiveError::Invalid)?;
        if bytes.len()>MAX_BYTES{return Err(ArchiveError::Invalid)}
        let _guard=self.lock.lock().map_err(|_|ArchiveError::Corrupt)?;
        let mut created=None;
        for _ in 0..8{
            let tick=SystemTime::now().duration_since(UNIX_EPOCH)
                .map_err(|_|ArchiveError::Corrupt)?.as_nanos();
            let id=SERIAL.fetch_add(1,Ordering::Relaxed);
            let temp=path.with_extension(format!("tmp-{}-{tick}-{id}",std::process::id()));
            match OpenOptions::new().create_new(true).write(true).open(&temp){
                Ok(file)=>{created=Some((temp,file));break}
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>continue,
                Err(e)=>return Err(ArchiveError::Io(e)),
            }
        }
        let (temp,mut file)=created.ok_or(ArchiveError::Corrupt)?;
        let wrote=(||->Result<(),ArchiveError>{
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp,&path)?;
            Ok(())
        })();
        if wrote.is_err(){let _=fs::remove_file(temp);}
        wrote?;
        #[cfg(unix)]
        if let Err(err)=File::open(&self.root).and_then(|f|f.sync_all()){
            eprintln!("World file replaced; directory fsync unconfirmed: {err}");
        }
        Ok(sha)
    }
    pub fn restore(&self,id:&str,pack_sha256:&str)
        ->Result<WorldSnapshot,ArchiveError>
    {
        let path=self.path(id)?;
        let _guard=self.lock.lock().map_err(|_|ArchiveError::Corrupt)?;
        let mut file=match File::open(path){
            Ok(f)=>f,
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Err(ArchiveError::Missing),
            Err(e)=>return Err(ArchiveError::Io(e)),
        };
        let mut bytes=Vec::new();
        Read::take(&mut file,(MAX_BYTES+1) as u64).read_to_end(&mut bytes)?;
        if bytes.len()>MAX_BYTES{return Err(ArchiveError::Corrupt)}
        let archive:Archive=serde_json::from_slice(&bytes)
            .map_err(|_|ArchiveError::Corrupt)?;
        if archive.checkpoint.campaign_id!=id ||
            content_hash(&archive.checkpoint)?!=archive.sha256{
            return Err(ArchiveError::Corrupt)
        }
        archive.checkpoint.restore(pack_sha256).map_err(|_|ArchiveError::Corrupt)
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    use myth_domain::CohortWeights;
    const PACK:&str="1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
    fn tmp()->PathBuf{
        let t=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("myth-private-{}-{t}",std::process::id()))
    }
    fn seed()->WorldSnapshot{
        WorldSnapshot::initial("campaign".into(),"test-pack".into(),
            CohortWeights::new([200_000,200_000,100_000,150_000,200_000,150_000]).unwrap()).unwrap()
    }
    #[test]
    fn survives_reopen_and_detects_corruption_and_pack_mismatch(){
        let root=tmp();
        let store=PrivateArchiveStore::new(root.clone()).unwrap();
        let digest=store.save("campaign",&seed(),PACK).unwrap();
        assert_eq!(digest.len(),64);
        let reopened=PrivateArchiveStore::new(root.clone()).unwrap();
        assert_eq!(reopened.restore("campaign",PACK).unwrap().world_version().value(),0);
        assert!(reopened.restore("campaign",&"0".repeat(64)).is_err());
        let path=reopened.path("campaign").unwrap();
        let mut content=fs::read_to_string(&path).unwrap();
        content=content.replacen(r#""quarter":1"#,r#""quarter":2"#,1);
        fs::write(path,content).unwrap();
        assert!(matches!(reopened.restore("campaign",PACK),Err(ArchiveError::Corrupt)));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejecting_cross_campaign_paths_and_ids(){
        let root=tmp();
        let store=PrivateArchiveStore::new(root.clone()).unwrap();
        assert!(matches!(store.save("../campaign",&seed(),PACK),Err(ArchiveError::Invalid)));
        assert!(matches!(store.save("other",&seed(),PACK),Err(ArchiveError::Invalid)));
        assert!(matches!(store.restore("Campaign",PACK),Err(ArchiveError::Invalid)));
        fs::remove_dir_all(root).unwrap();
    }
}
