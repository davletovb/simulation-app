# P3-07 offline fixed-action sensitivity — executed evidence

**Scope:** synthetic England school-service beneficiary game only. Not English population calibration, real minister predictions, or an estimate of policy efficacy. All three joint tables and residence/income margins are **authored**; the original official DfE headline records do not identify them.

## Registration and frozen comparisons

- Source inputs: P2 `england-school-delivery-v1`, three authored joint tables and case configuration; Rust rules: `myth-domain::reducer` and `myth-domain::sensitivity`.
- Policy order: balanced [50,40,10], staff_first [60,30,10], access_first [40,30,20], oversubscribed [60,40,10]. One Q1 proposal, all 3 first positions, all 3 final positions with fixed full effort (1000 permille), one Q1 ratification when votes admit, then exactly six explicit advances. No subsequent actor re-optimization.
- Grid: 3 magnitude multipliers (50/100/150%), extra delivery lag (0/1 quarter), response lag (1/2 quarters), three P2 authored population variants, four packages. Total 144 frozen runs, with identical all-zero authored exogenous shocks and Q2 fictional repairs-cap overlay at 8; this is a fixed neutral shock schedule, not a claim shocks never happen.
- Every variant is retained even if not ratified; shared oldest-funded-first capacity is used in every accepted run. The independent profile-only selector is used on the same permitted role view. No API/model calls.

## Outcome counts and ranking check

| Metric | Result |
|---|---:|
| Declared trials | 144 |
| Ratified | 72 |
| Vote-rejected (access_first) | 36 |
| Treasury-rejected (oversubscribed) | 36 |
| Valid balanced-versus-staff_first paired comparisons | 36 |
| Weighted-support rank reversals versus reference | 0 |
| Minimum accepted weighted support (ppm) | 500383 |
| Maximum accepted weighted support (ppm) | 503603 |

The reference ranking is the authored_baseline population, 100% response coefficients, original delivery lags and one-quarter response lag. A reversal is defined as a strict sign change in **balanced weighted support minus staff_first weighted support**; ties are not counted as reversals. Other metrics (delivered units, outstanding encumbrances, six cell-level support values and minister relationship scores) are recorded for every variant in [the complete results table](../data/england-school-pilot-v1/p3-sensitivity-results.csv), not silently omitted.

## Empirical and identification limits

The sweep is numerical sensitivity within an authored deterministic mechanism, not causal identification or validation of any real England policy. No observed school-beneficiary joint income-by-residence distribution or response elasticities are available. The program changes **only** the declared model knobs and weights, not actor choices or shocks. Policy rankings outside these magnitudes/delays/tables remain unexamined. Infeasible and vote-rejected proposals have no policy spending; social responses remain bounded. The all-zero shock schedule is an explicit controlled experiment assumption.

## Populations held fixed or varied

| Authored table | Urban low/middle/high | Rural low/middle/high |
|---|---|---|
| authored_baseline | 260000 / 320000 / 220000 | 70000 / 80000 / 50000 |
| low_rural_association | 250000 / 340000 / 210000 | 80000 / 60000 / 60000 |
| strong_rural_middle_association | 290000 / 280000 / 230000 | 40000 / 120000 / 40000 |

All tables retain the **authored** 800k/200k urban/rural and 330k/400k/270k low/middle/high margins and exactly 1,000,000 ppm. They are not verified DfE demographic margins.
