# P1-04 — Offline local backend slice

**Scope:** loopback-only, no paid inference, no Hiyal, no authoritative campaign transitions. This is an executable *service adapter* for the P1 interface and **not** the P3 institution/policy engine.

## Start

Rust **1.99.1**, Node **22.16.0** (as pinned in `apps/web/.nvmrc`), dependencies installed from committed locks.

From the repository root, in terminal one:

```sh
cargo +1.99.1 run --locked -p myth-local-api
```

In another terminal:

```sh
npm --prefix apps/web ci --ignore-scripts --no-audit --no-fund
npm --prefix apps/web run dev
```

Open the Vite localhost URL. The browser dev proxy forwards **only** `/api` paths to `http://127.0.0.1:8787`. The delivery chart and delivery-horizon warning now read **Rust** `/api/preview`; they show a distinct loading/unavailable state when the backend is not running, with a manual **Retry Rust preview** control. Slider updates are debounced by 180 ms; while updating, previous results are explicitly labeled stale and the current draft's shortfall remains unknown until the new response arrives. No TypeScript fallback calculator remains.

The service listens on **127.0.0.1:8787**, not `0.0.0.0`. Set `MYTH_BIND` to another numeric *loopback* address/port if needed. A `MYTH_BIND` without a loopback IP is rejected. There is no public host mode, no CORS enablement, and no intentional remote capability.

## Modes and credentials

`MYTH_MODE=scripted` is the default. `MYTH_MODE=recorded` additionally labels the local choice-record workflow; **both are completely offline**, share the same read/write routes and do **not** replay model outputs or an authoritative world. Only the two literal values are accepted.

`MYTH_MODE=live` is **rejected before binding or dispatch**, even if a provider key happens to exist in the environment. The backend neither loads nor forwards a model key in P1. This is intentional fail-closed behavior until P5 implements secret loading *on the server only*, conservative per-request pricing, and durable shared allowance reservations. No `VITE_*` / `MYTH_PUBLIC_*` key is required or supported.

## API

| Method and path | Input | Output and boundary |
|---|---|---|
| `GET /api/health` | none | offline mode, schema version, network=false, authoritative_simulation=false |
| `POST /api/preview` | `{"allocation":{"staff":50,"repairs":40,"access":10}}` | Optimistic Rust-computed per-family, six-quarter capacity ceiling; does not commit funding or simulate society |
| `POST /api/scripted/meeting` | same allocation | P0 **clean-belief scripted final vote snapshot only**, with Finance/Education/Infrastructure support thresholds and explicit no-ratification flag; not a two-round meeting |
| `PUT /api/recordings/{id}` | versioned `ScriptedRecord` with at most six ordered, validated quarterly allocations | Local choice record and SHA-256 (not a checkpoint or event journal) |
| `GET /api/recordings/{id}` | path ID | Validated, checksum-verified saved choice record; invalid/tampered files fail closed |

Example (with service running):

```sh
curl -fsS http://127.0.0.1:8787/api/health
curl -fsS -H 'Content-Type: application/json' \
  -d '{"allocation":{"staff":50,"repairs":40,"access":10}}' \
  http://127.0.0.1:8787/api/preview

curl -fsS -X PUT -H 'Content-Type: application/json' \
  -d '{"schema_version":1,"record_id":"pilot1","choices":[{"quarter":1,"meeting_id":"m1","allocation":{"staff":50,"repairs":40,"access":10}}]}' \
  http://127.0.0.1:8787/api/recordings/pilot1

curl -fsS http://127.0.0.1:8787/api/recordings/pilot1
```

## Save and export/recovery boundaries

By default, choice records live under **`local-data/saves/`** relative to the process working directory (excluded from Git). To choose a location, set the backend-only `MYTH_SAVE_DIR` environment variable before start. Back up/export a saved recording by **copying its JSON file**, and restore into the same directory by copying it back; the GET API checks its SHA-256 and schema. Each file stores `{payload, sha256}`, with the hash computed over canonical Serde JSON of the payload.

Writes serialize within one process, execute on Tokio's blocking thread pool, write a uniquely named same-directory temporary file, fsync the file and atomically rename. On Unix a directory fsync is attempted **after** replacement; if it fails, the API returns success with a server-side durability warning because the visible file has already changed. On systems without supported directory fsync, crash-durability is not guaranteed. This is not a fully verified multi-OS recovery protocol. File sizes and paths are bounded; recording IDs use lowercase ASCII letters, digits, `_` and `-` only to prevent case-insensitive filesystem collisions. Each write uses a unique exclusive temporary name so a crash orphan cannot block a new save; overwritten records replace their previous version. The service does **not** yet provide independent process locking, append-only event custody, Hiyal hashes, crash-recovery testing across operating systems, or a full authoritative save/export/import command. Those remain P4/P7 requirements.

Local request guards reject non-loopback Host and Origin values and require JSON on writes. Both preview and scripted meeting response DTOs are defined in `myth-contracts` and exported by Rust into `apps/web/src/generated/transport.ts`, with corresponding `docs/contracts/preview-response.schema.json` and `scripted-votes.schema.json`; CI enforces generator drift. This is **not authentication against another local process**. Do not expose the port through an internet reverse proxy or store private user data here.

## Verification and limitations

```sh
cargo +1.99.1 fmt --all -- --check
cargo +1.99.1 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.99.1 test --workspace --locked
bash .github/scripts/offline-api-smoke.sh
npm --prefix apps/web run build
```

The smoke test checks the running loopback service, Rust preview for 25 access units, clean-belief 3/3 votes, rejection of oversubscribed packages (400 rather than a fictional vote), save/restore exact response, rejected cross-origin and wrong content types, and fail-closed live/remote modes. All execution claims must be supported by **passing CI**, not by this document alone. Only Linux is in routine CI; macOS arm64 replay is still untested.

**After the P1 closure slice:** world/institution/receipt *types* and a server-private, checksum-verified world checkpoint now exist. The `myth-checkpoint` CLI only saves/validates a fictional Q1 fixture and does not expose this state to the public API. Still missing: P2 reviewed pack content; P3 authorized meetings, ratification, true minister effort, treasury transitions and society dynamics; P4 Hiyal journal and replay; P5 provider and shared-cost ledger; P7 complete campaign export/recovery. Never interpret these endpoints as verified real-world government forecasts.
