//! Versioned, validated Serde inputs for game commands and minister actions.
//! This crate is an adapter; authoritative rules remain in myth-domain.
#![forbid(unsafe_code)]
use myth_domain as d;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

pub const VERSION: u16 = d::SCHEMA_VERSION;

// Absence is allowed for Finance; explicitly sending null is not. This keeps
// Serde, optional TypeScript, and JSON Schema consistent.
fn deserialize_present_effort<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: Deserializer<'de>,
{
    u16::deserialize(deserializer).map(Some)
}

#[derive(Debug, PartialEq)]
pub enum WireError {
    Json(String),
    Version,
    Field(&'static str),
    Domain(d::ContractError),
}
impl From<d::ContractError> for WireError {
    fn from(e: d::ContractError) -> Self {
        Self::Domain(e)
    }
}
fn identifier(x: &str) -> Result<(), WireError> {
    d::validate_identifier(x).map_err(WireError::Domain)
}
fn version(x: &str) -> Result<d::WireU64, WireError> {
    Ok(d::WireU64::from_decimal(x)?)
}
fn schema(x: u16) -> Result<(), WireError> {
    if x == VERSION {
        Ok(())
    } else {
        Err(WireError::Version)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Allocation {
    #[schemars(range(min = 0, max = 100))]
    pub staff: u16,
    #[schemars(range(min = 0, max = 100))]
    pub repairs: u16,
    #[schemars(range(min = 0, max = 100))]
    pub access: u16,
}
impl TryFrom<Allocation> for d::Allocation {
    type Error = WireError;
    fn try_from(a: Allocation) -> Result<Self, Self::Error> {
        Ok(Self::draft(a.staff, a.repairs, a.access)?)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CapacityCaps {
    #[schemars(range(min = 0, max = 12))]
    pub staff: u16,
    #[schemars(range(min = 0, max = 10))]
    pub repairs: u16,
    #[schemars(range(min = 0, max = 4))]
    pub access: u16,
}
impl TryFrom<CapacityCaps> for d::CapacityCaps {
    type Error = WireError;
    fn try_from(c: CapacityCaps) -> Result<Self, Self::Error> {
        Ok(Self::new(c.staff, c.repairs, c.access)?)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Ministry {
    Finance,
    Education,
    Infrastructure,
}
impl From<Ministry> for d::Ministry {
    fn from(x: Ministry) -> Self {
        match x {
            Ministry::Finance => Self::Finance,
            Ministry::Education => Self::Education,
            Ministry::Infrastructure => Self::Infrastructure,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    Staffing,
    Repairs,
    Access,
}
impl From<Family> for d::PolicyFamily {
    fn from(x: Family) -> Self {
        match x {
            Family::Staffing => Self::Staffing,
            Family::Repairs => Self::Repairs,
            Family::Access => Self::Access,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Vote {
    Support,
    Conditional,
    Oppose,
}
impl From<Vote> for d::Position {
    fn from(x: Vote) -> Self {
        match x {
            Vote::Support => Self::Support,
            Vote::Conditional => Self::Conditional,
            Vote::Oppose => Self::Oppose,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub kind_id: String,
    pub family: Family,
    #[schemars(range(min = 0, max = 100))]
    pub units: u16,
    #[schemars(range(min = 1))]
    pub due_quarter: u16,
}
impl TryFrom<Condition> for d::FundedCondition {
    type Error = WireError;
    fn try_from(c: Condition) -> Result<Self, Self::Error> {
        Ok(Self::new(
            c.kind_id,
            c.family.into(),
            c.units,
            d::DueQuarter::new(c.due_quarter)?,
        )?)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlayerAction {
    Propose {
        meeting_id: String,
        allocation: Allocation,
    },
    Amend {
        meeting_id: String,
        #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
        proposal_version: String,
        allocation: Allocation,
    },
    Ratify {
        meeting_id: String,
        #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
        proposal_version: String,
    },
    RequestCapacityCap {
        meeting_id: String,
        caps: CapacityCaps,
    },
    OrderAudit {
        family: Family,
        #[schemars(range(min = 1, max = 6))]
        inspected_quarter: u8,
    },
    DiscloseEvidence {
        evidence_id: String,
        #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
        evidence_version: String,
        recipients: Vec<Ministry>,
    },
    Defer {},
    AdvanceQuarter {},
}
impl TryFrom<PlayerAction> for d::PlayerCommand {
    type Error = WireError;
    fn try_from(a: PlayerAction) -> Result<Self, Self::Error> {
        Ok(match a {
            PlayerAction::Propose {
                meeting_id,
                allocation,
            } => {
                identifier(&meeting_id)?;
                Self::Propose {
                    meeting_id,
                    allocation: allocation.try_into()?,
                }
            }
            PlayerAction::Amend {
                meeting_id,
                proposal_version,
                allocation,
            } => {
                identifier(&meeting_id)?;
                Self::Amend {
                    meeting_id,
                    proposal_version: version(&proposal_version)?,
                    allocation: allocation.try_into()?,
                }
            }
            PlayerAction::Ratify {
                meeting_id,
                proposal_version,
            } => {
                identifier(&meeting_id)?;
                Self::Ratify {
                    meeting_id,
                    proposal_version: version(&proposal_version)?,
                }
            }
            PlayerAction::RequestCapacityCap { meeting_id, caps } => {
                identifier(&meeting_id)?;
                Self::RequestCapacityCap {
                    meeting_id,
                    caps: caps.try_into()?,
                }
            }
            PlayerAction::OrderAudit {
                family,
                inspected_quarter,
            } => Self::OrderAudit {
                family: family.into(),
                inspected_quarter: d::Quarter::new(inspected_quarter)?,
            },
            PlayerAction::DiscloseEvidence {
                evidence_id,
                evidence_version,
                recipients,
            } => {
                identifier(&evidence_id)?;
                Self::DiscloseEvidence {
                    evidence_id,
                    evidence_version: version(&evidence_version)?,
                    recipients: d::EvidenceRecipients::new(
                        recipients.into_iter().map(Into::into).collect(),
                    )?,
                }
            }
            PlayerAction::Defer {} => Self::Defer,
            PlayerAction::AdvanceQuarter {} => Self::AdvanceQuarter,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub schema_version: u16,
    pub campaign_id: String,
    pub command_id: String,
    // Canonical decimal string, including when larger than JS safe integer.
    #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
    pub expected_world_version: String,
    pub command: PlayerAction,
}
impl TryFrom<CommandEnvelope> for d::CommandEnvelope {
    type Error = WireError;
    fn try_from(x: CommandEnvelope) -> Result<Self, Self::Error> {
        schema(x.schema_version)?;
        identifier(&x.campaign_id)?;
        identifier(&x.command_id)?;
        Ok(Self::new(
            x.campaign_id,
            x.command_id,
            version(&x.expected_world_version)?,
            x.command.try_into()?,
        )?)
    }
}
pub fn parse_command(raw: &str) -> Result<d::CommandEnvelope, WireError> {
    let x: CommandEnvelope =
        serde_json::from_str(raw).map_err(|e| WireError::Json(e.to_string()))?;
    x.try_into()
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    First,
    Final,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Counterproposal {
    Menu { menu_id: String },
    NewTyped {},
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema, TS)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub enum MinisterAction {
    First {
        schema_version: u16,
        meeting_id: String,
        #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
        proposal_version: String,
        ministry: Ministry,
        position: Vote,
        preferred_allocation: Allocation,
        counterproposal_source: Counterproposal,
        conditions: Vec<Condition>,
        evidence_ids: Vec<String>,
        rationale_summary: String,
    },
    Final {
        schema_version: u16,
        meeting_id: String,
        #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
        proposal_version: String,
        ministry: Ministry,
        position: Vote,
        preferred_allocation: Allocation,
        conditions: Vec<Condition>,
        evidence_ids: Vec<String>,
        rationale_summary: String,
        #[serde(default, deserialize_with = "deserialize_present_effort")]
        #[ts(optional)]
        #[schemars(
            with = "u16",
            range(min = 0, max = 1000),
            skip_serializing_if = "Option::is_none"
        )]
        implementation_effort_permille: Option<u16>,
    },
}
#[derive(Clone)]
pub enum PublicPosition {
    First(d::FirstPosition),
    Final(d::FinalPosition),
}
pub struct AcceptedMinisterAction {
    pub public: PublicPosition,
    pub private_effort: Option<d::PrivateEffortDeclaration>,
}
pub fn parse_minister_action(
    raw: &str,
    available_credits: u16,
) -> Result<AcceptedMinisterAction, WireError> {
    let action: MinisterAction =
        serde_json::from_str(raw).map_err(|e| WireError::Json(e.to_string()))?;
    match action {
        MinisterAction::First {
            schema_version,
            meeting_id,
            proposal_version,
            ministry,
            position,
            preferred_allocation,
            counterproposal_source,
            conditions,
            evidence_ids,
            rationale_summary,
        } => {
            schema(schema_version)?;
            let minister: d::Ministry = ministry.into();
            let parsed_version = version(&proposal_version)?;
            let preferred = d::PreferredAllocation::assess(
                preferred_allocation.try_into()?,
                available_credits,
            )?;
            let checked: Vec<d::FundedCondition> = conditions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?;
            let origin = match counterproposal_source {
                Counterproposal::Menu { menu_id } => {
                    identifier(&menu_id)?;
                    d::CounterproposalOrigin::Menu(menu_id)
                }
                Counterproposal::NewTyped {} => d::CounterproposalOrigin::NewTyped,
            };
            let mut p = d::FirstPosition::new(
                meeting_id,
                parsed_version,
                minister,
                position.into(),
                preferred,
                origin,
            )?;
            p.set_public_details(checked, evidence_ids, rationale_summary)?;
            Ok(AcceptedMinisterAction {
                public: PublicPosition::First(p),
                private_effort: None,
            })
        }
        MinisterAction::Final {
            schema_version,
            meeting_id,
            proposal_version,
            ministry,
            position,
            preferred_allocation,
            conditions,
            evidence_ids,
            rationale_summary,
            implementation_effort_permille,
        } => {
            schema(schema_version)?;
            let minister: d::Ministry = ministry.into();
            let parsed_version = version(&proposal_version)?;
            let preferred = d::PreferredAllocation::assess(
                preferred_allocation.try_into()?,
                available_credits,
            )?;
            let checked: Vec<d::FundedCondition> = conditions
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?;
            let private_effort = match (minister, implementation_effort_permille) {
                (d::Ministry::Finance, None) => None,
                (d::Ministry::Finance, Some(_)) => return Err(WireError::Field("finance_effort")),
                (_, None) => return Err(WireError::Field("missing_effort")),
                (_, Some(n)) => Some(d::PrivateEffortDeclaration::new(
                    meeting_id.clone(),
                    minister,
                    d::EffortPermille::new(n)?,
                )?),
            };
            let mut p = d::FinalPosition::new(
                meeting_id,
                parsed_version,
                minister,
                position.into(),
                preferred,
            )?;
            p.set_public_details(checked, evidence_ids, rationale_summary)?;
            Ok(AcceptedMinisterAction {
                public: PublicPosition::Final(p),
                private_effort,
            })
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
pub struct PublicCommitment {
    id: String,
    issuer: Ministry,
    beneficiary: Ministry,
    #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
    proposal_version: String,
    kind: String,
    #[schemars(range(min = 0, max = 100))]
    funded_units: u16,
    trigger: String,
    #[schemars(range(min = 1))]
    deadline_quarter: u16,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    resolution_event: Option<String>,
}
impl TryFrom<&d::Commitment> for PublicCommitment {
    type Error = WireError;
    fn try_from(c: &d::Commitment) -> Result<Self, Self::Error> {
        identifier(&c.id)?;
        identifier(&c.trigger)?;
        if let Some(event) = &c.resolution_event {
            identifier(event)?;
        }
        if c.funded_units > d::PILOT_CREDITS {
            return Err(WireError::Field("commitment.funded_units"));
        }
        Ok(Self {
            id: c.id.clone(),
            issuer: c.issuer.into(),
            beneficiary: c.beneficiary.into(),
            proposal_version: c.proposal_version.to_decimal(),
            kind: match c.kind {
                d::CommitmentKind::Enforceable => "enforceable",
                d::CommitmentKind::Political => "political",
            }
            .into(),
            funded_units: c.funded_units,
            trigger: c.trigger.clone(),
            deadline_quarter: c.deadline_quarter.value(),
            status: match c.status {
                d::CommitmentStatus::Pending => "pending",
                d::CommitmentStatus::Honored => "honored",
                d::CommitmentStatus::Broken => "broken",
            }
            .into(),
            resolution_event: c.resolution_event.clone(),
        })
    }
}
impl From<d::Ministry> for Ministry {
    fn from(m: d::Ministry) -> Self {
        match m {
            d::Ministry::Finance => Self::Finance,
            d::Ministry::Education => Self::Education,
            d::Ministry::Infrastructure => Self::Infrastructure,
        }
    }
}

/// Server-only public response. Fields are private and no Deserialize exists:
/// only checked TryFrom<&RoleObservation> can construct serializable data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema, TS)]
pub struct PublicRoleView {
    schema_version: u16,
    campaign_id: String,
    meeting_id: String,
    #[schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))]
    proposal_version: String,
    ministry: Ministry,
    #[schemars(range(min = 1, max = 6))]
    quarter: u8,
    allocation: Allocation,
    #[schemars(range(min = 0, max = 100))]
    available_credits: u16,
    #[schemars(range(min = 0, max = 1000000))]
    belief_ppm: u32,
    owned_commitments: Vec<PublicCommitment>,
    visible_evidence_ids: Vec<String>,
}
impl TryFrom<&d::RoleObservation> for PublicRoleView {
    type Error = WireError;
    fn try_from(role: &d::RoleObservation) -> Result<Self, Self::Error> {
        role.validate()?;
        let commitments = role
            .owned_commitments
            .iter()
            .map(PublicCommitment::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            schema_version: role.schema_version(),
            campaign_id: role.campaign_id.clone(),
            meeting_id: role.meeting_id.clone(),
            proposal_version: role.proposal_version.to_decimal(),
            ministry: role.ministry.into(),
            quarter: role.quarter.value(),
            allocation: Allocation {
                staff: role.allocation.staff(),
                repairs: role.allocation.repairs(),
                access: role.allocation.access(),
            },
            available_credits: role.available_credits,
            belief_ppm: role.belief_ppm.value(),
            owned_commitments: commitments,
            visible_evidence_ids: role.visible_evidence_ids.clone(),
        })
    }
}
/// Offline optimistic output types, generated for the browser from Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind {
    OptimisticUpperBound,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PreviewCalculator {
    MythDomain,
}
impl From<d::PolicyFamily> for Family {
    fn from(value: d::PolicyFamily) -> Self {
        match value {
            d::PolicyFamily::Staffing => Self::Staffing,
            d::PolicyFamily::Repairs => Self::Repairs,
            d::PolicyFamily::Access => Self::Access,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ForecastLine {
    pub family: Family,
    pub per_quarter: [u16; 6],
    #[schemars(range(min = 0, max = 100))]
    pub outstanding: u16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ForecastResponse {
    pub kind: PreviewKind,
    pub calculator: PreviewCalculator,
    pub authoritative_simulation: bool,
    pub conditional_access_bonus: bool,
    pub families: Vec<ForecastLine>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ScriptedVoteKind { CleanBeliefScriptedVotes }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ScriptedSeatVote {
    pub ministry: Ministry,
    pub position: Vote,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ScriptedVotes {
    pub kind: ScriptedVoteKind,
    pub passes_support_threshold: bool,
    pub institutional_ratification: bool,
    pub votes: [ScriptedSeatVote; 3],
    #[schemars(range(min = 0, max = 3))]
    pub support_count: u8,
    pub warning: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    const CMD: &str = r#"{"schema_version":1,"campaign_id":"c","command_id":"k","expected_world_version":"18446744073709551615","command":{"kind":"propose","meeting_id":"m","allocation":{"staff":50,"repairs":40,"access":10}}}"#;
    fn minister(stage: &str, ministry: &str, effort: &str) -> String {
        let extra = if stage == "first" {
            r#","counterproposal_source":{"kind":"new_typed"}"#.to_owned()
        } else if effort == "omit" {
            String::new()
        } else {
            format!(r#","implementation_effort_permille":{effort}"#)
        };
        format!(
            r#"{{"schema_version":1,"meeting_id":"m","proposal_version":"1","ministry":"{ministry}","stage":"{stage}","position":"support","preferred_allocation":{{"staff":50,"repairs":40,"access":10}},"conditions":[],"evidence_ids":[],"rationale_summary":""{extra}}}"#
        )
    }
    #[test]
    fn unknown_fields_on_empty_variants_are_rejected() {
        let defer = CMD.replace(r#""kind":"propose","meeting_id":"m","allocation":{"staff":50,"repairs":40,"access":10}"#, r#""kind":"defer","extra":1"#);
        assert!(parse_command(&defer).is_err());
        let advance = CMD.replace(r#""kind":"propose","meeting_id":"m","allocation":{"staff":50,"repairs":40,"access":10}"#, r#""kind":"advance_quarter","meeting_id":"m""#);
        assert!(parse_command(&advance).is_err());
        let extra = minister("first", "education", "omit")
            .replace(r#""kind":"new_typed""#, r#""kind":"new_typed","x":1"#);
        assert!(parse_minister_action(&extra, 100).is_err());
    }
    #[test]
    fn stage_fields_must_not_cross_and_absent_finance_effort_is_valid() {
        assert!(parse_minister_action(&minister("final", "finance", "omit"), 100).is_ok());
        assert!(parse_minister_action(&minister("final", "education", "omit"), 100).is_err());
        let injected = minister("final", "education", "500").replace(
            r#""conditions":[],"#,
            r#""counterproposal_source":{"kind":"menu","menu_id":"zz"},"conditions":[],"#,
        );
        assert!(parse_minister_action(&injected, 100).is_err());
        let bad_first = minister("first", "education", "omit").replace(
            r#""conditions":[],"#,
            r#""implementation_effort_permille":800,"conditions":[],"#,
        );
        assert!(parse_minister_action(&bad_first, 100).is_err());
    }
    #[test]
    fn public_view_requires_domain_validation_and_preserves_commitments() {
        let mut role = d::RoleObservation::new(d::RoleObservationInput {
            campaign_id: "c".into(),
            meeting_id: "m".into(),
            proposal_version: d::WireU64::new(1),
            ministry: d::Ministry::Finance,
            quarter: d::Quarter::new(1).unwrap(),
            allocation: d::Allocation::draft(50, 40, 10).unwrap(),
            available_credits: 0,
            belief_ppm: d::BeliefPpm::new(500000).unwrap(),
        })
        .unwrap();
        role.owned_commitments.push(d::Commitment {
            id: "commit".into(),
            issuer: d::Ministry::Finance,
            beneficiary: d::Ministry::Education,
            proposal_version: d::WireU64::new(u64::MAX),
            kind: d::CommitmentKind::Political,
            funded_units: 5,
            trigger: "trigger".into(),
            deadline_quarter: d::DueQuarter::new(8).unwrap(),
            status: d::CommitmentStatus::Pending,
            resolution_event: None,
        });
        let wire = PublicRoleView::try_from(&role).unwrap();
        let json = serde_json::to_value(&wire).unwrap();
        assert_eq!(
            json["owned_commitments"][0]["proposal_version"],
            u64::MAX.to_string()
        );
        assert_eq!(json["owned_commitments"][0]["deadline_quarter"], 8);
        assert!(json.get("implementation_effort_permille").is_none());
        role.visible_evidence_ids.push("bad\u{202E}id".into());
        assert!(PublicRoleView::try_from(&role).is_err());
    }
    #[test]
    fn generated_schema_carries_domain_numeric_and_canonical_bounds() {
        let command = serde_json::to_value(schemars::schema_for!(CommandEnvelope)).unwrap();
        assert_eq!(
            command["properties"]["expected_world_version"]["pattern"],
            "^(0|[1-9][0-9]{0,19})$"
        );
        let minister = serde_json::to_value(schemars::schema_for!(MinisterAction)).unwrap();
        let as_text = minister.to_string();
        assert!(as_text.contains("1000"), "effort range missing");
        assert!(as_text.contains("maximum"), "numeric maxima missing");
        assert!(as_text.contains("counterproposal_source"));
    }
    #[test]
    fn version_string_roundtrip_including_max_u64() {
        let x: CommandEnvelope = serde_json::from_str(CMD).unwrap();
        let json = serde_json::to_string(&x).unwrap();
        assert_eq!(x, serde_json::from_str::<CommandEnvelope>(&json).unwrap());
        assert_eq!(
            parse_command(CMD).unwrap().expected_world_version.value(),
            u64::MAX
        );
        assert!(json.contains(r#""18446744073709551615""#));
    }
    #[test]
    fn reject_unknown_fields_wrong_versions_and_unquoted_versions() {
        let unknown = CMD.replace(r#""command_id":"k""#, r#""command_id":"k","extra":true"#);
        assert!(parse_command(&unknown).is_err());
        let bad = CMD.replace(r#""schema_version":1"#, r#""schema_version":2"#);
        assert!(parse_command(&bad).is_err());
        let numeric = CMD.replace(r#""18446744073709551615""#, "18446744073709551615");
        assert!(parse_command(&numeric).is_err());
        for v in ["01", "-1", "18446744073709551616", "1.0", ""] {
            assert!(parse_command(&CMD.replace("18446744073709551615", v)).is_err());
        }
    }
    #[test]
    fn reject_invalid_domains_and_caps() {
        assert!(parse_command(&CMD.replace(r#""staff":50"#, r#""staff":101"#)).is_err());
        assert!(parse_command(&CMD.replace(r#""campaign_id":"c""#, r#""campaign_id":""#)).is_err());
        let input = r#"{"schema_version":1,"campaign_id":"c","command_id":"x","expected_world_version":"0","command":{"kind":"request_capacity_cap","meeting_id":"m","caps":{"staff":13,"repairs":10,"access":4}}}"#;
        assert!(parse_command(input).is_err());
    }
    #[test]
    fn private_effort_role_isolated_and_bounded() {
        let a = parse_minister_action(&minister("final", "education", "800"), 100).unwrap();
        assert_eq!(a.private_effort.as_ref().unwrap().effort().value(), 800);
        if let PublicPosition::Final(p) = a.public {
            assert!(!format!("{p:?}").contains("800"));
        } else {
            panic!("wrong stage");
        }
        assert!(parse_minister_action(&minister("final", "finance", "800"), 100).is_err());
        assert!(parse_minister_action(&minister("final", "finance", "null"), 100).is_err());
        assert!(parse_minister_action(&minister("final", "education", "null"), 100).is_err());
        assert!(parse_minister_action(&minister("final", "education", "1001"), 100).is_err());
        assert!(parse_minister_action(&minister("first", "education", "800"), 100).is_ok());
        assert!(parse_minister_action(&minister("first", "education", "null"), 100).is_ok());
    }
    #[test]
    fn overbudget_counteroffer_is_invalid_not_minted() {
        let raw = minister("final", "education", "800").replace(r#""staff":50"#, r#""staff":60"#);
        let a = parse_minister_action(&raw, 100).unwrap();
        if let PublicPosition::Final(p) = a.public {
            assert_eq!(
                p.preferred_allocation.validity(),
                d::AllocationValidity::InvalidDraft
            );
        } else {
            panic!("wrong stage")
        }
    }
    #[test]
    fn private_action_rejects_extra_fields_and_public_view_is_separate() {
        let raw = minister("final", "education", "800");
        let extra = raw.replace(
            "\"rationale_summary\":\"\"",
            "\"rationale_summary\":\"\",\"private_leak\":true",
        );
        assert!(matches!(
            parse_minister_action(&extra, 100),
            Err(WireError::Json(_))
        ));
        let schema = serde_json::to_value(schemars::schema_for!(PublicRoleView)).unwrap();
        assert!(
            schema["properties"]
                .get("implementation_effort_permille")
                .is_none()
        );
    }

    #[test]
    fn offline_api_response_contracts_are_serde_and_typescript_stable() {
        let response = ForecastResponse {
            kind: PreviewKind::OptimisticUpperBound,
            calculator: PreviewCalculator::MythDomain,
            authoritative_simulation: false,
            conditional_access_bonus: true,
            families: vec![ForecastLine {
                family: Family::Access,
                per_quarter: [4, 5, 5, 5, 5, 1],
                outstanding: 0,
            }],
        };
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["calculator"], "myth_domain");
        assert_eq!(value["families"][0]["family"], "access");
        assert_eq!(serde_json::from_value::<ForecastResponse>(value).unwrap(), response);
        assert!(<ForecastResponse as TS>::decl(&ts_rs::Config::default())
            .contains("families: Array<ForecastLine>"));
        let vote = ScriptedVotes {
            kind: ScriptedVoteKind::CleanBeliefScriptedVotes,
            passes_support_threshold: true, institutional_ratification: false,
            votes: [
                ScriptedSeatVote { ministry: Ministry::Finance, position: Vote::Support },
                ScriptedSeatVote { ministry: Ministry::Education, position: Vote::Support },
                ScriptedSeatVote { ministry: Ministry::Infrastructure, position: Vote::Support },
            ],
            support_count: 3, warning: "Not a ratification".into(),
        };
        let wire = serde_json::to_value(&vote).unwrap();
        assert_eq!(wire["votes"][0]["ministry"], "finance");
        assert_eq!(serde_json::from_value::<ScriptedVotes>(wire).unwrap(), vote);
    }

    #[test]
    fn generated_schema_and_types_show_string_versions_not_private_effort() {
        let schema = serde_json::to_value(schemars::schema_for!(CommandEnvelope)).unwrap();
        assert_eq!(
            schema["properties"]["expected_world_version"]["type"],
            "string"
        );
        assert!(
            <CommandEnvelope as TS>::decl(&ts_rs::Config::default())
                .contains("expected_world_version: string")
        );
        assert!(
            !<PublicRoleView as TS>::decl(&ts_rs::Config::default())
                .contains("implementation_effort")
        );
    }
}
