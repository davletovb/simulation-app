//! P3 first slice: deterministic, no-I/O, two-round authorization reducer.
//! All admission happens against a staged clone; failures never mutate the
//! campaign, treasury, funded scopes, phases or private minister effort.
//! P4 will own replay serialization; P5 may only supply typed role actions.
use crate::audit::{BeliefLedger, BeliefReceipt, DeliveryAudit};
use crate::world::{
    LedgerReceipt, MeetingPhase, Organisation, OrganisationAccount, QueuedResponse, SupportPpm,
    WorldSnapshot,
};
use crate::{
    Allocation, CapacityCaps, CohortWeights, CommandEnvelope, Commitment, CommitmentKind,
    CommitmentStatus, ContractError, DueQuarter, EffortPermille, FinalPosition, FirstPosition,
    FundedScope, LedgerMove, Ministry, PolicyFamily, Position, PrivateEffortDeclaration, Quarter,
    SIX_QUARTERS, WireU64, deliver_family_quarter, validate_identifier,
};
use std::collections::{BTreeMap, BTreeSet};

const FAMILIES: [PolicyFamily; 3] = [
    PolicyFamily::Staffing,
    PolicyFamily::Repairs,
    PolicyFamily::Access,
];
#[cfg(test)]
const ROLES: [Ministry; 3] = [
    Ministry::Finance,
    Ministry::Education,
    Ministry::Infrastructure,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
    InvalidPhase,
    MeetingAlreadyUsed,
    StaleVersion,
    InvalidVote,
    InsufficientFunding,
    ProtectedCommitment,
    Duplicate,
    WrongAuthority,
    InvalidCondition,
    Arithmetic,
    Contract(ContractError),
}
impl From<ContractError> for Rejection {
    fn from(value: ContractError) -> Self {
        Self::Contract(value)
    }
}

#[derive(Clone)]
struct Meeting {
    id: String,
    version: WireU64,
    allocation: Allocation,
    caps: CapacityCaps,
    first: BTreeMap<Ministry, FirstPosition>,
    final_votes: BTreeMap<Ministry, FinalPosition>,
    pending_efforts: [Option<EffortPermille>; 2],
    amended: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuarterReport {
    pub quarter: Quarter,
    pub delivered: [u16; 3],
    pub reported: [u16; 3],
    pub due_schedule: [u16; 3],
    pub delivered_cumulative: [u16; 3],
    pub service_spent: u16,
    pub weighted_support_ppm: u32,
    pub staff_slowdown_next: bool,
    pub access_bonus_next: bool,
}
#[derive(Clone, Debug)]
pub struct ReducerReceipt {
    pub world_version: u64,
    pub ledger: Vec<LedgerReceipt>,
    pub delivered: [u16; 3],
    pub report: Option<QuarterReport>,
    pub belief_updates: Vec<BeliefReceipt>,
}
#[derive(Clone)]
struct ConditionProgress {
    id: String,
    family: PolicyFamily,
    baseline_delivered: u16,
}
#[derive(Clone)]
pub struct Campaign {
    world: WorldSnapshot,
    meeting: Option<Meeting>,
    meeting_used: bool,
    command_ids: BTreeSet<String>,
    caps: CapacityCaps,
    private_efforts: [EffortPermille; 2],
    slowdown_due: bool,
    bonus_due: bool,
    delivered_lifetime: [u16; 3],
    conditions: Vec<ConditionProgress>,
    relationship: [i16; 3],
    reports: Vec<QuarterReport>,
    scheduled_lifetime: [u16; 3],
    audits: Vec<DeliveryAudit>,
    beliefs: BeliefLedger,
}

pub enum Action {
    Player(CommandEnvelope),
    First(FirstPosition),
    /// Private actual effort must match owner and meeting. Finance has none.
    Final(FinalPosition, Option<PrivateEffortDeclaration>),
    /// Freezes the proposed version after 3 first-round positions. Typed player
    /// command IDs are checked for uniqueness even on this nonfinancial action.
    FreezeFinal {
        command_id: String,
    },
}

impl Campaign {
    pub fn new(
        campaign_id: &str,
        pack_id: &str,
        weights: CohortWeights,
    ) -> Result<Self, Rejection> {
        Ok(Self {
            world: WorldSnapshot::initial(campaign_id.into(), pack_id.into(), weights)?,
            meeting: None,
            meeting_used: false,
            command_ids: BTreeSet::new(),
            caps: CapacityCaps::new(12, 10, 4)?,
            private_efforts: [EffortPermille::new(1000)?, EffortPermille::new(1000)?],
            slowdown_due: false,
            bonus_due: false,
            delivered_lifetime: [0; 3],
            conditions: vec![],
            relationship: [50; 3],
            reports: vec![],
            scheduled_lifetime: [0; 3],
            audits: vec![],
            beliefs: BeliefLedger::default(),
        })
    }
    pub fn world(&self) -> &WorldSnapshot {
        &self.world
    }
    pub fn reports(&self) -> &[QuarterReport] {
        &self.reports
    }
    /// Game-truth audit records are player-private until explicitly disclosed.
    pub fn player_audits(&self) -> &[DeliveryAudit] {
        &self.audits
    }
    pub fn belief_for(&self, ministry: Ministry) -> u32 {
        self.beliefs.belief(ministry)
    }
    pub fn relationship(&self) -> [i16; 3] {
        self.relationship
    }
    pub fn caps(&self) -> CapacityCaps {
        self.caps
    }
    /// Only the matching minister gets access to actual effort; public reports
    /// and player previews never include this field.
    pub(crate) fn private_effort_for(&self, ministry: Ministry) -> Option<u16> {
        match ministry {
            Ministry::Education => Some(self.private_efforts[0].value()),
            Ministry::Infrastructure => Some(self.private_efforts[1].value()),
            Ministry::Finance => None,
        }
    }
    pub fn phase(&self) -> MeetingPhase {
        self.world.phase()
    }
    pub fn proposal_version(&self) -> Option<WireU64> {
        self.meeting.as_ref().map(|m| m.version)
    }
    pub fn role_observation(
        &self,
        ministry: Ministry,
    ) -> Result<crate::RoleObservation, Rejection> {
        let meeting = self.meeting.as_ref().ok_or(Rejection::InvalidPhase)?;
        Ok(self.world.role_observation(
            ministry,
            meeting.id.clone(),
            meeting.version,
            meeting.allocation,
            crate::BeliefPpm::new(self.beliefs.belief(ministry))?,
        )?)
    }
    pub fn apply(&mut self, action: Action) -> Result<ReducerReceipt, Rejection> {
        let mut candidate = self.clone();
        let mut receipt = ReducerReceipt {
            world_version: 0,
            ledger: vec![],
            delivered: [0; 3],
            report: None,
            belief_updates: vec![],
        };
        candidate.apply_staged(action, &mut receipt)?;
        candidate.world.validate()?;
        receipt.world_version = candidate.world.world_version().value();
        *self = candidate;
        Ok(receipt)
    }

