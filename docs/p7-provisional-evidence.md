# P7 provisional implementation and acceptance boundary

**Branch:** `feat/p7-six-quarter-history`, stacked on the unmerged P6 draft PR #14. **Status:** implementation in progress; P7 and the full first-playable gate are **not complete**.

## Implemented in this slice

- Derived, read-only public delivery reports for each completed quarter, from the existing P3 reducer's `player_reports()`. These are reported and scheduled cumulative counts; **neither is true unaudited delivery**. Each report includes the authored weighted-support signal and next-quarter organization flags.
- Report history is tied to the same P6 verified Hiyal cursor and version as the rest of the public snapshot. The server bounds it to six ordered periods, preserving the role-visibility restriction. No frontend policy rules or private efforts are exposed.
- Fork provenance comes from the persisted, checked P6 fork marker. The UI distinguishes **exact recorded-action journal replay** from a **fresh scripted fork**, noting that the exogenous stream is inherited but subsequent actor actions are *not* held fixed.
- Frozen news-bundle entries are classified as real source-update *metadata* or wholly fictional simulation headlines. The cutoff is matched to the committed scenario; only archived real-source items can carry source links. No live news scraping or LLM-generated news is implied.
- Offline Rust full-campaign regression added: scripted Hiyal actions through Q6, six ordered public reports, exact 100-credit conservation, strict journal hash before and after restart, and stable public view identity.

## Remaining acceptance gates (do not check tracker items yet)

1. **P6:** independent review and outstanding fixes, then separately approved capped live C′ run. This branch must be rebased/retested onto the accepted P6 revision.
2. **P7-01:** finish/expose supported audits, commitment deadlines and explanations; demonstrate *two contrasting* feasible played resolutions and re-run P3-07 if mechanics change.
3. **P7-02:** implement verified state/receipt-grounded assistant country setup, briefings and outcome reports through a bounded, authorized OpenRouter route; unsupported policy ideas remain non-executable drafts. Current server is intentionally offline/scripted.
4. **P7-03:** expand public traceability to individual cost/delivery and approved agreement events, respecting private visibility, and compare diverging branches only with correct counterfactual labels.
5. **P7-04:** execute bounded A′ pilot **after C′**, with frozen inputs, allowed calls, failure measures and fallback record. **No paid calls authorized or executed by this PR.**
6. **P7-05:** actual observed convenience playtest and corrections; no invented usability evidence.
7. **P7-06:** execute full command/handoff and cross-OS replay verification, cost and latency evidence, released framing and exact release revision.

## Suggested local checks

```sh
cargo +1.99.1 fmt --all --check
cargo +1.99.1 test --workspace --locked
npm --prefix apps/web ci --ignore-scripts --no-audit --no-fund
npm --prefix apps/web run test:connected
npm --prefix apps/web run build
```

The above are **commands to run**, not claimed test results. No Rust compiler is available in the current authoring container; rely on executed GitHub CI and independent review before accepting changes. Live calls and broad OS CI are intentionally excluded.

### Connected audit input (added)

The connected player may issue an idempotent `order_audit` command with a typed family and an inspected, completed quarter. P3 enforces timing, historical eligibility, the first-free/later-two-credit rule, and duplicate audit validation. The public snapshot intentionally **does not reveal player-private audit results or minister-private efforts**; before exposing the player audit ledger, implement an authenticated chair-specific visibility boundary. The audit command's completion status and treasury balance remain observable. This is not P7-01's completed audit/commitment experience.
