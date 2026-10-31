# P5 — OpenRouter cabinet implementation and verification

**Status (31 October 2026):** Offline implementation and mock-provider verification in draft [PR #12](https://github.com/davletovb/simulation-app/pull/12); **P5's live Experiment A gate is not complete**. No paid inference, provider capability certification, or live model-behavior claims are recorded here. The P4 default scripted path and keyless replay remain supported. This document supersedes the earlier admission-only review packet.

## Architecture and implementation

The game owns an installation-specific, single-writer, durable `Allowance`. Its authoritative journal is independent of Hiyal trial receipts and rebuildable UI views. Before each external call, a stable remote attempt ID is reserved, an upper-bound charge is persisted and fsynced, and dispatch is durably marked. Crash/timeout/missing-usage charges are **unknown and retained**, never assumed zero. Reusing an existing attempt ID does not repeat a paid dispatch; a new provider attempt would require a fresh ID, new reservation, and independent operator action. The ledger has a $10 initial shared epoch; no automatic restart/monthly/campaign reset. Explicit funded epochs retain history and carry unresolved liabilities. Operator-verified prior-epoch charges remain deducted from the receiving epoch, not silently refunded. This is neither a distributed wallet nor provider billing truth.

The ordering matters: **Hiyal Trial → game-owned keyed SharedAdmissionBackend → a separate per-trial GovernedBackend → pinned provider**. Hiyal's pinned GovernedBackend forwards to its inner backend without the effect key; wrapping it *outside* the keyed game ledger would lose durable attempt identity. The game ledger therefore receives and verifies Hiyal's full effect key first, reserves durably, and only then invokes that trial's independently budgeted Hiyal backend. On resume the game ledger is opened/recovered before any trial dispatch; Hiyal recovers its immutable budget from existing receipts. The trial also persists its immutable route/provider/prices and refuses changed settings across resume. The backend-only player-command echo uses an explicit Hiyal allowlist and cannot mutate the world through model output.

The opt-in `OpenRouterBackend` targets only OpenRouter's HTTPS chat completions endpoint. It uses a pinned model/provider, restricted provider order and allowlist, `allow_fallbacks=false`, `require_parameters=true`, strict JSON-schema output, bounded output, no streaming or model-issued tools, no redirects or environment proxy, a finite timeout and bounded response size. It fetches generation metadata to check **actual** reported model/provider identity before a model action is accepted. A denied/malformed/ambiguous provider response is an operational failure, not a fabricated opposition/support vote, and its uncertain cost remains reserved. The actual model/route has **not** been certified live; route slugs versus provider display names must be tested on the deliberately selected provider.

The minister adapter supplies three distinct role mandates (Finance, Education and Infrastructure), frozen role-visible `PublicRoleView` data, an allowlisted projection of state, the latest six owned commitments as **prompt memory** and up to twelve visible evidence IDs. The P3 authoritative commitment registry remains unbounded by prompt eviction and is checked by the reducer; no speeches, raw player text or news bodies enter the role prompt. A minister can change its typed allocation preference, vote, conditions and (for Education or Infrastructure only) private effort. Schema validation enforces the frozen meeting, stage, proposal version and minister seat. Invalid model output aborts the current action rather than changing authority or mutating state. Repair/retry orchestration and an in-game retry control are **not** present in this slice.

## Cost and activation policy

The fixed contract caps are **$0.25/quarter, $1.50/campaign/trial, $3/evaluation batch, $10/installation's initial epoch**, at most 16 requests/quarter and 96/campaign, with minister input/output limits and route prices as declared in [P0-04](p0-case-contract.md). The shared ledger charges full conservative reservations even when a provider reports fewer tokens; Hiyal's immutable per-trial cap does likewise. Reported usage, estimates and verified bills are separately recorded. The game must never infer exact invoice amounts from the reservation.

**Routine CI is offline and cannot perform live inference.** The local API's `MYTH_MODE=live` remains disabled; P6 owns UI wiring. A private/manual CLI exists:

```sh
cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p5-pilot -- inspect <trial-dir>
```

This verifies the existing Hiyal journal and prints hashes without key/network calls.

To manually run a **new four-quarter / 24-normal-minister-slot** Experiment A candidate with the explicitly provisioned OpenRouter key, the operator must provide backend-only `OPENROUTER_API_KEY`, `MYTH_P5_MODEL`, `MYTH_P5_PROVIDER`, `MYTH_P5_INPUT_MICRO_PER_M`, `MYTH_P5_OUTPUT_MICRO_PER_M`, `MYTH_P5_ROUTE_PRICE_VERIFIED=yes`, and `MYTH_P5_MANUAL_AUTHORIZATION=AUTHORIZE_34_CALL_EXPERIMENT_A` in a private environment. Then:

```sh
cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p5-pilot -- pilot <new-trial-dir> <dedicated-allowance-dir> <installation-id>
```

**Do not run this command as CI.** Operator authorization, key provisioning, price/schema/route research, sufficient OpenRouter credits and a review of the conservative reservation assumptions are prerequisites; no secrets should appear in terminal history, shell scripts, commits or browser bundles. The CLI never automatically retries, never selects a model or paid alternative, and pauses on a failed seat; the originally planned 34-attempt envelope includes optional repair/manual retry allowances that are *not* yet orchestrated. The pilot can report the resulting authoritative state/receipt counters and exact keyless journal hash, but it does **not** automatically declare that P0-05 behavior criteria pass. A positive Experiment A decision still requires predeclared denominator/latency/privacy/commitment/feasibility and baseline comparisons.

## Verified billing and currently unsupported same-key retry

A previously dispatched call retains its full allowance reservation after errors, missing metadata, or a timeout. The raw successful HTTP response (when received) is fsynced in the trial's owner-private `p5-completion-custody/` directory before model/metadata verification; inspect it only on a trusted computer. Metadata GETs retry a few times with bounded backoff; paid completion POSTs **never** retry automatically.

An operator who has independently verified a provider bill can enter a charge without any model request:

```sh
MYTH_P5_VERIFIED_PROVIDER_INVOICE=yes cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p5-pilot -- verify-charge <allowance-dir> <installation-id> <call-id> <actual-micro-usd>
```

This records a verified bill, but does **not** manufacture a missing successful Hiyal receipt or turn a failed typed action into support. Hiyal's pinned effect journal deduplicates by logical effect key; **restarting the same failed slot does not create a new billable attempt**. Consequently, same-quarter paid minister retries remain disabled pending an explicit, receipt-safe key-version/authorisation protocol. The pilot fails closed and its message no longer implies that a same-key retry works.

Historical verified calls are compacted only on a deliberately funded new epoch to scope/maximum/verified-amount fingerprints and a SHA-256 of the original full call; archived call IDs remain blocked, and their original campaign/quarter/batch usage remains counted. Unverified calls are never compacted, and ledger capacity remains bounded and fails closed if exhausted. Verified reconciliation is necessary to clear long-lived unknown liabilities; never infer that a timeout cost zero.

## Executed offline evidence and limitations

- The earlier admission foundation passed full essential CI [run 41182729532](https://github.com/davletovb/simulation-app/actions/runs/41182729532).
- The enlarged fake-cabinet Hiyal integration, shared accounting, 20 instruction-injection probes, malformed/missing-usage provider payloads, full six-quarter mocked disagreement, and keyless exact replay passed Rust tests within full [CI run 41188415444](https://github.com/davletovb/simulation-app/actions/runs/41188415444). Later CLI/route-binding changes require their own latest green CI record before being called verified.
- Distinguish in-process restart tests from independent multi-process power-loss recovery. The latter and more exhaustive fsync/atomic-rename fault injection remain unverified.
- Neither offline fake provider success nor successful Hiyal replay establishes OpenRouter route availability, strict-output support, charged provider identity, observed response latency, billing accuracy, independent model judgments, confidentiality against live adversaries, or real-world educational predictions.
- P5-05 remains **open** pending explicit, budgeted live Experiment A; P5-04's complete recovery and adversarial suite, and P5-03's visible bounded retries also remain open as acceptance work. Do not mark whole P5 complete by documentation alone.

## CI and local verification

```sh
cargo +1.99.1 fmt --all -- --check
cargo +1.99.1 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.99.1 test --workspace --locked
```

The essential GitHub Actions workflow runs these offline, plus P2/P3 fixtures, API smoke, schema drift, frontend build and secret scans. Live calls are **never** a CI step.

## Explicit retry and paid-completion reconciliation (review follow-up)

Hiyal's pinned implementation needs **no core modification** for this path: a failed frozen round commits an immutable rejected event and its keyed failure receipt. The next separately authorized frozen round runs at a later event sequence and produces a fresh Hiyal effect key. The game now disallows any automatic continuation of an operationally failed minister seat until the owner explicitly authorizes the precise failed receipt.

Operator-only commands (no calls in normal CI):

```sh
MYTH_P5_RETRY_AUTHORIZATION=AUTHORIZE_FAILED_SEAT \
 cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p5-pilot -- \
 authorize-retry <trial-dir> <ledger-dir> <installation-id> finance

MYTH_P5_CERTIFY_SAVED=VERIFY_SAVED_COMPLETION \
 cargo +1.99.1 run --locked -p myth-hiyal-adapter --bin p5-pilot -- \
 certify-saved <trial-dir> <ledger-dir> <installation-id> finance
```

The first writes a fsynced private authorization for the **particular failed receipt, event hash, frozen role observation, quarter, stage and request digest**, capped at two manual operational retries per quarter. Repeating the same approval does not authorize extra attempts. Continuing the bounded pilot *after authorization* creates a new Hiyal effect key and a new application-ledger reservation. The old receipt and unknown-charge liability remain in both journals. A failed attempt with no matching Hiyal operational-failure receipt is not eligible for this operation; malformed JSON repair is **separate work**.

The optional second command performs **generation-metadata GET only**, verifying the saved paid completion's immutable model/provider and strict schema. It then persists a request/response/route-SHA bound certification. When that exact frozen request next executes, the OpenRouter transport prefers the certified saved response and makes **no additional paid completion POST**. If a paid response exists but is not certified or is ambiguous/corrupt, a new paid POST is refused until reconciliation; a missing paid response can be retried only after explicit authorization. The immutable original Hiyal failed receipt is never rewritten.

Accounting caveat: a recovered paid completion is emitted under a new Hiyal effect key. For safety, the per-trial Hiyal budget conservatively counts the reconstructed model effect in addition to its original unknown-charge receipt; no second actual POST is sent. The game shared ledger also reserves the new effect conservatively, even though it is a verified no-POST reuse. This may reduce the effective spending headroom but can never silently increase it. Exact one-charge sharing across distinct Hiyal receipts would require an independently reviewed Hiyal billing-link extension; none is claimed here.

No live OpenRouter credentials, paid experiment, real provider metadata certification or actual invoice verification were used in offline testing. Keep the real-model gates open pending those separately authorized tests.
