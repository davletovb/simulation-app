# The news calendar: design proposal

**Status (2026-11-01): proposal, nothing built.** The owner chose the news calendar as the fix for "a player who wins once can do
nothing and keep winning" ([0008](decisions/0008-terms-continue-while-the-player-wins.md)), said it must work **in any term, from the
first**, and must **add challenges**. This note is how I would build it, with numbers from a sketch on the support model
(`python3 tools/support-model/calendar_sim.py`). The choices that are the owner's are listed at the end.

## What problem it solves

Today support only moves when work is delivered, and nothing ever pulls it back. So after one win a player who does nothing keeps
winning (100% of games, every term, for ten terms), and a player who keeps delivering climbs without limit (the Balanced plan repeated
reaches 74% by term 4). Both are the same flaw: **nothing happens to the player that they did not cause.** A calendar of events gives
the world its own moves.

## Two things the sketch showed (these shape the design)

1. **Flat random shocks are wrong twice.** With three bad-leaning headlines a term at a fixed size, the *best* opening plan wins the
   first election only **23%** of the time, because its margin is 1 point (55.96% against a 55% bar) and one 2-point headline erases it.
   Yet after term 1 the same shocks are harmless: a player who keeps delivering has a 14-point cushion (median) and never notices.
   Unfair first, trivial later.
2. **So the calendar has to scale with the player's cushion, and the player needs something to do about it.** If bad news takes a
   *share of what you have built* (a group at 70% loses more than one at 52%), a big lead is no longer a safe lead and doing nothing
   stops working; and if the player can see it coming and pay to soften it, outcomes depend on choices and not on dice.

## How it would work

**One headline a quarter at most, announced a quarter ahead.** Each term carries four or five headlines in its quarters 2 to 6. From
the quarter before, the Play screen shows **"Coming next quarter"**: the headline (clearly labelled as invented), which groups it hits
and whether it is minor or serious. A **News** list on the Progress screen keeps the ones already landed. All of it is deterministic
from a seed in the hashed scenario and the term number, so any term has a calendar (no list of 60 quarters to author), saved games
replay exactly, and a "what-if" branch sees the same headlines as the game it was split from.

**Four kinds of headline** (the first two make the first version):

| Kind | Example | What it does |
|---|---|---|
| **Mood** | "Teacher shortage headlines", "Rural bus cuts", "Cost-of-living squeeze" | Moves support, group by group. Bad news takes a small base **plus a share of each group's lead over 50%**; good news ("Inspectorate praises you", "Volunteer surge") adds a flat bonus |
| **Disruption** | "Roof scare closes three schools" | One kind of work can deliver less for a quarter (Repairs 10 to 8 a quarter). The engine already has this for quarter 2 only; it becomes any quarter, any kind |
| **Scrutiny** *(later)* | "Inspectorate visit announced" | When it lands, work promised by now but not delivered costs support. Rewards honest plans |
| **Windfall** *(later)* | "A grant", "a volunteer surge" | A few extra credits, or extra Access capacity for a quarter |

**What the player can do about it.**
- *Plan around it.* A disruption is announced a quarter early: move work to another kind, or shift it a quarter.
- *Keep a reserve and respond.* In the quarter before a headline lands, a **Respond** move lets the player pay credits to cut the hit
  (for example 10 credits to take 60% off). Those credits do not become delivered work, so they count against Legacy and slow the
  next gains. It is a real trade: respond to the big ones, or when the margin is thin, not to everything.
- *Existing moves still count:* re-planning, audits, and the two organisations.

**How it ramps across terms.** The first term is gentle and is a fair test of the opening plan (see the table: with no bad news in
term 1, the best plan wins it every time). From term 2 the headlines are full strength and each term is about 35% heavier than the
last, up to a cap (the engine's existing limit of 4 points on one group from one headline).

## What the sketch says

Share of games still going after each election (a game ends at the first lost one); `idle` plays Balanced once and then nothing,
`repeat` plays Balanced every term, `smart` is `repeat` plus the paid response:

| Calendar | Player | Term 1 | Term 2 | Term 3 | Term 4 | Term 8 |
|---|---|--:|--:|--:|--:|--:|
| **None (today)** | idle | 100% | 100% | 100% | 100% | 100% |
| Flat shocks | repeat | 23% | 23% | 23% | 23% | 23% |
| Cushion-scaled, bad news takes 35% of the cushion, term 1 half strength | idle | 74% | 29% | 13% | 5% | 0% |
| | repeat | 74% | 74% | 74% | 74% | 73% |
| | smart | 74% | 74% | 74% | 74% | 74% |
| **Same, but term 1 has no bad news** *(recommended)* | idle | 100% | 47% | 21% | 8% | 0% |
| | repeat | 100% | 100% | 100% | 100% | 98% |
| | smart | 100% | 100% | 100% | 100% | 100% |
| Same, bad news takes 50% of the cushion | idle | 62% | 16% | 6% | 2% | 0% |
| | repeat | 62% | 62% | 61% | 59% | 53% |
| | smart | 70% | 70% | 70% | 70% | 70% |

Read it as: a player who stops playing **is out within two or three terms** instead of never; a player who keeps delivering stays in;
the response is worth having (it lifts the 50% setting from 53% to 70% by the eighth term); and the harder setting is real pressure
on a competent player too. The knob is one number (how much of the cushion bad news takes) and is meant to be tuned in play.

## Honest limits

- It is a sketch on the support model, not the engine. Delivery is best-case, the deck is nine invented headlines, and the response
  and its price are my guesses.
- A competent player is mostly safe at 35% (median margin about 8 points). Making it bite means turning the knob towards 50%, at
  which point luck decides some first terms unless term 1 is quiet.
- Nothing here gives groups opposed interests, so a headline hits groups in different amounts but still moves everyone the same way.
  That is a bigger design step (policy positions) and is not part of this.
- All headlines are invented and labelled so; none is a claim about real people or a forecast.

## Engineering, so you can judge the size

- **Engine:** the per-quarter shock table (`Rules.shocks`, always zero) becomes derived from the seed, the term and the scenario's
  deck; the Q2 repair cap generalises to any quarter and kind; one new player move (`respond`) and one new treasury account (credits
  spent on relief, so the six-account invariant keeps holding). Rules version v6: saves from v5 can no longer be opened. The
  sensitivity report stays byte-identical (its runs set shocks to zero themselves).
- **Snapshot and screens:** `news.upcoming` and `news.recent`, a "Coming next quarter" card, a Respond control, a News list.
- **Replica and tests:** the replica learns the calendar (so it can still predict the engine to the ppm), plus tests for determinism
  across terms, forks, the cap, and the response arithmetic.
- **Staging I would use:** (1) mood headlines with warning and the cushion rule, no response, term-1 quiet; (2) Respond and
  disruptions; (3) scrutiny and windfalls. Each stage is playable and checked against the sketch before the next.

## What I need from you

1. **Warning:** announce every headline a quarter ahead (my recommendation), or leave some as surprises?
2. **Respond from the first version?** Recommended: yes. Without it, headlines are dice and the first election is luck.
3. **Term 1:** no bad news at all (recommended: a fair test of the opening plan), half strength, or full?
4. **How hard later:** bad news takes 35% of the cushion (safe for a player who keeps delivering) or 50% (real pressure on a good
   player too)? It is a number, so it can change after you have played it.
5. **Tone:** all invented, plainly labelled. The current real-source news items stay separate.
