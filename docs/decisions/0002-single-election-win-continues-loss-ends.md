# 0002. One election at the end; a win continues, a loss ends the game

- **Status:** Accepted in principle. **One part is undefined: what "continues" means.**
- **Date:** 2026-11-01
- **Decided by:** the project owner, in conversation. Their words: *"one election at the end and
  if win then continues, if fail game end."* This record was written by Claude from that conversation.

## Decision

- There is **one election, after the last quarter (Q6) has resolved**. It counts the final
  quarter's work (the one-quarter response lag is settled first), so election night is not decided
  by a number that is still waiting to land.
- **Win:** the campaign continues.
- **Lose:** the game ends.
- There is **no mid-term poll at Q3 as a gate**. (A non-binding poll as a warning was offered and
  not chosen; it can be added later without changing this decision.)

## Open question: what does "continues" mean?

The engine has no second term. Today the campaign simply closes after Q6. Options, none chosen:

| Option | What it means | Notes |
|---|---|---|
| A. A second term | Another six quarters, carrying over support, lobbies, minister beliefs and any undelivered promises | Most natural fit. Needs a decision on whether the budget refills (the core rule today is "never refills") |
| B. Repeating terms | Re-election every six quarters until the player loses | Becomes an endless mode. Needs scaling, so later terms stay winnable |
| C. Marked won | The campaign ends as a win and a sandbox continues | Smallest change; a win is then only a label |

Until this is answered, the engine will report the election result at campaign close and **not**
start a second term. A loss is already consistent with "game ends".

## Consequences

- **Planned, not built:** the engine will record an election result when the campaign closes (stage B). Today nothing ends the game on a loss.
- The existing `closed` flag keeps meaning "the campaign is over"; a win will not reopen it until
  an option above is chosen.

## To change or revert

Change the answer here first, then the election rule in the scenario rules. Reverting to "no
election" means ignoring the election result; the support model itself is independent of it.
