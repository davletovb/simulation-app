# P2 evidence, population and scenario audit — England pilot

**Case:** `england-school-delivery-v1` (fictional Education Delivery Board). **Data cutoff:** 2026-10-29T23:59:59Z. **Source capture:** 2026-10-30T14:39:49Z (after the declared evidence cutoff). **Status:** Frozen **headline-based** country pack with reproducible offline source-byte custody; historical observations and authored game mechanics are distinct. Not empirical calibration or a forecast.

## Source, licence and geographic coverage

The input sources are three official Department for Education (DfE) *release pages*, not an underlying complete dataset export:

| Source | Reported reference | Published; last update by cutoff | Archived input |
|---|---|---|---|
| [School workforce in England, 2025](https://explore-education-statistics.service.gov.uk/find-statistics/school-workforce-in-england/2025) | November 2025 staff census | 4 June 2026; 17 August 2026 | `raw/workforce.html.gz` |
| [Schools, pupils and their characteristics, 2025/26](https://explore-education-statistics.service.gov.uk/find-statistics/school-pupils-and-their-characteristics/2025-26) | January 2026 school census | 4 June 2026; 16 October 2026 | `raw/pupils.html.gz` |
| [School funding statistics, financial year 2025-26](https://explore-education-statistics.service.gov.uk/find-statistics/school-funding-statistics/2025-26) | 2026-27 *allocations* included in 2025-26 edition | 29 January 2026; 29 July 2026 | `raw/funding.html.gz` |

Original downloaded HTML bytes are gzip-compressed losslessly with `mtime=0`. `raw-source-manifest.json` records each original byte length and SHA-256, the compressed-file SHA-256, URL, declared update date and the after-cutoff capture timestamp. Every offline compilation decompresses and independently hashes **both** forms, confirms the update date, checks the exact raw headline value tokens in the frozen pages, and rejects any source drift. No network access, refresh or implicit source substitution is possible in `build_pack.py --check`. The archived pages were fetched on 30 October; publication/update metadata, not a historical cryptographic timestamp, establish that the chosen releases precede the cutoff. We cannot infer that unarchived intermediate editions had identical content.

All three archived pages contain an Open Government Licence notice, checked by the compiler. Attribution: **Contains public sector information licensed under the Open Government Licence v3.0** ([licence](https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/)). Reuse is subject to third-party exclusions and no implied DfE endorsement. The frozen HTML is source custody, not in-game UI assets; don't reproduce official crests/logos or third-party media as game branding. Licence information and original source URLs are retained with every evidence record.

### Eight observed headline inputs

| Stable variable | Reported value | Normalized | Source | Coverage caveat |
|---|---:|---:|---|---|
| `teacher_fte` | 466,372 FTE | 466372 FTE | Workforce | All state-funded school teachers, not all England workers |
| `teaching_assistant_fte` | 295,183 FTE | 295183 FTE | Workforce | Role distinct from teacher FTE |
| `pupil_teacher_ratio` | 17.9 | 17.9 pupils/teacher | Workforce | Pupil/teacher data use differing census dates |
| `teacher_vacancy_rate` | 0.4% | 4000 ppm | Workforce | Rounded percentage; not a teacher headcount |
| `school_count` | 24,499 | 24499 schools | Pupils | Includes independent schools; different workforce scope |
| `fsm_eligibility_rate` | 26.5% | 265000 ppm | Pupils | FSM eligibility, **not** an observed income category |
| `allocated_funding_per_pupil_2026_27` | GBP 8,580 | GBP 8580/pupil | Funding | Cash allocations, not realized spend; estimates |
| `allocated_funding_total_2026_27` | GBP 65.9bn | GBP 65900000000 | Funding | Rounded allocated revenue, not capital or pilot credits |

`values.json` stores each displayed raw token, string-valued normalized number, transformation rule and quality flags. `country-pack.json` implements the existing P1 Serde `CountryPack` **provenance** schema. Its 12 stable IDs include these eight measures and four explicitly missing inputs: beneficiary residence–income joint distribution, teacher/parent overlap, pilot repair backlog, and pilot access gaps. All 12 `sha256_hex` fields now name **source HTML original-byte digests**, not digests of our transcriptions. The values are joined by immutable ID, rather than silently changing the existing P1 wire schema. `manifest.json` pins every input and generated output hash.

**Not comparable without adjustment:** school count includes independent schools, workforce and pupils use different universes/census dates, funding includes age/sector eligibility rules and estimate components, and revenue allocations do not represent capital repair need. No equality between 466372 × 17.9, the all-school pupil total, and funding recipient totals is implied. The funding headline values are not the economic interpretation of the 100 fictional pilot credits.

## Six-cell population accounting and uncertainty

The modeled universe is a **normalized fictional school-service beneficiary population**, exactly 1,000,000 parts per million, not a sample, reconstruction of pupils or census population. Canonical order: urban-low, urban-middle, urban-high, rural-low, rural-middle, rural-high.

| Construction | Urban low/mid/high | Rural low/mid/high | Urban/rural margins | Low/mid/high margins |
|---|---|---|---|---|
| Authored baseline | 260k / 320k / 220k | 70k / 80k / 50k | 800k / 200k | 330k / 400k / 270k |
| Low-rural association | 250k / 340k / 210k | 80k / 60k / 60k | 800k / 200k | 330k / 400k / 270k |
| Rural-middle association | 290k / 280k / 230k | 40k / 120k / 40k | 800k / 200k | 330k / 400k / 270k |

All these margins are **authored**. The only empirical constraints genuinely supported for this fictional cohort model are nonnegative valid cells and a chosen normalized sum. No compatible observed England school-beneficiary residence × income tabulation, geographic margin, income-band margin, teacher-linked/parent-linked cross-tab or school access backlog is asserted. The two alternatives intentionally hold the *authored* margins fixed to isolate only joint-association effects in P3-07. They **do not** claim to preserve an unobserved English margin. The case excludes unmatched census populations and does not invent missing association estimates.

Two overlapping affiliations, likewise authored: 5% of every cell teacher-linked, 25% parent/carer-linked, 2% both. This partitions each cell into **3% teacher-only, 23% parent-only, 2% both, 72% neither**. Memberships are indicators on the SAME modeled beneficiaries and never add to population size. Each selected cell weight is divisible by 10,000, so all four category counts are exact integers in ppm with no residual or extra people. The offline compiler checks nonnegativity, 1,000,000 total, authored margins, overlap consistency and exact affiliation accounting for every construction.

P3-07 should compare these joint constructions with identical policies, exogenous shocks, implementation choices and delivered-credit accounting, and state clearly that this is *authored mechanism sensitivity*, not inference about England.

## Scenario authoring, overlay and news

`authored.json` is the immutable, fictional campaign baseline: Q1 2027–Q2 2028, six quarterly decisions, 100 **lifetime** hypothetical credits, no borrowing or GBP conversion; finance/education/infrastructure fictional seats with 2/3 support, authorized capacities and minister-specific private efforts; scripted organization resources/actions; coefficient, lag and audit assumptions; five menu entries (three feasible policies, invalid oversubscription and defer). Initial enforceable commitments and authoritative registered initial promises are **empty**; opening political statements are not registry commitments or spend authority. Authored support starts at 500000 ppm and is not a poll.

`narrative-diff.json` ties the unmodified baseline scenario and country-pack SHA-256 to one precisely scoped **fictional** overlay: in Q2 only, reduce repair throughput cap from 10 to 8. The overlay must apply to a *copy*, never mutate the baseline, source data, or already-created run snapshot. `news-bundle.json` contains two short dated release-update summaries with source links and a separately marked 2027 fictional bulletin; no full copyrighted articles or invented historical reports. No future real news crosses the cutoff.

## Rebuild / gate checks

From repository root (Python 3.13 and `uv` 0.10.0):

```sh
uv sync --project tools/p2 --offline --locked --no-install-project
uv run --project tools/p2 --offline --locked python tools/p2/build_pack.py --check
# Only when deliberately changing authored inputs and reviewing output changes:
uv run --project tools/p2 --offline --locked python tools/p2/build_pack.py --write
cargo +1.99.1 test --workspace --locked
```

The existing single essential CI job runs the pinned Python offline check plus Rust `parse_country_pack` regression, strict Rust/TS checks, keyless local API and archive smoke, and secret scans. All source-asset downloads are **authoring-only**: `freeze_sources.py` documents how this *one-time* capture was performed, but is never invoked at runtime, build, or normal CI. Normal game loading must pin the pack hash and not refresh; P4 enforces ongoing-run replay binding.

**Acceptance interpretation:** P2 provides a source-backed frozen *headline* case, not full underlying CSV archives or an observed joint survey distribution. The original publication pages used to take these eight inputs are retained and hashed, making the limited scope reproducible. If future work needs disaggregated series or an empirically weighted joint model, source and assumptions must be amended under a new pack version, not silently substituted.
