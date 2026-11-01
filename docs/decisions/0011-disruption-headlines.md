# 0011. Disruption headlines (news calendar, stage 2)

- **Status:** **Implemented** (stage 2, rules v7). Builds on [0010](0010-news-calendar.md); not yet on `main` until the owner says so.
- **Date:** 2026-11-01
- **Decided by:** the project owner asked for stage 2 of the news calendar ("push and start stage 2"): headlines that make one kind of
  work deliver less for a quarter or two. The details below (which headlines, how deep, how long, how it is priced) were chosen by
  Claude from the sketch in `tools/support-model/disruption_sim.py` and are meant to be tuned in play. Recorded by Claude.

## What is built

- **Seven of the twelve bad headlines also disrupt one kind of work.** The other five are only stories.

  | Headline | Kind of work | Delivers | For |
  |---|---|--:|---|
  | Teacher shortage hits the headlines | Staffing | 50% | 1 quarter |
  | Roof scare closes classrooms | Repairs | 50% | 1 quarter |
  | Broadband outage hits remote lessons | Access | 50% | 1 quarter |
  | Staff union ballots on strike action | Staffing | 0% (stopped) | 2 quarters |
  | Flooding halts repair crews | Repairs | 0% (stopped) | 2 quarters |
  | Volunteer groups stretched thin | Access | 25% | 2 quarters |
  | Building materials shortage slows work | Repairs | 50% | 2 quarters |

  The first three existed in stage 1 as stories; stage 2 gave them a cut and added the last four. The deck is now twelve bad and
  four good headlines, all invented and labelled.
- **Hybrid, not pure.** Each of these is a story that shakes opinion *and* cuts delivery. The mood part works as in 0010 (a share of
  the lead, growing each term, answerable). The delivery part slows one kind of work from the quarter the headline lands in, for its
  quarters. Nothing is lost: what could not be delivered waits, and the usual lags and capacities apply when work resumes. What
  matters is **work that slips past the election** cannot help win it.
- **Terms.** A cut starting in the last quarter of a term carries into the first quarter of the next, so a flood in quarter 6 still
  hurts in quarter 7. There is never a disruption in term 1 (no bad news there). Where two cuts meet on one kind of work, the worse
  one rules. While Access is cut, the small volunteer bonus it normally gets is lost too.
- **The quote.** A quarter ahead, the News card says what the cut would do to **this** plan: it replays the promised work to the end
  of the term under the cut, counts the units that would now arrive after the election, and takes away what those units would have
  earned from each group's support. That is added to the mood loss and shown as a single "about N points" figure, together with
  "N units would land after the election". A player with nothing promised in that kind of work is not hurt by its delivery and sees
  only the mood. The price of answering covers both parts.
- **Answering.** The same paid response as 0010: 60% off every loss, and a stoppage becomes a slowdown (0% delivered becomes 60%; 50%
  becomes 80%; 25% becomes 70%). Once answered, the card shows what the answer bought (the softened figures). The price follows the
  whole expected damage, mood plus delay, at 10 credits a point.
- **Saves:** rules v7. The deck changed, so every calendar changed, so games saved under v6 cannot be opened (the start screen explains this).

## Why hybrid

A cut on its own is a weak challenge. `tools/support-model/disruption_sim.py` plays the Balanced plan, approved in quarter 1 of a
term, and cuts one kind of work from each start quarter; the table is the change in overall support at the election.

| Cut | Staffing | Repairs | Access |
|---|--:|--:|--:|
| Halved for 1 quarter | 0.00 | up to -0.24 | 0.00 |
| Stopped for 1 quarter | up to -0.13 | up to -0.48 | 0.00 |
| Halved for 2 quarters | up to -0.13 | up to -0.48 | 0.00 |
| Stopped for 2 quarters | up to **-0.90** | up to **-0.96** | 0.00 |

Under a point at worst, and often nothing: the plan's lead is not made of the last quarter's units, and Access is delivered early. A
player with a normal margin would shrug these off, so the cut needed the mood part to be a real test. As a hybrid, a stoppage
costs a point or two in all (the flood in the cross-check below: 1.2 points) and also pushes a plan that was already late over the line.
The worst case in the sketch is a two-quarter stoppage that starts anywhere from the second to the fifth quarter, because the
work it holds back is still owed at the election; the announcement a quarter ahead and a reserve to answer it are what blunt it.

## Checked against an independent implementation

`tools/support-model/news_model.py` repeats the new rules (the cut, the carry across a term boundary, the projected delivery, the
softened keep) from their description and reads the deck from `authored.json`. The engine's election-night test for the campaign
named `dz7` (through the HTTP command path) matches it to the ppm, including:

- term 1 won at 58.2% on the Balanced plan, with its two good headlines;
- term 2 plays 50/35/0 with a 15-credit reserve; a flood is announced for quarter 11, stopping Repairs for two quarters. The card
  says 15 of the repair units still to come would land after the election, and quotes the story and the delay together as
  minus 1.22 points (the figure is truncated toward zero: -12,208 ppm, not -12,209);
- the 13-credit answer turns the stoppage into a 60% slowdown: 32 of the 35 repair units arrive (10, 10, then 6 and 6), and the card,
  once paid, shows 3 units landing after the election instead of 15;
- term 2 is won at 60.2%; every credit is still in exactly one of seven accounts (they sum to the 200 granted).

A drift test compares the deck, its `disrupts` data and the constants in `authored.json` with the engine's.

## Observations worth knowing

- **Answering needs a reserve.** As in 0010: the useful reserve is 15 to 25 credits. The price now includes the delay a disruption
  would cause, so the "units would land after the election" line on the card is the one to watch.
- **A plan with work still owed feels it most.** A cut costs nothing to a player who has nothing promised in that kind of work, and
  most to one whose promised units were still to come at the election. The quote says which situation the player is in.
- **A pure cut is rarely decisive.** If a harder, more mechanical challenge is wanted later, the knob is the hybrid's mood part
  (`effect` per headline) and the depth/length of the cut, not a new rule.
- **Surprises** (a third of headlines) still cannot be answered. A surprise that disrupts lands without warning, so the quote cannot
  be shown beforehand.

## A defect found afterwards

The web app refused the quote for a disruption when it passed 4 points for one group, which froze the screen on "Saving…". Fixed with
stage 3; see [0012](0012-scrutiny-and-windfalls.md).

## Not built yet

- **Scrutiny** (unfinished promises cost support) and **windfalls** (extra credits to spend): built in stage 3, see [0012](0012-scrutiny-and-windfalls.md).
- **Real-world news** and **groups with opposed interests**: built in stages 4 and 5, see [0013](0013-real-world-news.md) and [0014](0014-opposed-interests.md).

## To change or revert

- **Make a cut deeper, shallower, longer or shorter:** `keep_pct` and `quarters` on the headline in `crates/domain/src/news.rs`
  and the same values in `authored.json` (a test keeps them equal). Re-run `disruption_sim.py` and `news_model.py --check`.
- **Add a disruption headline:** add it to both decks in the same order (the drift test checks) and bump `RULES_VERSION`.
- **Turn all disruptions off, keeping the stories:** remove `disrupts` from the headlines (no code change). Nothing else depends on it.
- **Revert fully:** `git revert` the stage 2 commits; the snapshot fields (`disruption`, `delayed_units`) are additive and the web
  checks tolerate their absence.
