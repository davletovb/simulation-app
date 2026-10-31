# P6 Experiment C′ — admission and acceptance runbook

**Status: NOT RUN.** This document is a pre-declared checklist, not an experimental result. CI, the React app, and the default Axum API remain offline. Do not enable live routing merely to satisfy a checklist; the C′ server-side implementation must first pass an independent review of the P5 governed adapter, shared durable allowance, immutable model/provider/pricing route and player command receipt path.

## Preconditions for an authorized live trial

1. Complete P6-03/05 offline checks, including a full connected typed decision, process-kill and Hiyal mid-round recovery, consistent initial and fallback SSE boundaries, private-field visibility tests, and browser decimal-cursor tests. Freeze the passing Git SHA and record exactly which Linux/macOS targets were executed.
2. Obtain the operator's **explicit approval of one bounded C′ evaluation**, with chosen model, exact OpenRouter provider, validated structured output, prices, declared batch ID and credits. Absence of approval or credentials means **zero calls**. Store keys only in the backend's private runtime configuration; never in GitHub Actions, web bundles, example commands or error logs.
3. Reuse the single-installation durable allowance: no campaign-spawn resets, no unsafe automatic paid retries, no unverified charge refund. Apply P0-04's limits of $0.25/quarter, $1.50/trial, $3/evaluation batch and $10/installation initial funded epoch. Preserve the 16-request quarterly and 96-request trial ceilings, the immutable Hiyal per-trial budget and trial route.
4. Enable a **separate opt-in** connected live backend selected by the operator; the current `MYTH_MODE=live` deliberately fails. Never switch the default offline game to paid inference and never add OpenRouter calls to routine CI.
5. Require a distinct frozen complete P3 decision, with assistant and minister invocations counted independently. Do not deem a rejected or incomplete model action a vote, and never infer provider bills from upper-bound reservations.

## Required evidence per complete trial

| Measurement | Exact interpretation |
| --- | --- |
| Waiting time | Per model wave elapsed and total three-wave end-to-end (exclude human decision time); n, median and p95 |
| Typed model behavior | Valid returned/minimum planned slots, failed/invalid/repair counts, actual recorded first/final proposals and institutionally accepted votes |
| Connected decisions | Authoritative P3 before/after state, frozen proposal versions, treasury/commitments and relevant model receipts |
| Command recovery | Idempotent 202/200 result, no duplicated commit across process restart, matching cursor/view version |
| Model usage | Provider-reported input/output/reasoning tokens, missing usage and unknown costs separately |
| Financial truth | Conservative reserved micro-USD, provider estimate, independently verified bill (if available), unresolved liabilities and installation remaining allowance |
| Replay | Exact strict Hiyal state hash and journal cursor with zero additional external requests |
| Resources | Per-phase preview, action commit, quarter advance, restore and full verify; hardware/OS/toolchain, CPU/RSS/disk/fsync settings |
| Safety | Only OpenRouter used; immutable provider/response-model identity, no leaked keys/latent role fields, no unapproved fallback |

P0-04's **declared, not observed** performance targets: p95 local preview <=100 ms; Hiyal cabinet action commit <=150 ms; quarter advance commit <=250 ms; one LLM wave <=20 s and three-wave end-to-end <=60 s (excluding human time); prototype restore <=2 s; strict full verify <=5 s; peak RSS <=256 MiB and full six-quarter journal/checkpoint <=10 MiB. Report sample sizes and breaches separately; a single favorable offline CI timing is not evidence for p95 or live provider response.

## Decision procedure

Record each declared criterion as **pass, fail or inconclusive** with a corresponding file, command/receipt ID or measurement. A reported action from a fake provider is never counted as a live OpenRouter action. Unknown provider bills remain liabilities, not zero cost. If credentials, funding, independent route validation, or sufficient samples are missing, report **C′ inconclusive / not run** and leave P6-06 unchecked. Do not merge PR #14 as full P6 completion on the basis of the offline slice alone.

**Current evidence:** [P6 offline verification record](p6-connected-evidence.md). The default app produces no paid requests and does not implement authorized connected live dispatch; that is a genuine remaining acceptance task.
