# 0005. A win starts a second term (option A)

- **Status:** Accepted and **implemented** by [0007](0007-two-terms-in-one-campaign.md). Budget decided: R3. On 2026-11-01 the owner also
  confirmed that **term 2 ends with its own election** and that **the free audit resets each term**. The recommended build
  (two chained campaigns) was **not** used: see 0007 for why one twelve-quarter campaign was built instead.
- **Date:** 2026-11-01
- **Decided by:** the project owner, in conversation. Asked what "if win then continues"
  (decision 0002) means, with three options, they answered **"Option A"**. Recorded by Claude.
- **Resolves:** the open question in [0002](0002-single-election-win-continues-loss-ends.md). The rest of
  0002 stands.

## Decision

Winning the election starts **a second term of six quarters**. The second term **carries over**:

- **public support** in each of the six groups, as it stood on election night (the last quarter's effect
  counted);
- the **lobbies'** balances (School Staff Association influence, Community Access Network volunteer days);
- each **minister's belief and relationship** with the player;
- **unfinished promises**: work approved but not yet delivered stays promised, as the current programme.

A loss still ends the game (0002).

## My reading of what A implies (please correct me)

The second term **ends with a second election** and then the game ends: a win there is the final victory,
a loss ends the game. Without a second election the second term would have no stakes. This was not
stated; I am assuming it.

## Sub-decision 1: does the budget refill for the second term? **Decided: R3** (2026-11-01)

The owner chose **R3**: a fresh allocation each term (starting at 100, set in the scenario) is **added** to anything
unspent, and unfinished promises stay promised. "Never refills" now means **within a term**. Not built yet; the
briefing line "one budget for the whole game, it never refills" stays true for a single term and must be
changed **when the second term is built**, not before (it would promise something that does not exist yet).

The analysis that led to it:

This is the one that changes how the second term plays, so it needs an answer before anything is built.
Real plays show why "no refill" does not work:

| If the player did this in term 1 | Credits left to plan with in term 2, with no refill |
|---|---|
| Balanced in Q1 (delivered 100 of 100) | **0 unspent, 0 undelivered: nothing to decide for six quarters** |
| Balanced a quarter late (delivered 88) | 12 credits of work still in flight, nothing new |
| Held back credits (unspent) | whatever they held back |

| Option | What happens | Verdict |
|---|---|---|
| **R1. No refill** | Term 2 starts with what is left | A player who did well gets a dead second term, and the best-placed player is the one who delivered *least*. Works against the goal |
| **R2. Reset to 100** | A full new budget | Simple, but term 2 repeats term 1's decisions, and it contradicts "never refills" without saying so |
| **R3. A new allocation each term (recommended)** | A fresh allocation (start at 100, set in the scenario) is **added** to anything unspent; unfinished promises stay promised | "Never refills **within a term**", with a spending review at each term. Doing well is not punished; hoarding still costs you delivery and votes |

**Recommendation was R3, and R3 was chosen.**

## Open sub-decision 2 (smaller): the audit allowance (still open)

The first audit is free from Q3. Does that reset each term? I would reset it per term. Not blocking.

## How it could be built (my recommendation, not decided, not prototyped)

Two ways, with different cost:

1. **One campaign of twelve quarters.** The engine assumes six: `Quarter` only accepts 1 to 6, the shock
   calendar is a 6×6 array, `SIX_QUARTERS` appears in 13 places across 5 files, the contract defines the
   case as six quarterly periods, and every pinned fixture and replay would change. The action journal is
   bounded at 512 actions; twelve quarters need roughly twice today's number, which fits, but it is a
   second thing to check.
2. **Two linked campaigns ("terms"), recommended.** When the player wins, the engine starts a *successor*
   campaign whose starting state is the predecessor's verified final state (support, lobbies, beliefs,
   unfinished promises), bound to the predecessor by its hash so replay can check the chain. The
   six-quarter rules stay exactly as they are. It needs a way to start a campaign from a carried state,
   and a calendar for the second term (Q3 2028 to Q4 2029).

Either way the election result (stage B) comes first, and it does not depend on this choice.

## Consequences

- The game becomes at most twelve quarters and two elections. The headline "Power" goal now spans two terms;
  **Legacy** (the delivery score) should be reported per term.
- `closed` keeps meaning "this campaign is over". A win must not reopen it; it creates a successor.
- Anything that pins six quarters in the UI (the quarter pips, `OFFLINE_PILOT.total_quarters`) must read the
  term's length from the campaign.

## To change or revert

Docs only so far. Change the answer here and mark this record superseded. Options B (repeating terms) and
C (a label only) from 0002 remain available; B is a natural extension of the chained-terms design.
