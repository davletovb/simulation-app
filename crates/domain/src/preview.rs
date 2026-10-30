//! Pre-authority one-scope optimistic schedule for P1 offline UI.
//! Not a committed run; no minister vote, funded tranche or shock is inferred.
use crate::{
    Allocation, ContractError, DueQuarter, EffortPermille, FundedScope, PILOT_CREDITS,
    PolicyFamily, Quarter, SIX_QUARTERS, deliver_family_quarter,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyPreview {
    pub family: PolicyFamily,
    pub per_quarter: [u16; SIX_QUARTERS as usize],
    pub outstanding: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preview {
    pub families: [FamilyPreview; 3],
    pub conditional_access_bonus: bool,
}

/// Optimistic capacity only, assuming initial ratification before Q1,
/// full effort, unblocked funds and *conditional* access volunteer support.
/// The bonus is available from Q2 if >=10 access units were initially funded.
/// No budget is committed; use P3 for an authoritative quarter transition.
pub fn optimistic_preview(allocation: Allocation) -> Result<Preview, ContractError> {
    if allocation.total() > u32::from(PILOT_CREDITS) {
        return Err(ContractError::InsufficientAvailableFunds);
    }
    let access_bonus_possible = allocation.access() >= 10;
    let mut result = Vec::with_capacity(3);
    for family in [PolicyFamily::Staffing, PolicyFamily::Repairs, PolicyFamily::Access] {
        let funded = allocation.units(family);
        let mut scopes = if funded == 0 {
            Vec::new()
        } else {
            vec![FundedScope::new(
                "initial".into(), family, Quarter::new(1)?, funded,
            )?]
        };
        let mut per_quarter = [0; SIX_QUARTERS as usize];
        for q in 1..=SIX_QUARTERS {
            let bonus = family == PolicyFamily::Access && access_bonus_possible && q >= 2;
            let receipt = deliver_family_quarter(
                family, family.nominal_capacity(), bonus,
                EffortPermille::new(1000)?, DueQuarter::new(u16::from(q))?, &mut scopes,
            )?;
            per_quarter[usize::from(q - 1)] = receipt.total_delivered;
        }
        let outstanding = scopes.first().map_or(0, FundedScope::remaining_units);
        result.push(FamilyPreview { family, per_quarter, outstanding });
    }
    let families: [FamilyPreview; 3] = result
        .try_into()
        .map_err(|_| ContractError::ArithmeticOverflow)?;
    Ok(Preview { families, conditional_access_bonus: access_bonus_possible })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_declared_lags_and_shared_capacity() {
        let p = optimistic_preview(Allocation::draft(50, 40, 10).unwrap()).unwrap();
        assert_eq!(p.families[0].per_quarter, [0, 12, 12, 12, 12, 2]);
        assert_eq!(p.families[1].per_quarter, [0, 0, 10, 10, 10, 10]);
        assert_eq!(p.families[2].per_quarter, [4, 5, 1, 0, 0, 0]);
        assert!(p.families.iter().all(|x| x.outstanding == 0));
    }

    #[test]
    fn access_bonus_is_attainable_not_guaranteed() {
        let p = optimistic_preview(Allocation::draft(0, 0, 25).unwrap()).unwrap();
        assert_eq!(p.families[2].per_quarter, [4, 5, 5, 5, 5, 1]);
        assert!(p.conditional_access_bonus);
        let low = optimistic_preview(Allocation::draft(0, 0, 9).unwrap()).unwrap();
        assert_eq!(low.families[2].per_quarter, [4, 4, 1, 0, 0, 0]);
        assert!(!low.conditional_access_bonus);
    }

    #[test]
    fn budget_overflow_rejected_before_any_projection() {
        let over = Allocation::draft(60, 40, 10).unwrap();
        assert_eq!(
            optimistic_preview(over),
            Err(ContractError::InsufficientAvailableFunds)
        );
    }

    #[test]
    fn exactly_100_access_funding_can_have_remainder() {
        let p = optimistic_preview(Allocation::draft(0, 0, 100).unwrap()).unwrap();
        assert_eq!(p.families[2].per_quarter, [4, 5, 5, 5, 5, 5]);
        assert_eq!(p.families[2].outstanding, 71);
    }
}
