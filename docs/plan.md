# Cowork — Roadmap & Plan

> Strategic plan of record: what Cowork is, the version roadmap, the locked decisions, and the
> cross-version forward dependencies. Re-cut 2026-07-14, after v0.2 shipped.
>
> **Authorities, when this file and another disagree:** the roadmap table in
> [`../README.md`](../README.md); isolation in
> [`architecture/isolation-and-platforms.md`](./architecture/isolation-and-platforms.md); working
> rules in [`../AGENTS.md`](../AGENTS.md). Open work is in **GitHub Issues** — milestones are
> versions, and nothing durable lives in an agent's context.
>
> **[`v0.1.md`](./v0.1.md) is a historical spec, not an authority.** It records what v0.1 was
> *planned* to be, and two of its instructions were reversed during implementation: credential
> routing to `~/.cowork/creds/` (decision 13) and the npm/Node install path for codex (decision 9).
> Corrections are marked in place there. Read the source before following it.
>
> **And when this file disagrees with the source, the source wins.** The 2026-07-14 re-cut of this
> document was itself written partly from memory and asserted an npm install path the product has
> never had. That is the failure mode this file invites: it is plausible, it is confident, and
> nothing recompiles when it is wrong.

## Context

Cowork is a Windows GUI tool that lets **non-developers** safely use AI coding agents.

The original plan was agreed **before any implementation**, decision-by-decision, through a
3-reviewer pass (Codex + Gemini + an independent Claude session) plus two ChatGPT reviews. Two
versions have since shipped, and reality has overruled some of it. Where a locked decision was
reversed, this file says so and names the commit — the archaeology is worth more than a clean
document.

## Roadmap (one goal per version)

| Ver | Single goal | State |
|---|---|---|
| **v0.1** | **Setup** — nothing → WSL2 → Ubuntu → toolchain → agent installed, signed in, terminal working | **Shipped** |
| **v0.2** | **Workspace** — workspaces, agent sessions that survive a restart, host↔guest file exchange | **Shipped** — closed by a full clean-room gate on real hardware (`v0.2-full-gate.md`) |
| **v0.3** | **Design** — the wizard, the shell and the terminal become one product | **Next** |
| v0.4 | Isolation — an environment the agent cannot reach out of | Deferred from v0.3 on priority, 2026-07-14 |
| v0.5 | Recovery — invisible versioning + one-click undo |  |
| v0.6 | Community — GitHub Issues/PRs/Discussions as the backend; Tauri hides the GitHub-ness |  |
| v0.7 | Observability + budget — per-workspace disk/token usage + enforcement; `coworkd` lands here |  |

**Two roadmap changes since the original plan**, both deliberate:

- **v0.2 Workspace was inserted** (it was going to be Isolation), shifting everything by one.
  Sessions, continuity and a terminal that *is* the agent had to exist before anything could be
  isolated — there was nothing to put in a sandbox.
- **v0.3 Design replaced Isolation** (2026-07-14), shifting again. On **priority**, not because the
  risk went away: the environment already works without isolation, and design does not. Cowork is a
  vitamin, not a painkiller — it is judged on how finished it feels, and the v0.2 gate exposed
  exactly the kind of rough edge that makes a non-developer quit. See `AGENTS.md` § Scope discipline
  and the ADR's D5, which records one risk (Windows interop, hence prompt injection) accepted **on
  purpose**, to be revisited before any release beyond the author.

Foundations (isolation, recovery, observability) remain deliberately **sequenced**, not bolted on
afterwards. Design moving ahead of them changes the order, not that principle.

## Locked decisions

Numbering is preserved for citability. **Struck items were reversed — read what replaced them.**

1. **v0.1 scope = setup only.** Held. WSL provisioning + Linuxbrew + mise + agent install/login;
   no isolation/recovery/observability in v0.1.
2. **A Cowork-dedicated Ubuntu distro named `Cowork`. Never touch the user's existing `Ubuntu`
   distro.** Held, and the mechanism is now settled: **`wsl --import Cowork <dir> <rootfs>`** from
   a rootfs we **re-host** (`.github/workflows/rootfs.yml` → GitHub Releases; pinned URL,
   SHA256-verified), with **`wsl --install Ubuntu-24.04 --name Cowork --no-launch`** as the fallback
   when the mirror is unreachable (`--no-launch` keeps first boot ours).
   **We re-host, we do not build.** `rootfs.yml` downloads Canonical's official
   `ubuntu-noble-wsl-amd64-wsl.rootfs.tar.gz`, verifies the pinned digest, and republishes the
   **identical bytes** — the point is to avoid a Microsoft Store dependency, not to customise the
   image. The toolchain is bootstrapped at runtime.
   **Consequence for [#6](https://github.com/conr2d/cowork/issues/6)** (bake brew/mise/apt-prereqs
   into the image): it presumes a build pipeline **that does not exist yet**. Sizing it as "add a
   step to rootfs.yml" is wrong — it is "turn the mirror into a build first". Note also that #6 is
   a **setup-time** decision (it deletes the slowest, flakiest stretch of first boot), not an
   isolation one, notwithstanding its own framing.
