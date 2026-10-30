//! Experiment B: fixed-action offline sensitivity; no policy re-optimization.
//! Inputs and outcomes are authored-model scenarios, NOT England forecasts.
use crate::reducer::{Action, Campaign, Rejection, Rules};
use crate::scripted;
use crate::world::MeetingPhase;
use crate::{
    Allocation, CohortWeights, CommandEnvelope, EffortPermille, Ministry, PlayerCommand,
    PolicyFamily, PrivateEffortDeclaration,
};

pub const COHORT_TABLES: [(&str, [u32; 6]); 3] = [
    (
        "authored_baseline",
        [260_000, 320_000, 220_000, 70_000, 80_000, 50_000],
    ),
    (
        "low_rural_association",
        [250_000, 340_000, 210_000, 80_000, 60_000, 60_000],
    ),
    (
        "strong_rural_middle_association",
        [290_000, 280_000, 230_000, 40_000, 120_000, 40_000],
    ),
];
pub const PACKAGES: [(&str, [u16; 3]); 4] = [
    ("balanced", [50, 40, 10]),
    ("staff_first", [60, 30, 10]),
    ("access_first", [40, 30, 20]),
    ("oversubscribed", [60, 40, 10]),
];
const MINISTERS: [Ministry; 3] = [
    Ministry::Finance,
    Ministry::Education,
    Ministry::Infrastructure,
];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    Ratified,
    RejectedByVotes,
    InfeasibleTreasury,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trial {
    pub package: &'static str,
    pub table: &'static str,
    pub coefficient_percent: u16,
    pub extra_delivery_lag: u16,
    pub response_lag: u16,
    pub admission: Admission,
    pub delivered: [u16; 3],
    pub encumbered: [u16; 3],
    pub service_spent: u16,
    pub audit_spent: u16,
    pub weighted_support_ppm: u32,
    pub group_support_ppm: [u32; 6],
    pub relationships: [i16; 3],
}
fn submit(c: &mut Campaign, id: &str, action: PlayerCommand) -> Result<(), Rejection> {
    let envelope = CommandEnvelope::new(
        "p3-sweep".into(),
        id.into(),
        c.world().world_version(),
        action,
    )?;
    c.apply(Action::Player(envelope))?;
    Ok(())
}
fn minister_round(c: &mut Campaign, meeting: &str) -> Result<usize, Rejection> {
    for role in MINISTERS {
        let obs = c.role_observation(role)?;
        let decision = scripted::decide(&obs)?;
        c.apply(Action::First(decision.first))?;
    }
    c.apply(Action::FreezeFinal {
        command_id: format!("{meeting}-freeze"),
    })?;
    let mut support = 0_usize;
    for role in MINISTERS {
        let observation = c.role_observation(role)?;
        let decision = scripted::decide(&observation)?;
        if decision.final_position.vote == crate::Position::Support {
            support += 1;
        }
        let effort = if role == Ministry::Finance {
            None
        } else {
            Some(PrivateEffortDeclaration::new(
                meeting.into(),
                role,
                EffortPermille::new(1000)?,
            )?)
        };
        c.apply(Action::Final(decision.final_position, effort))?;
    }
    Ok(support)
}
/// All variants receive the SAME Q1 proposal, scripted first/final vote law,
/// 1000‰ efforts and six explicit advances, with all-zero exogenous shocks.
/// No post-treatment minister action is re-optimized as social support changes.
pub fn run_one(
    name: &'static str,
    credits: [u16; 3],
    table: &'static str,
    weights: [u32; 6],
    magnitude: u16,
    extra_lag: u16,
    response_lag: u16,
) -> Result<Trial, Rejection> {
    let mut c = Campaign::with_rules(
        "p3-sweep",
        "england-school-delivery-v1",
        CohortWeights::new(weights)?,
        Rules {
            magnitude_percent: magnitude,
            delivery_extra_lag: extra_lag,
            response_lag,
            shocks: [[0; 6]; 6],
            q2_repair_cap: Some(8),
        },
    )?;
    let allocation = Allocation::draft(credits[0], credits[1], credits[2])?;
    let admitted = match submit(
        &mut c,
        "q1-proposal",
        PlayerCommand::Propose {
            meeting_id: "q1".into(),
            allocation,
        },
    ) {
        Err(Rejection::InsufficientFunding) => Admission::InfeasibleTreasury,
        Err(other) => return Err(other),
        Ok(()) => {
            let support = minister_round(&mut c, "q1")?;
            if support >= 2 {
                let proposal_version = c.proposal_version().ok_or(Rejection::InvalidPhase)?;
                submit(
                    &mut c,
                    "q1-ratify",
                    PlayerCommand::Ratify {
                        meeting_id: "q1".into(),
                        proposal_version,
                    },
                )?;
                Admission::Ratified
            } else {
                submit(&mut c, "q1-defer", PlayerCommand::Defer)?;
                Admission::RejectedByVotes
            }
        }
    };
    for q in 1..=6 {
        submit(
            &mut c,
            &format!("advance-{q}"),
            PlayerCommand::AdvanceQuarter,
        )?;
        let report = c.reports().last().ok_or(Rejection::InvalidPhase)?;
        for (ix, family) in [
            PolicyFamily::Staffing,
            PolicyFamily::Repairs,
            PolicyFamily::Access,
        ]
        .iter()
        .enumerate()
        {
            let limit = family.nominal_capacity() + u16::from(*family == PolicyFamily::Access);
            if report.delivered[ix] > limit {
                return Err(Rejection::Arithmetic);
            }
        }
    }
    if c.phase() != MeetingPhase::QuarterClosed {
        return Err(Rejection::InvalidPhase);
    }
    let world = c.world();
    let treasury = world.treasury();
    treasury.validate()?;
    let mut supports = [0_u32; 6];
    for (i, slot) in supports.iter_mut().enumerate() {
        *slot = world.support()[i].value();
    }
    let weighted = c
        .reports()
        .last()
        .ok_or(Rejection::InvalidPhase)?
        .weighted_support_ppm;
    Ok(Trial {
        package: name,
        table,
        coefficient_percent: magnitude,
        extra_delivery_lag: extra_lag,
        response_lag,
        admission: admitted,
        delivered: [
            c.reports()
                .iter()
                .map(|x| u32::from(x.delivered[0]))
                .sum::<u32>() as u16,
            c.reports()
                .iter()
                .map(|x| u32::from(x.delivered[1]))
                .sum::<u32>() as u16,
            c.reports()
                .iter()
                .map(|x| u32::from(x.delivered[2]))
                .sum::<u32>() as u16,
        ],
        encumbered: [
            treasury.encumbered(PolicyFamily::Staffing),
            treasury.encumbered(PolicyFamily::Repairs),
            treasury.encumbered(PolicyFamily::Access),
        ],
        service_spent: treasury.service_spent(),
        audit_spent: treasury.audit_spent(),
        weighted_support_ppm: weighted,
        group_support_ppm: supports,
        relationships: c.relationship(),
    })
}
pub fn sweep() -> Result<Vec<Trial>, Rejection> {
    let mut outcomes = Vec::new();
    for (package, credits) in PACKAGES {
        for (table, weights) in COHORT_TABLES {
            for magnitude in [50, 100, 150] {
                for extra_lag in [0, 1] {
                    for response_lag in [1, 2] {
                        outcomes.push(run_one(
                            package,
                            credits,
                            table,
                            weights,
                            magnitude,
                            extra_lag,
                            response_lag,
                        )?);
                    }
                }
            }
        }
    }
    Ok(outcomes)
}
/// Comparing like-for-like policy packages; infeasible/failed votes are retained.
pub fn contrast(outcomes: &[Trial]) -> (usize, usize) {
    let mut ranked = 0_usize;
    let mut reversed = 0_usize;
    for baseline in outcomes.iter().filter(|x| x.package == "balanced") {
        if let Some(comparator) = outcomes.iter().find(|x| {
            x.package == "staff_first"
                && x.table == baseline.table
                && x.coefficient_percent == baseline.coefficient_percent
                && x.extra_delivery_lag == baseline.extra_delivery_lag
                && x.response_lag == baseline.response_lag
        }) {
            if baseline.admission != Admission::Ratified
                || comparator.admission != Admission::Ratified
            {
                continue;
            }
            {
                ranked += 1;
                // Reference ranking comes from the baseline coefficient/lag/table case.
                let reference = outcomes.iter().find(|x| {
                    x.package == "balanced"
                        && x.table == "authored_baseline"
                        && x.coefficient_percent == 100
                        && x.extra_delivery_lag == 0
                        && x.response_lag == 1
                });
                let reference_other = outcomes.iter().find(|x| {
                    x.package == "staff_first"
                        && x.table == "authored_baseline"
                        && x.coefficient_percent == 100
                        && x.extra_delivery_lag == 0
                        && x.response_lag == 1
                });
                if let (Some(a), Some(b)) = (reference, reference_other) {
                    let baseline_delta =
                        i64::from(a.weighted_support_ppm) - i64::from(b.weighted_support_ppm);
                    let tested_delta = i64::from(baseline.weighted_support_ppm)
                        - i64::from(comparator.weighted_support_ppm);
                    if baseline_delta.signum() != 0
                        && tested_delta.signum() != 0
                        && baseline_delta.signum() != tested_delta.signum()
                    {
                        reversed += 1;
                    }
                }
            }
        }
    }
    (ranked, reversed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_grid_preserves_fixed_action_feasibility_and_accounting() {
        let all = sweep().unwrap();
        assert_eq!(all.len(), 4 * 3 * 3 * 2 * 2);
        assert_eq!(
            all.iter()
                .filter(|x| x.admission == Admission::Ratified)
                .count(),
            72
        );
        assert_eq!(
            all.iter()
                .filter(|x| x.admission == Admission::RejectedByVotes)
                .count(),
            36
        );
        assert_eq!(
            all.iter()
                .filter(|x| x.admission == Admission::InfeasibleTreasury)
                .count(),
            36
        );
        for x in &all {
            let stock = u32::from(x.service_spent)
                + u32::from(x.audit_spent)
                + x.encumbered.iter().map(|v| u32::from(*v)).sum::<u32>();
            assert!(stock <= 100);
            assert_eq!(
                x.delivered.iter().map(|v| u32::from(*v)).sum::<u32>(),
                u32::from(x.service_spent)
            );
            assert!(x.group_support_ppm.iter().all(|v| *v <= 1_000_000));
        }
        assert_eq!(contrast(&all).0, 36);
    }
    #[test]
    fn exact_population_variants_are_pinned_to_frozen_p2_authored_fixture() {
        let raw = include_str!("../../../data/england-school-pilot-v1/authored.json");
        let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        for (id, weights) in COHORT_TABLES {
            let cells = weights
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let needle = format!(r#""id":"{id}","weights_ppm":[{cells}]"#);
            assert!(
                compact.contains(&needle),
                "P2 population fixture drift for {id}"
            );
        }
    }
    #[test]
    fn deterministic_and_population_margins_are_pinned() {
        let a = sweep().unwrap();
        assert_eq!(a, sweep().unwrap());
        for (_, w) in COHORT_TABLES {
            assert_eq!(w.iter().sum::<u32>(), 1_000_000);
            assert_eq!(w[..3].iter().sum::<u32>(), 800_000);
            assert_eq!(w[3..].iter().sum::<u32>(), 200_000);
            for i in 0..3 {
                assert_eq!(w[i] + w[i + 3], [330_000, 400_000, 270_000][i]);
            }
        }
    }
}
