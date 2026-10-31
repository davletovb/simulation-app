//! Private offline P4 verification and microbenchmark CLI.
use myth_contracts::checkpoint::Phase;
use hiyal::storage::Durability;
use myth_hiyal_adapter::orchestrator::{
    advance_slice, benchmark_sequential_cabinet, create, create_stress, inspect, resume,
    run_until_closed,
};
use serde_json::json;
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

fn id_path(tag: &str, n: usize) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    env::temp_dir().join(format!("myth-p4-{tag}-{}-{stamp}-{n}", std::process::id()))
}
fn bytes_on_disk(path: &Path) -> Result<u64, String> {
    let mut sum = 0_u64;
    for f in fs::read_dir(path).map_err(|e| e.to_string())? {
        let f = f.map_err(|e| e.to_string())?;
        let md = f.metadata().map_err(|e| e.to_string())?;
        if md.is_file() {
            sum += md.len();
        } else if md.is_dir() {
            sum += bytes_on_disk(&f.path())?;
        }
    }
    Ok(sum)
}
fn peak_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
        let kb = line.split_whitespace().nth(1)?.parse::<u64>().ok()?;
        Some(kb * 1024)
    }
    #[cfg(not(target_os = "linux"))]
    {
        // Do not mislabel the sampled current RSS from ps as true peak RSS.
        None
    }
}
fn sorted_median_p95_us(samples: &[u128]) -> serde_json::Value {
    if samples.is_empty() {
        return json!(null);
    }
    let mut x = samples.to_vec();
    x.sort_unstable();
    let p95 = (95 * x.len())
        .div_ceil(100)
        .saturating_sub(1)
        .min(x.len() - 1);
    json!({"samples":x.len(),"median_us":x[x.len()/2].to_string(),
        "p95_us":x[p95].to_string(),"min_us":x[0].to_string(),
        "max_us":x[x.len()-1].to_string()})
}
async fn benchmark(count: usize, stress: bool) -> Result<(), String> {
    if !(3..=10).contains(&count) {
        return Err("benchmark sample count must be 3..10".into());
    }
    let mut cabinets = Vec::new();
    let mut advances = Vec::new();
    let mut restores = Vec::new();
    let mut verifies = Vec::new();
    let mut checkpoints = Vec::new();
    let mut journals = Vec::new();
    let mut verified_hash = None::<String>;
    let mut accepted_actions = 0;
    let mut effect_receipts = 0;
    for i in 0..count {
        let path = id_path(if stress { "stress" } else { "prototype" }, i);
        let mut trial = if stress {
            create_stress(&path, &format!("p4-bench-stress-{i}"), 23)?
        } else {
            create(&path, &format!("p4-bench-base-{i}"), 23)?
        };
        let start = Instant::now();
        for _ in 0..60 {
            if trial.store.world().is_closed() {
                break;
            }
            let phase = trial.store.world().phase.clone();
            if phase == Phase::FirstPositionsFrozen
                || (phase == Phase::FinalPositionsFrozen
                    && trial.store.world().final_votes_this_meeting() < 3)
            {
                for committed_us in benchmark_sequential_cabinet(&mut trial).await? {
                    cabinets.push(committed_us);
                }
            } else {
                let now = Instant::now();
                advance_slice(&mut trial).await?;
                let us = now.elapsed().as_micros();
                if phase == Phase::Resolved {
                    advances.push(us);
                }
            }
        }
        if !trial.store.world().is_closed() {
            return Err("benchmark trial did not close".into());
        }
        trial.checkpoint()?;
        // Read the actual configured mode, and perform an independent
        // explicit fsync probe. This is not proof of every prior syscall.
        if trial.store.durability()!=Durability::EveryEvent {
            return Err("EveryEvent durability was not configured".into());
        }
        trial.store.sync_log()?;
        let elapsed = start.elapsed();
        let hash = trial.store.world().private_state_sha256.clone();
        if verified_hash.as_ref().is_some_and(|known| known != &hash) {
            return Err("same authored world produced different state digests".into());
        }
        verified_hash = Some(hash.clone());
        accepted_actions = trial.store.world().actions.len();
        effect_receipts = trial.effects.receipt_count();
        let snap = fs::metadata(path.join("snapshot.json"))
            .map_err(|e| e.to_string())?
            .len();
        checkpoints.push(snap);
        journals.push(bytes_on_disk(&path)?);
        drop(trial);
        let timer = Instant::now();
        let recovered = resume(&path, true)?;
        restores.push(timer.elapsed().as_micros());
        assert_eq!(recovered.store.world().private_state_sha256, hash);
        drop(recovered);
        let now = Instant::now();
        let independent = inspect(&path)?;
        verifies.push(now.elapsed().as_micros());
        if independent.0 != hash {
            return Err("read-only replay digest differs".into());
        }
        fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
        eprintln!(
            "P4 sample {i}: runtime_ms={} world_hash={hash}",
            elapsed.as_millis()
        );
    }
    let hardware = std::thread::available_parallelism()
        .map(|x| x.get())
        .unwrap_or(0);
    let toolchain = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unavailable".into())
        .trim()
        .to_string();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version":1,"experiment":"P4-B-prime",
            "target_os":env::consts::OS,"target_arch":env::consts::ARCH,
            "logical_cpus":hardware,"rustc":toolchain,
            "durability":"every_event",
            "explicit_sync_probe":"succeeded",
            "synthetic_stress_records":if stress{1024}else{0},
            "model_receipts_are_zero_cost_synthetic":true,
            "fake_model_receipts":if stress{96}else{0},
            "total_effect_receipts":effect_receipts,
            "accepted_game_actions":accepted_actions,
            "case_sha256":verified_hash,
            "cabinet_action_commit":sorted_median_p95_us(&cabinets),
            "quarter_advance_commit":sorted_median_p95_us(&advances),
            "resume_full_validation":sorted_median_p95_us(&restores),
            "independent_full_verify":sorted_median_p95_us(&verifies),
            "checkpoint_bytes_max":checkpoints.iter().max(),
            "trial_disk_bytes_max":journals.iter().max(),
            "peak_rss_bytes":peak_rss_bytes(),
            "peak_rss_metric":if cfg!(target_os="linux"){"linux_proc_vm_hwm"}else{"not_measured"},
            "samples":count
        }))
        .map_err(|e| e.to_string())?
    );
    Ok(())
}
async fn entry() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args
        .first()
        .ok_or("use run|stress|resume|inspect|benchmark <dir|N>")?;
    match command.as_str() {
        "run" | "stress" => {
            let path = Path::new(args.get(1).ok_or("missing trial directory")?);
            let mut trial = if command == "stress" {
                create_stress(path, "p4-verified", 42)?
            } else {
                create(path, "p4-verified", 42)?
            };
            run_until_closed(&mut trial).await?;
            let hash = trial.store.world().private_state_sha256.clone();
            trial.checkpoint()?;
            drop(trial);
            if inspect(path)?.0 != hash {
                return Err("full verification drift".into());
            }
            println!("P4_AUTHORITATIVE_SHA256={hash}");
        }
        "resume" => {
            let path = Path::new(args.get(1).ok_or("missing trial directory")?);
            let mut trial = resume(path, false)?;
            run_until_closed(&mut trial).await?;
            let hash = trial.store.world().private_state_sha256.clone();
            drop(trial);
            if inspect(path)?.0 != hash {
                return Err("resume drift".into());
            }
            println!("P4_AUTHORITATIVE_SHA256={hash}");
        }
        "inspect" => {
            let path = Path::new(args.get(1).ok_or("missing trial directory")?);
            let (hash, chain, sequence) = inspect(path)?;
            println!("P4_AUTHORITATIVE_SHA256={hash} CHAIN={chain} SEQUENCE={sequence}");
        }
        "benchmark" => {
            let count = args
                .get(1)
                .map_or(Ok(3), |s| s.parse::<usize>().map_err(|e| e.to_string()))?;
            benchmark(count, false).await?;
        }
        "benchmark-stress" => {
            let count = args
                .get(1)
                .map_or(Ok(3), |s| s.parse::<usize>().map_err(|e| e.to_string()))?;
            benchmark(count, true).await?;
        }
        _ => return Err("use run|stress|resume|inspect|benchmark|benchmark-stress".into()),
    }
    Ok(())
}
#[tokio::main]
async fn main() {
    if let Err(e) = entry().await {
        eprintln!("P4 error: {e}");
        std::process::exit(2);
    }
}
