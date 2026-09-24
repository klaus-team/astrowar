# Code review — AstroWar

Checklist for authors and reviewers. Details live in the linked docs — do not duplicate rules here.

---

## Before the PR

- [ ] Minimal scope (one theme)
- [ ] Relevant `make check` / `make test` / client build still works
- [ ] [DECISIONS.md](../DECISIONS.md) respected (no invented product rules)
- [ ] Branch / commits / PR description in English — [CONTRIBUTING.md](../CONTRIBUTING.md)

---

## Review dimensions

| Area | Check | Doc |
| ---- | ----- | --- |
| Crates | Lobby vs gameplay boundary; no Bevy types in `server`/`protocol` | [ARCHITECTURE.md](./ARCHITECTURE.md) |
| Protocol | Breaking lobby change bumps `PROTOCOL_VERSION` | [CODE_GUIDE.md](./CODE_GUIDE.md) |
| Authority | Host owns sim; clients do not become authority for scores/lives | [ARCHITECTURE.md](./ARCHITECTURE.md) |
| Privacy | No peer IPs; traffic stays on the relay | [DECISIONS.md](../DECISIONS.md) |
| Tooling | Server via Docker/Make; no prod hostname in the diff | [ENVIRONMENT.md](./ENVIRONMENT.md) |
| Client UX | Menu text only; gameplay numbers come from sim/HUD systems | [CODE_GUIDE.md](./CODE_GUIDE.md) |
| Tests | Room/protocol behavior covered when those crates change | `make test` |

---

## Typical blockers

- Peer-to-peer game path or exposing peer addresses
- Gameplay simulation added to `astrowar-server` relay
- Lobby / `GameMessage` mixed incorrectly
- Incompatible protocol change without `PROTOCOL_VERSION` bump
- Production hostname or secret committed
- Scope creep (unrelated refactor bundled with a feature)

---

## Feedback tone

- **Block:** security/privacy, authority bugs, protocol breaks, DECISIONS violations
- **Suggest:** naming, smaller functions, extra tests, doc link updates
