#!/usr/bin/env bash
set -euo pipefail
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
hash=1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef
cargo run --locked -p myth-local-api --bin myth-checkpoint -- save-fixture "$tmp" example "$hash"
cargo run --locked -p myth-local-api --bin myth-checkpoint -- verify "$tmp" example "$hash"
test -f "$tmp/example.world.json"
if cargo run --locked -p myth-local-api --bin myth-checkpoint -- verify "$tmp" example "${hash/a/b}" >"$tmp/wrong-pack.log" 2>&1; then
  echo 'Wrong pack hash accepted' >&2
  exit 1
fi
grep -q 'PackMismatch' "$tmp/wrong-pack.log"
# The unkeyed in-file checksum rejects accidental corruption, NOT deliberate tampering.
sed -i 's/"quarter":1/"quarter":2/' "$tmp/example.world.json"
if cargo run --locked -p myth-local-api --bin myth-checkpoint -- verify "$tmp" example "$hash" >"$tmp/corrupt.log" 2>&1; then
  echo 'Corrupted checkpoint accepted' >&2
  exit 1
fi
grep -q 'Corrupt' "$tmp/corrupt.log"
echo 'Private checkpoint save / restart / corruption and pack-mismatch gate PASS'
