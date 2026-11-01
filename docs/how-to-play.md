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
| 1 | **Make a plan** | Pick a ready-made plan or set your own split, then propose it. From Q2 you may instead **keep the current programme** (no vote). | `QuarterOpen` | `propose` (or `continue_quarter`) |
| 2 | **Cabinet reacts** | Hear each minister's first reaction | `FirstPositionsFrozen` | `advance_slice` |
| 3 | **Revise** | Change your plan **once**, or keep it as it is | `AmendmentWindow` | `amend` then lock, or `advance_slice` |
| 4 | **Final vote** | Hold the vote on the locked plan | `FinalPositionsFrozen` | `advance_slice` |
| 5 | **You decide** | **Approve** (needs 2 of 3 ministers) or **Postpone** | `FinalPositionsFrozen`, 3 votes in | `decide` |
| 6 | **End quarter** | Time moves on: work arrives, organisations react | `Resolved` (or a kept programme) | `advance_slice` (or `finish_continuation`) |

Step 3's "Submit revised plan" sends the amendment and then locks the plan
automatically; the engine treats those as two committed actions, the player
should not have to.

### Rules that are easy to miss

- **The budget is for the whole game.** Credits you commit are gone. Credits you hold
  back can fund a later plan or an audit. If you commit all 100 in Q1, there is
  nothing new to fund in Q2–Q6; you can only keep the programme and end quarters.
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

## Known gaps (not changed here)

These need a design or backend decision rather than a UI fix:

1. **There is no goal or score.** The case contract defines no win condition; the game
   ends after Q6 with a tally of decisions and credits delivered. A familiar game needs
   something to aim at (e.g. "deliver X by Q2 2028 while keeping public support above Y").
2. **Audit results are not shown in the game.** They are private by design and can only be
   read with `myth-report` after stopping the service. The player pays credits and sees
   nothing, which breaks the "check the truth" loop the game is built around.
3. **The default plan spends the whole budget in Q1**, so later quarters have no decisions
   to make. Consider a smaller opening budget, a per-quarter allowance, or a reserve
   mechanic.
4. **`advance_slice` is overloaded in the API.** The UI now only sends it where it is
   safe, but separate commands per phase (and a clear rejection when `decide` has fewer
   than 2 supporting votes, which today is accepted and then fails with
   "Offline step failed; inspect trial") would remove the trap at the source.
5. **The delivery forecast is Q1-only.** The service forecasts a plan approved at the
   start of the campaign; later quarters show per-area start dates instead.
