//! Authoring-only report generator; deterministic Rust game reducer is pure.
use myth_domain::sensitivity::{Admission, COHORT_TABLES, contrast, sweep};
use std::fs;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let trials = sweep().map_err(|x| format!("sweep rejected a declared variant: {x:?}"))?;
    let mut csv = String::from(
        "package,table,coefficient_percent,extra_delivery_lag,response_lag,admission,staff_delivered,repair_delivered,access_delivered,staff_encumbered,repair_encumbered,access_encumbered,service_spent,audit_spent,weighted_support_ppm,urban_low,urban_middle,urban_high,rural_low,rural_middle,rural_high,relationship_finance,relationship_education,relationship_infrastructure\n",
    );
    let mut accepted = 0;
    let mut vote_fail = 0;
    let mut unaffordable = 0;
    let mut high = u32::MIN;
    let mut low = u32::MAX;
    for t in &trials {
        let kind = match t.admission {
            Admission::Ratified => {
                accepted += 1;
                "ratified"
            }
            Admission::RejectedByVotes => {
                vote_fail += 1;
                "vote_rejected"
            }
            Admission::InfeasibleTreasury => {
                unaffordable += 1;
                "unaffordable"
            }
        };
        let cells = t
            .group_support_ppm
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            t.package,
            t.table,
            t.coefficient_percent,
            t.extra_delivery_lag,
            t.response_lag,
            kind,
            t.delivered[0],
            t.delivered[1],
            t.delivered[2],
            t.encumbered[0],
            t.encumbered[1],
            t.encumbered[2],
            t.service_spent,
            t.audit_spent,
            t.weighted_support_ppm,
            cells,
            t.relationships[0],
            t.relationships[1],
            t.relationships[2]
        ));
        if kind == "ratified" {
            high = high.max(t.weighted_support_ppm);
            low = low.min(t.weighted_support_ppm);
        }
    }
    assert_eq!((accepted, vote_fail, unaffordable), (72, 36, 36));
    let (pairs, reversals) = contrast(&trials);
    let mut report =
        String::from("# P3-07 offline fixed-action sensitivity — executed evidence\n\n");
    report.push_str("**Scope:** synthetic England school-service beneficiary game only. Not English population calibration, real minister predictions, or an estimate of policy efficacy. All three joint tables and residence/income margins are **authored**; the original official DfE headline records do not identify them.\n\n");
    report.push_str("## Registration and frozen comparisons\n\n");
    report.push_str("- Source inputs: P2 `england-school-delivery-v1`, three authored joint tables and case configuration; Rust rules: `myth-domain::reducer` and `myth-domain::sensitivity`.\n");
    report.push_str("- Policy order: balanced [50,40,10], staff_first [60,30,10], access_first [40,30,20], oversubscribed [60,40,10]. One Q1 proposal, all 3 first positions, all 3 final positions with fixed full effort (1000 permille), one Q1 ratification when votes admit, then exactly six explicit advances. No subsequent actor re-optimization.\n");
    report.push_str("- Grid: 3 magnitude multipliers (50/100/150%), extra delivery lag (0/1 quarter), response lag (1/2 quarters), three P2 authored population variants, four packages. Total 144 frozen runs, with identical all-zero authored exogenous shocks and Q2 fictional repairs-cap overlay at 8; this is a fixed neutral shock schedule, not a claim shocks never happen.\n");
    report.push_str("- Every variant is retained even if not ratified; shared oldest-funded-first capacity is used in every accepted run. The independent profile-only selector is used on the same permitted role view. No API/model calls.\n\n");
    report.push_str("## Outcome counts and ranking check\n\n");
    report.push_str(&format!("| Metric | Result |\n|---|---:|\n| Declared trials | {} |\n| Ratified | {} |\n| Vote-rejected (access_first) | {} |\n| Treasury-rejected (oversubscribed) | {} |\n| Valid balanced-versus-staff_first paired comparisons | {} |\n| Weighted-support rank reversals versus reference | {} |\n| Minimum accepted weighted support (ppm) | {} |\n| Maximum accepted weighted support (ppm) | {} |\n\n",
        trials.len(),accepted,vote_fail,unaffordable,pairs,reversals,low,high));
    report.push_str("The reference ranking is the authored_baseline population, 100% response coefficients, original delivery lags and one-quarter response lag. A reversal is defined as a strict sign change in **balanced weighted support minus staff_first weighted support**; ties are not counted as reversals. Other metrics (delivered units, outstanding encumbrances, six cell-level support values and minister relationship scores) are recorded for every variant in [the complete results table](../data/england-school-pilot-v1/p3-sensitivity-results.csv), not silently omitted.\n\n");
    report.push_str("## Distributional gains, losses and rank sensitivity\n\n");
    report.push_str("| Synthetic beneficiary cell | Min support ppm | Max support ppm | Accepted runs below 500,000 | Accepted runs above 500,000 | Package rank reversals vs reference |\n|---|---:|---:|---:|---:|---:|\n");
    let cell_names=["urban-low","urban-middle","urban-high","rural-low","rural-middle","rural-high"];
    for (ix,cell) in cell_names.iter().enumerate() {
        let accepted_cells:Vec<_>=trials.iter().filter(|x|x.admission==Admission::Ratified)
            .map(|x|x.group_support_ppm[ix]).collect();
        let min=*accepted_cells.iter().min().expect("some accepted policy");
        let max=*accepted_cells.iter().max().expect("some accepted policy");
        let below=accepted_cells.iter().filter(|v|**v<500_000).count();
        let above=accepted_cells.iter().filter(|v|**v>500_000).count();
        let reference_a=trials.iter().find(|x| x.package=="balanced" && x.table=="authored_baseline"
            && x.coefficient_percent==100 && x.extra_delivery_lag==0 && x.response_lag==1)
            .expect("pinned reference");
        let reference_b=trials.iter().find(|x| x.package=="staff_first" && x.table=="authored_baseline"
            && x.coefficient_percent==100 && x.extra_delivery_lag==0 && x.response_lag==1)
            .expect("pinned reference");
        let reference_delta=i64::from(reference_a.group_support_ppm[ix])-i64::from(reference_b.group_support_ppm[ix]);
        let mut reversals=0_usize;
        for a in trials.iter().filter(|x|x.package=="balanced") {
            let b=trials.iter().find(|x|x.package=="staff_first" && x.table==a.table
                && x.coefficient_percent==a.coefficient_percent
                && x.extra_delivery_lag==a.extra_delivery_lag
                && x.response_lag==a.response_lag).expect("complete paired trial");
            let tested=i64::from(a.group_support_ppm[ix])-i64::from(b.group_support_ppm[ix]);
            if reference_delta.signum()!=0 && tested.signum()!=0
                && reference_delta.signum()!=tested.signum() {reversals+=1;}
        }
        report.push_str(&format!("| {cell} | {min} | {max} | {below} | {above} | {reversals} |\\n"));
    }
    report.push_str("\\nThese are *simulated cell support changes*, not observed gains/losses among England residents. The baseline of 500,000 ppm is authored, and ties are excluded from reversal counts.\\n\\n");
    report.push_str("## Empirical and identification limits\n\n");
    report.push_str("The sweep is numerical sensitivity within an authored deterministic mechanism, not causal identification or validation of any real England policy. No observed school-beneficiary joint income-by-residence distribution or response elasticities are available. The program changes **only** the declared model knobs and weights, not actor choices or shocks. Policy rankings outside these magnitudes/delays/tables remain unexamined. Infeasible and vote-rejected proposals have no policy spending; social responses remain bounded. The all-zero shock schedule is an explicit controlled experiment assumption.\n");
    report.push_str("\n## Populations held fixed or varied\n\n| Authored table | Urban low/middle/high | Rural low/middle/high |\n|---|---|---|\n");
    for (name, w) in COHORT_TABLES {
        report.push_str(&format!(
            "| {} | {} / {} / {} | {} / {} / {} |\n",
            name, w[0], w[1], w[2], w[3], w[4], w[5]
        ));
    }
    report.push_str("\nAll tables retain the **authored** 800k/200k urban/rural and 330k/400k/270k low/middle/high margins and exactly 1,000,000 ppm. They are not verified DfE demographic margins.\n");
    fs::write(
        "data/england-school-pilot-v1/p3-sensitivity-results.csv",
        csv,
    )?;
    fs::write("docs/p3-sensitivity-report.md", report)?;
    Ok(())
}
