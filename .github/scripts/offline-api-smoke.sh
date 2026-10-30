#!/usr/bin/env bash
set -euo pipefail
TMP=$(mktemp -d)
PORT=18787
PID=''
cleanup() {
  if [ -n "$PID" ]; then kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; fi
  rm -rf "$TMP"
}
trap cleanup EXIT

cargo build --locked -p myth-local-api
MYTH_SAVE_DIR="$TMP/saves" MYTH_BIND="127.0.0.1:$PORT" MYTH_MODE=scripted \
  target/debug/myth-local-api >"$TMP/server.log" 2>&1 &
PID=$!
BASE="http://127.0.0.1:$PORT"
READY=''
for _ in $(seq 1 40); do
  if curl -fsS "$BASE/api/health" >"$TMP/health"; then READY=yes; break; fi
  sleep 0.15
done
if [ -z "$READY" ]; then echo 'Offline API did not start' >&2; cat "$TMP/server.log"; exit 1; fi
grep -q '"network_calls":false' "$TMP/health"
grep -q '"authoritative_simulation":false' "$TMP/health"

curl -fsS "$BASE/api/preview" -H 'Content-Type: application/json' \
  -d '{"allocation":{"staff":0,"repairs":0,"access":25}}' >"$TMP/forecast"
grep -q '"calculator":"myth_domain"' "$TMP/forecast"
grep -q '"per_quarter":\[4,5,5,5,5,1\]' "$TMP/forecast"

curl -fsS "$BASE/api/scripted/meeting" -H 'Content-Type: application/json' \
  -d '{"allocation":{"staff":50,"repairs":40,"access":10}}' >"$TMP/votes"
grep -q '"support_count":3' "$TMP/votes"
grep -q '"institutional_ratification":false' "$TMP/votes"

# Unfunded, over-budget packages are invalid requests, not three legitimate no-votes.
status=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/scripted/meeting" \
  -H 'Content-Type: application/json' \
  -d '{"allocation":{"staff":60,"repairs":40,"access":10}}')
test "$status" = 400

curl -fsS -X PUT "$BASE/api/recordings/pilot1" -H 'Content-Type: application/json' \
  -d '{"schema_version":1,"record_id":"pilot1","choices":[{"quarter":1,"meeting_id":"m1","allocation":{"staff":50,"repairs":40,"access":10}}]}' \
  >"$TMP/saved"
curl -fsS "$BASE/api/recordings/pilot1" >"$TMP/restored"
cmp "$TMP/saved" "$TMP/restored"

status=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/preview" \
  -H 'Origin: https://foreign.example' -H 'Content-Type: application/json' \
  -d '{"allocation":{"staff":50,"repairs":40,"access":10}}')
test "$status" = 403
status=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/api/preview" \
  -H 'Content-Type: text/plain' -d '{}')
test "$status" = 415
if MYTH_MODE=live target/debug/myth-local-api >"$TMP/live.log" 2>&1; then
  echo 'Unimplemented live mode unexpectedly started' >&2
  exit 1
fi
if MYTH_BIND=0.0.0.0:9000 target/debug/myth-local-api >"$TMP/remote.log" 2>&1; then
  echo 'Remote binding unexpectedly succeeded' >&2
  exit 1
fi
echo 'Offline API smoke: PASS (health/preview/scripted/save-restore/guards)'
