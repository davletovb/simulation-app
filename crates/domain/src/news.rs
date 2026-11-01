//! The news calendar (decision 0010): invented headlines that land on a term's quarters and move public support.
//!
//! Everything here is a pure function of a seed and a term, in integer arithmetic only, so a saved game replays to the
//! same headlines every time and a what-if branch sees the headlines of the game it was split from. The numbers are the
//! ones tuned in `tools/support-model/calendar_sim.py`; `data/england-school-pilot-v1/authored.json` repeats them (with the
//! words) and a test keeps the two equal.
//!
//! Rules in one paragraph. Term 1 has two headlines, both good: no bad news yet. Every later term has four, 30% of them
//! good. A third of headlines are surprises that nobody is told about; the rest are announced a quarter ahead and can be
//! answered with credits. Bad news takes from each group its exposure (a share of the worst case) times a small base plus a
//! share of the group's lead over 50%, and both the base and the share grow with the term, so a big lead is never a safe one.
//! Some bad headlines also stop one kind of work (decision 0011) or send inspectors to count the promises that have not been
//! kept yet (decision 0012); some good ones bring a few credits (a windfall, decision 0012).

use crate::{
    PolicyFamily, Quarter, TERM_QUARTERS, reducer::ELECTION_HELD_ABOVE_PPM, reducer::SHOCK_CAP_PPM,
};

/// Headlines in term 1: good news only, so the first election is a fair test of the opening plan.
pub const QUIET_TERM_HEADLINES: u8 = 2;
/// Headlines in every later term.
pub const HEADLINES_PER_TERM: u8 = 4;
/// Of every 100 headlines after term 1, this many are good news.
pub const GOOD_PERCENT: u64 = 30;
/// One headline in this many is a surprise: not announced, and nothing can be done about it.
pub const SURPRISE_ONE_IN: u64 = 3;
/// What bad news takes before the lead is counted, in ppm of a group's support, at full exposure (term 2).
pub const BASE_PPM: u32 = 5_000;
/// The base grows by this many percent of itself each term after the second...
pub const BASE_STEP_PCT: u32 = 50;
/// ...up to this many percent of the starting base.
pub const BASE_CAP_PCT: u32 = 400;
/// The share (percent) of a group's lead over 50% that bad news takes, in term 2.
pub const SHARE_PCT: u32 = 40;
/// The share grows by this many points each term after the second...
pub const SHARE_STEP_PCT: u32 = 8;
/// ...up to this many percent.
pub const SHARE_CAP_PCT: u32 = 90;
/// A response costs this many credits for each point of overall support the headline would take (rounded up, at least 1).
pub const RELIEF_CREDITS_PER_POINT: u32 = 10;
/// A response takes this many percent off every group's loss.
pub const RELIEF_CUT_PCT: i32 = 60;
/// Inspectors take this many percent of what the promised-and-undelivered work would have earned a group, in term 2...
pub const SCRUTINY_PCT: u32 = 50;
/// ...growing by this many points each term after the second...
pub const SCRUTINY_STEP_PCT: u32 = 15;
/// ...up to this many percent. More than 100 because a broken promise is punished harder than it would have been rewarded.
pub const SCRUTINY_CAP_PCT: u32 = 200;
/// One bad headline in this many is a trade-off story instead: it helps some groups while it hurts others (decision 0014).
pub const MIXED_ONE_IN: u64 = 5;
/// Trade-off stories begin in this term: term 1 has good news only and term 2 adds bad news, so term 3 adds stories that cut both
/// ways, and the calendar of the first two terms is exactly what it was before.
pub const MIXED_FROM_TERM: u8 = 3;
/// The most real headlines that can replace invented ones in one term (decision 0013).
pub const MAX_REAL_PER_TERM: usize = 2;
/// The most real items one import can carry, counting those that are only shown and change nothing.
pub const MAX_REAL_ITEMS: usize = 4;
/// The most credits any one windfall brings.
pub const MAX_WINDFALL_CREDITS: u16 = 15;
/// The most credits windfalls can bring in a whole game: every headline of every term, if they were all the largest windfall.
pub const MAX_WINDFALL_TOTAL: u16 = MAX_WINDFALL_CREDITS
    * (QUIET_TERM_HEADLINES as u16 + HEADLINES_PER_TERM as u16 * (crate::MAX_TERMS as u16 - 1));

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    Bad,
    Good,
}

/// A headline that also stops work: one kind of work delivers only `keep_pct` percent of its usual capacity, for `quarters`
/// quarters counted from the one the headline lands in (decision 0011). It slows delivery; nothing is lost, and what could not
/// be delivered waits, but work that slips past the election cannot help win it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Disruption {
    pub family: PolicyFamily,
    pub keep_pct: u32,
    pub quarters: u8,
}

/// A headline that sends inspectors to count the promises that have not been kept yet (decision 0012). When it lands, the work
/// the player has promised in the named kinds of work and not yet delivered is counted, and each group loses a share of what
/// that work would have earned it (`scrutiny_pct`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scrutiny {
    /// Which kinds of work are counted: staffing, repairs, access.
    pub families: [bool; 3],
}

/// One kind of headline. For bad news `effect` is each group's exposure, in percent of the worst case; for good news it
/// is each group's gain in ppm. Groups: urban low, middle, high income, then rural low, middle, high. A bad headline may also
/// disrupt one kind of work or send inspectors; a good one may bring credits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Headline {
    pub id: &'static str,
    pub effect: [u32; 6],
    pub disrupts: Option<Disruption>,
    pub scrutiny: Option<Scrutiny>,
    /// Credits that arrive when this good news lands (a windfall); zero for everything else.
    pub grant: u16,
    /// A bad headline that is a trade-off also gives these groups a flat gain, in ppm (decision 0014); none of the groups it
    /// hurts, and zero for everything else.
    pub winners: [u32; 6],
}

const fn story(id: &'static str, effect: [u32; 6]) -> Headline {
    Headline {
        id,
        effect,
        disrupts: None,
        scrutiny: None,
        grant: 0,
        winners: [0; 6],
    }
}
const fn audit(id: &'static str, effect: [u32; 6], families: [bool; 3]) -> Headline {
    Headline {
        id,
        effect,
        disrupts: None,
        scrutiny: Some(Scrutiny { families }),
        grant: 0,
        winners: [0; 6],
    }
}
/// A trade-off story: `effect` is who it hurts (exposure, as for any bad news) and `winners` who it helps (a flat gain, in ppm).
const fn tradeoff(id: &'static str, effect: [u32; 6], winners: [u32; 6]) -> Headline {
    Headline {
        id,
        effect,
        disrupts: None,
        scrutiny: None,
        grant: 0,
        winners,
    }
}
const fn windfall(id: &'static str, credits: u16) -> Headline {
    Headline {
        id,
        effect: [0; 6],
        disrupts: None,
        scrutiny: None,
        grant: credits,
        winners: [0; 6],
    }
}
const fn stoppage(
    id: &'static str,
    effect: [u32; 6],
    family: PolicyFamily,
    keep_pct: u32,
    quarters: u8,
) -> Headline {
    Headline {
        id,
        effect,
        disrupts: Some(Disruption {
            family,
            keep_pct,
            quarters,
        }),
        scrutiny: None,
        grant: 0,
        winners: [0; 6],
    }
}

