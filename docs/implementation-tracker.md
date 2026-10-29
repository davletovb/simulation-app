# Myth Maker — implementation tracker

**Updated:** 29 October 2026  
**Design baseline:** [framework v0.8.1](framework.md), published at [9c32c58](https://github.com/davletovb/simulation-app/commit/9c32c587e0b1cc40a3349bbef1022cf00c1b7c46)  
**Operating constraints:** [README — initial operation and cost policy](../README.md#initial-operation-and-cost-policy)  
**Status:** Planning complete enough to start P0; game implementation and execution evidence have not started.

The framework governs game and model design. The README records the later local-first, OpenRouter-only, and cost constraints for the initial implementation. This tracker turns those discussions into work and gates without editing the reviewed framework. Public hosting is not a prerequisite for the first playable.

## Tracking rules

Check an item only after its acceptance conditions are met. Record the implementation commit and relevant test output, report, or manual result in the evidence log below; distinguish executed checks from code or documentation that was only written. Document a blocker rather than checking a partial task. Stable IDs can be referenced in commits and later issues; split a large item into suffixes if needed and retain its original acceptance conditions.

Phases are dependency gates, not calendar estimates. Small preparatory tasks can overlap, but do not proceed past a failed gate or silently change a model assumption to make a test pass. Live calls always obey the README's operating policy.

## Repository setup already completed

- [x] **SET-01** Preserve the complete legacy tree on `archive` at `79a1bd734c8e04535e1957d267df52e868402863`, including binary data and the frontend submodule reference.
- [x] **SET-02** Publish the unchanged reviewed framework and a new README on `main` in `9c32c587e0b1cc40a3349bbef1022cf00c1b7c46`.

These are repository/documentation steps. No game feature, benchmark, or live-model evaluation is completed by them.

## First-playable target

Use one reviewed real-country snapshot with approximately 6–10 indicators and visible gaps; fictional finance, education, and infrastructure ministers; six weighted population cells; two rule-driven organized interests; one narrative overlay; and six quarterly decisions. The education-reform case must include conditional agreements, delivery and audits, and remembered commitments. Define three small policy families within that case, with at least two feasible resolutions that lead to different, explainable tradeoffs.

Near-term outcomes concern funded activity, staffing, access, capacity, and delivery. Longer-term learning or growth remains an explicitly uncertain projection unless a justified mechanism supports an earlier effect. Country selection, exact mechanisms, institutional rules, model, locales, and numeric resource targets are still open tasks.

## Delivery order

| Phase | Deliverable | Depends on | Initial state |
|---|---|---|---|
| P0 | Case contract and declared evaluation limits | Reviewed framework and operating policy | Next |
| P1 | Minimal workspace, typed contracts, and offline setup | P0's scope and contract choices | Not started |
| P2 | Reviewed country pack and authored scenario | P0; P1's pack schema | Not started |
| P3 | Pure rules and a deterministic baseline | P0–P2 | Not started |
| P4 | Hiyal integration, recovery, and small scaling check | P3 | Not started |
| P5 | OpenRouter backend and a consequential AI cabinet | P4; declared live-call limits | Not started |
| P6 | Local API/interface and connected-slice gate C′ | P2, P4, P5 | Not started |
| P7 | Complete six-period case, evaluation, and local handoff | P6 | Not started |

### P0 — fix the case and evaluation limits

- [ ] **P0-01 — Country and evidence boundary.** Select the first country and cutoff after a small data-feasibility check. Record the policy question, supported indicators, missingness, and initial interface locale(s). Do not infer a country choice from examples in the framework.
- [ ] **P0-02 — Case contract.** Specify the simplified institution, each actor's authority and leverage, approval/override rules, quarterly time step, two-round meeting boundary, and the three supported policy families. Define one education-delivery mechanism, its units, lags, constraints, and evidence or authored parameter basis.
- [ ] **P0-03 — Decision and information contract.** Define proposals, amendments, enforceable versus political commitments, capacity choices, audits, and permitted observations. Keep chat/speeches outside minister decision inputs. Specify the single evidence type and adjustable belief, with relevance, conflict, magnitude, and duplicate-use rules.
- [ ] **P0-04 — Targets and inference allowance.** Declare numeric call, input/output/reasoning-token, repair/retry, and cost allowances for a period, campaign, and evaluation batch. Set latency, commit, restore, verification, memory, and disk targets before measuring. The lean planning count is eight calls per period or 48 per campaign before extras; it is not a provider quote or an all-inclusive cap.
- [ ] **P0-05 — Comparison protocol.** Define the independent profile-only baseline, scoring features and weights, reservation behavior, finite candidate menu, and tie-breaker. Define A′'s concession/anchoring measures and adoption thresholds before its first pilot. Record which model/provider settings, observations, menus, prompts, locale, and shocks are fixed or intentionally varied.

**Gate:** The case has declared variables, units, authorities, information boundaries, and testable success conditions. Targets and live-call allowances have concrete values before execution. No country, model, or performance result is implied by this tracker.

### P1 — establish the minimal workspace and contracts

- [ ] **P1-01 — Reproducible foundation.** Create a small Rust workspace and React/TypeScript/Vite interface. Pin a compatible Rust toolchain, the reviewed Hiyal revision from the framework, package versions, and consumer lockfiles. Keep pure domain/rules code independent of Hiyal and I/O; group other components into modules until separate crates earn their cost.
- [ ] **P1-02 — Typed boundaries.** Define world, country-pack, policy, institution, role, phase, commitment, action, receipt, command, and visible-view types. Version persistent and wire contracts. Specify exact integer/fixed-point units, rounding, residual allocation, and overflow handling; use decimal-string wire fields where large integers exceed JavaScript's safe range.
- [ ] **P1-03 — Generated contracts.** Generate action schemas with `schemars` and frontend types with `ts-rs`. Align them with actual Serde serialization, strict input parsing, and the selected structured-output schema subset. Check representative serialized round trips and large-number boundaries.
- [ ] **P1-04 — Offline local setup.** Provide a scripted/recorded backend mode that starts without an API key. Define local save/export paths, backend-only credential loading, and live-call configuration that fails closed when required limits are missing. Document actual build/start/test commands once the scripts exist.
- [ ] **P1-05 — Lean checks.** Add one essential Linux CI job for formatting/linting, meaningful Rust checks, contract drift, frontend type checks, and build. Use one test execution path rather than duplicate suites. Keep live inference, broad OS matrices, and expensive benchmarks out of routine PR CI; cancel superseded runs and cache dependencies.

**Gate:** The workspace builds and its offline fixture path runs locally. Contract checks and the essential CI job pass without live inference or paid services. No application credential is included in client assets, fixtures, or source.

### P2 — prepare the reviewed country and scenario

- [ ] **P2-01 — Country-pack schema.** Define the small versioned JSON pack plus provenance/coverage manifest: geographic scope, indicator definition, raw and normalized values, units, reference period, release/retrieval dates, source, transformation, quality flags, and content hashes.
- [ ] **P2-02 — Frozen evidence.** Use public sources and an offline Python pipeline to assemble approximately 6–10 inputs. Preserve reported values and transformations, enforce the evidence cutoff, and label estimates or gaps. Loading a pack must not require live retrieval or silently refresh an ongoing run.
- [ ] **P2-03 — Population accounting.** Establish or explicitly qualify the six cells' weights and joint distribution. Document teachers/parents or other overlapping affiliations without counting them as extra population. Reconcile cell totals with the modeled aggregate and record omissions.
- [ ] **P2-04 — Authored case content.** Version fictional minister profiles, initial commitments, the institutional simplifications, parameters, and three supported policy families. Separate sourced facts, estimates, authored mechanisms, narrative changes, and simulated outputs. The framework's 100 discretionary units remain a toy example unless a documented transformation gives them a country-specific meaning.
- [ ] **P2-05 — Narrative and archive.** Add one explicit narrative overlay and a dated, sourced news bundle. Preserve the unmodified baseline and a visible diff; keep real reports distinct from simulation news. Review the pack and retain a coverage/assumption report.

**Gate:** Every starting value and mechanism parameter has provenance or a labeled assumption. The pack is reproducible and immutable for a run; overlays cannot overwrite the source snapshot. Raw Parquet/analytical tables stay outside the game/browser runtime.

### P3 — prove the pure rules with scripted actors

- [ ] **P3-01 — Financial lifecycle.** Implement affordability, approved borrowing if in scope, future staffing/maintenance obligations, tranche conditions, and exact stock/flow accounting. Recording a promise or ratification does not automatically disburse cash. Costs and upkeep apply exactly once.
- [ ] **P3-02 — Phases and time.** Implement proposal registration, frozen first positions, bounded amendments, frozen final positions/votes, ratification, and explicit period advance. Advance due spending, delivery, shocks, public responses, relationships, and observations in a declared order; bargaining must not accidentally consume a quarter.
- [ ] **P3-03 — Institutions and commitments.** Enforce authority, capacity, funding, proposal versions, and conditions. Implement the commitment registry, deadlines, and consequences of honored/broken promises. Institutional invalidity is separate from a permitted but politically poor choice; utility gating is not the default.
- [ ] **P3-04 — Society and organizations.** Implement the six rule-driven cells and two organizations, including at least one organization action with a modeled consequence. Reconcile money and population across levels, use declared lags, and feed outcomes into the next meeting's constituency pressure through explicit rules.
- [ ] **P3-05 — Views, previews, and audits.** Build role-specific observations and player previews from permitted information. Keep true effort/losses/private commitments separate from reports. Implement the small audit and evidence-to-belief mechanisms from P0, including repeated/conflicting evidence checks.
- [ ] **P3-06 — Baseline and invariants.** Implement the independent deterministic profile-only selector and scripted cases. Verify accounting, delay/order rules, invalid/stale actions, failed votes, exact rounding/overflow behavior, information visibility, and at least two feasible tradeoffs plus an unaffordable proposal. Reconcile mechanical receipts against state differences.

**Gate — Experiment B:** Headless scripted cases are deterministic and explainable, accounts reconcile, and invalid actions cannot mutate state. Binding agreements remain in the authoritative registry even when prompt memories are evicted. No LLM, network, or Hiyal dependency enters the pure reducer.

### P4 — integrate Hiyal and verify recovery

- [ ] **P4-01 — One game world.** Wrap the pure rules in the game-owned Hiyal `World`. Use cabinet actors and one period-advance actor; keep rule-driven cohorts inside world records. Maintain one writer per trial and keep full transcripts/trajectories outside mutable world state.
- [ ] **P4-02 — Execution phases.** Map eligible actors to audited pause/resume operations and frozen rounds. Preserve deterministic commit order and separate logical actions from calendar time. Use sequential steps for named policy RNG until a supported round RNG contract is verified.
- [ ] **P4-03 — Recorded inputs and player effects.** Version country/scenario/rules, setup/random state, observations, prompts, request settings, commitments, and concise rationales. Implement the allowlisted, side-effect-free `player_command` effect with command body, ID, expected version, and stable operation ID. Preserve enough durable association to reconstruct pending work for P6's service.
- [ ] **P4-04 — Durability and replay.** Start with `EveryEvent`, verified checkpoints, normal full validation, resume, and strict recorded-action replay without dispatch. Test interruption between effect receipt and action, and during a frozen round. Reuse durable receipts; retain unknown billing when a remote response might have been charged before saving. Do not claim exactly-once remote billing.
- [ ] **P4-05 — Small B′ check.** Measure prototype-size and one declared small stress world for cabinet/period commit time, checkpoint size, restore/full verification, memory, and disk. Run representative replay on a second supported OS deliberately. Keep the full 10²–10⁵-record study deferred.
- [ ] **P4-06 — Adoption decision.** Record which A–C/B′ requirements the adapter meets and any generic engine gaps. Retain Hiyal provisionally only with evidence; keep corrective engine work separate from game mechanisms and preserve the replaceable rules boundary.

**Gate:** Saved scripted runs restore and replay to identical authoritative hashes on the declared supported setup, without network access. Recovery evidence and small-world resource measurements meet P0's declared targets. An adapter gap is recorded before proceeding; this phase alone does not establish the live cabinet or final engine adoption.

### P5 — add the OpenRouter cabinet

- [ ] **P5-01 — Schema-capable backend.** Implement and test an OpenRouter-only Hiyal backend. Require supported structured-output parameters, verify each action schema, and record actual model/provider identity and request settings. Pin routes/settings for controlled comparisons; reject unapproved route/model fallbacks. Any bounded JSON-text fallback and repair stays on OpenRouter.
- [ ] **P5-02 — Admission and accounting.** Wrap calls with `GovernedBackend` and the configured shared local application allowance, including assistant/authoring calls outside trials. Bound concurrency, request/output/reasoning sizes, repairs/retries, and estimated spend. Recover trial reservations; distinguish reserved capacity, reported usage, estimates, verified charges when available, and unknown billing. Do not present this as a distributed wallet or exact provider invoice.
- [ ] **P5-03 — Ministers and memory.** Add three distinct profiles with role-visible typed observations, persistent agreements, bounded context, and independent first positions. Support amendments, conditions, final positions/votes, and authorized effort choices. Keep speeches/chat out of decision inputs and classify timeout/parse failures as operational failures, with an explicit visible retry or labeled fallback.
- [ ] **P5-04 — Cheap verification first.** Use captured or fake HTTP responses for malformed/schema-invalid output, missing usage, errors, cancellation, quota/budget exhaustion, and partial streams. Check that no key reaches receipts/prompts, that failures do not fabricate support, and that an exhausted allowance pauses new inference.
- [ ] **P5-05 — Capped live Experiment A.** Run a small, manually triggered cabinet evaluation within P0's allowance, using one model across ministers. Compare against the deterministic baseline. Record whether concessions/constraints change typed actions and implementation, with latency, tokens, invalid responses, and failure cases; change the action space if the LLM changes only dialogue.

**Gate — Experiment A:** At least one genuine disagreement changes a permitted action path, agreements persist, and invalid responses are handled correctly. The chosen model/route meets the declared schema, behavior, latency, and allowance criteria; routine checks still work offline.

### P6 — connect the local service and minimal interface

- [ ] **P6-01 — Durable command API.** Add Axum command/status endpoints and one owning task per live trial. Deduplicate by campaign/command ID and normalized payload; return an existing operation/result for identical retries, reject ID reuse with changed content, and check expected versions on new application. Return `202 Accepted` only after durable acceptance and recovery association, independently of the original HTTP connection.
- [ ] **P6-02 — Recoverable SSE.** Publish role-visible snapshots/updates with trial identity, committed cursor, and view version. Join catch-up to live delivery without a gap; handle `Last-Event-ID`, duplicates/older versions, cursor gaps, and snapshot fallback. Keep provisional generation progress separate from committed state and explicitly enable keep-alives. Wire TanStack Query to these server views without interpreting raw simulation rules in TypeScript.
- [ ] **P6-03 — Local player surface.** Serve the built React interface and API from one loopback origin, with Vite proxying during development. Build briefing/evidence, typed proposal, cabinet/amendment, and outcome views. Use selective re-themed Tailwind/shadcn/Base UI components, explicit application locales, and safe exact budget-entry contracts. Include inspectable assumptions and pending/failure states.
- [ ] **P6-04 — Connected decision.** Wire briefing → proposal/preview → positions → player amendment → final decision → quarterly consequences → durable save → report. Demonstrate reload, strict replay, and a fork from a verified committed decision boundary. Preserve the same declared exogenous shock schedule where comparisons require it.
- [ ] **P6-05 — Fault and cost checks.** Test lost HTTP acknowledgments, receipt-before-action crashes, mid-round restart, forced SSE disconnects, duplicate/out-of-order updates, visibility leaks, exact numeric round trips, and a late report. Check that recorded replay uses no API key, every live LLM request uses OpenRouter, and no paid infrastructure is needed.
- [ ] **P6-06 — C′ evidence and decision.** Run the connected slice with a small capped live evaluation after offline fault checks. Record waiting time, invalid actions, preview/commit/restore/full-verification metrics, and separate usage/billing categories against P0 targets. Record pass/fail and any revision before freezing implementation choices.

**Gate — Experiment C′ (including C):** One complete connected decision survives the specified interruptions, saves and replays correctly, and visibly connects bargaining to material consequences. Cost and resource evidence meets the declared limits. No hosting deployment or full large-population benchmark is required for this gate.

### P7 — finish and evaluate the first playable

- [ ] **P7-01 — Six-quarter case.** Complete the six-period loop and three small policy families with conditional agreements, delivery/audits, and remembered commitments. Show at least two feasible resolutions with different, traceable tradeoffs, including later consequences of a promise and an implementation problem.
- [ ] **P7-02 — Assistant and news.** Complete the assistant's country/evidence setup, briefing, and outcome reporting through the declared OpenRouter route. Fill or verify figures against state/receipts; distinguish archived real news, narrative overrides, projections, agent assertions, and simulation news. Keep unsupported policy ideas as drafts.
- [ ] **P7-03 — Comparison and history.** Provide inspectable decisions, agreements, costs, delivery, group responses, and evidence references. Distinguish exact recorded-action replay from fresh-agent reruns; record and display which actor actions or shocks are held fixed in a comparison.
- [ ] **P7-04 — Bounded A′ pilot.** Run the declared architecture/anchoring tests within their allowance after C′. Freeze comparison inputs as specified; record action diversity, genuine versus unexplained concessions, belief-update correctness, variability, fallbacks, and downstream differences. Adopt a utility gate only if it clears the previously declared criteria; otherwise retain LLM-led decisions with institutional checks.
- [ ] **P7-05 — Local playtest and correction.** Observe a small convenience playtest and record whether a player can explain a concession, an implementation problem, and a later promise consequence. Fix misleading or unusable flows. Report failure cases and authored-model limits; this is usability/agent evidence, not population-level validation.
- [ ] **P7-06 — Local handoff.** Verify the documented install/start/test/replay/export/restore commands on the declared supported environment. Include model/allowance configuration, save paths, backup recovery, offline behavior, and handling of quota or budget exhaustion. Summarize executed checks, observed campaign cost/waiting, remaining defects, and the exact release revision.

**Gate — first playable:** A user can complete the reviewed case locally and understand the main tradeoffs; evidence supports the correctness, behavior, usability, and resource claims made for this build. The core replay/visibility/accounting checks and local operating constraints hold through a complete campaign.

## Expansion backlog — not prerequisites for the first playable

- [ ] **E-01 — Larger populations.** Run the full B′ sizes and durability comparisons before adopting L3 or another larger explicit population. Profile world copying/hashing and recovery; do not relax durability or integrity checks without measured justification.
- [ ] **E-02 — Public access.** Assess an ongoing free host with durable storage, actual resource use, SSE/reconnect behavior, backups, access control, and shared admission limits. A free frontend or temporary credits alone do not satisfy this. Paid hosting requires a later explicit budget decision.
- [ ] **E-03 — Broader content and cabinets.** Add another country, mechanism, institution, or model-per-seat only when it answers a clear question and has its own evidence/benchmark coverage. Keep model traffic on the agreed gateway unless that policy is explicitly changed.
- [ ] **E-04 — Optional research/features.** Consider the offline opinion-panel calibration, richer coalitions/leaks/reshuffles, household microsamples, desktop/local inference, or shared game infrastructure separately. Do not introduce them to satisfy a missing first-case gate.

## Evidence log

Implementation evidence is empty at tracker creation. Add a row as work executes; do not replace measured results with expectations.

| Slice or gate | Implementation commit | Executed check / artifact | Result and remaining blocker |
|---|---|---|---|
| SET-01 / SET-02 | `9c32c587e0b1cc40a3349bbef1022cf00c1b7c46` | Git refs and tree verified when publishing the framework | Repository baseline only; all implementation gates remain open |

**Next work:** P0-01 through P0-05. Case feasibility, the institutional/phase contract, exact numeric targets, and the capped model-evaluation protocol determine the first implementation slice.
