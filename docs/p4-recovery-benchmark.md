# P4-B′ — measured Linux/macOS replay and resource gate

**Execution:** [GitHub Actions run 41157622619](https://github.com/davletovb/simulation-app/actions/runs/41157622619), **passed** in all three jobs (Linux x86_64, macOS arm64, independent comparison) on branch source commit `9e85207e311c2444a6995faeb60080fba7f6dccf`. Subsequent P4 formatting and test-only changes do not alter P3 model equations or reviewed Hiyal revision; see the separately required final essential CI result before merge.

**Test type:** deterministic offline synthetic verification, not empirical or live-model validation. P4 is a single game-owned world with Hiyal **`050d1de094f5d0813817e2020ec5c5da97d5c42e`**; Rust **1.99.1**; Hiyal durability **`EveryEvent`**, fsync enabled; `default-features = false`, no network/provider/LLM billing. Run the *same* P2 frozen country pack, P3 rule-source fingerprint, case commands, zero-billed stress receipts and trial seed on both operating systems.

## Executed prototype and small stress metrics

All time entries are **milliseconds**, shown **median / p95**, not just a best-case timing. One experiment consists of three full six-quarter runs. Cabinet commits: **108 individually timed minister actions** per scenario (36 per full run); quarter commits: **18** (6 per full run); restart and full read-only integrity verification: **3** each. With n=3, empirical p95 is the **maximum of the three observations**, not a precise tail-latency estimate. The timing-only cabinet probe uses sequential `step()` for individual per-action timing; the operational frozen round uses `run_round(3)`. A dedicated regression checks both paths produce the same private state hash; the separate operational recovery tests cover pending frozen rounds.

| Metric | Linux x86_64 prototype | macOS arm64 prototype | Linux x86_64 stress | macOS arm64 stress |
|---|---:|---:|---:|---:|
| Cabinet action commit (median / p95, ms) | 3.882 / 5.933 | 8.516 / 13.000 | 5.426 / 7.712 | 12.829 / 17.551 |
| Quarter advance commit (median / p95, ms) | 5.318 / 12.229 | 13.862 / 18.802 | 6.009 / 7.833 | 13.022 / 20.530 |
| Resume with full validation (median / p95, ms) | 111.049 / 113.279 | 261.571 / 264.079 | 143.157 / 145.214 | 317.480 / 318.770 |
| Independent full verification (median / p95, ms) | 109.976 / 115.312 | 250.026 / 294.530 | 141.977 / 143.656 | 293.941 / 301.647 |
| Maximum checkpoint size (bytes) | 34,197 | 34,197 | 82,237 | 82,237 |
| Maximum full trial directory size (bytes) | 206,526 | 206,522 | 399,592 | 399,588 |
| Process high-water RSS (bytes) | 6,627,328 | 28,246,016 | 6,881,280 | 28,229,632 |
| Model receipts (offline simulated, cost $0) | 0 | 0 | 96 | 96 |
| Total Hiyal effect receipts (model + player tool) | 12 | 12 | 108 | 108 |
| Accepted logical game actions | 60 | 60 | 60 | 60 |

The 1,024 stress records are **synthetic immutable load fixtures**, not extra population cells or real England demographics. The game retains the same six authored P2 social cells. Both platforms executed 36 logical minister actions, six explicit quarter advances and 96 additional fake model receipts. The 12 other effect receipts are for the side-effect-free player command tool.

### Measured authoritative private state equality

The `compare` job independently checked the two platforms' saved report JSONs (three samples each) and committed-replay hashes:

| Scenario | Linux x86_64 authoritative SHA-256 | macOS arm64 authoritative SHA-256 | Result |
|---|---|---|---|
| Prototype | `cba6fce0972ce1b3944d9816d5fbb234b99b8bef9fb14e9f78abe188faa7a131` | same | **Identical** |
| 1,024-record stress | `fd121a8cb2f6c3c4320fddbaa972e319835f61261ee39699be8ca99b6a65df92` | same | **Identical** |

Hiyal's entire journal-chain head was *not* compared cross-OS because genesis provenance can carry build/host metadata. The source-fingerprinted **authoritative game-state digest** is the declared cross-OS acceptance quantity. Restore and independent read-only replay were checked for **each** sample. The GitHub Actions artifacts preserve the raw prototype/stress JSON and runtime stderr for seven days; this committed report preserves the key measurements thereafter.

### Resource and hardware provenance

- **Linux:** `ubuntu-24.04` GitHub-hosted x86_64; **4 logical CPUs reported by the program**, Rust 1.99.1, process memory peak read from Linux `/proc/self/status:VmHWM`.
- **macOS:** `macos-15` GitHub-hosted **Apple Silicon arm64**, **3 logical CPUs reported by the program**, Rust 1.99.1. The CLI deliberately reports `peak_rss_bytes: null` on non-Linux platforms rather than mislabel current memory as peak. The **28,246,016-byte prototype and 28,229,632-byte stress high-water marks** were measured separately by `/usr/bin/time -l`; the stress result was checked by the executed compare job, and the prototype result was subsequently read directly from the retained [macOS workflow artifact](https://github.com/davletovb/simulation-app/actions/runs/41157622619) without another runner or an estimate.
- Nominal runner memory/disk capacity is controlled by GitHub images and is **not independently benchmarked** here; refer to [GitHub's hosted-runner hardware specifications](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job) (the cited public hosted runners list Linux 4 CPU / 16 GB and macOS M1 3 CPU / 7 GB, with 14 GB SSD). The **actual trial storage** and peak memory above are independently measured, not those host capacities.
- No absolute performance generalization is implied from n=3 or GitHub's ephemeral shared runner load. For the **declared first-case P0-04 targets**, the observed cabinet p95 was below 150 ms, quarter p95 below 250 ms, prototype restore below 2 s, independent verification below 5 s, prototype storage below 10 MiB, stress storage below 64 MiB, stress restore below 10 s, stress verification below 30 s, and the measured prototype/stress Linux/macOS memory below their applicable 256/512 MiB thresholds. macOS prototype high-water memory was reconstructed from the saved `prototype-stderr.txt` in the executed artifact. Both memory measurements are direct `/usr/bin/time -l` observations, not an inferred ordering between prototype and stress.

## P4-B′ acceptance and inference boundaries

**Result:** software execution, documented cross-OS private-state equality and declared prototype/stress resource tests passed in the checked environment. Hiyal's pinned revision supports the first-case requirements, *provisionally*.

**Not proven:** real model latency, live minister behavior, unknown actual provider spend, exactly-once remote billing, resilience to distributed writers, larger 10²–10⁵ entity scaling, different operating systems, adaptive social calibration, or P5's shared durable budget across campaigns. P1's old world-only checkpoint is still **not** a complete P3 campaign checkpoint; P4's private Hiyal action fold is the authoritative persisted representation for this adapter only. No production website or remote service integration is implied.

## Repeatability and cost controls

The strict, private source boundary is tested by `cargo test --workspace --locked` in the **essential offline CI**. The expensive two-OS measurement is retained as a **manually dispatched** GitHub Actions workflow, not a recurring push/PR requirement. It builds one optimized binary per OS and executes three sample runs of each workload. An independent comparator rejects cross-OS private-hash drift, missing data, an incorrect architecture, loss of `EveryEvent`/fsync or breached prototype/stress targets. Do not automatically consume additional CI budget repeating these macOS runs for routine documentation changes.
