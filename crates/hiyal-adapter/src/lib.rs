//! P4 private, single-world Hiyal adapter. Pure P3 rules remain independent.
//!
//! The private, versioned snapshot is an action-fold checkpoint: it retains
//! every admitted *typed logical action*, including private minister effort,
//! plus a digest of the reconstructed complete P3 campaign. It deliberately
//! does not embed free-text transcripts, LLM prompts, provider keys or UI state.
//! On untrusted resume Hiyal replays the committed journal; the reducer is
//! reconstituted from its private action fold and checked against the digest.
pub mod orchestrator;
use hiyal::core::{AgentId, World};
use myth_contracts::{PublicPosition, checkpoint::PrivateCheckpoint, parse_command, parse_minister_action};
use myth_domain as d;
use myth_domain::{reducer::{Action,Campaign,Rules}, scripted};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest,Sha256};

pub const WORLD_KIND: &str = "myth.england-education-hiyal.v1";
pub const MODEL_KIND: &str = "p0-profile-only-scripted.v1";
pub const PLAYER: AgentId = AgentId(0);
pub const FINANCE: AgentId = AgentId(1);
pub const EDUCATION: AgentId = AgentId(2);
pub const INFRASTRUCTURE: AgentId = AgentId(3);
pub const PERIOD: AgentId = AgentId(4);
pub const ACTORS: [AgentId;5] = [PLAYER,FINANCE,EDUCATION,INFRASTRUCTURE,PERIOD];
const MAX_ACCEPTED_ACTIONS: usize = 512;
const MAX_ACTION_BYTES: usize = 12_288;
const PACK: &str = include_str!("../../../data/england-school-pilot-v1/country-pack.json");
const SCENARIO: &str = include_str!("../../../data/england-school-pilot-v1/authored.json");
const MANIFEST: &str = include_str!("../../../data/england-school-pilot-v1/manifest.json");

fn sha(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
fn rules_fingerprint()->String {
    // Content identity survives an OS change and fails closed on a rules
    // implementation change, not just when an operator edits a version label.
    sha(concat!(
        include_str!("../../domain/src/reducer.rs"),
        include_str!("../../domain/src/lib.rs"),
        include_str!("../../domain/src/world.rs"),
        include_str!("../../domain/src/audit.rs"),
        include_str!("../../domain/src/scripted.rs"),
    ).as_bytes())
}
fn encoded<T:Serialize>(x:&T)->Result<Vec<u8>,String>{
    serde_json::to_vec(x).map_err(|e|e.to_string())
}
fn error<E:std::fmt::Debug>(e:E)->String{format!("{e:?}")}
fn actor_role(actor:AgentId)->Option<d::Ministry>{
    match actor.0 {
        1=>Some(d::Ministry::Finance),2=>Some(d::Ministry::Education),
        3=>Some(d::Ministry::Infrastructure),_=>None
    }
}
fn role_name(role:d::Ministry)->&'static str{
    match role{
        d::Ministry::Finance=>"finance",
        d::Ministry::Education=>"education",
        d::Ministry::Infrastructure=>"infrastructure",
    }
}
fn position_name(v:d::Position)->&'static str{
    match v{
        d::Position::Support=>"support",
        d::Position::Conditional=>"conditional",
        d::Position::Oppose=>"oppose",
    }
}
fn allocation_value(a:d::Allocation)->Value {
    json!({"staff":a.staff(),"repairs":a.repairs(),"access":a.access()})
}

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ActionKind { PlayerCommand, MinisterAction, FreezeFinal }

/// Private, replay-bound record, never publicly serialized in HTTP responses.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameAction {
    pub actor: u8,
    pub kind: ActionKind,
    pub payload: Value,
    /// Exact role observation digest at the moment the decision was made.
    pub observation_sha256: String,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameObservation {
    pub schema_version: u16,
    pub synthetic_receipts: bool,
    pub actor: u8,
    pub phase: String,
    pub quarter: u8,
    pub expected_world_version: String,
    pub action: GameAction,
}