3. **Toolchain = Linuxbrew + mise, installed at runtime; agents installed at runtime.** Held. brew
   because much modern dev/AI tooling is Mac-first with brew-based instructions, and because brew
   installs without sudo — which is what a non-admin guest user needs, and what the isolation ladder
   (ADR D5-L2) will require. Agents change too fast to bake, in any design.
   *Note:* the sandbox binary itself (`bwrap`) is the exception and must come from **apt**, not brew
   — brew's prefix is writable by the very user bwrap confines. See the ADR's D5-L3.
4. **Delivery = a Tauri desktop app** (`Cowork.exe`), with an embedded xterm.js terminal. Held, and
   re-affirmed: a browser + WSL-server model was rejected (non-developer onboarding, native Explorer
   and tray integration, attack surface, lifecycle). The UI talks to a `HostClient` abstraction, so
   a future remote transport stays possible without a rewrite.
5. **WSL enablement = the wizard goes all the way** — preflight, `wsl --install --no-distribution` /
   `wsl --update`, UAC elevation, reboot-resume. Held; all of it shipped in v0.1.
6. **Agents = user-selectable install of claude / codex / antigravity (1+).** Held. gemini CLI was
   dropped (Google sunset it 2026-06-18 for individual AI Pro/Ultra/free users — our exact
   audience); **antigravity (`agy`)** is the successor.
7. ~~**Roadmap order = Isolation → Recovery → Community → Observability.**~~ **Superseded** — see the
   roadmap table above (Workspace inserted; Design ahead of Isolation).
8. **i18n = ja + ko + en, en as the base.** Held. All three shipped in v0.1. en is the authoring
   source of truth *and* the positioning language — the product is positioned for the global market.
   Runtime = Windows-locale auto-detect + a language selector on the first screen. Paraglide-JS;
   `errors.json` is the single source for error codes and is codegen'd into Rust.
9. **Agent install methods.** ~~mise-managed Node + `@openai/codex` for codex; `~/.default-npm-packages`
   as the npm source of truth.~~ **Reversed before it was ever built (re-spiked 2026-06-04, `f6159b2`).**
   **All three agents install from their own native installer. Nothing in the guest needs Node or npm.**
   - **claude** → `curl -fsSL https://claude.ai/install.sh | bash` → `~/.local/bin/claude`;
     self-updating.
   - **antigravity** → `curl -fsSL https://antigravity.google/cli/install.sh | bash` → `agy` in
     `~/.local/bin`; a Go binary; self-updates via `agy update`.
   - **codex** → `curl -fsSL https://chatgpt.com/codex/install.sh | sh` — a **standalone Rust
     binary**, not npm. Run with `CODEX_NON_INTERACTIVE=1`: its installer otherwise prompts
     `Start Codex now? [y/N]` by reading `/dev/tty` **directly**, so closing stdin does not stop
     it and a headless install hangs. (The npm package `@openai/codex` is a `#!/usr/bin/env node`
     shim — it would have made Node a permanent dependency. Never the unscoped `codex` package
     either; that is an unrelated 2012 squatter.)
   - **No Node in the guest, and no `~/.default-npm-packages`.** The bootstrap steps are exactly:
     apt prereqs → brew → mise → shellrc → locales → workspace. Node was dropped when codex went
     standalone, because it had no other customer.
   - **mise is still installed, deliberately, and manages nothing of ours.** It is there *for the
     user*: a workspace can `mise use node@…` / `python@…` without sudo. Same motive as brew — a
     non-admin guest user must be able to install what a task needs without ever being asked for
     root. It is not a leftover; do not remove it because it looks unused.
   - *Changed (0f03e5f):* the installer now **resolves an already-installed agent** (`bash -lc
     'command -v'`) and skips it, rather than reinstalling. Re-running setup on a provisioned
     machine is a normal path, not an error path.
   - **Supply-chain reality:** all three installers self-update, so agents are **not version-pinned
     by design**. Exact-pin discipline applies to the rootfs only (Ubuntu). Mitigation: pin the
     installer URLs, verify the binary exists and runs afterwards (`agent.integrity_check_failed`),
     run installers unattended (no TTY hangs).
