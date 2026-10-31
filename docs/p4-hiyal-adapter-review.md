# P4 — Hiyal integration and recovery verification (review packet)

**Branch:** `feat/p4-hiyal-recovery` / PR #11. **Status:** P4 first-case offline implementation and cross-OS B′ acceptance evidence complete; final PR code review remains required. [Review-corrected Linux x86_64 / macOS arm64 run #41160460004](https://github.com/davletovb/simulation-app/actions/runs/41160460004) **passed**, including the independent hash/resource comparison. See the [committed measurement record](p4-recovery-benchmark.md).

## Versioned, replaceable integration boundary

- Reviewed engine revision is **Hiyal `050d1de094f5d0813817e2020ec5c5da97d5c42e`**, pinned as a Cargo Git dependency with `default-features = false` and **no live HTTP**. Hiyal is an execution/recovery engine and never owns the society’s mechanism equations. The dependency is confined to `crates/hiyal-adapter`.
- Pure P3 `myth-domain` remains independent. The game-owned `GameWorld: hiyal::core::World` validates and folds *all accepted typed private actions* from frozen genesis into a complete P3 `Campaign`, rather than persisting only P1's incomplete `WorldSnapshot`. Its bounded private action-fold and snapshots include actual minister effort, meeting/ballot state, role-restricted evidence, funding obligations, audited beliefs, past due-schedule shadows, and private commitment history **by deterministic reconstruction**. There are no model transcripts/LLM prompts in the mutable world.
- The P2 country-pack and authored scenario digests are checked against the frozen `manifest.json`. A **declared `RULES_VERSION` behavior contract** is stored in `rules_identity`. Changing P3 behavior, adapter rules (`q2_repair_cap`), validated parser semantics, or profile policy requires an explicit version bump and migration decision; harmless formatting/comments do not invalidate saved campaigns. Every replay still reconstructs and verifies the private state digest, so unnoticed behavioral drift can fail closed on reconstruction. Trial names, seed and named RNG stream positions are Hiyal genesis data. Each accepted action carries the audited observation boundary SHA and actor.
- Logical actors: player `0`, Finance `1`, Education `2`, Infrastructure `3`, period advance `4`. Players and the period actor are not interchangeable; the latter alone calls the **explicit** `advance_quarter` command. Three ministers act in first/final frozen observations with a separate Hiyal `run_round(3)` for each; no round implies a calendar advance. Hiyal writes scheduler pauses/resumes as journal control records. The role-observation digest is anchored at **round entry** so a second/third frozen actor does not appear stale just because an earlier actor committed.
- Source-only names and concise deterministic rationale text are kept in versioned logical actions, not in public API responses. Strict role authorization and P1 validated Serde command/minister parsing are reused for reconstruction. No new arbitrary command parser is added.

## Receipts, immutable action flow and recovery

`player_command` is the only enabled custom tool effect. It is allowlisted, validates command ID, full typed body, expected world version, stable operation ID and serialized size, and **echoes** its input without changing persistent state. A Hiyal keyed receipt is fsynced before the game-owned World applies the logical action. Any changed or missing effect receipt fails closed. All request settings and identities are recorded in Hiyal's receipt journal.

The only `Model` requests currently permitted are labelled `p4-synthetic-offline` stress **fixtures**, never provider calls. They return deterministic zero-cost synthetic receipts; the P0 stress benchmark requests 96 such synthetic receipts, not measured model usage or billed requests. All other model specs are explicitly unsupported before dispatch.

Hiyal is configured with `EveryEvent` durability. A normal `Trial::resume` checks the full journal and snapshot, and then the adapter verifies its independently reconstructed private `Campaign` fingerprint. `Store::inspect_read_only` provides a second full verification without modifying data. `replay_only=true` **cannot dispatch a new effect**; an already journaled receipt may be reused after a simulated crash between effect resolution and World application. Recovery tests also deliberately interrupt a persisted frozen minister round and replay only the unfinished suffix, checking role IDs and private state.

The Hiyal journal contains the authoritative action and scheduler sequence, and the role/assistant tool journal contains keys and receipt hashes. The adapter includes a finite, bounded private action fold in snapshots to reconstruct otherwise-unserializable P3 private fields. It is not an ever-growing natural-language transcript. **P1 `PrivateCheckpoint` remains a world-snapshot prototype** and must never be advertised as a complete P3 campaign checkpoint.

