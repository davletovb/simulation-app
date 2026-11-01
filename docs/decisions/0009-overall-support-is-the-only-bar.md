# 0009. Overall support is the only election bar

- **Status:** **Implemented** (rules v5). **Supersedes** the second bar in [0004](0004-election-win-bar.md) (groups above 50% must
  hold at least 60% of the voters). The first bar (55% overall support) stands.
- **Date:** 2026-11-01
- **Decided by:** the project owner. They asked whether the two bars were a problem: *"what if the player wants to change the course
  in the middle, and wants to start taking a completely different position, different decisions and policies, therefore attracting
  support from different groups? at the end the overall election results will decide whether the player wins or not."* Shown the
  evidence below and my recommendation to make overall support the only bar, they answered **"yes"**. Recorded by Claude.

## Decision

An election is won with **at least 55% overall support**, and nothing else. It does not matter which groups the support comes
from, so a player may change course and win a different set of groups. Overall support already counts every group, weighted by its
share of the voters, so nobody is ignored by the arithmetic.

The screen still says which groups are behind the player (*"4 of 6 groups back you, holding 73.0% of the voters"*, and the map of
the six groups), but as information, never as a test. At the very start, when every group sits at exactly 50%, it says
**"No group has taken a side yet"** instead of showing "0.0%" and a red "short by 60 points" (what the owner's screenshot showed): a
group at exactly 50% is neutral, not against the player.

## Evidence

`tools/support-model/coalition_sweep.py` (runs in about ten seconds) plays the support model, which reproduces the engine to the ppm:

| What was played | Plays | Cleared 55% overall | Of those, stopped only by the coalition bar |
|---|---:|---:|---:|
| Every opening plan, in steps of 5 credits | 1,771 | 389 | **0** |
| Every opening plan, then a re-plan in quarter 2, 3, 4 or 5 (a change of course) | about 163,000 | about 16,600 | **0** |

So the second bar never decided a result; it only added a second, harder-to-read number to the screen. The reason is in the
model, not in the bar: every unit of delivered work lifts every group (by a different amount) and costs them all a little, and
nothing in the model *repels* a group. Plans change the margins but not who backs the player: urban low and middle income and
rural low and middle (sometimes rural high) end above 50% every time, and urban high income never does. That is the real answer to
"different policies attracting different groups": the model has **no positions** yet, so there are not really different groups to
attract. The bar belonged to a game with that trade-off.

The bar *could* bind in a richer game. In an artificial case (urban low and middle at 70%, the rest at 40%) overall support is 56.2%
while the groups above 50% hold only 54% of the voters: a win now, a defeat before. That is intended here, and is covered by a
test, but it is a result of the player's choices and not of anything the current content can produce.

## What changed

- **Engine:** `ELECTION_MIN_BLOCS_PPM` and `ElectionProjection::meets_blocs_bar` are gone; `is_win` is the support bar alone.
  `ELECTION_HELD_ABOVE_PPM` stays, because the screen still needs to know which groups back the player, and `blocs_held_ppm` stays
  in the snapshot as information. The domain test `overall_support_alone_decides_the_election` replaces the one that made each bar
  able to bind alone.
- **Scenario:** `election.min_blocs_ppm` is removed from `authored.json` (a test and `build_pack.py` both refuse it coming back);
  `bloc_held_above_ppm` stays with a note that it is for display. `RULES_VERSION` is now `p4-england-education-rules-v5`, so games
  saved under v4 cannot be opened; the start screen explains this and offers a fresh name.
- **Snapshot:** `election.min_blocs_ppm` is gone. Nothing else changes shape.
- **Screens:** one bar (`Overall support 55.9%, 55.0% needed: met`) with the plain-words groups line under it, on the election
  outlook, on election night and on the progress page. Headlines talk only about overall support
  (*"You cleared the bar: 55.9% overall support (55.0% needed)"*, *"So close: 54.0% overall support was short of the 55.0% you
  needed"*). The briefing says it does not matter which groups the support comes from. A web test pins that the verdict is the same
  whatever share of the voters sits in groups behind you.
- **Tools:** the replica's win rule is the support bar alone; the "voters held" column stays as information.

## Consequences

- **Nothing in the sensitivity report or the pinned numbers moves** (the report is byte-identical, checked in CI), and no play that
  used to win now loses. Plays that could have lost only on the coalition bar could not be found in the model.
- A player can no longer be told "you won overall but your coalition was too narrow". If a later version gives groups opposed
  interests (news events, policy positions), a coalition rule can come back as a new decision; the tool above is how to check
  whether it would change anything.
- The weakness recorded in [0008](0008-terms-continue-while-the-player-wins.md) (since addressed by [0010](0010-news-calendar.md)) was untouched here and mattered more: with one bar and
  no erosion of support, a player who wins once can do nothing and keep winning.

## To change or revert

To bring a coalition bar back, add `min_blocs_ppm` to `authored.json`, `ELECTION_MIN_BLOCS_PPM` and `meets_blocs_bar` to the engine,
use it in `is_win`, restore the second meter on the screen and in the validators, regenerate the manifest, and bump
`RULES_VERSION`. 0004 describes the rule and its evidence as it was.
