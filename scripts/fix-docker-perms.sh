#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

HOST_UID="$(id -u)"
HOST_GID="$(id -g)"

mkdir -p target .cargo-cache dist

PATHS=(target .cargo-cache dist Cargo.lock)
needs_fix=0
for path in "${PATHS[@]}"; do
  if [[ -e "$path" ]]; then
    owner="$(stat -c '%u' "$path" 2>/dev/null || stat -f '%u' "$path")"
    if [[ "$owner" != "$HOST_UID" ]]; then
      needs_fix=1
      break
    fi
  fi
done

if [[ "$needs_fix" -eq 0 ]]; then
  exit 0
fi

echo "Fixing Docker workspace ownership for uid ${HOST_UID}:${HOST_GID}..."
docker run --rm \
  -v "$ROOT:/workspace" \
  alpine:3.20 \
  chown -R "${HOST_UID}:${HOST_GID}" \
    /workspace/target \
    /workspace/.cargo-cache \
    /workspace/dist \
    /workspace/Cargo.lock
