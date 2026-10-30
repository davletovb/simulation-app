//! P1-02 domain-owned *shape and invariants* for an authoritative world.
//! This is NOT the P3 reducer, a persisted checkpoint, or a replay journal.
//! All state fields are private. Constructors validate inter-ledger consistency.
//! Private minister effort is deliberately kept outside public projections.
use crate::{
    CohortWeights, Commitment, CommitmentStatus, ContractError,
    DueQuarter, FundedScope, Ministry, PILOT_CREDITS, PolicyFamily, Quarter, SCHEMA_VERSION,
    Treasury, WireU64, validate_identifier,
};
use std::collections::BTreeSet;

/// Cohort support is not an actor's belief and has its own bounded type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupportPpm(u32);
impl SupportPpm {
    pub fn new(value: u32) -> Result<Self, ContractError> {
        if value > crate::SHARE_DENOMINATOR {
            Err(ContractError::InvalidSupport)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn value(self) -> u32 { self.0 }
}

/// The first case has exactly two fictional organizations. Not population cells.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Organisation {
    SchoolWorkforceAssociation,
    CommunityAccessNetwork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrganisationAccount {
    pub id: Organisation,
    /// Fictional organization resource units, never treasury credits.
    pub resources: u32,
}
impl OrganisationAccount {
    pub const fn new(id: Organisation, resources: u32) -> Self { Self { id, resources } }
}

/// A response is delayed after actual delivery. No response is applied here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueuedResponse {
    source_event_id: String,
    produced_quarter: Quarter,
    due_quarter: DueQuarter,
    delivered: [u16; 3],
    service_spent: u16,
}
impl QueuedResponse {
    pub fn new(source_event_id: String, produced_quarter: Quarter, due_quarter: DueQuarter,
        delivered: [u16; 3], service_spent: u16) -> Result<Self, ContractError>
    {
        validate_identifier(&source_event_id)?;
        if due_quarter.value() <= u16::from(produced_quarter.value()) {
            return Err(ContractError::InvalidQuarter);
        }
        let total = delivered.iter().try_fold(0_u16, |acc, units|
            acc.checked_add(*units).ok_or(ContractError::ArithmeticOverflow))?;
        if total != service_spent || total > PILOT_CREDITS {
            return Err(ContractError::InvalidTreasury);
        }
        Ok(Self { source_event_id, produced_quarter, due_quarter, delivered, service_spent })
    }
    pub fn source_event_id(&self) -> &str { &self.source_event_id }
    pub const fn produced_quarter(&self) -> Quarter { self.produced_quarter }
    pub const fn due_quarter(&self) -> DueQuarter { self.due_quarter }
    pub const fn delivered(&self) -> [u16; 3] { self.delivered }
    pub const fn service_spent(&self) -> u16 { self.service_spent }
}

/// State-machine labels, not authorization. P3 decides allowed transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeetingPhase {
    QuarterOpen,
    FirstPositionsFrozen,
    AmendmentWindow,
    FinalPositionsFrozen,
    Resolved,
    QuarterClosed,
}

/// Builder input; full historical event and role validation belongs to P3/P4.
#[derive(Clone)]
pub struct WorldParts {
    pub campaign_id: String,
    pub pack_id: String,
    pub world_version: WireU64,
    pub quarter: Quarter,
    pub phase: MeetingPhase,
    pub treasury: Treasury,
    pub scopes: Vec<FundedScope>,
    pub cohort_weights: CohortWeights,
    pub support: [SupportPpm; 6],
    pub organisations: [OrganisationAccount; 2],
    pub commitments: Vec<Commitment>,
    pub queued_responses: Vec<QueuedResponse>,
}

