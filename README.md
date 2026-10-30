# Myth Maker

A political simulation game about governing with an AI cabinet: choose a country baseline, propose policies, negotiate commitments, and follow their consequences through institutions and society.

The project is starting a new implementation. Read the [project framework — v0.8.1](docs/framework.md) for the agreed design, proposed architecture, and implementation gates. A runnable game has not yet been built. A **local React interface preview** is included, but it does not execute policy decisions.

Follow the [implementation tracker](docs/implementation-tracker.md) for the delivery sequence, open decisions, acceptance checks, and completion evidence.

## Initial operation and cost policy

These implementation constraints supplement the reviewed framework; the framework remains unchanged.

- **Local first:** the Rust/Axum service will serve the built browser interface and API on one loopback origin, with Hiyal saves on local disk and exported backups. Public hosting is deferred.
- **OpenRouter only:** every application LLM call will use OpenRouter, including ministers, assistant briefings, reports, response repairs, offline authoring/calibration, and live-model evaluations. Direct-provider APIs and Seatline are outside the initial implementation.
- **No required paid infrastructure or subscriptions:** no paid hosting, managed database, search/news API, monitoring subscription, or GPU service is needed. Explicitly budgeted OpenRouter usage and applicable credit-purchase fees are the only planned service expense.
- **Bounded inference:** select an inexpensive schema-capable model, initially shared by the three ministers. Declare numeric call, token, repair/retry, and cost allowances before live use; pause when an allowance is exhausted. No automatic model upgrade, paid fallback outside the declared budget, or automatic credit recharge is part of the plan. Free variants are optional; their availability and quotas do not guarantee campaign capacity.
- **Keep routine checks offline:** use scripted actors and recorded responses for development and CI. Live OpenRouter evaluations will be small, manually triggered, and capped. Keep CI to one essential Linux job, with expensive or cross-platform checks run deliberately.
- **Keep credentials in the backend:** the OpenRouter key must stay outside the browser bundle, committed files, and recorded prompts/receipts. Local replay must work without an API key or network access.

This is the intended operating policy, not an implemented setup guide. The tracker includes writing and verifying runnable setup instructions. A later public deployment needs its own persistent-storage and ongoing free-tier assessment; a free frontend or temporary trial credits alone do not establish a free backend.

## Local interface preview (P1b)

This is an **interface shell**, not an operational simulation. It is laid out as a game screen with four views: **Command** (play a policy card or tune a custom split, with a credit-token meter, organisation reactions and a best-case delivery ceiling), **Board** (the three seats, approval rule and meeting phases), **Campaign** (quarter track, per-quarter actions, organisations and cohorts) and **Intel** (evidence status). Keys `1`–`4` switch views and `?` opens the briefing. Drafting changes only what is displayed; voting and advancing stay locked, and no minister inference, replay journal, real evidence ingestion or authoritative policy commit occurs. The delivery ceiling is a display-only optimistic upper bound from P0 lag and capacity rules, including attainable Community Access Network access bonuses from Q2 when access funding is at least 10. Volunteer participation is not guaranteed; this is not a simulation result.

Requirements: Node.js **22.12+ within the 22.x line** and npm, with network access on the first install. CI and the included `.nvmrc` use Node **22.16.0**; `apps/web/.npmrc` enforces the supported engine range. From the repository root:

```sh
nvm use "$(cat apps/web/.nvmrc)"  # optional when using nvm
npm --prefix apps/web ci --ignore-scripts
npm --prefix apps/web run dev
```

Open the localhost URL printed by Vite (normally `http://127.0.0.1:5173`). The web app binds to loopback only. To check the production interface build, run `npm --prefix apps/web run build`. The `/api` development proxy currently targets a **future** local Rust service on `127.0.0.1:8787`; no backend is running for this preview. Do not place an API key in `apps/web`, environment variables exposed to Vite, or browser code. Vite exposes only variables with the `MYTH_PUBLIC_` prefix; its build refuses exposed names containing `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, or `CREDENTIAL`. This guard does not replace a complete security review. The server/API implementation and Serde-generated wire contracts are still P1 work.

## Rust-powered offline preview (P1d)

Start the loopback-only Axum backend with `cargo +1.99.1 run --locked -p myth-local-api`, then the frontend with `npm --prefix apps/web run dev`. The UI obtains its optimistic delivery ceilings from Rust via `/api/preview`; without the server, it explicitly displays an unavailable state (no duplicate TypeScript simulation). The backend also provides clean-belief scripted cabinet vote **snapshots** and bounded checksum-verified local **choice** recordings. It does **not** perform actual policy decisions, LLM calls, authoritative saves or Hiyal replay. Live mode fails closed. See [offline backend setup and API](docs/p1-04-offline-backend.md) for exact commands, files and security boundaries.

## Typed transport contracts (P1c)

The separate `myth-contracts` Rust crate validates JSON commands and minister-action inputs before converting them to the pure domain types. Rust generates TypeScript declarations at `apps/web/src/generated/transport.ts` and JSON Schema files in `docs/contracts/`. The checked-in artifacts are enforced by CI: `cargo +1.99.1 run --locked -p myth-contracts --bin export-contracts -- --check`.

See [the P1-03 contracts note](docs/p1-03-contracts.md) for accepted shapes, privacy boundaries, generator commands and remaining model-route compatibility checks. The P1-03 transport crate alone is not an operating API; the separate P1d loopback adapter now supplies Rust-calculated optimistic capacity previews, but still does not execute a complete policy simulation.

## Legacy prototype

All pre-framework repository files are preserved on the [archive branch](https://github.com/davletovb/simulation-app/tree/archive), pointing to [79a1bd7](https://github.com/davletovb/simulation-app/commit/79a1bd734c8e04535e1957d267df52e868402863). That snapshot includes the old application, data, images, databases, tests, and frontend submodule reference.

The default branch contains the framework, implementation tracker, and this README. Existing Git history is retained.
