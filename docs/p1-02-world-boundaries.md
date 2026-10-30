# P1-02e — Authoritative world, event and receipt **domain boundaries**

**Status:** first P1-02 Rust-domain slice on top of merged P1d. The full P1-02 gate is **not closed**. The only normative design is [the P0 England education pilot contract](p0-case-contract.md) and [framework v0.8.1](framework.md). This code introduces validated types, **not** an authoritative simulator or empirical model.

## Types added

`myth-domain::world` is pure Rust, dependency-free, and forbidden from network, model calls and filesystem access.

| Boundary | Validation and representation | Explicitly deferred |
| --- | --- | --- |
| `WorldSnapshot` + `WorldParts` | Private fields; `from_parts` validates campaign/pack IDs, treasury, unique funded scopes, exact by-family **active** scope remaining-funds/encumbrance equality, six fixed beneficiary cells, exactly two named fictional organizations with P0 starting resources **10 influence / 4 volunteer days** and bounded maxima **20 / 12**, shared commitment status/funding validation, response identities, chronological consistency, and aggregate queued spending ≤ actual service spending. Terminated scopes remain available for audit history but no longer contribute to encumbrance; P3 must atomically authorize and journal the corresponding treasury release. `initial` starts Q1 with 100 available fictional credits and each cell's authored support at 500,000 ppm. | The reducer's authoritative version advancement, funding admission, private minister effort, role-scoped observations, exact journal serialization, and P2 evidence-backed initialization |
| `QueuedResponse` | Requires a due quarter **after** production, an event ID, and the exact equality `new_service_spent = staff + repairs + access delivered`. Each queued response is revalidated at snapshot admission; production cannot lie in the future, due items cannot be overdue, and the sum of pending response spending cannot exceed lifetime `treasury.service_spent`. Q7+ is representable without pretending it was applied in Q6. | Coefficient application, delayed cohort support updates, and second-order dynamics |
| `MeetingPhase` | Finite typed phase labels; explicitly not a runnable state machine | Authority, freeze/revision rules, two-round limits, and actual phase transitions |
| `EventEnvelope` | Version-tagged typed event with distinct stable ID, nonzero event sequence and world-version reference, quarter, checked typed payload; impossible quarter jumps and invalid IDs are rejected. A `DeliveryRecorded` event must carry **1–12 staffing**, **1–10 repairs**, or **1–5 access units** (the access upper bound includes the possible conditional bonus), independently of later event-journal linkage. | Append-order correctness, idempotency, hash-chain rules, role visibility, command/event atomicity, and Hiyal |
| `LedgerMove` + `LedgerReceipt` | Typed `Reserve`, `Release`, `SpendService`, `SpendAudit`; credits strictly positive. A receipt is constructible only if its **entire before/after treasury state** exactly equals `Treasury::apply(movement)` with every other ledger account unchanged. `Treasury::record_delivery` delegates to this one checked implementation (retaining its zero-delivery no-op). Both boundaries must conserve the same 100 fictional credits. | Whether the move is legally approved, its linked funding scope/delivery/audit, unique payout, and event-chain atomicity |

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

The added tests exercise P0 organization initial resources and caps, cancellation plus treasury release without losing audit history, shared political commitment checks across world and role views, family-specific conservation, duplicate scope rejection, reconciliation with actual capacity-scheduler output, reserve/release/audit movements, invalid zero-credit moves, delayed responses past Q6, future/overdue payload rejection, aggregate pending spending and per-family delivery event bounds. No live inference or real country data are required.

**P1 closure update:** Versioned private checkpoint and event/receipt wire formats, checked country-pack provenance, domain-owned role-view construction, and fixed-point residual primitives are now implemented under PR #8. The private archive has owner-only Unix permissions, corruption detection, and single-instance version monotonicity, but **no authenticated tamper evidence or cross-process locking**. Actual access-control decisions, calibrated response dynamics and P3 institution/policy transitions remain unimplemented; P4 must provide hash/journal/replay binding and second-OS verification. The P1 checklist reflects offline foundational types, not the later reducer and replay gates.
