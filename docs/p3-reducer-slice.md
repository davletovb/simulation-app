# P3 pure reducer — first scripted slice (not the Experiment B gate)

Implementation: `crates/domain/src/reducer.rs`, `crates/domain/src/audit.rs`. This is a deterministic Rust-only, no-Hiyal, no-LLM, no-network implementation, built on P1/P2 contracts. The P3 tracker items remain unchecked until their **entire** acceptance criteria are met. It must not be merged as a completed P3 milestone.

## Executed behavior

- Stage each admission on a cloned campaign. Failed actor actions, invalid votes, stale player commands and bad allocations leave the authoritative world/treasury unchanged.
- Three minister first positions freeze; a maximum of one versioned player amendment; final freeze, all three typed final positions and private owner-controlled effort, then support threshold >=2 to ratify. Meetings do not advance time.
- New ratifications transfer only undelivered unprotected encumbrances before reserving the replacement. No refunds of completed delivery, audit spending or borrowing. Due conditions are registered as authoritative commitments and assessed on deadlines; future protected commitments currently cause new proposals to fail closed rather than implement the full protected tranche release/continuation flexibility.
- Explicit `AdvanceQuarter`: oldest-funded-first single shared capacity across scopes, fractional effort carry, checked ledger service spending, prior lagged cohort response, next-quarter fictional organization effects and deadline adjudication. No automatic turn advance. Q6 closure is explicit.
- Integer-only six-cell authored coefficients, per-channel geography/income floors and bounded response. Reporting is simulated and can differ from actual effort, which is private.
- Game-authored audit data is created from recorded quarterly delivery history, not free-text reports; first eligible audit is free, subsequent audits cost two available credits. Explicit disclosure uses a per-role belief ledger with idempotence, supersession without replay and per-seat 160,000 ppm variation cap. This is a **partial** P3-05 implementation, not signed cryptographic attestation.
- Deterministic unit tests cover ledger authorization, failed votes, source separation, Q1/Q2 delivery lags, low effort, Q6, negative cohort effects, relevant and duplicate evidence and variation limits.

## Not yet certified

**P3-01:** Full future staffing/maintenance obligations, funded tranche release authorization and protected partial reallocation still need design and tests. Current restrictions intentionally reject changes while any enforceable condition is pending.

**P3-02:** Explicit stage admission is present, but full retry/cancellation flow, per-quarter reporting and robust frozen observation history are incomplete.

**P3-03:** Registry and deadline changes are present, but enforceable condition scheduling/ownership and commitment-specific delivery receipts need expansion. Cross-quarter funding obligations cannot be simulated as automatically executable payments.

**P3-04:** Two organization actions, service response and money/weight accounting are implemented. The declared exogenous shock schedule and full constituency-pressure feedback into the next meeting are not.

**P3-05:** Audit evidence, disclosure and belief budget are implemented in memory, but schedule snapshots still require the full cumulative full-effort counterfactual, strict conflict/correction review and replay-pinned evidence provenance. Public-vs-private role observations are incomplete.

**P3-06:** Scripted regression tests cover mechanical paths, but the independent profile-only selector, complete canonical test matrix and receipts-versus-state diff audit remain.

**P3-07:** The declared sweep over coefficient magnitude, delivery lags, response lags and population alternatives has **not yet run**. P2 says all cohort margins are authored, not empirical; future P3 sensitivity must say so and must preserve the same actor actions, policy stream and exogenous shocks.

**P4:** The extra private reducer state (meeting, prior command IDs, private effort, condition baselines, audits and beliefs) is not yet included in the P1 checkpoint wire schema. A persisted gameplay path must not use the current world-only checkpoint as if it captured the whole reducer. Extend and test replay binding before operational use.

## Verification

Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked` in the normal existing essential CI job. No standalone new expensive CI suites or model calls.

This document distinguishes script tests that actually execute in CI from the broader Experiment B gate. The empirical and behavioral assumptions are illustrative only, not official England policy or causal predictions.
