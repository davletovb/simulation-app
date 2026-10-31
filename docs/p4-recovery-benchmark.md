# P4-B′ — measured Linux/macOS replay and resource gate

**Execution:** [GitHub Actions review-correction run #41160460004](https://github.com/davletovb/simulation-app/actions/runs/41160460004) **passed**, including Linux x86_64, macOS arm64 and independent comparison, on commit `a0e68d79c751c7542980403a4d09fe45f217875b`. This supersedes the initial [pre-review P4-B′ run #41157622619](https://github.com/davletovb/simulation-app/actions/runs/41157622619). Subsequent type-safe serialization and wrapper cleanups must be validated by final essential CI; see that check before merge.

**Test type:** deterministic offline synthetic verification, not empirical or live-model validation. P4 is a single game-owned world with Hiyal **`050d1de094f5d0813817e2020ec5c5da97d5c42e`**; Rust **1.99.1**; Hiyal durability **`EveryEvent`**, fsync enabled; `default-features = false`, no network/provider/LLM billing. Run the *same* P2 frozen country pack, declared P4 rules-behavior version, case commands, zero-billed stress receipts and trial seed on both operating systems.

## Executed prototype and small stress metrics

All time entries are **milliseconds**, shown **median / p95**, not just a best-case timing. One experiment consists of three full six-quarter runs. Cabinet commits: **108 individually timed minister actions** per scenario (36 per full run); quarter commits: **18** (6 per full run); restart and full read-only integrity verification: **3** each. With n=3, empirical p95 is the **maximum of the three observations**, not a precise tail-latency estimate. The timing-only cabinet probe uses sequential `step()` for individual per-action timing; the operational frozen round uses `run_round(3)`. A dedicated regression checks both paths produce the same private state hash; the separate operational recovery tests cover pending frozen rounds.

| Metric | Linux x86_64 prototype | macOS arm64 prototype | Linux x86_64 stress | macOS arm64 stress |
|---|---:|---:|---:|---:|
| Cabinet action commit (median / p95, ms) | 1.550 / 2.610 | 2.477 / 7.282 | 2.724 / 3.906 | 6.518 / 9.517 |
| Quarter advance commit (median / p95, ms) | 2.665 / 3.970 | 6.468 / 23.401 | 3.355 / 4.409 | 8.125 / 20.169 |
| Resume with full validation (median / p95, ms) | 74.460 / 75.348 | 82.248 / 108.247 | 129.069 / 131.030 | 211.261 / 265.469 |
| Independent full verification (median / p95, ms) | 37.344 / 37.574 | 44.271 / 45.772 | 65.071 / 65.079 | 103.782 / 150.275 |
| Maximum checkpoint size (bytes) | 34,162 | 34,162 | 82,202 | 82,202 |
| Maximum full trial directory size (bytes) | 206,456 | 206,452 | 399,330 | 399,326 |
| Process high-water RSS (bytes) | 6,594,560 | 28,213,248 | 7,290,880 | 28,229,632 |
| Model receipts (offline simulated, cost $0) | 0 | 0 | 96 | 96 |
| Total Hiyal effect receipts (model + player tool) | 12 | 12 | 108 | 108 |
| Accepted logical game actions | 60 | 60 | 60 | 60 |

The 1,024 stress records are **synthetic immutable load fixtures**, not extra population cells or real England demographics. The game retains the same six authored P2 social cells. Both platforms executed 36 logical minister actions, six explicit quarter advances and 96 additional fake model receipts. The 12 other effect receipts are for the side-effect-free player command tool.

### Measured authoritative private state equality

The `compare` job independently checked the two platforms' saved report JSONs (three samples each) and committed-replay hashes:

| Scenario | Linux x86_64 authoritative SHA-256 | macOS arm64 authoritative SHA-256 | Result |
|---|---|---|---|
| Prototype | `ddea8f2657a172dc6ba5e71905053750863c8532f81811d084108a4c532a36f4` | same | **Identical** |
| 1,024-record stress | `c7890b9d1bb0cce6856f024845ea6f6a015389f6291494ffe99730d396fb206d` | same | **Identical** |

Hiyal's entire journal-chain head was *not* compared cross-OS because genesis provenance can carry build/host metadata. The explicitly versioned **authoritative game-state digest** is the declared cross-OS acceptance quantity. Restore and independent read-only replay were checked for **each** sample. The GitHub Actions artifacts preserve the raw prototype/stress JSON and runtime stderr for seven days; this committed report preserves the key measurements thereafter.

### Resource and hardware provenance

- **Linux:** `ubuntu-24.04` GitHub-hosted x86_64; **4 logical CPUs reported by the program**, Rust 1.99.1, process memory peak read from Linux `/proc/self/status:VmHWM`.
- **macOS:** `macos-15` GitHub-hosted **Apple Silicon arm64**, **3 logical CPUs reported by the program**, Rust 1.99.1. The CLI deliberately reports `peak_rss_bytes: null` on non-Linux platforms rather than mislabel current memory as peak. The **28,213,248-byte prototype and 28,229,632-byte stress high-water marks** were measured separately by `/usr/bin/time -l`; both observations were checked in the executed [review-correction compare job](https://github.com/davletovb/simulation-app/actions/runs/41160460004).
- Nominal runner memory/disk capacity is controlled by GitHub images and is **not independently benchmarked** here; refer to [GitHub's hosted-runner hardware specifications](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job) (the cited public hosted runners list Linux 4 CPU / 16 GB and macOS M1 3 CPU / 7 GB, with 14 GB SSD). The **actual trial storage** and peak memory above are independently measured, not those host capacities.
- No absolute performance generalization is implied from n=3 or GitHub's ephemeral shared runner load. For the **declared first-case P0-04 targets**, the observed cabinet p95 was below 150 ms, quarter p95 below 250 ms, prototype restore below 2 s, independent verification below 5 s, prototype storage below 10 MiB, stress storage below 64 MiB, stress restore below 10 s, stress verification below 30 s, and the measured prototype/stress Linux/macOS memory below their applicable 256/512 MiB thresholds. macOS prototype high-water memory was reconstructed from the saved `prototype-stderr.txt` in the executed artifact. Both memory measurements are direct `/usr/bin/time -l` observations, not an inferred ordering between prototype and stress.

## P4-B′ acceptance and inference boundaries

**Result:** software execution, documented cross-OS private-state equality and declared prototype/stress resource tests passed in the checked environment. Hiyal's pinned revision supports the first-case requirements, *provisionally*.

**Not proven:** real model latency, live minister behavior, unknown actual provider spend, exactly-once remote billing, resilience to distributed writers, larger 10²–10⁵ entity scaling, different operating systems, adaptive social calibration, or P5's shared durable budget across campaigns. P1's old world-only checkpoint is still **not** a complete P3 campaign checkpoint; P4's private Hiyal action fold is the authoritative persisted representation for this adapter only. No production website or remote service integration is implied.

## Repeatability and cost controls

The strict, private source boundary is tested by `cargo test --workspace --locked` in the **essential offline CI**. The expensive two-OS measurement is retained as a **manually dispatched** GitHub Actions workflow, not a recurring push/PR requirement. It builds one optimized binary per OS and executes three sample runs of each workload. An independent comparator rejects cross-OS private-hash drift, missing data, an incorrect architecture, loss of Hiyal's *actual configured* `EveryEvent` durability or a failed explicit fsync probe or breached prototype/stress targets. Do not automatically consume additional CI budget repeating these macOS runs for routine documentation changes.
