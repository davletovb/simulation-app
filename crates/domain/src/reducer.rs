//! P3 first slice: deterministic, no-I/O, two-round authorization reducer.
//! All admission happens against a staged clone; failures never mutate the
//! campaign, treasury, funded scopes, phases or private minister effort.
//! P4 will own replay serialization; P5 may only supply typed role actions.
use crate::audit::{BeliefLedger, BeliefReceipt, DeliveryAudit};
use crate::news::{self, Landed, Response, Scheduled, Tone};
use crate::world::{
    LedgerReceipt, MeetingPhase, Organisation, OrganisationAccount, QueuedResponse, SupportPpm,
    WorldSnapshot,
};
use crate::{
    Allocation, CapacityCaps, CohortWeights, CommandEnvelope, Commitment, CommitmentKind,
    CommitmentStatus, ContractError, DueQuarter, EffortPermille, FinalPosition, FirstPosition,
    FundedScope, LedgerMove, MAX_QUARTERS, Ministry, PolicyFamily, Position,
    PrivateEffortDeclaration, Quarter, TERM_ALLOCATION_CREDITS, WireU64,
    deliver_family_quarter_with_lag, validate_identifier,
};
use std::collections::{BTreeMap, BTreeSet};

/// Authored public-response model, recalibrated by decision 0003 (`docs/decisions`).
///
/// Mirrored in `data/england-school-pilot-v1/authored.json` and cross-checked by a test in
/// `myth-contracts`, so the data file and the code cannot drift apart silently. Order is
/// staffing, repairs, access, cost: ppm of support per delivered unit, before the geography and
/// income multipliers. Before decision 0003 this was `[500, 400, 750, 400]`.
pub const RESPONSE_BASE_PPM: [i128; 4] = [3200, 2800, 3000, 2000];
/// Geography multipliers (urban, rural) per channel, in percent. Unchanged by decision 0003.
pub const GEOGRAPHY_PERCENT: [[i128; 4]; 2] = [[100, 100, 100, 100], [80, 130, 140, 100]];
/// Income multipliers (low, middle, high) per channel, in percent. Unchanged by decision 0003.
pub const INCOME_PERCENT: [[i128; 4]; 3] = [
    [120, 90, 130, 130],
    [100, 100, 100, 100],
    [80, 110, 70, 150],
];
/// The most one quarter's delivery can move one group's support, either way (was 8_000).
pub const RESPONSE_CLIP_PPM: i64 = 40_000;
/// The most one scripted exogenous event can move one group's support, either way (was 8_000).
pub const SHOCK_CAP_PPM: u32 = 40_000;

/// The election bar (decisions 0004 and 0009): win with at least this overall support, and nothing else. How the
/// support is spread across the groups does not matter, so a player may change course and win different groups.
pub const ELECTION_MIN_SUPPORT_PPM: u32 = 550_000;
/// A group "backs" the player only if its support is strictly above this. This no longer decides anything: it is
/// what the screen counts as the groups behind you.
pub const ELECTION_HELD_ABOVE_PPM: u32 = 500_000;

/// What an election would return. The bar only reads a state, but the verdict decides whether the game
/// goes on, so it is part of the hashed scenario (`authored.json`, kept equal by a drift test) and
/// changing it invalidates saved games (decision 0007, which reverses 0006 on this point).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElectionProjection {
    /// Support per group once every queued response has landed (urban low, middle, high, then rural).
    pub group_support_ppm: [u32; 6],
    /// Overall support: the groups' support weighted by their share of the voters.
    pub weighted_support_ppm: u32,
    /// Share of the voters (ppm) that sit in groups backing the player, above [`ELECTION_HELD_ABOVE_PPM`].
    /// Information for the screen; it plays no part in who wins.
    pub blocs_held_ppm: u32,
}
impl ElectionProjection {
    pub fn meets_support_bar(&self) -> bool {
        self.weighted_support_ppm >= ELECTION_MIN_SUPPORT_PPM
    }
    /// Overall support decides. The groups' own votes are already inside it, weighted by their share of the voters.
    pub fn is_win(&self) -> bool {
        self.meets_support_bar()
    }
}
/// The election rule as a pure function of the six groups' support and their voter shares.
pub fn evaluate_election(
    group_support_ppm: [u32; 6],
    weights: &CohortWeights,
) -> Result<ElectionProjection, Rejection> {
    let shares = weights.parts_ppm();
    let mut weighted = 0_u128;
    let mut held = 0_u32;
    for (support, share) in group_support_ppm.iter().zip(shares.iter()) {
        weighted += u128::from(*support) * u128::from(*share);
        if *support > ELECTION_HELD_ABOVE_PPM {
            held = held.checked_add(*share).ok_or(Rejection::Arithmetic)?;
        }
    }
    Ok(ElectionProjection {
        group_support_ppm,
        weighted_support_ppm: u32::try_from(weighted / 1_000_000)
            .map_err(|_| Rejection::Arithmetic)?,
        blocs_held_ppm: held,
    })
}

/// A headline the player has been told about and has not yet seen land.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpcomingHeadline {
    pub scheduled: Scheduled,
    /// What it would do to each group's support if it landed now, with no response: the mood it takes plus, for a
    /// disruption, the support lost because work is delayed past the election.
    pub expected: [i32; 6],
    /// What a disruption would do to delivery, if this headline is one.
    pub delivery: Option<DeliveryQuote>,
    /// What a response would cost now; `None` for good news, once a response has been paid, and when the headline would
    /// cost this player nothing.
    pub relief_cost: Option<u16>,
    /// The credits already paid to answer it, if they were.
    pub answered: Option<u16>,
}

/// How a disruption would hit the work the player has promised.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeliveryQuote {
    pub family: PolicyFamily,
    /// How much of its usual capacity that kind of work keeps, in percent, and for how many quarters.
    pub keep_pct: u32,
    pub quarters: u8,
    /// Units that would be delivered after the term's election instead of before it.
    pub delayed_units: u16,
    /// The support those units would have earned by the election, per group, taken away.
    pub effect: [i32; 6],
}

struct Quote {
    total: [i32; 6],
    delivery: Option<DeliveryQuote>,
    cost: Option<u16>,
}

/// The verdict of one election, kept for the rest of the game. It is derived while the journal is replayed,
/// so it is as reproducible as everything else, and it is what decides whether the game goes on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElectionRecord {
    /// Which term this election ended (1-based).
    pub term: u8,
    /// The last quarter of that term.
    pub quarter: Quarter,
    pub projection: ElectionProjection,
    pub won: bool,
}

