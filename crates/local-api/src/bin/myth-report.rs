//! P7 local-only private chair inspection. Not an HTTP endpoint.
//! Read a stopped campaign from its exact Hiyal journal; no live backend or API key.
use myth_hiyal_adapter::orchestrator::{inspect, resume};
use serde_json::json;
use std::{fs::OpenOptions, path::Path};

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err("usage: myth-report <private-hiyal-campaign-directory>".into());
    }
    let dir = Path::new(&args[1]);
    let root = dir.parent().ok_or("campaign needs a containing private save root")?;
    let lock = OpenOptions::new().read(true).write(true)
        .open(root.join(".p6-writer.lock"))
        .map_err(|_| "missing P6 writer lock: stop the API and inspect a managed save root")?;
    lock.try_lock().map_err(|_| "local API holds the save writer lock; stop it before running myth-report")?;
    // The OS lock remains held through inspection, replay and report output.
    // Complete read-only integrity verification occurs before any values
    // are printed. Replays use recorded actions/receipts without a provider.
    let (authoritative_hash, _journal_head, cursor) = inspect(dir)?;
    let trial = resume(dir, true)?;
    let world = trial.store.world();
    world.verify()?;
    if world.private_state_sha256 != authoritative_hash {
        return Err("replay/inspection hash mismatch".into());
    }
    let campaign = world.fold()?;
    let treasury = campaign.world().treasury();
    let audits: Vec<_> = campaign
        .player_audits()
        .iter()
        .map(|item| {
            json!({
                "id": item.id,
                "version": item.version.to_decimal(),
                "family": format!("{:?}", item.family),
                "inspected_quarter": item.inspected_quarter.value(),
                "scheduled_cumulative": item.scheduled_cumulative,
                "reported_cumulative": item.reported_cumulative,
                "inspected_cumulative": item.inspected_cumulative,
                "issuance_world_version": item.issuance_world_version.to_decimal(),
                "cost": item.cost,
            })
        })
        .collect();
    let commitments: Vec<_> = campaign
        .world()
        .commitments()
        .iter()
        .map(|item| {
            json!({
                "id": item.id,
                "issuer": format!("{:?}", item.issuer),
                "beneficiary": format!("{:?}", item.beneficiary),
                "kind": format!("{:?}", item.kind),
                "funded_units": item.funded_units,
                "deadline_quarter": item.deadline_quarter.value(),
                "status": format!("{:?}", item.status),
                "resolution_event": item.resolution_event,
            })
        })
        .collect();
    let reports: Vec<_> = campaign
        .player_reports()
        .iter()
        .map(|item| {
            json!({
                "quarter": item.quarter.value(),
                "reported_cumulative": item.reported_cumulative,
                "due_cumulative": item.due_cumulative,
                "weighted_support_ppm": item.weighted_support_ppm,
            })
        })
        .collect();
    let output = json!({
        "schema_version": 1,
        "report_kind": "privileged_operator_debug_verified",
        "campaign_id": world.campaign_id,
        "private_authoritative_sha256": authoritative_hash,
        "cursor": cursor.to_string(),
        "quarter": world.quarter,
        "phase": format!("{:?}", world.phase),
        "treasury": {
            "available_credits": treasury.available(),
            "encumbered_staff": treasury.encumbered(myth_domain::PolicyFamily::Staffing),
            "encumbered_repairs": treasury.encumbered(myth_domain::PolicyFamily::Repairs),
            "encumbered_access": treasury.encumbered(myth_domain::PolicyFamily::Access),
            "service_spent": treasury.service_spent(),
            "audit_spent": treasury.audit_spent(),
        },
        "audits": audits,
        "commitments": commitments,
        "public_quarterly_reports": reports,
        "warning": "PRIVILEGED OPERATOR DEBUG OUTPUT, NOT A CHAIR ROLE VIEW: includes private minister-to-minister commitments. Never publish in browser, SSE or public artefacts. Not evidence of real human behaviour.",
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .map_err(|_| "cannot serialize private local report")?
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Private report unavailable: {error}");
        std::process::exit(1);
    }
}