/// Internal validated state skeleton. Never derive Serialize or expose via HTTP.
/// In particular, world truth and reports must not be conflated with role views.
#[derive(Clone)]
pub struct WorldSnapshot {
    schema_version: u16,
    parts: WorldParts,
}
impl WorldSnapshot {
    pub fn initial(campaign_id: String, pack_id: String,
        cohort_weights: CohortWeights) -> Result<Self, ContractError>
    {
        Self::from_parts(WorldParts {
            campaign_id, pack_id, world_version: WireU64::new(0),
            quarter: Quarter::new(1)?, phase: MeetingPhase::QuarterOpen,
            treasury: Treasury::initial(), scopes: vec![], cohort_weights,
            support: [SupportPpm::new(500_000)?; 6],
            organisations: [
                OrganisationAccount::new(Organisation::SchoolWorkforceAssociation, 0),
                OrganisationAccount::new(Organisation::CommunityAccessNetwork, 0),
            ],
            commitments: vec![], queued_responses: vec![],
        })
    }
    pub fn from_parts(parts: WorldParts) -> Result<Self, ContractError> {
        let snapshot = Self { schema_version: SCHEMA_VERSION, parts };
        snapshot.validate()?;
        Ok(snapshot)
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ContractError::InvalidSchema);
        }
        validate_identifier(&self.parts.campaign_id)?;
        validate_identifier(&self.parts.pack_id)?;
        self.parts.treasury.validate()?;
        // Exact six cohort slots and a million ppm are enforced by fixed arrays
        // and the domain CohortWeights/SupportPpm constructors.
        if self.parts.organisations[0].id != Organisation::SchoolWorkforceAssociation
            || self.parts.organisations[1].id != Organisation::CommunityAccessNetwork {
            return Err(ContractError::InvalidScope);
        }
        let mut ids = BTreeSet::new();
        let mut totals = [0_u16; 3];
        for scope in &self.parts.scopes {
            validate_identifier(scope.id())?;
            if !ids.insert(scope.id()) { return Err(ContractError::DuplicateScope); }
            if scope.carry_permille() >= crate::EFFORT_DENOMINATOR {
                return Err(ContractError::InvalidScope);
            }
            let index = family_index(scope.family());
            totals[index] = totals[index].checked_add(scope.remaining_units())
                .ok_or(ContractError::ArithmeticOverflow)?;
        }
        for family in [PolicyFamily::Staffing, PolicyFamily::Repairs, PolicyFamily::Access] {
            if self.parts.treasury.encumbered(family) != totals[family_index(family)] {
                return Err(ContractError::InvalidTreasury);
            }
        }
        let mut commitment_ids = BTreeSet::new();
        for commitment in &self.parts.commitments {
            validate_identifier(&commitment.id)?;
            validate_identifier(&commitment.trigger)?;
            if commitment.funded_units > PILOT_CREDITS
                || !commitment_ids.insert(commitment.id.as_str()) {
                return Err(ContractError::InvalidCondition);
            }
            match (commitment.status, &commitment.resolution_event) {
                (CommitmentStatus::Pending, None) => {}
                (CommitmentStatus::Honored | CommitmentStatus::Broken, Some(event)) => {
                    validate_identifier(event)?;
                }
                _ => return Err(ContractError::InvalidCondition),
            }
        }
        let mut response_ids = BTreeSet::new();
        for response in &self.parts.queued_responses {
            if !response_ids.insert(response.source_event_id()) {
                return Err(ContractError::DuplicateScope);
            }
            // Re-derive the sum to catch accidental future unchecked builders.
            let total: u16 = response.delivered().iter().try_fold(0_u16, |sum, n|
                sum.checked_add(*n).ok_or(ContractError::ArithmeticOverflow))?;
            if total != response.service_spent() { return Err(ContractError::InvalidTreasury); }
        }
        Ok(())
    }
    pub fn campaign_id(&self) -> &str { &self.parts.campaign_id }
    pub fn pack_id(&self) -> &str { &self.parts.pack_id }
    pub fn world_version(&self) -> WireU64 { self.parts.world_version }
    pub fn quarter(&self) -> Quarter { self.parts.quarter }
    pub fn phase(&self) -> MeetingPhase { self.parts.phase }
    pub fn treasury(&self) -> Treasury { self.parts.treasury }
    pub fn scopes(&self) -> &[FundedScope] { &self.parts.scopes }
    pub fn cohort_weights(&self) -> &CohortWeights { &self.parts.cohort_weights }
    pub fn support(&self) -> &[SupportPpm; 6] { &self.parts.support }
    pub fn organisations(&self) -> &[OrganisationAccount; 2] { &self.parts.organisations }
    pub fn commitments(&self) -> &[Commitment] { &self.parts.commitments }
    pub fn queued_responses(&self) -> &[QueuedResponse] { &self.parts.queued_responses }
}
const fn family_index(family: PolicyFamily) -> usize {
    match family { PolicyFamily::Staffing => 0, PolicyFamily::Repairs => 1, PolicyFamily::Access => 2 }
}

/// Mechanical stock-to-stock transitions; not authorization to spend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerMove {
    Reserve { family: PolicyFamily, credits: u16 },
    Release { family: PolicyFamily, credits: u16 },
    SpendService { family: PolicyFamily, credits: u16 },
    SpendAudit { credits: u16 },
}
impl LedgerMove {
    pub const fn credits(self) -> u16 {
        match self {
            Self::Reserve { credits, .. } | Self::Release { credits, .. }
            | Self::SpendService { credits, .. } | Self::SpendAudit { credits } => credits
        }
    }
}

