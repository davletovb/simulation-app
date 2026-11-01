# 0007. Two terms in one twelve-quarter campaign, with an explicit "begin term" move

- **Status:** **Implemented** (rules v3), **partly superseded by [0008](0008-terms-continue-while-the-player-wins.md)**: the "exactly two terms" and
  "final victory" parts were a misreading of what the owner asked and are gone (rules v4); one campaign, `begin_term` and the bars in
  the hashed scenario stand. Supersedes the build recommendation in [0005](0005-a-win-starts-a-second-term.md)
  (two linked campaigns) and the "bars live in code" choice in [0006](0006-what-the-snapshot-exposes.md).
- **Date:** 2026-11-01
- **Decided by:** the project owner asked for the second term in conversation: *"start the second term, term 2 ends
  with its own election, the free audit resets each term, your recommended 55% / 60% bar is ok"*. The three
  engineering choices below (single campaign, bars in the hashed scenario, explicit `begin_term`) were made by Claude
  while building it, with the evidence given, so the owner can overrule them.

## Decided by the owner (2026-11-01)

1. **Term 2 ends with its own election.** A win there is the **final victory** and ends the game; a loss ends it too.
   There is no third term.
2. **The free audit resets each term.** One free audit per term, available from the third quarter of that term;
   later audits in the same term cost 2 credits.
3. **The bar is accepted:** 55% overall support **and** groups above 50% holding 60% of the voters ([0004](0004-election-win-bar.md)
   is no longer provisional).
4. **R3 budget** (from 0005): each term begins with a fresh 100 credits **added** to anything unspent.

## Decided by Claude while building, with the evidence

### 1. One campaign of twelve quarters, not two chained campaigns (reverses the recommendation in 0005)

0005 recommended chaining a second campaign onto the first. Building it showed why that does not work here: the engine
rebuilds a campaign by **replaying its journal**, and a `Campaign` holds about twenty private fields that are not
serialisable as state (each minister's beliefs and relationships, active efforts, audit records, shadow scopes,
conditions, vote history, command-id sets and more). A successor would have had to either copy all of it into a new
carried-state format, which is a large new surface to keep honest, or replay the predecessor's journal inside the
successor, which is a single campaign with extra steps. So the campaign is extended to twelve quarters instead.

- `Quarter` accepts 1 to 12 (`MAX_QUARTERS`); `TERM_QUARTERS = 6`, `TERMS = 2`. The shock calendar is 12 long.
- The treasury gained a `granted` total (100 after the first term's grant, 200 after the second). Its invariant is
  *"the six accounts add up to what has been granted"*, where it used to say 100. The only way credits enter is a
  `Grant` ledger move, made by `begin_term`.
- A new phase, `ElectionNight`, sits between `Resolved` and `QuarterClosed`.
- **Carry-over timing:** nothing about support is rewritten at the boundary. The election counts the queued
  last-quarter response (as it always did); that same response then lands during the second term's first quarter, as any
  queued response does. Support, ministers' memory, lobbies and unfinished promises are simply the same campaign
  continuing.

### 2. The election bars are now part of the hashed scenario (reverses 0006 choice 2)

0006 kept the bars in code so re-tuning would not invalidate saves, because they only *read* a final state. Now the
verdict **decides whether the game continues**, so it must be reproducible from the saved game. `authored.json` now
carries `terms` and an `election` block (`min_weighted_support_ppm`, `min_blocs_ppm`, `bloc_held_above_ppm`,
`held_after_quarters`), plus the audit allowance, and a drift test in `crates/contracts/src/checkpoint.rs` fails if the
file and the engine's constants ever disagree. **Re-tuning the bars now invalidates saves**, like any other rule.

### 3. Beginning a term is an explicit player command, never a generic advance

On election night the only legal move is `begin_term`. The HTTP API refuses `advance_slice` there (409), because the
generic advance would otherwise let the scripted player begin the term on the person's behalf, the same trap that was
fixed for "propose" and "decide" in the guided turn. The adapter's scripted player *does* begin it (so scripted full-game
runs and tests work); only the API refuses.

## Rules version and old saves

`RULES_VERSION` is now `p4-england-education-rules-v3`, and the scenario hash changed. Games saved under v2 cannot be
opened; the start screen explains this and offers a fresh name (this was built for 0003). The sensitivity report and CSV
are **byte-identical** (the second term does not touch the first six quarters), which CI checks.

## What the snapshot gained

`term`, `terms`, `term_quarters`, `past_elections[]` (term, quarter, won, support, blocs held), and `election.term` /
`election.terms`. `election.status` is `projected` while a term runs, and `won`/`lost` once its election is held (the
verdict is read from the recorded result, not recomputed). `closed` means *the game is over*: after a loss, or after the
second election. A first-term win leaves the phase at `ElectionNight` with `closed: false`.

## Known limitation (please read): the second election is nearly free

Support does not erode, and there are no events yet. So a player who wins the first election and then **does nothing**
for the whole second term wins the second one with exactly the same numbers (tested through the real command path:
55.96% overall, 73% of voters, urban-high and rural-high behind). Spending the second allocation well pushes it higher
(a repeat of the Balanced plan reached 61.9%). The bar is only a real test in the second term for a player who scraped
through the first.

This is a content problem, not a rules bug, and it is the same problem `stay-in-power.md` flagged ("the margin is thin,
and without events the best plan is known in advance"). Options, none decided:

| Option | Effect | Cost |
|---|---|---|
| **A. Raise the second bar** (for example 57% / 65%) | A hands-off second term loses; a good one still wins | One more number in the hashed scenario; the bar is now term-specific |
| **B. Let support drift back toward 50%** each quarter | Standing still costs you; delivery has to keep up | Changes the support model, so the pinned sensitivity report and the replica change |
| **C. A news calendar** (events that hit some groups) | Pressure with a story | The most work, and the most game |

My recommendation is B or C before treating the second term as a finished game; A is the cheap stopgap. The assertion
that pins the hands-off result is in `election_night_is_published_and_matches_the_independent_replica` and is marked
`KNOWN LIMITATION`; it is the one that should change.

## Consequences

- A whole game is up to 72 minister actions over twelve quarters. The command-journal bound (512) is far from
  reached by a human playing both terms (a full two-term run through the API is a test).
- Per-line plan size is still capped at 100 credits; a plan's total can be up to 200 in the second term (whatever is
  fundable). The delivery forecast and the plan builder accept that and label the columns with the term's own quarters.
- The forecast, the Progress track, the HUD, the briefing, the audit panel and the election screens all read the term
  from the snapshot instead of assuming one term of six quarters.
- Legacy (delivery grade) is computed over everything granted so far, so at the end of the game it grades 200 credits.

## To change or revert

- **Return to one term:** set `TERMS = 1` in `crates/domain/src/lib.rs`, set `terms` to 1 in `authored.json`, regenerate
  the manifest (`tools/p2/build_pack.py --write`) and the contracts (`export-contracts --write`), bump `RULES_VERSION`, and
  fix the web `OFFLINE_PILOT.terms`. A win on election night then closes the game (the engine already ends the game after
  the last term's election).
- **Change the bars:** edit both `authored.json` and the `ELECTION_*` constants in `reducer.rs` (a test fails if they
  differ), regenerate the manifest, bump `RULES_VERSION`.
- **Change the audit allowance:** `term_audits` and the `in_term() >= 3` check in `Campaign::order_audit`, and the matching
  `audit` block in `authored.json`.
