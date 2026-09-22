#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

KIND="${1:?usage: docker-run.sh <toolchain|client> [docker run args...] -- <command...>}"
shift

case "$KIND" in
  toolchain)
    IMAGE="${ASTROWAR_TOOLCHAIN_IMAGE:-astrowar-toolchain:local}"
    DOCKERFILE=docker/Dockerfile.toolchain
    ;;
  client)
    IMAGE="${ASTROWAR_CLIENT_IMAGE:-astrowar-client:local}"
    DOCKERFILE=docker/Dockerfile.client
    ;;
  *)
    echo "unknown kind: $KIND" >&2
    exit 1
    ;;
esac

docker build -f "$DOCKERFILE" -t "$IMAGE" .

DOCKER_ARGS=()
CMD=()
SEEN_SEPARATOR=0
DETACHED=0
for arg in "$@"; do
  if [[ "$SEEN_SEPARATOR" -eq 0 && "$arg" == "--" ]]; then
    SEEN_SEPARATOR=1
    continue
  fi
  if [[ "$SEEN_SEPARATOR" -eq 0 ]]; then
    DOCKER_ARGS+=("$arg")
    if [[ "$arg" == "-d" || "$arg" == "--detach" ]]; then
      DETACHED=1
    fi
  else
    CMD+=("$arg")
  fi
done

if [[ ${#CMD[@]} -eq 0 ]]; then
  echo "missing command after --" >&2
  exit 1
fi

TTY_ARGS=()
if [[ "$DETACHED" -eq 0 && -t 0 && -t 1 ]]; then
  TTY_ARGS+=(-it)
fi

exec docker run --rm \
  -v "$ROOT:/workspace" \
  -w /workspace \
  -e "CARGO_HOME=/workspace/.cargo-cache" \
  "${TTY_ARGS[@]}" \
  "${DOCKER_ARGS[@]}" \
  "$IMAGE" \
  "${CMD[@]}"