/// Verifies every account delta, not merely that both totals equal 100.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerReceipt {
    before: Treasury,
    after: Treasury,
    movement: LedgerMove,
}
impl LedgerReceipt {
    pub fn new(before: Treasury, movement: LedgerMove, after: Treasury)
        -> Result<Self, ContractError>
    {
        before.validate()?;
        after.validate()?;
        let credits = movement.credits();
        if credits == 0 || credits > PILOT_CREDITS {
            return Err(ContractError::InvalidCreditAmount);
        }
        let mut available = before.available();
        let mut encumbered = [
            before.encumbered(PolicyFamily::Staffing),
            before.encumbered(PolicyFamily::Repairs),
            before.encumbered(PolicyFamily::Access),
        ];
        let mut service = before.service_spent();
        let mut audit = before.audit_spent();
        match movement {
            LedgerMove::Reserve { family, .. } => {
                available = available.checked_sub(credits)
                    .ok_or(ContractError::InsufficientAvailableFunds)?;
                let slot = &mut encumbered[family_index(family)];
                *slot = slot.checked_add(credits).ok_or(ContractError::ArithmeticOverflow)?;
            }
            LedgerMove::Release { family, .. } => {
                let slot = &mut encumbered[family_index(family)];
                *slot = slot.checked_sub(credits).ok_or(ContractError::InvalidTreasury)?;
                available = available.checked_add(credits).ok_or(ContractError::ArithmeticOverflow)?;
            }
            LedgerMove::SpendService { family, .. } => {
                let slot = &mut encumbered[family_index(family)];
                *slot = slot.checked_sub(credits).ok_or(ContractError::InvalidTreasury)?;
                service = service.checked_add(credits).ok_or(ContractError::ArithmeticOverflow)?;
            }
            LedgerMove::SpendAudit { .. } => {
                available = available.checked_sub(credits)
                    .ok_or(ContractError::InsufficientAvailableFunds)?;
                audit = audit.checked_add(credits).ok_or(ContractError::ArithmeticOverflow)?;
            }
        }
        let expected = Treasury::from_parts(
            available, encumbered[0], encumbered[1], encumbered[2], service, audit,
        )?;
        if after != expected { return Err(ContractError::InvalidTreasury); }
        Ok(Self { before, after, movement })
    }
    pub const fn before(&self) -> Treasury { self.before }
    pub const fn after(&self) -> Treasury { self.after }
    pub const fn movement(&self) -> LedgerMove { self.movement }
}

