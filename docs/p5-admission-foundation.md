# P5 offline admission foundation — review packet

**Branch:** `feat/p5-offline-admission-ledger`. **State:** implementation candidate, **not** a completed P5 gate. No live calls have been made.

The `myth-hiyal-adapter::p5_allowance` module adds a single-owner, local, fail-closed shared allowance, independent of Hiyal's per-trial `GovernedBackend`. It intentionally does not enable network dispatch, configure a live trial, or make a minister decision.

## Implemented behavior

- A process-owned filesystem lock remains held for the allowance lifetime. A separate installation anchor detects a lost or replaced ledger; an invalid hash, identity, malformed state or unavailable persistence prevents recovery. The initial $10 allowance is provisioned only for a deliberately empty directory.
- Each call has a stable call ID, reservation ID, purpose (minister, assistant, authoring, evaluation), optional campaign/quarter/batch, pinned model/provider/prices, request digest, upper-bound reservation and lifecycle state. An existing call ID with identical data is inspectable but is **never** redispatched. A new billed attempt must have a new call ID.
- Before an application dispatches a provider request, it must call `reserve` and then `mark_dispatched`; both atomically replace and fsync the authoritative file. A crash at either boundary leaves the full reservation held. Missing provider usage never refunds it. A recorded response is distinct from operator-verified charge.
- The current epoch retains its own full reservations and all unresolved prior-epoch liabilities. A new explicitly funded epoch cannot be opened if its funding is below carried liabilities. Restart and campaign switching do not create an allowance.
- The request admission path accepts only `openrouter` provider requests with non-streaming strict JSON-schema response format and a pinned compatible route. It refuses unknown/unbounded prices or reasoning, oversized requests, excess output tokens, and quarter/campaign/batch/shared caps. It uses a **conservative request-byte-as-token** input test and the pinned Hiyal `BudgetConfig::reserve_amount` function, including Hiyal's framing allowance. This is not tokenizer accounting, exact billing or a provider quote.
- Tests cover two campaigns opened in either order, assistant/authoring calls outside a trial, occupied owner lock, replaying stable IDs, incompatible ID reuse, corrupted/deleted ledger, reserved/dispatched/recorded recovery, and funded epoch rollover with uncertain charges.

## Remaining work before P5 is complete

**P5-01:** route capability verification, actual OpenRouter-only HTTP transport (no other external providers), strict schema parsing against typed role observations and actual route/model identity validation. Keep all live requests opt-in and avoid automatic model/provider fallbacks.

**P5-02:** wire this ledger to the Hiyal *per-trial* `GovernedBackend`, with immutable trial budget configuration and explicit recovery ordering. Add stronger per-logical-action repair/retry ceilings, explicit reset operator command/audit history, genuine provider-bill reconciliation and broader fault injection around fsync boundaries. This candidate ledger currently has only its own local lock; it is **not** wired to trial orchestration or to the API.

**P5-03/P5-04:** ministers' independent frozen decisions, persistent commitment-based memory, bounded role-visible prompts, typed final votes and private effort, adversarial captured response suites, and genuine crash-injection tests through Hiyal's receipt path.

**P5-05:** manually authorized, strictly capped live Experiment A and exact offline recorded replay. No API key or paid call was used for this branch.

## Local checks to run

```sh
cargo +1.99.1 fmt --all -- --check
cargo +1.99.1 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.99.1 test --workspace --locked
```

Do **not** check off any P5 task based only on the presence of these source files. CI results and independent review must be recorded separately. Do not call this a distributed spending wallet or an exact OpenRouter invoice.
