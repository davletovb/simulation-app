# P7 — local campaign handoff and verification

**Work branch:** `feat/p7-six-quarter-history` stacked on P6 #14. This is a **candidate** handoff. Do not describe the full P7 first-playable gate as passed until the remaining evidence is obtained.

## Local operation

Operating baseline: Rust **1.99.1**, Node **22**, local-first English (`en-GB`), `127.0.0.1:8787`. No OpenRouter key is needed in offline/scripted mode. Always keep private saves separate from `apps/web/dist`.

```sh
npm --prefix apps/web ci --ignore-scripts --no-audit --no-fund
npm --prefix apps/web run test:connected
npm --prefix apps/web run build
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --locked
MYTH_MODE=scripted cargo +1.99.1 run --locked -p myth-local-api --bin myth-local-api
```

Navigate to `http://127.0.0.1:8787/?campaign=demo-england`. A Q1 programme can allocate 50/40/10 or 60/30/10 pilot credits to staffing/repairs/access. After a valid cabinet vote, advance Q1; for Q2–Q6 **continue the existing programme** to retain its undelivered credits and see policy lags, or propose a new affordable programme. The quarter changes only after a committed advance; a 202 response does not mean success. The intended audit action starts in Q3 and may inspect a previously completed quarter with a funded scheduled activity. An ineligible audit is rejected, not fabricated.

## Strict local inspection and privileged operator history

**Stop the API server first**: the CLI now takes the same exclusive save-root OS writer lock as the API and refuses to inspect while the API owns it. Then run:

```sh
cargo +1.99.1 run --locked -p myth-local-api --bin myth-report -- \
  local-data/saves/hiyal-campaigns/demo-england
```

This validates the exact Hiyal journal, resumes replay-only, reconstructs the P3 campaign and checks the private authoritative digest before printing. The report contains committed treasury, public quarterly observations, privileged audit truth and minister-private commitment statuses. It is **privileged local operator/debug output, not a chair role view**: don't publish it via the static site, SSE, or repository. `myth-report` does not call OpenRouter; it does not independently prove data fidelity or empirical policy effectiveness.

For a backup, **stop the server**, copy the entire `local-data/saves/hiyal-campaigns` directory (including manifest, events, effects, private snapshots, and admission logs) into a private backup location, and preserve restrictive filesystem permissions. Restore the complete set to an empty private save directory, restart offline, and compare the output of `myth-report` against the pre-backup cursor and authoritative hash. Never reconstruct a campaign from the browser cache, copy only a checkpoint, merge branch directories, or strip unconfirmed spending reservations.

## Provenance, privacy and replay

- A direct replay retains the exact recorded actor actions, shock schedule and Hiyal effects; this is a proof of the *defined software state*, not a causal claim.
- A verified fork starts from a committed decision boundary and inherits the original shock stream to that point. It does **not** freeze new minister actions after the fork. Compare forked outcomes as alternate scripted worlds, not exact recorded-action counterfactual evidence.
- Public SSE and snapshots contain only public quarterly reports and frozen source metadata. Actual private minister effort, player-only audit inspection and non-disclosed commitment detail must stay out of public output.
- Eight archived country indicators keep reference periods, scope qualifiers and quality flags. Narrative overlays and simulated news remain fictional; actual UK statistics are not the pilot's fictional credits.
- P3-07's archived fixed-action sensitivity analysis remains the last scientific sensitivity result unless a changed rule requires rerun. No new rule coefficient, lag, or population construction was altered by this P7 UI/API work.

## Authorization and incomplete acceptance

The default API **rejects `MYTH_MODE=live`**. The app's P5 shared spending allowance is not a permit to start live inference. Never put `OPENROUTER_API_KEY` in source or the browser. A future connected live route must pass the independent P6 review and C′ runbook, then an explicitly authorized A′ pilot within its fixed batch allowance (with generated and provider-billed charges recorded separately). No fixture test substitutes for that evidence.

**Remaining required real-world checks:** P6 C′ approval, independent P6/P7 implementation review, actual A′ model pilot, observed convenience playtesting, and final Linux x86_64 + macOS arm64 full-campaign hash comparison for this exact revision. Historic P4 cross-OS results are useful baseline evidence, not a P7 cross-OS pass.

**Known release limitations:** English-only; tiny six-cell artificial population; toy fiscal credits; no prediction of educational attainment, growth, welfare or UK political behaviour; no hosted services or online multiplayer; narrative/assistant live responses are not yet validated.


**PR #16 review corrections:** Continue an unchanged Q2–Q6 programme in two explicit, idempotent steps: record the player's choice, then advance via PERIOD. The first step's P3 Defer is replay-bound; this cannot be bypassed by a stale orchestrator pause state. The audit button is Q3+/QuarterOpen only, has an explicit up-to-2-credit acknowledgement, and visibly says findings require an offline operator/debug inspection. This CLI is **not** a playable chair-specific role projection; minister-private commitments must never be copied into the player's UI.
