#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Client Docker targets need a Linux host (or WSL2)." >&2
  exit 1
fi

if [[ -z "${DISPLAY:-}" ]]; then
  echo "DISPLAY is unset; cannot open a GUI client." >&2
  exit 1
fi

xhost +SI:localuser:"$(id -un)" >/dev/null 2>&1 || true
xhost +local: >/dev/null 2>&1 || true

XAUTH_HOST="${XAUTHORITY:-$HOME/.Xauthority}"
XAUTH_ARGS=()
if [[ -f "$XAUTH_HOST" ]]; then
  XAUTH_ARGS+=(-e "XAUTHORITY=/tmp/.Xauthority" -v "$XAUTH_HOST:/tmp/.Xauthority:ro")
else
  TMP_XAUTH="$(mktemp /tmp/astrowar-xauth.XXXXXX)"
  trap 'rm -f "$TMP_XAUTH"' EXIT
  touch "$TMP_XAUTH"
  if command -v xauth >/dev/null 2>&1; then
    xauth nlist "$DISPLAY" 2>/dev/null | sed -e 's/^..../ffff/' | xauth -f "$TMP_XAUTH" nmerge - 2>/dev/null || true
  fi
  XAUTH_ARGS+=(-e "XAUTHORITY=/tmp/.Xauthority" -v "$TMP_XAUTH:/tmp/.Xauthority:rw")
fi

GROUP_ARGS=()
if getent group video >/dev/null 2>&1; then
  GROUP_ARGS+=(--group-add "$(getent group video | cut -d: -f3)")
fi
if getent group render >/dev/null 2>&1; then
  GROUP_ARGS+=(--group-add "$(getent group render | cut -d: -f3)")
fi

DRI_ARGS=()
if [[ -d /dev/dri ]]; then
  DRI_ARGS+=(--device=/dev/dri)
fi

DEFAULT_URL="${ASTROWAR_DEFAULT_SERVER_URL:-ws://127.0.0.1:8080/ws}"

exec ./scripts/docker-run.sh client \
  --network host \
  -e DISPLAY \
  -e ASTROWAR_DEFAULT_SERVER_URL="$DEFAULT_URL" \
  -e RUST_LOG \
  -e WINIT_UNIX_BACKEND=x11 \
  -v /tmp/.X11-unix:/tmp/.X11-unix:rw \
  "${XAUTH_ARGS[@]}" \
  "${GROUP_ARGS[@]}" \
  "${DRI_ARGS[@]}" \
  -- \
  cargo run -p astrowar
