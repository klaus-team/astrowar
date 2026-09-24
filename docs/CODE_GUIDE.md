# Code guide — AstroWar

Architecture boundaries: [ARCHITECTURE.md](./ARCHITECTURE.md).
Locked rules: [DECISIONS.md](../DECISIONS.md).
Git / PR process: [CONTRIBUTING.md](../CONTRIBUTING.md).

---

## General

| Do | Avoid |
| -- | ----- |
| Clear names and small modules | Comments that restate the code |
| English identifiers, commits, branches, PRs | Mixing Portuguese into symbols or commit subjects |
| Mirror an existing pattern in the same crate | Inventing a new layering style for one PR |
| Prefer `make` targets for check/build/run | Ad-hoc Docker flags that bypass the Makefile |

---

## Protocol (`crates/protocol`)

| Do | Avoid |
| -- | ----- |
| Keep lobby messages in `ClientMessage` / `ServerMessage` | Putting asteroid/ship sim fields in lobby enums |
| Bump `PROTOCOL_VERSION` when messages break | Silent incompatible JSON changes |
| Keep room rules (max players, durations) aligned with DECISIONS | Adding public room listing or P2P fields |

Gameplay payloads belong in `crates/game/src/game_sync.rs` and travel inside `Relay`.

---

## Server (`crates/server`)

| Do | Avoid |
| -- | ----- |
| Room create/join/start/leave logic in `room.rs` | Gameplay simulation (spawns, hits, scores) on the server |
| Treat `Relay` payloads as opaque bytes | Parsing `GameMessage` in the relay |
| Parametrize bind/TTL via env ([ENVIRONMENT.md](./ENVIRONMENT.md)) | Hard-coding production hostnames |

---

## Client — net (`crates/net` + `net_bridge`)

| Do | Avoid |
| -- | ----- |
| Use `net::Client` for WS lobby I/O | Blocking the Bevy frame on network |
| Bridge async work through `NetBridge` commands/events | Spreading raw Tokio sockets across systems |

---

## Client — Bevy (`crates/game`)

| Do | Avoid |
| -- | ----- |
| Menu / session flow in `main.rs` systems gated by `AppState` | Spawning asteroids or scoring from menu handlers |
| Simulation, prediction, phases, sprite sync in `playing.rs` | Duplicating host rules on non-owner clients as authority |
| Fire SFX via `SfxTrigger` / `emit_sfx` and respect `AudioMuted` | Spawning audio when muted |
| Keep board coordinates in `board` logical space | Assuming window pixels == play area |
| Persist nick/scores through `storage` / `nickname` / `highscores` | Writing under arbitrary paths outside the OS data dir helpers |

Sideways-only ship motion and score-phase difficulty live in `playing.rs`; change them there, not in UI text alone.

---

## Security and privacy

- No peer IP exposure between clients.
- No production hostnames or secrets in tracked files; inject via env / CI.
- Do not log tokens or private connection strings beyond what local debug needs.
