# P6-03 / P6-05 implementation audit — 2026-10-31

**Review type:** Author-side adversarial inspection of the P6 branch and executed CI. **Not an independent external review.** A future independent reviewer should inspect the precise PR head, run the locked tests and evaluate this audit critically rather than relying on the conclusions below.

## P6-03 — local player surface

- React and Axum are served from a single loopback origin, with local Vite proxy in development. Live model mode is deliberately rejected.
- Player commands have serialized typed variants, version checks and persisted command IDs. The React UI only displays verified committed state; it does not implement P3 social rules.
- Public Cabinet, Campaign and Intel tabs now show recorded minister positions, fiscal consequences and the authored case pack's attribution/cutoff.
- **Selective accessible component adoption:** `@base-ui/react@1.9.0` for the connected action button and assumptions dialog, pinned exactly in the npm lockfile. Existing CSS theme variables are re-used; no runtime CDNs, external icon kits or broad component rewrite. The native briefing dialog and tabs remain keyboard navigable, and global view shortcuts are suppressed while either native or headless modal is open.
- **Exact integer budget:** both slider and input accept whole credits. The numeric parser refuses negative/decimal/exponential/grouped/Unicode digits, oversized values, and amounts above the current committed treasury. Server `serde` and the P3 reducer independently revalidate the typed allocation.
- Explicit `en-GB` language in the HTML and app root. Focusable disabled UI buttons and keyboard tab flows are used. Pending command IDs survive reloads and are looked up durably rather than regenerated after lost HTTP responses.
- The local optimistic preview remains labelled as such, distinct from authoritative Hiyal outcomes.

**Limit:** This is an implementation-level accessibility inspection, **not** a completed manual screen-reader or cross-device usability study. Those belong in first-playable P7 review, not proof of real-user comprehension.

## P6-05 — fault, authority, cost and recovery

| Failure/requirement | Executed evidence |
|---|---|
| Lost HTTP acknowledgment; repeated identical ID | `durable_idempotency_and_scoped_recovery`; `command_identity_persists_through_lost_ack_and_restart`; OS-process restart HTTP smoke |
| ID reused with changed payload | Rust conflict and HTTP 409 tests |
| Crash after command acceptance before dispatch | `fsynced_acceptance_before_dispatch_recovers_once` |
| Crash after Hiyal commit before operation status saved | `crash_after_hiyal_commit_before_command_status_rebuilds_once` |
| Receipt-before-action / mid-minister interruption | `mid_round_receipt_panic_restarts_without_duplicate_minister_actions` |
| Force OS-process exit/restart | `.github/scripts/offline-api-smoke.sh` sends SIGKILL, checks matching cursor and existing command ID |
| More than one server on the same storage | `one_save_root_allows_only_one_os_writer` and separate-port OS smoke; standard-library `File::try_lock` prevents simultaneous writers, with lock held while workers exist |
| Initial + reconnect SSE boundary under concurrent commit | `initial_and_fallback_join_never_skip_a_concurrent_commit` |
| Duplicate/out-of-order, same-cursor conflict and 64-bit decimal string | `apps/web/scripts/committed-regression.mjs` |
| Exact budget money | `apps/web/scripts/exact-budget-regression.mjs`; Rust `typed_wire_money_rejects_fractional_negative_and_huge_values` |
| Cross-campaign public streams and saves | `distinct_campaign_sse_and_command_namespaces_never_cross`; API smoke separate campaign and fork hash tests |
| Role visibility / late-quarter report | `public_evidence_and_late_report_exclude_private_role_fields` |
| Replay, credentials and external spend | Local scripted mode makes zero provider requests. `MYTH_MODE=live` and non-loopback binding are rejected in smoke. Existing Hiyal replay and P5 allowance tests are inherited; no invoiced OpenRouter results are claimed. |

**Boundaries:** The tests include OS-process SIGKILL after a committed decision and controlled failpoints at other state transitions, not an exhaustive power-loss campaign on every filesystem and kernel. Advisory locks have local-filesystem semantics; network shares require separate validation. JavaScript browser assertions use a pure reducer under Node, not a real screen reader. No replay is interpreted as verification of latent social dynamics.

## C′ is separate and not performed

The actual live OpenRouter route is disabled in the connected service; model selection, provider/pricing binding, explicit owner-approved spending, invoice reconciliation, response-time distribution and invalid-action results are therefore **unobserved**. [C′ runbook](p6-cprime-runbook.md) defines these gates. No author or reviewer should represent zero-model-call CI timings as a passed live experiment.

**Recommendation:** Accept the P6-03/05 *offline engineering implementation* once all checks on this PR head are green; keep P6-06 and independent scientific/live evidence open. Seek an independent review before merging, especially of concurrent-effect recovery, OS lock scope, failure classification, privacy projections and budget accounting.
