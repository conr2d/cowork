# Cowork — Agent Context

> Cross-tool context for any AI coding agent working on the Cowork repo. See `README.md` for the overview and `docs/plan.md` for the full plan.

## What you are working on

Cowork is a Windows GUI (Tauri v2) that bootstraps WSL2 + Ubuntu + an AI coding agent for non-developers.

**v0.1 single goal:** take a non-developer from nothing to "an AI coding agent installed inside WSL2, logged in, with a working terminal." **Setup only — nothing more.**

## Architecture (v0.1)

```
Windows host (Tauri v2 → Cowork.exe) — host driver:
├── Setup wizard: preflight → WSL enable (+ RunOnce reboot-resume) → distro provision → agent install/login
├── ConPTY ↔ wsl.exe PTY bridge → xterm.js embedded terminal
└── guest-CLI injection; parses JSON-lines progress from the guest

WSL guest (vanilla Ubuntu) — host-agnostic:
├── `cowork` CLI (Rust): apt prereqs → Linuxbrew → mise → Node (codex only) → agent install
│      emits JSON-lines progress; owns ~/.default-npm-packages and ~/.cowork/creds routing
└── agents: claude (native installer) · antigravity `agy` (native installer) · codex (npm via mise)
```

Naming: brand **Cowork**; GUI **Cowork.exe**; guest CLI **`cowork`**; future daemon **`coworkd`** (v0.5).

## Scope discipline (one goal per version)

v0.1 Setup · v0.2 Isolation (bubblewrap) · v0.3 Recovery (snapshots) · v0.4 Community · v0.5 Observability + budget.

**Do NOT** implement isolation, recovery, observability, budgets, community, or a credential vault in v0.1. Credentials are merely *routed* to `~/.cowork/creds` (not encrypted) to avoid breaking changes when v0.2/v0.3 land.

## i18n & errors

- ja + ko + en shipped in v0.1; **en is the base/canonical source and the positioning language** (global-market). Runtime = Windows-locale auto-detect + a first-screen selector.
- The `cowork` CLI and Rust backend return stable error **codes only**, never localized strings.
- `errors.json` is the single source of truth: `build.rs` codegens the Rust enum, the TS frontend imports the same file, and 3-locale parity is enforced (build fails on drift).

## Coding conventions

- Rust 2024, `cargo fmt`, `cargo clippy` clean. No `unwrap()` outside tests.
- TypeScript strict, no `any`.
- Svelte 5 runes (`$state`/`$derived`/`$effect`); no legacy `$:`.
- Every user-facing error = a code in `errors.json` with `title`/`body` in all three locales.
- UI tone: non-developer office worker — plain, concise. English is the base.

## Quality & conformance

Functional "done" is not enough — these guard against implementation drift from the plan.

**Automated conformance (CI / lint / test — must pass before a unit is sealed):**
- **Host/guest separation:** the `cowork` crate must not depend on Windows APIs (`windows`/`windows-sys`/`winapi`) — checked from cargo metadata; it builds for a Linux target. Windows-specific code lives only in `src-tauri`.
- **No backend localization:** neither `cowork` nor `src-tauri` imports/emits localized strings — error **codes** only. Enforced by a grep/lint test (no reference to the message catalog).
- **`errors.json` single source:** build fails if an emitted code is absent from `errors.json`, or if any of the 3 locales misses a `title`/`body` key (the WP2 parity test).
- **Envelope-only:** all guest→host user-facing signals go through the `{code,kind,stage,context,cause?}` envelope; no ad-hoc `eprintln!`/`println!` user strings (lint).

**Review pass (separate context, never self-approve):** every unit/WP gets an architecture-conformance + code review by a *different* agent (`oh-my-claudecode:code-reviewer` / `architect`), with evidence, before sealing. Risk-weighted: WP4–WP8 (WSL / reboot-resume / PTY / OAuth / agent install) heaviest; WP0–WP2 light.

**Deviation record:** if implementation departs from the plan, record it — a commit trailer (`Directive:`/`Rejected:`) for small ones, a `docs/adr/NNNN-*.md` for architectural ones (host/guest boundary, error model, distro model). Note temporary vs permanent and v0.2–v0.5 impact.

**Usability (the real risk is the user, not the tech):** every user-facing error renders a plain-language localized title + body + concrete next action — never a raw code or English stack. Final gate: a real non-developer completes setup unaided, time-boxed, observed.

## Commit discipline

- **1 commit = one meaningful feature unit** — the smallest change that compiles, passes tests + automated conformance, is independently reviewable, and does one thing (one intent line; if you need "and", split). A WP is a branch/milestone, not one commit.
- **Seal a unit only after** its functional DoD + automated conformance + a separate-context review sign-off. Never seal broken or unreviewed units.
- **Review fixes fold into the unit** — `git commit --amend` (or `git reset --soft <unit-base>` + recommit) **before push/merge**; `git rebase -i` is unavailable here. History shows the reviewed-final state, not the back-and-forth.
- **Exception — noteworthy fix = its own commit** when it: (a) corrects a wrong plan assumption, (b) is a non-obvious gotcha future work would re-hit (esp. external-tool / "fragile path" quirks), (c) changes a conformance rule or forward-dependency, or (d) is a workaround worth remembering. Put the lesson in the body / a `Lesson:` trailer.
- **Message** = OMC protocol (intent line + body + trailers `Constraint:`/`Rejected:`/`Directive:`/`Confidence:`/`Scope-risk:`/`Not-tested:`) + a `Reviewed:` trailer + `Co-Authored-By: Claude ...`.
- **Branch off `main`; commit per sealed unit; merge a WP** when all its units are sealed and the WP-level review passes. History hygiene may be delegated to `oh-my-claudecode:git-master`.

## Locked — do not relitigate

- Embedded terminal = xterm.js (no alternatives).
- Agents = claude / codex / antigravity (gemini CLI dropped — Google sunset 2026-06-18).
- v0.1 provisioning = vanilla Ubuntu + runtime bootstrap; custom baked rootfs is deferred.
- Windows-only build; preserve the host-driver ↔ guest-CLI split for future Mac/Linux portability.
