# Goal and score: draft for decision

> **Update:** a "stay in power" goal is the better headline, and it is worked out in
> [`stay-in-power.md`](stay-in-power.md). Public support cannot carry a goal *with today's numbers*
> (that is what this page shows), but a small recalibration fixes that. The Delivery Score below
> then becomes the second line on the end screen, **Legacy**.

**Status: partly built.** The headline goal (stay in power, two terms) and **Legacy** (the delivery grade, computed in
`apps/web/src/election.mjs`) are in the app; see [`decisions/`](decisions/README.md). The rest of this page is the original
proposal and its evidence. Every number below comes from
playing the real engine to the end (scripted ministers, `MYTH_MODE=scripted`), not from
reading the rules, because reading the rules alone led to two wrong assumptions (see
[Corrections](#corrections-to-earlier-statements)).

## In one paragraph

The case contract defines no win condition, so the game ends after Q6 with a tally. The
engine can reliably measure **how much work was actually delivered, and when**. It cannot
yet measure public support in a useful way (it barely moves, and moves the wrong way).
So the proposed goal is *deliver the programme, early, and keep your promises*, scored 0 to 100.
The score ranks sensible play correctly, but **Balanced in Q1 already scores 100**, so
the score is only interesting once the game gives you a reason not to play it. The levers that
would do that already exist in the case contract and are listed below.

## What the engine can measure today

| Measure | Where it comes from | Usable in a score? |
|---|---|---|
| Work actually delivered (credits) | `treasury.service_spent` | **Yes.** Exact, integer, public |
| Promised but undelivered at the end | `encumbered_*` | **Yes** |
| Unspent credits, audit spend | `available_credits`, `audit_spent` | Yes |
| When work was delivered | Only the final total is in the snapshot | **Needs a small change:** add `delivered_cumulative` per quarter to `delivery_history` (the figures below were recorded by polling after each quarter) |
| Public support | `weighted_support_ppm` | **No.** See below |
| Support by group (urban/rural, income) | Not in the snapshot | No (would need the API to expose it) |
| Ministers misreporting | Never happens in the scripted pilot: `reported == due == delivered` in every quarter of every run | Not yet |

**Why not public support.** The engine's own sensitivity report puts support between 50.04% and
50.36% across all 72 accepted variants, and balanced vs staffing-first tie exactly in all 36
pairs. In my runs it also points the wrong way: the best run ends at **50.08%**, the run that
delivered only 66 ends at **50.24%** (delivery costs credits, so the dip arrives later). A score
built on it would reward doing worse.

## What happened when I played seven strategies

All seven were played through Q6 against the real service. "Cumulative" is work delivered after
each quarter ends.

| Strategy | Delivered | Unfinished promises | Unspent | Cumulative delivered, Q1 to Q6 |
|---|---:|---:|---:|---|
| A. Balanced (50/40/10) in Q1 | **100** | 0 | 0 | 4, 21, 44, 66, 88, 100 |
| B. Staffing-first (60/30/10) in Q1 | **100** | 0 | 0 | identical to A |
| C. Staffing 45 / Repairs 40 / Access 10 | 94 | 1 | 5 | 4, 18, 39, 58, 78, 94 |
| D. Vote fails, Balanced a quarter late | 88 | 12 | 0 | 0, 4, 21, 44, 66, 88 |
| E. Two quarters late | 66 | 34 | 0 | 0, 0, 4, 21, 44, 66 |
| F. Staffing 45 in Q1, re-plan to 45/35/10 in Q2 | 78 | 12 | 10 | 0, 4, 18, 39, 58, 78 |
| G. Never spend | 0 | 0 | 100 | all zero |

Rules this exposed (all confirmed by running, not inferred):

- **Votes:** Finance supports anything affordable; Education needs Staffing ≥ 45; Infrastructure
  needs Repairs ≥ 35. You need 2 of 3, so a 45-credit Staffing-only plan passes.
- **A new plan replaces undelivered work.** The engine's funding check is
  `plan total ≤ unspent + all undelivered work`. Delivered work stays.
- **Re-planning restarts the delay clock** on the replaced work. In F, Q1's Staffing delivered
  nothing in Q2 (in A it delivered 12), which is why F loses 22 points against an ordinary plan.
- **Staffing below 50 triggers the School Staff Association slowdown** (C: 94 instead of 95 delivered).

## Proposed goal

> **Deliver the pilot.** Turn your 100 credits into work that actually reaches schools by the
> end of Q2 2028: as much as possible, as early as possible, without breaking promises.

It is one sentence a player can hold in their head, it uses only what the engine measures, and
it matches what the player already sees ("Delivered so far", "Promised, not yet delivered").

## Proposed score: the Delivery Score (0 to 100)

| Part | Points | Meaning | Formula |
|---|---:|---|---|
| **Delivered** | 70 | What reached schools | `70 × delivered / 100` |
| **On time** | 20 | How early it arrived | `20 × Σ cumulative delivered ÷ Σ cumulative delivered of the best run (A)`; A is 323 |
| **Promises kept** | 10 | Finished what you committed | `10 × delivered ÷ (delivered + unfinished)`, 0 if nothing was promised |

Unspent credits and audit spending need no extra term: they are simply not delivered work, so
they already lower **Delivered**. Not spending scores 0 by construction.

Computed from the runs above:

| Strategy | Delivered | On time | Kept | **Score** | Grade |
|---|---:|---:|---:|---:|---|
| A. Balanced in Q1 | 70.0 | 20.0 | 10.0 | **100.0** | Mandate delivered |
| B. Staffing-first in Q1 | 70.0 | 20.0 | 10.0 | **100.0** | Mandate delivered |
| C. Staffing 45/40/10 | 65.8 | 18.0 | 9.9 | **93.7** | Mandate delivered |
| D. A quarter late | 61.6 | 13.8 | 8.8 | **84.2** | Partly delivered |
| F. Stagger then re-plan | 54.6 | 12.2 | 8.7 | **75.5** | Partly delivered |
| E. Two quarters late | 46.2 | 8.4 | 6.6 | **61.2** | Falling short |
| G. Never spend | 0.0 | 0.0 | 0.0 | **0.0** | Mandate missed |

Suggested grades: **90+ Mandate delivered · 70 to 89 Partly delivered · 40 to 69 Falling short ·
under 40 Mandate missed.** These cut points sit in the gaps between the runs above; retune them
once the levers below make real differences between good players.

On the end screen it should read as a report card, in the same words as the rest of the game:
the grade, the three parts, and the line "You delivered 94 of 100 credits of work. 1 credit of
work was still promised when the campaign ended."

### What to build first (smallest useful version)

1. **v0, no engine change:** grade on `delivered` alone (`service_spent`), shown on the
   "Campaign complete" screen. Honest, simple, correct.
2. **v1:** add `delivered_cumulative` to `delivery_history` in the snapshot (one field, plus
   the generated TypeScript and schema), then add **On time** and **Promises kept**.

## Why it isn't a game yet, and the smallest fixes

Balanced (and Staffing-first) deliver everything with zero risk. Ministers never misreport, no
shock bites, and the 100-credit budget is below the delivery capacity (about 124 credits of work
can be delivered by Q6 given the caps), so nothing is ever a trade-off. A score cannot fix that.
These are already in the case contract and are, in my order of preference:

1. **Make a shock bite.** The Q2 repair-cap patch is documented as *inert* (Repairs first
   delivers in Q3), so it never matters. Move it to Q3 or Q4. Then re-planning in response is a
   real decision, and the clock-restart cost makes it a hard one.
2. **Let ministers slow down and misreport.** The contract already has the rule (effort under 900
   makes reports optimistic) but nothing triggers it. Tie a minister's effort to how their area
   is funded. This is what makes audits worth buying.
3. **Make the budget bind.** If the budget is close to or above deliverable capacity, choosing
   between Staffing, Repairs and Access becomes a real trade-off. For example 140 credits, or lower caps.
4. **Give support real range, then add a fairness term.** Scale the response coefficients and fix
   the sign problem, expose per-group support, and add "no group below X" as a fourth part of the score.

## Recommendations on the other items

| # | Item | Recommendation | Size | Priority |
|---|---|---|---|---|
| 1 | No goal or score | Adopt the Delivery Score; ship v0, then v1 | Small (UI) then small (one API field) | **Do first** |
| 2 | Solved game | Levers 1 and 2 above | Small to medium (content, scripted ministers) | **High**; without it the score is a formality |
| 3 | Audit results invisible | Show the chair's own audit result in the app as a plain card ("Staffing, Q2: ministry reported 12, inspected 9"). The contract says an audit result *may be privately held by the chair*, so showing it to the player is the design intent. "Stop the service and run `myth-report`" is an operator tool. Keep ministers' private effort hidden. | Medium (a typed `chair_audits` list in the snapshot, generated TS, one UI card) | **High**: you pay credits and see nothing today |
| 4 | Audit access and cost | The snapshot does not say whether the free audit is used. With 0 unspent credits a second audit fails with "Offline step failed". Add `audits_ordered` to the snapshot so the UI can show "free" or "2 credits (you have N)" and disable the button when it can't succeed | Small | Medium |
| 5 | A second possible leak | `Delivered so far` shows the *true* delivered total. Today it equals the reports, so nothing leaks. The moment ministers misreport (lever 2) that figure would reveal it for free and make audits pointless. Decide now: show reported delivery until audited | Small decision, small change | Do with lever 2 |
| 6 | Overloaded `advance_slice` | Add one named command per phase (`hear_reactions`, `lock_plan`, `hold_final_vote`, `end_quarter`) and keep `advance_slice` for the scripted runner only. At `QuarterOpen` it proposes a default plan on the player's behalf and at the final-vote phase it approves or postpones for them | Medium (Rust, plus replay tests) | Medium: the app no longer sends it unsafely, but the trap remains for any other client |
| 7 | Bad commands accepted, then fail | Reject at the door with a typed reason: `decide` approve with under 2 supporting votes, `propose` above the fundable total, `order_audit` the player can't afford. Today all three are accepted, then fail with "Offline step failed; inspect trial". Check how failed commands are journaled before changing admission, to keep replay deterministic | Small to medium (Rust) | Medium |
| 8 | Default plan spends everything | Not a dead end after all (you can re-plan), so leave the default. The real issue is that re-planning is costly and under-explained, which the app now says on the plan screen. Revisit after lever 1 | None | Low |
| 9 | Delivery forecast is Q1 only | Either send the current programme to `/api/preview` or leave as is; later quarters show per-area start dates | Small | Low |

## Status of the traps I reported earlier

| Trap | Status |
|---|---|
| Approve offered with under 2 supporting votes | **Fixed in the app** (disabled, with the reason). Server still accepts it; see item 7 |
| "Continue" auto-playing your own move | **Fixed in the app**: plan and decide can never use the generic advance (tested). Server command unchanged; item 6 |
| Revise was two engine actions | **Fixed in the app**: one click amends, then locks automatically |
| Live updates missing for a newly started campaign; failure over the stream never reported | **Fixed** |
| "No budget left" and re-planning | **Fixed on the working branch, not yet on main** (see below) |
| Audit shows nothing; paid audit fails with no credits | **Mitigated** in wording only; root causes are items 3 and 4 |
| No goal or score | **Built**: the goal is staying in power (election at the end of each term) with Legacy as the second line; see `decisions/` |

## Corrections to earlier statements

I got two things wrong in the first round of this work and fixed them after testing against the
engine:

1. I wrote that after spending 100 credits in Q1 *"there is nothing new to fund in Q2 to Q6"* and the
   screen said *"no budget left"*. **Wrong.** A new plan replaces undelivered work, so after a
   Balanced Q1 you can re-plan 96 credits. The app now uses the engine's rule (unspent plus
   undelivered), starts a re-plan from your current programme, and warns that delivery delays restart.
2. I said the default plan "leaves later quarters with nothing to decide". Also wrong, for the same reason.
