# 0004. The election win bar (provisional)

- **Status:** **Provisional.** The owner said they are not sure about it. Nothing in the engine reads it yet.
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

The two numbers should live in the scenario rules (and be mirrored in `authored.json`, with the same
drift test as the response numbers) when stage B is built, so changing them is one edit plus
regenerating the pinned files. Until then they exist only in `../stay-in-power.md` and in
`tools/support-model/support_model.py` (`BAR_SUPPORT_PCT`, `BAR_BLOCS_PCT`). Record any change as a new
decision and mark this one superseded.
