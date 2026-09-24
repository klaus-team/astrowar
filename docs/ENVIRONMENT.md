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
| `ASTROWAR_DEFAULT_SERVER_URL` | `ws://127.0.0.1:8080/ws` | Runtime override, else compile-time `option_env!`, else this default |

Players may override the server URL in the client UI. Official release builds can
bake a default via the `ASTROWAR_DEFAULT_SERVER_URL` compile-time env (set from
CI secrets); a non-empty runtime env still wins.
