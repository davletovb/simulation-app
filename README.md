# Myth Maker

A political simulation game about governing with an AI cabinet: choose a country baseline, propose policies, negotiate commitments, and follow their consequences through institutions and society.

The project is starting a new implementation. Read the [project framework — v0.8.1](docs/framework.md) for the agreed design, proposed architecture, and implementation gates. A runnable game has not yet been built. The first **offline React interface preview** is available on the stacked P1b branch; it does not execute policy decisions.

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

This is an **interface shell**, not an operational simulation. Selecting a candidate changes only the displayed allocation; no minister inference, replay journal, real evidence ingestion or authoritative policy commit occurs.

Requirements: Node.js **22.16.0** and npm with network access on the first install. In the repository root:

```sh
npm --prefix apps/web ci
npm --prefix apps/web run dev
```

Open the localhost URL printed by Vite (normally `http://127.0.0.1:5173`). The web app binds to loopback only. To check the production interface build, run `npm --prefix apps/web run build`. The `/api` development proxy currently targets a **future** local Rust service on `127.0.0.1:8787`; no backend is running for this preview. Do not place an API key in `apps/web`, environment variables exposed to Vite, or browser code. The server/API implementation and Serde-generated wire contracts are still P1 work.

## Legacy prototype

All pre-framework repository files are preserved on the [archive branch](https://github.com/davletovb/simulation-app/tree/archive), pointing to [79a1bd7](https://github.com/davletovb/simulation-app/commit/79a1bd734c8e04535e1957d267df52e868402863). That snapshot includes the old application, data, images, databases, tests, and frontend submodule reference.

The default branch contains the framework, implementation tracker, and this README. Existing Git history is retained.
