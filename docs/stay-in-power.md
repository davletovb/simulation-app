# "Stay in power" as the goal: design proposal

**Status (2026-11-01): decided and built. See [`decisions/`](decisions/README.md).** The goal is
accepted (0001), the election rule is accepted (0002), the **engine recalibration (0003, rules v2), stage B
(per-group support and the election result in the snapshot, 0006), the election night screen, and open-ended terms
(0005, 0007, 0008, rules v5) are done**, and the win bar is accepted: 55% overall support, nothing else (0004, 0009). Every win starts another term with a
fresh 100 credits added to anything unspent, and every term ends with its own election: the game goes on for as long as the player
keeps winning (there is only a technical ceiling of ten terms). The old weak point (after one win a player could do nothing and keep
winning) is addressed by **the news calendar** (0010, rules v6): bad headlines take a share of the lead and grow each term, with a paid
response.

The calibration below is now the engine's, expressed as the equivalent base numbers (Staffing 3200,
Repairs 2800, Access 3000, cost 2000, clip ±40,000) instead of "800 and 700 at 400% scale". The table
that follows was computed with the replica before the change; after it, real plays under the new engine
give the same results (`python3 tools/support-model/support_model.py`).

Evidence is reproducible: `python3 tools/support-model/support_model.py --check` replays ten strategies
that were played against the real service under **each** calibration and must match the engine's support
figures to the ppm (it does: 60 of 60 under both).

## Short answer

Yes, we can, and I now think it is the better goal. It is a concept every player knows (an
election), it makes the six public groups matter, and the engine was built with it in mind:
`framework.md` says *"Staying in power can be one scenario objective"* and *"election rules and a
'remain in power' objective belong in scenarios that specify them"*, and it already lists **polls as
"reported observations" kept separate from true support**.

I recommended a delivery score first because **today's support numbers can't carry a goal**, not
because the idea is wrong. Three specific things in the engine make them unusable, and they are all
changeable:

| # | What the engine does today | Where | Effect |
|---|---|---|---|
| 1 | Every delivered unit has a benefit **and** a flat cost (400 ppm per unit), so they nearly cancel: Staffing nets about +100, Repairs about 0 (negative for some groups), Access +350 | `cohort_response_scaled`, `reducer.rs` | Delivering the work your ministers need barely helps you, and Access-only plans out-poll Balanced |
| 2 | A hard clamp of ±8,000 ppm (0.8 points) per group per quarter, and a validator that only allows scale 50, 100 or 150 | `reducer.rs` `Rules::validate` and the clamp | A ceiling of about 4.8 points per group over a campaign; the whole range across 72 variants is 50.04% to 50.36%. **Correction:** measured later, the clip never actually bound (largest old move 4,450 of 8,000 ppm), so the flatness came from #1 and #3; the clip was raised only for headroom |
| 3 | A response lag of one quarter and a count at the end of Q6 | `reducer.rs` queued responses | The last quarter's work never reaches the final figure |

## Why "turn the volume up" is not enough

Playing real strategies and then re-running the same model with different numbers (the correlation
is how well the *ranking by support* follows the *ranking by work delivered*, 1.0 being perfect):

| Calibration | Correlation | What goes wrong |
|---|---:|---|
| Today | +0.30 | Support is flat (49.7 to 50.5%), and "Balanced two quarters late" beats "Balanced on time" |
| Today × 5, clamp × 5 | +0.30 | Same order, bigger swings: the Access-only plan (59 delivered) still wins and on-time Balanced still loses to late Balanced |
| Bigger benefits (Staffing 1500, Repairs 1200) | +0.98 | Ranking is right, but **every group ends above 50% for every plan that delivers anything**, so nobody is ever left behind and the election is trivial |

## A calibration that has the right shape

Changing only numbers the engine already has (benefit per unit, cost per unit, scale, clamp) and
counting the final quarter at election night. A search of 720 settings found **22** that both track
delivery (correlation +0.90 to +0.99) and still leave some groups behind. The one I'd start from:

