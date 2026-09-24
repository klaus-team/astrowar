# AstroWar — guide for AI assistants

Tool-agnostic entry point (Cursor, Copilot, Claude Code, etc.). This file does
**not** replace the docs — it points at the canonical sources.

## 1. Map

Open **[docs/INDEX.md](docs/INDEX.md)** first and load **only** what the task needs.

Product and locked rules (do not invent beyond this): **[DECISIONS.md](DECISIONS.md)**.

On conflict between summaries and detail: **`docs/` and `DECISIONS.md` win**.

## 2. Immutable principles

1. **Four crates:** `protocol` (lobby JSON), `server` (lobby + opaque relay), `net` (WS client), `game` (Bevy).
2. **Two protocols:** lobby messages in `protocol`; gameplay `GameMessage` in `game_sync`, carried inside `Relay`.
3. **Host-authoritative** simulation; clients predict locally. No gameplay sim on the server.
4. **No P2P** game path; **no peer IP** sharing; rooms are in-memory on the reference server.
5. **Make + Docker** for the server always; prefer Docker for client debug/build on Linux/WSL2.
6. **No production hostnames or secrets** in tracked files — env / CI only.
7. Prefer clear structure over obvious comments. English in code and git; see below.

Detail: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/CODE_GUIDE.md](docs/CODE_GUIDE.md).

## 3. Communication

- Chat and planning with the human: **pt-BR**.
- Code identifiers, file names, commits, branches, PRs: **English** — [CONTRIBUTING.md](CONTRIBUTING.md).
- Do not paste entire guides into replies; **cite** file and section.
- If a rule is missing or ambiguous: **ask** before inventing product behavior.

## 4. Expected behavior

- Find an equivalent pattern in the same crate before inventing a new one.
- Do not invent modes, network topology, or room rules beyond [DECISIONS.md](DECISIONS.md).
- Keep PRs small; follow [docs/CODE_REVIEW_GUIDE.md](docs/CODE_REVIEW_GUIDE.md) before claiming done.
- Roadmap / scratch notes may live in gitignored `temp/` — not in `DECISIONS.md`.

## 5. Commands

Prefer **`make`** at the repo root:

```bash
make env
make help
make dev-server          # Docker relay (debug)
make health
make dev-client          # Docker Bevy client (Linux/WSL2 + X11)
make check               # cargo check via Docker toolchain
make test                # protocol / server / net tests via Docker
make build               # release server image + Linux client binary
make start-server
make start-client        # native binary under dist/
```

Env vars: [docs/ENVIRONMENT.md](docs/ENVIRONMENT.md). Human setup: [README.md](README.md).
