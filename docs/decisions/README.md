# Decision log

One short record per decision that shapes the game or the engine, so any of them can be understood,
reverted or revisited later. Records are never deleted: a changed decision gets a new record, and the
old one is marked **Superseded by NNNN**.

| # | Decision | Status |
|---|---|---|
| [0001](0001-goal-stay-in-power.md) | The goal is to stay in power; delivery is the second line (Legacy) | Accepted (design) |
| [0002](0002-single-election-win-continues-loss-ends.md) | One election at the end; a win continues, a loss ends the game | Accepted |
| [0003](0003-recalibrate-the-support-model.md) | Recalibrate the public-support model (engine rules v2) | **Implemented** |
| [0004](0004-election-win-bar.md) | The election win bar (55% overall; its second bar, 60% of voters, was dropped by 0009) | **Accepted** (owner, 2026-11-01), implemented |
| [0005](0005-a-win-starts-a-second-term.md) | A win starts another term of six quarters (option A); **R3**: a new allocation each term, added to anything unspent | Accepted; **implemented** by 0007 and 0008 (build recommendation superseded) |
| [0006](0006-what-the-snapshot-exposes.md) | The snapshot exposes true per-group support and an election result; ~~the bars live in code~~ | **Implemented**; bars choice superseded by 0007 |
| [0007](0007-two-terms-in-one-campaign.md) | Terms in one campaign; explicit `begin_term`; the bars join the hashed scenario; audit resets per term. (Its "exactly two terms" part was a misreading.) | **Implemented**; two-term part superseded by 0008 |
| [0008](0008-terms-continue-while-the-player-wins.md) | Terms continue for as long as the player keeps winning; nothing announces a number of terms; technical ceiling of 10 | **Implemented** (rules v4); known weakness: winning needs no effort after the first win |
| [0009](0009-overall-support-is-the-only-bar.md) | Overall support is the only election bar; the groups behind you are shown as information | **Implemented** (rules v5); evidence: the second bar never decided a result |
| [0010](0010-news-calendar.md) | The news calendar: invented headlines each term, some announced a quarter ahead and some surprises, bad news that takes a share of the lead and grows each term, a paid response; term 1 has no bad news | **Implemented** (stage 1, rules v6); stage 2 is 0011; scrutiny, windfalls and real-world news still to come |
| [0011](0011-disruption-headlines.md) | Disruption headlines: seven bad headlines also slow or stop one kind of work for one or two quarters (a hybrid of mood and delivery); the quote counts work pushed past the election; answering softens both | **Implemented** (stage 2, rules v7); scrutiny, windfalls and real-world news still to come |

Each record has: the decision and who made it, the evidence, the consequences, and how to revert or change it.
Statuses: *Accepted*, *Provisional*, *Implemented*, *Superseded*, *Reverted*.

Supporting material: [`../stay-in-power.md`](../stay-in-power.md) (design),
[`../goal-and-score.md`](../goal-and-score.md) (the delivery score, now "Legacy"),
[`../../tools/support-model/`](../../tools/support-model/) (reproducible evidence).
