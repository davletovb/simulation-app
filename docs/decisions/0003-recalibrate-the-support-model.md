# 0003. Recalibrate the public-support model

- **Status:** Accepted and implemented (engine rules **v2**).
- **Date:** 2026-11-01
- **Decided by:** the project owner, in conversation ("Yes to 3": change the frozen engine numbers
  and regenerate the pinned report in the same change). Recorded by Claude.
- **Implemented in:** `1c2dbee` (engine, data, pinned artifacts, tool, contract text) and
  `02c1061` (start-screen handling of old saves).
- **Last commit with the old numbers:** `b117195`.

## Decision

Change the numbers behind the public-support response so that support follows how much work is
delivered and how early, while keeping real winners and losers among the six groups.

| What | Before (rules v1) | After (rules v2) |
|---|---:|---:|
| Benefit per delivered unit: Staffing | 500 | **3200** |
| Benefit per delivered unit: Repairs | 400 | **2800** |
| Benefit per delivered unit: Access | 750 | **3000** |
| Cost per delivered unit (all families) | 400 | **2000** |
| Most one quarter can move one group | ±8,000 ppm | **±40,000 ppm** |
| Most one scripted event can move one group | ±8,000 ppm | **±40,000 ppm** |
| `RULES_VERSION` | `p4-england-education-rules-v1` | `p4-england-education-rules-v2` |
| `authored.json` sha256 | `893aede9ea6d…349d8` | `8af4f2c773fb…a95ee` |
| `narrative-diff.json` sha256 | `ef3ee26c75dc…fba72` | `fcfb57afac26…6c970` |

**Not changed:** the geography and income multipliers, delivery capacity and lags, the 100-credit
budget, how ministers vote, the shock calendar (still all zeros), and the allowed values of
`magnitude_percent` (50, 100, 150: the sensitivity sweep is now ±50% around the new baseline).

Full hashes: v1 `893aede9ea6d6586b08328fae446fb9f3f120337d0e834be1bb00cede32349d8`, v2
`8af4f2c773fbd11bde80c8e2ecbfac30747c1bf88c5ab7c4827807cead2a95ee` (`authored_sha256` in
`data/england-school-pilot-v1/manifest.json`).

## Why

Across ten strategies played to the end against the real service, the ranking by public support
followed the ranking by work delivered with a rank correlation of only **+0.30**. Two quarters late
beat on time, and an Access-only plan beat Balanced. Two causes, in order of importance:

1. **Benefit and cost nearly cancelled.** Every delivered unit paid a flat cost of 400 against a
   benefit of 500 (Staffing) or 400 (Repairs), so Repairs netted about zero.
2. **The effects were tiny and arrived a quarter late.** Overall support stayed between 50.04% and
   50.36% across all 72 accepted sensitivity variants.

**Correction to the proposal (`../stay-in-power.md`).** The proposal listed the ±8,000 clip as a
cause. Measured afterwards, it was not: the largest single-quarter move in the ten old plays was
4,450 ppm, so the clip never bound. It was raised only to leave headroom for the larger numbers.
Under v2 the largest move is 27,400 ppm; a run at scale 150% would reach about 41,100 and be clipped
at 40,000.

### Result, election-night support (the last quarter's effect counted), same ten plays

| Strategy | Work delivered | Before | After |
|---|---:|---:|---:|
| Balanced in Q1 | 100 | 50.00% | **55.96%** |
| Staffing-first in Q1 | 100 | 50.07% | **56.12%** |
| Staffing 45 / Repairs 40 / Access 10 | 94 | 50.01% | 55.57% |
| Balanced a quarter late | 88 | 50.08% | 55.35% |
| Stagger then re-plan | 78 | 50.08% | 54.71% |
| Balanced two quarters late | 66 | 50.16% | 54.10% |
| Access-only 24 + Repairs 35 | 59 | 50.54% | 53.69% |
| Staffing-only 45 | 44 | 49.96% | 52.82% |
| Repairs-only 35 | 35 | 49.75% | 51.69% |
| Never spend | 0 | 50.00% | 50.00% |

Rank correlation with work delivered: **+0.30 before, +0.99 after.** In the engine's own sensitivity
sweep, accepted Q6 support went from 50.04–50.36% to **51.43–58.04%**, and **urban-high and
rural-high finish below 50% in all 72 accepted variants** while the other four groups stay above.
That is the "someone is always left behind" tension, kept on purpose.

## What else changed with it

- `crates/domain/src/reducer.rs`: the numbers are now named, documented public constants
  (`RESPONSE_BASE_PPM`, `RESPONSE_CLIP_PPM`, `SHOCK_CAP_PPM`, `GEOGRAPHY_PERCENT`, `INCOME_PERCENT`).
