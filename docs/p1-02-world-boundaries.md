# P1-02e — Authoritative world, event and receipt **domain boundaries**

**Status:** first P1-02 Rust-domain slice on top of merged P1d. The full P1-02 gate is **not closed**. The only normative design is [the P0 England education pilot contract](p0-case-contract.md) and [framework v0.8.1](framework.md). This code introduces validated types, **not** an authoritative simulator or empirical model.

## Types added

`myth-domain::world` is pure Rust, dependency-free, and forbidden from network, model calls and filesystem access.

| Boundary | Validation and representation | Explicitly deferred |
| --- | --- | --- |
| `WorldSnapshot` + `WorldParts` | Private fields; `from_parts` validates campaign/pack IDs, treasury, unique funded scopes, exact by-family remaining-funds/encumbrance equality, six fixed beneficiary cells, exactly two named fictional organizations, pending/resolved commitment status, and response identity. `initial` starts Q1 with 100 available fictional credits and each cell's authored support at 500,000 ppm. | The reducer's authoritative version advancement, funding admission, private minister effort, role-scoped observations, exact journal serialization, and P2 evidence-backed initialization |
| `QueuedResponse` | Requires a due quarter **after** production, an event ID, and the exact equality `new_service_spent = staff + repairs + access delivered`. Q7+ is representable without pretending it was applied in Q6. | Coefficient application, delayed cohort support updates, and second-order dynamics |
| `MeetingPhase` | Finite typed phase labels; explicitly not a runnable state machine | Authority, freeze/revision rules, two-round limits, and actual phase transitions |
| `EventEnvelope` | Version-tagged typed event with distinct stable ID, nonzero event sequence and world-version reference, quarter, checked typed payload; impossible quarter jumps and invalid IDs are rejected. | Append-order correctness, idempotency, hash-chain rules, role visibility, command/event atomicity, and Hiyal |
| `LedgerMove` + `LedgerReceipt` | Typed `Reserve`, `Release`, `SpendService`, `SpendAudit`; credits strictly positive. A receipt is constructible only if its **entire before/after treasury state** exactly equals the expected one-family account transfer with every other ledger account unchanged. Both boundaries must conserve the same 100 fictional credits. | Whether the move is legally approved, its linked funding scope/delivery/audit, unique payout, and event-chain atomicity |

This separation is essential: a mechanically valid receipt is **not** spending authorization. `Treasury::record_delivery` and `LedgerReceipt::new` are validation primitives. P3 must couple a unique authorized delivery receipt to one state mutation, protect prior commitments, and enforce idempotency.

## Precision and information boundaries

All financial values are **integer fictional credits**. Cohort weights and support are bounded `ppm` values, not floating-point percentages. Version and sequence fields use `WireU64`, whose existing Serde boundary requires canonical decimal strings across JavaScript. Source scopes and IDs must be stable and unique.

`WorldSnapshot` intentionally derives **neither Serialize nor Debug** and keeps its fields private. It contains authoritative values that **cannot** be exposed directly to React. Use the existing role-projection system and a future permit-filtered view to expose only authorized data; minister private effort remains on its separate private channel. Country-pack content comes from P2, not from an invented England baseline.

## Executable evidence

```sh
cargo +1.99.1 fmt --all -- --check
cargo +1.99.1 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.99.1 test --workspace --locked
```

The added tests exercise initial world construction, family-specific conservation, duplicate scope rejection, reconciliation with actual capacity-scheduler output, reserve/release/audit movements, invalid zero-credit moves, delayed responses past Q6, and event identity/quarter bounds. No live inference or real country data are required.

**Remaining P1-02 obligations:** validated versioned **persistent checkpoint and wire representation** of full world/receipt/event data; provenance/country-pack validation; actual end-to-end role projection with access control; fixed-point response arithmetic and residual distribution; P3 institution and policy transition enforcement; P4 hash/journal and replay binding; cross-platform byte-identical checks. Do not label this a completed P1-02 acceptance gate.