/// Versioned fixed synthetic load records. They are not additional modeled
/// beneficiaries: the pure P3 society still has exactly six authored cells.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StressCohortRecord {
    pub index: u16,
    pub cell: u8,
    pub fictional_resource: u16,
}
/// All private state needed to *reconstruct* P3 exists in the bounded action
/// prefix and frozen genesis. No hidden in-memory-only minister effort survives
/// outside this fold; every persisted action is also a Hiyal journal event.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameWorld {
    pub schema_version: u16,
    pub campaign_id: String,
    pub pack_id: String,
    pub pack_sha256: String,
    pub scenario_sha256: String,
    pub rules_identity: String,
    pub weights_ppm: [u32;6],
    pub source_cutoff_utc: String,
    pub synthetic_receipts: bool,
    pub stress_cohorts: Vec<StressCohortRecord>,
    pub actions: Vec<GameAction>,
    /// Authoritative private state hash; not a public role projection.
    pub private_state_sha256: String,
    /// Hiyal synchronous rounds freeze all three role observations at
    /// the phase-entry boundary, not after each actor's partial commit.
    pub frozen_role_anchor: Option<String>,
    pub world_version: String,
    pub quarter: u8,
    pub phase: String,
}

impl GameWorld {
    pub fn initial(campaign_id:&str)->Result<Self,String>{
        let meta:Value=serde_json::from_str(MANIFEST).map_err(error)?;
        let expected_pack=meta["country_pack_sha256"].as_str().ok_or("missing P2 pack digest")?;
        let expected_scenario=meta["authored_sha256"].as_str().ok_or("missing P2 scenario digest")?;
        if sha(PACK.as_bytes())!=expected_pack
            || sha(SCENARIO.as_bytes())!=expected_scenario {
            return Err("frozen P2 source hashes do not match manifest".into());
        }
        let scenario:Value=serde_json::from_str(SCENARIO).map_err(error)?;
        let arr=scenario["population"]["variants"][0]["weights_ppm"]
            .as_array().ok_or("missing authored population")?;
        if arr.len()!=6 {return Err("invalid cohort count".into());}
        let mut weights=[0_u32;6];
        for (i,val) in arr.iter().enumerate(){
            weights[i]=u32::try_from(val.as_u64().ok_or("invalid cohort weight")?)
                .map_err(error)?;
        }
        let mut world=Self {
            schema_version:1,campaign_id:campaign_id.into(),
            pack_id:"england-school-delivery-v1".into(),
            pack_sha256:expected_pack.into(),scenario_sha256:expected_scenario.into(),
            rules_identity:rules_fingerprint(),
            weights_ppm:weights,source_cutoff_utc:"2026-10-29T23:59:59Z".into(),
            synthetic_receipts:false,stress_cohorts:vec![],
            actions:vec![], private_state_sha256:String::new(),
            frozen_role_anchor:None,
            world_version:"0".into(),quarter:1,phase:"quarter_open".into(),
        };
        let campaign=world.fold()?;
        world.refresh(&campaign)?;
        Ok(world)
    }
    /// P0-04 stress has 1,024 synthetic *world records* while P3 continues
    /// to model only six authored social cells. All 96 synthetic model receipts
    /// are generated by the no-network test backend, never a real provider.
    pub fn stress(campaign_id:&str)->Result<Self,String>{
        let mut world=Self::initial(campaign_id)?;
        world.synthetic_receipts=true;
        world.stress_cohorts=(0..1024_u16).map(|index|StressCohortRecord{
            index,cell:(index%6) as u8,
            fictional_resource:10+(index%17),
        }).collect();
        let campaign=world.fold()?;
        world.refresh(&campaign)?;
        Ok(world)
    }
    fn rules(&self)->Rules{
        Rules{q2_repair_cap:Some(8),..Rules::default()}
    }
    fn apply_record(campaign:&mut Campaign,record:&GameAction)->Result<(),String>{
        if encoded(record)?.len()>MAX_ACTION_BYTES {return Err("oversize game action".into());}
        let action=match record.kind {
            ActionKind::PlayerCommand=>{
                if record.actor!=PLAYER.0 as u8 && record.actor!=PERIOD.0 as u8 {
                    return Err("non-player actor attempted player command".into());
                }
                let cmd=parse_command(&record.payload.to_string()).map_err(error)?;
                let advance=matches!(cmd.command(),d::PlayerCommand::AdvanceQuarter);
                if (record.actor==PERIOD.0 as u8)!=advance {
                    return Err("period actor must exclusively advance time".into());
                }
                Action::Player(cmd)
            }
            ActionKind::MinisterAction=>{
                let role=actor_role(AgentId(u64::from(record.actor)))
                    .ok_or("unapproved minister actor")?;
                let available=campaign.role_observation(role).map_err(error)?.available_credits;
                let parsed=parse_minister_action(&record.payload.to_string(),available)
                    .map_err(error)?;
                let expected_role=match &parsed.public {
                    PublicPosition::First(p)=>p.ministry,
                    PublicPosition::Final(p)=>p.ministry,
                };
                if role!=expected_role{return Err("forged ministry action".into());}
                match parsed.public {
                    PublicPosition::First(p)=>Action::First(p),
                    PublicPosition::Final(p)=>Action::Final(p,parsed.private_effort),
                }
            }
            ActionKind::FreezeFinal=>{
                if record.actor!=PLAYER.0 as u8{return Err("freeze requires player".into());}
                let id=record.payload["command_id"].as_str()
                    .ok_or("missing freeze command id")?;
                Action::FreezeFinal{command_id:id.into()}
            }
        };
        campaign.apply(action).map_err(error)?;
        Ok(())
    }
    pub fn fold(&self)->Result<Campaign,String>{
        let manifest:Value=serde_json::from_str(MANIFEST).map_err(error)?;
        if self.pack_sha256!=manifest["country_pack_sha256"].as_str().unwrap_or("")
            || self.scenario_sha256!=manifest["authored_sha256"].as_str().unwrap_or("")
            || sha(PACK.as_bytes())!=self.pack_sha256
            || sha(SCENARIO.as_bytes())!=self.scenario_sha256
        {return Err("P2 source or authored scenario binding changed".into());}
        if self.schema_version!=1 || self.actions.len()>MAX_ACCEPTED_ACTIONS
            || self.pack_sha256.len()!=64 || self.scenario_sha256.len()!=64
            || self.rules_identity!=rules_fingerprint()
            || self.source_cutoff_utc!="2026-10-29T23:59:59Z"
            || (self.synthetic_receipts && self.stress_cohorts.len()!=1024)
            || (!self.synthetic_receipts && !self.stress_cohorts.is_empty())
            || self.stress_cohorts.iter().enumerate().any(|(i,row)|
                usize::from(row.index)!=i || row.cell!=(i%6) as u8
                || row.fictional_resource!=10+(i as u16%17))
        {return Err("unsupported or oversized P4 world version".into());}
        let mut campaign=Campaign::with_rules(
            &self.campaign_id,&self.pack_id,d::CohortWeights::new(self.weights_ppm)
                .map_err(error)?,self.rules()).map_err(error)?;
        for record in &self.actions {
            Self::apply_record(&mut campaign,record)?;
        }
        Ok(campaign)
    }
    fn digest_campaign(&self,campaign:&Campaign)->Result<String,String>{
        let snapshot=PrivateCheckpoint::capture(campaign.world(),self.pack_sha256.clone())
            .map_err(error)?;
        // Replaying all privately authenticated actions reconstitutes every
        // private P3 field (meeting rounds, effort, due scopes, audits, beliefs).
        // These are not exposed via the P1 public DTO or web API.
        Ok(sha(&encoded(&(snapshot,&self.actions,&self.scenario_sha256,
            &self.rules_identity,&self.weights_ppm,&self.stress_cohorts,
            self.synthetic_receipts,MODEL_KIND))?))
    }
    fn phase_of(campaign:&Campaign)->&'static str{
        use d::world::MeetingPhase;
        match campaign.phase(){
            MeetingPhase::QuarterOpen=>"quarter_open",
            MeetingPhase::FirstPositionsFrozen=>"first_positions_frozen",
            MeetingPhase::AmendmentWindow=>"amendment_window",
            MeetingPhase::FinalPositionsFrozen=>"final_positions_frozen",
            MeetingPhase::Resolved=>"resolved",
            MeetingPhase::QuarterClosed=>"quarter_closed",
        }
    }
    fn refresh(&mut self,campaign:&Campaign)->Result<(),String>{
        self.private_state_sha256=self.digest_campaign(campaign)?;
        self.world_version=campaign.world().world_version().to_decimal();
        self.quarter=campaign.world().quarter().value();
        self.phase=Self::phase_of(campaign).into();
        Ok(())
    }
    /// Full private fold check on a restored world, not trusting snapshot
    /// checksums alone. The complete Hiyal journal still must be verified.
    pub fn verify(&self)->Result<(),String>{
        let campaign=self.fold()?;
        let expected_anchor = if self.phase=="first_positions_frozen"
            || self.phase=="final_positions_frozen" {
            let entry_kind = if self.phase=="first_positions_frozen" {"propose"}
                else {"freeze_final"};
            let entry = self.actions.iter().rposition(|a| {
                if entry_kind=="freeze_final" {a.kind==ActionKind::FreezeFinal}
                else {a.kind==ActionKind::PlayerCommand
                    && a.payload["command"]["kind"]=="propose"}
            }).ok_or("frozen phase lacks boundary action")?;
            let mut start=self.clone();
            start.actions.truncate(entry+1);
            let begin=start.fold()?;
            Some(start.digest_campaign(&begin)?)
        } else {None};
        if expected_anchor!=self.frozen_role_anchor {
            return Err("frozen role observation boundary mismatch".into());
        }
        if campaign.world().world_version().to_decimal()!=self.world_version
            || campaign.world().quarter().value()!=self.quarter
            || Self::phase_of(&campaign)!=self.phase
            || self.digest_campaign(&campaign)?!=self.private_state_sha256
        {return Err("private campaign fold does not match checkpoint".into());}
        Ok(())
    }
    pub fn is_closed(&self)->bool{self.phase=="quarter_closed"}
    fn meeting_id(&self)->String{format!("q{}",self.quarter)}
    pub fn final_votes_this_meeting(&self)->usize{
        let mut count=0;
        for record in self.actions.iter().rev() {
            if record.kind==ActionKind::PlayerCommand
                && record.payload["command"]["kind"]=="propose" {break;}
            if record.kind==ActionKind::MinisterAction
                && record.payload["stage"]=="final" {count+=1;}
        }
        count
    }
    fn planned(&self,actor:AgentId)->Result<GameAction,String>{
        let campaign=self.fold()?;
        let meeting=self.meeting_id();
        let kind:ActionKind;
        let payload:Value;
        if actor==PLAYER {
            kind=ActionKind::PlayerCommand;
            if self.phase=="amendment_window" {
                return Ok(GameAction{
                    actor:actor.0 as u8,kind:ActionKind::FreezeFinal,
                    payload:json!({"command_id":format!("{meeting}-freeze")}),
                    observation_sha256:self.private_state_sha256.clone(),
                });
            }
            let body=match self.phase.as_str(){
                "quarter_open"=>json!({"kind":"propose","meeting_id":meeting,
                    "allocation":if self.quarter==1 {
                        json!({"staff":50,"repairs":40,"access":10})
                    } else {
                        json!({"staff":0,"repairs":0,"access":0})
                    }}),
                "final_positions_frozen"=>{
                    if self.final_votes_this_meeting()!=3 {
                        return Err("must finish frozen final votes before player decision".into());
                    }
                    let mut supporting=0;
                    for record in self.actions.iter().rev() {
                        if record.kind==ActionKind::PlayerCommand
                            && record.payload["command"]["kind"]=="propose" {break;}
                        if record.kind==ActionKind::MinisterAction
                            && record.payload["stage"]=="final"
                            && record.payload["position"]=="support"
                        {supporting+=1;}
                    }
                    if supporting>=2 {
                        json!({"kind":"ratify","meeting_id":meeting,
                            "proposal_version":campaign.proposal_version()
                                .ok_or("meeting lost version")?.to_decimal()})
                    } else {json!({"kind":"defer"})}
                }
                _=>return Err("player not eligible in this phase".into()),
            };
            payload=json!({"schema_version":1,"campaign_id":self.campaign_id,
                "command_id":format!("{meeting}-player-{}",self.world_version),
                "expected_world_version":self.world_version,"command":body});
        } else if actor==PERIOD {
            if self.phase!="resolved" {return Err("period actor not eligible".into());}
            kind=ActionKind::PlayerCommand;
            payload=json!({"schema_version":1,"campaign_id":self.campaign_id,
                "command_id":format!("{meeting}-advance"),
                "expected_world_version":self.world_version,
                "command":{"kind":"advance_quarter"}});
        } else {
            let role=actor_role(actor).ok_or("unknown cabinet actor")?;
            if self.phase!="first_positions_frozen"
                && self.phase!="final_positions_frozen" {
                return Err("minister actor not eligible".into());
            }
            let round=self.phase.as_str();
            for record in self.actions.iter().rev() {
                if record.kind==ActionKind::PlayerCommand
                    && record.payload["command"]["kind"]=="propose" {break;}
                if record.kind==ActionKind::MinisterAction
                    && record.payload["ministry"]==role_name(role)
                    && ((round=="first_positions_frozen" && record.payload["stage"]=="first")
                    || (round=="final_positions_frozen" && record.payload["stage"]=="final"))
                {return Err("minister already acted in frozen round".into());}
            }
            let view=campaign.role_observation(role).map_err(error)?;
            let decision=scripted::decide(&view).map_err(error)?;
            let first=round=="first_positions_frozen";
            let preference=if first {decision.first.preferred_allocation.allocation()}
                else {decision.final_position.preferred_allocation.allocation()};
            let position=if first {decision.first.position}
                else {decision.final_position.vote};
            kind=ActionKind::MinisterAction;
            payload=if first {
                let origin=match &decision.first.counterproposal_source {
                    d::CounterproposalOrigin::Menu(id)=>json!({"kind":"menu","menu_id":id}),
                    d::CounterproposalOrigin::NewTyped=>json!({"kind":"new_typed"}),
                };
                json!({"stage":"first","schema_version":1,"meeting_id":meeting,
                    "proposal_version":view.proposal_version.to_decimal(),
                    "ministry":role_name(role),"position":position_name(position),
                    "preferred_allocation":allocation_value(preference),
                    "counterproposal_source":origin,"conditions":[],"evidence_ids":[],
                    "rationale_summary":"Deterministic P0 profile decision"})
            } else {
                let mut output=json!({"stage":"final","schema_version":1,
                    "meeting_id":meeting,"proposal_version":view.proposal_version.to_decimal(),
                    "ministry":role_name(role),"position":position_name(position),
                    "preferred_allocation":allocation_value(preference),
                    "conditions":[],"evidence_ids":[],
                    "rationale_summary":"Deterministic P0 profile final vote"});
                if role!=d::Ministry::Finance {
                    output["implementation_effort_permille"]=json!(1000);
                }
                output
            };
        }
        Ok(GameAction {
            actor:u8::try_from(actor.0).map_err(error)?,kind,payload,
            observation_sha256:if actor_role(actor).is_some() {
                self.frozen_role_anchor.clone().ok_or("missing frozen role anchor")?
            } else { self.private_state_sha256.clone() },
        })
    }
}
impl World for GameWorld {
    type Action=GameAction;
    type Observation=GameObservation;
    fn observe(&self,actor:AgentId)->Result<Self::Observation,String>{
        self.verify()?;
        let action=self.planned(actor)?;
        Ok(GameObservation{
            schema_version:1,synthetic_receipts:self.synthetic_receipts,
            actor:u8::try_from(actor.0).map_err(error)?,
            phase:self.phase.clone(),quarter:self.quarter,
            expected_world_version:self.world_version.clone(),action,
        })
    }
    fn apply(&mut self,actor:AgentId,action:GameAction)->Result<(),String>{
        self.verify()?;
        if u64::from(action.actor)!=actor.0{return Err("actor mismatch".into());}
        let expected_anchor=if actor_role(actor).is_some() {
            self.frozen_role_anchor.as_ref().ok_or("missing frozen role anchor")?
        } else {&self.private_state_sha256};
        if &action.observation_sha256!=expected_anchor {
            return Err("stale or unbound observation".into());
        }
        // The P4 demonstration policy chooses deterministic scripted actions,
        // but a future P6 recorded player command is admitted by the SAME P3
        // typed reducer after the Hiyal keyed player_command receipt.
        // This world never authorizes an operation merely from free text.
        if self.actions.len()>=MAX_ACCEPTED_ACTIONS {return Err("action cap exceeded".into());}
        let mut candidate=self.clone();
        candidate.actions.push(action);
        let result=candidate.fold()?;
        let was_phase=self.phase.clone();
        candidate.refresh(&result)?;
        candidate.frozen_role_anchor =
            if candidate.phase=="first_positions_frozen"
                || candidate.phase=="final_positions_frozen" {
                if candidate.phase!=was_phase {
                    Some(candidate.private_state_sha256.clone())
                } else {self.frozen_role_anchor.clone()}
            } else {None};
        *self=candidate;
        Ok(())
    }
}