- `data/england-school-pilot-v1/authored.json` states the same numbers. A new test in
  `crates/contracts/src/checkpoint.rs` fails if the file and the engine ever disagree.
- Regenerated, never hand-edited: `manifest.json` and `narrative-diff.json` (by `build_pack.py
  --write`) and `docs/p3-sensitivity-report.md` plus `p3-sensitivity-results.csv` (by the
  `p3_report` example). CI fails if they differ from what the code produces.
- The sensitivity report's "all 36 balanced/staff-first pairs tie at Q6" finding **still holds**: it is
  structural (both deliver identically through Q5; they differ only in Q6 work whose effect has not
  landed). The report's prose did not need changing.
- Tests: the single-unit sign test now says rural-low *gains* from Staffing (it lost before) and
  urban-high and rural-high lose; one new test pins the clip. Total passing: 157 (was 156).
- `docs/p0-case-contract.md` states the new numbers and links here.
- `tools/support-model/`: a replica of the model that matches the engine to the ppm under both
  calibrations, and `capture_runs.py`, which regenerates the evidence.

## Consequences

- **Games saved under the old numbers cannot be opened.** The rules version and the scenario hash are
  checked on load. Checked directly: opening an old campaign returns 404 and re-creating its name
  returns 409; the files on disk are left untouched. The start screen now explains this and offers a
  fresh name (`02c1061`). This includes any local `demo-england` save from before this change.
- **Ministers vote the same way.** Scripted votes depend on the allocation thresholds (Staffing ≥ 45,
  Repairs ≥ 35) and on a minister's own belief, never on public support. The sensitivity counts
  (72 ratified, 36 rejected, 36 unaffordable) are unchanged.
- **Public support is now meaningful, but the engine does not yet read an election bar.** Nothing
  ends the game on a loss yet (decisions 0002 and 0004; stage B).
- **Balanced still wins comfortably in scripted play.** It clears the provisional bar by about one
  point and there are no events yet, so the best plan is still known in advance. See `../stay-in-power.md`.

## Verified

`cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`
(157 passed); `build_pack.py --check` and its unit tests; `export-contracts --check`; the offline-API and
checkpoint smoke scripts; regenerating the report a second time gives identical files; web tests,
typecheck and build; a full 6-quarter browser playthrough and the edge-case scenarios on the
recalibrated service; and `python3 tools/support-model/support_model.py --check` (60/60 quarter-values
identical under each calibration).

## To revert

Reverting restores the old numbers **and** the old pinned artifacts together, because they were
committed together.

```bash
git revert 1c2dbee        # then, if you want the start-screen message gone too: git revert 02c1061
cargo test --workspace --locked
uv run --project tools/p2 --offline --locked python tools/p2/build_pack.py --check
```

Or restore the files from the last commit with the old numbers without making a revert commit:

```bash
git checkout b117195 -- crates data docs/p3-sensitivity-report.md docs/p0-case-contract.md tools/support-model
```

That restores the old file contents but does not delete files added since (`capture_runs.py`,
`captured_runs_v2.json`); remove them by hand if you want them gone. Prefer `git revert` unless you
need exactly the old tree.

After a revert: games saved under rules **v1** open again, games saved under **v2** stop opening, and
these decision records stay as the history. **Revert any later commit that depends on v2 first**
(for example the stage-B snapshot fields and election rule, if they exist by then).

## To re-tune later

1. Pick numbers with the tool: add an entry such as `v3` to `CALIBRATIONS` in
   `tools/support-model/support_model.py` and read the election table
   (`python3 tools/support-model/support_model.py`). It needs no engine change.
2. Engine: edit `RESPONSE_BASE_PPM`, `RESPONSE_CLIP_PPM`, `SHOCK_CAP_PPM` in
   `crates/domain/src/reducer.rs`.
3. Data: edit `pilot.authoritative_mechanism` in `data/england-school-pilot-v1/authored.json` to match
   (a test fails until it does).
4. Bump `RULES_VERSION` in `crates/hiyal-adapter/src/lib.rs`, so older saves are refused clearly.
5. Regenerate what CI pins:
   ```bash
   uv run --project tools/p2 --offline --locked python tools/p2/build_pack.py --write
   cargo run --locked -p myth-domain --example p3_report
   ```
6. Update `docs/p0-case-contract.md` (the coefficient and clip lines).
7. Capture evidence from the new engine and prove the replica still matches it:
   ```bash
   MYTH_SAVE_DIR=/tmp/myth-capture cargo run --locked -p myth-local-api    # one terminal
   python3 tools/support-model/capture_runs.py v3                          # another terminal
   python3 tools/support-model/support_model.py --check
   ```
8. Add a new numbered decision record that says what changed and why, and mark this one
   "Superseded by 000N" (do not delete it).
9. Run the full checks listed under "Verified".
