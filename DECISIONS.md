# Project Decisions

This file lists **locked decisions** that must guide implementation: game rules,
protocol/network behavior, and reference infrastructure conventions.

Do **not** put roadmap items, marketing plans, or optional future work here.
Update this file only when a decision that affects how we build changes.

## What belongs here

- Rules of the game and multiplayer session model
- Networking and privacy constraints for the reference stack
- Tech choices that PRs are expected to follow (engine, server shape, envs)
- License, language, and contribution constraints for this repository

## What does not belong here

- Roadmaps, milestones, or “later” feature lists
- Website, storefront, or release-marketing plans
- Local scratch notes (use gitignored `temp/` if needed)

---

## Product

- Official name: **AstroWar**
- Unofficial project inspired by Atari Astroblast; not affiliated with Atari
- License: **MIT**
- Repository language: **English** (code, docs, commits, branches, PRs)
- Prefer clear structure and naming over explanatory comments
- Production hostnames and production URLs must **not** appear in tracked files;
  inject them via environment / CI secrets at build or deploy time

## Client and server stack

- Game client: **Bevy**, native on Windows, macOS, and Linux
- Reference server: single binary (**lobby + relay**), **always run via Docker**
  (debug and release)
- Client **prefer Docker** for debug and compile so collaborators need no host
  Rust toolchain; `start-client` runs the compiled **native** binary on the host
- Client Docker GUI/build targets are supported on **Linux/WSL2**; other hosts use
  CI artifacts or equivalent until platform pipelines exist
- Prefer ephemeral Docker containers; Compose is optional for the release server
- Local entrypoint: **Makefile** targets (`make help`)

## Multiplayer rules

- Rooms are joined **only by shared room code** (no public room list)
- Maximum **4** players per room
- Nickname is required
- Match duration is chosen by the room owner from **5, 10, or 15 minutes**
- Supported mode to implement: **competitive** (cooperative is out of scope until
  a new decision is recorded)
- Room owner may **start at any time**, including alone
- Players may **join after the match has started** and play immediately
- Leaving or disconnecting during a match is a **loss** for that player

## Networking

- Peer IP addresses must not be exposed between players
- All game traffic goes through the **central relay** (no peer-to-peer game path)
- Rooms are stored **in memory** on the reference server (restart clears rooms)
- Default server URL for official client builds comes from **env at build time**
- Clients must allow overriding the server URL locally
- Forks may run their own servers and clients

## Contributions

- PRs should fit the reference server and official client described above
- Forks may diverge freely under MIT
- The project Code of Conduct applies to public project spaces
