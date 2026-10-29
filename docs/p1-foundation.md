# P1 — Rust foundation slice (partial implementation)

**P0 baseline:** [England education pilot contract](p0-case-contract.md). **Scope:** first executable code only; **P1 is NOT completed** by this slice.

## Added

- A dependency-free Rust workspace with `crates/domain`, locked to Rust **1.99.1** (edition 2024).
- Pure bounded types for fictional 100-credit pilot allocations, treasury invariant, six cohort keys/weights, ministry roles, proposals, commitments, typed actions and role observations.
- Deterministic **family-wide shared-capacity** delivery helper, with multiple funded scopes, stable oldest-funded-first ordering, checked fractional carry per immutable scope, explicit termination/discard receipts, staffing/repair/access maxima and 0–1000 permille effort validation.
- Canonical decimal-string parser for full-range `u64` wire values, used by persistent/wire version fields. The actual Serde JSON representation and JSON Schema remain P1-03 and must serialize the wrapper as **string**, not default numeric encoding.
- One Linux CI job with format/Clippy, **12 Rust unit tests**, fail-closed deterministic-source scan across Rust sources/tests, and credential-pattern scan across tracked project files. PR-only superseded runs can be cancelled; main runs are not cancelled. The check runs for docs-only PRs, skipping Rust commands when a branch has no Cargo workspace. The P1b CI now builds the frontend and scans compiled assets; generated-contract drift checks remain P1-05 work.

## Commands once the toolchain is available

```sh
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
cargo +1.99.1 fmt --all -- --check
cargo +1.99.1 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.99.1 test --workspace --locked
```

These unit tests do not need an API key, a database, Hiyal or model inference. The lockfile currently has no third-party dependencies. Before claiming a reproducible P1 gate, verify that Cargo regenerates this lockfile with **no diff** under the pinned toolchain.

## Explicit boundaries and limitations

- These are contract **data types and primitives**, not the P3 authoritative policy reducer. In particular, `Treasury::can_fund` is **not** cabinet approval; it is only an affordability check. It does not yet handle protected commitments or allocation amendment transitions.
- `deliver_family_quarter` implements a **pure multi-scope family-capacity scheduling primitive** with immutable scope ID and per-scope carry. It does not implement the P3 world reducer, adjudication of funding/conditions/audits, historical project schedule storage, or ledger/receipt atomicity. `Treasury::record_delivery` is a separate account-transition helper that **must only** be invoked after a single, validated authoritative payout. The P3 world reducer must compose them and verify receipt-level equality; no live multi-quarter case has been executed.
- The country evidence list is a typed provenance skeleton; P2 has not ingested or validated an England country pack. Cohort shares remain evidence-dependent.
- `WireU64` versions, private effort declarations, validated allocation/effort/belief/cap types, explicit invalid-draft preference and audited-recipient collections now enforce P0's typed **Rust construction** constraints. Serde must use the same validation route when added; actual JSON serialization, `schemars` schemas, `ts-rs` output, versioned event receipts and command deduplication await P1-02/03 and later phases. `RoleObservation` and `FinalPosition` are not substitutes for role-aware authorization or serialization controls.
- React/TypeScript/Vite are present as a **preview shell**, with an npm-generated `apps/web/package-lock.json` and validated CI build; **Axum, OpenRouter and Hiyal are not yet integrated**. The reviewed Hiyal candidate revision remains `050d1de094f5d0813817e2020ec5c5da97d5c42e`; introduce it only at P4. The frontend's display-only TypeScript interfaces are **not** generated Serde contracts.
- Supported replay claim is intended for Linux x86_64 and macOS arm64, **not verified** yet. CI currently covers Linux only.
- Local authoring environment has Node but **no installed Rust compiler/Cargo** and cannot resolve registries; tests were not executed there. **GitHub Actions run #40892336573**, on 2026-10-29, **passed** pinned Rust 1.99.1 formatting, Clippy, fail-closed source guard, 12/12 Rust unit tests and the tracked-file credential-pattern scan on Linux. This verifies the *Rust foundation slice*, not the full P1 acceptance gate. Initial CI failures (formatting and a credential-scanner self-match) were corrected in subsequent commits; final run passed. The wider P1 checklist still remains open.

## Executed verification evidence

- [Essential offline checks — successful run #40892336573](https://github.com/davletovb/simulation-app/actions/runs/40892336573), commit `7229418ee7639ec73b5ad7cb736a47e14363d05b`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (**12 passing tests**), fail-closed source-rule and tracked-file credential pattern checks. No external inference or country-data fetch was executed.

## Next P1 work

1. **P1b completed as a preview slice**: React/Vite shell, real npm lockfile, strict typecheck/build and compiled-asset scan in the same Linux CI job. This is not an interactive simulation.
2. Introduce Serde, schemars and ts-rs on the pinned compatible toolchain, with generated schemas/TypeScript, canonical decimal-string fields and serialization boundary tests.
3. Build a recorded/scripted local API mode with backend-only credentials and an executable offline smoke path.
4. Complete whole-tree/compiled-asset credential scanning, contract-drift detection and the final P1 evidence log. Keep live model evaluation out of routine CI.
