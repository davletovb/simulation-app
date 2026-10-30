//! Independent, profile-only, deterministic non-LLM actor selector.
//! Consumes exactly a typed role-visible observation. No hidden world state.
use crate::{
    Allocation, AllocationValidity, ContractError, CounterproposalOrigin, FinalPosition,
    FirstPosition, Ministry, Position, PreferredAllocation, RoleObservation,
    validate_identifier,
};

pub const MENU: [(&str, [u16; 3]); 4] = [
    ("access_first",[40,30,20]),
    ("balanced",[50,40,10]),
    ("oversubscribed",[60,40,10]),
    ("staff_first",[60,30,10]),
];
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptDecision {
    pub first: FirstPosition,
    pub final_position: FinalPosition,
}
fn missed(view: &RoleObservation) -> bool {
    view.owned_commitments.iter().any(|x|
        x.status == crate::CommitmentStatus::Broken
            && (x.issuer == view.ministry || x.beneficiary == view.ministry)
    )
}
fn score(view: &RoleObservation, a: Allocation, bad_promise: bool) -> i64 {
    let s=i64::from(a.staff()); let r=i64::from(a.repairs());
    let ac=i64::from(a.access());
    let reserve=i64::from(100)-s-r-ac;
    let belief=i64::from(view.belief_ppm.value());
    let low=(500_000-belief).max(0)/10_000;
    let high=(belief-500_000).max(0)/20_000;
    let missed_penalty=if bad_promise {20} else {0};
    match view.ministry {
        Ministry::Finance => 3*reserve-s-r-ac-if reserve<10 {low} else {0}-missed_penalty,
        Ministry::Education => 4*s+2*ac-r+if s>=45 {high} else {0}-missed_penalty,
        Ministry::Infrastructure => 4*r+ac-2*s+if r>=35 {high} else {0}-missed_penalty,
    }
}
fn supports(view: &RoleObservation, bad: bool) -> bool {
    let a=view.allocation;
    let believe=view.belief_ppm.value();
    match view.ministry {
        Ministry::Finance => a.total()<=u32::from(view.available_credits)
            && believe>=420_000 && !bad,
        Ministry::Education => u32::from(a.staff()) >=
            45+if bad {5}else{0}+if believe<460_000 {5}else{0},
        Ministry::Infrastructure => u32::from(a.repairs()) >=
            35+if bad {5}else{0}+if believe<460_000 {5}else{0},
    }
}
pub fn decide(view: &RoleObservation) -> Result<ScriptDecision, ContractError> {
    view.validate()?;
    let bad=missed(view);
    let mut ranked: Vec<(i64,&str, Allocation)> = MENU.iter().filter_map(|(id,c)| {
        let a=Allocation::draft(c[0],c[1],c[2]).ok()?;
        if PreferredAllocation::assess(a,view.available_credits).ok()?.validity()
            == AllocationValidity::InvalidDraft { return None; }
        Some((score(view,a,bad),*id,a))
    }).collect();
    ranked.sort_by(|a,b|b.0.cmp(&a.0).then_with(||a.1.cmp(b.1)));
    let (_score, menu_id, preferred)=ranked.first().copied().ok_or(ContractError::InvalidCreditAmount)?;
    validate_identifier(menu_id)?;
    let choice=PreferredAllocation::assess(preferred,view.available_credits)?;
    let first=FirstPosition::new(view.meeting_id.clone(),view.proposal_version,
        view.ministry,if supports(view,bad){Position::Support}else{Position::Oppose},
        choice,CounterproposalOrigin::Menu(menu_id.into()))?;
    let vote=if supports(view,bad){Position::Support}else{Position::Oppose};
    let final_position=FinalPosition::new(view.meeting_id.clone(),view.proposal_version,
        view.ministry,vote,choice)?;
    Ok(ScriptDecision{first,final_position})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeliefPpm, Quarter, RoleObservationInput, WireU64};
    fn view(m:Ministry,a:Allocation,belief:u32)->RoleObservation{
        RoleObservation::new(RoleObservationInput {
            campaign_id:"case".into(),meeting_id:"q1".into(),proposal_version:WireU64::new(1),
            ministry:m,quarter:Quarter::new(1).unwrap(),allocation:a,
            available_credits:100,belief_ppm:BeliefPpm::new(belief).unwrap()
        }).unwrap()
    }
    #[test]
    fn predeclared_three_policy_vote_matrix_and_independent_ranking(){
        let menu=[
            ([50,40,10],[true,true,true]),
            ([60,30,10],[true,true,false]),
            ([40,30,20],[true,false,false]),
        ];
        for (a,expected) in menu {
            for (ix,m) in [Ministry::Finance,Ministry::Education,Ministry::Infrastructure]
                .into_iter().enumerate() {
                let x=view(m,Allocation::draft(a[0],a[1],a[2]).unwrap(),500_000);
                let decision=decide(&x).unwrap();
                assert_eq!(decision.final_position.vote==Position::Support,expected[ix]);
                // Counteroffer preference is not required to echo the player's package.
                assert_eq!(decision.first.proposal_version,x.proposal_version);
            }
        }
    }
    #[test]
    fn audit_belief_and_missed_promise_shift_support_independently_of_offer(){
        let a=Allocation::draft(50,40,10).unwrap();
        let f=view(Ministry::Finance,a,419_999);
        assert_eq!(decide(&f).unwrap().final_position.vote,Position::Oppose);
        let mut e=view(Ministry::Education,Allocation::draft(45,40,10).unwrap(),459_999);
        assert_eq!(decide(&e).unwrap().final_position.vote,Position::Oppose);
        e.owned_commitments.push(crate::Commitment{
            id:"missed".into(),issuer:Ministry::Education,beneficiary:Ministry::Education,
            proposal_version:WireU64::new(1),kind:crate::CommitmentKind::Political,
            funded_units:0,trigger:"staff".into(),deadline_quarter:crate::DueQuarter::new(1).unwrap(),
            status:crate::CommitmentStatus::Broken,resolution_event:Some("deadline".into())
        });
        assert_eq!(decide(&e).unwrap().final_position.vote,Position::Oppose);
        assert!(decide(&e).unwrap().first.preferred_allocation.allocation().total()<=100);
    }
}
