# Contributing

Thanks for contributing to AstroWar.

## Language

English is required for code, documentation, commit messages, branch names, and pull requests.

## Before you start

1. Read [DECISIONS.md](DECISIONS.md).
2. Follow the [Code of Conduct](CODE_OF_CONDUCT.md).
3. Prefer small, focused pull requests.
4. Skim [docs/CODE_GUIDE.md](docs/CODE_GUIDE.md) and use
   [docs/CODE_REVIEW_GUIDE.md](docs/CODE_REVIEW_GUIDE.md) before opening a PR.
5. Full doc map: [docs/INDEX.md](docs/INDEX.md).

## Development

- Use the Makefile (`make help`). Server always runs in Docker.
- Prefer Docker for client debug/build on Linux or WSL2 (`make dev-client`, `make build-client`).
- `make start-client` runs the native binary produced under `dist/`.
- Do not add speculative design notes to the repository; update `DECISIONS.md` only when something is decided

## Code style

- Follow [docs/CODE_GUIDE.md](docs/CODE_GUIDE.md)
- Prefer clear structure and naming over comments
- Avoid comments that restate the code
- Keep protocol changes versioned in `protocol::PROTOCOL_VERSION` when messages become incompatible

## Pull requests

- Describe the change and how to test it
- Include env or Docker notes when relevant
- Self-check against [docs/CODE_REVIEW_GUIDE.md](docs/CODE_REVIEW_GUIDE.md)
- Contributions should make sense for the reference server and official client; forks may diverge freely under MIT
- **PR title drives the semver bump** after merge into `main` (not on the PR
  branch). Concurrent PRs can stay open without colliding on version commits.

  | Title prefix | Bump |
  | ------------ | ---- |
  | `feat:` / `feat(` | minor |
  | `fix:` / `refactor:` / `perf:` / `style:` (and `fix(` …) | patch |
  | `breaking:` or `type!:` / `type(scope)!:` | major |
  | `docs:`, `chore:`, `ci:`, `test:`, … | no bump |

  Matching titles produce a bot commit on `main` (`chore: bump version to X.Y.Z`)
  and git tag `vX.Y.Z`. Releases and server deploy should key off that tag only.

  Pushing a `v*` tag runs `.github/workflows/release.yml`, which attaches portable
  client zips to the GitHub Release:

  - `astrowar-linux-x86_64.zip`
  - `astrowar-windows-x86_64.zip`
  - `astrowar-macos-universal.zip` (`.app` + `LICENSE`)

  Official builds embed the default relay URL from the repository secret
  `ASTROWAR_DEFAULT_SERVER_URL` at compile time (runtime env still overrides).
  If the secret is unset, the client falls back to `ws://127.0.0.1:8080/ws`.

  The same `v*` tag also runs `.github/workflows/deploy-server.yml`: publish
  `ghcr.io/<owner>/astrowar-server:<tag>` (and `:latest`), then SSH to the VPS to
  update `ASTROWAR_SERVER_IMAGE`, `docker compose pull`, and restart when these
  Actions secrets are set:

  | Secret | Purpose |
  | ------ | ------- |
  | `VPS_HOST` | SSH host |
  | `VPS_USER` | SSH user |
  | `VPS_SSH_KEY` | Private key |
  | `VPS_DEPLOY_PATH` | Directory on the VPS that holds `compose.yaml` + `.env` |
  | `VPS_SSH_FINGERPRINT` | Optional host key fingerprint |

  One-time on the VPS (no git clone): create `VPS_DEPLOY_PATH`, copy
  `docker/compose.deploy.yaml` as `compose.yaml` manually (re-copy only when that
  file changes), and a local `.env` with at least `ASTROWAR_HOST_PORT` (and
  `ASTROWAR_PUBLISH_ADDR=127.0.0.1`). CI only updates `ASTROWAR_SERVER_IMAGE` and
  restarts the container. Prefer a **public** GHCR package so the VPS can
  `docker pull` without a registry token.

  If `main` is branch-protected, allow `github-actions[bot]` to push (or use a
  fine-grained PAT stored as a secret) so the bump commit can land.

## Reporting issues

Use GitHub issues for bugs and concrete proposals aligned with locked decisions.
