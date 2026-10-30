//! Reproducible offline Experiment B authoring report. No LLM or game I/O.
use myth_domain::sensitivity::{Admission, COHORT_TABLES, Trial, accepted_pairs, contrast, sweep};
use std::fs;

fn csv_row(t: &Trial) -> String {
    let kind = match t.admission {
        Admission::Ratified => "ratified",
        Admission::RejectedByVotes => "vote_rejected",
        Admission::InfeasibleTreasury => "unaffordable",
    };
    let mut values = vec![
        t.package.to_string(),
        t.table.to_string(),
        t.coefficient_percent.to_string(),
        t.extra_delivery_lag.to_string(),
        t.response_lag.to_string(),
        kind.to_string(),
    ];
    values.extend(t.delivered.iter().map(ToString::to_string));
    values.extend(t.encumbered.iter().map(ToString::to_string));
    values.extend(
        [t.service_spent, t.audit_spent]
            .iter()
            .map(ToString::to_string),
    );
    values.push(t.weighted_support_ppm.to_string());
    values.extend(t.group_support_ppm.iter().map(ToString::to_string));
    values.extend(t.relationships.iter().map(ToString::to_string));
    values.push(t.pending_response_count.to_string());
    values.push(t.settled_weighted_support_ppm.to_string());
    values.extend(t.settled_group_support_ppm.iter().map(ToString::to_string));
    values.extend(t.pending_cell_delta_ppm.iter().map(ToString::to_string));
    values.join(",") + "\n"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trials = sweep().map_err(|e| format!("fixed-action trial rejected: {e:?}"))?;
    let accepted = trials
        .iter()
        .filter(|t| t.admission == Admission::Ratified)
        .count();
    let rejected = trials
        .iter()
        .filter(|t| t.admission == Admission::RejectedByVotes)
        .count();
    let unaffordable = trials
        .iter()
        .filter(|t| t.admission == Admission::InfeasibleTreasury)
        .count();
    assert_eq!(
        (trials.len(), accepted, rejected, unaffordable),
        (144, 72, 36, 36)
    );
    let stats = contrast(&trials);
    let pairs = accepted_pairs(&trials);
    assert_eq!(pairs.len(), stats.eligible);
    let headers = "package,table,coefficient_percent,extra_delivery_lag,response_lag,admission,staff_delivered,repair_delivered,access_delivered,staff_encumbered,repair_encumbered,access_encumbered,service_spent,audit_spent,weighted_support_ppm,urban_low,urban_middle,urban_high,rural_low,rural_middle,rural_high,relationship_finance,relationship_education,relationship_infrastructure,pending_response_count,settled_weighted_support_ppm,settled_urban_low,settled_urban_middle,settled_urban_high,settled_rural_low,settled_rural_middle,settled_rural_high,pending_delta_urban_low,pending_delta_urban_middle,pending_delta_urban_high,pending_delta_rural_low,pending_delta_rural_middle,pending_delta_rural_high\n";
    let mut csv = headers.to_string();
    for trial in &trials {
        csv.push_str(&csv_row(trial));
    }
    let min_q6 = trials
        .iter()
        .filter(|t| t.admission == Admission::Ratified)
        .map(|t| t.weighted_support_ppm)
        .min()
        .ok_or("no accepted cases")?;
    let max_q6 = trials
        .iter()
        .filter(|t| t.admission == Admission::Ratified)
        .map(|t| t.weighted_support_ppm)
        .max()
        .ok_or("no accepted cases")?;
    let min_settled = trials
        .iter()
        .filter(|t| t.admission == Admission::Ratified)
        .map(|t| t.settled_weighted_support_ppm)
        .min()
        .ok_or("no accepted cases")?;
    let max_settled = trials
        .iter()
        .filter(|t| t.admission == Admission::Ratified)
        .map(|t| t.settled_weighted_support_ppm)
        .max()
        .ok_or("no accepted cases")?;
    let mut report =
        String::from("# P3-07 fixed-action sensitivity — corrected executed evidence\n\n");
    report.push_str("**Scope:** an authored fictional school-service simulation, not empirical English demographics, behavioral calibration or a causal forecast. All cohort tables and multipliers are authored hypotheses.\n\n");
    report.push_str("## Frozen comparison protocol\n\n");
    report.push_str("- Four Q1 packages: balanced [50,40,10], staff_first [60,30,10], access_first [40,30,20], and oversubscribed [60,40,10]; three P2-authored cohort tables; coefficient scale 50/100/150%; added delivery lag 0/1; response lag 1/2. This is **144 trials**, including every failed adoption. Same scripted actor profile, no policy re-optimization, full effort and exactly six explicitly advanced quarters.\n");
    report.push_str("- The Q2 authored repair-cap patch of 8 is pinned unchanged to P2 and all alternatives, but is **inert**: Q1 repairs first become eligible in Q3, so Q2's cap cannot restrict delivery. Do not interpret it as an active shock. The remaining shocks are an explicit all-zero common schedule.\n");
    report.push_str("- Ratification is determined by the authoritative reducer, not a duplicate vote-counting algorithm. All comparison pairs are defined once by `accepted_pairs`, excluding rejected or unaffordable policy rows.\n");
    report.push_str("- Two endpoints are shown separately: **Q6 authoritative support** (does not include events due after the horizon) and **queue-settled support** (applies only the already produced Q7/Q8 queued responses, in stable due-date/event-ID order, with no invented Q7/Q8 spending, deliveries or shocks). Queue-settled is a derived diagnostic, **not** a changed Q6 score or a prediction of future action.\n\n");
    report.push_str("## Counts and identifiable comparisons\n\n");
    report.push_str(&format!("| Metric | Result |\n|---|---:|\n| Declared variants | {} |\n| Ratified | {} |\n| Rejected by votes | {} |\n| Unaffordable | {} |\n| Eligible balanced/staff-first pairs | {} |\n| **Q6 weighted-support ties** | {} |\n| Q6 non-ties | {} |\n| Queue-settled ties | {} |\n| Queue-settled non-ties | {} |\n| Queue-settled reversals versus non-tied reference | {} |\n| Reference itself tied after settlement? | {} |\n| Accepted Q6 support range (ppm) | {}–{} |\n| Accepted queue-settled support range (ppm) | {}–{} |\n\n",
        trials.len(),accepted,rejected,unaffordable,stats.eligible,stats.q6_ties,
        stats.q6_differences,stats.settled_ties,stats.settled_differences,
        stats.settled_reversals,stats.reference_settled_tied,min_q6,max_q6,
        min_settled,max_settled));
    report.push_str("**Interpretation:** identical Q6 support in all 36 pairs is a **tie**, not evidence of ranking stability or zero reversals. Q6 deliveries can differ while the corresponding response is still pending. A reversal is defined only when the pinned authored_baseline/100%/original-lag/one-quarter-response **queue-settled** difference is strictly nonzero and the compared queue-settled difference has the opposite strict sign; tied comparisons are reported separately, never scored as stable ranks.\n\n");
    report.push_str("## Distributional effects and comparison identifiability\n\n");
    report.push_str("| Fictional cell | Q6 min ppm | Q6 max ppm | Accepted Q6 below 500k | Accepted Q6 above 500k | Q6 paired ties | Settled paired ties | Settled reversals* |\n|---|---:|---:|---:|---:|---:|---:|---:|\n");
    for (ix, name) in [
        "urban-low",
        "urban-middle",
        "urban-high",
        "rural-low",
        "rural-middle",
        "rural-high",
    ]
    .iter()
    .enumerate()
    {
        let cells: Vec<_> = trials
            .iter()
            .filter(|t| t.admission == Admission::Ratified)
            .map(|t| t.group_support_ppm[ix])
            .collect();
        let min = cells.iter().min().ok_or("empty accepted table")?;
        let max = cells.iter().max().ok_or("empty accepted table")?;
        let below = cells.iter().filter(|x| **x < 500_000).count();
        let above = cells.iter().filter(|x| **x > 500_000).count();
        let reference = pairs
            .iter()
            .find(|(a, _)| {
                a.table == "authored_baseline"
                    && a.coefficient_percent == 100
                    && a.extra_delivery_lag == 0
                    && a.response_lag == 1
            })
            .ok_or("missing pinned pair")?;
        let original_delta = i64::from(reference.0.settled_group_support_ppm[ix])
            - i64::from(reference.1.settled_group_support_ppm[ix]);
        let q6_ties = pairs
            .iter()
            .filter(|(a, b)| a.group_support_ppm[ix] == b.group_support_ppm[ix])
            .count();
        let settled_ties = pairs
            .iter()
            .filter(|(a, b)| a.settled_group_support_ppm[ix] == b.settled_group_support_ppm[ix])
            .count();
        let reversals = pairs
            .iter()
            .filter(|(a, b)| {
                let change = i64::from(a.settled_group_support_ppm[ix])
                    - i64::from(b.settled_group_support_ppm[ix]);
                original_delta != 0 && change != 0 && original_delta.signum() != change.signum()
            })
            .count();
        report.push_str(&format!("| {name} | {min} | {max} | {below} | {above} | {q6_ties} | {settled_ties} | {reversals} |\n"));
    }
    report.push_str("\n*When a pinned reference is tied, reversals are **not identifiable**; the correct diagnostic is the tie count, not a claim of stability. Full per-cell pending deltas and settled support appear in the CSV next to the untouched Q6 values.\n\n");
    report.push_str("## Attribution limits\n\n");
    report.push_str("Response-lag comparisons at Q6 change the **measurement cutoff** as well as the timing: Q6 credits may be spent while their social responses remain queued. Queue-settled comparisons isolate the pending event effect under the same zero-future-action projection; differing timing before Q6 can still affect the path if clamps or other endogenous conditions bind. This authored experiment cannot estimate real beneficiaries' response lags or compare untested counterfactual actions.\n\n");
    report.push_str("## P2-authored population sensitivity\n\n| Table | Urban low/middle/high | Rural low/middle/high |\n|---|---|---|\n");
    for (name, w) in COHORT_TABLES {
        report.push_str(&format!(
            "| {name} | {} / {} / {} | {} / {} / {} |\n",
            w[0], w[1], w[2], w[3], w[4], w[5]
        ));
    }
    report.push_str("\nEvery table has authored urban/rural 800k/200k and income 330k/400k/270k parts-per-million margins, not observed England income–residence data. No real-world robustness claim follows.\n");
    fs::write(
        "data/england-school-pilot-v1/p3-sensitivity-results.csv",
        csv,
    )?;
    fs::write("docs/p3-sensitivity-report.md", report)?;
    Ok(())
}
