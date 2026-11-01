# 0004. The election win bar

- **Status:** **Accepted** by the owner on 2026-11-01 (*"your recommended 55% / 60% bar is ok"*) and implemented. It was provisional
  before that; the original wording is kept below. The bars are now part of the hashed scenario (see [0007](0007-two-terms-in-one-campaign.md)).
- **Date:** 2026-11-01
- **Decided by:** the project owner, in conversation: *"On 4: i am not sure about."* Recorded by Claude,
  which proposed the starting value below. It is a starting point, not a considered choice.

## Proposed value (not yet implemented)

Win the election if **both** hold on election night (after the last quarter's effect has landed):

1. overall support is **at least 55%**, and
2. the groups above 50% together hold **at least 60% of the voters**.

Lose otherwise. Suggested labels for the end screen: *Re-elected*; *Narrow defeat* (at least 52%);
*Heavy defeat* (below 52%).

## What is known about it

From ten real plays under rules v2 (`../../tools/support-model/support_model.py`):

| Plays that win at 55% / 60% | Plays that lose |
|---|---|
| Balanced, Staffing-first, Staffing 45/40/10, Balanced a quarter late (barely, 55.35%) | Stagger then re-plan (54.71%), two quarters late (54.10%), Access-only, Staffing-only, Repairs-only, never spend |

**The bar is sensitive to a number nobody has measured.** In the engine's own sensitivity sweep, on-time
Balanced at the model's central strength clears 55% (55.96%), but at **half** the response strength it
does not (52.98%). 48 of the 72 accepted variants clear 55%; all 72 clear 52% and 50%. So:

| If the bar is | Effect |
|---|---|
| 50% | Almost any plan that spends wins. Too easy |
| 52% | Every sensitivity variant of a normal plan wins. Safe but undemanding |
| **55%** | On time wins at central strength, a quarter late barely, two quarters late loses. Demanding, and fragile if the true strength is lower |
| 58% | On-time Balanced loses at central strength. Needs better-than-baseline play |

## What would settle it

- Playtesting once events exist (a news calendar, ministers who slow down). With scripted ministers and no
  events the best plan is known in advance, so any bar measured now will move.
- A decision on how much the model's strength is trusted (the 50/100/150% sweep).

## To change

The bars are now named constants in `crates/domain/src/reducer.rs` (`ELECTION_MIN_SUPPORT_PPM = 550_000`,
`ELECTION_MIN_BLOCS_PPM = 600_000`, `ELECTION_HELD_ABOVE_PPM = 500_000`), mirrored in
`tools/support-model/support_model.py` (`BAR_SUPPORT_PCT`, `BAR_BLOCS_PCT`). **They are deliberately not in the
hashed `authored.json`** (see [0006](0006-what-the-snapshot-exposes.md)): they only read a final state, so
changing them does **not** invalidate saved games, and a change is a single edit plus the tests below.

To change a bar: edit the constant, update the tool, then run `cargo test -p myth-domain -p myth-local-api`;
the tests that pin a result (`balanced_term_wins_...`, `election_night_is_published_...`) will tell you if a
change flips the headline cases. Record the change as a new decision and mark this one superseded.

**Status update (2026-11-01):** implemented in the engine as the rule above, **still provisional**.
