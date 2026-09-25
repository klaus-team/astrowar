# AstroWar

AstroWar is an open-source multiplayer arcade shooter inspired by the classic
Atari game Astroblast. It is an unofficial fan project and is not affiliated
with, endorsed by, or associated with Atari.

## Workspace

| Crate | Role |
|-------|------|
| `crates/protocol` | Shared messages and room model |
| `crates/server` | Lobby + relay (`astrowar-server`) |
| `crates/net` | Client WebSocket helper |
| `crates/game` | Bevy client (`astrowar`) |

## Requirements

- Docker
- `make`
- Linux or WSL2 for **client** Docker targets (display + GPU passthrough)
- Server targets work on any Docker host

Collaborators do not need a host Rust install for the documented workflow.

## Validate locally

```bash
make env
make help
```

### Development (debug)

```bash
make dev-server    # terminal 1 — Docker, cargo watch, debug logs
make health        # expect ok
make dev-client    # terminal 2 — Docker Bevy (Linux/WSL2 + X11)
```

Or both:

```bash
make dev           # detached server + foreground client; stops server on exit
```

### Release build and run

```bash
make build-server  # Docker image astrowar-server:local
make build-client  # Linux binary at dist/astrowar
make start-server  # run release server container
make start-client  # run native host binary
```

`make build` runs `build-server` and `build-client`.

### Persistent server (Compose)

Optional. Local/dev can keep using `make start-server` (`docker run`). Compose is
handy on a VPS for `restart: unless-stopped` and a short command after each
image update (you can do the same with a longer `docker run`).

```bash
cp .env.example .env   # set ASTROWAR_HOST_PORT if 8080 is taken
docker compose -f docker/compose.yaml --project-directory . up -d --build
curl -fsS http://127.0.0.1:${ASTROWAR_HOST_PORT:-8080}/health
```

Publishes on `127.0.0.1` by default (`ASTROWAR_PUBLISH_ADDR`). Proxy `/health`
and `/ws` to that host port. Production hostnames stay out of the repo.

VPS deploys (no git clone) use `docker/compose.deploy.yaml` plus a local `.env`;
CI on `v*` tags publishes to GHCR and SSHs into that directory. See
[CONTRIBUTING.md](CONTRIBUTING.md).

### Notes

- Client Docker GUI needs `DISPLAY` and X11 (`/tmp/.X11-unix`). On WSL2, WSLg usually provides this.
- macOS/Windows hosts: use Docker for **server** only; client build/run via Linux CI artifacts or WSL2 until native pipelines exist.
- Default WebSocket URL: `ws://127.0.0.1:8080/ws` (see `.env.example`).

## Documentation

- [docs/INDEX.md](docs/INDEX.md) — documentation map
- [AGENTS.md](AGENTS.md) — entry for AI assistants
- [DECISIONS.md](DECISIONS.md) — locked project decisions
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — crates and runtime flows
- [docs/CODE_GUIDE.md](docs/CODE_GUIDE.md) — coding conventions
- [docs/CODE_REVIEW_GUIDE.md](docs/CODE_REVIEW_GUIDE.md) — PR review checklist
- [CONTRIBUTING.md](CONTRIBUTING.md) — how to contribute
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — community rules
- [docs/ENVIRONMENT.md](docs/ENVIRONMENT.md) — environment variables

## License

MIT. See [LICENSE](LICENSE).
