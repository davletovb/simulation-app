# 0004. The election win bar

- **Status:** **The second bar (60% of voters in groups above 50%) was removed by [0009](0009-overall-support-is-the-only-bar.md); the first bar (55% overall support) stands.**
  Originally **accepted** by the owner on 2026-11-01 (*"your recommended 55% / 60% bar is ok"*) and implemented. It was provisional
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

The bar is a named constant in `crates/domain/src/reducer.rs` (`ELECTION_MIN_SUPPORT_PPM = 550_000`; `ELECTION_MIN_BLOCS_PPM`
was removed by 0009, and `ELECTION_HELD_ABOVE_PPM = 500_000` now only says which groups back you), mirrored in
`tools/support-model/support_model.py` (`BAR_SUPPORT_PCT`) and, since [0007](0007-two-terms-in-one-campaign.md),
**in the hashed `authored.json`** (`election.min_weighted_support_ppm`, `bloc_held_above_ppm`), because the
verdict decides whether the game continues. A test fails if the file and the constants disagree, and **changing a bar
invalidates saved games**: bump `RULES_VERSION` and regenerate the manifest (`tools/p2/build_pack.py --write`).

To change a bar: edit the constant and `authored.json`, update the tool, then run `cargo test --workspace`;
the tests that pin a result (`balanced_term_wins_...`, `election_night_is_published_...`) will tell you if a
change flips the headline cases. Record the change as a new decision and mark this one superseded.

**Status update (2026-11-01):** the owner accepted 55% / 60% and it is implemented. **Evidence that the second bar decides
nothing today:** `tools/support-model/coalition_sweep.py` plays 1,771 opening plans and about 163,000 mid-term changes of
course; every play that clears 55% overall also clears the 60% coalition bar, so the second bar has never been the reason
for a defeat. It is the first bar alone that decides, and the same four or five groups are always the ones above 50%.