/// Authored, immutable per-run offline mechanism assumptions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rules {
    pub magnitude_percent: u16,
    pub delivery_extra_lag: u16,
    pub response_lag: u16,
    pub shocks: [[i32; 6]; MAX_QUARTERS as usize],
    pub q2_repair_cap: Option<u16>,
    /// The news calendar (decision 0010) runs when there is a seed. Without one (the fixed-action sensitivity runs, most
    /// unit tests) no headline ever lands.
    pub news_seed: Option<u64>,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            magnitude_percent: 100,
            delivery_extra_lag: 0,
            response_lag: 1,
            shocks: [[0; 6]; MAX_QUARTERS as usize],
            q2_repair_cap: None,
            news_seed: None,
        }
    }
}
impl Rules {
    pub fn validate(&self) -> Result<(), Rejection> {
        if ![50, 100, 150].contains(&self.magnitude_percent)
            || self.delivery_extra_lag > 1
            || !(1..=2).contains(&self.response_lag)
            || self.q2_repair_cap.is_some_and(|x| x > 10)
            || self
                .shocks
                .iter()
                .flatten()
                .any(|v| v.unsigned_abs() > SHOCK_CAP_PPM)
        {
            return Err(Rejection::InvalidCondition);
        }
        Ok(())
    }
}
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
/// The explicitly public observation boundary. No actual delivered units,
/// fractional carry, true minister effort or undisclosed audit figures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicQuarterReport {
    pub quarter: Quarter,
    pub reported_cumulative: [u16; 3],
    pub due_cumulative: [u16; 3],
    pub weighted_support_ppm: u32,
    pub staff_slowdown_next: bool,
    pub access_bonus_next: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicVote {
    pub quarter: Quarter,
    pub ministry: Ministry,
    pub proposal_version: WireU64,
    pub vote: Position,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleView {
    pub observation: crate::RoleObservation,
    pub constituency_support_ppm: u32,
    pub latest_public_report: Option<PublicQuarterReport>,
    pub visible_audits: Vec<DeliveryAudit>,
    pub prior_public_votes: Vec<PublicVote>,
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
    /// Credits are earmarked to the scope created by this ratification.
    /// A unit from any older/other scope cannot satisfy this obligation.
    scope_id: String,
    credited: u16,
}
#[derive(Clone)]
pub struct Campaign {
    world: WorldSnapshot,
    meeting: Option<Meeting>,
    meeting_used: bool,
    used_meeting_ids: BTreeSet<String>,
    command_ids: BTreeSet<String>,
    caps: CapacityCaps,
    private_efforts: [EffortPermille; 2],
    slowdown_due: bool,
    bonus_due: bool,
    delivered_lifetime: [u16; 3],
    conditions: Vec<ConditionProgress>,
    relationship: [i16; 3],
    reports: Vec<QuarterReport>,
    rules: Rules,
    shadow_scopes: Vec<FundedScope>,
    active_package: Allocation,
    scheduled_lifetime: [u16; 3],
    audits: Vec<DeliveryAudit>,
    beliefs: BeliefLedger,
    disclosed: BTreeMap<Ministry, BTreeSet<String>>,
    vote_history: Vec<PublicVote>,
    /// One record per election held so far, in order.
    elections: Vec<ElectionRecord>,
    /// Audits ordered in the current term; the first of each term is free (decision 0005).
    term_audits: u16,
    /// Headlines that have landed, oldest first (decision 0010).
    landed: Vec<Landed>,
    /// Responses that were paid for, oldest first.
    responses: Vec<Response>,
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
        Self::with_rules(campaign_id, pack_id, weights, Rules::default())
    }
    pub fn with_rules(
        campaign_id: &str,
        pack_id: &str,
        weights: CohortWeights,
        rules: Rules,
    ) -> Result<Self, Rejection> {
        rules.validate()?;
        Ok(Self {
            world: WorldSnapshot::initial(campaign_id.into(), pack_id.into(), weights)?,
            meeting: None,
            meeting_used: false,
            used_meeting_ids: BTreeSet::new(),
            command_ids: BTreeSet::new(),
            caps: CapacityCaps::new(12, 10, 4)?,
            private_efforts: [EffortPermille::new(1000)?, EffortPermille::new(1000)?],
            slowdown_due: false,
            bonus_due: false,
            delivered_lifetime: [0; 3],
            conditions: vec![],
            relationship: [50; 3],
            reports: vec![],
            rules,
            shadow_scopes: vec![],
            active_package: Allocation::default(),
            scheduled_lifetime: [0; 3],
            audits: vec![],
            beliefs: BeliefLedger::default(),
            disclosed: BTreeMap::new(),
            vote_history: vec![],
            elections: vec![],
            term_audits: 0,
            landed: vec![],
            responses: vec![],
        })
    }
    /// The elections held so far. After the last quarter of a term there is a new entry, and its `won`
    /// decides whether the game goes on.
    pub fn elections(&self) -> &[ElectionRecord] {
        &self.elections
    }
    /// Headlines that have landed, oldest first.
    pub fn landed_headlines(&self) -> &[Landed] {
        &self.landed
    }
    /// Responses that were paid for.
    pub fn responses(&self) -> &[Response] {
        &self.responses
    }
    /// Announced headlines that have not landed yet and are close enough to answer: those landing at the end of this
    /// quarter or the next. Surprises are never listed (nobody is told), and nothing is listed without a calendar.
    pub fn upcoming_headlines(&self) -> Vec<UpcomingHeadline> {
        let Some(seed) = self.rules.news_seed else {
            return vec![];
        };
        let now = self.world.quarter();
        if matches!(
            self.world.phase(),
            MeetingPhase::QuarterClosed | MeetingPhase::ElectionNight
        ) {
            return vec![];
        }
        news::schedule(seed, now.term())
            .into_iter()
            .filter(|s| {
                !s.surprise
                    && s.quarter >= now
                    && s.quarter.value() <= now.value() + 1
                    && !self.landed.iter().any(|l| l.scheduled.quarter == s.quarter)
            })
            .filter_map(|scheduled| {
                let answered = self
                    .responses
                    .iter()
                    .find(|r| r.headline_quarter == scheduled.quarter)
                    .map(|r| r.credits);
                // Once answered, the quote shows what the answer bought: the softened loss and the softened cut.
                let quote = self.quote(&scheduled, answered.is_some()).ok()?;
                Some(UpcomingHeadline {
                    scheduled,
                    expected: quote.total,
                    delivery: quote.delivery,
                    relief_cost: (scheduled.tone == Tone::Bad && answered.is_none())
                        .then_some(quote.cost)
                        .flatten(),
                    answered,
                })
            })
            .collect()
    }
    /// What a headline would do if it landed now, and what answering it would cost: the mood it takes plus, for a
    /// disruption, the support lost because work is delayed past the election (decision 0011).
    fn quote(&self, scheduled: &Scheduled, answered: bool) -> Result<Quote, Rejection> {
        let support = self.support_values();
        let weights = *self.world.cohort_weights().parts_ppm();
        let mut total = news::landing_shock(scheduled, scheduled.quarter.term(), &support);
        if answered {
            total = news::soften(total);
        }
        let mut delivery = None;
        if scheduled.tone == Tone::Bad
            && let Some(d) = scheduled.headline().disrupts
        {
            let from = scheduled.quarter.value();
            let during = move |q: u8| (from..from.saturating_add(d.quarters)).contains(&q);
            let kept = if answered {
                news::softened_keep(d.keep_pct)
            } else {
                d.keep_pct
            };
            let usual = self.projected_delivery(d.family, |_| 100)?;
            let cut = self.projected_delivery(d.family, |q| if during(q) { kept } else { 100 })?;
            let delayed = usual.saturating_sub(cut);
            let mut lost = [0_u16; 3];
            lost[news::family_slot(d.family)] = delayed;
            let mut effect = [0_i32; 6];
            for (ix, e) in effect.iter_mut().enumerate() {
                // The support those units would have earned by the election, taken away (or, where the work would have
                // cost a group more than it gave, given back).
                let earned = cohort_response_scaled(ix, lost, self.rules.magnitude_percent)?;
                *e = i32::try_from(-earned).map_err(|_| Rejection::Arithmetic)?;
                total[ix] = total[ix].saturating_add(*e);
            }
            delivery = Some(DeliveryQuote {
                family: d.family,
                keep_pct: kept,
                quarters: d.quarters,
                delayed_units: delayed,
                effect,
            });
        }
        // Nothing to protect, nothing to sell: a headline that would cost this player nothing cannot be answered.
        let cost = (news::overall_loss_ppm(&total, &weights) > 0)
            .then(|| news::relief_cost(&total, &weights));
        Ok(Quote {
            total,
            delivery,
            cost,
        })
    }
    /// Units of `family` that would be delivered between now and the end of the term, at full effort and with nothing
    /// changed in the plan, if the family could deliver only `keep(q)` percent of its usual capacity in quarter `q`.
    fn projected_delivery(
        &self,
        family: PolicyFamily,
        keep: impl Fn(u8) -> u32,
    ) -> Result<u16, Rejection> {
        let parts = self.world.private_checkpoint_parts();
        let mut scopes: Vec<FundedScope> = parts
            .scopes
            .iter()
            .filter(|s| s.family() == family)
            .cloned()
            .collect();
        let now = self.world.quarter();
        let term_end = now.term() * crate::TERM_QUARTERS;
        let mut total = 0_u16;
        for q in now.value()..=term_end {
            let usual = match family {
                PolicyFamily::Staffing => self.caps.staff(),
                PolicyFamily::Repairs if q == 2 => self
                    .rules
                    .q2_repair_cap
                    .map_or(self.caps.repairs(), |v| v.min(self.caps.repairs())),
                PolicyFamily::Repairs => self.caps.repairs(),
                PolicyFamily::Access => self.caps.access(),
            };
            let cap = u16::try_from(u32::from(usual) * keep(q) / 100).unwrap_or(usual);
            let delivered = deliver_family_quarter_with_lag(
                family,
                cap,
                false,
                EffortPermille::new(1000)?,
                DueQuarter::from(Quarter::new(q)?),
                self.rules.delivery_extra_lag,
                &mut scopes,
            )?;
            total = total.saturating_add(delivered.total_delivered);
        }
        Ok(total)
    }
    fn support_values(&self) -> [u32; 6] {
        let mut out = [0_u32; 6];
        for (o, cell) in out.iter_mut().zip(self.world.support()) {
            *o = cell.value();
        }
        out
    }
    pub fn world(&self) -> &WorldSnapshot {
        &self.world
    }
    /// Counterfactual *reporting* projection only. Q6's authoritative score is
    /// unchanged; apply already-queued responses through their fixed due dates
    /// without inventing future deliveries or charging additional credits.
    pub(crate) fn settle_pending_support(&self) -> Result<([u32; 6], usize), Rejection> {
        let mut cells = [0_u32; 6];
        for (ix, cell) in cells.iter_mut().enumerate() {
            *cell = self.world.support()[ix].value();
        }
        let mut pending = self.world.queued_responses().to_vec();
        pending.sort_by(|a, b| {
            a.due_quarter()
                .cmp(&b.due_quarter())
                .then_with(|| a.source_event_id().cmp(b.source_event_id()))
        });
        for event in &pending {
            for (ix, cell) in cells.iter_mut().enumerate() {
                let delta =
                    cohort_response_scaled(ix, event.delivered(), self.rules.magnitude_percent)?;
                *cell = u32::try_from((i64::from(*cell) + delta).clamp(0, 1_000_000))
                    .map_err(|_| Rejection::Arithmetic)?;
            }
        }
        Ok((cells, pending.len()))
    }

    /// The election if it were held now: every response already queued lands first (the last
    /// quarter's work counts) and nothing new is delivered. Once the campaign is over this is the
    /// result; before that it is a projection. It reads state and never changes it.
    pub fn election_projection(&self) -> Result<ElectionProjection, Rejection> {
        let (settled, _) = self.settle_pending_support()?;
        evaluate_election(settled, self.world.cohort_weights())
    }

    pub(crate) fn reports(&self) -> &[QuarterReport] {
        &self.reports
    }
    pub fn player_reports(&self) -> Vec<PublicQuarterReport> {
        self.reports
            .iter()
            .map(|x| PublicQuarterReport {
                quarter: x.quarter,
                reported_cumulative: x.reported,
                due_cumulative: x.due_schedule,
                weighted_support_ppm: x.weighted_support_ppm,
                staff_slowdown_next: x.staff_slowdown_next,
                access_bonus_next: x.access_bonus_next,
            })
            .collect()
    }
    pub fn role_view(&self, ministry: Ministry) -> Result<RoleView, Rejection> {
        let observation = self.role_observation(ministry)?;
        let latest = self.player_reports().into_iter().last();
        let mut weighted = 0_u128;
        for ix in 0..6 {
            weighted += u128::from(self.world.support()[ix].value())
                * u128::from(self.world.cohort_weights().parts_ppm()[ix]);
        }
        Ok(RoleView {
            observation,
            constituency_support_ppm: u32::try_from(weighted / 1_000_000)
                .map_err(|_| Rejection::Arithmetic)?,
            latest_public_report: latest,
            prior_public_votes: self.vote_history.clone(),
            visible_audits: self
                .audits
                .iter()
                .filter(|audit| {
                    self.disclosed
                        .get(&ministry)
                        .is_some_and(|ids| ids.contains(&audit.id))
                })
                .cloned()
                .collect(),
        })
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
    #[cfg(test)]
    fn private_effort_for(&self, ministry: Ministry) -> Option<u16> {
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
        let mut view = self.world.role_observation(
            ministry,
            meeting.id.clone(),
            meeting.version,
            meeting.allocation,
            crate::BeliefPpm::new(self.beliefs.belief(ministry))?,
        )?;
        // A replacement proposal can release undelivered, unprotected
        // encumbrances. The role sees the net fundable campaign stock, not just
        // the current 'available' treasury bucket; legal protection is checked
        // independently at proposal admission and ratification.
        let fundable = u32::from(self.world.treasury().available())
            + [
                PolicyFamily::Staffing,
                PolicyFamily::Repairs,
                PolicyFamily::Access,
            ]
            .iter()
            .map(|f| u32::from(self.world.treasury().encumbered(*f)))
            .sum::<u32>();
        view.available_credits = u16::try_from(fundable).map_err(|_| Rejection::Arithmetic)?;
        view.visible_evidence_ids = self
            .disclosed
            .get(&ministry)
            .map_or_else(Vec::new, |ids| ids.iter().cloned().collect());
        view.validate()?;
        Ok(view)
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
                        if self.used_meeting_ids.contains(meeting_id) {
                            return Err(Rejection::MeetingAlreadyUsed);
                        }
                        self.check_funding(*allocation)?;
                        self.used_meeting_ids.insert(meeting_id.clone());
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
                            || m.amended
                        {
                            return Err(Rejection::InvalidPhase);
                        }
                        // A cap change is a new proposal version; no old vote
                        // may authorize it and there is only one amendment.
                        m.caps = *caps;
                        m.version = WireU64::new(
                            m.version
                                .value()
                                .checked_add(1)
                                .ok_or(Rejection::Arithmetic)?,
                        );
                        m.amended = true;
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
                        if matches!(
                            self.world.phase(),
                            MeetingPhase::QuarterClosed | MeetingPhase::ElectionNight
                        ) {
                            return Err(Rejection::InvalidPhase);
                        }
                        if self.meeting.is_some() {
                            self.change_phase(MeetingPhase::Resolved)?;
                        }
                    }
                    crate::PlayerCommand::AdvanceQuarter => self.advance(receipt)?,
                    crate::PlayerCommand::BeginTerm => self.begin_term(receipt)?,
                    crate::PlayerCommand::Respond { headline_quarter } => {
                        self.respond(*headline_quarter, receipt)?;
                    }
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
                            self.disclosed
                                .entry(*ministry)
                                .or_default()
                                .insert(evidence.id.clone());
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
    fn protected_scope_units(
        &self,
        proposal: Allocation,
    ) -> Result<BTreeMap<String, u16>, Rejection> {
        let mut retained = BTreeMap::<String, u16>::new();
        let mut family_totals = [0_u16; 3];
        for commitment in self.world.commitments().iter().filter(|item| {
            item.kind == CommitmentKind::Enforceable && item.status == CommitmentStatus::Pending
        }) {
            let entry = self
                .conditions
                .iter()
                .find(|p| p.id == commitment.id)
                .ok_or(Rejection::InvalidCondition)?;
            if entry.credited > commitment.funded_units {
                return Err(Rejection::InvalidCondition);
            }
            let owed = commitment.funded_units - entry.credited;
            let prior = retained.get(&entry.scope_id).copied().unwrap_or(0);
            retained.insert(
                entry.scope_id.clone(),
                prior.checked_add(owed).ok_or(Rejection::Arithmetic)?,
            );
            let ix = family_index(entry.family);
            family_totals[ix] = family_totals[ix]
                .checked_add(owed)
                .ok_or(Rejection::Arithmetic)?;
        }
        for family in FAMILIES {
            if family_totals[family_index(family)] > proposal.units(family) {
                return Err(Rejection::ProtectedCommitment);
            }
        }
        for (scope_id, owed) in &retained {
            let scope = self
                .world
                .scopes()
                .iter()
                .find(|s| s.id() == scope_id)
                .ok_or(Rejection::ProtectedCommitment)?;
            if scope.is_terminated() || scope.remaining_units() < *owed {
                return Err(Rejection::ProtectedCommitment);
            }
        }
        Ok(retained)
    }
    fn check_funding(&self, proposal: Allocation) -> Result<(), Rejection> {
        self.protected_scope_units(proposal)?;
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
        // Preserve oldest scopes needed for authoritative pending obligations.
        // Never move fractional work from one scope to another.
        let protected = self.protected_scope_units(m.allocation)?;
        let mut kept = [0_u16; 3];
        for scope in &mut parts.scopes {
            if scope.is_terminated() || scope.remaining_units() == 0 {
                continue;
            }
            let remaining = scope.remaining_units();
            let held = protected.get(scope.id()).copied().unwrap_or(0);
            let releasable = remaining.checked_sub(held).ok_or(Rejection::Arithmetic)?;
            if held == 0 {
                scope.terminate();
            } else if releasable > 0 {
                *scope = FundedScope::restore(
                    scope.id().to_string(),
                    scope.family(),
                    scope.activation_quarter(),
                    held,
                    scope.carry_permille(),
                    scope.is_blocked(),
                    false,
                )?;
            }
            if let Some(shadow) = self
                .shadow_scopes
                .iter_mut()
                .find(|item| item.id() == scope.id())
            {
                if held == 0 {
                    shadow.terminate();
                } else if shadow.remaining_units() > held {
                    *shadow = FundedScope::restore(
                        shadow.id().to_string(),
                        shadow.family(),
                        shadow.activation_quarter(),
                        held,
                        shadow.carry_permille(),
                        shadow.is_blocked(),
                        false,
                    )?;
                }
            }
            let ix = family_index(scope.family());
            kept[ix] = kept[ix].checked_add(held).ok_or(Rejection::Arithmetic)?;
            if releasable > 0 {
                let movement = LedgerMove::Release {
                    family: scope.family(),
                    credits: releasable,
                };
                let before = parts.treasury;
                parts.treasury = before.apply(movement)?;
                receipt
                    .ledger
                    .push(LedgerReceipt::new(before, movement, parts.treasury)?);
            }
        }
        let mut freshly_funded = BTreeMap::<PolicyFamily, (String, u16)>::new();
        for family in FAMILIES {
            let units = m
                .allocation
                .units(family)
                .checked_sub(kept[family_index(family)])
                .ok_or(Rejection::ProtectedCommitment)?;
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
            let scope = FundedScope::new(
                format!("{}-v{}-{key}", m.id, m.version.value()),
                family,
                self.world.quarter(),
                units,
            )?;
            freshly_funded.insert(family, (scope.id().to_string(), units));
            self.shadow_scopes.push(scope.clone());
            parts.scopes.push(scope);
        }
        let mut condition_ids = BTreeSet::new();
        let mut protected = [0_u16; 3];
        for vote in m
            .final_votes
            .values()
            .filter(|v| v.vote == Position::Support)
        {
            for condition in vote.conditions() {
                // Only Finance may condition any family; the other two seats
                // cannot bind funds outside their own delivery remit.
                if (vote.ministry == Ministry::Education
                    && condition.family() != PolicyFamily::Staffing)
                    || (vote.ministry == Ministry::Infrastructure
                        && condition.family() != PolicyFamily::Repairs)
                {
                    return Err(Rejection::WrongAuthority);
                }
                if condition.due().value() < u16::from(self.world.quarter().value())
                    || condition.due().value() > u16::from(MAX_QUARTERS)
                    || !condition_ids.insert(condition.kind_id().to_string())
                {
                    return Err(Rejection::InvalidCondition);
                }
                // Milestones must be demonstrably deliverable by their due date.
                // An approved but impossible promise is institutionally invalid.
                let eligible_q = u16::from(self.world.quarter().value())
                    .checked_add(condition.family().lag_quarters())
                    .and_then(|v| v.checked_add(self.rules.delivery_extra_lag))
                    .ok_or(Rejection::Arithmetic)?;
                if condition.due().value() < eligible_q {
                    return Err(Rejection::InvalidCondition);
                }
                let periods = condition.due().value() - eligible_q + 1;
                let family_cap = match condition.family() {
                    PolicyFamily::Staffing => m.caps.staff(),
                    PolicyFamily::Repairs => m.caps.repairs(),
                    PolicyFamily::Access => m.caps.access(),
                };
                if u32::from(condition.units()) > u32::from(periods) * u32::from(family_cap) {
                    return Err(Rejection::InvalidCondition);
                }
                let ix = family_index(condition.family());
                protected[ix] = protected[ix]
                    .checked_add(condition.units())
                    .ok_or(Rejection::Arithmetic)?;
                let (condition_scope, available_new) = freshly_funded
                    .get(&condition.family())
                    .ok_or(Rejection::InvalidCondition)?;
                // New conditions must use newly funded credits, not units
                // still reserved for obligations from earlier meetings.
                if protected[ix] > *available_new {
                    return Err(Rejection::InvalidCondition);
                }
                let condition_scope = condition_scope.clone();
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
                    scope_id: condition_scope,
                    credited: 0,
                });
                parts.commitments.push(commitment);
            }
        }
        self.caps = m.caps;
        self.active_package = m.allocation;
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
        if self.world.quarter().in_term() < 3
            || inspected > self.world.quarter()
            || matches!(
                self.world.phase(),
                MeetingPhase::QuarterClosed | MeetingPhase::ElectionNight
            )
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
        let cost = if self.term_audits == 0 { 0 } else { 2 };
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
        self.term_audits = self.term_audits.saturating_add(1);
        Ok(())
    }
    /// Start the next term. Only possible on election night after a win. A fresh allocation is added to
    /// anything unspent (R3); the support, lobbies, ministers' trust and unfinished promises all carry on,
    /// because this is the same campaign rather than a new one (decision 0007).
    /// Pay to soften an announced bad headline before it lands (decision 0010).
    fn respond(
        &mut self,
        headline_quarter: Quarter,
        receipt: &mut ReducerReceipt,
    ) -> Result<(), Rejection> {
        let seed = self.rules.news_seed.ok_or(Rejection::InvalidCondition)?;
        let now = self.world.quarter();
        if matches!(
            self.world.phase(),
            MeetingPhase::QuarterClosed | MeetingPhase::ElectionNight
        ) {
            return Err(Rejection::InvalidPhase);
        }
        // Only a headline that has been announced and has not landed: this quarter's or the next one's.
        if headline_quarter < now || headline_quarter.value() > now.value() + 1 {
            return Err(Rejection::InvalidCondition);
        }
        let scheduled = news::schedule(seed, headline_quarter.term())
            .into_iter()
            .find(|s| s.quarter == headline_quarter)
            .ok_or(Rejection::InvalidCondition)?;
        if scheduled.tone != Tone::Bad
            || scheduled.surprise
            || self
                .responses
                .iter()
                .any(|r| r.headline_quarter == headline_quarter)
        {
            return Err(Rejection::InvalidCondition);
        }
        // The price is the one the player was shown: what the headline would cost them now. A headline that would cost them
        // nothing (a disruption to work they have not promised) cannot be answered.
        let credits = self
            .quote(&scheduled, false)?
            .cost
            .ok_or(Rejection::InvalidCondition)?;
        let mut parts = self.world.private_checkpoint_parts();
        let movement = LedgerMove::SpendRelief { credits };
        let before = parts.treasury;
        parts.treasury = before.apply(movement)?;
        receipt
            .ledger
            .push(LedgerReceipt::new(before, movement, parts.treasury)?);
        self.world = WorldSnapshot::from_parts(parts)?;
        self.responses.push(Response {
            headline_quarter,
            credits,
        });
        Ok(())
    }
    fn begin_term(&mut self, receipt: &mut ReducerReceipt) -> Result<(), Rejection> {
        let q = self.world.quarter();
        let won = self
            .elections
            .last()
            .is_some_and(|e| e.won && e.quarter == q);
        if self.world.phase() != MeetingPhase::ElectionNight || !won {
            return Err(Rejection::InvalidPhase);
        }
        let mut parts = self.world.private_checkpoint_parts();
        let movement = LedgerMove::Grant {
            credits: TERM_ALLOCATION_CREDITS,
        };
        let before = parts.treasury;
        parts.treasury = before.apply(movement)?;
        receipt
            .ledger
            .push(LedgerReceipt::new(before, movement, parts.treasury)?);
        parts.quarter = Quarter::new(q.value() + 1)?;
        parts.phase = MeetingPhase::QuarterOpen;
        self.meeting = None;
        self.meeting_used = false;
        self.term_audits = 0;
        self.world = WorldSnapshot::from_parts(parts)?;
        Ok(())
    }
    fn advance(&mut self, receipt: &mut ReducerReceipt) -> Result<(), Rejection> {
        if self.meeting.is_some() && self.world.phase() != MeetingPhase::Resolved {
            return Err(Rejection::InvalidPhase);
        }
        let q = self.world.quarter();
        if matches!(
            self.world.phase(),
            MeetingPhase::QuarterClosed | MeetingPhase::ElectionNight
        ) {
            return Err(Rejection::InvalidPhase);
        }
        let mut parts = self.world.private_checkpoint_parts();
        let caps = [
            self.caps.staff(),
            if q.value() == 2 {
                self.rules
                    .q2_repair_cap
                    .map_or(self.caps.repairs(), |v| v.min(self.caps.repairs()))
            } else {
                self.caps.repairs()
            },
            self.caps.access(),
        ];
        // Disruptions in the news cut what a kind of work can deliver this quarter (decision 0011). The due schedule is cut the
        // same way, so a minister is never reported as behind schedule for something the world did.
        let kept = self.rules.news_seed.map_or([100_u32; 3], |seed| {
            news::capacity_kept(seed, q, |hq| {
                self.responses.iter().any(|r| r.headline_quarter == hq)
            })
        });
        let caps: [u16; 3] = std::array::from_fn(|ix| {
            u16::try_from(u32::from(caps[ix]) * kept[ix] / 100).unwrap_or(caps[ix])
        });
        // A volunteer bonus is part of capacity: it does not survive a stoppage.
        let access_bonus = self.bonus_due && kept[2] == 100;
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
            // Maintain a separate full-effort due-schedule scope ledger.
            // Actual minister effort never rewrites historical scheduled work.
            let shadow_ids: Vec<usize> = self
                .shadow_scopes
                .iter()
                .enumerate()
                .filter(|(_, s)| s.family() == *family)
                .map(|(i, _)| i)
                .collect();
            let mut shadow_family: Vec<FundedScope> = shadow_ids
                .iter()
                .map(|i| self.shadow_scopes[*i].clone())
                .collect();
            let due = deliver_family_quarter_with_lag(
                *family,
                caps[ix],
                ix == 2 && access_bonus,
                EffortPermille::new(1000)?,
                DueQuarter::from(q),
                self.rules.delivery_extra_lag,
                &mut shadow_family,
            )?;
            for (index, updated) in shadow_ids.into_iter().zip(shadow_family) {
                self.shadow_scopes[index] = updated;
            }
            scheduled[ix] = due.total_delivered;
            self.scheduled_lifetime[ix] = self.scheduled_lifetime[ix]
                .checked_add(due.total_delivered)
                .ok_or(Rejection::Arithmetic)?;
            let delivery = deliver_family_quarter_with_lag(
                *family,
                caps[ix],
                ix == 2 && access_bonus,
                EffortPermille::new(efforts[ix])?,
                DueQuarter::from(q),
                self.rules.delivery_extra_lag,
                &mut scoped,
            )?;
            // Allocate each delivered service unit once, within its funded
            // scope, to pending obligations ordered by deadline then ID.
            for completed in &delivery.scopes {
                let mut unassigned = completed.delivered;
                let mut claims: Vec<(u16, String, usize)> = self
                    .conditions
                    .iter()
                    .enumerate()
                    .filter_map(|(j, info)| {
                        if info.scope_id != completed.scope_id {
                            return None;
                        }
                        let c = parts.commitments.iter().find(|c| c.id == info.id)?;
                        if c.status != CommitmentStatus::Pending {
                            return None;
                        }
                        Some((c.deadline_quarter.value(), info.id.clone(), j))
                    })
                    .collect();
                claims.sort();
                for (_, id, j) in claims {
                    if unassigned == 0 {
                        break;
                    }
                    let claim = parts
                        .commitments
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or(Rejection::InvalidCondition)?;
                    let outstanding = claim
                        .funded_units
                        .checked_sub(self.conditions[j].credited)
                        .ok_or(Rejection::Arithmetic)?;
                    let credited = outstanding.min(unassigned);
                    self.conditions[j].credited = self.conditions[j]
                        .credited
                        .checked_add(credited)
                        .ok_or(Rejection::Arithmetic)?;
                    unassigned -= credited;
                }
            }
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
        // The news calendar: a headline due this quarter lands now, before the queued responses (decision 0010). What it
        // takes depends on where each group stands, a response softens it, and good news is a flat gift.
        let mut headline_shock = [0_i32; 6];
        if let Some(seed) = self.rules.news_seed
            && let Some(scheduled) = news::schedule(seed, q.term())
                .into_iter()
                .find(|s| s.quarter == q)
        {
            let mut support = [0_u32; 6];
            for (now, cell) in support.iter_mut().zip(parts.support.iter()) {
                *now = cell.value();
            }
            let mut shock = news::landing_shock(&scheduled, q.term(), &support);
            let responded = scheduled.tone == Tone::Bad
                && self.responses.iter().any(|r| r.headline_quarter == q);
            if responded {
                shock = news::soften(shock);
            }
            headline_shock = shock;
            self.landed.push(Landed {
                scheduled,
                shock,
                responded,
            });
        }
        // Frozen exogenous shocks precede previously queued cohort responses.
        for (ix, headline) in headline_shock.iter().enumerate() {
            let candidate = (i64::from(parts.support[ix].value())
                + i64::from(self.rules.shocks[usize::from(q.value() - 1)][ix])
                + i64::from(*headline))
            .clamp(0, 1_000_000);
            parts.support[ix] =
                SupportPpm::new(u32::try_from(candidate).map_err(|_| Rejection::Arithmetic)?)?;
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
                let delta =
                    cohort_response_scaled(ix, response.delivered(), self.rules.magnitude_percent)?;
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
                DueQuarter::new(u16::from(q.value()) + self.rules.response_lag)?,
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
            let honored = progress.credited >= commitment.funded_units;
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
        let funded_staffing = self.active_package.staff();
        let staff_resources = parts.organisations[0].resources().saturating_add(2).min(20);
        let next_slowdown =
            q.value() < MAX_QUARTERS && funded_staffing < 50 && staff_resources >= 4;
        parts.organisations[0] = OrganisationAccount::new(
            Organisation::SchoolWorkforceAssociation,
            staff_resources - if next_slowdown { 4 } else { 0 },
        )?;
        let access_resources = parts.organisations[1].resources().saturating_add(1).min(12);
        let next_bonus = q.value() < MAX_QUARTERS
            && parts.treasury.encumbered(PolicyFamily::Access) > 0
            && self.active_package.access() >= 10
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
        if q.ends_term() {
            // Provisionally closed: the election below decides whether the game goes on.
            parts.phase = MeetingPhase::QuarterClosed;
        } else {
            parts.quarter = Quarter::new(q.value() + 1)?;
            parts.phase = MeetingPhase::QuarterOpen;
        }
        if let Some(meeting) = self.meeting.as_ref() {
            for action in meeting.final_votes.values() {
                self.vote_history.push(PublicVote {
                    quarter: q,
                    ministry: action.ministry,
                    proposal_version: action.proposal_version,
                    vote: action.vote,
                });
            }
        }
        self.meeting = None;
        self.meeting_used = false;
        self.world = WorldSnapshot::from_parts(parts)?;
        if q.ends_term() {
            // Election night: the verdict is derived from the settled state and recorded. A loss ends the
            // game. A win waits for BeginTerm, however many terms came before: the game goes on for as long
            // as the player keeps winning. Only the technical ceiling (MAX_TERMS) ends a won game.
            let projection = self.election_projection()?;
            let won = projection.is_win();
            self.elections.push(ElectionRecord {
                term: q.term(),
                quarter: q,
                projection,
                won,
            });
            if won && q.value() < MAX_QUARTERS {
                self.change_phase(MeetingPhase::ElectionNight)?;
            }
        }
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
#[cfg(test)]
fn cohort_response(ix: usize, delivered: [u16; 3]) -> Result<i64, Rejection> {
    cohort_response_scaled(ix, delivered, 100)
}
fn cohort_response_scaled(
    ix: usize,
    delivered: [u16; 3],
    magnitude_percent: u16,
) -> Result<i64, Rejection> {
    let geography = GEOGRAPHY_PERCENT;
    let income = INCOME_PERCENT;
    let base = RESPONSE_BASE_PPM;
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
    let scaled = result
        .checked_mul(i128::from(magnitude_percent))
        .ok_or(Rejection::Arithmetic)?
        / 100;
    let clip = i128::from(RESPONSE_CLIP_PPM);
    i64::try_from(scaled.clamp(-clip, clip)).map_err(|_| Rejection::Arithmetic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CounterproposalOrigin, MAX_TERMS, PILOT_CREDITS, PlayerCommand, PreferredAllocation,
        Treasury,
    };

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
        // The authored Q2 repairs cap is intentionally inert: a Q1
        // repairs scope only becomes eligible in Q3.
        assert_eq!(b.delivered[1], 0);
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
    fn funded_deadline_is_protected_until_delivered_and_resolved_once() {
        use crate::FundedCondition;
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        for role in ROLES {
            first(&mut c, role);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        let mut finance = FinalPosition::new(
            "q1".into(),
            c.proposal_version().unwrap(),
            Ministry::Finance,
            Position::Support,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
        )
        .unwrap();
        finance
            .set_public_details(
                vec![
                    FundedCondition::new(
                        "staff-by-q2".into(),
                        PolicyFamily::Staffing,
                        12,
                        DueQuarter::new(2).unwrap(),
                    )
                    .unwrap(),
                ],
                vec![],
                "funded staffing deadline".into(),
            )
            .unwrap();
        c.apply(Action::Final(finance, None)).unwrap();
        final_vote(&mut c, Ministry::Education, Position::Support, 1000);
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 1000);
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        assert_eq!(c.world().commitments().len(), 1);
        assert_eq!(c.world().treasury().service_spent(), 0);
        c.apply(player(&c, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        let before = c.world().world_version();
        assert_eq!(
            c.apply(player(
                &c,
                "propose-q2",
                PlayerCommand::Propose {
                    meeting_id: "q2".into(),
                    allocation: Allocation::draft(10, 30, 10).unwrap()
                }
            ))
            .err(),
            Some(Rejection::ProtectedCommitment)
        );
        assert_eq!(c.world().world_version(), before);
        // A smaller but lawful Q2 amendment retains exactly 12 protected
        // staff credits in the original scope, and releases the other 38.
        c.apply(player(
            &c,
            "propose-q2-allowed",
            PlayerCommand::Propose {
                meeting_id: "q2".into(),
                allocation: Allocation::draft(30, 30, 10).unwrap(),
            },
        ))
        .unwrap();
        for role in ROLES {
            let pos = FirstPosition::new(
                "q2".into(),
                c.proposal_version().unwrap(),
                role,
                Position::Support,
                PreferredAllocation::assess(Allocation::draft(30, 30, 10).unwrap(), 100).unwrap(),
                CounterproposalOrigin::NewTyped,
            )
            .unwrap();
            c.apply(Action::First(pos)).unwrap();
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze-q2".into(),
        })
        .unwrap();
        for role in ROLES {
            let pos = FinalPosition::new(
                "q2".into(),
                c.proposal_version().unwrap(),
                role,
                Position::Support,
                PreferredAllocation::assess(Allocation::draft(30, 30, 10).unwrap(), 100).unwrap(),
            )
            .unwrap();
            let effort = if role == Ministry::Finance {
                None
            } else {
                Some(
                    PrivateEffortDeclaration::new(
                        "q2".into(),
                        role,
                        EffortPermille::new(1000).unwrap(),
                    )
                    .unwrap(),
                )
            };
            c.apply(Action::Final(pos, effort)).unwrap();
        }
        c.apply(player(
            &c,
            "ratify-q2",
            PlayerCommand::Ratify {
                meeting_id: "q2".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        let staff: Vec<_> = c
            .world()
            .scopes()
            .iter()
            .filter(|scope| scope.family() == PolicyFamily::Staffing && !scope.is_terminated())
            .collect();
        assert_eq!(staff.len(), 2);
        assert_eq!(
            staff
                .iter()
                .map(|scope| scope.remaining_units())
                .sum::<u16>(),
            30
        );
        assert_eq!(c.world().treasury().available(), 26);
        c.apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.world().commitments()[0].status, CommitmentStatus::Honored);
        assert_eq!(c.relationship()[0], 52);
        assert_eq!(c.world().treasury().service_spent(), 21);
        c.apply(player(
            &c,
            "propose-q3",
            PlayerCommand::Propose {
                meeting_id: "q3".into(),
                allocation: Allocation::draft(20, 30, 10).unwrap(),
            },
        ))
        .unwrap();
        assert_eq!(c.world().quarter().value(), 3);
        c.world().validate().unwrap();
    }
    #[test]
    fn game_truth_audit_free_then_paid_and_duplicate_disclosure_does_not_double_move_belief() {
        use crate::EvidenceRecipients;
        let mut c = campaign();
        open(&mut c, Allocation::draft(40, 30, 20).unwrap());
        for role in ROLES {
            first(&mut c, role);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        final_vote(&mut c, Ministry::Finance, Position::Support, 1000);
        final_vote(&mut c, Ministry::Education, Position::Support, 500);
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 1000);
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        c.apply(player(&c, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        c.apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.world().quarter().value(), 3);
        assert_eq!(c.reports()[1].due_schedule[0], 12);
        assert_eq!(c.reports()[1].delivered_cumulative[0], 6);
        let free = c
            .apply(player(
                &c,
                "first-audit",
                PlayerCommand::OrderAudit {
                    family: PolicyFamily::Staffing,
                    inspected_quarter: Quarter::new(2).unwrap(),
                },
            ))
            .unwrap();
        assert!(free.ledger.is_empty());
        let first = c.player_audits()[0].clone();
        assert_eq!(first.cost, 0);
        let disclosed = c
            .apply(player(
                &c,
                "disclose1",
                PlayerCommand::DiscloseEvidence {
                    evidence_id: first.id.clone(),
                    evidence_version: first.version,
                    recipients: EvidenceRecipients::new(vec![Ministry::Education]).unwrap(),
                },
            ))
            .unwrap();
        assert_eq!(disclosed.belief_updates.len(), 1);
        assert_eq!(disclosed.belief_updates[0].delta, -80000);
        assert_eq!(c.belief_for(Ministry::Education), 420000);
        assert_eq!(c.belief_for(Ministry::Infrastructure), 500000);
        let dup = c
            .apply(player(
                &c,
                "disclose2",
                PlayerCommand::DiscloseEvidence {
                    evidence_id: first.id,
                    evidence_version: first.version,
                    recipients: EvidenceRecipients::new(vec![Ministry::Education]).unwrap(),
                },
            ))
            .unwrap();
        assert_eq!(dup.belief_updates[0].delta, 0);
        let paid = c
            .apply(player(
                &c,
                "second-audit",
                PlayerCommand::OrderAudit {
                    family: PolicyFamily::Staffing,
                    inspected_quarter: Quarter::new(2).unwrap(),
                },
            ))
            .unwrap();
        assert_eq!(paid.ledger.len(), 1);
        assert_eq!(c.world().treasury().audit_spent(), 2);
        assert_eq!(c.player_audits()[1].version.value(), 2);
        let new = c.player_audits()[1].clone();
        let correction = c
            .apply(player(
                &c,
                "disclose3",
                PlayerCommand::DiscloseEvidence {
                    evidence_id: new.id,
                    evidence_version: new.version,
                    recipients: EvidenceRecipients::new(vec![Ministry::Education]).unwrap(),
                },
            ))
            .unwrap();
        assert_eq!(correction.belief_updates[0].delta, 0);
        assert_eq!(c.belief_for(Ministry::Education), 420000);
        c.apply(player(
            &c,
            "q3-proposal",
            PlayerCommand::Propose {
                meeting_id: "q3".into(),
                allocation: Allocation::draft(30, 20, 10).unwrap(),
            },
        ))
        .unwrap();
        let education = c.role_view(Ministry::Education).unwrap();
        let infrastructure = c.role_view(Ministry::Infrastructure).unwrap();
        assert_eq!(education.visible_audits.len(), 2);
        assert_eq!(education.observation.visible_evidence_ids.len(), 2);
        assert_eq!(infrastructure.visible_audits.len(), 0);
        assert!(infrastructure.observation.visible_evidence_ids.is_empty());
        c.world().validate().unwrap();
    }
    #[test]
    fn cap_change_invalidates_original_proposal_version_and_cannot_hide_second_amendment() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        let prior = c.proposal_version().unwrap();
        c.apply(player(
            &c,
            "cap-edit",
            PlayerCommand::RequestCapacityCap {
                meeting_id: "q1".into(),
                caps: CapacityCaps::new(10, 8, 4).unwrap(),
            },
        ))
        .unwrap();
        assert_eq!(c.proposal_version().unwrap().value(), prior.value() + 1);
        for role in ROLES {
            first(&mut c, role);
        }
        assert!(
            c.apply(player(
                &c,
                "second-edit",
                PlayerCommand::Amend {
                    meeting_id: "q1".into(),
                    proposal_version: c.proposal_version().unwrap(),
                    allocation: Allocation::draft(60, 30, 10).unwrap(),
                }
            ))
            .is_err()
        );
        assert_eq!(c.caps().staff(), 12);
        assert_eq!(c.world().treasury().available(), 100);
    }
    #[test]
    fn player_and_minister_public_views_hide_inspected_truth_before_audit() {
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
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 1000);
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        c.apply(player(&c, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        c.apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.reports()[1].delivered_cumulative[0], 6);
        let public = c.player_reports();
        assert_eq!(public[1].reported_cumulative[0], 8);
        assert_eq!(public[1].due_cumulative[0], 12);
        // Public role view is independently constructible only during an open
        // meeting. No private effort, truth or unpublished audit field is in it.
        c.apply(player(
            &c,
            "propose-q3",
            PlayerCommand::Propose {
                meeting_id: "q3".into(),
                allocation: Allocation::draft(30, 20, 10).unwrap(),
            },
        ))
        .unwrap();
        let role = c.role_view(Ministry::Education).unwrap();
        assert_eq!(role.latest_public_report.unwrap().reported_cumulative[0], 8);
        assert_eq!(role.observation.ministry, Ministry::Education);
        assert_eq!(role.prior_public_votes.len(), 3);
        assert_eq!(role.prior_public_votes[0].ministry, Ministry::Finance);
        assert!(role.constituency_support_ppm <= 1_000_000);
        assert_eq!(c.private_effort_for(Ministry::Education), Some(500));
    }
    #[test]
    fn missed_funded_staffing_deadline_changes_relationship_once_without_double_spend() {
        use crate::FundedCondition;
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        for role in ROLES {
            first(&mut c, role);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        let mut finance = FinalPosition::new(
            "q1".into(),
            c.proposal_version().unwrap(),
            Ministry::Finance,
            Position::Support,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
        )
        .unwrap();
        finance
            .set_public_details(
                vec![
                    FundedCondition::new(
                        "staff-by-q2".into(),
                        PolicyFamily::Staffing,
                        12,
                        DueQuarter::new(2).unwrap(),
                    )
                    .unwrap(),
                ],
                vec![],
                "enforceable staffing deadline".into(),
            )
            .unwrap();
        c.apply(Action::Final(finance, None)).unwrap();
        final_vote(&mut c, Ministry::Education, Position::Support, 0);
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 1000);
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        c.apply(player(&c, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        c.apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.world().commitments()[0].status, CommitmentStatus::Broken);
        assert_eq!(c.relationship()[0], 45);
        assert_eq!(c.world().treasury().encumbered(PolicyFamily::Staffing), 50);
        assert_eq!(c.world().treasury().service_spent(), 9);
        c.apply(player(&c, "advance3", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.relationship()[0], 45);
        c.world().validate().unwrap();
    }
    #[test]
    fn organisation_actions_consume_resources_only_for_next_quarter() {
        let mut no_policy = campaign();
        let a = no_policy
            .apply(player(
                &no_policy,
                "advance1",
                PlayerCommand::AdvanceQuarter,
            ))
            .unwrap();
        assert!(a.report.unwrap().staff_slowdown_next);
        assert_eq!(no_policy.world().organisations()[0].resources(), 8);
        assert_eq!(no_policy.world().organisations()[1].resources(), 5);
        let mut funded = campaign();
        open(&mut funded, Allocation::draft(50, 40, 10).unwrap());
        votes(&mut funded, [true, true, true]);
        funded
            .apply(player(
                &funded,
                "ratify",
                PlayerCommand::Ratify {
                    meeting_id: "q1".into(),
                    proposal_version: funded.proposal_version().unwrap(),
                },
            ))
            .unwrap();
        let q1 = funded
            .apply(player(&funded, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(q1.delivered[2], 4);
        assert!(q1.report.unwrap().access_bonus_next);
        assert_eq!(funded.world().organisations()[1].resources(), 4);
        let q2 = funded
            .apply(player(&funded, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(q2.delivered[2], 5);
        assert_eq!(
            funded.world().treasury().service_spent(),
            4 + q2.delivered.iter().sum::<u16>()
        );
    }
    #[test]
    fn reused_meeting_id_fails_before_consuming_next_quarter_meeting() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        votes(&mut c, [true, true, true]);
        let v = c.proposal_version().unwrap();
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: v,
            },
        ))
        .unwrap();
        c.apply(player(&c, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        let version = c.world().world_version();
        assert_eq!(
            c.apply(player(
                &c,
                "reuse",
                PlayerCommand::Propose {
                    meeting_id: "q1".into(),
                    allocation: Allocation::draft(30, 20, 10).unwrap(),
                }
            ))
            .err(),
            Some(Rejection::MeetingAlreadyUsed)
        );
        assert_eq!(c.world().world_version(), version);
        c.apply(player(
            &c,
            "fresh",
            PlayerCommand::Propose {
                meeting_id: "q2".into(),
                allocation: Allocation::draft(30, 20, 10).unwrap(),
            },
        ))
        .unwrap();
        assert_eq!(c.phase(), MeetingPhase::FirstPositionsFrozen);
    }
    fn conditional_vote(c: &mut Campaign, role: Ministry, id: &str, due: u16, units: u16) {
        use crate::FundedCondition;
        let mut vote = FinalPosition::new(
            "q1".into(),
            c.proposal_version().unwrap(),
            role,
            Position::Support,
            PreferredAllocation::assess(Allocation::draft(50, 40, 10).unwrap(), 100).unwrap(),
        )
        .unwrap();
        vote.set_public_details(
            vec![
                FundedCondition::new(
                    id.into(),
                    PolicyFamily::Staffing,
                    units,
                    DueQuarter::new(due).unwrap(),
                )
                .unwrap(),
            ],
            vec![],
            "funded scope commitment".into(),
        )
        .unwrap();
        let effort = if role == Ministry::Finance {
            None
        } else {
            Some(
                PrivateEffortDeclaration::new(
                    "q1".into(),
                    role,
                    EffortPermille::new(1000).unwrap(),
                )
                .unwrap(),
            )
        };
        c.apply(Action::Final(vote, effort)).unwrap();
    }
    #[test]
    fn two_funded_promises_cannot_count_one_delivery_twice() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        for role in ROLES {
            first(&mut c, role);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        conditional_vote(&mut c, Ministry::Finance, "finance-staff", 2, 10);
        conditional_vote(&mut c, Ministry::Education, "education-staff", 2, 10);
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 1000);
        let v = c.proposal_version().unwrap();
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: v,
            },
        ))
        .unwrap();
        c.apply(player(&c, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        c.apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.reports()[1].delivered[0], 12);
        let honored = c
            .world()
            .commitments()
            .iter()
            .filter(|x| x.status == CommitmentStatus::Honored)
            .count();
        let broken = c
            .world()
            .commitments()
            .iter()
            .filter(|x| x.status == CommitmentStatus::Broken)
            .count();
        assert_eq!((honored, broken), (1, 1));
        // Lexicographic commitment IDs give Education the first ten units.
        assert_eq!(c.relationship(), [45, 52, 50]);
        c.world().validate().unwrap();
    }
    #[test]
    fn pending_shared_family_obligations_protect_remaining_claims() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        for role in ROLES {
            first(&mut c, role);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        conditional_vote(&mut c, Ministry::Finance, "finance-staff", 3, 10);
        conditional_vote(&mut c, Ministry::Education, "education-staff", 3, 10);
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 1000);
        c.apply(player(
            &c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
        c.apply(player(&c, "advance1", PlayerCommand::AdvanceQuarter))
            .unwrap();
        c.apply(player(&c, "advance2", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert_eq!(c.reports()[1].delivered[0], 12);
        assert!(
            c.world()
                .commitments()
                .iter()
                .all(|x| x.status == CommitmentStatus::Pending)
        );
        // Only 12 credited; 8 separate protected credits remain.
        assert_eq!(
            c.protected_scope_units(Allocation::draft(0, 30, 10).unwrap())
                .err(),
            Some(Rejection::ProtectedCommitment)
        );
        assert_eq!(
            c.protected_scope_units(Allocation::draft(7, 30, 10).unwrap())
                .err(),
            Some(Rejection::ProtectedCommitment)
        );
        let protected = c
            .protected_scope_units(Allocation::draft(8, 30, 10).unwrap())
            .unwrap();
        assert_eq!(protected.values().copied().sum::<u16>(), 8);
        c.apply(player(&c, "advance3", PlayerCommand::AdvanceQuarter))
            .unwrap();
        assert!(
            c.world()
                .commitments()
                .iter()
                .all(|x| x.status == CommitmentStatus::Honored)
        );
        assert_eq!(c.relationship(), [52, 52, 50]);
    }
    #[test]
    fn staff_milestone_due_before_activation_is_rejected_atomically() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        for role in ROLES {
            first(&mut c, role);
        }
        c.apply(Action::FreezeFinal {
            command_id: "freeze".into(),
        })
        .unwrap();
        conditional_vote(&mut c, Ministry::Finance, "impossible-q1", 1, 12);
        final_vote(&mut c, Ministry::Education, Position::Support, 1000);
        final_vote(&mut c, Ministry::Infrastructure, Position::Support, 1000);
        let old = c.world().world_version();
        assert_eq!(
            c.apply(player(
                &c,
                "ratify",
                PlayerCommand::Ratify {
                    meeting_id: "q1".into(),
                    proposal_version: c.proposal_version().unwrap()
                }
            ))
            .err(),
            Some(Rejection::InvalidCondition)
        );
        assert_eq!(c.world().world_version(), old);
        assert_eq!(c.world().treasury().available(), 100);
        assert!(c.world().commitments().is_empty());
    }
    #[test]
    fn signed_coefficients_keep_losing_cells_negative() {
        // High-income cells lose from staffing (benefit shrinks, cost grows): urban-high and
        // rural-high. This is the "popular versus needed" tension the calibration keeps on purpose.
        assert!(cohort_response(2, [1, 0, 0]).unwrap() < 0);
        assert!(cohort_response(5, [1, 0, 0]).unwrap() < 0);
        // Rural-low used to lose from staffing under the pre-0003 numbers (480 against 520); it
        // gains now. Access stays the strongest single channel for most cells.
        assert!(cohort_response(3, [1, 0, 0]).unwrap() > 0);
        assert!(cohort_response(0, [0, 0, 1]).unwrap() > 0);
    }
    #[test]
    fn one_quarter_can_move_a_cell_by_four_points_but_never_more() {
        // The clip is the loudest a single quarter can be.
        let loudest = cohort_response_scaled(0, [0, 0, 100], 150).unwrap();
        assert_eq!(loudest, RESPONSE_CLIP_PPM);
        let harshest = cohort_response_scaled(2, [100, 0, 0], 150).unwrap();
        assert_eq!(harshest, -RESPONSE_CLIP_PPM);
    }
    fn voters() -> CohortWeights {
        CohortWeights::new([260000, 320000, 220000, 70000, 80000, 50000]).unwrap()
    }
    #[test]
    fn overall_support_alone_decides_the_election() {
        let w = voters();
        // Nobody moved: 50% overall, no group backs the player. Loses.
        let flat = evaluate_election([500_000; 6], &w).unwrap();
        assert_eq!(flat.weighted_support_ppm, 500_000);
        assert_eq!(flat.blocs_held_ppm, 0);
        assert!(!flat.is_win());
        // Everyone just above the bar wins.
        let broad = evaluate_election([551_000; 6], &w).unwrap();
        assert_eq!(broad.weighted_support_ppm, 551_000);
        assert_eq!(broad.blocs_held_ppm, 1_000_000);
        assert!(broad.is_win());
        // Broad but lukewarm: every group backs the player, yet 52% overall is below the bar.
        let lukewarm = evaluate_election([520_000; 6], &w).unwrap();
        assert_eq!(lukewarm.blocs_held_ppm, 1_000_000);
        assert!(!lukewarm.is_win());
        // Warm but narrow: urban middle and high at 70% lift overall support to 56.2%, and those two groups are
        // only 54% of the voters. That used to lose on a second, coalition bar; it now wins, because a player who
        // changes course and wins different groups is judged by the overall result (decision 0009).
        let narrow =
            evaluate_election([400_000, 700_000, 700_000, 400_000, 400_000, 400_000], &w).unwrap();
        assert_eq!(narrow.weighted_support_ppm, 562_000);
        assert_eq!(narrow.blocs_held_ppm, 540_000);
        assert!(narrow.is_win());
        // Which groups carry the result is irrelevant: any spread with a high enough total wins.
        let other_groups =
            evaluate_election([700_000, 700_000, 400_000, 400_000, 400_000, 400_000], &w).unwrap();
        assert_eq!(other_groups.weighted_support_ppm, 574_000);
        assert_eq!(other_groups.blocs_held_ppm, 580_000);
        assert!(other_groups.is_win());
    }
    #[test]
    fn a_group_is_held_only_when_strictly_above_fifty_percent() {
        let w = voters();
        let at =
            evaluate_election([500_000, 600_000, 600_000, 600_000, 600_000, 600_000], &w).unwrap();
        let above =
            evaluate_election([500_001, 600_000, 600_000, 600_000, 600_000, 600_000], &w).unwrap();
        // Urban low holds 260,000 of the voters; exactly 50% does not count, one ppm more does.
        assert_eq!(at.blocs_held_ppm, 740_000);
        assert_eq!(above.blocs_held_ppm, 1_000_000);
    }
    #[test]
    fn balanced_term_wins_with_the_numbers_the_independent_replica_predicts() {
        // Expected values come from tools/support-model (a separate implementation that matches the
        // engine to the ppm), not from this code, so this is a cross-check and not a tautology.
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
        for i in 0..6 {
            c.apply(player(
                &c,
                &format!("advance{i}"),
                PlayerCommand::AdvanceQuarter,
            ))
            .unwrap();
        }
        let election = c.election_projection().unwrap();
        assert_eq!(
            election.group_support_ppm,
            [571_800, 602_000, 472_200, 579_240, 615_600, 491_960]
        );
        assert_eq!(election.weighted_support_ppm, 559_584);
        assert_eq!(election.blocs_held_ppm, 730_000);
        assert!(election.is_win());
        // High-income urban and rural groups are left behind even in a win: the intended tension.
        assert!(election.group_support_ppm[2] < 500_000 && election.group_support_ppm[5] < 500_000);
    }
    fn ratify_balanced_q1(c: &mut Campaign) {
        open(c, Allocation::draft(50, 40, 10).unwrap());
        votes(c, [true, true, true]);
        c.apply(player(
            c,
            "ratify",
            PlayerCommand::Ratify {
                meeting_id: "q1".into(),
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
    }
    fn advance(c: &mut Campaign, id: &str) -> Result<ReducerReceipt, Rejection> {
        c.apply(player(c, id, PlayerCommand::AdvanceQuarter))
    }
    /// Keep the programme and let time pass: no meetings, just the quarters turning over.
    fn coast(c: &mut Campaign, quarters: usize, tag: &str) {
        for i in 0..quarters {
            advance(c, &format!("{tag}{i}")).unwrap();
        }
    }
    #[test]
    fn a_won_election_waits_for_the_player_and_then_grants_a_fresh_allocation() {
        let mut c = campaign();
        ratify_balanced_q1(&mut c);
        coast(&mut c, 6, "t1-");
        // Term one is over and was won: one election on record, and the game is waiting for the player.
        assert_eq!(c.phase(), MeetingPhase::ElectionNight);
        assert_eq!(c.world().quarter().value(), 6);
        assert_eq!(c.elections().len(), 1);
        assert!(c.elections()[0].won && c.elections()[0].term == 1);
        assert_eq!(c.world().treasury().granted(), 100);
        // Nothing but BeginTerm moves it on: time, deferral and audits are all refused.
        assert_eq!(advance(&mut c, "late").err(), Some(Rejection::InvalidPhase));
        assert_eq!(
            c.apply(player(&c, "defer", PlayerCommand::Defer)).err(),
            Some(Rejection::InvalidPhase)
        );
        assert_eq!(
            c.apply(player(
                &c,
                "audit",
                PlayerCommand::OrderAudit {
                    family: PolicyFamily::Staffing,
                    inspected_quarter: Quarter::new(3).unwrap(),
                }
            ))
            .err(),
            Some(Rejection::InvalidPhase)
        );
        let before_service = c.world().treasury().service_spent();
        let support_before = *c.world().support();
        let receipt = c
            .apply(player(&c, "begin", PlayerCommand::BeginTerm))
            .unwrap();
        // The second term has begun: new quarter, new allocation, and one ledger entry that says so.
        assert_eq!(c.world().quarter().value(), 7);
        assert_eq!(c.world().quarter().term(), 2);
        assert_eq!(c.world().quarter().in_term(), 1);
        assert_eq!(c.phase(), MeetingPhase::QuarterOpen);
        assert_eq!(receipt.ledger.len(), 1);
        let t = c.world().treasury();
        assert_eq!(
            t.available(),
            100,
            "everything was delivered, so the allocation is all there is"
        );
        assert_eq!(t.granted(), 200);
        // What was delivered stays delivered, and the groups' mood carries over untouched.
        assert_eq!(t.service_spent(), before_service);
        assert_eq!(*c.world().support(), support_before);
        c.world().validate().unwrap();
        // BeginTerm is not a loophole: it works once per election.
        assert_eq!(
            c.apply(player(&c, "again", PlayerCommand::BeginTerm)).err(),
            Some(Rejection::InvalidPhase)
        );
    }
    #[test]
    fn unspent_credits_carry_into_the_next_term_and_are_added_to() {
        let mut c = campaign();
        // A failed vote spends nothing; coasting six quarters then leaves all 100 unspent.
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        votes(&mut c, [false, true, false]);
        c.apply(player(&c, "defer", PlayerCommand::Defer)).unwrap();
        coast(&mut c, 6, "t1-");
        // Nothing delivered, so support never moved: that election is lost and the game is over.
        assert_eq!(c.phase(), MeetingPhase::QuarterClosed);
        assert!(!c.elections()[0].won);
        assert_eq!(c.world().treasury().available(), 100);
        assert_eq!(
            c.apply(player(&c, "begin", PlayerCommand::BeginTerm)).err(),
            Some(Rejection::InvalidPhase),
            "a loss cannot be undone by starting a term"
        );
    }
    #[test]
    fn terms_go_on_for_as_long_as_the_player_keeps_winning() {
        let mut c = campaign();
        ratify_balanced_q1(&mut c);
        coast(&mut c, 6, "t1-");
        c.apply(player(&c, "begin-2", PlayerCommand::BeginTerm))
            .unwrap();
        coast(&mut c, 6, "t2-");
        // Six more quarters with the same programme: the second term ends with its own election, won again.
        assert_eq!(c.world().quarter().value(), 12);
        assert_eq!(c.elections().len(), 2);
        assert_eq!(c.elections()[1].term, 2);
        assert!(c.elections()[1].won);
        // A second win does not end the game: nothing is special about the second term.
        assert_eq!(c.phase(), MeetingPhase::ElectionNight);
        assert_eq!(
            advance(&mut c, "too-soon").err(),
            Some(Rejection::InvalidPhase)
        );
        c.apply(player(&c, "begin-3", PlayerCommand::BeginTerm))
            .unwrap();
        assert_eq!(c.world().quarter().value(), 13);
        assert_eq!(c.world().quarter().term(), 3);
        assert_eq!(c.phase(), MeetingPhase::QuarterOpen);
        // Credits are conserved however long it goes on: three grants, every credit in exactly one account.
        c.world().validate().unwrap();
        assert_eq!(c.world().treasury().granted(), 300);
    }
    #[test]
    fn only_the_technical_ceiling_ends_a_won_game() {
        let mut c = campaign();
        ratify_balanced_q1(&mut c);
        for term in 1..=MAX_TERMS {
            coast(&mut c, 6, &format!("t{term}-"));
            assert_eq!(c.elections().len(), usize::from(term));
            assert!(c.elections().last().unwrap().won, "term {term} is won");
            if term < MAX_TERMS {
                assert_eq!(c.phase(), MeetingPhase::ElectionNight, "term {term}");
                c.apply(player(
                    &c,
                    &format!("begin-{term}"),
                    PlayerCommand::BeginTerm,
                ))
                .unwrap();
            }
        }
        // The longest tenure the engine records: there is nowhere further to go.
        assert_eq!(c.world().quarter().value(), MAX_QUARTERS);
        assert_eq!(c.phase(), MeetingPhase::QuarterClosed);
        assert_eq!(
            advance(&mut c, "past-the-end").err(),
            Some(Rejection::InvalidPhase)
        );
        assert_eq!(
            c.apply(player(&c, "one-more", PlayerCommand::BeginTerm))
                .err(),
            Some(Rejection::InvalidPhase)
        );
        c.world().validate().unwrap();
        assert_eq!(
            c.world().treasury().granted(),
            PILOT_CREDITS * u16::from(MAX_TERMS)
        );
    }
    // ---- the news calendar (decision 0010) ----
    fn with_calendar(seed: u64) -> Campaign {
        Campaign::with_rules(
            "case",
            "england-school-delivery-v1",
            voters(),
            Rules {
                news_seed: Some(seed),
                ..Rules::default()
            },
        )
        .unwrap()
    }
    /// The first seed whose term `term` has an announced (not surprise) headline of this tone.
    fn seed_with(term: u8, tone: Tone, surprise: bool) -> (u64, Scheduled) {
        (0..10_000_u64)
            .find_map(|seed| {
                news::schedule(seed, term)
                    .into_iter()
                    .find(|s| s.tone == tone && s.surprise == surprise)
                    .map(|s| (seed, s))
            })
            .unwrap()
    }
    /// Plays the game forward by ending quarters until `quarter` has been reached (not ended).
    fn coast_to(c: &mut Campaign, quarter: u8, tag: &str) {
        let mut n = 0;
        while c.world().quarter().value() < quarter {
            advance(c, &format!("{tag}{n}")).unwrap();
            n += 1;
        }
    }
    fn respond_cmd(c: &mut Campaign, id: &str, q: Quarter) -> Result<ReducerReceipt, Rejection> {
        c.apply(player(
            c,
            id,
            PlayerCommand::Respond {
                headline_quarter: q,
            },
        ))
    }
    #[test]
    fn without_a_seed_no_headline_ever_lands() {
        let mut c = campaign();
        ratify_balanced_q1(&mut c);
        coast(&mut c, 6, "t-");
        assert!(c.landed_headlines().is_empty());
        assert!(c.upcoming_headlines().is_empty());
        let (_, s) = seed_with(2, Tone::Bad, false);
        assert_eq!(
            respond_cmd(&mut c, "no-calendar", s.quarter).err(),
            Some(Rejection::InvalidCondition)
        );
    }
    #[test]
    fn the_first_term_brings_good_news_only_and_it_helps() {
        let mut plain = campaign();
        ratify_balanced_q1(&mut plain);
        coast(&mut plain, 6, "p-");
        for seed in [1_u64, 2, 3, 99, 12345] {
            let mut c = with_calendar(seed);
            ratify_balanced_q1(&mut c);
            coast(&mut c, 6, "t-");
            assert_eq!(c.landed_headlines().len(), 2, "seed {seed}");
            assert!(
                c.landed_headlines()
                    .iter()
                    .all(|l| l.scheduled.tone == Tone::Good && l.shock.iter().all(|x| *x >= 0)),
                "term 1 has no bad news (seed {seed})"
            );
            // Good news is a gift on top of what the work earned, so the first election is easier, never harder.
            let with = c.elections()[0].projection.weighted_support_ppm;
            let without = plain.elections()[0].projection.weighted_support_ppm;
            assert!(with > without, "seed {seed}: {with} vs {without}");
            assert!(c.elections()[0].won);
        }
    }
    #[test]
    fn an_announced_headline_is_listed_a_quarter_ahead_and_a_surprise_never_is() {
        let (seed, bad) = seed_with(2, Tone::Bad, false);
        let mut c = with_calendar(seed);
        ratify_balanced_q1(&mut c);
        coast(&mut c, 6, "t1-");
        c.apply(player(&c, "begin", PlayerCommand::BeginTerm))
            .unwrap();
        // Walk up to the quarter before it lands: the headline is listed, with its price, and not before.
        let listed = |c: &Campaign| {
            c.upcoming_headlines()
                .into_iter()
                .any(|u| u.scheduled.quarter == bad.quarter)
        };
        assert!(
            bad.quarter.value() >= 8,
            "headlines never fall in the first quarter of a term"
        );
        if bad.quarter.value() > 8 {
            coast_to(&mut c, bad.quarter.value() - 2, "early-");
            assert!(!listed(&c), "two quarters ahead is too early");
        }
        coast_to(&mut c, bad.quarter.value() - 1, "ahead-");
        assert!(listed(&c), "announced a quarter ahead");
        let up = c
            .upcoming_headlines()
            .into_iter()
            .find(|u| u.scheduled.quarter == bad.quarter)
            .unwrap();
        assert!(up.relief_cost.is_some_and(|cost| cost >= 1));
        assert_eq!(up.answered, None);
        // Surprises are never listed, at any point.
        let (sseed, surprise) = seed_with(2, Tone::Bad, true);
        let mut d = with_calendar(sseed);
        ratify_balanced_q1(&mut d);
        coast(&mut d, 6, "s1-");
        d.apply(player(&d, "begin", PlayerCommand::BeginTerm))
            .unwrap();
        for step in 0..6 {
            assert!(
                d.upcoming_headlines()
                    .iter()
                    .all(|u| u.scheduled.quarter != surprise.quarter),
                "a surprise was announced at step {step}"
            );
            if d.world().quarter() < surprise.quarter {
                advance(&mut d, &format!("s2-{step}")).unwrap();
            }
        }
        coast_to(&mut d, surprise.quarter.value(), "s3-");
        assert_eq!(
            respond_cmd(&mut d, "too-late", surprise.quarter).err(),
            Some(Rejection::InvalidCondition),
            "a surprise cannot be answered"
        );
    }
    #[test]
    fn a_response_softens_the_hit_for_credits_and_the_money_still_adds_up() {
        let (seed, bad) = seed_with(2, Tone::Bad, false);
        let play = |respond: bool| {
            let mut c = with_calendar(seed);
            ratify_balanced_q1(&mut c);
            coast(&mut c, 6, "t1-");
            c.apply(player(&c, "begin", PlayerCommand::BeginTerm))
                .unwrap();
            coast_to(&mut c, bad.quarter.value() - 1, "to-");
            let before = c.world().treasury();
            let mut paid = 0;
            if respond {
                let quote = c
                    .upcoming_headlines()
                    .into_iter()
                    .find(|u| u.scheduled.quarter == bad.quarter)
                    .unwrap()
                    .relief_cost
                    .unwrap();
                let receipt = respond_cmd(&mut c, "answer", bad.quarter).unwrap();
                assert_eq!(receipt.ledger.len(), 1);
                let t = c.world().treasury();
                paid = quote;
                assert_eq!(t.relief_spent(), quote, "what was quoted is what was paid");
                assert_eq!(t.available(), before.available() - quote);
                assert_eq!(t.granted(), before.granted());
                t.validate().unwrap();
                // Once is enough: the same headline cannot be answered twice.
                assert_eq!(
                    respond_cmd(&mut c, "twice", bad.quarter).err(),
                    Some(Rejection::InvalidCondition)
                );
                assert!(
                    c.upcoming_headlines()
                        .iter()
                        .any(|u| u.answered == Some(quote))
                );
            }
            coast_to(&mut c, bad.quarter.value() + 1, "past-");
            let landed = *c
                .landed_headlines()
                .iter()
                .find(|l| l.scheduled.quarter == bad.quarter)
                .unwrap();
            c.world().validate().unwrap();
            (landed, paid)
        };
        let (unanswered, _) = play(false);
        let (answered, paid) = play(true);
        assert!(!unanswered.responded && answered.responded);
        assert!(paid >= 1);
        // The same headline took 60% less from every group that lost something, and gave nothing back to the rest.
        let lost = |l: &Landed| l.shock.iter().filter(|x| **x < 0).count();
        assert!(lost(&unanswered) > 0 && lost(&answered) == lost(&unanswered));
        for (a, u) in answered.shock.iter().zip(unanswered.shock) {
            assert!(
                *a >= u && *a <= 0,
                "a response never makes it worse: {a} vs {u}"
            );
            assert_eq!(*a, u * 40 / 100);
        }
    }
    #[test]
    fn a_response_has_to_be_possible_to_be_paid() {
        let (seed, bad) = seed_with(2, Tone::Bad, false);
        let mut c = with_calendar(seed);
        ratify_balanced_q1(&mut c);
        coast(&mut c, 6, "t1-");
        c.apply(player(&c, "begin", PlayerCommand::BeginTerm))
            .unwrap();
        // Too early: it has not been announced yet.
        if bad.quarter.value() > 8 {
            assert_eq!(
                respond_cmd(&mut c, "early", bad.quarter).err(),
                Some(Rejection::InvalidCondition)
            );
        }
        coast_to(&mut c, bad.quarter.value() - 1, "to-");
        // Good news, headlines that do not exist, and ones already past cannot be answered.
        let good = news::schedule(seed, 2)
            .into_iter()
            .find(|s| s.tone == Tone::Good);
        if let Some(g) = good.filter(|g| g.quarter.value() <= c.world().quarter().value() + 1) {
            assert_eq!(
                respond_cmd(&mut c, "good", g.quarter).err(),
                Some(Rejection::InvalidCondition)
            );
        }
        let nowhere = Quarter::new(c.world().quarter().value() + 1).unwrap();
        if nowhere != bad.quarter {
            assert!(respond_cmd(&mut c, "nowhere", nowhere).is_err() || nowhere.in_term() == 1);
        }
        // Not enough credits: spend them all first (a fresh plan commits everything), then the answer is refused.
        let mut parts = c.world.private_checkpoint_parts();
        parts.treasury = Treasury::from_parts_all(200, 0, 0, 0, 0, 200, 0, 0).unwrap();
        c.world = WorldSnapshot::from_parts(parts).unwrap();
        assert!(respond_cmd(&mut c, "broke", bad.quarter).is_err());
        assert!(
            c.responses().is_empty(),
            "a refused response leaves no trace"
        );
    }
    #[test]
    fn a_disruption_delays_work_past_the_election_and_is_quoted_before_it_lands() {
        // A flood that stops repairs for two quarters, announced for quarter 11.
        let (seed, flood) = (0..20_000_u64)
            .find_map(|seed| {
                news::schedule(seed, 2)
                    .into_iter()
                    .find(|s| {
                        s.tone == Tone::Bad
                            && !s.surprise
                            && s.quarter.value() == 11
                            && s.headline().disrupts.is_some_and(|d| {
                                d.family == PolicyFamily::Repairs
                                    && d.keep_pct == 0
                                    && d.quarters == 2
                            })
                    })
                    .map(|s| (seed, s))
            })
            .unwrap();
        let play = |answer: bool| {
            let mut c = with_calendar(seed);
            ratify_balanced_q1(&mut c);
            coast(&mut c, 6, "t1-");
            c.apply(player(&c, "begin", PlayerCommand::BeginTerm))
                .unwrap();
            // A plan that keeps 20 credits back, so there is something to answer with.
            open_meeting_with(&mut c, 7, Allocation::draft(40, 30, 10).unwrap());
            coast_to(&mut c, 10, "to-");
            let quote = c
                .upcoming_headlines()
                .into_iter()
                .find(|u| u.scheduled.quarter == flood.quarter)
                .expect("announced a quarter ahead");
            let delivery = quote
                .delivery
                .expect("a disruption says what it does to delivery");
            assert_eq!(delivery.family, PolicyFamily::Repairs);
            assert_eq!((delivery.keep_pct, delivery.quarters), (0, 2));
            // The plan promised 30 repair units this term, ten a quarter from quarter 9. By quarter 10 twenty are left;
            // stopped in quarters 11 and 12, ten of them would land after the election.
            assert_eq!(delivery.delayed_units, 10);
            // The quote includes what that costs in support, on top of the mood the story takes.
            assert!(
                quote
                    .expected
                    .iter()
                    .zip(delivery.effect)
                    .all(|(total, d)| *total <= d.max(0) || *total <= 0)
            );
            let cost = quote.relief_cost.expect("answerable");
            if answer {
                respond_cmd(&mut c, "answer", flood.quarter).unwrap();
                assert_eq!(c.world().treasury().relief_spent(), cost);
            }
            coast(&mut c, 3, "end-");
            c.world().validate().unwrap();
            (c.world().treasury().service_spent(), cost)
        };
        let (plain, cost) = play(false);
        let (answered, paid) = play(true);
        assert_eq!(cost, paid, "what was quoted is what was paid");
        // Term 1 delivered all 100 credits. In term 2 nothing is lost to the stoppage, but 10 units of repairs slip past
        // the election; answering turns a stoppage into a slowdown (60% less cut), and all of them are delivered after all.
        assert_eq!(plain, 100 + 40 + 20 + 10);
        assert_eq!(answered, plain + 10);
        assert!(
            (1..=20).contains(&cost),
            "an answer the player could afford: {cost}"
        );
    }
    #[test]
    fn a_disruption_to_work_the_player_never_promised_costs_them_nothing_and_cannot_be_answered() {
        // Repairs are stopped, but this player funded none: there is nothing to delay and nothing to protect except mood.
        let (seed, s) = (0..20_000_u64)
            .find_map(|seed| {
                news::schedule(seed, 2)
                    .into_iter()
                    .find(|s| {
                        s.tone == Tone::Bad
                            && !s.surprise
                            && s.headline().disrupts.is_some_and(|d| {
                                d.family == PolicyFamily::Access && d.keep_pct == 25
                            })
                            && s.quarter.value() >= 9
                    })
                    .map(|s| (seed, s))
            })
            .unwrap();
        let mut c = with_calendar(seed);
        ratify_balanced_q1(&mut c);
        coast(&mut c, 6, "t1-");
        c.apply(player(&c, "begin", PlayerCommand::BeginTerm))
            .unwrap();
        coast_to(&mut c, s.quarter.value() - 1, "to-");
        let quote = c
            .upcoming_headlines()
            .into_iter()
            .find(|u| u.scheduled.quarter == s.quarter)
            .unwrap();
        let delivery = quote.delivery.unwrap();
        assert_eq!(delivery.family, PolicyFamily::Access);
        assert_eq!(delivery.delayed_units, 0, "no access work is promised");
        assert!(delivery.effect.iter().all(|e| *e == 0));
    }
    #[test]
    fn a_player_who_does_nothing_is_voted_out_in_the_end_and_one_who_keeps_delivering_is_not() {
        // Across several calendars, doing nothing after the first win loses an election within a handful of terms,
        // while repeating the Balanced plan every term stays ahead at least that long (decision 0010).
        let mut idle_lost_by = vec![];
        let mut busy_lost_by = vec![];
        for seed in 0..12_u64 {
            for busy in [false, true] {
                let mut c = with_calendar(seed);
                ratify_balanced_q1(&mut c);
                let mut term = 1;
                loop {
                    coast(&mut c, 6, &format!("s{seed}b{busy}t{term}-"));
                    if !c.elections().last().unwrap().won || term == 5 {
                        break;
                    }
                    c.apply(player(
                        &c,
                        &format!("begin{seed}{busy}{term}"),
                        PlayerCommand::BeginTerm,
                    ))
                    .unwrap();
                    term += 1;
                    if busy {
                        let q = c.world().quarter().value();
                        open_meeting_and_ratify(&mut c, q);
                    }
                }
                let verdicts: Vec<bool> = c.elections().iter().map(|e| e.won).collect();
                (if busy {
                    &mut busy_lost_by
                } else {
                    &mut idle_lost_by
                })
                .push(verdicts.iter().position(|w| !*w).map_or(99, |i| i + 1));
            }
        }
        let median = |v: &Vec<usize>| {
            let mut s = v.clone();
            s.sort_unstable();
            s[s.len() / 2]
        };
        assert!(
            idle_lost_by.iter().filter(|t| **t <= 5).count() >= 8,
            "most idle games end within five terms: {idle_lost_by:?}"
        );
        assert!(
            median(&busy_lost_by) >= median(&idle_lost_by),
            "delivering is never worse than doing nothing: busy {busy_lost_by:?} vs idle {idle_lost_by:?}"
        );
    }
    /// Proposes the Balanced plan in the quarter the campaign is now in, with every minister in favour, and ratifies it.
    fn open_meeting_and_ratify(c: &mut Campaign, quarter: u8) {
        open_meeting_with(c, quarter, Allocation::draft(50, 40, 10).unwrap());
    }
    /// The same, for any plan.
    fn open_meeting_with(c: &mut Campaign, quarter: u8, draft: Allocation) {
        let meeting = format!("q{quarter}");
        let assess = PreferredAllocation::assess(draft, 100).unwrap();
        c.apply(player(
            c,
            &format!("m{quarter}-propose"),
            PlayerCommand::Propose {
                meeting_id: meeting.clone(),
                allocation: draft,
            },
        ))
        .unwrap();
        for m in ROLES {
            let pos = FirstPosition::new(
                meeting.clone(),
                c.proposal_version().unwrap(),
                m,
                Position::Support,
                assess,
                CounterproposalOrigin::Menu("balanced".into()),
            )
            .unwrap();
            c.apply(Action::First(pos)).unwrap();
        }
        c.apply(Action::FreezeFinal {
            command_id: format!("m{quarter}-freeze"),
        })
        .unwrap();
        for m in ROLES {
            let pos = FinalPosition::new(
                meeting.clone(),
                c.proposal_version().unwrap(),
                m,
                Position::Support,
                assess,
            )
            .unwrap();
            let private = (m != Ministry::Finance).then(|| {
                PrivateEffortDeclaration::new(
                    meeting.clone(),
                    m,
                    EffortPermille::new(1000).unwrap(),
                )
                .unwrap()
            });
            c.apply(Action::Final(pos, private)).unwrap();
        }
        c.apply(player(
            c,
            &format!("m{quarter}-ratify"),
            PlayerCommand::Ratify {
                meeting_id: meeting,
                proposal_version: c.proposal_version().unwrap(),
            },
        ))
        .unwrap();
    }
    #[test]
    fn the_free_audit_comes_back_each_term() {
        let mut c = campaign();
        ratify_balanced_q1(&mut c);
        let order = |c: &mut Campaign, id: &str| {
            c.apply(player(
                c,
                id,
                PlayerCommand::OrderAudit {
                    family: PolicyFamily::Staffing,
                    inspected_quarter: Quarter::new(2).unwrap(),
                },
            ))
        };
        let at = |c: &Campaign, quarter: u8| assert_eq!(c.world().quarter().value(), quarter);
        // Term one. In its second quarter there is nothing to audit yet; from its third the first audit is free.
        coast(&mut c, 1, "t1-a-");
        at(&c, 2);
        assert_eq!(
            order(&mut c, "t1-early").err(),
            Some(Rejection::InvalidPhase)
        );
        coast(&mut c, 1, "t1-b-");
        at(&c, 3);
        order(&mut c, "t1-audit").unwrap();
        assert_eq!(c.audits.last().unwrap().cost, 0);
        // Finish the term (Q3 to Q6, then the sixth advance ends it) and begin the next.
        coast(&mut c, 4, "t1-rest-");
        assert_eq!(c.phase(), MeetingPhase::ElectionNight);
        c.apply(player(&c, "begin", PlayerCommand::BeginTerm))
            .unwrap();
        at(&c, 7);
        // Term two. The allowance counts from the term's own third quarter (Q9), not the game's.
        coast(&mut c, 1, "t2-a-");
        at(&c, 8);
        assert_eq!(
            order(&mut c, "t2-early").err(),
            Some(Rejection::InvalidPhase)
        );
        coast(&mut c, 1, "t2-b-");
        at(&c, 9);
        assert_eq!(c.world().quarter().in_term(), 3);
        order(&mut c, "t2-audit").unwrap();
        assert_eq!(
            c.audits.last().unwrap().cost,
            0,
            "free again in the second term"
        );
        // A second audit in the same term costs 2, paid from the new allocation.
        let before = c.world().treasury().available();
        order(&mut c, "t2-audit-2").unwrap();
        assert_eq!(c.audits.last().unwrap().cost, 2);
        assert_eq!(c.world().treasury().available(), before - 2);
        c.world().validate().unwrap();
    }
    #[test]
    fn projection_reads_state_without_changing_it() {
        let mut c = campaign();
        open(&mut c, Allocation::draft(50, 40, 10).unwrap());
        let before = c.world().world_version();
        let first = c.election_projection().unwrap();
        assert_eq!(first, c.election_projection().unwrap());
        assert_eq!(c.world().world_version(), before);
        assert!(!first.is_win());
    }
}
