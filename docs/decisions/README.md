# Decision log

One short record per decision that shapes the game or the engine, so any of them can be understood,
reverted or revisited later. Records are never deleted: a changed decision gets a new record, and the
old one is marked **Superseded by NNNN**.

| # | Decision | Status |
|---|---|---|
| [0001](0001-goal-stay-in-power.md) | The goal is to stay in power; delivery is the second line (Legacy) | Accepted (design) |
| [0002](0002-single-election-win-continues-loss-ends.md) | One election at the end; a win continues, a loss ends the game | Accepted |
| [0003](0003-recalibrate-the-support-model.md) | Recalibrate the public-support model (engine rules v2) | **Implemented** |
| [0004](0004-election-win-bar.md) | The election win bar (55% overall, 60% of voters) | **Accepted** (owner, 2026-11-01), implemented |
| [0005](0005-a-win-starts-a-second-term.md) | A win starts a second term of six quarters (option A); **R3**: a new allocation each term, added to anything unspent | Accepted; **implemented** by 0007 (build recommendation superseded) |
| [0006](0006-what-the-snapshot-exposes.md) | The snapshot exposes true per-group support and an election result; ~~the bars live in code~~ | **Implemented**; bars choice superseded by 0007 |
| [0007](0007-two-terms-in-one-campaign.md) | Two terms in one twelve-quarter campaign; explicit `begin_term`; the bars join the hashed scenario; audit resets per term; second term ends with its own election | **Implemented** (rules v3); known limitation: the second election is nearly free |

Each record has: the decision and who made it, the evidence, the consequences, and how to revert or change it.
Statuses: *Accepted*, *Provisional*, *Implemented*, *Superseded*, *Reverted*.

Supporting material: [`../stay-in-power.md`](../stay-in-power.md) (design),
[`../goal-and-score.md`](../goal-and-score.md) (the delivery score, now "Legacy"),
[`../../tools/support-model/`](../../tools/support-model/) (reproducible evidence).