pub const BAD: [Headline; 15] = [
    stoppage(
        "teacher_shortage",
        [50, 50, 50, 25, 25, 25],
        PolicyFamily::Staffing,
        50,
        1,
    ),
    stoppage(
        "roof_scare",
        [25, 25, 25, 75, 75, 75],
        PolicyFamily::Repairs,
        50,
        1,
    ),
    story("funding_row", [50, 50, 50, 50, 50, 50]),
    story("rural_bus_cuts", [0, 0, 0, 100, 100, 100]),
    story("cost_of_living", [75, 40, 0, 75, 40, 0]),
    story("exam_results_row", [0, 60, 75, 0, 40, 50]),
    story("agency_scandal", [60, 60, 30, 30, 30, 15]),
    stoppage(
        "broadband_outage",
        [10, 10, 10, 80, 80, 80],
        PolicyFamily::Access,
        50,
        1,
    ),
    stoppage(
        "strike_ballot",
        [20, 20, 20, 10, 10, 10],
        PolicyFamily::Staffing,
        0,
        2,
    ),
    stoppage(
        "flooding_halts_repairs",
        [10, 10, 10, 30, 30, 30],
        PolicyFamily::Repairs,
        0,
        2,
    ),
    stoppage(
        "volunteers_stretched",
        [0, 0, 0, 20, 20, 20],
        PolicyFamily::Access,
        25,
        2,
    ),
    stoppage(
        "materials_shortage",
        [15, 15, 15, 15, 15, 15],
        PolicyFamily::Repairs,
        50,
        2,
    ),
    audit(
        "teacher_promises_audit",
        [30, 30, 30, 20, 20, 20],
        [true, false, false],
    ),
    audit(
        "building_promises_audit",
        [20, 20, 20, 30, 30, 30],
        [false, true, false],
    ),
    audit(
        "spot_check_on_every_promise",
        [25, 25, 25, 25, 25, 25],
        [true, true, true],
    ),
];
pub const GOOD: [Headline; 6] = [
    story("inspectorate_praise", [15_000; 6]),
    story("volunteer_surge", [0, 0, 0, 25_000, 25_000, 25_000]),
    story("attendance_rises", [20_000, 10_000, 0, 20_000, 10_000, 0]),
    story("recruitment_up", [12_000; 6]),
    windfall("education_grant", 15),
    windfall("charity_donation", 10),
];

/// Trade-off stories (decision 0014): bad news for some groups and good news for others. One bad headline in `MIXED_ONE_IN` is one
/// of these instead of an ordinary one. `effect` is who loses (exposure, in percent of the worst case) and `winners` who gains
/// (a flat gift in ppm); no group is in both.
pub const MIXED: [Headline; 4] = [
    tradeoff(
        "funding_formula_shift",
        [40, 40, 40, 0, 0, 0],
        [0, 0, 0, 20_000, 20_000, 20_000],
    ),
    tradeoff(
        "school_costs_tax_rise",
        [0, 0, 60, 0, 0, 60],
        [10_000, 10_000, 0, 10_000, 10_000, 0],
    ),
    tradeoff(
        "free_meals_widened",
        [0, 0, 30, 0, 0, 30],
        [25_000, 10_000, 0, 25_000, 10_000, 0],
    ),
    tradeoff(
        "small_schools_merged",
        [0, 0, 0, 70, 70, 40],
        [0, 8_000, 12_000, 0, 0, 0],
    ),
];

pub const fn deck(tone: Tone) -> &'static [Headline] {
    match tone {
        Tone::Bad => &BAD,
        Tone::Good => &GOOD,
    }
}

/// What a real headline does, in numbers (decision 0013). The words, the link and the rule that chose these numbers are kept
/// with the campaign; the rules of the game only ever see this. For bad news `effect` is each group's exposure in percent of the
/// worst case, as for an invented headline; for good news it is each group's gain in ppm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealEffect {
    pub tone: Tone,
    pub effect: [u32; 6],
    /// For bad news that is also a trade-off: the groups it helps, a flat gain in ppm (decision 0014). Zero otherwise.
    pub winners: [u32; 6],
}
/// A real headline waiting to take a slot: which item of the term's import it is, and what it does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealSlot {
    pub item: u8,
    pub effect: RealEffect,
}
/// The real headlines each term has imported, by term (decision 0013).
pub type RealCalendar = std::collections::BTreeMap<u8, Vec<RealSlot>>;

/// The rule that turned a real headline into numbers: which it was, in words, and what it does.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealRule {
    /// A short identifier, for example `teacher_workforce_bad`.
    pub id: String,
    /// What the rule is about, in words for the screen.
    pub label: String,
    pub effect: RealEffect,
}
/// A real headline, imported once and from then on part of the campaign's record (decision 0013). Everything shown about it is
/// here, so a replay shows exactly what the player saw; only `rule.effect` is ever used by the rules of the game.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealItem {
    pub title: String,
    pub summary: String,
    /// The page it came from. Always an `https://` link.
    pub url: String,
    /// `YYYY-MM-DD`: the day it was published.
    pub published: String,
    /// The rule that matched. `None` when nothing matched: it is shown and changes nothing.
    pub rule: Option<RealRule>,
}

