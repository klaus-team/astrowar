# Architecture — AstroWar

Locked product and stack rules: [DECISIONS.md](../DECISIONS.md).
Code conventions: [CODE_GUIDE.md](./CODE_GUIDE.md).

## Workspace

| Crate | Binary / lib | Role |
| ----- | ------------ | ---- |
| `crates/protocol` | lib | Lobby WebSocket JSON (rooms, hello, start, leave) |
| `crates/server` | `astrowar-server` | Lobby + opaque relay (Axum) |
| `crates/net` | lib | Client WebSocket helper over `protocol` |
| `crates/game` | `astrowar` | Bevy client (menus + gameplay) |

Dependency direction:

```text
astrowar (game) → net → protocol
astrowar (game) → protocol
astrowar-server → protocol
```

The server never depends on Bevy or game simulation crates.

## Two message layers

1. **Lobby** (`protocol::{ClientMessage,ServerMessage}`) — JSON on `/ws`.
2. **Gameplay** (`game::game_sync::GameMessage`) — serialized payload inside lobby `Relay` / `Relayed`. The relay treats bytes as opaque; it does not interpret asteroids or scores.

Bump `protocol::PROTOCOL_VERSION` when lobby messages become incompatible. Gameplay payload changes are coordinated by client versions playing together (same build family).

## Multiplayer data flow

```text
Host client                    Relay server                 Join client
───────────                    ────────────                 ───────────
CreateRoom / StartGame  ───►   rooms in memory
Input / Hit (GameMessage) ──►  broadcast Relayed      ───►  apply state
◄── Relayed (other peers) ◄──  (no gameplay logic)
```

- Peer IPs are never shared; all game traffic goes through the central relay.
- Rooms live in memory; restart clears them.
- Max 4 players; join by room code only.

## Authority model (idea-2 lite)

| Concern | Who decides |
| ------- | ----------- |
| Spawns, asteroid motion, scoring, lives, match over | **Host** (`HostSim` in `playing.rs`) |
| Local ship pose / fire feel | **Local client** prediction (`LocalPrediction`) |
| Authoritative ship score/lives shown | Host `State` snapshots |
| Solo | Local process is host; no WebSocket |

Difficulty **phase** is derived from the **leading score** among non-forfeited ships (`PHASE_SCORE_STEP = 500`). Effective modifiers clamp at phase 6; the HUD may show higher phase numbers in endless.

## Client UI states

`AppState` in `crates/game/src/main.rs`:

```text
MainMenu → SoloSetup | HostSetup | JoinSetup → Connecting → Lobby → Playing
```

Solo skips the relay and enters `Playing` with a local `HostSim`.

## Client modules (`crates/game/src`)

| Module | Responsibility |
| ------ | ---------------- |
| `main.rs` | App wiring, menus, `poll_net_events`, session |
| `playing.rs` | Host sim, prediction, collisions, phases, HUD sync, sprites |
| `game_sync.rs` | `GameMessage` + entity state DTOs |
| `net_bridge.rs` | Background Tokio thread ↔ Bevy events/commands |
| `board.rs` | Fixed 960×720 logical board + letterbox camera |
| `shapes.rs` | Procedural meshes (ship, asteroids) |
| `sounds.rs` | Procedural SFX + mute |
| `storage.rs` / `nickname.rs` / `highscores.rs` | OS data-dir persistence |

## Server modules (`crates/server/src`)

| Module | Responsibility |
| ------ | ---------------- |
| `main.rs` | HTTP `/health`, WebSocket `/ws`, connection loop |
| `room.rs` | Room create/join/start/leave, nicknames, capacity |
| `state.rs` | Connection map + room store |

Gameplay simulation does **not** run on the server.

## Local tooling

Prefer root `Makefile` + Docker for server (always) and for client debug/build on Linux/WSL2. See [AGENTS.md](../AGENTS.md) and [ENVIRONMENT.md](./ENVIRONMENT.md).
