# Decision log

One short record per decision that shapes the game or the engine, so any of them can be understood,
reverted or revisited later. Records are never deleted: a changed decision gets a new record, and the
old one is marked **Superseded by NNNN**.

| # | Decision | Status |
|---|---|---|
| [0001](0001-goal-stay-in-power.md) | The goal is to stay in power; delivery is the second line (Legacy) | Accepted (design) |
| [0002](0002-single-election-win-continues-loss-ends.md) | One election at the end; a win continues, a loss ends the game | Accepted |
| [0003](0003-recalibrate-the-support-model.md) | Recalibrate the public-support model (engine rules v2) | **Implemented** |
| [0004](0004-election-win-bar.md) | The election win bar (55% overall, 60% of voters) | **Provisional**, not implemented |
| [0005](0005-a-win-starts-a-second-term.md) | A win starts a second term of six quarters (option A) | Accepted; **term-2 budget open** |

Each record has: the decision and who made it, the evidence, the consequences, and how to revert or change it.
Statuses: *Accepted*, *Provisional*, *Implemented*, *Superseded*, *Reverted*.

Supporting material: [`../stay-in-power.md`](../stay-in-power.md) (design),
[`../goal-and-score.md`](../goal-and-score.md) (the delivery score, now "Legacy"),
[`../../tools/support-model/`](../../tools/support-model/) (reproducible evidence).