Do **not** infer exactly-once remote billing from these tests. Hiyal’s `BillingDisposition::Unknown` and the P5 durable shared allowance must conservatively preserve uncertain charges. P4 has no real provider dispatch or credit authorization. Any P6 remote-triggered operation will require separately versioned effect and authorization rules.

## Performance and cross-OS gate (P0-04)

**Executed on 2026-10-31** using Linux x86_64 (`ubuntu-24.04`) and macOS **arm64** (`macos-15`), the same frozen case and Hiyal revision, and Rust 1.99.1: [review-corrected run 41160460004](https://github.com/davletovb/simulation-app/actions/runs/41160460004). Both operating systems reconstructed **identical** `private_state_sha256` values in prototype and 1,024-record stress scenarios. Compare the game-state hash rather than the platform-specific Hiyal journal provenance. Each of three repeats passed strict save/restart and full read-only verification. Full per-OS median/p95, sizes, RSS measurement and output hashes are recorded in [P4-B′ evidence](p4-recovery-benchmark.md). This is bounded first-case evidence, not a statistical or global cross-platform guarantee.

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

The benchmark prints bounded JSON records containing OS, architecture, Rust toolchain, sample counts, individually timed cabinet commits, checkpoint size, whole trial disk footprint, restore/full-verify timings and Linux `/proc/self/status` high-water RSS. On macOS the **separate executed `/usr/bin/time -l` wrapper** measured 28,229,632-byte stress peak RSS; the CLI itself correctly leaves its macOS peak RSS nullable. The declared small-world memory and disk targets were met. The 1,024-record stress is intentionally far smaller than any future 10²–10⁵ parameter scaling study.

## P4-06 adoption decision: retain reviewed Hiyal for first-case alpha

**Decision:** Retain **Hiyal at the reviewed, pinned revision `050d1de094f5d0813817e2020ec5c5da97d5c42e`** as the replaceable offline orchestration/recovery engine for the first scripted England education pilot. This decision rests on **executed** Linux/macOS arm64 equality, a full six-quarter restore and strict audit trail, pending minister-round crash recovery, player receipt reuse, exclusive writer locks, unknown-charge receipt custody, and prototype/stress metrics below P0's declared ceilings. The independent `myth-domain` crate remains the only owner of simulation and actor mechanism equations; Hiyal owns schedule, effects and durable journal machinery.

**Satisfied in P4 (first-case only):** source- and explicitly versioned rule-bound genesis; explicit five-actor phase/order mapping; frozen-round and sequential action equality in the modeled baseline; private, typed command and minister observations/efforts; side-effect-free allowlisted player receipt; `EveryEvent` and verified checkpoint recovery; fault injection at post-receipt/pre-action and mid-round; strict read-only replay without dispatch; 1,024-record 96-fake-receipt stress; identical game-state SHA-256 and bounded timing/memory/storage across both P1-01 operating systems.

**Known limits and separately tracked future work:**

- **P4 checkpoint is a bounded private action-fold (maximum 512 logical game actions)** plus strict verified Hiyal journal, *not* the P1 world-only checkpoint. The private P3 state is reconstructed deterministically on resume; long campaigns and 10²–10⁵ record scaling need separate time/size benchmarking and possibly a versioned compact private schema.
- **No live LLM requests or remote billing were dispatched.** P5 must implement durable application-level shared spending reservations and uncertain-charge carry-forward before any paid call. Hiyal's per-trial journal is not a shared cross-campaign wallet; successful offline receipt recovery does not establish exactly-once remote billing.
- **No publicly authenticated cabinet service or player web API is included.** P6 must add a typed authenticated adapter, strict private/public projection boundary and bounded schema-aware model responses. Non-scripted input must be frozen into an authorized, receipt-bound operation before replay.
- **Only the declared six-quarter case and 1,024 synthetic load records were measured.** The larger scaling study, cross-OS journal-chain byte identity (not a goal here), dynamic actor generation, and human/LLM behavior validation remain outside P4. The `macos-15` prototype and stress high-water RSS readings (28,246,016 and 28,229,632 bytes) were independently recovered from the retained `/usr/bin/time -l` workflow artifacts. Future **manually dispatched** benchmarks check both memory ceilings explicitly.

No generic Hiyal API modification was required for this P4 alpha path. The design-specific frozen-observation anchors, typed game action fold, manually versioned rule behavior, profile selector and country provenance remain in the game adapter. **Do not transfer them into Hiyal** or introduce a game-specific Hiyal fork. If the P5/P6 service reveals a genuinely generic requirement, record it as a separate Hiyal issue with regression tests and a reviewed commit before pin changes.

## Release gate

The successful [review-corrected cross-OS B′ run](https://github.com/davletovb/simulation-app/actions/runs/41160460004) establishes the declared **offline, deterministic P4 gate**. The [essential CI workflow](../.github/workflows/ci.yml) separately validates all workspace Rust tests/Clippy, P2/P3 frozen evidence, API/checkpoint smoke and generated contracts. P4 checkmarks describe this **headless first-case scope only**; live operational persistence is not claimed until P5/P6 service wiring passes its own authorization and budget tests. Review PR #11 before merge.

## PR #11 follow-up review — recovery and reliability fixes (31 October 2026)

The follow-up independent review raised ten comments. The code now contains these corrections, subject to final head/second-OS verification:

- **Last-minister crash:** `advance_slice` drains any persisted `pending-round.json` with Hiyal `run_round` *before* attempting a scheduler change, even if the world has already reached the next phase. The regression restores a valid completed plan file to model a crash between final commit and unlink.
- **Failed-seat retry:** after a fully processed round with rejected actions, `remaining_ministers` restricts the next attempt to seats with no accepted action. The regression simulates an Education receipt failure while Finance/Infrastructure commit and verifies the retry commits only Education with no extra minister rejections.
- **Source identity:** raw text hashing (which invalidated campaigns for documentation changes) was replaced with a declared, manually bumped `RULES_VERSION`; the existing exact private action-fold digest remains independently replay-validated. The pinned P2 content hashes are cached with `OnceLock` and still verified against the frozen manifest at startup. A future behavioral edit must intentionally bump this constant.
- **Durability evidence:** benchmark startup verifies the **actual Hiyal `Store::durability()`** is `EveryEvent`, executes `sync_log()`, and records `explicit_sync_probe`. The cross-OS gate checks this receipt, not a hardcoded `fsync: true` field. This confirms configuration and an actual fsync probe, not proof that each historical kernel syscall executed.
- **Performance and cloning:** one validated `Campaign` is reused for `observe` and `apply`; rule admission occurs before mutation, without an additional entire `GameWorld` clone. Frozen observation boundaries replay the relevant action prefix without cloning 1,024 stress fixtures. The full action fold remains history-linear *per independent check*, giving quadratic total replay work over a growing run; the 512-action explicit cap and measured first-case resource gate limit that cost. Future long campaigns would benefit from a separately versioned compact private checkpoint.
- **Scheduler isolation:** the process-wide `PAUSE_CACHE` mutex was removed. `GameTrial` owns its pause state alongside the Hiyal trial, reconstructed once through strict read-only inspection on resume, with each durable pause/resume mirrored only upon success. There is no cross-trial global lock/poisoning. Wrapper-level `pause_actor` and `resume_actor` methods also maintain this mirror for direct callers.
- **Typed phases and normalization:** the game world, observations, phase transitions and benchmark branching use `myth_contracts::checkpoint::Phase`, while minister/vote payloads now use the typed `Ministry`/`Vote` contract enums. One `current_meeting_actions` helper defines vote counting and seat duplication checks. `create`/`create_stress` share the same durable trial constructor. The duplicate observation equality guard and impossible `run_round(3)` length guard were removed.
- **Verification:** the added frozen vs individually stepped cabinet equivalence test, last-committed plan cleanup regression, partial rejected-seat retry regression and changed-player-action rejection ran under the workspace Rust tests. The [fresh review-corrected Linux/macOS arm64 replay and measurement run #41160460004](https://github.com/davletovb/simulation-app/actions/runs/41160460004) **passed**. Both operating systems reconstructed the **updated** prototype SHA-256 `ddea8f2657a172dc6ba5e71905053750863c8532f81811d084108a4c532a36f4` and 1,024-record stress SHA-256 `c7890b9d1bb0cce6856f024845ea6f6a015389f6291494ffe99730d396fb206d` and stayed below P0 resource limits. Final essential CI on the exact reviewed head remains separately required.

**Limit:** this correction does not create live provider billing, a P5 application wallet, a web-facing cabinet, or empirically validated social response. PR #11 remains unmerged for independent review.
