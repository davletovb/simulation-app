# P4 — Hiyal integration and recovery verification (review packet)

**Branch:** `feat/p4-hiyal-recovery` / PR #11. **Status:** implementation in progress; no P4 gate or cross-OS metric is accepted without executed evidence.

## Versioned, replaceable integration boundary

- Reviewed engine revision is **Hiyal `050d1de094f5d0813817e2020ec5c5da97d5c42e`**, pinned as a Cargo Git dependency with `default-features = false` and **no live HTTP**. Hiyal is an execution/recovery engine and never owns the society’s mechanism equations. The dependency is confined to `crates/hiyal-adapter`.
- Pure P3 `myth-domain` remains independent. The game-owned `GameWorld: hiyal::core::World` validates and folds *all accepted typed private actions* from frozen genesis into a complete P3 `Campaign`, rather than persisting only P1's incomplete `WorldSnapshot`. Its bounded private action-fold and snapshots include actual minister effort, meeting/ballot state, role-restricted evidence, funding obligations, audited beliefs, past due-schedule shadows, and private commitment history **by deterministic reconstruction**. There are no model transcripts/LLM prompts in the mutable world.
- The P2 country-pack and authored scenario digests are checked against the frozen `manifest.json`. The actual P3 domain source bytes are hashed into `rules_identity`; changing them invalidates old state until a deliberate migration. Trial names, seed and named RNG stream positions are Hiyal genesis data. Each accepted action carries the audited observation boundary SHA and actor.
- Logical actors: player `0`, Finance `1`, Education `2`, Infrastructure `3`, period advance `4`. Players and the period actor are not interchangeable; the latter alone calls the **explicit** `advance_quarter` command. Three ministers act in first/final frozen observations with a separate Hiyal `run_round(3)` for each; no round implies a calendar advance. Hiyal writes scheduler pauses/resumes as journal control records. The role-observation digest is anchored at **round entry** so a second/third frozen actor does not appear stale just because an earlier actor committed.
- Source-only names and concise deterministic rationale text are kept in versioned logical actions, not in public API responses. Strict role authorization and P1 validated Serde command/minister parsing are reused for reconstruction. No new arbitrary command parser is added.

## Receipts, immutable action flow and recovery

`player_command` is the only enabled custom tool effect. It is allowlisted, validates command ID, full typed body, expected world version, stable operation ID and serialized size, and **echoes** its input without changing persistent state. A Hiyal keyed receipt is fsynced before the game-owned World applies the logical action. Any changed or missing effect receipt fails closed. All request settings and identities are recorded in Hiyal's receipt journal.

The only `Model` requests currently permitted are labelled `p4-synthetic-offline` stress **fixtures**, never provider calls. They return deterministic zero-cost synthetic receipts; the P0 stress benchmark requests 96 such synthetic receipts, not measured model usage or billed requests. All other model specs are explicitly unsupported before dispatch.

Hiyal is configured with `EveryEvent` durability. A normal `Trial::resume` checks the full journal and snapshot, and then the adapter verifies its independently reconstructed private `Campaign` fingerprint. `Store::inspect_read_only` provides a second full verification without modifying data. `replay_only=true` **cannot dispatch a new effect**; an already journaled receipt may be reused after a simulated crash between effect resolution and World application. Recovery tests also deliberately interrupt a persisted frozen minister round and replay only the unfinished suffix, checking role IDs and private state.

The Hiyal journal contains the authoritative action and scheduler sequence, and the role/assistant tool journal contains keys and receipt hashes. The adapter includes a finite, bounded private action fold in snapshots to reconstruct otherwise-unserializable P3 private fields. It is not an ever-growing natural-language transcript. **P1 `PrivateCheckpoint` remains a world-snapshot prototype** and must never be advertised as a complete P3 campaign checkpoint.

Do **not** infer exactly-once remote billing from these tests. Hiyal’s `BillingDisposition::Unknown` and the P5 durable shared allowance must conservatively preserve uncertain charges. P4 has no real provider dispatch or credit authorization. Any P6 remote-triggered operation will require separately versioned effect and authorization rules.

## Performance and cross-OS gate (P0-04)

Run deliberately on Linux x86_64 and macOS **arm64**, with the same reviewed Git revision and Rust 1.99.1. Compare the adapter's **`private_state_sha256`** across OS, not the full Hiyal journal chain head, because Hiyal's genesis includes target-specific build provenance. For each OS, report measured median/p95 and sample counts. Do not infer a cross-OS guarantee from compilation or from one Linux test.

| Metric | Prototype target | 1,024-record stress target |
|---|---:|---:|
| Cabinet action commit, p95 | ≤150 ms | report measured |
| Quarter advance commit, p95 | ≤250 ms | report measured |
| Checkpoint bytes | ≤10 MiB | report measured |
| Restore | ≤2 s | ≤10 s |
| Full independent verification | ≤5 s | ≤30 s |
| Peak RSS | ≤256 MiB | ≤512 MiB |
| Total journal/checkpoint storage | ≤10 MiB | ≤64 MiB |

The stress case is **1,024 synthetic immutable world cohort records**, 3 ministers, 36 logical minister actions, 6 period advances and 96 zero-cost synthetic `Model` receipts. These records are a **load fixture**, not an additional statistically calibrated population or a replacement for P3's six modeled cells. The prototype exercises the six-quarter England baseline. The larger 10²–10⁵ record scaling study remains intentionally deferred.

CLI commands (offline, no provider credentials):

```sh
cargo +1.99.1 test --locked -p myth-hiyal-adapter
cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p4-trial -- run /tmp/p4-case
cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p4-trial -- inspect /tmp/p4-case
cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p4-trial -- benchmark 3
cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p4-trial -- benchmark-stress 3
```

The benchmark prints bounded JSON records containing OS, architecture, Rust toolchain, sample counts, commit times, snapshot size, full-disk footprint, restore/full-verify timing and Linux `/proc/self/status` high-water RSS. On macOS, peak RSS is **not measured** by this CLI yet; do not misrepresent a current-RSS sample as peak. Until macOS peak is measured independently, that numeric acceptance target remains open.

## Adoption decision (provisional)

The first-case application needs strict source binding, one writer, frozen cabinet rounds, receipt custody, deterministic redo, failure inspection and replay without dispatch. Hiyal's pinned APIs provide the underlying trial, schedule, event journal, `EffectJournal`, `EveryEvent`, and read-only full verification. The game adapter owns P3 rule application and a bounded authoritative action-fold. **The replaceable pure-domain boundary is preserved**; there is no need to move social or minister decisions into Hiyal.

The adoption is *provisional*, not a global endorsement, until the exact pinned revision passes the Linux/macOS replay identity gate and P0 prototype/stress targets. Remaining generic engine needs should be filed against Hiyal separately, with no speculative game-side forks. P5/P6/P7 integration, provider metering, web UI authentication and live cabinet behavior are separate, **not** P4 evidence.
