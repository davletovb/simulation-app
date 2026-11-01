# How Myth Maker works

This is the plain-language version of the game, for players and for anyone who
needs to remember what the game actually is. The authoritative rules are in
[`p0-case-contract.md`](p0-case-contract.md); this page explains them in familiar terms.

## The idea in one paragraph

You chair a cabinet of three ministers. You have **one budget of 100 credits for
the whole game; it never refills**. The game lasts **six quarters**. Each quarter is
one **turn**: you make a **spending plan**, the ministers **vote** on it, you
**approve or postpone** it, and then you **end the turn** and watch work get
delivered, late and in limited amounts.

## Familiar ideas, familiar names

| The game | Well-known idea | In the screen |
|---|---|---|
| A quarter | A turn (as in any turn-based strategy game, with an "End turn" button) | "Quarter 2 of 6", **End quarter** |
| 100 credits, no refill | A fixed budget / war chest | **Budget**, "Left to spend" |
| Staffing / Repairs / Access | Budget lines you allocate money to | The three sliders |
| The board seats | A cabinet of ministers with portfolios | **Cabinet**: Finance, Education, Infrastructure |
| Proposal / package | A spending plan or bill | **Spending plan** |
| Ratify / defer | Approve / postpone | **Approve**, **Postpone** |
| Audit | Check the real numbers against ministers' own reports | **Check the real numbers (audit)** |
| Fork | "What if?" / branch of a saved game | **Create a what-if branch** |

## A quarter, in six steps

The same six steps every quarter. The screen always shows which one you are on and
offers exactly one thing to do.

| # | Step (what the player sees) | What you do | Engine phase | Command sent |
|---|---|---|---|---|
| 1 | **Make a plan** | Pick a ready-made plan or set your own split, then propose it. From Q2 a new plan **replaces** work not yet delivered, or you can **keep the current programme** (no vote). | `QuarterOpen` | `propose` (or `continue_quarter`) |
| 2 | **Cabinet reacts** | Hear each minister's first reaction | `FirstPositionsFrozen` | `advance_slice` |
| 3 | **Revise** | Change your plan **once**, or keep it as it is | `AmendmentWindow` | `amend` then lock, or `advance_slice` |
| 4 | **Final vote** | Hold the vote on the locked plan | `FinalPositionsFrozen` | `advance_slice` |
| 5 | **You decide** | **Approve** (needs 2 of 3 ministers) or **Postpone** | `FinalPositionsFrozen`, 3 votes in | `decide` |
| 6 | **End quarter** | Time moves on: work arrives, organisations react | `Resolved` (or a kept programme) | `advance_slice` (or `finish_continuation`) |

Step 3's "Submit revised plan" sends the amendment and then locks the plan
automatically; the engine treats those as two committed actions, the player
should not have to.

### Rules that are easy to miss

- **The budget is for the whole game.** Credits turn into delivered work over time, and
  delivered work is spent for good. Credits you hold back, and credits promised to work
  that has not been delivered yet, can both be re-planned: a new plan may commit up to
  *unspent + undelivered* credits and **replaces** the undelivered work. Re-planning
  restarts the delivery delay for that work, so changing your mind late is costly.
- **Keep some credits unspent if you want more than one audit.** The first audit is free;
  later ones cost 2 credits from "Left to spend".
- **Work is never instant.** Staffing starts arriving one quarter after approval,
  Repairs two quarters after, Access immediately, each up to a per-quarter cap.
  Funding approved too late to arrive before Q2 2028 is flagged on the sliders.
- **Approving needs 2 of 3 supporting votes.** "Supports, with conditions" does not
  count. If fewer than 2 support, only Postpone is offered.
- **Ministers' reports can be optimistic.** The Progress screen shows what they
  *report*. An audit (free once, from Q3; then 2 credits) checks the truth.
- **Ministers are scripted** in this build. No AI is called.

## What changed in the interface, and why

The previous interface worked, but the player had to understand the engine to use it:

- One button, **"Continue scripted phase"**, meant six different things depending on a
  hidden phase. In two of those phases it *made the player's own move* (it proposed
  a default plan, or approved/postponed for them).
- The plan builder sat *below* the decision controls, and its submit button scrolled
  you back up.
- The header said "Quarter: Not started" and "not connected" while the game was running.
- Engine words (`FinalPositionsFrozen`, journal cursor, command IDs, "ratify") were
  the main vocabulary.

Now there is one guided screen, a six-step tracker, and one clearly named action per
step. Engineering details live in **Technical details** at the bottom of the Play screen.
The step logic is in one tested module, `apps/web/src/turn.mjs`.

## Known gaps and what to do about them

See [`goal-and-score.md`](goal-and-score.md) for the full analysis: a proposed goal and
score, why the scripted pilot is currently "solved" by the Balanced plan, the audit
visibility problem, and the API traps (an overloaded `advance_slice`, and commands that
are accepted and then fail with "Offline step failed; inspect trial").
