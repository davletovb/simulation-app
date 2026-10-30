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
    /// Derived counterfactual after applying only already queued Q7/Q8
    /// responses; Q6 score remains the authoritative game-horizon score.
    pub settled_weighted_support_ppm: u32,
    pub settled_group_support_ppm: [u32; 6],
    pub pending_cell_delta_ppm: [i64; 6],
    pub pending_response_count: usize,
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
fn minister_round(c: &mut Campaign, meeting: &str) -> Result<(), Rejection> {
    for role in MINISTERS {
        let obs = c.role_observation(role)?;
        let decision = scripted::decide(&obs)?;
        c.apply(Action::First(decision.first))?;
    }
    c.apply(Action::FreezeFinal {
        command_id: format!("{meeting}-freeze"),
    })?;
    for role in MINISTERS {
        let observation = c.role_observation(role)?;
        let decision = scripted::decide(&observation)?;
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
    Ok(())
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
            minister_round(&mut c,"q1")?;
            let proposal_version=c.proposal_version().ok_or(Rejection::InvalidPhase)?;
            match submit(&mut c,"q1-ratify",PlayerCommand::Ratify {
                meeting_id:"q1".into(),proposal_version,
            }) {
                Ok(()) => Admission::Ratified,
                Err(Rejection::InvalidVote) => {
                    submit(&mut c,"q1-defer",PlayerCommand::Defer)?;
                    Admission::RejectedByVotes
                }
                Err(other)=>return Err(other),
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
    let (settled,pending_count)=c.settle_pending_support()?;
    let mut pending_delta=[0_i64;6];
    let mut weighted_settled=0_u128;
    for ix in 0..6 {
        pending_delta[ix]=i64::from(settled[ix])-i64::from(supports[ix]);
        weighted_settled+=u128::from(settled[ix])*u128::from(weights[ix]);
    }
    let settled_weighted_support_ppm=u32::try_from(weighted_settled/1_000_000)
        .map_err(|_|Rejection::Arithmetic)?;
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
        settled_weighted_support_ppm,
        settled_group_support_ppm: settled,
        pending_cell_delta_ppm: pending_delta,
        pending_response_count: pending_count,
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
/// One pairing implementation shared by aggregate and all six-cell reports.
/// Infeasible or vote-rejected rows are kept in the raw trial table but never
/// treated as an eligible policy-ranking comparison.
pub fn accepted_pairs(outcomes:&[Trial])->Vec<(&Trial,&Trial)> {
    outcomes.iter().filter(|x|x.package=="balanced"
        && x.admission==Admission::Ratified)
        .filter_map(|a| {
            outcomes.iter().find(|b|b.package=="staff_first"
                && b.admission==Admission::Ratified
                && b.table==a.table
                && b.coefficient_percent==a.coefficient_percent
                && b.extra_delivery_lag==a.extra_delivery_lag
                && b.response_lag==a.response_lag)
                .map(|b|(a,b))
        }).collect()
}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub struct PairStats {
    pub eligible:usize,
    pub q6_ties:usize,
    pub q6_differences:usize,
    pub settled_ties:usize,
    pub settled_differences:usize,
    /// Only strict sign comparisons with a nonzero pinned reference qualify.
    pub settled_reversals:usize,
    pub reference_settled_tied:bool,
}
pub fn contrast(outcomes:&[Trial])->PairStats {
    let pairs=accepted_pairs(outcomes);
    let reference=pairs.iter().find(|(a,_)|
        a.table=="authored_baseline" && a.coefficient_percent==100
        && a.extra_delivery_lag==0 && a.response_lag==1);
    let reference_delta=reference.map_or(0_i64,|(a,b)|
        i64::from(a.settled_weighted_support_ppm)
        -i64::from(b.settled_weighted_support_ppm));
    let mut result=PairStats{
        eligible:pairs.len(),q6_ties:0,q6_differences:0,
        settled_ties:0,settled_differences:0,settled_reversals:0,
        reference_settled_tied:reference_delta==0,
    };
    for (a,b) in pairs {
        if a.weighted_support_ppm==b.weighted_support_ppm {
            result.q6_ties+=1;
        } else {result.q6_differences+=1;}
        let difference=i64::from(a.settled_weighted_support_ppm)
            -i64::from(b.settled_weighted_support_ppm);
        if difference==0 { result.settled_ties+=1; }
        else {
            result.settled_differences+=1;
            if reference_delta!=0 && difference.signum()!=reference_delta.signum() {
                result.settled_reversals+=1;
            }
        }
    }
    result
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
        let stats=contrast(&all);
        assert_eq!(stats.eligible,36);
        assert_eq!(stats.q6_ties,36);
        assert_eq!(stats.q6_differences,0);
        assert_eq!(stats.q6_ties+stats.q6_differences,stats.eligible);
        assert_eq!(stats.settled_ties+stats.settled_differences,stats.eligible);
        for row in &all {
            assert!(row.pending_response_count<=2);
            for i in 0..6 {
                assert_eq!(i64::from(row.settled_group_support_ppm[i])
                    -i64::from(row.group_support_ppm[i]),row.pending_cell_delta_ppm[i]);
            }
        }
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
