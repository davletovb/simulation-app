# 0008. Terms continue for as long as the player keeps winning

- **Status:** **Implemented** (rules v4). **Supersedes** the "two terms" parts of [0007](0007-two-terms-in-one-campaign.md) and the
  "final victory" idea in it. The rest of 0007 (one campaign, `begin_term`, bars in the hashed scenario) stands.
- **Date:** 2026-11-01
- **Decided by:** the project owner, correcting a misreading. Their words: *"I didn't say it will be two terms, the terms can
  continue as long as the player is winning the elections at the end, but the game now shows two terms, while the first term is
  not yet finished and we don't know if the player can win or not."* They attached a screenshot of the header reading
  "Term 1 of 2" with twelve quarter pips. Recorded by Claude.

## What went wrong

The owner had said *"term 2 ends with its own election"*. I read that as "the game is exactly two terms long" and built a
**final victory** at the end of term 2, a header that said "Term 1 of 2", twelve pips, a progress page that drew both terms before
the first had been played, and copy such as "Your second term" shown while the first election was still unknown. What was meant:
every term ends with an election, and a win simply starts another term, for as long as the player keeps winning. The screens
must never announce a number of terms: before the first election nobody knows whether there will be a second.

## Decision

- **Every win starts another term.** There is no "last" election and no "final victory". A loss ends the game, at any term.
- **The screen only shows the term being played:** "Term 3 · quarter 2 of 6", six quarter pips, a progress page that draws the
  current term only, and a list of the elections already held. The button after a win says "Begin term N+1".
- **Nothing is announced ahead of time.** A source test (`apps/web/scripts/no-fixed-terms-regression.mjs`) fails if the web
  code mentions "second term", "two terms", "term N of M", "final election/victory", or reads a fixed count of terms.
- **The snapshot no longer carries `terms`.** `election.ceiling` (see below) is the only hint of an end, and it is only true on
  the one election it concerns.

## The one technical limit (please confirm or change it)

A game cannot literally go on forever: it is a journal that is replayed from the start, and the journal, the API's command log and
the credit counters are bounded. So there is a **technical ceiling of `MAX_TERMS = 10` terms (60 quarters)**. It is not a rule of
the game and is not shown to the player in advance; it only matters if someone wins ten elections in a row, and then the screen
says so plainly: *"The longest tenure on record: you have served 10 terms, the most this version can record."* (`election.ceiling`
is true for that election only, and winning it closes the game.)

Measured, not assumed: a scripted game played all ten terms in one run (60 quarters, 609 actions, closing at quarter 60), and each
step stayed under about 0.4 seconds in a debug build at the end (replay cost grows with the game: roughly 2 s per term at the
start of a scripted run, 36 s per term at the end, in the slowest build). The bounds were raised to hold it: action journal
512 to 1024, API command log 512 to 1024.

To raise or remove the ceiling: change `MAX_TERMS` (`crates/domain/src/lib.rs`), the matching `technical_ceiling_terms` in
`authored.json`, the two bounds above, and the web validators' `MAX_TERMS`; regenerate the manifest; bump `RULES_VERSION`. Credits
(`u16`) allow far more than 10 terms; the practical limit is replay time, which a snapshot/checkpoint of carried state would remove.

## Rules version and old saves

`RULES_VERSION` is now `p4-england-education-rules-v4`: a win at quarter 12 used to end the game and now starts term 3. Games
saved under v3 cannot be opened; the start screen explains this and offers a fresh name.

## What changed in the engine

- `TERMS` (2) became `MAX_TERMS` (10), documented as a ceiling. `Quarter` accepts 1 to 60; the shock calendar is 60 long; the
  treasury's `granted` may reach 1,000 (a whole number of 100s).
- A won election always leaves the game on `ElectionNight`, except at the ceiling, where it closes.
- `authored.json` lost `terms` and gained `election.held_after_every_quarters` and `election.technical_ceiling_terms`
  (a test keeps both equal to the engine; `build_pack.py` refuses a scenario that fixes a number of terms).
- The scripted-run helper `run_until_closed` was **removed**: with a scripted player who always wins it would play all ten terms.
  Scripted runs stop at the end of a term (`run_until_term_end`), including the `p4-trial` benchmark.

## Known weakness, now more important (unchanged from 0007)

Nothing erodes support, so a player who wins the first election can **do nothing in every later term and keep winning**, up to the
ceiling (the pinned test shows identical numbers on the second election). With open-ended terms this stops being a quirk of term 2
and becomes the shape of the whole game: there is no pressure to keep playing well. The three options in 0007 still stand
(a rising bar, support that drifts back toward 50%, or a news calendar); with unlimited terms I would now call at least one of
them **required** before this is a finished game. Nothing has been changed here.

## Also fixed in passing

A false red alert "That move wasn't found after reconnecting" could appear right after a click in a long game. The recovery check
asked the server about the move while it was still being admitted, and the admission takes longer the longer the game is. It now
waits until the send has finished before treating "not found" as a problem.

## To revert

Reverting to exactly two terms means restoring 0007's rule: `MAX_TERMS = 2`, a win at the end of term 2 closes the game, and the
screens say "Term N of 2" again. The owner has said they do not want that; this is recorded only so the history is complete.
