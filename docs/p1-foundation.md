# P1 — Rust foundation slice (partial implementation)

**P0 baseline:** [England education pilot contract](p0-case-contract.md). **Scope:** first executable code only; **P1 is NOT completed** by this slice.

## Added

- A dependency-free Rust workspace with `crates/domain`, locked to Rust **1.99.1** (edition 2024).
- Pure bounded types for fictional 100-credit pilot allocations, treasury invariant, six cohort keys/weights, ministry roles, proposals, commitments, typed actions and role observations.
- Deterministic integer fractional-delivery primitive, with explicit 0–1000 permille effort validation.
- Canonical decimal-string parser for full-range `u64` wire values (the actual Serde JSON representation and JSON Schema remain P1-03).
- Minimal Linux PR CI with formatter, Clippy, unit tests, a source-rule guard and coarse credential-like literal scan. This is a **starter** check, not the complete frontend/build/contract-drift scan required by P1-05.

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
- The country evidence list is a typed provenance skeleton; P2 has not ingested or validated an England country pack. Cohort shares remain evidence-dependent.
- `WireU64` demonstrates string-boundary validation; actual JSON serialization, `schemars` schemas, `ts-rs` output, versioned event receipts and command deduplication await P1-02/03 and later phases.
- React/TypeScript/Vite, Axum, OpenRouter and Hiyal are not yet installed. The reviewed Hiyal candidate revision remains `050d1de094f5d0813817e2020ec5c5da97d5c42e`; introduce it only at P4. No npm lockfile is invented; create it from a real package installation in a tool-enabled environment.
- Supported replay claim is intended for Linux x86_64 and macOS arm64, **not verified** yet. CI currently covers Linux only.
- Local authoring environment has Node but **no installed Rust compiler/Cargo** and cannot resolve registries; tests were not executed there. **GitHub Actions run #40888773433**, on 2026-10-29, **passed** pinned Rust 1.99.1 formatting, Clippy, source guard, 8/8 Rust unit tests and the basic credential scan on Linux. This verifies the *Rust foundation slice*, not the full P1 acceptance gate. Initial CI failures (formatting and a credential-scanner self-match) were corrected in subsequent commits; final run passed. The wider P1 checklist still remains open.

## Executed verification evidence

- [Essential offline checks — successful run #40888773433](https://github.com/davletovb/simulation-app/actions/runs/40888773433), commit `722137344291553a3991ae2187b9c1a092391a93`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (**8 passing tests**), source-rule and credential pattern checks. No external inference or country-data fetch was executed.

## Next P1 work

1. Build the React/Vite shell, generate a real npm lockfile, add its typecheck/build to the same essential offline CI job.
2. Introduce Serde, schemars and ts-rs on the pinned compatible toolchain, with generated schemas/TypeScript, canonical decimal-string fields and serialization boundary tests.
3. Build a recorded/scripted local API mode with backend-only credentials and an executable offline smoke path.
4. Complete whole-tree/compiled-asset credential scanning, contract-drift detection and the final P1 evidence log. Keep live model evaluation out of routine CI.
