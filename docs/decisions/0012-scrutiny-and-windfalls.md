# 0012. Scrutiny and windfalls (news calendar, stage 3)

- **Status:** **Implemented** (stage 3, rules v8). Builds on [0010](0010-news-calendar.md) and [0011](0011-disruption-headlines.md). Not on
  `main` until the owner says so.
- **Date:** 2026-11-01
- **Decided by:** the project owner asked to go ahead with the next stage of the news calendar, which the plan in
  [`../news-calendar.md`](../news-calendar.md) lists as scrutiny (unfinished promises cost support) and windfalls (extra credits). What
  exactly those do, how hard, and how they are priced were chosen by Claude from the sketch in `tools/support-model/scrutiny_sim.py`
  and are meant to be tuned in play. Recorded by Claude.

## What is built

- **Three inspection headlines (bad news).** *Watchdog audits teacher promises* (counts Staffing), *Watchdog checks school building
  promises* (counts Repairs) and *Inspectors announce a spot check on every promise* (counts all three). Each is a hybrid, like a
  disruption: a story that shakes opinion as in 0010, **and** a count of the promises the player has made and not yet kept.
- **What the count takes.** When an inspection lands (the end of its quarter, after that quarter's delivery) the inspectors take the
  work promised in the named kinds of work and still undelivered, work out what delivering it would have earned each group, and take
  **50% of that in term 2, growing 15 points a term to 200%**. A group the work would have cost more than it gave loses nothing, and
  no group loses more than 4 points to one headline (the engine's existing cap). More than 100% because a broken promise is punished
  harder than it would have been rewarded.
- **Who it touches.** A player who has promised nothing owes nothing, so an inspection can only hurt someone who is *doing* something.
  That is deliberate: the mood part of the story keeps the pressure on a player who does nothing (see below), and the count adds
  pressure on a busy one. It is largest early in a term (a plan approved in quarter 1 is mostly unfinished in quarter 2 or 3) and nil
  by the end of a plan that finished.
- **The quote.** A quarter ahead the card says what would still be owed when the inspectors arrive, if nothing changes
  ("15 units of Repairs would still be owed by then"), and the loss in points, counted into the same "about N points" figure and the
  same price to answer. A smaller plan has less to count, and the card says so.
- **Answering.** The same paid response as 0010: 60% off every loss, mood and count together, at 10 credits a point.
- **Two windfalls (good news).** *Government releases an education grant* (+15 credits) and *Charity donates to the school fund* (+10).
  They are credits, not mood, so the card says "Brings 15 credits to spend, on top of anything you have left" and shows no
  "Helps". They arrive in the treasury when they land (a ledger grant, the only way credits enter the game) and are in every
  budget figure from then on. A windfall can come in any term, including the first, announced a quarter ahead or as a surprise.
- **The screens.** The News card (inspection line with a magnifier; windfall line; "Just in" with what was counted or what arrived),
  the news log on the Progress screen, a briefing paragraph, the budget note.
- **Saves:** rules v8, so games saved under v7 cannot be opened (the start screen explains this).

## What had to change for credits to arrive mid-game

Until now credits came only as a 100-credit allocation at the start of each term, and the treasury insisted that the total granted was
a whole number of allocations. A windfall is not 100, so:

- the "whole terms" check on the treasury is gone; what remains is that the total granted is at least the opening allocation, at most
  `MAX_CREDITS`, and **equal to the sum of the seven accounts**, so every credit is still in exactly one place;
- `MAX_CREDITS` is `100 x 10 terms + MAX_WINDFALL_TOTAL`, where the latter is 15 credits for every headline of every term (1,570), so
  a game of windfalls and ten terms still fits (a test fills it);
- the web validators carry the same bound.

## Why scrutiny is a hybrid, and what the sketch showed

`tools/support-model/scrutiny_sim.py` plays the same simple players as 0010 (`idle` plays once, `repeat` plays Balanced every term,
`smart` is `repeat` plus the paid response), counting the share of games still going after each election.

The first attempt added scrutiny as three *pure* headlines (no mood part). It made the game **easier**, not harder: `repeat` survived
91% of the way to term 10 against 80% before. Two reasons: an idle player owes nothing, so a pure inspection does nothing to them;
and every pure inspection drawn is a harsher mood headline not drawn, so the deck got softer. Giving the three a mood part (20 to 30%
exposure, a little under the others') and a count on top fixes both.

| Player | Stages 1-2 (T2, T3, T4 / T10) | With stage 3 (T2, T3, T4 / T10) |
|---|---|---|
| Plays once, then nothing | 57%, 19%, 6% / 0% | 44%, 11%, 3% / 0% |
| Plays Balanced every term | 100%, 100%, 98% / 80% | 100%, 100%, 96% / 65% |
| Same, and answers when it matters | 100%, 100%, 100% / 97% | 100%, 100%, 100% / 92% |

A player who stops playing is out a little sooner, one who keeps delivering is squeezed harder, and one who also protects votes at
the right moment still lasts. Windfalls account for part of that: without them the same table reads 64%, 24%, 7% / 0% (idle), 70% at
term 10 (repeat) and 95% (smart), because a windfall replaces a good-news story that was worth a point or two and `repeat` can only
turn its credits into a slightly bigger plan next term. The choice of 50% for the count is not a sensitive one (30% and 80% give
66% and 62% at term 10): the mood part of the hybrid sets most of the pressure, and the count is what makes a busy player choose.

## Checked against an independent implementation

`tools/support-model/news_model.py` repeats the schedule for the new deck, the count (a share of what owed work would have earned,
floored at zero, capped, then softened by an answer) and the windfall credits from their description. The engine's test for the
campaign named `sc156` (through the HTTP command path) matches it to the ppm:

- term 1 has an education grant in quarter 4: 15 credits arrive and are the whole budget; beginning term 2 makes it 115;
- a plan of 50/35/0 keeps 30 credits back; a quarter ahead the audit of building promises is quoted at -10,115 ppm for a price of 11
  credits, counting 15 repair units still owed after quarter 10; the quote moves as support moves (-10,864 ppm a quarter later);
- the spot check on every promise is answered for 10 credits: it counts 2 staffing and 5 repair units and lands at -4,118 ppm of
  overall support instead of the -9,947 ppm it was quoted at;
- the charity donation arrives in the last quarter; term 2 is won at 58.9%; the seven accounts add up to 225 (200 allocated and
  25 in windfalls).

The schedule for a fixed seed is pinned in `news.rs` from the same port. The flood test of 0011 moved to the campaign `dz4827`
because every calendar changed with the deck. In a real browser the same flow showed the same numbers and wording.

## Observations worth knowing

- **Withdrawing a promise is the real lever, and it is not free.** Re-planning smaller before an inspection cuts the count, but a new
  plan restarts the delivery delays of anything not yet delivered and gives up the support that work would have earned. The sketch
  cannot model re-planning, so how often it pays is not measured; the answer is probably "when the count is large and the inspection
  is near".
- **A windfall is best kept or carried.** Adding credits to a running plan means a new plan, with the same restart. They are most
  useful as a reserve to answer a headline with, or as part of next term's allocation, and the briefing and card say what they are.
- **Inspections are bad luck early in a term and harmless late.** Which quarter one lands in is drawn with the calendar; the
  announcement a quarter ahead is the only warning, and a surprise inspection cannot be answered.

## A defect in stage 2, found and fixed here

Stage 3's browser run froze on "Saving…" when a flood was announced. The cause was in stage 2 (0011), already on `main`: the web
app refused a live update whose announced headline would take more than 4 points from one group, but the quote for a disruption
is the story (capped at 4 points) **plus** what the delayed work would have earned (up to 4 more), so a quote can reach 8. Quotes
of -42,264 and -48,200 ppm were seen. The refused update was never applied, so the screen waited for it for good. It first showed
as a one-off timeout in a stage 2 multi-term browser run (a quote of -48,200 on one group, only understood now), and then every time
on the campaign `dz4827`; the stage 2 runs that passed had smaller quotes. Fixed: the web bound for a quote is
twice the cap (a landing is still capped at one), with a test built from the two real quotes that fails without the fix, and an
engine test that sweeps 40 calendars and checks every quote and landing against both bounds. If stage 2 is wanted on its own
without stage 3, the part to take is the `quoteOk` bound in `apps/web/src/committedReducer.mjs` and its test.

## Not built yet

- **Real-world news**: built in stage 4, see [0013](0013-real-world-news.md). **Groups with opposed interests**: as in 0010.
- **Polls and a scrutiny of what ministers report** (the audit result shown on screen): unchanged.

## To change or revert

- **Make inspections harder or easier:** `SCRUTINY_PCT`, `SCRUTINY_STEP_PCT`, `SCRUTINY_CAP_PCT` in `crates/domain/src/news.rs` and the
  same three numbers under `news_calendar.scrutiny` in `authored.json` (a test keeps them equal); re-run `scrutiny_sim.py` and
  `news_model.py --check`; regenerate the manifest; bump `RULES_VERSION`.
- **Change what counts:** the `families` an inspection names, in both places (same order, a test checks).
- **Change a windfall:** the `grant_credits` on the headline in both places, never above `MAX_WINDFALL_CREDITS`.
- **Remove windfalls:** delete the two good headlines from both decks; `MAX_WINDFALL_TOTAL` can then go back to zero and the treasury's
  upper bound with it.
- **Revert fully:** `git revert` the stage 3 commits; the snapshot fields (`scrutiny`, `grant_credits`) are additive for a browser that
  tolerates their absence, but this one refuses an older server's snapshot on purpose.
