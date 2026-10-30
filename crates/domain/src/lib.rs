//! Pure, bounded domain contracts for Myth Maker.
//!
//! This crate does not perform network, Hiyal, storage or LLM operations.
//! A family-wide delivery primitive prevents duplicate capacity when later
//! funding creates multiple eligible scopes in the same quarter.
#![forbid(unsafe_code)]

pub mod preview;
pub mod world;

use std::collections::BTreeSet;
use std::fmt;

pub const SCHEMA_VERSION: u16 = 1;
pub const PILOT_CREDITS: u16 = 100;
pub const SIX_QUARTERS: u8 = 6;
pub const SHARE_DENOMINATOR: u32 = 1_000_000;
pub const EFFORT_DENOMINATOR: u16 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractError {
    InvalidQuarter,
    InvalidCreditAmount,
    InvalidEffort,
    InvalidBelief,
    InvalidPopulationWeights,
    InvalidDecimal,
    InvalidTreasury,
    InvalidSchema,
    InvalidMinistry,
    InvalidRecipients,
    InvalidScope,
    InvalidIdentifier,
    InvalidRationale,
    InvalidCondition,
    DuplicateScope,
    InsufficientAvailableFunds,
    ArithmeticOverflow,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ContractError {}

// Domain-owned validators: both scripted commands and JSON input must use them.
// IDs are bounded by UTF-8 bytes so the same limits apply on every platform.
pub fn validate_identifier(value: &str) -> Result<(), ContractError> {
    if value.is_empty()
        || value.len() > 128
        || value.trim() != value
        || value.chars().any(|c| c.is_control() || forbidden_format(c))
    {
        Err(ContractError::InvalidIdentifier)
    } else {
        Ok(())
    }
}
fn forbidden_format(c: char) -> bool {
    matches!(c,
        '\u{061C}' | '\u{200B}'..='\u{200F}' |
        '\u{202A}'..='\u{202E}' | '\u{2060}' |
        '\u{2066}'..='\u{2069}' | '\u{FEFF}'
    )
}
pub fn validate_rationale(value: &str) -> Result<(), ContractError> {
    if value.len() > 1024 || value.chars().any(|c| c.is_control() || forbidden_format(c)) {
        Err(ContractError::InvalidRationale)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Quarter(u8);

impl Quarter {
    pub fn new(value: u8) -> Result<Self, ContractError> {
        if (1..=SIX_QUARTERS).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::InvalidQuarter)
        }
    }
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// Dates after the sixth playable quarter remain representable for lags,
/// conditions and unsettled commitments. Zero is not a valid date.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct DueQuarter(u16);

impl DueQuarter {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if value == 0 {
            Err(ContractError::InvalidQuarter)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn value(self) -> u16 {
        self.0
    }
}

impl From<Quarter> for DueQuarter {
    fn from(value: Quarter) -> Self {
        Self(u16::from(value.value()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Ministry {
    Finance,
    Education,
    Infrastructure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PolicyFamily {
    Staffing,
    Repairs,
    Access,
}

impl PolicyFamily {
    pub const fn nominal_capacity(self) -> u16 {
        match self {
            Self::Staffing => 12,
            Self::Repairs => 10,
            Self::Access => 4,
        }
    }
    pub const fn lag_quarters(self) -> u16 {
        match self {
            Self::Staffing => 1,
            Self::Repairs => 2,
            Self::Access => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Residence {
    Urban,
    Rural,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum IncomeBand {
    Low,
    Middle,
    High,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CohortId {
    pub residence: Residence,
    pub income: IncomeBand,
}

pub const COHORT_ORDER: [CohortId; 6] = [
    CohortId {
        residence: Residence::Urban,
        income: IncomeBand::Low,
    },
    CohortId {
        residence: Residence::Urban,
        income: IncomeBand::Middle,
    },
    CohortId {
        residence: Residence::Urban,
        income: IncomeBand::High,
    },
    CohortId {
        residence: Residence::Rural,
        income: IncomeBand::Low,
    },
    CohortId {
        residence: Residence::Rural,
        income: IncomeBand::Middle,
    },
    CohortId {
        residence: Residence::Rural,
        income: IncomeBand::High,
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CohortWeights {
    parts_ppm: [u32; 6],
}
impl CohortWeights {
    pub fn new(parts_ppm: [u32; 6]) -> Result<Self, ContractError> {
        let total: u64 = parts_ppm.iter().map(|v| u64::from(*v)).sum();
        if total != u64::from(SHARE_DENOMINATOR) {
            return Err(ContractError::InvalidPopulationWeights);
        }
        Ok(Self { parts_ppm })
    }
    pub const fn parts_ppm(&self) -> &[u32; 6] {
        &self.parts_ppm
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EffortPermille(u16);
impl EffortPermille {
    pub fn new(value: u16) -> Result<Self, ContractError> {
        if value > EFFORT_DENOMINATOR {
            Err(ContractError::InvalidEffort)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn value(self) -> u16 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct BeliefPpm(u32);
impl BeliefPpm {
    pub fn new(value: u32) -> Result<Self, ContractError> {
        if value > SHARE_DENOMINATOR {
            Err(ContractError::InvalidBelief)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// 64-bit versions/IDs exposed over JSON use canonical *decimal strings*.
/// A later Serde implementation must serialize this wrapper as a JSON string,
/// never derive its default numeric encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct WireU64(u64);
impl WireU64 {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn value(self) -> u64 {
        self.0
    }
    pub fn from_decimal(value: &str) -> Result<Self, ContractError> {
        if value.is_empty()
            || (value.len() > 1 && value.starts_with('0'))
            || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(ContractError::InvalidDecimal);
        }
        value
            .parse::<u64>()
            .map(Self)
            .map_err(|_| ContractError::InvalidDecimal)
    }
    pub fn to_decimal(self) -> String {
        self.0.to_string()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Allocation {
    staff: u16,
    repairs: u16,
    access: u16,
}
impl Allocation {
    /// Allows an over-budget *draft* such as 60+40+10, but no individual
    /// line can exceed the entire campaign's fictional 100-credit allocation.
    pub fn draft(staff: u16, repairs: u16, access: u16) -> Result<Self, ContractError> {
        if [staff, repairs, access].iter().any(|v| *v > PILOT_CREDITS) {
            Err(ContractError::InvalidCreditAmount)
        } else {
            Ok(Self {
                staff,
                repairs,
                access,
            })
        }
    }
    pub const fn staff(self) -> u16 {
        self.staff
    }
    pub const fn repairs(self) -> u16 {
        self.repairs
    }
    pub const fn access(self) -> u16 {
        self.access
    }
    pub fn total(self) -> u32 {
        u32::from(self.staff) + u32::from(self.repairs) + u32::from(self.access)
    }
    pub fn units(self, family: PolicyFamily) -> u16 {
        match family {
            PolicyFamily::Staffing => self.staff,
            PolicyFamily::Repairs => self.repairs,
            PolicyFamily::Access => self.access,
        }
    }
}

/// The invalid-draft marker is computed from a mechanical affordability
/// snapshot, not supplied by an LLM or arbitrary caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationValidity {
    Feasible,
    InvalidDraft,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreferredAllocation {
    draft: Allocation,
    validity: AllocationValidity,
}
impl PreferredAllocation {
    pub fn assess(draft: Allocation, eligible_credits: u16) -> Result<Self, ContractError> {
        if eligible_credits > PILOT_CREDITS {
            return Err(ContractError::InvalidCreditAmount);
        }
        let validity = if draft.total() <= u32::from(eligible_credits) {
            AllocationValidity::Feasible
        } else {
            AllocationValidity::InvalidDraft
        };
        Ok(Self { draft, validity })
    }
    pub const fn allocation(self) -> Allocation {
        self.draft
    }
    pub const fn validity(self) -> AllocationValidity {
        self.validity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityCaps {
    staff: u16,
    repairs: u16,
    access: u16,
}
impl CapacityCaps {
    pub fn new(staff: u16, repairs: u16, access: u16) -> Result<Self, ContractError> {
        if staff > 12 || repairs > 10 || access > 4 {
            Err(ContractError::InvalidCreditAmount)
        } else {
            Ok(Self {
                staff,
                repairs,
                access,
            })
        }
    }
    pub const fn staff(self) -> u16 {
        self.staff
    }
    pub const fn repairs(self) -> u16 {
        self.repairs
    }
    pub const fn access(self) -> u16 {
        self.access
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Treasury {
    available: u16,
    encumbered_staff: u16,
    encumbered_repairs: u16,
    encumbered_access: u16,
    service_spent: u16,
    audit_spent: u16,
}
impl Treasury {
    pub const fn initial() -> Self {
        Self {
            available: PILOT_CREDITS,
            encumbered_staff: 0,
            encumbered_repairs: 0,
            encumbered_access: 0,
            service_spent: 0,
            audit_spent: 0,
        }
    }
    pub fn from_parts(
        available: u16,
        encumbered_staff: u16,
        encumbered_repairs: u16,
        encumbered_access: u16,
        service_spent: u16,
        audit_spent: u16,
    ) -> Result<Self, ContractError> {
        let result = Self {
            available,
            encumbered_staff,
            encumbered_repairs,
            encumbered_access,
            service_spent,
            audit_spent,
        };
        result.validate()?;
        Ok(result)
    }
    pub const fn available(self) -> u16 {
        self.available
    }
    pub const fn service_spent(self) -> u16 {
        self.service_spent
    }
    pub const fn audit_spent(self) -> u16 {
        self.audit_spent
    }
    pub fn encumbered(self, family: PolicyFamily) -> u16 {
        match family {
            PolicyFamily::Staffing => self.encumbered_staff,
            PolicyFamily::Repairs => self.encumbered_repairs,
            PolicyFamily::Access => self.encumbered_access,
        }
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        let sum = [
            self.available,
            self.encumbered_staff,
            self.encumbered_repairs,
            self.encumbered_access,
            self.service_spent,
            self.audit_spent,
        ]
        .iter()
        .try_fold(0_u32, |acc, value| acc.checked_add(u32::from(*value)))
        .ok_or(ContractError::ArithmeticOverflow)?;
        if sum == u32::from(PILOT_CREDITS) {
            Ok(())
        } else {
            Err(ContractError::InvalidTreasury)
        }
    }
    pub fn can_fund(&self, additional: Allocation) -> Result<(), ContractError> {
        self.validate()?;
        if additional.total() > u32::from(self.available) {
            Err(ContractError::InsufficientAvailableFunds)
        } else {
            Ok(())
        }
    }
    /// Primitive receipt move; the P3 reducer must first enforce votes,
    /// authority, conditions, scope identity and one-time payout.
    pub fn record_delivery(
        &mut self,
        family: PolicyFamily,
        units: u16,
    ) -> Result<(), ContractError> {
        self.validate()?;
        let current = self.encumbered(family);
        let outstanding = current
            .checked_sub(units)
            .ok_or(ContractError::InvalidTreasury)?;
        let spent = self
            .service_spent
            .checked_add(units)
            .ok_or(ContractError::ArithmeticOverflow)?;
        match family {
            PolicyFamily::Staffing => self.encumbered_staff = outstanding,
            PolicyFamily::Repairs => self.encumbered_repairs = outstanding,
            PolicyFamily::Access => self.encumbered_access = outstanding,
        }
        self.service_spent = spent;
        self.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundedScope {
    id: String,
    family: PolicyFamily,
    activation_quarter: Quarter,
    remaining_units: u16,
    carry_permille: u16,
    blocked: bool,
    terminated: bool,
}
impl FundedScope {
    pub fn new(
        id: String,
        family: PolicyFamily,
        activation_quarter: Quarter,
        funded_units: u16,
    ) -> Result<Self, ContractError> {
        if id.is_empty() || id.len() > 128 || funded_units == 0 || funded_units > PILOT_CREDITS {
            return Err(ContractError::InvalidScope);
        }
        Ok(Self {
            id,
            family,
            activation_quarter,
            remaining_units: funded_units,
            carry_permille: 0,
            blocked: false,
            terminated: false,
        })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub const fn family(&self) -> PolicyFamily {
        self.family
    }
    pub const fn remaining_units(&self) -> u16 {
        self.remaining_units
    }
    pub const fn carry_permille(&self) -> u16 {
        self.carry_permille
    }
    pub fn set_blocked(&mut self, blocked: bool) {
        self.blocked = blocked;
    }
    /// Explicitly removes fractional progress from a terminated scope.
    /// Does not itself release treasury credits: P3 must authorize that step.
    pub fn terminate(&mut self) -> DiscardedWork {
        let receipt = DiscardedWork {
            family: self.family,
            scope_id: self.id.clone(),
            dropped_carry_permille: self.carry_permille,
        };
        self.carry_permille = 0;
        self.terminated = true;
        receipt
    }
    fn eligible(&self, quarter: DueQuarter) -> bool {
        !self.terminated
            && !self.blocked
            && self.remaining_units > 0
            && quarter.value()
                >= u16::from(self.activation_quarter.value()) + self.family.lag_quarters()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscardedWork {
    pub family: PolicyFamily,
    pub scope_id: String,
    pub dropped_carry_permille: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopeDelivery {
    pub scope_id: String,
    pub allocated_capacity: u16,
    pub delivered: u16,
    pub carry_permille: u16,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FamilyDeliveryReceipt {
    pub family: PolicyFamily,
    pub authorized_capacity: u16,
    pub total_allocated: u16,
    pub total_delivered: u16,
    pub scopes: Vec<ScopeDelivery>,
}

/// Applies the *one* quarterly family pool across all eligible scopes.
/// Clone-before-apply prevents partial state mutation on a failed validation.
pub fn deliver_family_quarter(
    family: PolicyFamily,
    authorized_capacity: u16,
    access_bonus: bool,
    effort: EffortPermille,
    quarter: DueQuarter,
    scopes: &mut [FundedScope],
) -> Result<FamilyDeliveryReceipt, ContractError> {
    if authorized_capacity > family.nominal_capacity()
        || (access_bonus && family != PolicyFamily::Access)
    {
        return Err(ContractError::InvalidCreditAmount);
    }
    let limit = authorized_capacity
        .checked_add(u16::from(access_bonus))
        .ok_or(ContractError::ArithmeticOverflow)?;
    let mut ids = BTreeSet::new();
    if scopes
        .iter()
        .any(|s| s.family != family || !ids.insert(s.id.clone()))
    {
        return Err(ContractError::DuplicateScope);
    }
    let mut staged = scopes.to_vec();
    let mut sorted: Vec<usize> = (0..staged.len()).collect();
    sorted.sort_by(|a, b| {
        staged[*a]
            .activation_quarter
            .cmp(&staged[*b].activation_quarter)
            .then_with(|| staged[*a].id.cmp(&staged[*b].id))
    });
    let mut capacity_left = limit;
    let mut total_delivered = 0_u16;
    let mut scope_receipts = Vec::new();
    for index in sorted {
        let scope = &mut staged[index];
        if !scope.eligible(quarter) {
            continue;
        }
        let allocated = scope.remaining_units.min(capacity_left);
        capacity_left -= allocated;
        let work = u32::from(scope.carry_permille)
            .checked_add(u32::from(allocated) * u32::from(effort.value()))
            .ok_or(ContractError::ArithmeticOverflow)?;
        let delivered = u16::try_from(
            (work / u32::from(EFFORT_DENOMINATOR)).min(u32::from(scope.remaining_units)),
        )
        .map_err(|_| ContractError::ArithmeticOverflow)?;
        if delivered > allocated {
            return Err(ContractError::ArithmeticOverflow);
        }
        scope.remaining_units -= delivered;
        scope.carry_permille = if scope.remaining_units == 0 {
            0
        } else {
            u16::try_from(work % u32::from(EFFORT_DENOMINATOR))
                .map_err(|_| ContractError::ArithmeticOverflow)?
        };
        total_delivered = total_delivered
            .checked_add(delivered)
            .ok_or(ContractError::ArithmeticOverflow)?;
        scope_receipts.push(ScopeDelivery {
            scope_id: scope.id.clone(),
            allocated_capacity: allocated,
            delivered,
            carry_permille: scope.carry_permille,
        });
    }
    if total_delivered > limit {
        return Err(ContractError::ArithmeticOverflow);
    }
    scopes.clone_from_slice(&staged);
    Ok(FamilyDeliveryReceipt {
        family,
        authorized_capacity: limit,
        total_allocated: limit - capacity_left,
        total_delivered,
        scopes: scope_receipts,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Position {
    Support,
    Conditional,
    Oppose,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitmentKind {
    Enforceable,
    Political,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitmentStatus {
    Pending,
    Honored,
    Broken,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Commitment {
    pub id: String,
    pub issuer: Ministry,
    pub beneficiary: Ministry,
    pub proposal_version: WireU64,
    pub kind: CommitmentKind,
    pub funded_units: u16,
    pub trigger: String,
    pub deadline_quarter: DueQuarter,
    pub status: CommitmentStatus,
    pub resolution_event: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundedCondition {
    kind_id: String,
    family: PolicyFamily,
    units: u16,
    due: DueQuarter,
}
impl FundedCondition {
    pub fn new(
        kind_id: String,
        family: PolicyFamily,
        units: u16,
        due: DueQuarter,
    ) -> Result<Self, ContractError> {
        validate_identifier(&kind_id)?;
        if units > PILOT_CREDITS {
            return Err(ContractError::InvalidCondition);
        }
        Ok(Self {
            kind_id,
            family,
            units,
            due,
        })
    }
    pub fn kind_id(&self) -> &str {
        &self.kind_id
    }
    pub const fn family(&self) -> PolicyFamily {
        self.family
    }
    pub const fn units(&self) -> u16 {
        self.units
    }
    pub const fn due(&self) -> DueQuarter {
        self.due
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CounterproposalOrigin {
    Menu(String),
    NewTyped,
}

/// Only mechanically assessed counteroffers can appear as preferences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstPosition {
    schema_version: u16,
    meeting_id: String,
    pub proposal_version: WireU64,
    pub ministry: Ministry,
    pub position: Position,
    pub preferred_allocation: PreferredAllocation,
    pub counterproposal_source: CounterproposalOrigin,
    conditions: Vec<FundedCondition>,
    evidence_ids: Vec<String>,
    rationale_summary: String,
}
impl FirstPosition {
    pub fn set_public_details(
        &mut self,
        conditions: Vec<FundedCondition>,
        evidence_ids: Vec<String>,
        rationale: String,
    ) -> Result<(), ContractError> {
        for id in &evidence_ids {
            validate_identifier(id)?;
        }
        validate_rationale(&rationale)?;
        self.conditions = conditions;
        self.evidence_ids = evidence_ids;
        self.rationale_summary = rationale;
        Ok(())
    }
    pub fn rationale_summary(&self) -> &str {
        &self.rationale_summary
    }
    pub fn conditions(&self) -> &[FundedCondition] {
        &self.conditions
    }

    pub fn new(
        meeting_id: String,
        proposal_version: WireU64,
        ministry: Ministry,
        position: Position,
        preferred_allocation: PreferredAllocation,
        counterproposal_source: CounterproposalOrigin,
    ) -> Result<Self, ContractError> {
        validate_identifier(&meeting_id)?;
        if let CounterproposalOrigin::Menu(ref id) = counterproposal_source {
            validate_identifier(id)?;
        }
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            meeting_id,
            proposal_version,
            ministry,
            position,
            preferred_allocation,
            counterproposal_source,
            conditions: vec![],
            evidence_ids: vec![],
            rationale_summary: String::new(),
        })
    }
    pub fn meeting_id(&self) -> &str {
        &self.meeting_id
    }
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalPosition {
    schema_version: u16,
    meeting_id: String,
    pub proposal_version: WireU64,
    pub ministry: Ministry,
    pub vote: Position,
    pub preferred_allocation: PreferredAllocation,
    conditions: Vec<FundedCondition>,
    evidence_ids: Vec<String>,
    rationale_summary: String,
}
impl FinalPosition {
    pub fn set_public_details(
        &mut self,
        conditions: Vec<FundedCondition>,
        evidence_ids: Vec<String>,
        rationale: String,
    ) -> Result<(), ContractError> {
        for id in &evidence_ids {
            validate_identifier(id)?;
        }
        validate_rationale(&rationale)?;
        self.conditions = conditions;
        self.evidence_ids = evidence_ids;
        self.rationale_summary = rationale;
        Ok(())
    }
    pub fn rationale_summary(&self) -> &str {
        &self.rationale_summary
    }
    pub fn conditions(&self) -> &[FundedCondition] {
        &self.conditions
    }

    pub fn new(
        meeting_id: String,
        proposal_version: WireU64,
        ministry: Ministry,
        vote: Position,
        preferred_allocation: PreferredAllocation,
    ) -> Result<Self, ContractError> {
        validate_identifier(&meeting_id)?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            meeting_id,
            proposal_version,
            ministry,
            vote,
            preferred_allocation,
            conditions: vec![],
            evidence_ids: vec![],
            rationale_summary: String::new(),
        })
    }
    pub fn meeting_id(&self) -> &str {
        &self.meeting_id
    }
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
}

/// Not Debug or serializable: stored and routed only through a private role
/// channel. The publicly printable FinalPosition cannot contain actual effort.
#[derive(Clone, Eq, PartialEq)]
pub struct PrivateEffortDeclaration {
    meeting_id: String,
    ministry: Ministry,
    effort: EffortPermille,
}
impl PrivateEffortDeclaration {
    pub fn new(
        meeting_id: String,
        ministry: Ministry,
        effort: EffortPermille,
    ) -> Result<Self, ContractError> {
        validate_identifier(&meeting_id)?;
        if ministry == Ministry::Finance {
            return Err(ContractError::InvalidMinistry);
        }
        Ok(Self {
            meeting_id,
            ministry,
            effort,
        })
    }
    pub const fn ministry(&self) -> Ministry {
        self.ministry
    }
    pub const fn effort(&self) -> EffortPermille {
        self.effort
    }
    pub fn meeting_id(&self) -> &str {
        &self.meeting_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceRecipients {
    roles: Vec<Ministry>,
}
impl EvidenceRecipients {
    pub fn new(mut roles: Vec<Ministry>) -> Result<Self, ContractError> {
        if roles.is_empty() {
            return Err(ContractError::InvalidRecipients);
        }
        roles.sort();
        roles.dedup();
        Ok(Self { roles })
    }
    pub fn roles(&self) -> &[Ministry] {
        &self.roles
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlayerCommand {
    Propose {
        meeting_id: String,
        allocation: Allocation,
    },
    Amend {
        meeting_id: String,
        proposal_version: WireU64,
        allocation: Allocation,
    },
    Ratify {
        meeting_id: String,
        proposal_version: WireU64,
    },
    RequestCapacityCap {
        meeting_id: String,
        caps: CapacityCaps,
    },
    OrderAudit {
        family: PolicyFamily,
        inspected_quarter: Quarter,
    },
    DiscloseEvidence {
        evidence_id: String,
        evidence_version: WireU64,
        recipients: EvidenceRecipients,
    },
    Defer,
    AdvanceQuarter,
}

impl PlayerCommand {
    pub fn validate_ids(&self) -> Result<(), ContractError> {
        match self {
            Self::Propose { meeting_id, .. }
            | Self::Amend { meeting_id, .. }
            | Self::Ratify { meeting_id, .. }
            | Self::RequestCapacityCap { meeting_id, .. } => validate_identifier(meeting_id),
            Self::DiscloseEvidence { evidence_id, .. } => validate_identifier(evidence_id),
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandEnvelope {
    schema_version: u16,
    campaign_id: String,
    command_id: String,
    pub expected_world_version: WireU64,
    command: PlayerCommand,
}
impl CommandEnvelope {
    pub fn new(
        campaign_id: String,
        command_id: String,
        expected_world_version: WireU64,
        command: PlayerCommand,
    ) -> Result<Self, ContractError> {
        validate_identifier(&campaign_id)?;
        validate_identifier(&command_id)?;
        command.validate_ids()?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            campaign_id,
            command_id,
            expected_world_version,
            command,
        })
    }
    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }
    pub fn command_id(&self) -> &str {
        &self.command_id
    }
    pub fn command(&self) -> &PlayerCommand {
        &self.command
    }
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceProvenance {
    pub stable_variable_id: String,
    pub geographic_scope: String,
    pub definition: String,
    pub source_url: String,
    pub publisher: String,
    pub reference_period: String,
    pub release_date: String,
    pub retrieval_date: String,
    pub raw_unit: String,
    pub normalized_unit: String,
    pub transformation: String,
    pub licence_or_terms: String,
    pub sha256_hex: String,
    pub missing_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CountryPack {
    pub schema_version: u16,
    pub country_iso: String,
    pub simulated_jurisdiction: String,
    pub cutoff_utc: String,
    pub locale: String,
    pub evidence: Vec<EvidenceProvenance>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleObservationInput {
    pub campaign_id: String,
    pub meeting_id: String,
    pub proposal_version: WireU64,
    pub ministry: Ministry,
    pub quarter: Quarter,
    pub allocation: Allocation,
    pub available_credits: u16,
    pub belief_ppm: BeliefPpm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleObservation {
    schema_version: u16,
    pub campaign_id: String,
    pub meeting_id: String,
    pub proposal_version: WireU64,
    pub ministry: Ministry,
    pub quarter: Quarter,
    pub allocation: Allocation,
    pub available_credits: u16,
    pub belief_ppm: BeliefPpm,
    pub owned_commitments: Vec<Commitment>,
    pub visible_evidence_ids: Vec<String>,
    // Deliberately no private effort field on this public/view-safe type.
}
impl RoleObservation {
    pub fn new(input: RoleObservationInput) -> Result<Self, ContractError> {
        validate_identifier(&input.campaign_id)?;
        validate_identifier(&input.meeting_id)?;
        if input.available_credits > PILOT_CREDITS {
            return Err(ContractError::InvalidCreditAmount);
        }
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            campaign_id: input.campaign_id,
            meeting_id: input.meeting_id,
            proposal_version: input.proposal_version,
            ministry: input.ministry,
            quarter: input.quarter,
            allocation: input.allocation,
            available_credits: input.available_credits,
            belief_ppm: input.belief_ppm,
            owned_commitments: vec![],
            visible_evidence_ids: vec![],
        })
    }
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_identifier(&self.campaign_id)?;
        validate_identifier(&self.meeting_id)?;
        for id in &self.visible_evidence_ids {
            validate_identifier(id)?;
        }
        if self.available_credits > PILOT_CREDITS {
            return Err(ContractError::InvalidCreditAmount);
        }
        for commitment in &self.owned_commitments {
            validate_identifier(&commitment.id)?;
            validate_identifier(&commitment.trigger)?;
            if commitment.funded_units > PILOT_CREDITS {
                return Err(ContractError::InvalidCondition);
            }
            if let Some(event) = &commitment.resolution_event {
                validate_identifier(event)?;
            }
        }
        Ok(())
    }
}

/// Separately handled, intentionally not Debug or serializable. A private role
/// view is allowed to expose only *that same role's* actual effort.
#[derive(Clone)]
pub struct PrivateRoleObservation {
    public: RoleObservation,
    own_effort: Option<PrivateEffortDeclaration>,
}
impl PrivateRoleObservation {
    pub fn new(
        public: RoleObservation,
        own_effort: Option<PrivateEffortDeclaration>,
    ) -> Result<Self, ContractError> {
        match (public.ministry, own_effort.as_ref()) {
            (Ministry::Finance, None) => {}
            (Ministry::Finance, Some(_)) => return Err(ContractError::InvalidMinistry),
            (m, Some(decl)) if decl.ministry() == m && decl.meeting_id() == public.meeting_id => {}
            _ => return Err(ContractError::InvalidMinistry),
        }
        Ok(Self { public, own_effort })
    }
    pub fn public(&self) -> &RoleObservation {
        &self.public
    }
    pub fn own_effort(&self) -> Option<EffortPermille> {
        self.own_effort
            .as_ref()
            .map(PrivateEffortDeclaration::effort)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(id: &str, family: PolicyFamily, at: u8, units: u16) -> FundedScope {
        FundedScope::new(id.to_string(), family, Quarter::new(at).unwrap(), units).unwrap()
    }
    fn effort(value: u16) -> EffortPermille {
        EffortPermille::new(value).unwrap()
    }
    fn due(value: u16) -> DueQuarter {
        DueQuarter::new(value).unwrap()
    }

    #[test]
    fn domain_rejects_bypass_for_conditions_and_spoofed_text() {
        assert_eq!(
            FundedCondition::new(
                "".into(),
                PolicyFamily::Staffing,
                2,
                DueQuarter::new(1).unwrap()
            ),
            Err(ContractError::InvalidIdentifier)
        );
        assert_eq!(
            FundedCondition::new(
                "ok".into(),
                PolicyFamily::Staffing,
                5000,
                DueQuarter::new(1).unwrap()
            ),
            Err(ContractError::InvalidCondition)
        );
        for bad in [
            "\u{202E}override",
            "a\nspoof",
            "bad\u{2066}name",
            "\u{0000}",
        ] {
            assert!(validate_rationale(bad).is_err());
            assert!(validate_identifier(bad).is_err());
        }
        let mut pos = FinalPosition::new(
            "meeting".into(),
            WireU64::new(0),
            Ministry::Education,
            Position::Support,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
        )
        .unwrap();
        assert!(
            pos.set_public_details(vec![], vec![], "x\u{202E}y".into())
                .is_err()
        );
        assert_eq!(pos.rationale_summary(), "");
    }

    #[test]
    fn bounded_types_reject_invalid_construction() {
        assert_eq!(
            CapacityCaps::new(13, 10, 4),
            Err(ContractError::InvalidCreditAmount)
        );
        assert_eq!(
            Allocation::draft(101, 0, 0),
            Err(ContractError::InvalidCreditAmount)
        );
        assert_eq!(
            CohortWeights::new([0; 6]),
            Err(ContractError::InvalidPopulationWeights)
        );
        assert_eq!(
            EffortPermille::new(1_001),
            Err(ContractError::InvalidEffort)
        );
        assert_eq!(BeliefPpm::new(1_000_001), Err(ContractError::InvalidBelief));
        assert_eq!(
            Treasury::from_parts(101, 0, 0, 0, 0, 0),
            Err(ContractError::InvalidTreasury)
        );
    }

    #[test]
    fn cohort_distribution_is_exact_and_ordered() {
        assert_eq!(COHORT_ORDER.len(), 6);
        let weights =
            CohortWeights::new([166_667, 166_667, 166_666, 166_667, 166_667, 166_666]).unwrap();
        assert_eq!(u64::from(weights.parts_ppm()[0]), 166_667);
    }

    #[test]
    fn treasury_exactly_reconciles_service_and_audit_spending() {
        let mut treasury = Treasury::from_parts(8, 40, 30, 10, 10, 2).unwrap();
        treasury.record_delivery(PolicyFamily::Staffing, 3).unwrap();
        assert_eq!(treasury.service_spent(), 13);
        assert_eq!(treasury.encumbered(PolicyFamily::Staffing), 37);
        assert_eq!(treasury.audit_spent(), 2);
        assert_eq!(treasury.validate(), Ok(()));
        assert_eq!(
            treasury.record_delivery(PolicyFamily::Access, 11),
            Err(ContractError::InvalidTreasury)
        );
        assert_eq!(treasury.validate(), Ok(()));
        assert_eq!(
            Treasury::initial().can_fund(Allocation::draft(60, 40, 10).unwrap()),
            Err(ContractError::InsufficientAvailableFunds)
        );
    }

    #[test]
    fn shares_capacity_over_old_and_new_scopes_without_duplication() {
        let mut scopes = vec![
            scope("new", PolicyFamily::Staffing, 2, 12),
            scope("old", PolicyFamily::Staffing, 1, 12),
        ];
        let receipt = deliver_family_quarter(
            PolicyFamily::Staffing,
            12,
            false,
            effort(1000),
            due(3),
            &mut scopes,
        )
        .unwrap();
        assert_eq!(receipt.total_allocated, 12);
        assert_eq!(receipt.total_delivered, 12);
        assert_eq!(receipt.scopes[0].scope_id, "old");
        assert_eq!(receipt.scopes[1].delivered, 0);
        assert_eq!(scopes[0].remaining_units(), 12);
        assert_eq!(scopes[1].remaining_units(), 0);
    }

    #[test]
    fn fractional_work_completes_final_unit_without_exceeding_pool() {
        for rate in [800, 999, 1000] {
            let mut scopes = vec![scope("one", PolicyFamily::Staffing, 1, 40)];
            let mut total = 0;
            for quarter in 2..=7 {
                let receipt = deliver_family_quarter(
                    PolicyFamily::Staffing,
                    12,
                    false,
                    effort(rate),
                    due(quarter),
                    &mut scopes,
                )
                .unwrap();
                assert!(receipt.total_delivered <= 12);
                assert!(receipt.total_allocated <= 12);
                total += receipt.total_delivered;
            }
            assert_eq!(total, 40, "rate={rate}");
            assert_eq!(scopes[0].carry_permille(), 0);
        }
    }

    #[test]
    fn no_capacity_preserves_carry_and_scope_termination_records_discard() {
        let mut scopes = vec![scope("one", PolicyFamily::Staffing, 1, 10)];
        deliver_family_quarter(
            PolicyFamily::Staffing,
            1,
            false,
            effort(800),
            due(2),
            &mut scopes,
        )
        .unwrap();
        assert_eq!(scopes[0].carry_permille(), 800);
        deliver_family_quarter(
            PolicyFamily::Staffing,
            0,
            false,
            effort(800),
            due(3),
            &mut scopes,
        )
        .unwrap();
        assert_eq!(scopes[0].carry_permille(), 800);
        let dropped = scopes[0].terminate();
        assert_eq!(dropped.dropped_carry_permille, 800);
        assert_eq!(scopes[0].carry_permille(), 0);
        let later = deliver_family_quarter(
            PolicyFamily::Staffing,
            12,
            false,
            effort(1000),
            due(4),
            &mut scopes,
        )
        .unwrap();
        assert_eq!(later.total_delivered, 0);
    }

    #[test]
    fn family_bounds_and_failed_actions_leave_state_unchanged() {
        let mut scopes = vec![scope("s", PolicyFamily::Staffing, 1, 100)];
        let before = scopes.clone();
        assert_eq!(
            deliver_family_quarter(
                PolicyFamily::Staffing,
                500,
                false,
                effort(1000),
                due(2),
                &mut scopes
            ),
            Err(ContractError::InvalidCreditAmount)
        );
        assert_eq!(scopes, before);
        assert_eq!(
            deliver_family_quarter(
                PolicyFamily::Repairs,
                10,
                false,
                effort(1000),
                due(3),
                &mut scopes
            ),
            Err(ContractError::DuplicateScope)
        );
        assert_eq!(scopes, before);
        assert_eq!(
            deliver_family_quarter(
                PolicyFamily::Staffing,
                12,
                true,
                effort(1000),
                due(3),
                &mut scopes
            ),
            Err(ContractError::InvalidCreditAmount)
        );
        assert_eq!(scopes, before);
    }

    #[test]
    fn access_bonus_and_stable_id_ties() {
        let mut scopes = vec![
            scope("z", PolicyFamily::Access, 1, 10),
            scope("a", PolicyFamily::Access, 1, 10),
        ];
        let r = deliver_family_quarter(
            PolicyFamily::Access,
            4,
            true,
            effort(1000),
            due(1),
            &mut scopes,
        )
        .unwrap();
        assert_eq!(r.total_delivered, 5);
        assert_eq!(r.scopes[0].scope_id, "a");
        assert_eq!(r.scopes[0].delivered, 5);
    }

    #[test]
    fn private_effort_cannot_attach_to_finance_or_other_role() {
        assert!(
            PrivateEffortDeclaration::new("m".to_string(), Ministry::Finance, effort(5)).is_err()
        );
        let pub_view = RoleObservation::new(RoleObservationInput {
            campaign_id: "c".into(),
            meeting_id: "m".into(),
            proposal_version: WireU64::new(1),
            ministry: Ministry::Education,
            quarter: Quarter::new(1).unwrap(),
            allocation: Allocation::draft(50, 40, 10).unwrap(),
            available_credits: 0,
            belief_ppm: BeliefPpm::new(500_000).unwrap(),
        })
        .unwrap();
        let other =
            PrivateEffortDeclaration::new("m".into(), Ministry::Infrastructure, effort(800))
                .unwrap();
        assert!(PrivateRoleObservation::new(pub_view.clone(), Some(other)).is_err());
        let own =
            PrivateEffortDeclaration::new("m".into(), Ministry::Education, effort(800)).unwrap();
        let view = PrivateRoleObservation::new(pub_view.clone(), Some(own)).unwrap();
        assert_eq!(view.own_effort(), Some(effort(800)));
        let vote = FinalPosition::new(
            "m".into(),
            WireU64::new(1),
            Ministry::Education,
            Position::Support,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
        )
        .unwrap();
        assert!(!format!("{vote:?}").contains("800"));
        assert!(!format!("{pub_view:?}").contains("800"));
    }

    #[test]
    fn proposals_and_versions_are_typed_and_precise() {
        let over =
            PreferredAllocation::assess(Allocation::draft(60, 40, 10).unwrap(), 100).unwrap();
        assert_eq!(over.validity(), AllocationValidity::InvalidDraft);
        assert_eq!(
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100)
                .unwrap()
                .validity(),
            AllocationValidity::Feasible
        );
        let max = WireU64::new(u64::MAX);
        assert_eq!(WireU64::from_decimal(&max.to_decimal()), Ok(max));
        assert_eq!(WireU64::from_decimal("0"), Ok(WireU64::new(0)));
        assert_eq!(WireU64::from_decimal("1"), Ok(WireU64::new(1)));
        for invalid in ["", "-1", "01", "1.0", " 1", "18446744073709551616"] {
            assert_eq!(
                WireU64::from_decimal(invalid),
                Err(ContractError::InvalidDecimal)
            );
        }
        let command = CommandEnvelope::new(
            "c".into(),
            "cmd".into(),
            max,
            PlayerCommand::Amend {
                meeting_id: "m".into(),
                proposal_version: max,
                allocation: Allocation::draft(50, 40, 10).unwrap(),
            },
        )
        .unwrap();
        assert_eq!(
            command.expected_world_version.to_decimal(),
            u64::MAX.to_string()
        );
        assert_eq!(command.schema_version(), SCHEMA_VERSION);
    }

    #[test]
    fn future_due_quarters_and_canonical_disclosures() {
        assert!(Quarter::new(7).is_err());
        assert_eq!(DueQuarter::new(8).unwrap().value(), 8);
        assert_eq!(DueQuarter::new(0), Err(ContractError::InvalidQuarter));
        let roles = EvidenceRecipients::new(vec![
            Ministry::Infrastructure,
            Ministry::Education,
            Ministry::Education,
        ])
        .unwrap();
        assert_eq!(
            roles.roles(),
            &[Ministry::Education, Ministry::Infrastructure]
        );
        assert_eq!(
            EvidenceRecipients::new(vec![]),
            Err(ContractError::InvalidRecipients)
        );
    }

    #[test]
    fn typed_meeting_and_advance_commands_are_distinct() {
        let q = Quarter::new(1).unwrap();
        let position = FirstPosition::new(
            "m".into(),
            WireU64::new(1),
            Ministry::Finance,
            Position::Support,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
            CounterproposalOrigin::Menu("balanced".into()),
        )
        .unwrap();
        assert_eq!(position.schema_version(), SCHEMA_VERSION);
        assert_eq!(q.value(), 1);
        let _separate_explicit_advance_command = PlayerCommand::AdvanceQuarter;
        // No reducer exists in P1; this verifies distinct typed actions only.
    }
}