/// Versioned event *envelope shape*. Actual command admission, Hiyal event
/// hashes, event order and replay binding are explicitly deferred to P3/P4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventKind {
    CommandAdmitted { command_id: String },
    MeetingPhaseDeclared { phase: MeetingPhase },
    LedgerTransfer(LedgerReceipt),
    DeliveryRecorded { family: PolicyFamily, delivered: u16 },
    EvidenceDisclosed { evidence_id: String, recipient: Ministry },
    ResponseQueued { source_event_id: String },
    QuarterAdvanced { next: Quarter },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventEnvelope {
    schema_version: u16,
    campaign_id: String,
    event_id: String,
    sequence: WireU64,
    world_version: WireU64,
    quarter: Quarter,
    kind: EventKind,
}
impl EventEnvelope {
    pub fn new(campaign_id: String, event_id: String, sequence: WireU64,
        world_version: WireU64, quarter: Quarter, kind: EventKind)
        -> Result<Self, ContractError>
    {
        validate_identifier(&campaign_id)?;
        validate_identifier(&event_id)?;
        if sequence.value() == 0 { return Err(ContractError::InvalidDecimal); }
        match &kind {
            EventKind::CommandAdmitted { command_id } => validate_identifier(command_id)?,
            EventKind::EvidenceDisclosed { evidence_id, .. } => validate_identifier(evidence_id)?,
            EventKind::ResponseQueued { source_event_id } => validate_identifier(source_event_id)?,
            EventKind::QuarterAdvanced { next } if next.value() != quarter.value() + 1 =>
                return Err(ContractError::InvalidQuarter),
            _ => {}
        }
        Ok(Self {
            schema_version: SCHEMA_VERSION, campaign_id, event_id, sequence,
            world_version, quarter, kind,
        })
    }
    pub const fn schema_version(&self) -> u16 { self.schema_version }
    pub fn campaign_id(&self) -> &str { &self.campaign_id }
    pub fn event_id(&self) -> &str { &self.event_id }
    pub const fn sequence(&self) -> WireU64 { self.sequence }
    pub const fn world_version(&self) -> WireU64 { self.world_version }
    pub const fn quarter(&self) -> Quarter { self.quarter }
    pub fn kind(&self) -> &EventKind { &self.kind }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Allocation, deliver_family_quarter, EffortPermille};

    fn weights() -> CohortWeights {
        CohortWeights::new([200_000, 200_000, 100_000, 150_000, 200_000, 150_000]).unwrap()
    }
    #[test]
    fn support_is_bounded_independently_of_minister_beliefs() {
        assert_eq!(SupportPpm::new(0).unwrap().value(), 0);
        assert_eq!(SupportPpm::new(1_000_000).unwrap().value(), 1_000_000);
        assert_eq!(SupportPpm::new(1_000_001), Err(ContractError::InvalidSupport));
    }
    #[test]
    fn initial_world_has_exact_six_cell_stock_and_no_fake_spending() {
        let world = WorldSnapshot::initial("campaign".into(), "england-v1".into(), weights()).unwrap();
        assert_eq!(world.treasury(), Treasury::initial());
        assert_eq!(world.world_version().value(), 0);
        assert_eq!(world.quarter().value(), 1);
        assert_eq!(world.support().len(), 6);
        assert!(world.support().iter().all(|n| n.value() == 500_000));
        assert_eq!(world.organisations().len(), 2);
        assert!(world.scopes().is_empty());
        world.validate().unwrap();
    }
    #[test]
    fn funded_scope_stocks_must_equal_encumbered_family_by_family() {
        let initial = WorldSnapshot::initial("campaign".into(), "pack".into(), weights()).unwrap();
        let mut parts = initial.parts;
        parts.treasury = Treasury::from_parts(88, 12, 0, 0, 0, 0).unwrap();
        assert!(matches!(WorldSnapshot::from_parts(parts.clone()), Err(ContractError::InvalidTreasury)));
        parts.scopes.push(FundedScope::new("staff-a".into(),PolicyFamily::Staffing,
            Quarter::new(1).unwrap(),12).unwrap());
        WorldSnapshot::from_parts(parts.clone()).unwrap();
        parts.scopes.push(parts.scopes[0].clone());
        assert!(matches!(WorldSnapshot::from_parts(parts), Err(ContractError::DuplicateScope)));
    }
    #[test]
    fn actual_family_delivery_receipt_reconciles_exact_ledger_delta() {
        let before = Treasury::from_parts(88, 12, 0, 0, 0, 0).unwrap();
        let mut scopes = vec![FundedScope::new("s".into(),PolicyFamily::Staffing,
            Quarter::new(1).unwrap(),12).unwrap()];
        let receipt = deliver_family_quarter(PolicyFamily::Staffing,12,false,
            EffortPermille::new(1000).unwrap(), DueQuarter::new(2).unwrap(), &mut scopes).unwrap();
        assert_eq!(receipt.total_delivered,12);
        let after = Treasury::from_parts(88,0,0,0,12,0).unwrap();
        LedgerReceipt::new(before,LedgerMove::SpendService{
            family:PolicyFamily::Staffing,credits:receipt.total_delivered,
        },after).unwrap();
        let incorrect = Treasury::from_parts(88,0,0,0,11,1).unwrap();
        assert!(matches!(LedgerReceipt::new(before,LedgerMove::SpendService{
            family:PolicyFamily::Staffing,credits:12,
        },incorrect),Err(ContractError::InvalidTreasury)));
    }
    #[test]
    fn all_ledger_moves_preserve_whole_campaign_100_credit_stock() {
        let start=Treasury::initial();
        let reserved=Treasury::from_parts(90,10,0,0,0,0).unwrap();
        LedgerReceipt::new(start,LedgerMove::Reserve{
            family:PolicyFamily::Staffing,credits:10,
        },reserved).unwrap();
        let released=Treasury::from_parts(95,5,0,0,0,0).unwrap();
        LedgerReceipt::new(reserved,LedgerMove::Release{
            family:PolicyFamily::Staffing,credits:5,
        },released).unwrap();
        let audit=Treasury::from_parts(93,5,0,0,0,2).unwrap();
        LedgerReceipt::new(released,LedgerMove::SpendAudit{credits:2},audit).unwrap();
        assert!(matches!(LedgerReceipt::new(released,LedgerMove::SpendAudit{credits:2},reserved),
            Err(ContractError::InvalidTreasury)));
        assert!(matches!(LedgerReceipt::new(start,LedgerMove::SpendAudit{credits:0},start),
            Err(ContractError::InvalidCreditAmount)));
    }
    #[test]
    fn queued_response_reconciles_delivered_units_and_future_lag() {
        let q=Quarter::new(6).unwrap();
        assert!(QueuedResponse::new("event".into(),q,DueQuarter::new(7).unwrap(),
            [3,2,4],9).is_ok());
        assert!(QueuedResponse::new("event".into(),q,DueQuarter::new(6).unwrap(),
            [3,2,4],9).is_err());
        assert!(QueuedResponse::new("event".into(),q,DueQuarter::new(7).unwrap(),
            [3,2,4],8).is_err());
    }
    #[test]
    fn versioned_event_envelope_rejects_invalid_ids_and_impossible_advance() {
        let q=Quarter::new(2).unwrap();
        let event=EventEnvelope::new("campaign".into(),"event-1".into(),
            WireU64::new(u64::MAX),WireU64::new(u64::MAX),q,
            EventKind::CommandAdmitted{command_id:"cmd-1".into()}).unwrap();
        assert_eq!(event.sequence().to_decimal(),u64::MAX.to_string());
        assert!(EventEnvelope::new("campaign".into(),"".into(),WireU64::new(1),
            WireU64::new(0),q,EventKind::MeetingPhaseDeclared{
                phase:MeetingPhase::FinalPositionsFrozen}).is_err());
        assert!(EventEnvelope::new("campaign".into(),"e".into(),WireU64::new(1),
            WireU64::new(0),q,EventKind::QuarterAdvanced{
                next:Quarter::new(4).unwrap()}).is_err());
        assert!(EventEnvelope::new("campaign".into(),"e".into(),WireU64::new(0),
            WireU64::new(0),q,EventKind::CommandAdmitted{command_id:"c".into()}).is_err());
    }
    #[test]
    fn invalid_commitments_and_duplicate_delayed_events_fail_closed() {
        let mut parts = WorldSnapshot::initial("campaign".into(),"pack".into(),weights())
            .unwrap().parts;
        parts.commitments.push(Commitment {
            id:"condition".into(), issuer:Ministry::Finance, beneficiary:Ministry::Education,
            proposal_version:WireU64::new(1),
            kind:crate::CommitmentKind::Political, funded_units:2,
            trigger:"has budget".into(), deadline_quarter:DueQuarter::new(3).unwrap(),
            status:CommitmentStatus::Honored, resolution_event:None,
        });
        assert!(matches!(WorldSnapshot::from_parts(parts.clone()),
            Err(ContractError::InvalidCondition)));
        parts.commitments[0].resolution_event = Some("event".into());
        WorldSnapshot::from_parts(parts.clone()).unwrap();
        parts.commitments[0].status=CommitmentStatus::Pending;
        assert!(matches!(WorldSnapshot::from_parts(parts.clone()),
            Err(ContractError::InvalidCondition)));
        parts.commitments.clear();
        let response=QueuedResponse::new("source-1".into(),Quarter::new(1).unwrap(),
            DueQuarter::new(2).unwrap(),[2,0,1],3).unwrap();
        parts.queued_responses=vec![response.clone(),response];
        assert!(matches!(WorldSnapshot::from_parts(parts),
            Err(ContractError::DuplicateScope)));
    }
    #[test]
    fn organisation_registry_order_is_checked_not_just_two_entries() {
        let mut parts=WorldSnapshot::initial("campaign".into(),"pack".into(),weights())
            .unwrap().parts;
        parts.organisations.swap(0,1);
        assert!(matches!(WorldSnapshot::from_parts(parts),Err(ContractError::InvalidScope)));
    }
    #[test]
    fn mechanical_receipt_rejects_overspend_from_wrong_account() {
        let before=Treasury::from_parts(90,10,0,0,0,0).unwrap();
        let after=Treasury::from_parts(90,0,0,0,10,0).unwrap();
        assert!(LedgerReceipt::new(before,LedgerMove::SpendService{
            family:PolicyFamily::Repairs,credits:10,
        },after).is_err());
        assert!(LedgerReceipt::new(before,LedgerMove::Release{
            family:PolicyFamily::Staffing,credits:11,
        },Treasury::initial()).is_err());
    }

    #[test]
    fn overbudget_draft_does_not_create_world_financial_entries() {
        let allocation=Allocation::draft(60,40,10).unwrap();
        assert!(Treasury::initial().can_fund(allocation).is_err());
        let world=WorldSnapshot::initial("campaign".into(),"pack".into(),weights()).unwrap();
        assert_eq!(world.treasury(),Treasury::initial());
    }
}
