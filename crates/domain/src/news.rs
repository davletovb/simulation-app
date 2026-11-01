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

use crate::{Quarter, TERM_QUARTERS, reducer::ELECTION_HELD_ABOVE_PPM, reducer::SHOCK_CAP_PPM};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tone {
    Bad,
    Good,
}

/// One kind of headline. For bad news `effect` is each group's exposure, in percent of the worst case; for good news it
/// is each group's gain in ppm. Groups: urban low, middle, high income, then rural low, middle, high.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Headline {
    pub id: &'static str,
    pub effect: [u32; 6],
}

pub const BAD: [Headline; 8] = [
    Headline {
        id: "teacher_shortage",
        effect: [50, 50, 50, 25, 25, 25],
    },
    Headline {
        id: "roof_scare",
        effect: [25, 25, 25, 75, 75, 75],
    },
    Headline {
        id: "funding_row",
        effect: [50, 50, 50, 50, 50, 50],
    },
    Headline {
        id: "rural_bus_cuts",
        effect: [0, 0, 0, 100, 100, 100],
    },
    Headline {
        id: "cost_of_living",
        effect: [75, 40, 0, 75, 40, 0],
    },
    Headline {
        id: "exam_results_row",
        effect: [0, 60, 75, 0, 40, 50],
    },
    Headline {
        id: "agency_scandal",
        effect: [60, 60, 30, 30, 30, 15],
    },
    Headline {
        id: "broadband_outage",
        effect: [10, 10, 10, 80, 80, 80],
    },
];
pub const GOOD: [Headline; 4] = [
    Headline {
        id: "inspectorate_praise",
        effect: [15_000; 6],
    },
    Headline {
        id: "volunteer_surge",
        effect: [0, 0, 0, 25_000, 25_000, 25_000],
    },
    Headline {
        id: "attendance_rises",
        effect: [20_000, 10_000, 0, 20_000, 10_000, 0],
    },
    Headline {
        id: "recruitment_up",
        effect: [12_000; 6],
    },
];

pub const fn deck(tone: Tone) -> &'static [Headline] {
    match tone {
        Tone::Bad => &BAD,
        Tone::Good => &GOOD,
    }
}

/// A headline placed on a quarter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scheduled {
    /// The quarter it lands in, at the end of that quarter.
    pub quarter: Quarter,
    pub tone: Tone,
    /// Position in the deck of its tone.
    pub index: usize,
    /// Not announced in advance, and nothing can be done about it.
    pub surprise: bool,
}
impl Scheduled {
    pub fn headline(&self) -> &'static Headline {
        &deck(self.tone)[self.index]
    }
}

/// A headline that has landed, kept for the news list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Landed {
    pub scheduled: Scheduled,
    /// What it did to each group's support, after any response.
    pub shock: [i32; 6],
    pub responded: bool,
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
        }
    }
    out
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
                    let mut ids: Vec<usize> = s
                        .iter()
                        .filter(|h| h.tone == tone)
                        .map(|h| h.index)
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
            [(2, Tone::Good, 3, false), (4, Tone::Good, 0, false)]
        );
        assert_eq!(
            plan(12345, 2),
            [
                (8, Tone::Bad, 4, false),
                (9, Tone::Bad, 5, true),
                (10, Tone::Bad, 6, false),
                (11, Tone::Bad, 7, false)
            ]
        );
        assert_eq!(
            plan(12345, 3),
            [
                (14, Tone::Bad, 7, false),
                (16, Tone::Bad, 6, false),
                (17, Tone::Bad, 0, true),
                (18, Tone::Bad, 1, true)
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
        };
        assert_eq!(
            landing_shock(&good, 1, &[900_000; 6]),
            [0, 0, 0, 25_000, 25_000, 25_000]
        );
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