> Benefit per delivered unit: Staffing **800**, Repairs **700**, Access **750** (ppm, before group multipliers).
> Cost per unit: **500**. Scale **400%**, clamp **±40,000**. Response lag 1. Election after Q6 resolves.
> **Win = overall support ≥ 55%.** (Originally also "and hold blocs worth ≥ 60% of voters"; that second bar never decided a result
> and was removed, see decision 0009.)

Its correlation with work delivered is **+0.99** (today: +0.30), and the results are in the table below.

Played against the ten real strategies:

| Strategy | Work delivered | Support | Voters held | Urban L/M/H · Rural L/M/H | Result |
|---|---:|---:|---:|---|---|
| Balanced in Q1 | 100 | 55.96% | 73% | 57 · 60 · **47** · 58 · 62 · **49** | **Win** |
| Staffing-first | 100 | 56.12% | 73% | 59 · 61 · **47** · 58 · 61 · **47** | **Win** |
| Staffing 45 / 40 / 10 | 94 | 55.57% | 73% | 56 · 60 · **48** · 58 · 61 · **50** | **Win** |
| Balanced a quarter late | 88 | 55.35% | 73% | 57 · 59 · **47** · 57 · 60 · **48** | **Win** (barely) |
| Stagger then re-plan | 78 | 54.71% | 73% | 56 · 58 · **48** · 57 · 59 · **49** | Lose (narrowly) |
| Balanced two quarters late | 66 | 54.10% | 73% | 56 · 57 · **48** · 56 · 58 · **49** | Lose (narrowly) |
| Access-only 24 + Repairs 35 | 59 | 53.69% | 78% | 53 · 55 · **48** · 59 · 61 · 53 | Lose |
| Staffing-only 45 | 44 | 52.82% | 73% | 56 · 55 · **48** · 52 · 53 · **46** | Lose |
| Repairs-only 35 | 35 | 51.69% | 74% | 50 · 53 · 50 · 52 · 56 · 54 | Lose |
| Never spend | 0 | 50.00% | 0% | all 50 | Lose |

What this gives a player:

- **Doing the job, early, wins.** Work that arrives late or is dropped loses the election, and
  re-planning has a visible political cost (the stagger run loses).
- **You always leave someone behind.** High-income urban and rural groups finish below 50% even
  when you win, because delivery costs them more than it gives. That is the "popular versus
  needed" trade-off the framework describes, and it now has numbers.
- **Doing nothing is not safe.** It scores exactly 50.0%, which is a loss.

## The goal and the end screen

> **Win the election.** Six quarters from now the public votes. Hold at least 55% overall support, whichever groups it comes
> from, while the people who were promised work are still waiting for it.

Election night, as the end screen, in this order: the result (**Re-elected**, **Narrow defeat** at
52% or more, **Heavy defeat** below that), a map of the six blocs sized by how many voters they hold
(the weights are 26/32/22/7/8/5%), each bloc's final mood, and who was left behind.

Keep the delivery score from [`goal-and-score.md`](goal-and-score.md) as a second line, **Legacy**:
"Re-elected, legacy 94." Power is what you play for; Legacy is whether it was real. They mostly
agree under this calibration and diverge at the margins, which is where the interesting stories are.

## Gamifying the mechanisms

Each of these already exists in the engine or the framework; the column on the right says what it
takes to show it to a player.

| Mechanism | What the player sees | Status |
|---|---|---|
| **Voter blocs** | Six bars with a mood, sized on a map by voter weight | Exists in the engine; **the snapshot only exposes the weighted total.** Needs per-group support in the snapshot |
| **Next quarter's mood** | "Coming next quarter: urban-middle +1.4, urban-high −0.9" | The engine already queues the response a quarter ahead. Expose the queue |
| **Honeymoon and backlash** | The one-quarter lag becomes the drama: you act now, you are judged next quarter | Exists; needs the previous item to be visible |
| **Lobbies** | School Staff Association (influence, strike threat) and Community Access Network (volunteer days, access boost) as allies you keep or lose | Exist in the engine and are described on the Progress screen; their live balances are not in the snapshot |
| **News calendar** | A headline in most quarters that hits some groups (a shortage headline, a funding row) | **Built (0010, stage 1):** invented headlines, announced a quarter ahead or a surprise, a paid response. The calendar is fixed by the campaign's identity, so replays and branches match |
| **Scrutiny** | An audit is the inspectorate report. If a minister overstated, it hurts | Needs audit results shown in the app and ministers that can misreport (see `goal-and-score.md`) |
| **Polls** | A noisy poll each quarter, with the true figure revealed only on election night | The framework calls polls "reported observations". New content: a deterministic, seeded error so replays still match |

