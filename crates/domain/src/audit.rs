//! Authored, game-signed (not real-world certified) delivery audits and
//! bounded persuasion rules. No text generation, network, or floating point.
use crate::{ContractError, Ministry, PolicyFamily, Quarter, WireU64, validate_identifier};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryAudit {
    pub id: String,
    pub version: WireU64,
    pub family: PolicyFamily,
    pub inspected_quarter: Quarter,
    pub scheduled_cumulative: u16,
    pub reported_cumulative: u16,
    pub inspected_cumulative: u16,
    pub issuance_world_version: WireU64,
    pub cost: u16,
}
impl DeliveryAudit {
    pub(crate) fn validate(&self) -> Result<(), ContractError> {
        validate_identifier(&self.id)?;
        if self.version.value() == 0 || self.cost > 2 {
            return Err(ContractError::InvalidCondition);
        }
        Ok(())
    }
    pub fn requested_delta(&self) -> i32 {
        if self.scheduled_cumulative == 0 {
            return 0;
        }
        let missed = u32::from(
            self.scheduled_cumulative
                .saturating_sub(self.inspected_cumulative),
        ) * 1000
            / u32::from(self.scheduled_cumulative.max(1));
        let over = u32::from(
            self.reported_cumulative
                .saturating_sub(self.inspected_cumulative),
        ) * 1000
            / u32::from(self.reported_cumulative.max(1));
        if missed >= 200 || over > 200 {
            -80_000
        } else if missed >= 50 || over > 50 {
            -20_000
        } else if self.inspected_cumulative >= self.scheduled_cumulative
            && self.reported_cumulative == self.inspected_cumulative
        {
            40_000
        } else {
            0
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeliefDisposition {
    Applied,
    Irrelevant,
    Duplicate,
    SupersedingWithoutReplay,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeliefReceipt {
    pub recipient: Ministry,
    pub evidence_id: String,
    pub before: u32,
    pub after: u32,
    pub delta: i32,
    pub variation_left: u32,
    pub disposition: BeliefDisposition,
}
#[derive(Clone)]
pub struct BeliefLedger {
    beliefs: [u32; 3],
    variation: [u32; 3],
    targets: BTreeMap<(Ministry, PolicyFamily, Quarter), (String, u64)>,
}
impl Default for BeliefLedger {
    fn default() -> Self {
        Self {
            beliefs: [500_000; 3],
            variation: [0; 3],
            targets: BTreeMap::new(),
        }
    }
}
fn role_index(m: Ministry) -> usize {
    match m {
        Ministry::Finance => 0,
        Ministry::Education => 1,
        Ministry::Infrastructure => 2,
    }
}
impl BeliefLedger {
    pub fn belief(&self, ministry: Ministry) -> u32 {
        self.beliefs[role_index(ministry)]
    }
    pub fn variation(&self, ministry: Ministry) -> u32 {
        self.variation[role_index(ministry)]
    }
    /// Relevance is an authored functional responsibility, not an LLM opinion.
    /// Access evidence is visible to its recipients but never shifts beliefs.
    fn relevant(e: &DeliveryAudit, ministry: Ministry) -> bool {
        match (ministry, e.family) {
            (_, PolicyFamily::Access) => false,
            (Ministry::Finance, _) => true,
            (Ministry::Education, PolicyFamily::Staffing) => true,
            (Ministry::Infrastructure, PolicyFamily::Repairs) => true,
            _ => false,
        }
    }
    pub(crate) fn observe(
        &mut self,
        audit: &DeliveryAudit,
        recipient: Ministry,
    ) -> Result<BeliefReceipt, ContractError> {
        audit.validate()?;
        let i = role_index(recipient);
        let before = self.beliefs[i];
        let mut disposition = BeliefDisposition::Irrelevant;
        let mut delta = 0_i32;
        if Self::relevant(audit, recipient) {
            let key = (recipient, audit.family, audit.inspected_quarter);
            match self.targets.get(&key) {
                Some((id, v)) if *v > audit.version.value() => {
                    return Err(ContractError::InvalidCondition);
                }
                Some((id, v)) if *v == audit.version.value() && *id != audit.id => {
                    return Err(ContractError::InvalidCondition);
                }
                Some((id, v)) if *v == audit.version.value() && *id == audit.id => {
                    disposition = BeliefDisposition::Duplicate
                }
                Some(_) => {
                    self.targets
                        .insert(key, (audit.id.clone(), audit.version.value()));
                    disposition = BeliefDisposition::SupersedingWithoutReplay;
                }
                None => {
                    self.targets
                        .insert(key, (audit.id.clone(), audit.version.value()));
                    let requested = audit.requested_delta();
                    let headroom = (160_000 - self.variation[i]) as i32;
                    let limited = requested.clamp(-headroom, headroom);
                    let after = (i64::from(before) + i64::from(limited)).clamp(0, 1_000_000);
                    delta = i32::try_from(after - i64::from(before))
                        .map_err(|_| ContractError::ArithmeticOverflow)?;
                    self.beliefs[i] =
                        u32::try_from(after).map_err(|_| ContractError::ArithmeticOverflow)?;
                    self.variation[i] += delta.unsigned_abs();
                    disposition = BeliefDisposition::Applied;
                }
            }
        }
        Ok(BeliefReceipt {
            recipient,
            evidence_id: audit.id.clone(),
            before,
            after: self.beliefs[i],
            delta,
            variation_left: 160_000 - self.variation[i],
            disposition,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn example(scheduled: u16, reported: u16, inspected: u16) -> DeliveryAudit {
        DeliveryAudit {
            id: "audit-1".into(),
            version: WireU64::new(1),
            family: PolicyFamily::Staffing,
            inspected_quarter: Quarter::new(2).unwrap(),
            scheduled_cumulative: scheduled,
            reported_cumulative: reported,
            inspected_cumulative: inspected,
            issuance_world_version: WireU64::new(10),
            cost: 0,
        }
    }
    #[test]
    fn zero_due_never_creates_positive_evidence() {
        assert_eq!(example(0, 0, 0).requested_delta(), 0);
        assert_eq!(example(10, 10, 0).requested_delta(), -80_000);
        assert_eq!(example(10, 10, 10).requested_delta(), 40_000);
    }
    #[test]
    fn duplicate_and_supersession_cannot_replay_same_target_delta() {
        let mut ledger = BeliefLedger::default();
        let e = example(10, 8, 2);
        let first = ledger.observe(&e, Ministry::Education).unwrap();
        assert_eq!(first.delta, -80_000);
        assert_eq!(
            ledger.observe(&e, Ministry::Education).unwrap().disposition,
            BeliefDisposition::Duplicate
        );
        let mut corrected = e.clone();
        corrected.id = "audit-2".into();
        corrected.version = WireU64::new(2);
        corrected.inspected_cumulative = 10;
        corrected.reported_cumulative = 10;
        assert_eq!(
            ledger
                .observe(&corrected, Ministry::Education)
                .unwrap()
                .delta,
            0
        );
        assert_eq!(ledger.belief(Ministry::Education), 420_000);
        assert_eq!(ledger.belief(Ministry::Infrastructure), 500_000);
    }
    #[test]
    fn variation_budget_is_per_minister_and_nonreplenishing() {
        let mut ledger = BeliefLedger::default();
        for q in 1..=5 {
            let mut e = example(10, 0, 0);
            e.id = format!("audit-{q}");
            e.inspected_quarter = Quarter::new(q).unwrap();
            ledger.observe(&e, Ministry::Education).unwrap();
        }
        assert_eq!(ledger.variation(Ministry::Education), 160_000);
        assert_eq!(ledger.belief(Ministry::Education), 340_000);
        assert_eq!(ledger.variation(Ministry::Finance), 0);
    }
}
