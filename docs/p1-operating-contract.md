# P1 operating and verification contract

**Scope:** first-playable foundation only; no real-world causality, provider dispatch, P3 game transitions or Hiyal replay.

## Versions and supported operating systems

- **Rust:** 1.99.1 pinned in `rust-toolchain.toml`; Cargo 2024 workspace, fully committed `Cargo.lock`.
- **Node.js:** 22.16.0 via `apps/web/.nvmrc`; `npm ci --ignore-scripts` uses `apps/web/package-lock.json` with `engine-strict`.
- **Reviewed Hiyal upstream:** `050d1de094f5d0813817e2020ec5c5da97d5c42e`. **Not linked yet** (scheduled P4): the pure Rust domain has no Hiyal, I/O or LLM dependencies. Pin adoption to this exact revision when creating the adapter, and run all Hiyal-specific replay checks before changing the pin.
- **Declared reproducibility targets:** Linux x86_64 and macOS arm64, using the pinned Rust/Node toolchains. The routine GitHub Actions job exercises **Linux x86_64 only**. macOS arm64 is a required **second-OS verification target for P4-05 and P7-06**, not a passed test today.
- **No separate release claim** for Windows, hosted services, real country outcomes or live LLM evaluation.

## Deterministic types and private recovery

The `myth-domain` crate owns integer limits and exact ledger transfers, six fixed beneficiary cells, checked organization and role projections, and a residual-preserving fixed-point apportionment primitive. World snapshots are private Rust types and are **not** serialized to clients.

`myth-contracts::checkpoint` provides checked, versioned Serde wire forms for **full private world snapshots**, country-pack provenance and typed event/receipt envelopes. Numeric version/sequence values remain canonical **decimal strings**, including `u64::MAX`; no round-trip through browser JS floating-point. World restore invokes domain validation of scope lifecycle, funding, delayed effects, organizational caps and political promises, and binds to an expected *pack-content SHA-256*. Neither file hash nor schema alone establishes provenance of real public data; P2 must produce and review the actual pack.

The private local checkpoint store writes bounded, checksum-protected `<campaign>.world.json` files using a uniquely named temporary file and fsync/rename, then verifies the file on re-open. **Do not expose these files to the React API**: they contain authoritative private world values. This is a *checkpoint-only* prototype, not an append-only journal, P3 command processing, P4 Hiyal replay, or P7 campaign export.

From the repository root, run a deliberately **fictional** seed/save/verify:
```sh
hash=1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
cargo +1.99.1 run --locked -p myth-local-api --bin myth-checkpoint -- save-fixture local-data/private-world example "$hash"
cargo +1.99.1 run --locked -p myth-local-api --bin myth-checkpoint -- verify local-data/private-world example "$hash"
```
Do not treat this hash as a real source manifest; it is a fixed **test fixture**. Copy the `example.world.json` file to local backup storage and run `verify` after copying back. A missing file, wrong pack digest, malformed/changed JSON, or invalid domain state must fail. `local-data/` is ignored by Git. The service does not support public checkpoint endpoints.

## Live-call and schema boundary

The local backend accepts only `MYTH_MODE=scripted` or `recorded` and refuses `live` even if API keys are present. It does not read, log or forward provider credentials. A P5 implementation must first configure the selected OpenRouter model, provider, price/receipt semantics, per-period/trial/batch caps, and the durable **shared application allowance** from P0-04. Uncertain charges cannot become zero after restart. This P1 sandbox makes no paid requests.

`schemars` schemas and `ts-rs` types are generated from validated Rust DTOs, with **exact artifact drift** verified in CI. They reflect general JSON Schema; a particular serving provider's supported strict structured-output subset is **not certified by offline schema tests**. P5-01 must choose/pin one provider route with disabled fallback, verify the subset accepted by that route, and include a negative test that malformed remote output cannot enter the domain. P1-03's **offline profile-and-Serde alignment** is verified; do not claim **P5-01 provider compatibility** without that later evidence.

## Exact gate boundary

P1-01 foundation and P1-05 routine Linux checks can be demonstrated with the installed CI. P1-02 domain/wire types and private checkpoint can be validated as *shape and consistency*; P3 still owns policy authorization, and P2 supplies the reviewed country data. P1-04 supports offline operation and private fixture save; **not** full campaign replay. P1-03 selects and tests a conservative **nine-profile schema subset** offline; its actual use on a pinned provider remains a distinct **P5-01** acceptance gate. The P1 tracker checkmarks cover the offline contract boundary only.

P1 CI should remain **one essential Linux job**, with Rust format/Clippy/test, guarded deterministic primitives, generated contract drift, npm strict typecheck+build, tracked/compiled asset scans, and a real keyless loopback smoke. No paid inference, broad OS matrix, or costly synthetic benchmark belongs in routine PR CI. Run the second OS deliberately and record the tested commit, hardware, OS, toolchain, fixture hashes and full result when P4 begins.