10. **Naming.** Held. Brand **Cowork**; GUI **`Cowork.exe`**; in-WSL CLI **`cowork`**; future daemon
    **`coworkd`**; guest-CLI crate dir `cowork/`.
11. **Windows-first; portability as a design invariant.** Held, and enforced in CI: the `cowork`
    guest CLI must not depend on Windows APIs (`scripts/conformance/host_guest_separation.py`).
    Windows-specific code lives only in `src-tauri` and the `cfg(windows)` half of `cowork-host`. A
    later port is "write a new host driver", not a rewrite. macOS = a Linux VM (Lima), decided in the
    ADR (D1) — the *environment* is the product, and App Sandbox cannot reproduce it.
12. **Preflight disk gate = 16 GiB hard.** Held — but only half of it was built. The
    "32 GiB recommended" soft tier **was never implemented**: `preflight/decide.rs` has one
    threshold (`MIN_FREE_BYTES = 16 GiB`) and no advisory warning. Decide whether we want the soft
    tier or whether one gate is the right answer; do not assume it is there.
13. ~~**Agent credentials routed to `~/.cowork/creds/` from day one.**~~ **Reversed (eab7522).**
    Credentials live at **each agent's own default path** inside the distro (`~/.claude`, `~/.codex`,
    `~/.gemini/antigravity-cli/`). Central routing fought the agents' OAuth refresh, which rewrites
    those paths on its own terms; the distro *is* the boundary, so a vault bought nothing a
    dedicated distro did not already give. A credential vault stays out of scope.
    **Consequence for isolation:** per-agent config dirs are what an L3 sandbox binds — the agent's
    own dir read-write, the rest of `$HOME` invisible. That is *why* B2 (agent ↔ agent credentials)
    is a real boundary in the ADR and not a hypothetical one.
14. **Agents drive their own sign-in (816317a, protocol v4).** Added after v0.2. Cowork does **not**
    run login commands on the user's behalf and does not pre-flight auth. Every agent, launched bare,
    prompts for its own sign-in; the host's only job is to notice a printed OAuth URL and open the
    real browser. Eager and lazy auth checks were removed outright.
15. **The terminal hosts the agent, not a shell (v0.2).** A tab runs `bash -lic 'exec <agent>'` —
    nothing is typed, so there is no echo, no MOTD, no prompt, and no shell to leave behind when the
    agent exits. A non-developer never sees a command line. Shell access returns later, deliberately,
    behind an advanced/developer mode ([#39](https://github.com/conr2d/cowork/issues/39)).

## Forward dependencies (note now, build later)

- **A single `Cowork` distro** (one `wsl --import` root) is a v0.1 commitment. v0.4 isolation and
  v0.5 recovery both want per-workspace boundaries. v0.2 settled the layout — user `cowork`,
  workspaces at `~/workspaces/<name>` — so per-workspace isolation is a **bind-mount** problem
  (bubblewrap, ADR D5-L3), not a multi-distro one. Do not deepen the single-root assumption further,
  but do not fight it either: it was the right call.
- **Per-agent credential paths** (decision 13) → an L3 sandbox binds the *running agent's* config dir
  and hides the others. Any future change to where creds live must keep that separation possible.
- **Self-updating agents** → v0.5/v0.7 reproducibility claims must exclude agent versions. Say so
  explicitly wherever we claim reproducibility; a claim we cannot keep is worse than none.
- **Session model** — v0.2 shipped it: sessions are keyed, each tab owns its PTY, and the host keeps
  a registry. The old warning ("do not bake in a single-session assumption") is discharged.
- **Deferred: baking the rootfs** ([#6](https://github.com/conr2d/cowork/issues/6)) → decide on
  measured setup time, not on principle. Measure how long the apt prereqs + brew + mise actually take
  on a clean provision before giving it a milestone.
- **`coworkd`** sits beside `cowork`; the CLI/daemon split mirrors docker/dockerd. Lands with v0.7.

## Engineering method

Development follows the project rules in [`../AGENTS.md`](../AGENTS.md): plan-first, verifiable Done-when,
automated conformance as the commit-seal gate, a **separate-context review pass** (never
self-approve), atomic commit discipline — and, since the v0.2 gate, **`main` is never pushed to
directly**: branch → author → cold review → PR → squash-merge, so flailing does not ship.