    fn apply_staged(
        &mut self,
        action: Action,
        receipt: &mut ReducerReceipt,
    ) -> Result<(), Rejection> {
        match action {
            Action::Player(envelope) => {
                if envelope.campaign_id() != self.world.campaign_id()
                    || envelope.expected_world_version != self.world.world_version()
                {
                    return Err(Rejection::StaleVersion);
                }
                self.record_id(envelope.command_id())?;
                match envelope.command() {
                    crate::PlayerCommand::Propose {
                        meeting_id,
                        allocation,
                    } => {
                        if self.meeting_used
                            || self.meeting.is_some()
                            || self.world.phase() != MeetingPhase::QuarterOpen
                        {
                            return Err(Rejection::MeetingAlreadyUsed);
                        }
                        self.check_funding(*allocation)?;
                        self.meeting = Some(Meeting {
                            id: meeting_id.clone(),
                            version: WireU64::new(1),
                            allocation: *allocation,
                            caps: self.caps,
                            first: BTreeMap::new(),
                            final_votes: BTreeMap::new(),
                            pending_efforts: [None, None],
                            amended: false,
                        });
                        self.meeting_used = true;
                        self.change_phase(MeetingPhase::FirstPositionsFrozen)?;
                    }
                    crate::PlayerCommand::Amend {
                        meeting_id,
                        proposal_version,
                        allocation,
                    } => {
                        if self.world.phase() != MeetingPhase::AmendmentWindow {
                            return Err(Rejection::InvalidPhase);
                        }
                        self.check_funding(*allocation)?;
                        let m = self.meeting.as_mut().ok_or(Rejection::InvalidPhase)?;
                        if &m.id != meeting_id || &m.version != proposal_version {
                            return Err(Rejection::StaleVersion);
                        }
                        if m.amended {
                            return Err(Rejection::InvalidPhase);
                        }
                        m.version = WireU64::new(
                            m.version
                                .value()
                                .checked_add(1)
                                .ok_or(Rejection::Arithmetic)?,
                        );
                        m.allocation = *allocation;
                        m.amended = true;
                    }
                    crate::PlayerCommand::RequestCapacityCap { meeting_id, caps } => {
                        let m = self.meeting.as_mut().ok_or(Rejection::InvalidPhase)?;
                        if m.id != *meeting_id
                            || self.world.phase() != MeetingPhase::FirstPositionsFrozen
                            || !m.first.is_empty()
                        {
                            return Err(Rejection::InvalidPhase);
                        }
                        m.caps = *caps;
                    }
                    crate::PlayerCommand::Ratify {
                        meeting_id,
                        proposal_version,
                    } => {
                        if self.world.phase() != MeetingPhase::FinalPositionsFrozen {
                            return Err(Rejection::InvalidPhase);
                        }
                        let m = self
                            .meeting
                            .as_ref()
                            .ok_or(Rejection::InvalidPhase)?
                            .clone();
                        if m.id != *meeting_id || m.version != *proposal_version {
                            return Err(Rejection::StaleVersion);
                        }
                        if m.final_votes.len() != 3
                            || m.final_votes
                                .values()
                                .filter(|v| v.vote == Position::Support)
                                .count()
                                < 2
                        {
                            return Err(Rejection::InvalidVote);
                        }
                        self.ratify(m, receipt)?;
                        self.change_phase(MeetingPhase::Resolved)?;
                    }
                    crate::PlayerCommand::Defer => {
                        if self.world.phase() == MeetingPhase::QuarterClosed {
                            return Err(Rejection::InvalidPhase);
                        }
                        if self.meeting.is_some() {
                            self.change_phase(MeetingPhase::Resolved)?;
                        }
                    }
                    crate::PlayerCommand::AdvanceQuarter => self.advance(receipt)?,
                    crate::PlayerCommand::OrderAudit {
                        family,
                        inspected_quarter,
                    } => {
                        self.order_audit(*family, *inspected_quarter, receipt)?;
                    }
                    crate::PlayerCommand::DiscloseEvidence {
                        evidence_id,
                        evidence_version,
                        recipients,
                    } => {
                        let evidence = self
                            .audits
                            .iter()
                            .find(|a| a.id == *evidence_id && a.version == *evidence_version)
                            .ok_or(Rejection::InvalidCondition)?
                            .clone();
                        // The recipient list is prevalidated by EvidenceRecipients.
                        for ministry in recipients.roles() {
                            receipt
                                .belief_updates
                                .push(self.beliefs.observe(&evidence, *ministry)?);
                        }
                    }
                }
            }
            Action::First(position) => {
                let m = self.meeting.as_mut().ok_or(Rejection::InvalidPhase)?;
                if self.world.phase() != MeetingPhase::FirstPositionsFrozen
                    || m.id != position.meeting_id()
                    || m.version != position.proposal_version
                    || m.first.contains_key(&position.ministry)
                {
                    return Err(Rejection::InvalidPhase);
                }
                let role = position.ministry;
                m.first.insert(role, position);
                if m.first.len() == 3 {
                    self.change_phase(MeetingPhase::AmendmentWindow)?;
                }
            }
            Action::FreezeFinal { command_id } => {
                self.record_id(&command_id)?;
                if self.world.phase() != MeetingPhase::AmendmentWindow {
                    return Err(Rejection::InvalidPhase);
                }
                self.change_phase(MeetingPhase::FinalPositionsFrozen)?;
            }
            Action::Final(position, effort) => {
                let m = self.meeting.as_mut().ok_or(Rejection::InvalidPhase)?;
                if self.world.phase() != MeetingPhase::FinalPositionsFrozen
                    || m.id != position.meeting_id()
                    || m.version != position.proposal_version
                    || m.final_votes.contains_key(&position.ministry)
                {
                    return Err(Rejection::InvalidPhase);
                }
                let idx = match position.ministry {
                    Ministry::Finance => {
                        if effort.is_some() {
                            return Err(Rejection::WrongAuthority);
                        }
                        None
                    }
                    Ministry::Education => Some(0),
                    Ministry::Infrastructure => Some(1),
                };
                if let Some(index) = idx {
                    let declaration = effort.ok_or(Rejection::WrongAuthority)?;
                    if declaration.ministry() != position.ministry
                        || declaration.meeting_id() != m.id
                    {
                        return Err(Rejection::WrongAuthority);
                    }
                    m.pending_efforts[index] = Some(declaration.effort());
                }
                m.final_votes.insert(position.ministry, position);
            }
        }
        self.bump_version()?;
        Ok(())
    }
    fn bump_version(&mut self) -> Result<(), Rejection> {
        let mut parts = self.world.private_checkpoint_parts();
        parts.world_version = WireU64::new(
            parts
                .world_version
                .value()
                .checked_add(1)
                .ok_or(Rejection::Arithmetic)?,
        );
        self.world = WorldSnapshot::from_parts(parts)?;
        Ok(())
    }
    fn change_phase(&mut self, phase: MeetingPhase) -> Result<(), Rejection> {
        let mut parts = self.world.private_checkpoint_parts();
        parts.phase = phase;
        self.world = WorldSnapshot::from_parts(parts)?;
        Ok(())
    }
    fn record_id(&mut self, id: &str) -> Result<(), Rejection> {
        validate_identifier(id)?;
        if !self.command_ids.insert(id.into()) {
            return Err(Rejection::Duplicate);
        }
        Ok(())
    }
    fn check_funding(&self, proposal: Allocation) -> Result<(), Rejection> {
        if self
            .world
            .commitments()
            .iter()
            .any(|c| c.kind == CommitmentKind::Enforceable && c.status == CommitmentStatus::Pending)
        {
            return Err(Rejection::ProtectedCommitment);
        }
        let stock = u32::from(self.world.treasury().available())
            + FAMILIES
                .iter()
                .map(|f| u32::from(self.world.treasury().encumbered(*f)))
                .sum::<u32>();
        if proposal.total() > stock {
            return Err(Rejection::InsufficientFunding);
        }
        Ok(())
    }
    fn ratify(&mut self, m: Meeting, receipt: &mut ReducerReceipt) -> Result<(), Rejection> {
        self.check_funding(m.allocation)?;
        let mut parts = self.world.private_checkpoint_parts();
        // Replace undelivered unprotected commitments, not consumed cash;
        // terminate scopes before releasing encumbered credits atomically.
        for scope in &mut parts.scopes {
            if !scope.is_terminated() && scope.remaining_units() != 0 {
                let units = scope.remaining_units();
                let family = scope.family();
                scope.terminate();
                let before = parts.treasury;
                parts.treasury = before.apply(LedgerMove::Release {
                    family,
                    credits: units,
                })?;
                receipt.ledger.push(LedgerReceipt::new(
                    before,
                    LedgerMove::Release {
                        family,
                        credits: units,
                    },
                    parts.treasury,
                )?);
            }
        }
        for family in FAMILIES {
            let units = m.allocation.units(family);
            if units == 0 {
                continue;
            }
            let movement = LedgerMove::Reserve {
                family,
                credits: units,
            };
            let before = parts.treasury;
            parts.treasury = before.apply(movement)?;
            receipt
                .ledger
                .push(LedgerReceipt::new(before, movement, parts.treasury)?);
            let key = match family {
                PolicyFamily::Staffing => "staff",
                PolicyFamily::Repairs => "repairs",
                PolicyFamily::Access => "access",
            };
            parts.scopes.push(FundedScope::new(
                format!("{}-v{}-{key}", m.id, m.version.value()),
                family,
                self.world.quarter(),
                units,
            )?);
        }
        let mut condition_ids = BTreeSet::new();
        let mut protected = [0_u16; 3];
        for vote in m
            .final_votes
            .values()
            .filter(|v| v.vote == Position::Support)
        {
            for condition in vote.conditions() {
                if condition.due().value() < u16::from(self.world.quarter().value())
                    || condition.due().value() > u16::from(SIX_QUARTERS)
                    || !condition_ids.insert(condition.kind_id().to_string())
                {
                    return Err(Rejection::InvalidCondition);
                }
                let ix = family_index(condition.family());
                protected[ix] = protected[ix]
                    .checked_add(condition.units())
                    .ok_or(Rejection::Arithmetic)?;
                if protected[ix] > m.allocation.units(condition.family()) {
                    return Err(Rejection::InvalidCondition);
                }
                let commitment = Commitment {
                    id: format!("{}-{}", m.id, condition.kind_id()),
                    issuer: vote.ministry,
                    beneficiary: vote.ministry,
                    proposal_version: m.version,
                    kind: CommitmentKind::Enforceable,
                    funded_units: condition.units(),
                    trigger: condition.kind_id().into(),
                    deadline_quarter: condition.due(),
                    status: CommitmentStatus::Pending,
                    resolution_event: None,
                };
                commitment.validate()?;
                self.conditions.push(ConditionProgress {
                    id: commitment.id.clone(),
                    family: condition.family(),
                    baseline_delivered: self.delivered_lifetime[ix],
                });
                parts.commitments.push(commitment);
            }
        }
        self.caps = m.caps;
        for (index, maybe) in m.pending_efforts.into_iter().enumerate() {
            if let Some(effort) = maybe {
                self.private_efforts[index] = effort;
            }
        }
        self.world = WorldSnapshot::from_parts(parts)?;
        Ok(())
    }
    fn order_audit(
        &mut self,
        family: PolicyFamily,
        inspected: Quarter,
        receipt: &mut ReducerReceipt,
    ) -> Result<(), Rejection> {
        // Audit only a committed historical quarter starting in Q3. A minister
        // cannot request it, and nothing here reveals another minister's effort.
        if self.world.quarter().value() < 3
            || inspected > self.world.quarter()
            || self.world.phase() == MeetingPhase::QuarterClosed
        {
            return Err(Rejection::InvalidPhase);
        }
        let report = self
            .reports
            .iter()
            .find(|r| r.quarter == inspected)
            .ok_or(Rejection::InvalidCondition)?;
        let ix = family_index(family);
        if report.due_schedule[ix] == 0 {
            return Err(Rejection::InvalidCondition);
        }
        let cost = if self.audits.is_empty() { 0 } else { 2 };
        if cost > 0 {
            let mut parts = self.world.private_checkpoint_parts();
            let movement = LedgerMove::SpendAudit { credits: cost };
            let before = parts.treasury;
            parts.treasury = before.apply(movement)?;
            receipt
                .ledger
                .push(LedgerReceipt::new(before, movement, parts.treasury)?);
            self.world = WorldSnapshot::from_parts(parts)?;
        }
        let number = self
            .audits
            .iter()
            .filter(|a| a.family == family && a.inspected_quarter == inspected)
            .count()
            + 1;
        let key = match family {
            PolicyFamily::Staffing => "staffing",
            PolicyFamily::Repairs => "repairs",
            PolicyFamily::Access => "access",
        };
        let evidence = DeliveryAudit {
            id: format!("audit-q{}-{key}-v{number}", inspected.value()),
            version: WireU64::new(u64::try_from(number).map_err(|_| Rejection::Arithmetic)?),
            family,
            inspected_quarter: inspected,
            scheduled_cumulative: report.due_schedule[ix],
            reported_cumulative: report.reported[ix],
            inspected_cumulative: report.delivered_cumulative[ix],
            issuance_world_version: self.world.world_version(),
            cost,
        };
        evidence.validate()?;
        self.audits.push(evidence);
        Ok(())
    }
    fn advance(&mut self, receipt: &mut ReducerReceipt) -> Result<(), Rejection> {
        if self.meeting.is_some() && self.world.phase() != MeetingPhase::Resolved {
            return Err(Rejection::InvalidPhase);
        }
        let q = self.world.quarter();
        if self.world.phase() == MeetingPhase::QuarterClosed {
            return Err(Rejection::InvalidPhase);
        }
        let mut parts = self.world.private_checkpoint_parts();
        let caps = [self.caps.staff(), self.caps.repairs(), self.caps.access()];
        let private_staff =
            self.private_efforts[0]
                .value()
                .min(if self.slowdown_due { 800 } else { 1000 });
        let efforts = [private_staff, self.private_efforts[1].value(), 1000];
        let mut delivered = [0_u16; 3];
        let mut scheduled = [0_u16; 3];
        let mut newly_spent = 0_u16;
        for (ix, family) in FAMILIES.iter().enumerate() {
            let indexes: Vec<usize> = parts
                .scopes
                .iter()
                .enumerate()
                .filter(|(_, s)| s.family() == *family)
                .map(|(i, _)| i)
                .collect();
            let mut scoped: Vec<FundedScope> =
                indexes.iter().map(|i| parts.scopes[*i].clone()).collect();
            // Predict the authorized due schedule using the exact same shared
            // allocator and fractional carry, but with full permitted effort.
            let mut hypothetical = scoped.clone();
            let due_receipt = deliver_family_quarter(
                *family,
                caps[ix],
                ix == 2 && self.bonus_due,
                EffortPermille::new(1000)?,
                DueQuarter::from(q),
                &mut hypothetical,
            )?;
            scheduled[ix] = due_receipt.total_delivered;
            self.scheduled_lifetime[ix] = self.scheduled_lifetime[ix]
                .checked_add(scheduled[ix])
                .ok_or(Rejection::Arithmetic)?;
            let delivery = deliver_family_quarter(
                *family,
                caps[ix],
                ix == 2 && self.bonus_due,
                EffortPermille::new(efforts[ix])?,
                DueQuarter::from(q),
                &mut scoped,
            )?;
            delivered[ix] = delivery.total_delivered;
            for (index, updated) in indexes.into_iter().zip(scoped) {
                parts.scopes[index] = updated;
            }
            if delivery.total_delivered > 0 {
                let movement = LedgerMove::SpendService {
                    family: *family,
                    credits: delivery.total_delivered,
                };
                let before = parts.treasury;
                parts.treasury = before.apply(movement)?;
                receipt
                    .ledger
                    .push(LedgerReceipt::new(before, movement, parts.treasury)?);
                newly_spent = newly_spent
                    .checked_add(delivery.total_delivered)
                    .ok_or(Rejection::Arithmetic)?;
                self.delivered_lifetime[ix] = self.delivered_lifetime[ix]
                    .checked_add(delivery.total_delivered)
                    .ok_or(Rejection::Arithmetic)?;
            }
        }
        // Apply prior response events due NOW, after this quarter's delivery.
        // The response for newly delivered units may first affect next quarter.
        let mut due = vec![];
        parts.queued_responses.retain(|x| {
            if x.due_quarter().value() == u16::from(q.value()) {
                due.push(x.clone());
                false
            } else {
                true
            }
        });
        for response in &due {
            for ix in 0..6 {
                let delta = cohort_response(ix, response.delivered())?;
                let previous = i64::from(parts.support[ix].value());
                let updated = (previous + delta).clamp(0, 1_000_000);
                parts.support[ix] =
                    SupportPpm::new(u32::try_from(updated).map_err(|_| Rejection::Arithmetic)?)?;
            }
        }
        if newly_spent > 0 {
            parts.queued_responses.push(QueuedResponse::new(
                format!("delivery-q{}", q.value()),
                q,
                DueQuarter::new(u16::from(q.value()) + 1)?,
                delivered,
                newly_spent,
            )?);
        }
        // Resolve deadline conditions ONCE, before changing the quarter.
        for commitment in &mut parts.commitments {
            if commitment.status != CommitmentStatus::Pending
                || commitment.deadline_quarter.value() != u16::from(q.value())
            {
                continue;
            }
            let progress = self
                .conditions
                .iter()
                .find(|x| x.id == commitment.id)
                .ok_or(Rejection::InvalidCondition)?;
            let actual = self.delivered_lifetime[family_index(progress.family)]
                .checked_sub(progress.baseline_delivered)
                .ok_or(Rejection::Arithmetic)?;
            let honored = actual >= commitment.funded_units;
            commitment.status = if honored {
                CommitmentStatus::Honored
            } else {
                CommitmentStatus::Broken
            };
            commitment.resolution_event =
                Some(format!("deadline-q{}-{}", q.value(), commitment.id));
            let ix = role_index(commitment.issuer);
            self.relationship[ix] =
                (self.relationship[ix] + if honored { 2 } else { -5 }).clamp(0, 100);
        }
        // Two distinct fictional organizations regenerate at end of advance.
        let funded_staffing = parts
            .treasury
            .encumbered(PolicyFamily::Staffing)
            .checked_add(self.delivered_lifetime[0])
            .ok_or(Rejection::Arithmetic)?;
        let staff_resources = parts.organisations[0].resources().saturating_add(2).min(20);
        let next_slowdown = funded_staffing < 50 && staff_resources >= 4;
        parts.organisations[0] = OrganisationAccount::new(
            Organisation::SchoolWorkforceAssociation,
            staff_resources - if next_slowdown { 4 } else { 0 },
        )?;
        let access_resources = parts.organisations[1].resources().saturating_add(1).min(12);
        let next_bonus = parts.treasury.encumbered(PolicyFamily::Access) > 0
            && self.delivered_lifetime[2] + parts.treasury.encumbered(PolicyFamily::Access) >= 10
            && access_resources > 0;
        parts.organisations[1] = OrganisationAccount::new(
            Organisation::CommunityAccessNetwork,
            access_resources - u32::from(next_bonus),
        )?;
        self.slowdown_due = next_slowdown;
        self.bonus_due = next_bonus;
        let mut weighted = 0_u128;
        for ix in 0..6 {
            weighted += u128::from(parts.support[ix].value())
                * u128::from(parts.cohort_weights.parts_ppm()[ix]);
        }
        let aggregate = u32::try_from(weighted / 1_000_000).map_err(|_| Rejection::Arithmetic)?;
        let reported = [
            self.delivered_lifetime[0]
                .saturating_add(if self.private_efforts[0].value() < 900 {
                    2
                } else {
                    0
                })
                .min(
                    self.delivered_lifetime[0] + parts.treasury.encumbered(PolicyFamily::Staffing),
                ),
            self.delivered_lifetime[1]
                .saturating_add(if self.private_efforts[1].value() < 900 {
                    3
                } else {
                    0
                })
                .min(self.delivered_lifetime[1] + parts.treasury.encumbered(PolicyFamily::Repairs)),
            self.delivered_lifetime[2],
        ];
        let report = QuarterReport {
            quarter: q,
            delivered,
            reported,
            due_schedule: self.scheduled_lifetime,
            delivered_cumulative: self.delivered_lifetime,
            service_spent: newly_spent,
            weighted_support_ppm: aggregate,
            staff_slowdown_next: next_slowdown,
            access_bonus_next: next_bonus,
        };
        self.reports.push(report.clone());
        receipt.delivered = delivered;
        receipt.report = Some(report);
        if q.value() < SIX_QUARTERS {
            parts.quarter = Quarter::new(q.value() + 1)?;
            parts.phase = MeetingPhase::QuarterOpen;
        } else {
            parts.phase = MeetingPhase::QuarterClosed;
        }
        self.meeting = None;
        self.meeting_used = false;
        self.world = WorldSnapshot::from_parts(parts)?;
        Ok(())
    }
}
const fn family_index(f: PolicyFamily) -> usize {
    match f {
        PolicyFamily::Staffing => 0,
        PolicyFamily::Repairs => 1,
        PolicyFamily::Access => 2,
    }
}
const fn role_index(m: Ministry) -> usize {
    match m {
        Ministry::Finance => 0,
        Ministry::Education => 1,
        Ministry::Infrastructure => 2,
    }
}
/// Six authored, non-empirical, floor-rounded response kernels. Integer-only.
fn cohort_response(ix: usize, delivered: [u16; 3]) -> Result<i64, Rejection> {
    let geography: [[i128; 4]; 2] = [[100, 100, 100, 100], [80, 130, 140, 100]];
    let income: [[i128; 4]; 3] = [
        [120, 90, 130, 130],
        [100, 100, 100, 100],
        [80, 110, 70, 150],
    ];
    let base = [500_i128, 400, 750, 400];
    let geo = ix / 3;
    let inc = ix % 3;
    let mut terms = [0_i128; 4];
    for channel in 0..4 {
        terms[channel] = base[channel]
            .checked_mul(geography[geo][channel])
            .and_then(|v| v.checked_mul(income[inc][channel]))
            .ok_or(Rejection::Arithmetic)?
            / 10_000;
    }
    let spent: i128 = delivered.iter().map(|v| i128::from(*v)).sum();
    let result = terms[0] * i128::from(delivered[0])
        + terms[1] * i128::from(delivered[1])
        + terms[2] * i128::from(delivered[2])
        - terms[3] * spent;
    i64::try_from(result.clamp(-8000, 8000)).map_err(|_| Rejection::Arithmetic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CounterproposalOrigin, PlayerCommand, PreferredAllocation};

    fn campaign() -> Campaign {
        Campaign::new(
            "case",
            "england-school-delivery-v1",
            CohortWeights::new([260000, 320000, 220000, 70000, 80000, 50000]).unwrap(),
        )
        .unwrap()
    }
    fn player(c: &Campaign, id: &str, command: PlayerCommand) -> Action {
        Action::Player(
            CommandEnvelope::new("case".into(), id.into(), c.world().world_version(), command)
                .unwrap(),
        )
    }
    fn open(c: &mut Campaign, allocation: Allocation) {
        c.apply(player(
            c,
            "propose",
            PlayerCommand::Propose {
                meeting_id: "q1".into(),
                allocation,
            },
        ))
        .unwrap();
    }
    fn first(c: &mut Campaign, ministry: Ministry) {
        let pos = FirstPosition::new(
            "q1".into(),
            c.proposal_version().unwrap(),
            ministry,
            Position::Support,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
            CounterproposalOrigin::Menu("balanced".into()),
        )
        .unwrap();
        c.apply(Action::First(pos)).unwrap();
    }
    fn final_vote(c: &mut Campaign, ministry: Ministry, vote: Position, effort: u16) {
        let pos = FinalPosition::new(
            "q1".into(),
            c.proposal_version().unwrap(),
            ministry,
            vote,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
        )
        .unwrap();
        let private = if ministry == Ministry::Finance {
            None
        } else {
            Some(
                PrivateEffortDeclaration::new(
                    "q1".into(),
                    ministry,
                    EffortPermille::new(effort).unwrap(),
                )
                .unwrap(),
            )
        };
        c.apply(Action::Final(pos, private)).unwrap();
    }
    fn votes(c: &mut Campaign, support: [bool; 3]) {
        for m in ROLES {
            first(c, m);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        for (i, m) in ROLES.into_iter().enumerate() {
            final_vote(
                c,
                m,
                if support[i] {
                    Position::Support
                } else {
                    Position::Oppose
                },
                1000,
            );
        }
    }
    #[test]
    fn approved_package_moves_funds_only_after_three_final_votes() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        assert_eq!(c.world().treasury().available(), 100);
        votes(&mut c, [true, true, false]);
        assert_eq!(c.world().treasury().available(), 100);
        let r = c
            .apply(player(
                &c,
                "ratify",
                PlayerCommand::Ratify {
                    meeting_id: "q1".into(),
                    proposal_version: c.proposal_version().unwrap(),
                },
            ))
            .unwrap();
        assert_eq!(r.ledger.len(), 3);
        assert_eq!(c.world().treasury().available(), 0);
        assert_eq!(c.world().treasury().encumbered(PolicyFamily::Repairs), 40);
        c.world().validate().unwrap();
    }
    #[test]
    fn failed_vote_does_not_block_explicit_advance_or_spend() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        votes(&mut c, [false, true, false]);
        let before = c.world().world_version().value();
        assert!(matches!(
            c.apply(player(
                &c,
                "ratify",
                PlayerCommand::Ratify {
                    meeting_id: "q1".into(),
                    proposal_version: c.proposal_version().unwrap(),
                }
            )),
            Err(Rejection::InvalidVote)
        ));
        assert_eq!(c.world().world_version().value(), before);
        assert_eq!(c.world().treasury().available(), 100);
        c.apply(player(&c, "defer", PlayerCommand::Defer)).unwrap();
        c.apply(player(&c, "advance", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.world().quarter().value(), 2);
        assert_eq!(c.world().treasury().service_spent(), 0);
    }
    #[test]
    fn ratified_access_spends_once_and_delays_support_to_next_quarter() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        votes(&mut c, [true, true, true]);
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        let a = c
            .apply(player(&c, "advance", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(a.delivered, [0, 0, 4]);
        assert_eq!(c.world().treasury().service_spent(), 4);
        assert!(c.world().support().iter().all(|v| v.value() == 500000));
        assert_eq!(c.world().quarter().value(), 2);
        let b = c
            .apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(b.delivered[0], 12);
        assert!(c.world().support().iter().any(|v| v.value() != 500000));
        assert_eq!(
            c.world().treasury().service_spent(),
            4 + b.delivered.iter().sum::<u16>()
        );
        c.world().validate().unwrap();
    }
    #[test]
    fn stale_actions_and_aborted_amendments_do_not_mutate_ledger() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        let version = c.world().world_version();
        let bad = Action::Player(
            CommandEnvelope::new(
                "case".into(),
                "bad".into(),
                WireU64::new(0),
                PlayerCommand::AdvanceQuarter,
            )
            .unwrap(),
        );
        assert_eq!(c.apply(bad).err(), Some(Rejection::StaleVersion));
        assert_eq!(c.world().world_version(), version);
        assert_eq!(c.world().treasury().available(), 100);
        assert!(
            c.apply(player(&c, "early", PlayerCommand::AdvanceQuarter))
                .is_err()
        );
        assert_eq!(c.world().world_version(), version);
    }
    #[test]
    fn unaffordable_draft_never_enters_meeting() {
        let mut c = campaign();
        let before = c.world().world_version();
        assert_eq!(
            c.apply(player(
                &c,
                "bad",
                PlayerCommand::Propose {
                    meeting_id: "q1".into(),
                    allocation: Allocation::draft(60, 40, 10).unwrap(),
                }
            ))
            .err(),
            Some(Rejection::InsufficientFunding)
        );
        assert_eq!(c.world().world_version(), before);
        assert_eq!(c.phase(), MeetingPhase::QuarterOpen);
    }
    #[test]
    fn low_effort_produces_stable_fractional_carry_without_spending_unfinished_work() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        for m in ROLES {
            first(&mut c, m);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        final_vote(&mut c, Ministry::Finance, Position::Support, 1000);
        final_vote(&mut c, Ministry::Education, Position::Support, 500);
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 500);
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        c.apply(player(&c, "advance", PlayerCommand::AdvanceQuarter))
            .unwrap();
        let q2 = c
            .apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(q2.delivered[0], 6);
        assert_eq!(c.private_effort_for(Ministry::Education), Some(500));
        assert_eq!(c.private_effort_for(Ministry::Finance), None);
        assert!(c.reports()[1].reported[0] > c.reports()[1].delivered[0]);
    }
    #[test]
    fn no_policy_horizon_does_not_advance_beyond_q6() {
        let mut c = campaign();
        for q in 1..=6 {
            c.apply(player(
                &c,
                &format!("advance{q}"),
                PlayerCommand::AdvanceQuarter,
            ))
            .unwrap();
        }
        assert_eq!(c.world().quarter().value(), 6);
        assert_eq!(c.phase(), MeetingPhase::QuarterClosed);
        assert!(
            c.apply(player(&c, "too-late", PlayerCommand::AdvanceQuarter))
                .is_err()
        );
        assert_eq!(c.world().treasury().available(), 100);
    }
    #[test]
    fn signed_coefficients_keep_losing_cells_negative() {
        assert!(cohort_response(2, [1, 0, 0]).unwrap() < 0);
        assert!(cohort_response(3, [1, 0, 0]).unwrap() < 0);
        assert!(cohort_response(0, [0, 0, 1]).unwrap() > 0);
    }
}
