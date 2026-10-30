# P3-07 fixed-action sensitivity — corrected executed evidence

**Scope:** an authored fictional school-service simulation, not empirical English demographics, behavioral calibration or a causal forecast. All cohort tables and multipliers are authored hypotheses.

## Frozen comparison protocol

- Four Q1 packages: balanced [50,40,10], staff_first [60,30,10], access_first [40,30,20], and oversubscribed [60,40,10]; three P2-authored cohort tables; coefficient scale 50/100/150%; added delivery lag 0/1; response lag 1/2. This is **144 trials**, including every failed adoption. Same scripted actor profile, no policy re-optimization, full effort and exactly six explicitly advanced quarters.
- The Q2 authored repair-cap patch of 8 is pinned unchanged to P2 and all alternatives, but is **inert**: Q1 repairs first become eligible in Q3, so Q2's cap cannot restrict delivery. Do not interpret it as an active shock. The remaining shocks are an explicit all-zero common schedule.
- Ratification is determined by the authoritative reducer, not a duplicate vote-counting algorithm. All comparison pairs are defined once by `accepted_pairs`, excluding rejected or unaffordable policy rows.
- Two endpoints are shown separately: **Q6 authoritative support** (does not include events due after the horizon) and **queue-settled support** (applies only the already produced Q7/Q8 queued responses, in stable due-date/event-ID order, with no invented Q7/Q8 spending, deliveries or shocks). Queue-settled is a derived diagnostic, **not** a changed Q6 score or a prediction of future action.

## Counts and identifiable comparisons

| Metric | Result |
|---|---:|
| Declared variants | 144 |
| Ratified | 72 |
| Rejected by votes | 36 |
| Unaffordable | 36 |
| Eligible balanced/staff-first pairs | 36 |
| **Q6 weighted-support ties** | 36 |
| Q6 non-ties | 0 |
| Queue-settled ties | 18 |
| Queue-settled non-ties | 18 |
| Queue-settled reversals versus non-tied reference | 0 |
| Reference itself tied after settlement? | false |
| Accepted Q6 support range (ppm) | 500383–503603 |
| Accepted queue-settled support range (ppm) | 500013–501161 |

**Interpretation:** identical Q6 support in all 36 pairs is a **tie**, not evidence of ranking stability or zero reversals. Q6 deliveries can differ while the corresponding response is still pending. A reversal is defined only when the pinned authored_baseline/100%/original-lag/one-quarter-response **queue-settled** difference is strictly nonzero and the compared queue-settled difference has the opposite strict sign; tied comparisons are reported separately, never scored as stable ranks.

## Distributional effects and comparison identifiability

| Fictional cell | Q6 min ppm | Q6 max ppm | Accepted Q6 below 500k | Accepted Q6 above 500k | Q6 paired ties | Settled paired ties | Settled reversals* |
|---|---:|---:|---:|---:|---:|---:|---:|
| urban-low | 501795 | 507304 | 0 | 72 | 36 | 18 | 0 |
| urban-middle | 502950 | 512450 | 0 | 72 | 36 | 18 | 0 |
| urban-high | 477276 | 496426 | 72 | 0 | 36 | 18 | 0 |
| rural-low | 502485 | 510454 | 0 | 72 | 36 | 18 | 0 |
| rural-middle | 503850 | 515150 | 0 | 72 | 36 | 18 | 0 |
| rural-high | 480606 | 497175 | 72 | 0 | 36 | 18 | 0 |

*When a pinned reference is tied, reversals are **not identifiable**; the correct diagnostic is the tie count, not a claim of stability. Full per-cell pending deltas and settled support appear in the CSV next to the untouched Q6 values.

## Attribution limits

Response-lag comparisons at Q6 change the **measurement cutoff** as well as the timing: Q6 credits may be spent while their social responses remain queued. Queue-settled comparisons isolate the pending event effect under the same zero-future-action projection; differing timing before Q6 can still affect the path if clamps or other endogenous conditions bind. This authored experiment cannot estimate real beneficiaries' response lags or compare untested counterfactual actions.

## P2-authored population sensitivity

| Table | Urban low/middle/high | Rural low/middle/high |
|---|---|---|
| authored_baseline | 260000 / 320000 / 220000 | 70000 / 80000 / 50000 |
| low_rural_association | 250000 / 340000 / 210000 | 80000 / 60000 / 60000 |
| strong_rural_middle_association | 290000 / 280000 / 230000 | 40000 / 120000 / 40000 |

Every table has authored urban/rural 800k/200k and income 330k/400k/270k parts-per-million margins, not observed England income–residence data. No real-world robustness claim follows.