fn plain_text(s: &str, max_chars: usize) -> bool {
    !s.is_empty() && s.chars().count() <= max_chars && !s.chars().any(char::is_control)
}
fn plain_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && matches!(s[5..7].parse::<u8>(), Ok(1..=12))
        && matches!(s[8..10].parse::<u8>(), Ok(1..=31))
}
impl RealItem {
    /// Whether this is a well-formed record: bounded text, a real-looking link and date, and numbers inside the same limits
    /// as an invented headline.
    pub fn is_valid(&self) -> bool {
        let rule_ok = self.rule.as_ref().is_none_or(|r| {
            plain_text(&r.id, 48)
                && r.id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                && plain_text(&r.label, 80)
                && match r.effect.tone {
                    // Bad news may also help some groups, but never the ones it hurts (decision 0014).
                    Tone::Bad => {
                        r.effect.effect.iter().all(|e| *e <= 100)
                            && r.effect.winners.iter().all(|w| *w <= SHOCK_CAP_PPM)
                            && r.effect
                                .effect
                                .iter()
                                .zip(r.effect.winners)
                                .all(|(loses, gains)| *loses == 0 || gains == 0)
                    }
                    Tone::Good => {
                        r.effect.effect.iter().all(|e| *e <= SHOCK_CAP_PPM)
                            && r.effect.winners == [0; 6]
                    }
                }
        });
        plain_text(&self.title, 200)
            && plain_text(&self.summary, 400)
            && self.url.starts_with("https://")
            && plain_text(&self.url, 300)
            && !self.url.contains(char::is_whitespace)
            && plain_date(&self.published)
            && rule_ok
    }
}
/// A batch of real headlines is acceptable when every item is, there are not too many, only as many as can change the game
/// carry a rule, and no link appears twice.
pub fn valid_import(items: &[RealItem]) -> bool {
    let mut urls: Vec<&str> = items.iter().map(|i| i.url.as_str()).collect();
    urls.sort_unstable();
    urls.dedup();
    !items.is_empty()
        && items.len() <= MAX_REAL_ITEMS
        && items.iter().all(RealItem::is_valid)
        && items.iter().filter(|i| i.rule.is_some()).count() <= MAX_REAL_PER_TERM
        && urls.len() == items.len()
}

/// A headline placed on a quarter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scheduled {
    /// The quarter it lands in, at the end of that quarter.
    pub quarter: Quarter,
    pub tone: Tone,
    /// Position in the deck of its tone (meaningless for a real headline).
    pub index: usize,
    /// Not announced in advance, and nothing can be done about it.
    pub surprise: bool,
    /// Set when a real headline took this slot: which item of the term's import it is, and what it does.
    pub real: Option<RealSlot>,
    /// Set when this bad quarter carries a trade-off story: `index` is then a position in `MIXED`.
    pub mixed: bool,
}
impl Scheduled {
    /// What it does. A real headline is a mood story and nothing else (it may help some groups and hurt others, but it never
    /// stops work, sends inspectors or brings credits).
    pub fn headline(&self) -> Headline {
        match self.real {
            Some(slot) => Headline {
                id: "real_news",
                effect: slot.effect.effect,
                disrupts: None,
                scrutiny: None,
                grant: 0,
                winners: slot.effect.winners,
            },
            None if self.mixed => MIXED[self.index],
            None => deck(self.tone)[self.index],
        }
    }
}

/// A headline that has landed, kept for the news list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Landed {
    pub scheduled: Scheduled,
    /// What it did to each group's support, after any response.
    pub shock: [i32; 6],
    pub responded: bool,
    /// For an inspection, the units promised and not yet delivered that it counted, per kind of work; zero otherwise.
    pub owed: [u16; 3],
    /// For a windfall, the credits that arrived; zero otherwise.
    pub granted: u16,
}

/// A response that was paid for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Response {
    pub headline_quarter: Quarter,
    pub credits: u16,
}

/// SplitMix64: a small, well-known integer generator. Pure, so replays agree on every platform.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The headlines of one term, in quarter order. Empty for a term the engine cannot record.
///
/// The order of the draws is part of the rules (a Python port in `tools/support-model/news_model.py` repeats it): the quarters
/// are chosen first by a partial shuffle of quarters 2 to 6, then, quarter by quarter, the tone, the headline and whether it
/// is a surprise.
pub fn schedule(seed: u64, term: u8) -> Vec<Scheduled> {
    schedule_with(seed, term, &[])
}

/// The headlines of one term when real news has been imported for it (decision 0013). The invented calendar is drawn exactly
/// as without real news, then each real headline in turn takes the first quarter of its tone not yet taken: it is announced a
/// quarter ahead (it is already news) and is never a surprise. A real headline whose tone has no quarter left is not
/// scheduled, so term 1, which has no bad news, can never carry a bad real headline.
pub fn schedule_with(seed: u64, term: u8, real: &[RealSlot]) -> Vec<Scheduled> {
    let mut out = invented(seed, term);
    if term >= MIXED_FROM_TERM {
        trade_offs(seed, &mut out);
    }
    for slot in real.iter().take(MAX_REAL_PER_TERM) {
        if let Some(target) = out
            .iter_mut()
            .find(|s| s.real.is_none() && s.tone == slot.effect.tone)
        {
            target.real = Some(*slot);
            target.surprise = false;
        }
    }
    out
}

