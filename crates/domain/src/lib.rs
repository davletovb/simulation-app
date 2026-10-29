//! Authoritative, side-effect-free types and checked arithmetic for Myth Maker.
//!
//! P1 foundation only. The treasury and delivery APIs here are primitives,
//! not the complete P3 policy reducer, P4 Hiyal adapter or P5 inference backend.
#![forbid(unsafe_code)]

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
    InvalidPopulationWeights,
    InvalidDecimal,
    InvalidTreasury,
    InsufficientAvailableFunds,
    ArithmeticOverflow,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ContractError {}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Ministry {
    Finance,
    Education,
    Infrastructure,
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
    // Index order is COHORT_ORDER; not a statistical estimate by itself.
    pub parts_ppm: [u32; 6],
}

impl CohortWeights {
    pub fn new(parts_ppm: [u32; 6]) -> Result<Self, ContractError> {
        let total: u64 = parts_ppm.iter().map(|v| u64::from(*v)).sum();
        if total != u64::from(SHARE_DENOMINATOR) {
            return Err(ContractError::InvalidPopulationWeights);
        }
        Ok(Self { parts_ppm })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Allocation {
    pub staffing: u16,
    pub repairs: u16,
    pub access: u16,
}

impl Allocation {
    // Deliberately permits an over-budget *draft* (e.g. 60/40/10);
    // ratification performs a separate affordability check.
    pub fn draft(staffing: u16, repairs: u16, access: u16) -> Result<Self, ContractError> {
        if [staffing, repairs, access]
            .iter()
            .any(|v| *v > PILOT_CREDITS)
        {
            return Err(ContractError::InvalidCreditAmount);
        }
        Ok(Self {
            staffing,
            repairs,
            access,
        })
    }

    pub fn total(self) -> u32 {
        u32::from(self.staffing) + u32::from(self.repairs) + u32::from(self.access)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Treasury {
    pub available: u16,
    pub encumbered: Allocation,
    pub service_spent: u16,
    pub audit_spent: u16,
}

impl Treasury {
    pub fn initial() -> Self {
        Self {
            available: PILOT_CREDITS,
            encumbered: Allocation::default(),
            service_spent: 0,
            audit_spent: 0,
        }
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        let sum = u32::from(self.available)
            .checked_add(self.encumbered.total())
            .and_then(|v| v.checked_add(u32::from(self.service_spent)))
            .and_then(|v| v.checked_add(u32::from(self.audit_spent)))
            .ok_or(ContractError::ArithmeticOverflow)?;
        if sum == u32::from(PILOT_CREDITS) {
            Ok(())
        } else {
            Err(ContractError::InvalidTreasury)
        }
    }

    // P1 admission primitive, not a substitute for P3 voting and
    // commitment checks. It does not mutate any authoritative state.
    pub fn can_fund(&self, additional: Allocation) -> Result<(), ContractError> {
        self.validate()?;
        if additional.total() > u32::from(self.available) {
            Err(ContractError::InsufficientAvailableFunds)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeliveryProgress {
    pub carry_permille: u16,
}

impl DeliveryProgress {
    pub fn step(
        &mut self,
        remaining_units: u16,
        authorized_capacity: u16,
        effective_effort_permille: u16,
    ) -> Result<u16, ContractError> {
        if effective_effort_permille > EFFORT_DENOMINATOR
            || self.carry_permille >= EFFORT_DENOMINATOR
        {
            return Err(ContractError::InvalidEffort);
        }
        if remaining_units == 0 {
            self.carry_permille = 0;
            return Ok(0);
        }
        let capacity = remaining_units.min(authorized_capacity);
        let work_permille = u32::from(capacity)
            .checked_mul(u32::from(effective_effort_permille))
            .and_then(|v| v.checked_add(u32::from(self.carry_permille)))
            .ok_or(ContractError::ArithmeticOverflow)?;
        let delivered_u32 =
            (work_permille / u32::from(EFFORT_DENOMINATOR)).min(u32::from(remaining_units));
        let delivered =
            u16::try_from(delivered_u32).map_err(|_| ContractError::ArithmeticOverflow)?;
        self.carry_permille = if delivered == remaining_units {
            0
        } else {
            u16::try_from(work_permille % u32::from(EFFORT_DENOMINATOR))
                .map_err(|_| ContractError::ArithmeticOverflow)?
        };
        Ok(delivered)
    }
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
    pub proposal_version: u64,
    pub kind: CommitmentKind,
    pub funded_units: u16,
    pub trigger_id: String,
    pub deadline: Quarter,
    pub status: CommitmentStatus,
    pub resolution_event_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FundedCondition {
    pub kind_id: String,
    pub family: PolicyFamily,
    pub units: u16,
    pub due: Quarter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyFamily {
    Staffing,
    Repairs,
    Access,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CounterproposalOrigin {
    Menu(String),
    NewTyped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstPosition {
    pub meeting_id: String,
    pub proposal_version: u64,
    pub ministry: Ministry,
    pub position: Position,
    pub preferred_allocation: Allocation,
    pub origin: CounterproposalOrigin,
    pub conditions: Vec<FundedCondition>,
    pub evidence_ids: Vec<String>,
    pub rationale_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalPosition {
    pub meeting_id: String,
    pub proposal_version: u64,
    pub ministry: Ministry,
    pub vote: Position,
    pub preferred_allocation: Allocation,
    pub conditions: Vec<FundedCondition>,
    pub evidence_ids: Vec<String>,
    pub rationale_summary: String,
    // Required only for Education and Infrastructure by P3 validation;
    // Finance must omit. This is role-private actual effort.
    pub implementation_effort_permille: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityCaps {
    pub staffing: u16,
    pub repairs: u16,
    pub access: u16,
}

impl CapacityCaps {
    pub fn new(staffing: u16, repairs: u16, access: u16) -> Result<Self, ContractError> {
        if staffing > 12 || repairs > 10 || access > 4 {
            return Err(ContractError::InvalidCreditAmount);
        }
        Ok(Self {
            staffing,
            repairs,
            access,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlayerCommand {
    Propose {
        allocation: Allocation,
    },
    Amend {
        proposal_version: u64,
        allocation: Allocation,
    },
    Ratify {
        proposal_version: u64,
    },
    RequestCapacityCaps {
        caps: CapacityCaps,
    },
    OrderAudit {
        family: PolicyFamily,
        inspected_quarter: Quarter,
    },
    DiscloseEvidence {
        evidence_id: String,
        to: Ministry,
    },
    Defer,
    AdvanceQuarter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandEnvelope {
    pub campaign_id: String,
    pub command_id: String,
    pub expected_world_version: u64,
    pub command: PlayerCommand,
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
    // Actual numerical observations are typed/validated in P2.
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleObservation {
    pub schema_version: u16,
    pub campaign_id: String,
    pub meeting_id: String,
    pub proposal_version: u64,
    pub ministry: Ministry,
    pub quarter: Quarter,
    pub allocation: Allocation,
    pub available_credits: u16,
    pub belief_ppm: u32,
    pub owned_commitments: Vec<Commitment>,
    pub visible_evidence_ids: Vec<String>,
    // None for Finance. Not exposed to another ministry.
    pub own_private_effort_permille: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WireU64(pub u64);

impl WireU64 {
    // Canonical decimal text for JavaScript-safe JSON transport (no JSON number).
    // Serde/schema generators must preserve this representation in P1-03.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_cells_are_checked_without_guessing_distribution() {
        assert_eq!(COHORT_ORDER.len(), 6);
        assert!(CohortWeights::new([166_667, 166_667, 166_666, 166_667, 166_667, 166_666]).is_ok());
        assert_eq!(
            CohortWeights::new([1, 1, 1, 1, 1, 1]),
            Err(ContractError::InvalidPopulationWeights)
        );
    }

    #[test]
    fn draft_can_be_over_budget_but_not_funded() {
        let over = Allocation::draft(60, 40, 10).unwrap();
        assert_eq!(over.total(), 110);
        assert_eq!(
            Treasury::initial().can_fund(over),
            Err(ContractError::InsufficientAvailableFunds)
        );
        assert!(
            Treasury::initial()
                .can_fund(Allocation::draft(50, 40, 10).unwrap())
                .is_ok()
        );
        assert_eq!(
            Allocation::draft(101, 0, 0),
            Err(ContractError::InvalidCreditAmount)
        );
    }

    #[test]
    fn treasury_invariant_includes_audit_spend() {
        let valid = Treasury {
            available: 8,
            encumbered: Allocation::draft(40, 30, 10).unwrap(),
            service_spent: 10,
            audit_spent: 2,
        };
        assert_eq!(valid.validate(), Ok(()));
        assert_eq!(
            Treasury {
                available: 10,
                ..valid
            }
            .validate(),
            Err(ContractError::InvalidTreasury)
        );
    }

    #[test]
    fn fractional_work_completes_final_unit_without_creating_money() {
        for effort in [800, 999, 1_000] {
            let mut progress = DeliveryProgress::default();
            let mut remaining = 40;
            for _ in 0..6 {
                let delivered = progress.step(remaining, 12, effort).unwrap();
                assert!(delivered <= remaining);
                remaining -= delivered;
            }
            assert_eq!(remaining, 0, "effort={effort}");
            assert_eq!(progress.carry_permille, 0);
        }
    }

    #[test]
    fn zero_effort_and_ineligible_capacity_never_deliver() {
        let mut progress = DeliveryProgress::default();
        assert_eq!(progress.step(4, 4, 0), Ok(0));
        assert_eq!(progress.step(4, 0, 1_000), Ok(0));
        assert_eq!(
            progress.step(1, 1, 1_001),
            Err(ContractError::InvalidEffort)
        );
    }

    #[test]
    fn exact_large_decimal_wire_boundary() {
        let max = u64::MAX.to_string();
        assert_eq!(WireU64::from_decimal(&max), Ok(WireU64(u64::MAX)));
        assert_eq!(WireU64(u64::MAX).to_decimal(), max);
        for noncanonical in ["", "-1", "01", "1.0", " 1", "18446744073709551616"] {
            assert_eq!(
                WireU64::from_decimal(noncanonical),
                Err(ContractError::InvalidDecimal)
            );
        }
    }

    #[test]
    fn authorized_caps_are_distinct_from_hidden_effort() {
        assert!(CapacityCaps::new(12, 10, 4).is_ok());
        assert!(CapacityCaps::new(13, 10, 4).is_err());
        let position = FinalPosition {
            meeting_id: "m".into(),
            proposal_version: 1,
            ministry: Ministry::Education,
            vote: Position::Support,
            preferred_allocation: Allocation::draft(50, 40, 10).unwrap(),
            conditions: vec![],
            evidence_ids: vec![],
            rationale_summary: "Staffing first".into(),
            implementation_effort_permille: Some(800),
        };
        assert_eq!(position.implementation_effort_permille, Some(800));
    }

    #[test]
    fn calendar_bounds_do_not_advance_with_negotiation() {
        assert_eq!(Quarter::new(1).unwrap().value(), 1);
        assert_eq!(Quarter::new(6).unwrap().value(), 6);
        assert_eq!(Quarter::new(0), Err(ContractError::InvalidQuarter));
        assert_eq!(Quarter::new(7), Err(ContractError::InvalidQuarter));
    }
}
