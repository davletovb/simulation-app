//! P1-02 domain-owned *shape and invariants* for an authoritative world.
//! This is NOT the P3 reducer, a persisted checkpoint, or a replay journal.
//! All state fields are private. Constructors validate inter-ledger consistency.
//! Private minister effort is deliberately kept outside public projections.
use crate::{
    CohortWeights, Commitment, CommitmentStatus, ContractError, DueQuarter, FundedScope,
    LedgerMove, Ministry, PILOT_CREDITS, PolicyFamily, Quarter, SCHEMA_VERSION, Treasury, WireU64,
    validate_identifier,
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
    pub const fn value(self) -> u32 {
        self.0
    }
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
    resources: u32,
}
impl OrganisationAccount {
    pub fn new(id: Organisation, resources: u32) -> Result<Self, ContractError> {
        let account = Self { id, resources };
        account.validate()?;
        Ok(account)
    }
    pub const fn id(&self) -> Organisation {
        self.id
    }
    pub const fn resources(&self) -> u32 {
        self.resources
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        let max = match self.id {
            Organisation::SchoolWorkforceAssociation => 20,
            Organisation::CommunityAccessNetwork => 12,
        };
        if self.resources > max {
            Err(ContractError::InvalidScope)
        } else {
            Ok(())
        }
    }
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
    pub fn new(
        source_event_id: String,
        produced_quarter: Quarter,
        due_quarter: DueQuarter,
        delivered: [u16; 3],
        service_spent: u16,
    ) -> Result<Self, ContractError> {
        let candidate = Self {
            source_event_id,
            produced_quarter,
            due_quarter,
            delivered,
            service_spent,
        };
        candidate.validate()?;
        Ok(candidate)
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_identifier(&self.source_event_id)?;
        if self.due_quarter.value() <= u16::from(self.produced_quarter.value()) {
            return Err(ContractError::InvalidQuarter);
        }
        let total = self.delivered.iter().try_fold(0_u16, |acc, units| {
            acc.checked_add(*units)
                .ok_or(ContractError::ArithmeticOverflow)
        })?;
        if total != self.service_spent || total > PILOT_CREDITS {
            return Err(ContractError::InvalidTreasury);
        }
        Ok(())
    }
    pub fn source_event_id(&self) -> &str {
        &self.source_event_id
    }
    pub const fn produced_quarter(&self) -> Quarter {
        self.produced_quarter
    }
    pub const fn due_quarter(&self) -> DueQuarter {
        self.due_quarter
    }
    pub const fn delivered(&self) -> [u16; 3] {
        self.delivered
    }
    pub const fn service_spent(&self) -> u16 {
        self.service_spent
    }
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
    pub fn initial(
        campaign_id: String,
        pack_id: String,
        cohort_weights: CohortWeights,
    ) -> Result<Self, ContractError> {
        Self::from_parts(WorldParts {
            campaign_id,
            pack_id,
            world_version: WireU64::new(0),
            quarter: Quarter::new(1)?,
            phase: MeetingPhase::QuarterOpen,
            treasury: Treasury::initial(),
            scopes: vec![],
            cohort_weights,
            support: [SupportPpm::new(500_000)?; 6],
            organisations: [
                OrganisationAccount::new(Organisation::SchoolWorkforceAssociation, 10)?,
                OrganisationAccount::new(Organisation::CommunityAccessNetwork, 4)?,
            ],
            commitments: vec![],
            queued_responses: vec![],
        })
    }
    pub fn from_parts(parts: WorldParts) -> Result<Self, ContractError> {
        let snapshot = Self {
            schema_version: SCHEMA_VERSION,
            parts,
        };
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
        if self.parts.organisations[0].id() != Organisation::SchoolWorkforceAssociation
            || self.parts.organisations[1].id() != Organisation::CommunityAccessNetwork
        {
            return Err(ContractError::InvalidScope);
        }
        for org in &self.parts.organisations {
            org.validate()?;
        }
        let mut ids = BTreeSet::new();
        let mut totals = [0_u16; 3];
        for scope in &self.parts.scopes {
            validate_identifier(scope.id())?;
            if !ids.insert(scope.id()) {
                return Err(ContractError::DuplicateScope);
            }
            if scope.carry_permille() >= crate::EFFORT_DENOMINATOR
                || scope.activation_quarter() > self.parts.quarter
            {
                return Err(ContractError::InvalidScope);
            }
            // Cancellation discards fractional carry and releases unspent
            // credits atomically in P3. Keep the scope for history, not funds.
            if scope.is_terminated() {
                if scope.carry_permille() != 0 {
                    return Err(ContractError::InvalidScope);
                }
                continue;
            }
            let index = family_index(scope.family());
            totals[index] = totals[index]
                .checked_add(scope.remaining_units())
                .ok_or(ContractError::ArithmeticOverflow)?;
        }
        for family in [
            PolicyFamily::Staffing,
            PolicyFamily::Repairs,
            PolicyFamily::Access,
        ] {
            if self.parts.treasury.encumbered(family) != totals[family_index(family)] {
                return Err(ContractError::InvalidTreasury);
            }
        }
        let mut commitment_ids = BTreeSet::new();
        for commitment in &self.parts.commitments {
            commitment.validate()?;
            if !commitment_ids.insert(commitment.id.as_str())
                || (commitment.status == CommitmentStatus::Pending
                    && commitment.deadline_quarter.value() < u16::from(self.parts.quarter.value()))
            {
                return Err(ContractError::InvalidCondition);
            }
        }
        let mut response_ids = BTreeSet::new();
        let mut queued_spend = 0_u16;
        for response in &self.parts.queued_responses {
            response.validate()?;
            if !response_ids.insert(response.source_event_id()) {
                return Err(ContractError::DuplicateScope);
            }
            let now = u16::from(self.parts.quarter.value());
            if response.produced_quarter() > self.parts.quarter
                || response.due_quarter().value() < now
            {
                return Err(ContractError::InvalidQuarter);
            }
            queued_spend = queued_spend
                .checked_add(response.service_spent())
                .ok_or(ContractError::ArithmeticOverflow)?;
        }
        // Already-applied responses have left this queue; not every lifetime
        // service unit must remain queued, but no queued unit can be invented.
        if queued_spend > self.parts.treasury.service_spent() {
            return Err(ContractError::InvalidTreasury);
        }
        Ok(())
    }
    pub fn campaign_id(&self) -> &str {
        &self.parts.campaign_id
    }
    pub fn pack_id(&self) -> &str {
        &self.parts.pack_id
    }
    pub fn world_version(&self) -> WireU64 {
        self.parts.world_version
    }
    pub fn quarter(&self) -> Quarter {
        self.parts.quarter
    }
    pub fn phase(&self) -> MeetingPhase {
        self.parts.phase
    }
    pub fn treasury(&self) -> Treasury {
        self.parts.treasury
    }
    pub fn scopes(&self) -> &[FundedScope] {
        &self.parts.scopes
    }
    pub fn cohort_weights(&self) -> &CohortWeights {
        &self.parts.cohort_weights
    }
    pub fn support(&self) -> &[SupportPpm; 6] {
        &self.parts.support
    }
    pub fn organisations(&self) -> &[OrganisationAccount; 2] {
        &self.parts.organisations
    }
    pub fn commitments(&self) -> &[Commitment] {
        &self.parts.commitments
    }
    pub fn queued_responses(&self) -> &[QueuedResponse] {
        &self.parts.queued_responses
    }
    /// The P1 projection boundary is server-owned and role-scoped. It cannot
    /// reveal world truth or other ministers' private effort (not stored here).
    /// Full audit/evidence provenance and authorization remain P3.
    pub fn role_observation(
        &self, ministry: Ministry, meeting_id: String, proposal_version: WireU64,
        allocation: crate::Allocation, belief: crate::BeliefPpm,
    ) -> Result<crate::RoleObservation, ContractError> {
        self.validate()?;
        let mut view = crate::RoleObservation::new(crate::RoleObservationInput {
            campaign_id: self.campaign_id().into(), meeting_id, proposal_version,
            ministry, quarter: self.quarter(), allocation,
            available_credits: self.treasury().available(), belief_ppm: belief,
        })?;
        view.owned_commitments = self.parts.commitments.iter()
            .filter(|item| item.issuer == ministry || item.beneficiary == ministry)
            .cloned().collect();
        view.validate()?;
        Ok(view)
    }
    /// Checked owned state parts for server-private checkpoint serialization.
    /// Do not expose this from HTTP, browser, or player reports.
    pub fn private_checkpoint_parts(&self) -> WorldParts {
        self.parts.clone()
    }
}
const fn family_index(family: PolicyFamily) -> usize {
    match family {
        PolicyFamily::Staffing => 0,
        PolicyFamily::Repairs => 1,
        PolicyFamily::Access => 2,
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
    pub fn new(
        before: Treasury,
        movement: LedgerMove,
        after: Treasury,
    ) -> Result<Self, ContractError> {
        // No independent balance-moving code: Treasury::apply is authoritative.
        let expected = before.apply(movement)?;
        if after != expected {
            return Err(ContractError::InvalidTreasury);
        }
        Ok(Self {
            before,
            after,
            movement,
        })
    }
    pub const fn before(&self) -> Treasury {
        self.before
    }
    pub const fn after(&self) -> Treasury {
        self.after
    }
    pub const fn movement(&self) -> LedgerMove {
        self.movement
    }
}

/// Versioned event *envelope shape*. Actual command admission, Hiyal event
/// hashes, event order and replay binding are explicitly deferred to P3/P4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventKind {
    CommandAdmitted {
        command_id: String,
    },
    MeetingPhaseDeclared {
        phase: MeetingPhase,
    },
    LedgerTransfer(LedgerReceipt),
    DeliveryRecorded {
        family: PolicyFamily,
        delivered: u16,
    },
    EvidenceDisclosed {
        evidence_id: String,
        recipient: Ministry,
    },
    ResponseQueued {
        source_event_id: String,
    },
    QuarterAdvanced {
        next: Quarter,
    },
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
    pub fn new(
        campaign_id: String,
        event_id: String,
        sequence: WireU64,
        world_version: WireU64,
        quarter: Quarter,
        kind: EventKind,
    ) -> Result<Self, ContractError> {
        validate_identifier(&campaign_id)?;
        validate_identifier(&event_id)?;
        if sequence.value() == 0 {
            return Err(ContractError::InvalidDecimal);
        }
        match &kind {
            EventKind::CommandAdmitted { command_id } => validate_identifier(command_id)?,
            EventKind::EvidenceDisclosed { evidence_id, .. } => validate_identifier(evidence_id)?,
            EventKind::ResponseQueued { source_event_id } => validate_identifier(source_event_id)?,
            EventKind::DeliveryRecorded { family, delivered } => {
                let cap = family.nominal_capacity() + u16::from(*family == PolicyFamily::Access);
                if *delivered == 0 || *delivered > cap || *delivered > PILOT_CREDITS {
                    return Err(ContractError::InvalidCreditAmount);
                }
            }
            EventKind::QuarterAdvanced { next } if next.value() != quarter.value() + 1 => {
                return Err(ContractError::InvalidQuarter);
            }
            _ => {}
        }
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            campaign_id,
            event_id,
            sequence,
            world_version,
            quarter,
            kind,
        })
    }
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }
    pub fn event_id(&self) -> &str {
        &self.event_id
    }
    pub const fn sequence(&self) -> WireU64 {
        self.sequence
    }
    pub const fn world_version(&self) -> WireU64 {
        self.world_version
    }
    pub const fn quarter(&self) -> Quarter {
        self.quarter
    }
    pub fn kind(&self) -> &EventKind {
        &self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Allocation, EffortPermille, deliver_family_quarter};

    fn weights() -> CohortWeights {
        CohortWeights::new([200_000, 200_000, 100_000, 150_000, 200_000, 150_000]).unwrap()
    }
    #[test]
    fn support_is_bounded_independently_of_minister_beliefs() {
        assert_eq!(SupportPpm::new(0).unwrap().value(), 0);
        assert_eq!(SupportPpm::new(1_000_000).unwrap().value(), 1_000_000);
        assert_eq!(
            SupportPpm::new(1_000_001),
            Err(ContractError::InvalidSupport)
        );
    }
    #[test]
    fn role_projection_filters_other_seats_commitments() {
        let mut parts = WorldSnapshot::initial("c".into(),"pack".into(),
            CohortWeights::new([200_000, 200_000, 100_000, 150_000, 200_000, 150_000]).unwrap())
            .unwrap().private_checkpoint_parts();
        parts.commitments.push(Commitment {
            id: "education-only".into(), issuer: Ministry::Education,
            beneficiary: Ministry::Education, proposal_version: WireU64::new(0),
            kind: crate::CommitmentKind::Political, funded_units: 0,
            trigger: "promise".into(), deadline_quarter: DueQuarter::new(2).unwrap(),
            status: CommitmentStatus::Pending, resolution_event: None,
        });
        let snapshot = WorldSnapshot::from_parts(parts).unwrap();
        let finance = snapshot.role_observation(
            Ministry::Finance,"m".into(),WireU64::new(0),
            Allocation::draft(40,40,20).unwrap(),crate::BeliefPpm::new(500_000).unwrap()).unwrap();
        let education = snapshot.role_observation(
            Ministry::Education,"m".into(),WireU64::new(0),
            Allocation::draft(40,40,20).unwrap(),crate::BeliefPpm::new(500_000).unwrap()).unwrap();
        assert!(finance.owned_commitments.is_empty());
        assert_eq!(education.owned_commitments.len(),1);
    }

    #[test]
    fn initial_world_has_exact_six_cell_stock_and_no_fake_spending() {
        let world =
            WorldSnapshot::initial("campaign".into(), "england-v1".into(), weights()).unwrap();
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
        assert!(matches!(
            WorldSnapshot::from_parts(parts.clone()),
            Err(ContractError::InvalidTreasury)
        ));
        parts.scopes.push(
            FundedScope::new(
                "staff-a".into(),
                PolicyFamily::Staffing,
                Quarter::new(1).unwrap(),
                12,
            )
            .unwrap(),
        );
        WorldSnapshot::from_parts(parts.clone()).unwrap();
        parts.scopes.push(parts.scopes[0].clone());
        assert!(matches!(
            WorldSnapshot::from_parts(parts),
            Err(ContractError::DuplicateScope)
        ));
    }
    #[test]
    fn actual_family_delivery_receipt_reconciles_exact_ledger_delta() {
        let before = Treasury::from_parts(88, 12, 0, 0, 0, 0).unwrap();
        let mut scopes = vec![
            FundedScope::new(
                "s".into(),
                PolicyFamily::Staffing,
                Quarter::new(1).unwrap(),
                12,
            )
            .unwrap(),
        ];
        let receipt = deliver_family_quarter(
            PolicyFamily::Staffing,
            12,
            false,
            EffortPermille::new(1000).unwrap(),
            DueQuarter::new(2).unwrap(),
            &mut scopes,
        )
        .unwrap();
        assert_eq!(receipt.total_delivered, 12);
        let after = Treasury::from_parts(88, 0, 0, 0, 12, 0).unwrap();
        LedgerReceipt::new(
            before,
            LedgerMove::SpendService {
                family: PolicyFamily::Staffing,
                credits: receipt.total_delivered,
            },
            after,
        )
        .unwrap();
        let incorrect = Treasury::from_parts(88, 0, 0, 0, 11, 1).unwrap();
        assert!(matches!(
            LedgerReceipt::new(
                before,
                LedgerMove::SpendService {
                    family: PolicyFamily::Staffing,
                    credits: 12,
                },
                incorrect
            ),
            Err(ContractError::InvalidTreasury)
        ));
    }
    #[test]
    fn all_ledger_moves_preserve_whole_campaign_100_credit_stock() {
        let start = Treasury::initial();
        let reserved = Treasury::from_parts(90, 10, 0, 0, 0, 0).unwrap();
        LedgerReceipt::new(
            start,
            LedgerMove::Reserve {
                family: PolicyFamily::Staffing,
                credits: 10,
            },
            reserved,
        )
        .unwrap();
        let released = Treasury::from_parts(95, 5, 0, 0, 0, 0).unwrap();
        LedgerReceipt::new(
            reserved,
            LedgerMove::Release {
                family: PolicyFamily::Staffing,
                credits: 5,
            },
            released,
        )
        .unwrap();
        let audit = Treasury::from_parts(93, 5, 0, 0, 0, 2).unwrap();
        LedgerReceipt::new(released, LedgerMove::SpendAudit { credits: 2 }, audit).unwrap();
        assert!(matches!(
            LedgerReceipt::new(released, LedgerMove::SpendAudit { credits: 2 }, reserved),
            Err(ContractError::InvalidTreasury)
        ));
        assert!(matches!(
            LedgerReceipt::new(start, LedgerMove::SpendAudit { credits: 0 }, start),
            Err(ContractError::InvalidCreditAmount)
        ));
    }
    #[test]
    fn queued_response_reconciles_delivered_units_and_future_lag() {
        let q = Quarter::new(6).unwrap();
        assert!(
            QueuedResponse::new("event".into(), q, DueQuarter::new(7).unwrap(), [3, 2, 4], 9)
                .is_ok()
        );
        assert!(
            QueuedResponse::new("event".into(), q, DueQuarter::new(6).unwrap(), [3, 2, 4], 9)
                .is_err()
        );
        assert!(
            QueuedResponse::new("event".into(), q, DueQuarter::new(7).unwrap(), [3, 2, 4], 8)
                .is_err()
        );
    }
    #[test]
    fn versioned_event_envelope_rejects_invalid_ids_and_impossible_advance() {
        let q = Quarter::new(2).unwrap();
        let event = EventEnvelope::new(
            "campaign".into(),
            "event-1".into(),
            WireU64::new(u64::MAX),
            WireU64::new(u64::MAX),
            q,
            EventKind::CommandAdmitted {
                command_id: "cmd-1".into(),
            },
        )
        .unwrap();
        assert_eq!(event.sequence().to_decimal(), u64::MAX.to_string());
        assert!(
            EventEnvelope::new(
                "campaign".into(),
                "".into(),
                WireU64::new(1),
                WireU64::new(0),
                q,
                EventKind::MeetingPhaseDeclared {
                    phase: MeetingPhase::FinalPositionsFrozen
                }
            )
            .is_err()
        );
        assert!(
            EventEnvelope::new(
                "campaign".into(),
                "e".into(),
                WireU64::new(1),
                WireU64::new(0),
                q,
                EventKind::QuarterAdvanced {
                    next: Quarter::new(4).unwrap()
                }
            )
            .is_err()
        );
        assert!(
            EventEnvelope::new(
                "campaign".into(),
                "e".into(),
                WireU64::new(0),
                WireU64::new(0),
                q,
                EventKind::CommandAdmitted {
                    command_id: "c".into()
                }
            )
            .is_err()
        );
    }
    #[test]
    fn invalid_commitments_and_duplicate_delayed_events_fail_closed() {
        let mut parts = WorldSnapshot::initial("campaign".into(), "pack".into(), weights())
            .unwrap()
            .parts;
        parts.commitments.push(Commitment {
            id: "condition".into(),
            issuer: Ministry::Finance,
            beneficiary: Ministry::Education,
            proposal_version: WireU64::new(1),
            kind: crate::CommitmentKind::Political,
            funded_units: 0,
            trigger: "has budget".into(),
            deadline_quarter: DueQuarter::new(3).unwrap(),
            status: CommitmentStatus::Honored,
            resolution_event: None,
        });
        assert!(matches!(
            WorldSnapshot::from_parts(parts.clone()),
            Err(ContractError::InvalidCondition)
        ));
        parts.commitments[0].resolution_event = Some("event".into());
        WorldSnapshot::from_parts(parts.clone()).unwrap();
        parts.commitments[0].status = CommitmentStatus::Pending;
        assert!(matches!(
            WorldSnapshot::from_parts(parts.clone()),
            Err(ContractError::InvalidCondition)
        ));
        parts.commitments.clear();
        let response = QueuedResponse::new(
            "source-1".into(),
            Quarter::new(1).unwrap(),
            DueQuarter::new(2).unwrap(),
            [2, 0, 1],
            3,
        )
        .unwrap();
        parts.queued_responses = vec![response.clone(), response];
        assert!(matches!(
            WorldSnapshot::from_parts(parts),
            Err(ContractError::DuplicateScope)
        ));
    }
    #[test]
    fn organisation_registry_order_is_checked_not_just_two_entries() {
        let mut parts = WorldSnapshot::initial("campaign".into(), "pack".into(), weights())
            .unwrap()
            .parts;
        parts.organisations.swap(0, 1);
        assert!(matches!(
            WorldSnapshot::from_parts(parts),
            Err(ContractError::InvalidScope)
        ));
    }
    #[test]
    fn mechanical_receipt_rejects_overspend_from_wrong_account() {
        let before = Treasury::from_parts(90, 10, 0, 0, 0, 0).unwrap();
        let after = Treasury::from_parts(90, 0, 0, 0, 10, 0).unwrap();
        assert!(
            LedgerReceipt::new(
                before,
                LedgerMove::SpendService {
                    family: PolicyFamily::Repairs,
                    credits: 10,
                },
                after
            )
            .is_err()
        );
        assert!(
            LedgerReceipt::new(
                before,
                LedgerMove::Release {
                    family: PolicyFamily::Staffing,
                    credits: 11,
                },
                Treasury::initial()
            )
            .is_err()
        );
    }

    #[test]
    fn organisation_starts_and_caps_match_pilot_contract() {
        let world = WorldSnapshot::initial("campaign".into(), "pack".into(), weights()).unwrap();
        assert_eq!(world.organisations()[0].resources(), 10);
        assert_eq!(world.organisations()[1].resources(), 4);
        assert!(OrganisationAccount::new(Organisation::SchoolWorkforceAssociation, 20).is_ok());
        assert!(OrganisationAccount::new(Organisation::CommunityAccessNetwork, 12).is_ok());
        assert!(OrganisationAccount::new(Organisation::SchoolWorkforceAssociation, 21).is_err());
        assert!(OrganisationAccount::new(Organisation::CommunityAccessNetwork, 13).is_err());
    }
    #[test]
    fn termination_releases_credits_without_discarding_historical_scope() {
        let mut parts = WorldSnapshot::initial("c".into(), "pack".into(), weights())
            .unwrap()
            .parts;
        parts.treasury = Treasury::from_parts(88, 12, 0, 0, 0, 0).unwrap();
        parts.scopes.push(
            FundedScope::new(
                "old".into(),
                PolicyFamily::Staffing,
                Quarter::new(1).unwrap(),
                12,
            )
            .unwrap(),
        );
        WorldSnapshot::from_parts(parts.clone()).unwrap();
        let receipt = parts.scopes[0].terminate();
        assert_eq!(receipt.dropped_carry_permille, 0);
        // The mechanical release and termination must be committed atomically
        // by P3. Either half in isolation fails this snapshot invariant.
        assert!(WorldSnapshot::from_parts(parts.clone()).is_err());
        parts.treasury = parts
            .treasury
            .apply(LedgerMove::Release {
                family: PolicyFamily::Staffing,
                credits: 12,
            })
            .unwrap();
        let world = WorldSnapshot::from_parts(parts).unwrap();
        assert!(world.scopes()[0].is_terminated());
        assert_eq!(world.scopes()[0].remaining_units(), 12);
        assert_eq!(world.treasury().encumbered(PolicyFamily::Staffing), 0);
    }
    #[test]
    fn pending_political_funding_rejected_consistently_in_world_and_role_view() {
        let invalid = Commitment {
            id: "political-1".into(),
            issuer: Ministry::Finance,
            beneficiary: Ministry::Education,
            proposal_version: WireU64::new(0),
            kind: crate::CommitmentKind::Political,
            funded_units: 1,
            trigger: "promise".into(),
            deadline_quarter: DueQuarter::new(3).unwrap(),
            status: CommitmentStatus::Pending,
            resolution_event: None,
        };
        assert!(invalid.validate().is_err());
        let mut parts = WorldSnapshot::initial("c".into(), "pack".into(), weights())
            .unwrap()
            .parts;
        parts.commitments.push(invalid.clone());
        assert!(WorldSnapshot::from_parts(parts).is_err());
        let mut role = crate::RoleObservation::new(crate::RoleObservationInput {
            campaign_id: "c".into(),
            meeting_id: "m".into(),
            proposal_version: WireU64::new(0),
            ministry: Ministry::Finance,
            quarter: Quarter::new(1).unwrap(),
            allocation: Allocation::draft(0, 0, 0).unwrap(),
            available_credits: 100,
            belief_ppm: crate::BeliefPpm::new(500_000).unwrap(),
        })
        .unwrap();
        role.owned_commitments.push(invalid);
        assert!(role.validate().is_err());
    }
    #[test]
    fn future_or_overdue_world_components_and_unspent_queued_effects_fail() {
        let seed = WorldSnapshot::initial("c".into(), "pack".into(), weights())
            .unwrap()
            .parts;
        let mut scope = seed.clone();
        scope.scopes.push(
            FundedScope::new(
                "future".into(),
                PolicyFamily::Staffing,
                Quarter::new(2).unwrap(),
                1,
            )
            .unwrap(),
        );
        scope.treasury = Treasury::from_parts(99, 1, 0, 0, 0, 0).unwrap();
        assert!(WorldSnapshot::from_parts(scope).is_err());

        let make = |produced: u8, due: u16| {
            QueuedResponse::new(
                "source".into(),
                Quarter::new(produced).unwrap(),
                DueQuarter::new(due).unwrap(),
                [2, 1, 0],
                3,
            )
            .unwrap()
        };
        let mut no_spend = seed.clone();
        no_spend.queued_responses.push(make(1, 2));
        assert!(matches!(
            WorldSnapshot::from_parts(no_spend),
            Err(ContractError::InvalidTreasury)
        ));

        let mut cumulative = seed.clone();
        cumulative.treasury = Treasury::from_parts(95, 0, 0, 0, 5, 0).unwrap();
        cumulative.queued_responses.push(make(1, 2));
        cumulative.queued_responses.push(
            QueuedResponse::new(
                "another".into(),
                Quarter::new(1).unwrap(),
                DueQuarter::new(2).unwrap(),
                [0, 3, 0],
                3,
            )
            .unwrap(),
        );
        assert!(matches!(
            WorldSnapshot::from_parts(cumulative),
            Err(ContractError::InvalidTreasury)
        ));

        let mut future = seed.clone();
        future.treasury = Treasury::from_parts(97, 0, 0, 0, 3, 0).unwrap();
        future.queued_responses.push(make(2, 3));
        assert!(matches!(
            WorldSnapshot::from_parts(future),
            Err(ContractError::InvalidQuarter)
        ));

        let mut overdue = seed.clone();
        overdue.quarter = Quarter::new(4).unwrap();
        overdue.treasury = Treasury::from_parts(97, 0, 0, 0, 3, 0).unwrap();
        overdue.queued_responses.push(make(1, 2));
        assert!(matches!(
            WorldSnapshot::from_parts(overdue),
            Err(ContractError::InvalidQuarter)
        ));

        let mut deadline = seed;
        deadline.quarter = Quarter::new(3).unwrap();
        deadline.commitments.push(Commitment {
            id: "pending".into(),
            issuer: Ministry::Finance,
            beneficiary: Ministry::Education,
            proposal_version: WireU64::new(0),
            kind: crate::CommitmentKind::Political,
            funded_units: 0,
            trigger: "trigger".into(),
            deadline_quarter: DueQuarter::new(2).unwrap(),
            status: CommitmentStatus::Pending,
            resolution_event: None,
        });
        assert!(matches!(
            WorldSnapshot::from_parts(deadline),
            Err(ContractError::InvalidCondition)
        ));
    }
    #[test]
    fn queued_effect_recheck_validates_every_field_not_only_sum() {
        let mut parts = WorldSnapshot::initial("c".into(), "pack".into(), weights())
            .unwrap()
            .parts;
        parts.treasury = Treasury::from_parts(97, 0, 0, 0, 3, 0).unwrap();
        let valid = QueuedResponse::new(
            "source".into(),
            Quarter::new(1).unwrap(),
            DueQuarter::new(2).unwrap(),
            [3, 0, 0],
            3,
        )
        .unwrap();
        parts.queued_responses.push(valid);
        assert!(WorldSnapshot::from_parts(parts.clone()).is_ok());
        parts.queued_responses[0].source_event_id = "".into();
        assert!(WorldSnapshot::from_parts(parts.clone()).is_err());
        parts.queued_responses[0].source_event_id = "source".into();
        parts.queued_responses[0].due_quarter = DueQuarter::new(1).unwrap();
        assert!(WorldSnapshot::from_parts(parts.clone()).is_err());
        parts.queued_responses[0].due_quarter = DueQuarter::new(2).unwrap();
        parts.queued_responses[0].service_spent = 101;
        assert!(WorldSnapshot::from_parts(parts).is_err());
    }
    #[test]
    fn delivered_event_must_fit_one_family_cap_and_be_positive() {
        let check = |family, delivered| {
            EventEnvelope::new(
                "c".into(),
                "e".into(),
                WireU64::new(1),
                WireU64::new(1),
                Quarter::new(2).unwrap(),
                EventKind::DeliveryRecorded { family, delivered },
            )
        };
        assert!(check(PolicyFamily::Staffing, 12).is_ok());
        assert!(check(PolicyFamily::Repairs, 10).is_ok());
        assert!(check(PolicyFamily::Access, 5).is_ok());
        assert!(check(PolicyFamily::Staffing, 13).is_err());
        assert!(check(PolicyFamily::Repairs, 11).is_err());
        assert!(check(PolicyFamily::Access, 6).is_err());
        assert!(check(PolicyFamily::Access, 0).is_err());
        assert!(check(PolicyFamily::Access, u16::MAX).is_err());
    }

    #[test]
    fn overbudget_draft_does_not_create_world_financial_entries() {
        let allocation = Allocation::draft(60, 40, 10).unwrap();
        assert!(Treasury::initial().can_fund(allocation).is_err());
        let world = WorldSnapshot::initial("campaign".into(), "pack".into(), weights()).unwrap();
        assert_eq!(world.treasury(), Treasury::initial());
    }
}
