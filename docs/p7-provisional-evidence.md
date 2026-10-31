# P7 provisional implementation and acceptance boundary

**Branch:** `feat/p7-six-quarter-history`, stacked on unmerged P6 draft PR #14. **Status:** offline implementation and verification are in progress; P7 and the first-playable acceptance gate are **not complete**.

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

The above commands can be rerun locally. The core P7 offline regression and P6 inherited tests **executed successfully in Linux CI** at [run #41214815520](https://github.com/davletovb/simulation-app/actions/runs/41214815520), commit `a55f47c0f5201309b1cb861c35998d899015c9f4`. This includes 20 contracts tests, 68 domain tests, 33 Hiyal-adapter tests and 29 local API tests, plus P3 sensitivity drift, keyless loopback/checkpoint smoke, generated contract drift, Node SSE reducer, React build and secret scans. A subsequent cleanup/audit test commit requires its **own** green CI; the above run is not evidence for later commits. No paid inference or broad cross-OS P7 replay was executed.

### Connected audit input (added)

The connected player may issue an idempotent `order_audit` command with a typed family and an inspected, completed quarter. P3 enforces timing, historical eligibility, the first-free/later-two-credit rule, and duplicate audit validation. The public snapshot intentionally **does not reveal player-private audit results or minister-private efforts**; before exposing the player audit ledger, implement an authenticated chair-specific visibility boundary. The audit command's completion status and treasury balance remain observable. This is not P7-01's completed audit/commitment experience.

### Source-grounded country setup (added)

The P7 public evidence projection now binds the eight frozen `country-pack.json` source descriptors to the eight frozen `values.json` recorded headlines by stable variable ID, with strict size, schema, cutoff, exact ID uniqueness and unit checks. Public output retains original displayed values and caveats/quality flags, not a fabricated conversion to pilot credits. UI shows source URL and period per indicator and labels the mixed measurement universes. This is **deterministic evidence display**, not the still-required OpenRouter assistant or independent revalidation of each original publication.

### Additional connected public/private audit regression

At a Q3 open boundary after the approved Q1 package and Q2 delivery, the first free staffing audit inspects Q2. The test checks that the committed private P3 audit ledger contains one audit and that the public snapshot does not expose `inspected_cumulative`. The later-quarter continuation still uses the single authorized PERIOD actor, not an arbitrary player clock action. See the current PR CI for executed status.
