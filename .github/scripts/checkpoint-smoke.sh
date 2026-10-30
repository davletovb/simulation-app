#!/usr/bin/env bash
set -euo pipefail
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
hash=1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
cargo run --locked -p myth-local-api --bin myth-checkpoint -- save-fixture "$tmp" example "$hash"
cargo run --locked -p myth-local-api --bin myth-checkpoint -- verify "$tmp" example "$hash"
test -f "$tmp/example.world.json"
if cargo run --locked -p myth-local-api --bin myth-checkpoint -- verify "$tmp" example "${hash/a/b}"; then
  echo 'Wrong pack hash accepted' >&2
  exit 1
fi
# Restore rejects a locally tampered record even if its JSON parses.
sed -i 's/"quarter":1/"quarter":2/' "$tmp/example.world.json"
if cargo run --locked -p myth-local-api --bin myth-checkpoint -- verify "$tmp" example "$hash"; then
  echo 'Tampered checkpoint accepted' >&2
  exit 1
fi
echo 'Private checkpoint save / restart / tamper gate PASS'
