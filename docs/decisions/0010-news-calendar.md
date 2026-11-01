# 0010. The news calendar

- **Status:** **Implemented** (stage 1, rules v6). Addresses the weakness recorded in [0008](0008-terms-continue-while-the-player-wins.md)
  and [0009](0009-overall-support-is-the-only-bar.md): after one win a player could do nothing and keep winning.
- **Date:** 2026-11-01
- **Decided by:** the project owner. They chose the news calendar over the other two fixes (a rising bar, support that drifts back
  toward 50%), said it must work in any term from the first and add challenges, and answered the five questions in
  [`../news-calendar.md`](../news-calendar.md). Recorded by Claude.

## What the owner decided (2026-11-01)

1. **Some surprises.** Most headlines are announced a quarter ahead; some arrive unannounced.
2. **A paid response in the first version.** Yes.
3. **Term 1 has no bad news.**
4. **Increasingly harder with more terms.** Not a fixed ceiling of difficulty; it keeps rising (to the engine's own cap).
5. **Invented news, plainly labelled, kept separate from the real-source items; real-world news will be added too, later.**

## What is built

- **A calendar for every campaign and every term.** Each term has headlines in its quarters 2 to 6: two in term 1 (good news
  only), four in every later term (30% good). A third are surprises. The calendar is a pure function of the campaign's identity and
  the term (SplitMix64, integers only), so a replay and a what-if branch see exactly the same headlines, and no list of 60 quarters
  had to be written. The deck was eight bad and four good headlines in stage 1 (four more bad ones arrived in stage 2), all invented (`authored.json` has the words; `news.rs` the numbers).
- **Bad news takes a share of what you have built.** For each group: exposure to that headline times a small base plus a share of the
  group's lead over 50%. The base grows 50% a term (to four times) and the share grows 8 points a term (from 40% to 90%) after term 2,
  so a big lead is never a safe lead and every term is heavier than the last. One headline never takes more than 4 points from one
  group (the engine's existing cap). Good news is a flat gift.
- **A paid response.** An announced bad headline can be answered at the start of a quarter, before planning, in the quarter it is
  announced or the next: it takes 60% off every loss and costs **10 credits for every point of overall support it would take**
  (rounded up, at least one), quoted from where support stands then. Those credits are a new treasury account (`relief_spent`), so
  every credit is still in exactly one place; they do not become delivered work, so they count against Legacy.
- **The screens.** A **News** card on the Play screen ("Coming up", with who it hits, about how many points it would cost, the price and
  an **Answer it** button that says why it is unavailable; "Just in" for what landed). A **news log** on the Progress screen. A
  budget line and a Legacy chip for credits spent answering. A paragraph in the briefing.
- **Saves:** rules v6, so games saved under v5 cannot be opened (the start screen explains this).

## Why these numbers

`tools/support-model/calendar_sim.py` (a sketch on the replica) chose them. Share of games still going after each election:

| Player | T1 | T2 | T3 | T4 | T6 | T8 | T10 |
|---|--:|--:|--:|--:|--:|--:|--:|
| Plays once, then nothing | 100% | 57% | 19% | 6% | 0% | 0% | 0% |
| Plays Balanced every term | 100% | 100% | 100% | 98% | 94% | 87% | 80% |
| Same, and answers headlines when the lead is thin | 100% | 100% | 100% | 100% | 99% | 98% | 97% |

A player who stops playing is out within two or three terms. One who keeps delivering is squeezed harder every term; one who also
protects votes at the right moment lasts longest. A response priced at a flat 10 credits was far too cheap (with it nobody ever lost),
which is why the price follows the damage. The ramp is two numbers per term and is meant to be tuned in play.

## Checked against an independent implementation

`tools/support-model/news_model.py` repeats the schedule, the shocks, the response price and the arithmetic from their description and
reads the deck from `authored.json`. The engine's election-night test for a real campaign (through the HTTP command path) matches it to
the ppm: term 1's two good headlines, the 13-credit quote, the softened hit (60% less) and a hands-off second term that loses at 52.97%.
The schedule for a fixed seed is pinned in `news.rs` from the same port, and `news_model.py --check` repeats every pinned value.
In a real browser the replica's predictions held: Balanced repeated won two terms in a row (58.2%, then 58.0%, no longer compounding),
and doing nothing lost the second term at 54.4%.

## Observations worth knowing

- **Answering needs a reserve.** A player who commits all 100 credits to the Balanced plan has nothing to answer with (the button says
  "You have 0 credits left to spend and an answer costs 14"). Real prices for serious headlines were 14 to 21 credits, so a useful
  reserve is 15 to 25 credits, a fifth of a term's allocation. That is the intended trade, but it may be steeper than a first-time
  player expects; the price per point (10 credits) is the knob.
- **The first term is easier than before.** Its two good headlines add about 1 to 3 points: the best opening plan clears the bar by
  more, a plan two quarters late can now win (it did, at 56.7%, in the one campaign tried), and a player who never spends ends a narrow
  defeat (52.2% in the campaign tried) instead of exactly 50%. Which side of the bar a marginal first term lands on now depends on that
  campaign's calendar.
- **Surprises cannot be answered** and are the part of the calendar that is luck. A third of headlines.

## Not built yet (and why)

- **Disruption headlines** (one kind of work delivers less for a quarter): **built in stage 2, see [0011](0011-disruption-headlines.md).**
  **Scrutiny** (unfinished promises cost support) and **windfalls** (extra credits): **built in stage 3, see [0012](0012-scrutiny-and-windfalls.md).**
- **Real-world news.** The data and screens carry a `source` field (`invented` today; `real` is accepted and must be labelled), so a
  real lane can be added without reshaping anything. It needs its own decision: whether such items only inform or also move support,
  and how that stays reproducible for a saved game.
- **Groups with opposed interests.** Headlines hit groups in different amounts, but everything still moves everyone the same way.

## To change or revert

- **Make it harder or easier:** the numbers in `crates/domain/src/news.rs` (`BASE_*`, `SHARE_*`, `RELIEF_*`, `HEADLINES_PER_TERM`,
  `GOOD_PERCENT`, `SURPRISE_ONE_IN`) and the same numbers in `authored.json` (a test keeps them equal); re-run the sketch first;
  regenerate the manifest; bump `RULES_VERSION`.
- **Add or reword headlines:** the deck in `news.rs` and `authored.json` (same order, same length: a test checks both).
- **Switch it off for a campaign:** leave `Rules.news_seed` as `None` (what the sensitivity report does, which is why that report is
  byte-identical).
