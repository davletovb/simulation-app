//! Explicit private local checkpoint commands for P1 handoff verification.
//! These do not execute a game or ingest real England data.
use myth_domain::{CohortWeights, world::WorldSnapshot};
use myth_local_api::checkpoint_store::PrivateArchiveStore;
use std::path::PathBuf;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err("usage: myth-checkpoint <save-fixture|verify> <save-directory> <campaign-id> <pack-sha256>".into());
    }
    let save_dir = PathBuf::from(&args[2]);
    let store = PrivateArchiveStore::new(save_dir)?;
    match args[1].as_str() {
        "save-fixture" => {
            let weights =
                CohortWeights::new([200_000, 200_000, 100_000, 150_000, 200_000, 150_000])?;
            let world = WorldSnapshot::initial(args[3].clone(), "fixture-pack".into(), weights)?;
            let hash = store.save(&args[3], &world, &args[4])?;
            println!(
                "Saved fictional Q1 test checkpoint: campaign={} sha256={hash}",
                args[3]
            );
        }
        "verify" => {
            let world = store.restore(&args[3], &args[4])?;
            println!(
                "Verified private checkpoint: campaign={} version={} quarter={}",
                world.campaign_id(),
                world.world_version().to_decimal(),
                world.quarter().value()
            );
        }
        _ => return Err("unknown checkpoint command; use save-fixture or verify".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Checkpoint failed: {error}");
        std::process::exit(1)
    }
}
