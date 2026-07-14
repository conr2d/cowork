# Cowork

> A safe, disposable computer for AI coding agents — built for people who are not developers.

**Status:** pre-release. Setup (v0.1) and Workspace (v0.2) are built and in final hardware
validation; nothing has shipped publicly yet. See
[`docs/product-brief.md`](./docs/product-brief.md) for what Cowork is and why.

## What it does

People who pay for an AI coding subscription — Claude Code, OpenAI Codex CLI, Google
Antigravity CLI — but who do not write code hit a wall: the agents ship as terminal tools.
The ones who climb that wall hand a capable agent full access to their real machine.

Cowork gives the agent a Linux environment of its own — reproducible, isolated from the
user's real files, and undoable — then hides that environment completely behind a desktop
app. Work happens in a **workspace**, created as casually as a new chat. Files move in and
out through a folder the user opens in Explorer. The agent's own interface is left alone:
Cowork hosts each vendor's real terminal UI rather than rebuilding it.

It runs on an AI subscription the user already has — no extra cost, and no model of our own.

## Roadmap (one goal per version)

| Ver | Goal |
|---|---|
| v0.1 | **Setup** — WSL2 → Ubuntu → toolchain → agent install → terminal |
| v0.2 | **Workspace** — workspaces, sessions that survive a restart, file exchange |
| v0.3 | **Isolation** — an environment the agent cannot reach out of |
| v0.4 | Recovery — invisible versioning + one-click undo |
| v0.5 | Community — template sharing |
| v0.6 | Observability + budget |

Foundations (isolation, recovery, observability) are deliberately **sequenced**, not bolted
on afterwards.

## Principles

- **Vendor-neutral** — whichever agent subscription you already have. Never our own model,
  never our own API key.
- **The user never learns Git, WSL, Linux, or a package manager.** A leaked concept is a bug.
- **Global-first, multilingual** — English is the base language; Japanese and Korean ship
  alongside it, not after it.
- **Never destroy the user's work** — undo restores; it never deletes.

## Project layout

```
cowork/
├── src/            SvelteKit-static frontend — wizard, shell, embedded terminal (en/ja/ko)
├── src-tauri/      Tauri v2 — the Windows host driver (builds Cowork.exe)
├── cowork/         host-agnostic guest CLI (runs inside the Linux environment)
├── cowork-host/    host logic, unit-testable off-Windows
├── cowork-errors/  shared error model + host↔guest wire protocol
├── templates/      workspace presets
└── docs/           product brief, plan, architecture decisions, gate runbooks
```

## Build (developers)

Full end-to-end behavior requires Windows + WSL2. Linux and macOS can run the frontend and
the guest/host unit tests.

```bash
pnpm install
cargo test -p cowork -p cowork-errors -p cowork-host
pnpm test
pnpm tauri dev   # full behavior on Windows only
```

## License

Apache-2.0. See [LICENSE](./LICENSE). Cowork redistributes an unmodified Ubuntu WSL root
filesystem during setup; see [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md) for the
corresponding-source offer and trademark notice.

## Contributing

Closed to external contributions until the community phase. Design discussion is welcome in
GitHub Discussions.