## Engine changes this needs (Rust, so deliberate)

1. **Done** (`1c2dbee`): the new benefit and cost table, clip and shock cap in
   `crates/domain/src/reducer.rs`, the matching `authored.json`, and a test that keeps them equal.
   The scale stays within the validator's 50, 100, 150 because the baseline itself was rescaled.
2. **Done (stage B, 0006):** the snapshot carries each group's support, what is queued to land, and each
   group's share of the voters. Still to do: the two lobbies' balances.
3. **Done (stage B, 0006):** an election rule and result in the snapshot, counting the last quarter's work.
   The bars were constants in `reducer.rs`, deliberately outside the hashed scenario file. Since the verdict now
   decides whether the game continues, they moved into `authored.json` (0007), and re-tuning them invalidates saves.
4. **Done** (`1c2dbee`): CI pins the sensitivity results, so the report and CSV were regenerated in the
   same change (`cargo run -p myth-domain --example p3_report`), and the replica tool holds both
   calibrations so `--check` passes.
5. **Done:** the rules version was bumped to v2, so games saved under the old numbers are refused (and
   the start screen explains it), and the full Rust, pack, smoke and browser checks were re-run.

## Honest limits

- **These are authored hypotheses, not data.** The framework and the case contract say so, and this
  proposal keeps that: the numbers are tuned for play, not predicted from England.
- **Balanced still wins, with a margin of only about 1 point over the bar.** With scripted ministers
  and no events, the best plan is still known in advance. The news calendar and ministers who slow
  down are what make the player adapt. I have *not* simulated them, so treat the thin margin as a
  reason to build them, not as a tested design.
- **The model is validated against ten plays**, which is enough to trust the replica's arithmetic
  but is not a tuned, balanced game. Expect to re-tune once events exist.
- A "remain in power" objective for a real-country twin needs the framework's care: fictional
  officeholders and the renamed data-twin mode it already suggests.

## Decisions

Asked on 2026-11-01 and answered by the owner (full records in [`decisions/`](decisions/README.md)):

1. **Power as the headline, Legacy as the second line:** yes. ([0001](decisions/0001-goal-stay-in-power.md))
2. **One election at the end; a win continues, a loss ends the game.** No mid-term gate. "Continues" means
   **another term of six quarters, and so on for as long as the player wins**, carrying over support, lobbies, minister beliefs and unfinished
   promises, with **a new allocation each term added to anything unspent** (R3). ([0002](decisions/0002-single-election-win-continues-loss-ends.md),
   [0005](decisions/0005-a-win-starts-a-second-term.md))
3. **Change the frozen engine numbers and regenerate the pinned report in the same change:** yes, and done.
   ([0003](decisions/0003-recalibrate-the-support-model.md))
4. **The win bar:** 55% overall support and nothing else: the owner accepted 55%, and dropped the second "60% of voters" bar
   after seeing it never decided anything. ([0004](decisions/0004-election-win-bar.md), [0009](decisions/0009-overall-support-is-the-only-bar.md))
5. **Terms continue for as long as the player keeps winning, each ending with its own election; the free audit resets each
   term.** Built as one campaign that grows. ([0007](decisions/0007-two-terms-in-one-campaign.md),
   [0008](decisions/0008-terms-continue-while-the-player-wins.md))

## Suggested order of work

1. **Recalibrate** (done, 0003) **and expose** (done, 0006). Support is meaningful and available to the screen.
2. **Election night and the bloc map** (UI, plus the Legacy line) and **open-ended terms**. Done: the player has a goal,
   and a win leads to another term.
3. **Events and ministers that slow down or misreport** (content). This is what makes it a game
   rather than a solved puzzle.
4. **Mood forecast, lobbies, polls** (UI and light engine exposure). The gamification layer.
