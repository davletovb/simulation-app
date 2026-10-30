# P2 England evidence and scenario review — partial implementation

Status: **partial, not accepted**. Baseline cutoff 2026-10-29 UTC. Data twin: United Kingdom selector, England school-service jurisdiction, `en-GB`, six fictional quarterly periods from 2027 Q1. This is a game, not England government policy or a forecast.

## Current source coverage

Four **manually transcribed release headline** observations are frozen at `data/england-school-pilot-v1/observations.json`: 466,372 FTE teachers, 17.9 pupil–teacher ratio and 0.4% vacancy rate from the DfE 2025 [school workforce release](https://explore-education-statistics.service.gov.uk/find-statistics/school-workforce-in-england/2025), plus 26.5% free-school-meal eligibility from the [2025/26 pupils release](https://explore-education-statistics.service.gov.uk/find-statistics/school-pupils-and-their-characteristics/2025-26). The latter is **not** an observed low-income-band share. Census dates, eligible school scopes and denominators differ. The 2026-10-30 transcription/retrieval occurred **after** the cutoff; evidence release/update must be separately checked against the cutoff. Do **not** treat this as a full raw source archive or an independently revalidated historical version of each web page.

The evidence bundle is **not yet** the complete P1-compatible `CountryPack`: full stable-variable provenance and per-source SHA-256, raw and normalized values with confirmed source licensing, archived source downloads, release history, and 6–10 **supported nonmissing** indicators remain outstanding. There is no automatic live refresh or download.

## Population construction and validity

Exactly six modeled beneficiary cells in canonical order `urban_low, urban_middle, urban_high, rural_low, rural_middle, rural_high`, all in normalized ppm. Baseline `[260000, 320000, 220000, 70000, 80000, 50000]`; alternatives `[220000, 270000, 180000, 110000, 130000, 90000]` and `[300000, 260000, 240000, 30000, 140000, 30000]`. Each totals 1,000,000. **None** is inferred from observed pupil residence or income distributions. Alternative tables preserve only total and nonnegativity (not an observed urban/rural or income marginal). Thus P3-07 must either justify additional observed margins or remain explicit about purely authored sensitivity.

Teacher-linked 50,000 ppm, parent/carer-linked 250,000 ppm, and both 20,000 ppm per cell are **authored** overlap assumptions, not census data. The sum of the memberships must not be added to beneficiaries or treated as disjoint. No observed survey associations are inferred. FSM eligibility and national resident rural shares must not be imported as a substitute joint distribution.

## Authored scenario and separation

`authored.json` fixes the fictional board, three minister profiles, three policy families, candidate package menu (including an invalid oversubscribed case), six-quarter 100-credit stock with no GBP exchange rate, synthetic organization behavior, and one Q2 repair-capacity counterfactual. All minister positions, mechanisms, claims about support, and narrative events are invented. There are no initially funded enforceable commitments. Its news items are tagged `real_source_update` or `fictional_simulation_news`; none contains copied full news text. The narrative patch changes only a simulated parameter in one run, never the evidence snapshot.

## Validation and blockers

Offline Python 3.13, stdlib-only: `uv --project tools/p2 sync --offline --locked`; `python3 tools/p2/build_pack.py --check`. The program validates six-cell total, overlap, menu, source link shapes, cutoff labels and baseline–overlay separation, and can produce a SHA-256 partial-manifest with `python3 tools/p2/build_pack.py`. **It does not build a complete P1 country-pack or validate a source licence**. It makes no network or LLM calls.

P2-01: **partial** (provenance field requirements exist in P1, actual full pack missing). P2-02: **blocked** by complete raw source downloads, verified terms, and 6–10 compatible nonmissing indicators. P2-03: **partial** (explicit six-cell assumptions and alternatives, no observed marginals). P2-04: **draft** (fictional typed case values; not integrated into a pure reducer). P2-05: **draft** (explicit overlay and typed news; review and completed P2 gate pending). Do not check these items or enter P3's empirical/sensitivity gate as if supported by data.
