# Environment variables

## Server (`astrowar-server`)

| Variable | Default | Description |
|----------|---------|-------------|
| `ASTROWAR_SERVER_HOST` | `0.0.0.0` | Bind address |
| `ASTROWAR_SERVER_PORT` | `8080` | Bind port |
| `ASTROWAR_ROOM_CODE_LENGTH` | `6` | Characters in room codes |
| `ASTROWAR_ROOM_TTL_SECS` | `3600` | In-memory room lifetime |
| `RUST_LOG` | `astrowar_server=info,tower_http=info` | Tracing filter |

## Client (`astrowar`)

| Variable | Default | Description |
|----------|---------|-------------|
| `ASTROWAR_DEFAULT_SERVER_URL` | `ws://127.0.0.1:8080/ws` | Build/runtime default WebSocket URL |

Players may override the server URL in the client. The env value is the default
shipped with a given build.
