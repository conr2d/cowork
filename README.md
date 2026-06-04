# Cowork

> A Windows tool that makes AI coding agents safe and approachable for non-developers.

**Status:** v0.1 in development (Setup). Plan & spec live in `docs/plan.md` and `docs/v0.1.md` (materialized at implementation start).

## What it does

Cowork is a Windows GUI (Tauri v2) that walks a non-developer through a one-time setup wizard: enable WSL2, provision Ubuntu, install a toolchain (Linuxbrew + mise), and install + log in to the AI coding agent of their choice — ending in a working terminal. **That end-to-end setup is the entire v0.1 goal.**

It uses an AI subscription the user already has — no extra cost.

## Roadmap (one goal per version)

| Ver | Goal |
|---|---|
| **v0.1** | **Setup** — WSL2 → Ubuntu → brew/mise → agent install → terminal login |
| v0.2 | Isolation — per-workspace bubblewrap sandbox |
| v0.3 | Recovery — snapshots + 1-click restore |
| v0.4 | Community — GitHub-backed template sharing |
| v0.5 | Observability + budget |

Foundations (isolation, recovery, observability) are deliberately **sequenced after** setup, not bolted on.

## Principles

- **Vendor-neutral** — Claude Code · OpenAI Codex CLI · Antigravity CLI; use whichever subscription you have.
- **Global-first, multilingual** — ja + ko + en from v0.1; English is the base language.
- **Foundation-first** — one clear goal per version; trust (isolation/recovery/observability) is built before features pile on.

## Project layout

```
cowork/
├── src/          SvelteKit-static frontend — wizard + embedded terminal (i18n: ja/ko/en)
├── src-tauri/    Tauri v2 Rust — Windows host driver (builds Cowork.exe)
├── cowork/       host-agnostic guest CLI (runs inside WSL)
├── templates/    default workspace template
└── docs/         plan + v0.1 spec
```

## Build (developers)

Full end-to-end behavior requires Windows + WSL2. Linux/macOS can do partial UI work and guest-CLI unit tests.

```bash
pnpm install
cargo check --workspace
cargo test -p cowork
pnpm tauri dev   # full behavior on Windows only
```

## License

Apache-2.0. See [LICENSE](./LICENSE).

## Contributing

Closed to external contributions until the v0.4 community phase. Design discussion is welcome in GitHub Discussions.
