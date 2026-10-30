//! Private server-side P1 checkpoint and event wire formats.
//! Never expose these DTOs as public role views or browser state.
use crate::{Family, Ministry, WireError};
use d::world;
use myth_domain as d;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

fn exact_version(s: &str) -> Result<d::WireU64, WireError> {
    Ok(d::WireU64::from_decimal(s)?)
}
fn digest(s: &str) -> Result<(), WireError> {
    if d::is_sha256_hex(s) {
        Ok(())
    } else {
        Err(WireError::Field("sha256"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
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
impl From<&d::EvidenceProvenance> for EvidenceRecord {
    fn from(x: &d::EvidenceProvenance) -> Self {
        Self {
            stable_variable_id: x.stable_variable_id.clone(),
            geographic_scope: x.geographic_scope.clone(),
            definition: x.definition.clone(),
            source_url: x.source_url.clone(),
            publisher: x.publisher.clone(),
            reference_period: x.reference_period.clone(),
            release_date: x.release_date.clone(),
            retrieval_date: x.retrieval_date.clone(),
            raw_unit: x.raw_unit.clone(),
            normalized_unit: x.normalized_unit.clone(),
            transformation: x.transformation.clone(),
            licence_or_terms: x.licence_or_terms.clone(),
            sha256_hex: x.sha256_hex.clone(),
            missing_reason: x.missing_reason.clone(),
        }
    }
}
impl From<EvidenceRecord> for d::EvidenceProvenance {
    fn from(x: EvidenceRecord) -> Self {
        Self {
            stable_variable_id: x.stable_variable_id,
            geographic_scope: x.geographic_scope,
            definition: x.definition,
            source_url: x.source_url,
            publisher: x.publisher,
            reference_period: x.reference_period,
            release_date: x.release_date,
            retrieval_date: x.retrieval_date,
            raw_unit: x.raw_unit,
            normalized_unit: x.normalized_unit,
            transformation: x.transformation,
            licence_or_terms: x.licence_or_terms,
            sha256_hex: x.sha256_hex,
            missing_reason: x.missing_reason,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CountryPack {
    pub schema_version: u16,
    pub country_iso: String,
    pub simulated_jurisdiction: String,
    pub cutoff_utc: String,
    pub locale: String,
    pub evidence: Vec<EvidenceRecord>,
}
impl TryFrom<CountryPack> for d::CountryPack {
    type Error = WireError;
    fn try_from(x: CountryPack) -> Result<Self, WireError> {
        let pack = Self {
            schema_version: x.schema_version,
            country_iso: x.country_iso,
            simulated_jurisdiction: x.simulated_jurisdiction,
            cutoff_utc: x.cutoff_utc,
            locale: x.locale,
            evidence: x.evidence.into_iter().map(Into::into).collect(),
        };
        pack.validate()?;
        Ok(pack)
    }
}
impl From<&d::CountryPack> for CountryPack {
    fn from(x: &d::CountryPack) -> Self {
        Self {
            schema_version: x.schema_version,
            country_iso: x.country_iso.clone(),
            simulated_jurisdiction: x.simulated_jurisdiction.clone(),
            cutoff_utc: x.cutoff_utc.clone(),
            locale: x.locale.clone(),
            evidence: x.evidence.iter().map(Into::into).collect(),
        }
    }
}
pub fn parse_country_pack(raw: &str) -> Result<d::CountryPack, WireError> {
    let pack: CountryPack =
        serde_json::from_str(raw).map_err(|err| WireError::Json(err.to_string()))?;
    pack.try_into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    QuarterOpen,
    FirstPositionsFrozen,
    AmendmentWindow,
    FinalPositionsFrozen,
    Resolved,
    QuarterClosed,
}
impl From<world::MeetingPhase> for Phase {
    fn from(p: world::MeetingPhase) -> Self {
        match p {
            world::MeetingPhase::QuarterOpen => Self::QuarterOpen,
            world::MeetingPhase::FirstPositionsFrozen => Self::FirstPositionsFrozen,
            world::MeetingPhase::AmendmentWindow => Self::AmendmentWindow,
            world::MeetingPhase::FinalPositionsFrozen => Self::FinalPositionsFrozen,
            world::MeetingPhase::Resolved => Self::Resolved,
            world::MeetingPhase::QuarterClosed => Self::QuarterClosed,
        }
    }
}
impl From<Phase> for world::MeetingPhase {
    fn from(p: Phase) -> Self {
        match p {
            Phase::QuarterOpen => Self::QuarterOpen,
            Phase::FirstPositionsFrozen => Self::FirstPositionsFrozen,
            Phase::AmendmentWindow => Self::AmendmentWindow,
            Phase::FinalPositionsFrozen => Self::FinalPositionsFrozen,
            Phase::Resolved => Self::Resolved,
            Phase::QuarterClosed => Self::QuarterClosed,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Treasury {
    pub available: u16,
    pub encumbered_staff: u16,
    pub encumbered_repairs: u16,
    pub encumbered_access: u16,
    pub service_spent: u16,
    pub audit_spent: u16,
}
impl From<d::Treasury> for Treasury {
    fn from(t: d::Treasury) -> Self {
        Self {
            available: t.available(),
            encumbered_staff: t.encumbered(d::PolicyFamily::Staffing),
            encumbered_repairs: t.encumbered(d::PolicyFamily::Repairs),
            encumbered_access: t.encumbered(d::PolicyFamily::Access),
            service_spent: t.service_spent(),
            audit_spent: t.audit_spent(),
        }
    }
}
impl TryFrom<Treasury> for d::Treasury {
    type Error = WireError;
    fn try_from(t: Treasury) -> Result<Self, WireError> {
        Ok(Self::from_parts(
            t.available,
            t.encumbered_staff,
            t.encumbered_repairs,
            t.encumbered_access,
            t.service_spent,
            t.audit_spent,
        )?)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub id: String,
    pub family: Family,
    pub activation_quarter: u8,
    pub remaining_units: u16,
    pub carry_permille: u16,
    pub blocked: bool,
    pub terminated: bool,
}
impl From<&d::FundedScope> for Scope {
    fn from(s: &d::FundedScope) -> Self {
        Self {
            id: s.id().into(),
            family: s.family().into(),
            activation_quarter: s.activation_quarter().value(),
            remaining_units: s.remaining_units(),
            carry_permille: s.carry_permille(),
            blocked: s.is_blocked(),
            terminated: s.is_terminated(),
        }
    }
}
impl TryFrom<Scope> for d::FundedScope {
    type Error = WireError;
    fn try_from(s: Scope) -> Result<Self, WireError> {
        Ok(Self::restore(
            s.id,
            s.family.into(),
            d::Quarter::new(s.activation_quarter)?,
            s.remaining_units,
            s.carry_permille,
            s.blocked,
            s.terminated,
        )?)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Organisation {
    SchoolWorkforceAssociation,
    CommunityAccessNetwork,
}
impl From<world::Organisation> for Organisation {
    fn from(v: world::Organisation) -> Self {
        match v {
            world::Organisation::SchoolWorkforceAssociation => Self::SchoolWorkforceAssociation,
            world::Organisation::CommunityAccessNetwork => Self::CommunityAccessNetwork,
        }
    }
}
impl From<Organisation> for world::Organisation {
    fn from(v: Organisation) -> Self {
        match v {
            Organisation::SchoolWorkforceAssociation => Self::SchoolWorkforceAssociation,
            Organisation::CommunityAccessNetwork => Self::CommunityAccessNetwork,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OrganisationAccount {
    pub id: Organisation,
    pub resources: u32,
}
impl TryFrom<OrganisationAccount> for world::OrganisationAccount {
    type Error = WireError;
    fn try_from(a: OrganisationAccount) -> Result<Self, WireError> {
        Ok(Self::new(a.id.into(), a.resources)?)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentKind {
    Enforceable,
    Political,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommitmentStatus {
    Pending,
    Honored,
    Broken,
}
impl From<d::CommitmentKind> for CommitmentKind {
    fn from(v: d::CommitmentKind) -> Self {
        match v {
            d::CommitmentKind::Enforceable => Self::Enforceable,
            d::CommitmentKind::Political => Self::Political,
        }
    }
}
impl From<CommitmentKind> for d::CommitmentKind {
    fn from(v: CommitmentKind) -> Self {
        match v {
            CommitmentKind::Enforceable => Self::Enforceable,
            CommitmentKind::Political => Self::Political,
        }
    }
}
impl From<d::CommitmentStatus> for CommitmentStatus {
    fn from(v: d::CommitmentStatus) -> Self {
        match v {
            d::CommitmentStatus::Pending => Self::Pending,
            d::CommitmentStatus::Honored => Self::Honored,
            d::CommitmentStatus::Broken => Self::Broken,
        }
    }
}
impl From<CommitmentStatus> for d::CommitmentStatus {
    fn from(v: CommitmentStatus) -> Self {
        match v {
            CommitmentStatus::Pending => Self::Pending,
            CommitmentStatus::Honored => Self::Honored,
            CommitmentStatus::Broken => Self::Broken,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Commitment {
    pub id: String,
    pub issuer: Ministry,
    pub beneficiary: Ministry,
    #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
    pub proposal_version: String,
    pub kind: CommitmentKind,
    pub funded_units: u16,
    pub trigger: String,
    pub deadline_quarter: u16,
    pub status: CommitmentStatus,
    pub resolution_event: Option<String>,
}
impl From<&d::Commitment> for Commitment {
    fn from(c: &d::Commitment) -> Self {
        Self {
            id: c.id.clone(),
            issuer: c.issuer.into(),
            beneficiary: c.beneficiary.into(),
            proposal_version: c.proposal_version.to_decimal(),
            kind: c.kind.into(),
            funded_units: c.funded_units,
            trigger: c.trigger.clone(),
            deadline_quarter: c.deadline_quarter.value(),
            status: c.status.into(),
            resolution_event: c.resolution_event.clone(),
        }
    }
}
impl TryFrom<Commitment> for d::Commitment {
    type Error = WireError;
    fn try_from(c: Commitment) -> Result<Self, WireError> {
        let v = Self {
            id: c.id,
            issuer: c.issuer.into(),
            beneficiary: c.beneficiary.into(),
            proposal_version: exact_version(&c.proposal_version)?,
            kind: c.kind.into(),
            funded_units: c.funded_units,
            trigger: c.trigger,
            deadline_quarter: d::DueQuarter::new(c.deadline_quarter)?,
            status: c.status.into(),
            resolution_event: c.resolution_event,
        };
        v.validate()?;
        Ok(v)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueuedResponse {
    pub source_event_id: String,
    pub produced_quarter: u8,
    pub due_quarter: u16,
    pub delivered: [u16; 3],
    pub service_spent: u16,
}
impl From<&world::QueuedResponse> for QueuedResponse {
    fn from(x: &world::QueuedResponse) -> Self {
        Self {
            source_event_id: x.source_event_id().into(),
            produced_quarter: x.produced_quarter().value(),
            due_quarter: x.due_quarter().value(),
            delivered: x.delivered(),
            service_spent: x.service_spent(),
        }
    }
}
impl TryFrom<QueuedResponse> for world::QueuedResponse {
    type Error = WireError;
    fn try_from(x: QueuedResponse) -> Result<Self, WireError> {
        Ok(Self::new(
            x.source_event_id,
            d::Quarter::new(x.produced_quarter)?,
            d::DueQuarter::new(x.due_quarter)?,
            x.delivered,
            x.service_spent,
        )?)
    }
}
/// Only persist server-side to an operator-controlled directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrivateCheckpoint {
    pub schema_version: u16,
    pub campaign_id: String,
    pub pack_id: String,
    pub pack_sha256: String,
    #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
    pub world_version: String,
    pub quarter: u8,
    pub phase: Phase,
    pub treasury: Treasury,
    pub scopes: Vec<Scope>,
    pub cohort_weights_ppm: [u32; 6],
    pub support_ppm: [u32; 6],
    pub organisations: [OrganisationAccount; 2],
    pub commitments: Vec<Commitment>,
    pub queued_responses: Vec<QueuedResponse>,
}
impl PrivateCheckpoint {
    pub fn capture(
        snapshot: &world::WorldSnapshot,
        pack_sha256: String,
    ) -> Result<Self, WireError> {
        // Snapshot was checked at construction and cannot be externally
        // mutated. Borrow its fields; do not deep-clone or revalidate.
        digest(&pack_sha256)?;
        let parts = snapshot.private_checkpoint_parts_ref();
        Ok(Self {
            schema_version: d::SCHEMA_VERSION,
            campaign_id: parts.campaign_id.clone(),
            pack_id: parts.pack_id.clone(),
            pack_sha256,
            world_version: parts.world_version.to_decimal(),
            quarter: parts.quarter.value(),
            phase: parts.phase.into(),
            treasury: parts.treasury.into(),
            scopes: parts.scopes.iter().map(Into::into).collect(),
            cohort_weights_ppm: *parts.cohort_weights.parts_ppm(),
            support_ppm: parts.support.map(|v| v.value()),
            organisations: std::array::from_fn(|i| OrganisationAccount {
                id: parts.organisations[i].id().into(),
                resources: parts.organisations[i].resources(),
            }),
            commitments: parts.commitments.iter().map(Into::into).collect(),
            queued_responses: parts.queued_responses.iter().map(Into::into).collect(),
        })
    }
    pub fn restore(self, expected_pack_sha256: &str) -> Result<world::WorldSnapshot, WireError> {
        if self.schema_version != d::SCHEMA_VERSION {
            return Err(WireError::Version);
        }
        digest(&self.pack_sha256)?;
        if self.pack_sha256 != expected_pack_sha256 {
            return Err(WireError::Field("pack_sha256"));
        }
        let parts = world::WorldParts {
            campaign_id: self.campaign_id,
            pack_id: self.pack_id,
            world_version: exact_version(&self.world_version)?,
            quarter: d::Quarter::new(self.quarter)?,
            phase: self.phase.into(),
            treasury: self.treasury.try_into()?,
            scopes: self
                .scopes
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            cohort_weights: d::CohortWeights::new(self.cohort_weights_ppm)?,
            support: self
                .support_ppm
                .map(world::SupportPpm::new)
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| WireError::Field("support_ppm"))?,
            organisations: self
                .organisations
                .map(TryInto::try_into)
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| WireError::Field("organisations"))?,
            commitments: self
                .commitments
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            queued_responses: self
                .queued_responses
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(world::WorldSnapshot::from_parts(parts)?)
    }
}
pub fn parse_checkpoint(raw: &str, pack_sha256: &str) -> Result<world::WorldSnapshot, WireError> {
    let data: PrivateCheckpoint =
        serde_json::from_str(raw).map_err(|err| WireError::Json(err.to_string()))?;
    data.restore(pack_sha256)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    CommandAdmitted {
        command_id: String,
    },
    MeetingPhaseDeclared {
        phase: Phase,
    },
    LedgerTransfer {
        before: Treasury,
        after: Treasury,
        movement: Move,
    },
    DeliveryRecorded {
        family: Family,
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
        next: u8,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Move {
    Reserve { family: Family, credits: u16 },
    Release { family: Family, credits: u16 },
    SpendService { family: Family, credits: u16 },
    SpendAudit { credits: u16 },
}
impl From<d::LedgerMove> for Move {
    fn from(x: d::LedgerMove) -> Self {
        match x {
            d::LedgerMove::Reserve { family, credits } => Self::Reserve {
                family: family.into(),
                credits,
            },
            d::LedgerMove::Release { family, credits } => Self::Release {
                family: family.into(),
                credits,
            },
            d::LedgerMove::SpendService { family, credits } => Self::SpendService {
                family: family.into(),
                credits,
            },
            d::LedgerMove::SpendAudit { credits } => Self::SpendAudit { credits },
        }
    }
}
impl From<Move> for d::LedgerMove {
    fn from(x: Move) -> Self {
        match x {
            Move::Reserve { family, credits } => Self::Reserve {
                family: family.into(),
                credits,
            },
            Move::Release { family, credits } => Self::Release {
                family: family.into(),
                credits,
            },
            Move::SpendService { family, credits } => Self::SpendService {
                family: family.into(),
                credits,
            },
            Move::SpendAudit { credits } => Self::SpendAudit { credits },
        }
    }
}
impl From<&world::EventKind> for Event {
    fn from(x: &world::EventKind) -> Self {
        match x {
            world::EventKind::CommandAdmitted { command_id } => Self::CommandAdmitted {
                command_id: command_id.clone(),
            },
            world::EventKind::MeetingPhaseDeclared { phase } => Self::MeetingPhaseDeclared {
                phase: (*phase).into(),
            },
            world::EventKind::LedgerTransfer(receipt) => Self::LedgerTransfer {
                before: receipt.before().into(),
                after: receipt.after().into(),
                movement: receipt.movement().into(),
            },
            world::EventKind::DeliveryRecorded { family, delivered } => Self::DeliveryRecorded {
                family: (*family).into(),
                delivered: *delivered,
            },
            world::EventKind::EvidenceDisclosed {
                evidence_id,
                recipient,
            } => Self::EvidenceDisclosed {
                evidence_id: evidence_id.clone(),
                recipient: (*recipient).into(),
            },
            world::EventKind::ResponseQueued { source_event_id } => Self::ResponseQueued {
                source_event_id: source_event_id.clone(),
            },
            world::EventKind::QuarterAdvanced { next } => {
                Self::QuarterAdvanced { next: next.value() }
            }
        }
    }
}
impl TryFrom<Event> for world::EventKind {
    type Error = WireError;
    fn try_from(x: Event) -> Result<Self, WireError> {
        Ok(match x {
            Event::CommandAdmitted { command_id } => Self::CommandAdmitted { command_id },
            Event::MeetingPhaseDeclared { phase } => Self::MeetingPhaseDeclared {
                phase: phase.into(),
            },
            Event::LedgerTransfer {
                before,
                after,
                movement,
            } => Self::LedgerTransfer(world::LedgerReceipt::new(
                before.try_into()?,
                movement.into(),
                after.try_into()?,
            )?),
            Event::DeliveryRecorded { family, delivered } => Self::DeliveryRecorded {
                family: family.into(),
                delivered,
            },
            Event::EvidenceDisclosed {
                evidence_id,
                recipient,
            } => Self::EvidenceDisclosed {
                evidence_id,
                recipient: recipient.into(),
            },
            Event::ResponseQueued { source_event_id } => Self::ResponseQueued { source_event_id },
            Event::QuarterAdvanced { next } => Self::QuarterAdvanced {
                next: d::Quarter::new(next)?,
            },
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventRecord {
    pub schema_version: u16,
    pub campaign_id: String,
    pub event_id: String,
    #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
    pub sequence: String,
    #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
    pub world_version: String,
    pub quarter: u8,
    pub event: Event,
}
impl EventRecord {
    pub fn capture(x: &world::EventEnvelope) -> Self {
        Self {
            schema_version: x.schema_version(),
            campaign_id: x.campaign_id().into(),
            event_id: x.event_id().into(),
            sequence: x.sequence().to_decimal(),
            world_version: x.world_version().to_decimal(),
            quarter: x.quarter().value(),
            event: x.kind().into(),
        }
    }
    pub fn restore(self) -> Result<world::EventEnvelope, WireError> {
        if self.schema_version != d::SCHEMA_VERSION {
            return Err(WireError::Version);
        }
        Ok(world::EventEnvelope::new(
            self.campaign_id,
            self.event_id,
            exact_version(&self.sequence)?,
            exact_version(&self.world_version)?,
            d::Quarter::new(self.quarter)?,
            self.event.try_into()?,
        )?)
    }
}
pub fn parse_event(raw: &str) -> Result<world::EventEnvelope, WireError> {
    let e: EventRecord = serde_json::from_str(raw).map_err(|e| WireError::Json(e.to_string()))?;
    e.restore()
}
#[cfg(test)]
mod tests {
    use super::*;
    const PACK_HASH: &str = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
    fn seed() -> world::WorldSnapshot {
        world::WorldSnapshot::initial(
            "campaign".into(),
            "pack".into(),
            d::CohortWeights::new([200_000, 200_000, 100_000, 150_000, 200_000, 150_000]).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn full_checkpoint_roundtrip_preserves_precision_and_rejects_tamper() {
        let base = seed();
        let mut p = base.private_checkpoint_parts();
        p.world_version = d::WireU64::new(u64::MAX);
        p.treasury = d::Treasury::from_parts(88, 12, 0, 0, 0, 0).unwrap();
        p.scopes.push(
            d::FundedScope::restore(
                "fund".into(),
                d::PolicyFamily::Staffing,
                d::Quarter::new(1).unwrap(),
                12,
                17,
                true,
                false,
            )
            .unwrap(),
        );
        p.commitments.push(d::Commitment {
            id: "promise".into(),
            issuer: d::Ministry::Finance,
            beneficiary: d::Ministry::Education,
            proposal_version: d::WireU64::new(u64::MAX),
            kind: d::CommitmentKind::Political,
            funded_units: 0,
            trigger: "promise".into(),
            deadline_quarter: d::DueQuarter::new(2).unwrap(),
            status: d::CommitmentStatus::Pending,
            resolution_event: None,
        });
        let world = world::WorldSnapshot::from_parts(p).unwrap();
        let captured = PrivateCheckpoint::capture(&world, PACK_HASH.into()).unwrap();
        let json = serde_json::to_string(&captured).unwrap();
        assert!(json.contains(r#""18446744073709551615""#));
        let restored = parse_checkpoint(&json, PACK_HASH).unwrap();
        assert_eq!(restored.world_version().value(), u64::MAX);
        assert_eq!(restored.scopes()[0].carry_permille(), 17);
        assert!(restored.scopes()[0].is_blocked());
        assert_eq!(restored.commitments().len(), 1);
        let mut phantom = captured.clone();
        phantom.scopes[0].remaining_units = 0;
        phantom.scopes[0].carry_permille = 999;
        assert!(matches!(
            phantom.restore(PACK_HASH),
            Err(WireError::Domain(_))
        ));

        assert!(
            parse_checkpoint(
                &json,
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
            .is_err()
        );
        let extra = json.replacen(
            r#""schema_version":1"#,
            r#""schema_version":1,"unknown":true"#,
            1,
        );
        assert!(parse_checkpoint(&extra, PACK_HASH).is_err());
        let unquoted = json.replace(r#""18446744073709551615""#, "18446744073709551615");
        assert!(parse_checkpoint(&unquoted, PACK_HASH).is_err());
    }
    #[test]
    fn versioned_events_roundtrip_and_reject_invalid_transfer() {
        let first = d::Treasury::initial();
        let second = first
            .apply(d::LedgerMove::Reserve {
                family: d::PolicyFamily::Staffing,
                credits: 5,
            })
            .unwrap();
        let receipt = world::LedgerReceipt::new(
            first,
            d::LedgerMove::Reserve {
                family: d::PolicyFamily::Staffing,
                credits: 5,
            },
            second,
        )
        .unwrap();
        let event = world::EventEnvelope::new(
            "campaign".into(),
            "event-1".into(),
            d::WireU64::new(u64::MAX),
            d::WireU64::new(7),
            d::Quarter::new(1).unwrap(),
            world::EventKind::LedgerTransfer(receipt),
        )
        .unwrap();
        let wire = EventRecord::capture(&event);
        let data = serde_json::to_string(&wire).unwrap();
        assert_eq!(parse_event(&data).unwrap(), event);
        let altered = data.replace(r#""credits":5"#, r#""credits":6"#);
        assert!(parse_event(&altered).is_err());
    }
    #[test]
    fn provenance_pack_rejects_duplicate_or_missing_source() {
        let pack = CountryPack {
            schema_version: 1,
            country_iso: "GB".into(),
            simulated_jurisdiction: "England".into(),
            cutoff_utc: "2026-10-01T00:00:00Z".into(),
            locale: "en-GB".into(),
            evidence: vec![EvidenceRecord {
                stable_variable_id: "indicator-1".into(),
                geographic_scope: "England".into(),
                definition: "Measure".into(),
                source_url: "https://example.org/series".into(),
                publisher: "Publisher".into(),
                reference_period: "2025".into(),
                release_date: "2026-01-01".into(),
                retrieval_date: "2026-10-01".into(),
                raw_unit: "index".into(),
                normalized_unit: "index".into(),
                transformation: "none".into(),
                licence_or_terms: "Public metadata".into(),
                sha256_hex: PACK_HASH.into(),
                missing_reason: None,
            }],
        };
        let good = serde_json::to_string(&pack).unwrap();
        assert!(parse_country_pack(&good).is_ok());
        let mut duplicate = pack.clone();
        duplicate.evidence.push(duplicate.evidence[0].clone());
        assert!(parse_country_pack(&serde_json::to_string(&duplicate).unwrap()).is_err());
        let mut http = pack.clone();
        http.evidence[0].source_url = "http://example.org".into();
        assert!(parse_country_pack(&serde_json::to_string(&http).unwrap()).is_err());
        let mut bare = pack.clone();
        bare.evidence[0].source_url = "https://".into();
        assert!(parse_country_pack(&serde_json::to_string(&bare).unwrap()).is_err());
        let mut newline = pack.clone();
        newline.evidence[0].source_url = "https://x\n y".into();
        assert!(parse_country_pack(&serde_json::to_string(&newline).unwrap()).is_err());
        let mut bidi = pack.clone();
        bidi.evidence[0].geographic_scope = "bad\u{202e}text".into();
        assert!(parse_country_pack(&serde_json::to_string(&bidi).unwrap()).is_err());
        let mut invalid_cutoff = pack.clone();
        invalid_cutoff.cutoff_utc = "2026-99-99T99:99:99Z".into();
        assert!(parse_country_pack(&serde_json::to_string(&invalid_cutoff).unwrap()).is_err());
        let mut malformed_release = pack.clone();
        malformed_release.evidence[0].release_date = "garbage".into();
        assert!(parse_country_pack(&serde_json::to_string(&malformed_release).unwrap()).is_err());
        let mut malformed_retrieval = pack.clone();
        malformed_retrieval.evidence[0].retrieval_date = "2026-02-30".into();
        assert!(parse_country_pack(&serde_json::to_string(&malformed_retrieval).unwrap()).is_err());
        let mut bad_date = pack.clone();
        bad_date.evidence[0].release_date = "2026-02-30".into();
        assert!(parse_country_pack(&serde_json::to_string(&bad_date).unwrap()).is_err());
        let mut after_cutoff = pack;
        after_cutoff.evidence[0].release_date = "2026-10-02".into();
        assert!(parse_country_pack(&serde_json::to_string(&after_cutoff).unwrap()).is_err());
    }
}

#[cfg(test)]
mod p2_frozen_pack_tests {
    #[test]
    fn frozen_england_headline_provenance_matches_p1_contract() {
        let raw = include_str!("../../../data/england-school-pilot-v1/country-pack.json");
        let pack = super::parse_country_pack(raw)
            .expect("P2 fixture must satisfy P1 country-pack contract");
        assert_eq!(pack.country_iso, "GB");
        assert_eq!(pack.simulated_jurisdiction, "england");
        assert_eq!(pack.cutoff_utc, "2026-10-29T23:59:59Z");
        assert_eq!(pack.evidence.len(), 12);
        assert_eq!(
            pack.evidence
                .iter()
                .filter(|entry| entry.missing_reason.is_none())
                .count(),
            8
        );
    }
}
