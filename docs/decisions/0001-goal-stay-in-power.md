# 0001. The goal is to stay in power; delivery is the second line

- **Status:** Accepted (design). Not yet built in the app.
- **Date:** 2026-11-01
- **Decided by:** the project owner, in conversation. Their answer to the proposal in
  [`../stay-in-power.md`](../stay-in-power.md) was "Yes to 1" (headline goal plus second line).
  This record was written by Claude from that conversation.

## Decision

The headline goal is **Power: win the election** at the end of the campaign. The Delivery Score
from [`../goal-and-score.md`](../goal-and-score.md) becomes a second line on the end screen,
called **Legacy**, so a report reads "Re-elected, legacy 94".

## Why

- It is a concept every player already knows, and it makes the six public groups matter.
- `framework.md` already allows it: *"Staying in power can be one scenario objective"*.
- Public support could not carry a goal with the old numbers (see decision 0003), but a
  recalibration fixes that, so the original reason for preferring a delivery score no longer holds.
- Delivery still matters as Legacy, because popular and effective are not the same thing.

## Consequences

- The end screen, the bloc map and the Legacy line are UI work still to do (phase 2 in `stay-in-power.md`).
- Ministers are currently indifferent to public support: scripted votes depend on the allocation
  and on their own belief, not on `constituency_support_ppm`. Making them care is a future lever.

## To change or revert

Docs only so far. To return to a delivery-only goal, keep `goal-and-score.md` as the plan and
drop the election line from the end screen. Nothing in the engine depends on this decision except
the election result added in decision 0002.