/// Some bad quarters carry a trade-off story instead of an ordinary one (decision 0014). Each bad quarter is decided on its own by
/// a draw from the seed and the quarter alone, so the quarters, the tones and the surprises of the invented calendar are
/// untouched; a story is not repeated in a term. A trade-off replaces whatever was drawn for the quarter, including a stoppage
/// or an inspection, which then does not happen.
fn trade_offs(seed: u64, slots: &mut [Scheduled]) {
    let mut used = [false; MIXED.len()];
    for s in slots.iter_mut().filter(|s| s.tone == Tone::Bad) {
        let mut state = seed
            .wrapping_add(u64::from(s.quarter.value()).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            ^ 0xD1B5_4A32_D192_ED03;
        let roll = next(&mut state);
        if !roll.is_multiple_of(MIXED_ONE_IN) {
            continue;
        }
        let first = ((roll >> 16) % MIXED.len() as u64) as usize;
        if let Some(index) = (0..MIXED.len())
            .map(|k| (first + k) % MIXED.len())
            .find(|i| !used[*i])
        {
            used[index] = true;
            s.mixed = true;
            s.index = index;
        }
    }
}

fn invented(seed: u64, term: u8) -> Vec<Scheduled> {
    if term == 0 || term > crate::MAX_TERMS {
        return vec![];
    }
    let count = usize::from(if term == 1 {
        QUIET_TERM_HEADLINES
    } else {
        HEADLINES_PER_TERM
    });
    let mut state = seed.wrapping_add(u64::from(term).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let mut slots: [u8; 5] = [2, 3, 4, 5, 6];
    for i in 0..count {
        let left = (slots.len() - i) as u64;
        let j = i + (next(&mut state) % left) as usize;
        slots.swap(i, j);
    }
    let mut chosen: Vec<u8> = slots[..count].to_vec();
    chosen.sort_unstable();
    let mut used_bad = [false; BAD.len()];
    let mut used_good = [false; GOOD.len()];
    let mut out = Vec::with_capacity(count);
    for in_term in chosen {
        let tone_roll = next(&mut state) % 100;
        let index_roll = next(&mut state);
        let surprise_roll = next(&mut state) % SURPRISE_ONE_IN;
        let tone = if term == 1 || tone_roll < GOOD_PERCENT {
            Tone::Good
        } else {
            Tone::Bad
        };
        let used: &mut [bool] = match tone {
            Tone::Bad => &mut used_bad,
            Tone::Good => &mut used_good,
        };
        let mut index = (index_roll % used.len() as u64) as usize;
        // No headline twice in a term: take the next unused one.
        while used[index] {
            index = (index + 1) % used.len();
        }
        used[index] = true;
        let Ok(quarter) = Quarter::new((term - 1) * TERM_QUARTERS + in_term) else {
            continue;
        };
        out.push(Scheduled {
            quarter,
            tone,
            index,
            surprise: surprise_roll == 0,
            real: None,
            mixed: false,
        });
    }
    out
}

/// What bad news takes in a given term: (base in ppm, share of a group's lead over 50% in percent).
pub fn severity(term: u8) -> (u32, u32) {
    let n = u32::from(term.saturating_sub(2));
    (
        BASE_PPM * (100 + BASE_STEP_PCT * n).min(BASE_CAP_PCT) / 100,
        (SHARE_PCT + SHARE_STEP_PCT * n).min(SHARE_CAP_PCT),
    )
}

/// What a headline would do to each group's support if nothing were done, given where support stands now.
pub fn landing_shock(s: &Scheduled, term: u8, support: &[u32; 6]) -> [i32; 6] {
    let effect = s.headline().effect;
    let mut out = [0_i32; 6];
    match s.tone {
        Tone::Good => {
            for (o, gain) in out.iter_mut().zip(effect) {
                *o = i32::try_from(gain.min(SHOCK_CAP_PPM)).unwrap_or(0);
            }
        }
        Tone::Bad => {
            let (base, share) = severity(term);
            for ((o, exposure), now) in out.iter_mut().zip(effect).zip(support) {
                let lead = u64::from(now.saturating_sub(ELECTION_HELD_ABOVE_PPM));
                let inner = u64::from(base) + lead * u64::from(share) / 100;
                let hit = (u64::from(exposure) * inner / 100).min(u64::from(SHOCK_CAP_PPM));
                *o = -i32::try_from(hit).unwrap_or(0);
            }
            // A trade-off story also gives some groups a flat gain (decision 0014).
            for (o, gain) in out.iter_mut().zip(s.headline().winners) {
                *o = o.saturating_add(i32::try_from(gain.min(SHOCK_CAP_PPM)).unwrap_or(0));
            }
        }
    }
    out
}

/// How much of what unfinished work would have earned them inspectors take, in a given term, in percent.
pub fn scrutiny_pct(term: u8) -> u32 {
    let n = u32::from(term.saturating_sub(2));
    (SCRUTINY_PCT + SCRUTINY_STEP_PCT * n).min(SCRUTINY_CAP_PCT)
}

/// What inspectors take from each group, given what the promised-and-undelivered work would have earned it (`earned`, in
/// ppm, as the delivery rules compute it). A group that the work would have cost more than it gave loses nothing.
pub fn scrutiny_shock(earned: &[i32; 6], term: u8) -> [i32; 6] {
    let pct = i64::from(scrutiny_pct(term));
    earned.map(|e| {
        let take = (i64::from(e.max(0)) * pct / 100).min(i64::from(SHOCK_CAP_PPM));
        -i32::try_from(take).unwrap_or(0)
    })
}

/// A response takes this much off every loss (toward zero).
pub fn soften(shock: [i32; 6]) -> [i32; 6] {
    shock.map(|x| {
        if x < 0 {
            x * (100 - RELIEF_CUT_PCT) / 100
        } else {
            x
        }
    })
}

/// How much of its usual capacity each kind of work (staffing, repairs, access) delivers in `quarter`, in percent, given the
/// disruptions in the news. A disruption lands in the quarter its headline is scheduled for and lasts its `quarters`; one that
/// starts in the last quarter of a term carries into the first of the next. An answered headline is cut by the same share a
/// response takes off every loss. Where several disruptions meet, the worst one rules.
pub fn capacity_kept(
    seed: u64,
    quarter: Quarter,
    answered: impl Fn(Quarter) -> bool,
    real: &RealCalendar,
) -> [u32; 3] {
    let mut keep = [100_u32; 3];
    let term = quarter.term();
    for t in [term.saturating_sub(1), term] {
        for s in schedule_with(seed, t, real.get(&t).map_or(&[][..], Vec::as_slice)) {
            let Some(d) = s.headline().disrupts.filter(|_| s.tone == Tone::Bad) else {
                continue;
            };
            let from = s.quarter.value();
            if !(from..from.saturating_add(d.quarters)).contains(&quarter.value()) {
                continue;
            }
            let kept = if answered(s.quarter) {
                softened_keep(d.keep_pct)
            } else {
                d.keep_pct
            };
            let slot = &mut keep[family_slot(d.family)];
            *slot = (*slot).min(kept);
        }
    }
    keep
}

/// A response takes the same share off the cut as off a loss: a stoppage becomes a slowdown.
pub fn softened_keep(keep_pct: u32) -> u32 {
    let cut = 100 - keep_pct.min(100);
    100 - cut * (100 - RELIEF_CUT_PCT.unsigned_abs()) / 100
}

/// Index of a kind of work in the `[staffing, repairs, access]` order used everywhere.
pub const fn family_slot(family: PolicyFamily) -> usize {
    match family {
        PolicyFamily::Staffing => 0,
        PolicyFamily::Repairs => 1,
        PolicyFamily::Access => 2,
    }
}

/// Support lost overall, in ppm, if every group took `shock`: the groups' losses weighted by their share of the voters.
pub fn overall_loss_ppm(shock: &[i32; 6], weights_ppm: &[u32; 6]) -> u64 {
    shock
        .iter()
        .zip(weights_ppm)
        .filter(|(s, _)| **s < 0)
        .map(|(s, w)| u64::from(s.unsigned_abs()) * u64::from(*w))
        .sum::<u64>()
        / 1_000_000
}

/// What a response costs: credits in proportion to the damage it was expected to do, so protecting votes is a real trade
/// against delivering work. At least one credit.
pub fn relief_cost(shock: &[i32; 6], weights_ppm: &[u32; 6]) -> u16 {
    let lost = overall_loss_ppm(shock, weights_ppm);
    let credits = (lost * u64::from(RELIEF_CREDITS_PER_POINT))
        .div_ceil(10_000)
        .max(1);
    u16::try_from(credits).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEIGHTS: [u32; 6] = [260_000, 320_000, 220_000, 70_000, 80_000, 50_000];

    #[test]
    fn a_term_has_the_headlines_the_rules_promise() {
        for seed in [0_u64, 1, 7, 0xDEAD_BEEF, u64::MAX] {
            let first = schedule(seed, 1);
            assert_eq!(first.len(), usize::from(QUIET_TERM_HEADLINES));
            assert!(
                first.iter().all(|s| s.tone == Tone::Good),
                "term 1 has no bad news (seed {seed})"
            );
            for term in 2..=crate::MAX_TERMS {
                let s = schedule(seed, term);
                assert_eq!(s.len(), usize::from(HEADLINES_PER_TERM), "term {term}");
                // Quarters 2 to 6 of that term, distinct and in order; never the first quarter of a term.
                let in_term: Vec<u8> = s.iter().map(|h| h.quarter.in_term()).collect();
                assert!(in_term.windows(2).all(|w| w[0] < w[1]), "{in_term:?}");
                assert!(in_term.iter().all(|q| (2..=6).contains(q)), "{in_term:?}");
                assert!(s.iter().all(|h| h.quarter.term() == term));
                // No headline twice in a term.
                for tone in [Tone::Bad, Tone::Good] {
                    let mut ids: Vec<(bool, usize)> = s
                        .iter()
                        .filter(|h| h.tone == tone)
                        .map(|h| (h.mixed, h.index))
                        .collect();
                    let n = ids.len();
                    ids.sort_unstable();
                    ids.dedup();
                    assert_eq!(ids.len(), n);
                }
            }
        }
        assert!(schedule(1, 0).is_empty());
        assert!(schedule(1, crate::MAX_TERMS + 1).is_empty());
    }

    #[test]
    fn the_schedule_is_exactly_what_the_independent_python_port_draws() {
        // Expected values come from tools/support-model/news_model.py, a separate implementation of the same rules that
        // reads the deck from authored.json: (quarter, tone, index, surprise).
        let plan = |seed, term| -> Vec<(u8, Tone, usize, bool)> {
            schedule(seed, term)
                .iter()
                .map(|s| (s.quarter.value(), s.tone, s.index, s.surprise))
                .collect()
        };
        assert_eq!(
            plan(12345, 1),
            [(2, Tone::Good, 3, false), (4, Tone::Good, 2, false)]
        );
        assert_eq!(
            plan(12345, 2),
            [
                (8, Tone::Bad, 8, false),
                (9, Tone::Bad, 2, true),
                (10, Tone::Bad, 9, false),
                (11, Tone::Bad, 7, false)
            ]
        );
        assert_eq!(
            plan(12345, 3),
            [
                (14, Tone::Bad, 13, false),
                (16, Tone::Bad, 3, false),
                (17, Tone::Bad, 10, true),
                (18, Tone::Bad, 5, true)
            ]
        );
    }

    #[test]
    fn the_trade_off_overlay_is_exactly_what_the_independent_python_port_draws() {
        // Expected values come from tools/support-model/news_model.py (schedule_real): (quarter, tone, index, surprise, mixed).
        // For a trade-off quarter `index` is a position in MIXED; the surprise flag is the invented draw's, untouched.
        let plan = |seed, term| -> Vec<(u8, Tone, usize, bool, bool)> {
            schedule(seed, term)
                .iter()
                .map(|s| (s.quarter.value(), s.tone, s.index, s.surprise, s.mixed))
                .collect()
        };
        assert_eq!(
            plan(7, 3),
            [
                (14, Tone::Bad, 2, false, true),
                (15, Tone::Bad, 1, true, false),
                (16, Tone::Bad, 3, true, true),
                (17, Tone::Good, 5, false, false)
            ]
        );
        assert_eq!(
            plan(99, 4),
            [
                (20, Tone::Bad, 3, false, true),
                (21, Tone::Bad, 0, false, true),
                (22, Tone::Bad, 8, false, false),
                (24, Tone::Bad, 2, false, false)
            ]
        );
        assert_eq!(
            plan(424242, 6),
            [
                (33, Tone::Bad, 2, true, false),
                (34, Tone::Bad, 3, true, false),
                (35, Tone::Bad, 8, false, false),
                (36, Tone::Bad, 3, false, true)
            ]
        );
    }

    #[test]
    fn the_calendar_is_a_pure_function_of_the_seed_and_the_term() {
        assert_eq!(schedule(42, 3), schedule(42, 3));
        assert_ne!(schedule(42, 3), schedule(43, 3));
        // Different terms of one game differ too, so a calendar is not one term repeated.
        assert_ne!(
            schedule(42, 2).iter().map(|s| s.index).collect::<Vec<_>>(),
            schedule(42, 3).iter().map(|s| s.index).collect::<Vec<_>>()
        );
    }

    #[test]
    fn over_many_games_about_a_third_are_surprises_and_three_in_ten_are_good() {
        let (mut total, mut surprises, mut good) = (0_u32, 0_u32, 0_u32);
        for seed in 0..400_u64 {
            for term in 2..=4 {
                for s in schedule(seed, term) {
                    total += 1;
                    surprises += u32::from(s.surprise);
                    good += u32::from(s.tone == Tone::Good);
                }
            }
        }
        let pct = |n: u32| n * 100 / total;
        assert!((28..=38).contains(&pct(surprises)), "{}", pct(surprises));
        assert!((24..=36).contains(&pct(good)), "{}", pct(good));
    }

    #[test]
    fn bad_news_takes_more_the_longer_you_have_served_and_the_higher_you_stand() {
        // Severity grows with every term after the second, up to a cap.
        assert_eq!(severity(2), (5_000, 40));
        assert_eq!(severity(3), (7_500, 48));
        assert_eq!(severity(4), (10_000, 56));
        assert_eq!(severity(10), (20_000, 90));
        assert_eq!(severity(1), severity(2));
        let bad = Scheduled {
            quarter: Quarter::new(8).unwrap(),
            tone: Tone::Bad,
            index: 3, // rural bus cuts: 0% urban, 100% rural
            surprise: false,
            real: None,
            mixed: false,
        };
        let flat = landing_shock(&bad, 2, &[500_000; 6]);
        assert_eq!(flat, [0, 0, 0, -5_000, -5_000, -5_000]);
        // The same headline costs a group that stands high far more than one that stands lower: the lead is taxed.
        let high = landing_shock(
            &bad,
            2,
            &[500_000, 500_000, 500_000, 560_000, 520_000, 500_000],
        );
        assert_eq!(high[3], -(5_000 + 60_000 * 40 / 100)); // base 5,000 + 40% of a 6-point lead
        assert_eq!(high[4], -(5_000 + 20_000 * 40 / 100)); // base 5,000 + 40% of a 2-point lead
        assert_eq!(high[5], -5_000); // no lead, base only
        assert!(high[3] < high[4] && high[4] < high[5]);
        // Never more than one headline may take from one group (the engine's shock cap).
        let capped = landing_shock(&bad, 10, &[1_000_000; 6]);
        assert!(capped.iter().all(|s| s.unsigned_abs() <= SHOCK_CAP_PPM));
        assert_eq!(capped[5], -(SHOCK_CAP_PPM as i32));
        // Good news is a flat gift, whatever the lead.
        let good = Scheduled {
            quarter: Quarter::new(3).unwrap(),
            tone: Tone::Good,
            index: 1, // volunteer surge: rural groups
            surprise: true,
            real: None,
            mixed: false,
        };
        assert_eq!(
            landing_shock(&good, 1, &[900_000; 6]),
            [0, 0, 0, 25_000, 25_000, 25_000]
        );
    }

    #[test]
    fn disruptions_cut_one_kind_of_work_for_their_quarters_and_never_in_the_quiet_first_term() {
        // Nothing is ever disrupted in term 1: it has no bad news.
        for seed in 0..200_u64 {
            for q in 1..=6 {
                assert_eq!(
                    capacity_kept(
                        seed,
                        Quarter::new(q).unwrap(),
                        |_| false,
                        &RealCalendar::new()
                    ),
                    [100; 3],
                    "seed {seed} quarter {q}"
                );
            }
        }
        // Find a disruption of two quarters and watch it come and go.
        let (seed, s) = (0..10_000_u64)
            .find_map(|seed| {
                {
                    schedule(seed, 2).into_iter().find(|s| {
                        s.tone == Tone::Bad
                            && s.headline().disrupts.is_some_and(|d| d.quarters == 2)
                            && s.quarter.value() < 12
                    })
                }
                .map(|s| (seed, s))
            })
            .unwrap();
        let d = s.headline().disrupts.unwrap();
        let slot = family_slot(d.family);
        let at = |q: u8, answered: bool| {
            capacity_kept(
                seed,
                Quarter::new(q).unwrap(),
                move |_| answered,
                &RealCalendar::new(),
            )
        };
        assert_eq!(
            at(s.quarter.value() - 1, false)[slot],
            100,
            "not before it lands"
        );
        assert!(at(s.quarter.value(), false)[slot] <= d.keep_pct);
        assert!(
            at(s.quarter.value() + 1, false)[slot] <= d.keep_pct,
            "and the quarter after"
        );
        assert_eq!(
            at(s.quarter.value() + 2, false)[slot].min(100),
            at(s.quarter.value() + 2, false)[slot]
        );
        // The other kinds of work are untouched by this headline (another may hit them).
        // An answered headline is cut by 60% less than it would have been.
        assert_eq!(softened_keep(0), 60);
        assert_eq!(softened_keep(50), 80);
        assert_eq!(softened_keep(25), 70);
        assert_eq!(softened_keep(100), 100);
        assert!(
            at(s.quarter.value(), true)[slot]
                >= softened_keep(d.keep_pct).min(at(s.quarter.value(), false)[slot])
        );
    }

    #[test]
    fn a_disruption_in_the_last_quarter_of_a_term_carries_into_the_next() {
        let found = (0..20_000_u64).find_map(|seed| {
            schedule(seed, 2)
                .into_iter()
                .find(|s| {
                    s.tone == Tone::Bad
                        && s.quarter.value() == 12
                        && s.headline().disrupts.is_some_and(|d| d.quarters == 2)
                })
                .map(|s| (seed, s))
        });
        let (seed, s) = found.expect("some calendar disrupts the last quarter of a term");
        let d = s.headline().disrupts.unwrap();
        let carried = capacity_kept(
            seed,
            Quarter::new(13).unwrap(),
            |_| false,
            &RealCalendar::new(),
        );
        assert!(
            carried[family_slot(d.family)] <= d.keep_pct,
            "the first quarter of term 3 is still disrupted"
        );
    }

    #[test]
    fn inspectors_take_a_growing_multiple_of_what_unfinished_work_would_have_earned() {
        // 26 staffing and 30 repair units still owed: what delivering them would earn each group (as the delivery rules
        // compute it), from tools/support-model/news_model.py.
        let earned = [29_840, 40_000, -9_040, 32_552, 40_000, 5_368];
        assert_eq!(
            scrutiny_shock(&earned, 2),
            [-14_920, -20_000, 0, -16_276, -20_000, -2_684]
        );
        assert_eq!(
            scrutiny_shock(&earned, 3),
            [-19_396, -26_000, 0, -21_158, -26_000, -3_489]
        );
        // Later terms hit the cap on one group from one headline; a group the work would have cost loses nothing.
        let late = scrutiny_shock(&earned, 10);
        assert_eq!(late, [-40_000, -40_000, 0, -40_000, -40_000, -9_125]);
        assert!(late.iter().all(|x| x.unsigned_abs() <= SHOCK_CAP_PPM));
        // 50% in term 2, 15 points more each term after, and never more than 200%.
        let pcts: Vec<u32> = (1..=crate::MAX_TERMS).map(scrutiny_pct).collect();
        assert_eq!(pcts, [50, 50, 65, 80, 95, 110, 125, 140, 155, 170]);
        assert_eq!(scrutiny_pct(40), SCRUTINY_CAP_PCT);
        // Nothing owed, nothing taken.
        assert_eq!(scrutiny_shock(&[0; 6], 5), [0; 6]);
    }

    #[test]
    fn the_deck_keeps_inspections_in_bad_news_and_windfalls_in_good_and_a_whole_game_of_windfalls_fits()
     {
        for h in BAD {
            assert_eq!(h.grant, 0, "{}", h.id);
            assert!(
                !(h.disrupts.is_some() && h.scrutiny.is_some()),
                "{} does one or the other",
                h.id
            );
            if let Some(inspection) = h.scrutiny {
                assert!(
                    inspection.families.iter().any(|counted| *counted),
                    "{}",
                    h.id
                );
            }
        }
        let windfalls: Vec<_> = GOOD.iter().filter(|h| h.grant > 0).collect();
        assert_eq!(windfalls.len(), 2);
        for h in GOOD {
            assert!(h.disrupts.is_none() && h.scrutiny.is_none(), "{}", h.id);
            assert!(h.grant <= MAX_WINDFALL_CREDITS, "{}", h.id);
            if h.grant > 0 {
                assert_eq!(h.effect, [0; 6], "a windfall brings credits, not mood");
            }
        }
        assert_eq!(
            BAD.iter().filter(|h| h.scrutiny.is_some()).count(),
            3,
            "three inspections"
        );
        // Every headline of every term being the largest windfall still fits inside what a game may hold.
        let most = crate::PILOT_CREDITS * u16::from(crate::MAX_TERMS) + MAX_WINDFALL_TOTAL;
        assert_eq!(crate::MAX_CREDITS, most);
        assert_eq!(MAX_WINDFALL_TOTAL, 15 * (2 + 4 * 9));
    }

    #[test]
    fn trade_off_stories_replace_some_bad_quarters_from_term_three_and_move_nothing_else() {
        let (mut bad, mut mixed, mut late_bad) = (0_u32, 0_u32, 0_u32);
        for seed in 0..600_u64 {
            // The first two terms are exactly as they were: no trade-off story, ever.
            for term in 1..MIXED_FROM_TERM {
                assert!(
                    schedule(seed, term).iter().all(|s| !s.mixed),
                    "seed {seed} term {term}"
                );
            }
            for term in MIXED_FROM_TERM..=crate::MAX_TERMS {
                let with = schedule(seed, term);
                let plain = invented(seed, term);
                assert_eq!(with.len(), plain.len());
                for (a, b) in with.iter().zip(&plain) {
                    // The quarters, the tones and who is told in advance never move.
                    assert_eq!(
                        (a.quarter, a.tone, a.surprise),
                        (b.quarter, b.tone, b.surprise)
                    );
                    if a.mixed {
                        assert_eq!(a.tone, Tone::Bad, "only a bad quarter can carry one");
                        assert!(a.index < MIXED.len());
                        assert_eq!(a.headline(), MIXED[a.index]);
                    } else {
                        assert_eq!(a, b, "an untouched quarter is the invented one");
                    }
                    late_bad += u32::from(a.tone == Tone::Bad);
                    mixed += u32::from(a.mixed);
                }
                // A story is told once in a term.
                let mut told: Vec<usize> =
                    with.iter().filter(|s| s.mixed).map(|s| s.index).collect();
                let n = told.len();
                told.sort_unstable();
                told.dedup();
                assert_eq!(told.len(), n);
                bad += u32::from(with.iter().any(|s| s.tone == Tone::Bad));
            }
        }
        // About one bad quarter in five, and a good number of terms have none at all.
        assert!(bad > 0 && late_bad > 100);
        let share = mixed * 100 / late_bad;
        assert!(
            (12..=28).contains(&share),
            "{share}% of {late_bad} bad quarters"
        );
    }
    #[test]
    fn a_trade_off_hurts_some_groups_as_bad_news_does_and_gives_others_a_flat_gift() {
        for (index, story) in MIXED.iter().enumerate() {
            // No group is both hurt and helped, and every story does both.
            assert!(
                story.effect.iter().any(|e| *e > 0) && story.winners.iter().any(|w| *w > 0),
                "{}",
                story.id
            );
            assert!(
                story
                    .effect
                    .iter()
                    .zip(story.winners)
                    .all(|(l, g)| *l == 0 || g == 0),
                "{}",
                story.id
            );
            assert!(
                story.effect.iter().all(|e| *e <= 100)
                    && story.winners.iter().all(|w| *w <= SHOCK_CAP_PPM)
            );
            assert_eq!(
                (story.disrupts, story.scrutiny, story.grant),
                (None, None, 0)
            );
            let s = Scheduled {
                quarter: Quarter::new(14).unwrap(),
                tone: Tone::Bad,
                index,
                surprise: false,
                real: None,
                mixed: true,
            };
            let shock = landing_shock(&s, 3, &[500_000; 6]);
            for (g, got) in shock.iter().enumerate() {
                if story.winners[g] > 0 {
                    assert_eq!(
                        *got,
                        i32::try_from(story.winners[g]).unwrap(),
                        "a flat gift, whatever the lead"
                    );
                } else if story.effect[g] > 0 {
                    assert!(*got < 0);
                }
            }
            // A response takes 60% off what is lost and leaves what is gained alone.
            let softened = soften(shock);
            for g in 0..6 {
                if shock[g] > 0 {
                    assert_eq!(softened[g], shock[g]);
                } else {
                    assert_eq!(softened[g], shock[g] * 40 / 100);
                }
            }
            // The weighted overall effect is a trade: the stories are not all losses or all gains.
            let overall: i64 = shock
                .iter()
                .zip(WEIGHTS)
                .map(|(s, w)| i64::from(*s) * i64::from(w))
                .sum();
            assert!(overall.abs() < 40_000 * 1_000_000, "{}", story.id);
        }
        // Only what is lost is priced: a response never costs anything for the groups that gain.
        let story = Scheduled {
            quarter: Quarter::new(14).unwrap(),
            tone: Tone::Bad,
            index: 0,
            surprise: false,
            real: None,
            mixed: true,
        };
        let shock = landing_shock(&story, 3, &[500_000; 6]);
        let only_losses = shock.map(|x| x.min(0));
        assert_eq!(
            overall_loss_ppm(&shock, &WEIGHTS),
            overall_loss_ppm(&only_losses, &WEIGHTS)
        );
    }

    fn real_slot(item: u8, tone: Tone, effect: [u32; 6]) -> RealSlot {
        RealSlot {
            item,
            effect: RealEffect {
                tone,
                effect,
                winners: [0; 6],
            },
        }
    }
    #[test]
    fn real_headlines_take_the_first_free_quarter_of_their_tone_and_nothing_else_moves() {
        for seed in [0_u64, 1, 7, 12345, u64::MAX] {
            // No real news: the calendar is exactly the invented one.
            for term in 1..=crate::MAX_TERMS {
                assert_eq!(schedule_with(seed, term, &[]), schedule(seed, term));
            }
            // Term 2 has bad headlines; two real bad ones take the first two bad quarters, in order, announced.
            let plain = schedule(seed, 2);
            let bad: Vec<_> = plain.iter().filter(|s| s.tone == Tone::Bad).collect();
            let real = [
                real_slot(0, Tone::Bad, [50; 6]),
                real_slot(3, Tone::Bad, [20; 6]),
            ];
            let with = schedule_with(seed, 2, &real);
            assert_eq!(with.len(), plain.len());
            for (a, b) in with.iter().zip(&plain) {
                assert_eq!(
                    (a.quarter, a.tone),
                    (b.quarter, b.tone),
                    "quarters never move"
                );
            }
            let taken: Vec<_> = with.iter().filter(|s| s.real.is_some()).collect();
            assert_eq!(taken.len(), bad.len().min(2), "seed {seed}");
            for (s, wanted) in taken.iter().zip(&bad) {
                assert_eq!(
                    s.quarter, wanted.quarter,
                    "the first free quarter of that tone"
                );
                assert!(!s.surprise, "a real headline is already news");
                assert_eq!(s.headline().effect, s.real.unwrap().effect.effect);
                assert_eq!(
                    (
                        s.headline().disrupts,
                        s.headline().scrutiny,
                        s.headline().grant
                    ),
                    (None, None, 0)
                );
            }
            // Untouched slots are the invented ones.
            for (a, b) in with.iter().zip(&plain).filter(|(a, _)| a.real.is_none()) {
                assert_eq!(a, b);
            }
            // A third real headline of the same tone finds no room: at most two a term.
            let three = [real[0], real[1], real_slot(9, Tone::Bad, [10; 6])];
            assert_eq!(
                schedule_with(seed, 2, &three)
                    .iter()
                    .filter(|s| s.real.is_some())
                    .count(),
                taken.len()
            );
            // Term 1 has good news only, so a bad real headline has nowhere to go and a good one replaces a good one.
            assert!(
                schedule_with(seed, 1, &[real_slot(0, Tone::Bad, [50; 6])])
                    .iter()
                    .all(|s| s.real.is_none())
            );
            let good = schedule_with(seed, 1, &[real_slot(2, Tone::Good, [9_000; 6])]);
            assert_eq!(good.iter().filter(|s| s.real.is_some()).count(), 1);
            assert_eq!(good[0].real.unwrap().item, 2);
        }
    }

    fn item(url: &str, rule: Option<RealRule>) -> RealItem {
        RealItem {
            title: "Teacher pay award announced".into(),
            summary: "Schools will receive funding for the award.".into(),
            url: url.into(),
            published: "2026-11-02".into(),
            rule,
        }
    }
    fn rule(tone: Tone, effect: [u32; 6]) -> Option<RealRule> {
        Some(RealRule {
            id: "teacher_workforce_good".into(),
            label: "Teacher pay and recruitment".into(),
            effect: RealEffect {
                tone,
                effect,
                winners: [0; 6],
            },
        })
    }
    #[test]
    fn an_import_must_be_small_well_formed_and_bounded_like_an_invented_headline() {
        let good = || {
            item(
                "https://www.gov.uk/government/news/a",
                rule(Tone::Good, [12_000; 6]),
            )
        };
        assert!(valid_import(&[good()]));
        assert!(valid_import(&[good(), item("https://www.gov.uk/b", None)]));
        assert!(!valid_import(&[]), "nothing to import");
        let many: Vec<_> = (0..=MAX_REAL_ITEMS)
            .map(|i| item(&format!("https://www.gov.uk/n{i}"), None))
            .collect();
        assert!(!valid_import(&many), "too many");
        // Only as many as can change the game may carry a rule.
        let three_rules: Vec<_> = (0..3)
            .map(|i| {
                item(
                    &format!("https://www.gov.uk/r{i}"),
                    rule(Tone::Good, [1; 6]),
                )
            })
            .collect();
        assert!(!valid_import(&three_rules));
        // The same link twice, a link that is not https, a bad date, empty or oversized text, control characters.
        assert!(!valid_import(&[good(), good()]));
        assert!(!valid_import(&[item("http://www.gov.uk/x", None)]));
        assert!(!valid_import(&[item("https://www.gov.uk/x y", None)]));
        let mut bad = good();
        bad.published = "2026-13-02".into();
        assert!(!valid_import(&[bad]));
        let mut bad = good();
        bad.title = String::new();
        assert!(!valid_import(&[bad]));
        let mut bad = good();
        bad.summary = "x".repeat(401);
        assert!(!valid_import(&[bad]));
        let mut bad = good();
        bad.title = "line\nbreak".into();
        assert!(!valid_import(&[bad]));
        // A real story may be a trade-off, helping some groups while it hurts others, but never the same group twice.
        let with_winners = |effect: [u32; 6], winners: [u32; 6]| {
            item(
                "https://www.gov.uk/x",
                Some(RealRule {
                    id: "funding_shift_mixed".into(),
                    label: "Money moves".into(),
                    effect: RealEffect {
                        tone: Tone::Bad,
                        effect,
                        winners,
                    },
                }),
            )
        };
        assert!(valid_import(&[with_winners(
            [40, 40, 40, 0, 0, 0],
            [0, 0, 0, 20_000, 20_000, 20_000]
        )]));
        assert!(
            !valid_import(&[with_winners(
                [40, 40, 40, 0, 0, 0],
                [1_000, 0, 0, 20_000, 0, 0]
            )]),
            "a group both hurt and helped"
        );
        assert!(
            !valid_import(&[with_winners(
                [40, 0, 0, 0, 0, 0],
                [0, 0, 0, 0, 0, SHOCK_CAP_PPM + 1]
            )]),
            "a gift beyond the cap"
        );
        // Good news is a gift already and has no winners to add.
        let mut good = rule(Tone::Good, [1_000; 6]).unwrap();
        good.effect.winners = [1; 6];
        assert!(!valid_import(&[item("https://www.gov.uk/x", Some(good))]));
        // Numbers stay inside the limits an invented headline keeps to.
        assert!(!valid_import(&[item(
            "https://www.gov.uk/x",
            rule(Tone::Bad, [101; 6])
        )]));
        assert!(!valid_import(&[item(
            "https://www.gov.uk/x",
            rule(Tone::Good, [SHOCK_CAP_PPM + 1; 6])
        )]));
    }

    #[test]
    fn a_response_cuts_every_loss_by_sixty_percent_and_costs_credits_in_proportion_to_the_damage() {
        let shock = [-10_000, -20_000, 0, -30_000, 0, 5_000];
        assert_eq!(soften(shock), [-4_000, -8_000, 0, -12_000, 0, 5_000]);
        // Weighted loss: 10,000*.26 + 20,000*.32 + 30,000*.07 = 2,600 + 6,400 + 2,100 = 11,100 ppm = 1.11 points,
        // ten credits a point, rounded up: 12.
        assert_eq!(overall_loss_ppm(&shock, &WEIGHTS), 11_100);
        assert_eq!(relief_cost(&shock, &WEIGHTS), 12);
        // Never free, even for a trifle.
        assert_eq!(relief_cost(&[-1, 0, 0, 0, 0, 0], &WEIGHTS), 1);
        assert_eq!(relief_cost(&[0; 6], &WEIGHTS), 1);
    }
}
