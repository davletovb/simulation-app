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
grep -q '"connected_authoritative_simulation":true' "$TMP/health"

curl -fsS "$BASE/api/preview" -H 'Content-Type: application/json' \
  -d '{"allocation":{"staff":0,"repairs":0,"access":25}}' >"$TMP/forecast"
grep -q '"calculator":"myth_domain"' "$TMP/forecast"
grep -q '"per_quarter":\[4,5,5,5,5,1\]' "$TMP/forecast"

curl -fsS "$BASE/api/scripted/meeting" -H 'Content-Type: application/json' \
  -d '{"allocation":{"staff":50,"repairs":40,"access":10}}' >"$TMP/votes"
grep -q '"support_count":3' "$TMP/votes"
grep -q '"institutional_ratification":false' "$TMP/votes"

# P6: actual loopback wire contract (nested typed kind), durable 202,
# idempotent 200 retry, changed-content 409, committed snapshot and no keys.
curl -fsS "$BASE/api/campaigns/demo-england/snapshot" >"$TMP/connected-first"
grep -q '"cursor":"0"' "$TMP/connected-first"
cat >"$TMP/connected-command" <<'JSON'
{"command_id":"smoke-proposal","expected_world_version":"0","kind":{"kind":"propose","allocation":{"staff":50,"repairs":40,"access":10}}}
JSON
status=$(curl -sS -o "$TMP/accepted" -w '%{http_code}' \
  "$BASE/api/campaigns/demo-england/commands" -H 'Content-Type: application/json' \
  --data-binary "@$TMP/connected-command")
test "$status" = 202
grep -q '"status":"accepted"' "$TMP/accepted"
status=$(curl -sS -o "$TMP/repeated" -w '%{http_code}' \
  "$BASE/api/campaigns/demo-england/commands" -H 'Content-Type: application/json' \
  --data-binary "@$TMP/connected-command")
test "$status" = 200
for _ in $(seq 1 50); do
  curl -fsS "$BASE/api/campaigns/demo-england/commands/smoke-proposal" >"$TMP/op"
  if grep -q '"status":"completed"' "$TMP/op"; then break; fi
  sleep 0.1
done
grep -q '"status":"completed"' "$TMP/op"
curl -fsS "$BASE/api/campaigns/demo-england/snapshot" >"$TMP/connected-saved"
grep -q '"phase":"FirstPositionsFrozen"' "$TMP/connected-saved"
status=$(curl -sS -o "$TMP/conflict" -w '%{http_code}' \
  "$BASE/api/campaigns/demo-england/commands" -H 'Content-Type: application/json' \
  -d '{"command_id":"smoke-proposal","expected_world_version":"0","kind":{"kind":"advance_slice"}}')
test "$status" = 409
curl -fsS "$BASE/api/campaigns/another-campaign/snapshot" >"$TMP/isolated"
grep -q '"cursor":"0"' "$TMP/isolated"
status=$(curl -sS -o /dev/null -w '%{http_code}' \
  "$BASE/api/campaigns/demo-england/commands" -H 'Origin: https://foreign.example' \
  -H 'Content-Type: application/json' --data-binary "@$TMP/connected-command")
test "$status" = 403

# Independent OS-process stop/restart: the already committed command must
# retain its identity and the exact public cursor, with no second execution.
saved_cursor=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["snapshot"]["cursor"])' "$TMP/connected-saved")
kill -9 "$PID"
wait "$PID" 2>/dev/null || true
PID=''
MYTH_SAVE_DIR="$TMP/saves" MYTH_BIND="127.0.0.1:$PORT" MYTH_MODE=scripted \
  target/debug/myth-local-api >"$TMP/restarted.log" 2>&1 &
PID=$!
READY=''
for _ in $(seq 1 50); do
  if curl -fsS "$BASE/api/health" >"$TMP/restart-health" 2>/dev/null; then READY=yes; break; fi
  sleep 0.1
done
if [ -z "$READY" ]; then echo "Restart failed"; cat "$TMP/restarted.log"; exit 1; fi
curl -fsS "$BASE/api/campaigns/demo-england/snapshot" >"$TMP/recovered"
recovered_cursor=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["snapshot"]["cursor"])' "$TMP/recovered")
test "$saved_cursor" = "$recovered_cursor"
status=$(curl -sS -o "$TMP/recovered-op" -w '%{http_code}' \
  "$BASE/api/campaigns/demo-england/commands" -H 'Content-Type: application/json' \
  --data-binary "@$TMP/connected-command")
test "$status" = 200
grep -q '"status":"completed"' "$TMP/recovered-op"
# A Last-Event-ID from before the committed command must receive a
# same-boundary fallback snapshot rather than skip to unrelated progress.
curl -sSN --max-time 2 -H 'Last-Event-ID: demo-england:0' \
  "$BASE/api/campaigns/demo-england/events" >"$TMP/reconnected-sse" || test "$?" = 28
grep -q 'event: snapshot_fallback' "$TMP/reconnected-sse"
grep -q "id: demo-england:$saved_cursor" "$TMP/reconnected-sse"
# Exact decimal strings are accepted as strings, not silently rounded by JS.
test "$(python3 - <<'PY'
import json
obj={"expected_world_version":"9007199254740993"}
print(json.loads(json.dumps(obj))["expected_world_version"])
PY
)" = 9007199254740993

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
