#!/usr/bin/env bash
# Shared fail-closed credential pattern scanner; never prints matching secrets.
set -euo pipefail
mode=${1:-}
openrouter_prefix='OPENROUTER_API_'
key_suffix='KEY'
pattern="sk-or-v1-[A-Za-z0-9]{16,}|${openrouter_prefix}${key_suffix}=[^[:space:]]+"
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

if [ "$mode" = tracked ]; then
  git ls-files -z > "$tmp"
  mapfile -d '' -t files < "$tmp"
  if [ "${#files[@]}" -eq 0 ]; then
    echo 'Secret scan cannot run: no tracked files' >&2
    exit 1
  fi
  for file in "${files[@]}"; do
    if [ ! -f "$file" ]; then
      echo "Secret scan cannot read tracked file: $file" >&2
      exit 1
    fi
    rc=0
    grep -qaE "$pattern" "$file" || rc=$?
    if [ "$rc" -eq 0 ]; then
      echo "Credential-like content found in $file (value suppressed)" >&2
      exit 1
    fi
    if [ "$rc" -ne 1 ]; then
      echo "Tracked-file scan failed ($rc): $file" >&2
      exit 1
    fi
  done
elif [ "$mode" = dist ]; then
  dir=${2:-}
  if [ -z "$dir" ] || [ ! -d "$dir" ]; then
    echo 'Compiled-asset scan directory missing' >&2
    exit 1
  fi
  find -L "$dir" -type f -print0 > "$tmp" || {
    echo 'Compiled-asset file enumeration failed' >&2
    exit 1
  }
  if [ ! -s "$tmp" ]; then
    echo 'Compiled-asset directory is empty' >&2
    exit 1
  fi
  rc=0
  grep -RqaE "$pattern" "$dir" || rc=$?
  if [ "$rc" -eq 0 ]; then
    echo 'Credential-like content found in compiled assets (value suppressed)' >&2
    exit 1
  fi
  if [ "$rc" -ne 1 ]; then
    echo "Compiled-asset scan failed ($rc)" >&2
    exit 1
  fi
else
  echo 'Usage: scan-secrets.sh tracked | dist <directory>' >&2
  exit 2
fi
