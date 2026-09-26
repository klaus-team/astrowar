# Environment variables

## Server (`astrowar-server`)

| Variable | Default | Description |
|----------|---------|-------------|
| `ASTROWAR_SERVER_HOST` | `0.0.0.0` | Bind address |
| `ASTROWAR_SERVER_PORT` | `8080` | Bind port inside the container |
| `ASTROWAR_ROOM_CODE_LENGTH` | `6` | Characters in room codes |
| `ASTROWAR_ROOM_TTL_SECS` | `3600` | In-memory room lifetime |
| `RUST_LOG` | `astrowar_server=info,tower_http=info` | Tracing filter |
| `ASTROWAR_PUBLISH_ADDR` | `127.0.0.1` | Host address published by Compose (use loopback behind a reverse proxy) |
| `ASTROWAR_HOST_PORT` | `8080` | Host port mapped to container `8080` |
| `ASTROWAR_SERVER_IMAGE` | `astrowar-server:local` (dev) | Image reference for Compose; deploy sets `ghcr.io/<owner>/astrowar-server:<tag>` |

## Client (`astrowar`)

| Variable | Default | Description |
|----------|---------|-------------|
| `ASTROWAR_DEFAULT_SERVER_URL` | `ws://127.0.0.1:8080/ws` | Runtime override, else compile-time `option_env!`, else this default |

Not part of `.env.example` / VPS Compose. Official release builds bake the default from
the GitHub Actions secret `ASTROWAR_DEFAULT_SERVER_URL`; locally you may export the
same name when running the client (or override the URL in the UI). A non-empty
runtime env still wins over the compile-time value.
