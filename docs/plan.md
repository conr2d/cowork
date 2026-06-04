# Cowork — Roadmap & Plan

> Strategic plan: what Cowork is, the version roadmap, the locked decisions, and the cross-version forward dependencies. The full v0.1 implementation spec lives in [`v0.1.md`](./v0.1.md).

## Context

Cowork is a Windows GUI tool that lets **non-developers** safely use AI coding agents.

**v0.1 goal (deliberately narrow):** on Windows, take a non-developer from "nothing" to "an AI coding agent installed inside WSL2, logged in, with a working terminal." Foundation pillars (isolation / recovery / observability / credentials-vault) are deferred to later versions — setup precedes the foundations that harden it.

This plan was agreed **before any implementation**, decision-by-decision, and incorporates a 3-reviewer pass (Codex + Gemini + an independent Claude session) plus two ChatGPT reviews. Their findings are folded into the locked decisions below.

## Locked decisions

1. **v0.1 scope = setup only (minimal).** WSL provisioning + Linuxbrew + mise + agent install/login. No isolation/recovery/observability in v0.1.
2. **WSL provisioning = a Cowork-dedicated, Ubuntu-based distro named `Cowork` (vanilla image, not custom-built).** **Never touch a user's existing `Ubuntu` distro** — install / re-run / uninstall must be fully isolated (critical UX for a non-developer product). Mechanism (confirm exact form at build time): `wsl --install -d Ubuntu --name Cowork` if the installed WSL supports `--name`, else `wsl --import Cowork <dir> <vanilla Ubuntu rootfs>` using Canonical's official rootfs (SHA256-verified). Either way the image is **vanilla** — the custom *baked* rootfs (brew/mise baked, "Stage B") stays **deferred** (build only if field evidence shows proxies break brew/npm; agents install at runtime in any design, so baking only removes the brew/node network surface — a partial win not worth the distro-build CI / 2 GiB cap / doubled matrix for setup-only v0.1). Determinism via exact version pins in the bootstrap + a pinned rootfs.
3. **Toolchain = Linuxbrew + mise, installed at runtime by the bootstrap; agents installed at runtime.** brew is kept (rationale: much modern dev/AI tooling is Mac-first with brew-based instructions/scripts; brew also installs without sudo, which suits the non-admin guest user). Agents change frequently → always runtime, never baked.
4. **Delivery = minimal Tauri wizard GUI** (single setup wizard + embedded xterm.js terminal). Builds `Cowork.exe`.
5. **WSL enablement = wizard goes all the way** — preflight + `wsl --install --no-distribution` / `wsl --update` with UAC elevation and reboot-resume.
6. **Agents = user-selectable install of claude / codex / antigravity** (1+); auth via native login in the embedded terminal. (gemini CLI dropped — Google sunsets it 2026-06-18 for individual AI Pro/Ultra/free users, our exact audience; **Antigravity CLI** is the successor.)
7. **Roadmap order = Isolation → Recovery → Community → Observability/Budget.**
8. **i18n = ja + ko + en in v0.1; en is the base/canonical language.** All three shipped. en is the authoring source of truth AND the positioning base — the product is positioned as **global-market** (strategic for a Japan Startup Visa application). Runtime display = auto-detect Windows locale (ja/ko/en, else en) + a **language selector on the first install screen**. ja/ko are full peers, not defaults. Paraglide-JS (`sourceLanguageTag: "en"`); `errors.json` single-source codegen. (Template/manifest locale maps arrive with the template system → v0.4, not v0.1.)
9. **Agent install methods (Linux/WSL, confirmed + spiked 2026-06).** Two native shell installers, one npm:
   - **claude** → `curl -fsSL https://claude.ai/install.sh | bash` → `~/.local/bin/claude`, self-updating, no Node. (npm `@anthropic-ai/claude-code` deprecated since v2.1.15; brew cask is macOS-only.)
   - **antigravity** → `curl -fsSL https://antigravity.google/cli/install.sh | bash` → binary **`agy`** at `~/.local/bin/agy` (Go, not npm; replaces gemini CLI). Installer "staging-verifies" (integrity check present), supports `--dir`, musl-aware. Auth = **first-run interactive browser OAuth** (no `login` subcommand); token at `~/.gemini/antigravity-cli/antigravity-oauth-token`. Self-updates via `agy update`. CLI surface is Claude-Code-like (`-p`, `-i`, `--continue`, `--sandbox`, `--dangerously-skip-permissions`).
   - **codex** → mise-managed Node + `@openai/codex` (the only Node-dependent agent). ⚠️ never the unscoped `codex` package.
   - **Node runtime** = mise, **Node 24 LTS, exact patch pinned** (`24.x.y`); needed solely for codex (≥22). Installed only if codex is selected. mise does not auto-update (declarative; `mise up` is explicit) → bumps are deliberate, app-controlled.
   - **npm globals source of truth** = `~/.default-npm-packages` (owned by the `cowork` CLI) → currently just `@openai/codex`. File is authoritative; any `npm i -g` is paired with a file edit.
   - **Install-domain isolation:** npm prefix stays default (per-node-version dir); `~/.local/bin` holds native-installer binaries (`claude`, `agy` — distinct names, no collision); no agent is npm-installed into a name clashing with a native one.
   - **Supply-chain reality (reviewer-flagged):** the native installers self-update, so agents are **NOT version-pinned by design**. The "reproducibility/exact-pin" discipline applies to the rootfs/toolchain (Ubuntu, mise, Node), **not** to agents. Mitigation: pin the installer URLs, verify the resulting binary exists + runs post-install (`agent.integrity_check_failed`), and run installers with unattended flags (avoid TTY hangs).
