//! Versioned, validated Serde inputs for game commands and minister actions.
//! This crate is an adapter; authoritative rules remain in myth-domain.
#![forbid(unsafe_code)]
use myth_domain as d;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const VERSION: u16 = d::SCHEMA_VERSION;
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
            c.kind_id, c.family.into(), c.units, d::DueQuarter::new(c.due_quarter)?
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
        proposal_version: String,
        allocation: Allocation,
    },
    Ratify {
        meeting_id: String,
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
        #[serde(default)]
        #[ts(optional)]
        #[schemars(range(min = 0, max = 1000))]
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
pub fn parse_minister_action(raw: &str, available_credits: u16)
    -> Result<AcceptedMinisterAction, WireError>
{
    let action: MinisterAction =
        serde_json::from_str(raw).map_err(|e| WireError::Json(e.to_string()))?;
    match action {
        MinisterAction::First {
            schema_version, meeting_id, proposal_version, ministry, position,
            preferred_allocation, counterproposal_source, conditions,
            evidence_ids, rationale_summary,
        } => {
            schema(schema_version)?;
            let minister: d::Ministry = ministry.into();
            let parsed_version = version(&proposal_version)?;
            let preferred = d::PreferredAllocation::assess(
                preferred_allocation.try_into()?, available_credits)?;
            let checked: Vec<d::FundedCondition> = conditions
                .into_iter().map(TryInto::try_into).collect::<Result<_, _>>()?;
            let origin = match counterproposal_source {
                Counterproposal::Menu { menu_id } => {
                    identifier(&menu_id)?;
                    d::CounterproposalOrigin::Menu(menu_id)
                }
                Counterproposal::NewTyped {} => d::CounterproposalOrigin::NewTyped,
            };
            let mut p = d::FirstPosition::new(
                meeting_id, parsed_version, minister, position.into(), preferred, origin)?;
            p.set_public_details(checked, evidence_ids, rationale_summary)?;
            Ok(AcceptedMinisterAction { public: PublicPosition::First(p), private_effort: None })
        }
        MinisterAction::Final {
            schema_version, meeting_id, proposal_version, ministry, position,
            preferred_allocation, conditions, evidence_ids, rationale_summary,
            implementation_effort_permille,
        } => {
            schema(schema_version)?;
            let minister: d::Ministry = ministry.into();
            let parsed_version = version(&proposal_version)?;
            let preferred = d::PreferredAllocation::assess(
                preferred_allocation.try_into()?, available_credits)?;
            let checked: Vec<d::FundedCondition> = conditions
                .into_iter().map(TryInto::try_into).collect::<Result<_, _>>()?;
            let private_effort = match (minister, implementation_effort_permille) {
                (d::Ministry::Finance, None) => None,
                (d::Ministry::Finance, Some(_)) => return Err(WireError::Field("finance_effort")),
                (_, None) => return Err(WireError::Field("missing_effort")),
                (_, Some(n)) => Some(d::PrivateEffortDeclaration::new(
                    meeting_id.clone(), minister, d::EffortPermille::new(n)?)?),
            };
            let mut p = d::FinalPosition::new(
                meeting_id, parsed_version, minister, position.into(), preferred)?;
            p.set_public_details(checked, evidence_ids, rationale_summary)?;
            Ok(AcceptedMinisterAction { public: PublicPosition::Final(p), private_effort })
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PublicRoleView {
    pub schema_version: u16,
    pub campaign_id: String,
    pub meeting_id: String,
    pub proposal_version: String,
    pub ministry: Ministry,
    pub quarter: u8,
    pub allocation: Allocation,
    pub available_credits: u16,
    pub belief_ppm: u32,
    pub visible_evidence_ids: Vec<String>,
}
impl PublicRoleView {
    pub fn validate(&self) -> Result<(), WireError> {
        schema(self.schema_version)?;
        identifier(&self.campaign_id)?;
        identifier(&self.meeting_id)?;
        version(&self.proposal_version)?;
        d::Quarter::new(self.quarter)?;
        d::Allocation::try_from(self.allocation.clone())?;
        d::BeliefPpm::new(self.belief_ppm)?;
        if self.available_credits > d::PILOT_CREDITS {
            return Err(WireError::Field("available_credits"));
        }
        for e in &self.visible_evidence_ids {
            identifier(e)?
        }
        Ok(())
    }
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
