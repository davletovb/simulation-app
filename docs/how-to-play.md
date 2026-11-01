# How Myth Maker works

This is the plain-language version of the game, for players and for anyone who
needs to remember what the game actually is. The authoritative rules are in
[`p0-case-contract.md`](p0-case-contract.md); this page explains them in familiar terms.

## The idea in one paragraph

You chair a cabinet of three ministers. The game is played in **terms of six quarters**, and each term ends with an
**election**; win it and another term begins, for as long as you keep winning. Each term gives you **100 credits that never
refill within the term**. Each quarter is one **turn**: you make a **spending plan**, the ministers **vote** on it, you
**approve or postpone** it, and then you **end the turn** and watch work get
delivered, late and in limited amounts.

## Familiar ideas, familiar names

| The game | Well-known idea | In the screen |
|---|---|---|
| A quarter | A turn (as in any turn-based strategy game, with an "End turn" button) | "Term 1 · quarter 2 of 6", **End quarter** |
| A term and its election | A term of office that ends at the ballot box | **Election night**, **Begin term 2** |
| 100 credits a term, no refill | A fixed budget / war chest | **Budget**, "Left to spend" |
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

- **The budget is for the term.** Each new term adds a fresh 100 credits to anything you
  did not spend. Credits turn into delivered work over time, and
  delivered work is spent for good. Credits you hold back, and credits promised to work
  that has not been delivered yet, can both be re-planned: a new plan may commit up to
  *unspent + undelivered* credits and **replaces** the undelivered work. Re-planning
  restarts the delivery delay for that work, so changing your mind late is costly.
- **Keep some credits unspent if you want more than one audit.** The first audit each term
  is free; later ones cost 2 credits from "Left to spend".
- **Work is never instant.** Staffing starts arriving one quarter after approval,
  Repairs two quarters after, Access immediately, each up to a per-quarter cap.
  Funding approved too late to arrive before Q2 2028 is flagged on the sliders.
- **Approving needs 2 of 3 supporting votes.** "Supports, with conditions" does not
  count. If fewer than 2 support, only Postpone is offered.
- **Ministers' reports can be optimistic.** The Progress screen shows what they
  *report*. An audit (free once per term, from the third quarter of the term; then 2 credits)
  checks the truth.
- **Ministers are scripted** in this build. No AI is called.

## The news

Each term brings a few **invented** headlines (they say so on screen). Good ones lift support. Bad ones take some of it back, and
**the higher a group stands, the more they take**, so a lead is never a safe lead; every term is heavier than the last. The first term
brings good news only.

- **Most headlines are announced a quarter ahead** in the **News** card: who it hits, about how many points of overall support it would
  cost, and its price to answer. **A few arrive unannounced** and cannot be answered.
- **Some headlines also stop work.** A flood can stop Repairs for two quarters; a strike ballot can stop Staffing. Nothing is lost: what
  could not be delivered waits. But work that slips past the election cannot help you win it, and the card says how many of *your*
  promised units that would be. A stoppage that touches work you have not promised costs you nothing.
- **Some send inspectors.** An audit or a spot check counts the work you have **promised and not yet finished** in the kinds of work it
  names, and takes a share of what that work would have earned each group. The share grows every term. A plan you have just made is
  mostly unfinished, so an inspection hurts most early in a term and not at all after a plan has been delivered; a player who has
  promised nothing owes nothing. The card says how many units would still be owed by then. A smaller plan has less to count, though
  re-planning restarts the delivery delays of work not yet delivered.
- **Some are real.** When a campaign starts, and again when each term begins, the game (if this computer is online) takes the newest
  stories from GOV.UK's Department for Education news and a published list of keyword rules decides whether each helps, hurts or does
  nothing. A real headline is marked **Real news**, says who published it and when, names the rule that moved support, and links to the
  story. It is announced a quarter ahead like the others and can be answered. At most two a term; term 1 still has no bad news. The
  Progress screen lists every real story used, including those that matched nothing and so change nothing. The stories are frozen
  into your game when they arrive, so a saved game never changes. Set `MYTH_REAL_NEWS=off` to play with invented headlines only.
- **A few bring credits.** A grant or a donation adds 10 or 15 credits. They arrive when the news lands, on top of anything you have
  left, and count in every budget figure. Planning them into a running programme means a new plan (with the same restart), so they are
  most useful as a **reserve** to answer a headline with, or carried into the next term.
- **Answer it** (at the start of a quarter, before you plan) takes 60% off the loss, and turns a stoppage into a slowdown, for credits:
  10 for every point of overall support it would have taken (mood, delayed work and unfinished work together). Those credits do not become delivered work, so you need a **reserve** of unspent credits, and they count against
  your Legacy. Do it for the big ones, or when your lead is thin.
- The Progress screen keeps the whole **news log**.

## Elections and further terms

After the sixth quarter of every term the public votes. You win with **at least 55% overall
support**. It does not matter which groups it comes from, so you can change course halfway and
win a different set of groups. The screen also says which groups back you (above 50%), as
information only; at the very start every group is exactly at 50% and none has taken a side.

- **Win:** the screen stops on **election night**. Beginning the next term is always your own
  move (the **Begin term N** button; the game never starts it for you). You get a fresh 100
  credits on top of anything unspent. Public support, ministers' memory, the two organisations
  and any unfinished work carry over, and the free audit comes back. Then it happens again:
  another six quarters and another election, for as long as you keep winning. The game never
  tells you in advance how many terms there will be, because that depends on you.
- **Lose:** the game is over (a *narrow defeat* at 52% or more, otherwise a *heavy defeat*).
- There is only a technical ceiling: a game records at most ten terms, and winning the tenth
  election ends it as *the longest tenure on record*.

The rules for all of this are recorded in [`decisions/`](decisions/README.md) (0008 is the one
that says terms continue while you win, and lists what is still weak about it).

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