10. **Naming (locked).** Brand = **Cowork**. GUI app (Windows) = **`Cowork.exe`** (PascalCase Windows GUI convention). In-WSL CLI = **`cowork`** (lowercase Unix convention). Future daemon (v0.5) = **`coworkd`** (docker/dockerd model). Guest-CLI crate dir = **`cowork/`**.
11. **Windows-first; portability as a design invariant.** v0.1+ builds Windows only; no Mac/Linux code now. Enforce a strict **host driver ↔ guest CLI split**: the `cowork`/`coworkd` guest CLI is host-agnostic and reusable as-is; only the thin host driver is OS-specific (Windows = `wsl.exe`; future Mac = Lima/Apple Virtualization; future native Linux = direct). A later port = "write a new host driver", not a rewrite.
12. **Preflight disk gate = 16 GiB hard / 32 GiB recommended** (reviewer-raised from 8/16: Ubuntu growth, apt caches, brew bottles, mise Node, npm cache, 3 agents, VHDX slack, retry leftovers).
13. **Agent credentials routed to `~/.cowork/creds/` from day one** (via per-agent symlink/env mapping), NOT left in each agent's default home location. Agents store creds in disparate places (claude `~/.claude`, codex `~/.codex`, antigravity `~/.gemini/antigravity-cli/`); centralizing now prevents breaking changes when v0.2 bubblewrap isolates the home dir and v0.3 snapshots the workspace. Creds must live **outside** any future per-workspace writable root.

## Roadmap (one goal per version)

| Ver | Single goal | Notes |
|---|---|---|
| **v0.1** | **Setup** | WSL → Cowork distro (vanilla Ubuntu) → brew/mise → agent install → terminal login. ja+ko+en (en base; global positioning). |
| v0.2 | **Isolation** | Per-workspace bubblewrap sandbox; makes `--dangerously-skip-permissions` safe; task-scoped workspaces. Must account for `~/.cowork/creds` and the single-distro layout. |
| v0.3 | **Recovery** | rootfs/workspace snapshots, 1-click restore, survive corruption. |
| v0.4 | **Community** | GitHub Issues/PRs/Discussions as backend; Tauri hides the GitHub-ness; template sharing. |
| v0.5 | **Observability + budget** | Per-workspace disk/token usage display + budget enforcement. `coworkd` daemon lands here. |
| (later) | **Provisioning hardening** | Custom baked rootfs (deferred Stage B), if proxy/determinism field evidence justifies it. |

## Forward dependencies (note now, build later)

- **Creds** routed to `~/.cowork/creds` (decision #13) → v0.2 isolation / v0.3 recovery must keep this path outside per-workspace writable roots.
- **Single `Cowork` distro** (one `wsl --import`/`-d` root) is a v0.1 commitment that collides with v0.2 per-workspace isolation and v0.3 per-workspace snapshots (which want multiple roots / btrfs subvols). Design the workspace layout in v0.2 with this in mind; don't deepen the single-root assumption.
- **Self-updating agents** (claude/agy self-update; codex via npm) → v0.3/v0.5 reproducibility claims must exclude agent versions.
- **Deferred Stage B (custom rootfs)** → revisit only on proxy/determinism field evidence; agents stay runtime-installed regardless.
- `coworkd` daemon (v0.5) sits beside `cowork`; CLI/daemon split mirrors docker/dockerd.

## Engineering method

Development follows [`ENGINEERING_PRINCIPLES.md`](../../ENGINEERING_PRINCIPLES.md) (the portfolio default) and the project-specific rules in [`../AGENTS.md`](../AGENTS.md) — plan-first, verifiable Done-when, automated conformance gates as the commit-seal gate, a separate-context review pass, and atomic commit discipline.
