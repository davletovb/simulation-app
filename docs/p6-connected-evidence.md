# P6 — Connected offline service: implementation and evidence

**Status:** Implemented as a draft, offline-first connected prototype in [PR #14](https://github.com/davletovb/simulation-app/pull/14). **Do not mark the full P6 gate complete:** the live Experiment C′, independently authorized OpenRouter route and measured usage/billing, real-user playthrough, and extended fault matrix remain outstanding. The local API explicitly rejects live mode; no paid model calls are made during CI.

## Executable path

The Rust Axum server owns local campaign state, Hiyal journals and one writer per campaign. The React client uses TanStack Query for validated committed snapshots plus SSE with a pre-subscribed receiver, a same-boundary `campaign_id:cursor` event ID and `view_version`. The reducer ignores older cursors, detects two different view versions at the same cursor, and refetches verified snapshots after disconnect. Model/tool receipts, private world state and provider keys never appear in SSE.

A command is validated into a versioned typed envelope and persisted with a SHA-256 integrity envelope and filesystem sync **before** HTTP 202. Its campaign/command ID and normalized payload define idempotency. Identical retries return the same operation; changed-payload ID reuse or stale expected state are conflicts, not second actions. Accepted operations survive the original HTTP connection. On restart, a command which already committed to Hiyal is marked completed without stepping the world again; pending Hiyal rounds use existing receipt-bound replay.

### Local operation (no hosted or paid services)

From the repository root after installing the pinned Rust and Node versions:

```sh
npm --prefix apps/web ci --ignore-scripts --no-audit --no-fund
npm --prefix apps/web run build
cargo +1.99.1 run --locked -p myth-local-api
```

The server runs at `http://127.0.0.1:8787` by default. Default `MYTH_MODE=scripted`, with the private Hiyal campaign journals under `local-data/saves/hiyal-campaigns`. `MYTH_SAVE_DIR` changes the parent private save directory; `MYTH_WEB_DIST` may point to a separate built-asset directory, never to private saves. Vite development proxy uses the same loopback API origin. No OpenRouter key is read by this mode.

The public connected API includes:

| Route | Purpose |
| --- | --- |
| `GET /api/campaigns/{id}/snapshot` | Verified replay-compatible public boundary |
| `POST /api/campaigns/{id}/commands` | Durable, idempotent typed command |
| `GET /api/campaigns/{id}/commands/{command_id}` | Recover status after lost HTTP acknowledgment |
| `GET /api/campaigns/{id}/events` | SSE with catch-up snapshot, `Last-Event-ID` fallback and keepalive |
| `POST /api/campaigns/{id}/forks` | Exact local offline fork at a verified quiescent `Resolved` decision boundary |

A typed proposal request has the **nested command kind** shape:

```json
{
  "command_id": "example-1",
  "expected_world_version": "0",
  "kind": {
    "kind": "propose",
    "allocation": {"staff": 50, "repairs": 40, "access": 10}
  }
}
```

The Rust engine, not TypeScript, authorizes phases, amendments, treasury commitments and ratification. The UI keeps the same durable command ID through reload/lost acknowledgment, and polls the saved operation until it completes or fails. A 202 does not mean that the game move succeeded. Numeric credits are checked against the *currently available* treasury using safe integers; state versions and cursors are strings.

### Fork and comparison

An offline fork is accepted only with a matching **committed cursor and world version**, after a final decision has reached `Resolved`, with no pending round or accepted command. The service copies only the verified Hiyal manifest, events, effects and snapshot into a private staged directory, verifies its exact hash and cursor, then atomically publishes the branch. It preserves the original Hiyal genesis/stream state, including declared exogenous shocks; the original and branch then have separate journal directories, command IDs and SSE namespaces. Neither live-model receipts nor credential files are copied. The UI exposes the independent branch via a `?campaign=<branch-id>` link.

### Offline tests and what they establish

The PR includes Rust tests for the full typed proposal → first cabinet round → amendment → final round → ratification → consequences → exact save/restore flow; changed-payload conflict and lost acknowledgment across restart; source/branch hashes and independent divergence; concurrent commit at the subscribe/snapshot boundary; duplicate/outdated catch-up handling; SSE cross-campaign isolation; accepted-status crash recovery after an already-committed action; and a mid-round failpoint after effect receipts but before all minister actions. Existing P4/P5 tests additionally validate receipt-bound deterministic replay and fail-closed model admission.

The loopback smoke now exercises the actual nested command wire format, HTTP 202/200/409 results, committed view, cross-campaign isolation, and foreign-origin rejection.

The focused timing test produced one observed measurement on [GitHub Actions CI #41204694487](https://github.com/davletovb/simulation-app/actions/runs/41204694487):

| Measurement | Observed CI runner sample |
| --- | ---: |
| Campaign creation | 5 ms |
| Optimistic Rust preview | 0 ms (rounded to nearest millisecond) |
| Accepted command through committed phase | 29 ms |
| Strict complete journal verification | 1 ms |
| Restart recovery and committed snapshot | 4 ms |
| Model calls | 0 |
| Paid infrastructure | $0 |
| Live model usage and invoice | **Not measured** |

These are single-run, uncalibrated offline samples and **not** user latency guarantees. They are not live cabinet waiting time, P0-05 behavior evidence or billed OpenRouter usage.

### Outstanding P6 acceptance

1. **P6-03:** Static preview panels still need full connected briefing/evidence, responsive cabinet and outcome/report integration and the declared selective component styling. The connected decision room is functional but not a complete first-playable UX.
2. **P6-05:** Additional independently terminated multi-process fault cases, exhaustive forced SSE reconnections, privacy/visibility audit, exact numeric round-trip matrix, and late-report assertions remain necessary beyond the executed tests.
3. **P6-06 / Experiment C′:** The actual capped live connected pilot must be separately authorized, with reviewed model/provider and credentials; record live waiting, invalid responses, previews, restore/verification, and invoiced versus reserved costs against the previously declared P0 targets. No live run is claimed, and the code does not automatically turn on paid inference.

**Gate policy:** Keep PR #14 draft/unmerged until independent review and the missing acceptance conditions are met. A passing offline job is evidence for the offline mechanism, not for LLM behavior, forecasting validity, user comprehension, or live economics.
