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
│      emits JSON-lines progress; owns ~/.default-npm-packages
└── agents: claude (native installer) · antigravity `agy` (native installer) · codex (npm via mise)
```

Naming: brand **Cowork**; GUI **Cowork.exe**; guest CLI **`cowork`**; future daemon **`coworkd`** (v0.5).

## Scope discipline (one goal per version)

v0.1 Setup · v0.2 Workspace · **v0.3 Design** · v0.4 Isolation · v0.5 Recovery · v0.6 Community · v0.7 Observability + budget.

**Do NOT** implement a later version's goal early. Credentials stay at each agent's default path inside the distro, which is the isolation boundary; a credential vault remains out of scope.

**Isolation moved from v0.3 to v0.4** (2026-07-14) — on **priority**, not because the risk went away. Cowork is single-user and single-machine, and the environment already works without isolation; design does not. A vitamin-class product is judged on how finished it feels, and the v0.2 gate exposed exactly the kind of rough edge that makes a non-developer quit. Note that "we have not seen harm" is *not* part of the argument: with no users, that is a statement about our user count, not about the risk. See `docs/architecture/isolation-and-platforms.md` D5, which records one risk (Windows interop, and therefore prompt injection) that we accept **on purpose and revisit before any release beyond the author**.

## Work intake — file it, do not fix it

**Defects and ideas go to GitHub Issues, never straight to a fix.** A conversation is not durable storage: anything held only in an agent's context is lost at the end of the session, and fixing on discovery is what kept the v0.2 gate from ever finishing (each fix invalidated the installer and restarted the run — see `docs/v0.2-full-gate.md` §0).

- Before starting work: `gh issue list`. Do not rediscover what is already filed.
- On finding a defect or having an idea **mid-task**: `gh issue create`, then carry on with the task you were on. Do not detour.
- **Milestones are versions** (`v0.3 — Design`, `v0.4 — Isolation`). Labels are orthogonal and reusable, so they do not accumulate per release:
  - `gate-blocker` — blocks closing the current version (passes the bug bar in the gate runbook §0)
  - `polish` — rough, but setup still completes; does not block
  - `design` — absorbed by the design overhaul
  - `spike` — investigation needed before a design decision
  - `idea` — not committed to a version yet
- The **only** thing that may be fixed on discovery is a `gate-blocker` — and only after it is filed.

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

## Branch discipline — `main` is never pushed to directly

**No one commits to `main`. Ever.** Not the author, not the reviewer, not for a one-line fix.
The v0.2 gate was run against `main` directly, and the result was a history where half the
commits exist only to repair the other half — three of them blockers manufactured by the fixes
themselves. That history had to be rewritten. It is the last time.

The loop, per unit of work:

1. **Branch.** `git switch -c fix/<issue>-<slug>` (or `feat/…`, `docs/…`) off `main`. A unit is
   normally one issue. Use a **git worktree** when an author agent needs to build while `main`
   stays usable.
2. **Author.** The author (usually Codex, via a deterministic spec) commits freely on the
   branch. **Flailing on a branch is fine** — that is what the branch is for. Do not amend, do
   not curate; just get it right.
3. **Review.** A *separate context* cold-reviews the diff and runs the full battery. Review
   fixes are more commits on the branch, not amendments.
4. **Seal.** `gh pr create`, let CI run, then **squash-merge**. The squash message is the
   reviewed unit's message — one intent line, the reasoning, the measurements. **The branch's
   back-and-forth collapses and never reaches `main`.**
5. Delete the branch.

So `main` only ever contains sealed, reviewed, green units — one commit each. The archaeology
lives in the PR, where it belongs, and nobody has to rewrite history to get a readable log.

**Squash-merge is the mechanism that enforces "flailing does not ship".** Do not merge-commit,
do not rebase-merge.

## Commit discipline

- **1 commit = one meaningful feature unit** — the smallest change that compiles, passes tests + automated conformance, is independently reviewable, and does one thing (one intent line; if you need "and", split). A WP is a branch/milestone, not one commit.
- **Seal a unit only after** its functional DoD + automated conformance + a separate-context review sign-off. Never seal broken or unreviewed units.
- **Review fixes are commits on the branch**, not amendments. The squash-merge collapses them, so `main` shows the reviewed-final state and the PR keeps the back-and-forth.
- **Exception — noteworthy fix = its own commit** when it: (a) corrects a wrong plan assumption, (b) is a non-obvious gotcha future work would re-hit (esp. external-tool / "fragile path" quirks), (c) changes a conformance rule or forward-dependency, or (d) is a workaround worth remembering. Put the lesson in the body / a `Lesson:` trailer.
- **Message** = OMC protocol (intent line + body + trailers `Constraint:`/`Rejected:`/`Directive:`/`Confidence:`/`Scope-risk:`/`Not-tested:`) + a `Reviewed:` trailer + `Co-Authored-By: Claude ...`.
- **Branch off `main`; commit per sealed unit; merge a WP** when all its units are sealed and the WP-level review passes. History hygiene may be delegated to `oh-my-claudecode:git-master`.

## Locked — do not relitigate

- Embedded terminal = xterm.js (no alternatives).
- Agents = claude / codex / antigravity (gemini CLI dropped — Google sunset 2026-06-18).
- v0.1 provisioning = vanilla Ubuntu + runtime bootstrap; custom baked rootfs is deferred.
- Windows-only build; preserve the host-driver ↔ guest-CLI split for future Mac/Linux portability.
