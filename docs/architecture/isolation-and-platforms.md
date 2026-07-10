# Isolation and platforms

**Status:** accepted (design), 2026-07-10. Supersedes the implicit isolation claim of v0.1.
**Scope:** what Cowork isolates, how, and what that implies for macOS. Feeds the v0.3
Isolation work package. Nothing here is implemented yet.

---

## 1. What Cowork sells

Cowork sells an **environment**, not merely a confinement. A non-developer must get the
same working toolchain, the same locales, the same workspace layout, and the same agent
behaviour on every machine. Confinement is a property that environment must also have.

Reading the two apart matters, because macOS offers a first-class answer to *confinement*
(App Sandbox) and no answer at all to *reproducibility*. Windows conflated the two for us —
the dedicated WSL2 distro delivered both at once, so we never had to name them separately.

---

## 2. Threat model

Two boundaries, not one.

| | Boundary | Question it answers |
|---|---|---|
| **B1** | host ↔ agent environment | What of the user's machine can an agent reach? |
| **B2** | inside the environment | Can workspace A's agent read workspace B's files, or another agent's credentials? |

**Non-goal: preventing exfiltration.** Agents must reach their vendor's API, so outbound
network is open. Neither bubblewrap's network namespace nor App Sandbox's
`com.apple.security.network.client` restricts a destination usefully. Any claim that Cowork
prevents an agent from sending your data somewhere is false, and must never be made.

---

## 3. Current state (measured, not assumed)

v0.1/v0.2 have **no isolation at all**. The dedicated distro is packaging isolation, not
execution isolation.

- `cowork-host/src/provision/inject.rs:72` — `firstboot_script()` writes exactly
  `[user]\ndefault=cowork` to `/etc/wsl.conf`. `[automount]` and `[interop]` are therefore
  left at their defaults, i.e. **enabled**. `/mnt/c` is mounted read-write with the Windows
  user's privileges: an agent inside the distro can delete the user's Documents.
- The same script writes `cowork ALL=(ALL) NOPASSWD:ALL` to `/etc/sudoers.d/cowork`.
- There is no per-workspace or per-agent boundary inside the distro.

---

## 4. Decisions

### D1 — The environment is a Linux environment on every platform

Windows: the existing WSL2 distro. macOS: a Linux VM (Lima, `vz` backend), running the same
rootfs and the same guest CLI. The `cowork` guest crate is already host-agnostic (enforced by
`scripts/conformance/host_guest_separation.py`), so this is a **transport swap**
(`wsl.exe` → `limactl`), not a port. The isolation layer inside the environment is identical
on both.

### D2 — Rejected: native macOS + App Sandbox

The supervisor + sandboxed-helper + security-scoped-bookmark pattern is the model Apple
supports, and Seatbelt is not going away (Chrome depends on it). It was still rejected:

1. **B2 is unsolvable.** App Sandbox gives one container *per app*, not per workspace or
   per session. Workspace A's agent would see workspace B's files and every agent's
   credentials. Getting per-session confinement means writing Seatbelt profiles by hand —
   precisely what Apple warns against.
2. **`$HOME` is relocated** to `~/Library/Containers/<id>/Data`, and children inherit it.
   That collides head-on with two decisions we just made: credentials at each agent's
   default path, and reusing agents the user already installed. Granting access back means
   a security-scoped bookmark per path, and bookmarks are only issued for paths the user
   picks in an `NSOpenPanel` — "please select your `~/.claude` folder" is not an onboarding
   flow.
3. **No reproducibility.** Nothing installs `pandoc` or a pinned Node for the user.
4. **Homebrew cannot work**: writes outside the container are denied and brew's bottle
   prefix is fixed at `/opt/homebrew`.

**What we keep from it:** security-scoped bookmarks are the right way to express B1 on
macOS. They govern *what the VM may see of the host*, and are used inside the VM model.

### D3 — Rejected: Apple `container`

Per-container VMs give strong isolation and fast cold start, but require macOS 26 on Apple
silicon, and would make B2 a VM boundary on macOS while it stays bubblewrap on Windows.
Two isolation designs is the outcome we are trying to avoid.

### D4 — File model: workspace inside, `files/` bound from the host

```
~/workspaces/<slug>/          # in the distro / VM disk — the agent's work tree
  AGENTS.md, CLAUDE.md        # preset instructions (v0.2)
  files/                      # bind of the host's ~/Cowork/<slug>
```

Rationale is performance, and it is the whole of the performance story:

| path | mechanism | cost vs native |
|---|---|---|
| workspace root | ext4 on the VM/WSL disk (virtioblk) | ~2–3× |
| `files/` | virtiofs (macOS) / drvfs (Windows) | ~6–9× |

(Numbers from the [apple/container maintainers](https://github.com/apple/container/discussions/1516);
CPU overhead on `Virtualization.framework` with an arm64 guest is under ~5% and is not a
consideration.)

The agent's heavy I/O — greps, builds, `git status` — happens on the fast path. The slow
path carries the documents a user drags in and the artefacts they take out, where a 6× cost
on a handful of files is invisible. Workspace instruction templates must state the
convention: *work in the workspace root; read inputs from and write deliverables to `files/`.*

This also replaces `\\wsl.localhost` (`cowork-host/src/workspace/mod.rs::files_unc_path`).
On Windows, `automount=false` kills `/mnt/c` and we mount **only** `C:\Users\<u>\Cowork\<slug>`
onto `files/` via drvfs. The Explorer / Finder button then opens a native host path. Both
platforms get the same file model, and the host surface exposed to an agent shrinks to
exactly the folder the user puts files in.

### D5 — Hardening ladder

| | Change | Residual risk / what breaks |
|---|---|---|
| **L1** | `wsl.conf`: `[automount] enabled=false`, `[interop] enabled=false, appendWindowsPath=false` (Lima: no host mounts beyond `files/`) | With sudo, `mount -t drvfs` re-mounts `C:`. **`interop=false` breaks the agents' automatic browser launch for OAuth.** |
| **L2** | Drop `/etc/sudoers.d/cowork` after provisioning; installs run as root at provision time | brew and mise self-update still work (they own their prefixes); `apt` becomes app-mediated |
| **L3** | bubblewrap per session: workspace RW, that agent's config dir RW, toolchain RO, rest of `$HOME` invisible, private `/tmp` | Network stays open (§2). Agents that apply their own sandbox nest a second one. |

L1's browser breakage is already mitigated: sign-in happens inside the session terminal, and
the host detects the OAuth URL and opens it in the real browser ("Open sign-in page",
release-blocking in `v0.2-full-gate.md` §8). Whether all three agents *print* the URL when
interop is off is unverified — spike S1.

L2 makes a **sudo-free package manager mandatory**, which is why brew exists (§6).

L3's git-dir placement (§7) means the undo history sits outside every sandbox: an agent
cannot destroy the record of what it did.

### D6 — VM lifecycle on macOS

Default: **lazy start when the app launches**, stop on quit and after N minutes idle.
Opt-in "start at login" in settings. Docker Desktop's always-on daemon is the failure mode
to avoid on a laptop. This is a **UX asymmetry with Windows**, where WSL2 starts on demand
and the environment is effectively instant. Whether lazy is acceptable depends on cold-boot
time — spike S4.

---

## 5. Architecture support (Windows-on-ARM, Apple silicon)

D1 puts a Linux VM on Apple silicon, so the rootfs must exist for **aarch64**. Today
`ROOTFS_URL` pins `ubuntu-noble-wsl-amd64`. Running it under Rosetta was rejected: it costs
CPU, adds a compatibility surface, and permanently forecloses Windows-on-ARM (for which
`cowork-host/src/preflight/decide.rs` already accepts `ARCH_ARM64`).

Homebrew was the only blocker, and it no longer is: Linux ARM64/AArch64 was
[promoted to Tier 1 on 2025-11-05](https://github.com/Homebrew/brew/pull/20974), which
[guarantees](https://docs.brew.sh/Support-Tiers) full CI coverage and bottles. Ship a second
rootfs; keep everything else.

`cowork/src/bootstrap/command.rs:11` still calls `/home/linuxbrew/.linuxbrew` "Homebrew's
official default install prefix on Linux **x86_64**". The prefix is arch-independent; only
the comment is wrong.

---

## 6. Package management

An agent mid-session needs `pandoc`, `poppler-utils`, `ripgrep`. After L2 there is no sudo,
so `apt` is out. Homebrew is the sudo-free, session-shared, arm64-clean answer, and — not a
small thing — the one an LLM reaches for unprompted. It stays.

Division of labour: **brew = packages, mise = language runtimes** (`node`, `python`) for the
user's own projects. Note that brew's prefix is global, so a package installed from one
workspace is visible from every other. That is the intended "loosely shared" behaviour, and
its cost is that one workspace can poison the set for the others. Accepted.

---

## 7. Undo ("go back to before")

The user is a non-developer and must never learn that git exists.

**Mechanism.** Cowork snapshots the workspace with a bare repository kept **outside the work
tree**: `git --git-dir=~/.cowork/history/<slug>.git --work-tree=~/workspaces/<slug>`. The work
tree has no `.git`, so nothing shows up in `ls -a`, no agent can `git reset --hard` our
history, and L3 hides the git-dir from the sandbox entirely. Because the git-dir is ours,
`.git/info/exclude` carries `node_modules/`, `target/`, `.venv/` without touching the user's
tree.

**Trigger.** The working→idle transition already implemented in
`src/lib/shell/sessions.svelte.ts` (output quiet for `WORKING_QUIET_MS`) is our only
available approximation of an agent turn boundary — we host the agent's TUI and cannot see
its turns. Also on session close and app quit. A no-change snapshot is skipped.

**Rejected: instructing the agent to commit.** It is cheaper and it is wrong. Undo exists for
when the agent misbehaves, and an agent that is misbehaving is the least likely to honour
`AGENTS.md` — the instruction is also the first thing lost when its context compacts. It can
`git reset --hard` or delete the repo. The three agents follow instructions at different
rates, so the product would be differently safe per vendor. And the user would watch git
commands scroll past in the terminal, which defeats the requirement outright.

**Invariants.**

1. **Undo restores; it never deletes.** `git checkout <snapshot> -- .`, never `git clean`.
   Files the agent created stay behind. "I went back and my file vanished" is the worst
   possible outcome for this user.
2. **Nothing the user put in `files/` is ever removed.** Invariant 1 already guarantees it.

With those two, whether `files/` is inside the snapshot becomes a pure performance question
(few documents: include; large media: exclude), decidable later — spike S5.

Nested repositories the user clones into a workspace are recorded as gitlinks and their
contents are not snapshotted. They carry their own history. Accepted.

---

## 8. Open questions and spikes

Each is stated so a result decides something.

| # | Spike | Decides |
|---|---|---|
| **S1** | With `interop=false`, do claude / codex / agy print the OAuth URL to stdout? | Whether L1 ships, or whether we need a host-side browser bridge |
| **S2** | Does Ubuntu 24.04's `kernel.apparmor_restrict_unprivileged_userns` block bubblewrap inside the WSL2 kernel? Does an agent's own sandbox nest inside ours? | Whether L3 is bubblewrap or something else |
| **S3** | With `automount=false`, can a single host directory still be mounted via `mount -t drvfs`? | Whether D4 works on Windows |
| **S4** | Cold-boot time of Lima + our rootfs on Apple silicon | D6: lazy start vs start-at-login default |
| **S5** | Snapshot latency of `git add -A` over a realistic workspace, with and without `files/` on the slow path | §7 scope |
| **S6** | `brew install poppler pandoc` on aarch64 Ubuntu 24.04 — bottles, or source builds? | Confirms §5/§6 |
| **S7** | Reproduce the mise/`.bashrc` defect (§9) in the distro: which caller actually gets an empty PATH? | The shape of the fix |

---

## 9. Known defect: shell activation

`cowork/src/bootstrap/command.rs:118` appends two activation lines to `~/.bashrc`:

```
eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
eval "$(~/.local/bin/mise activate bash)"
```

Ubuntu's stock `.bashrc` opens with

```bash
case $- in
    *i*) ;;
      *) return;;
esac
```

so a non-interactive shell never reaches either line — and does not read `.bashrc` at all in
the first place. Both brew and mise are affected; this is not a mise bug, it is the
activation pattern.

It is masked today because the embedded terminal spawns `bash -li` and children inherit the
exported `PATH`. It breaks for shells that are not descendants of that login shell:
`wsl.exe -d Cowork -- bash -c …` (our own `run_guest` path), headless `codex exec`, cron.

The invariant to adopt: **anything an arbitrary non-interactive shell must find belongs on
the default `PATH` (`/usr/local/bin`), not behind a shell hook** — the rule the injected guest
binary already follows. Candidate fixes, to be settled by S7: move activation to
`/etc/profile.d/cowork.sh`, use `mise activate --shims` (mise's own recommendation for
non-interactive contexts), and/or symlink shims into `/usr/local/bin`. Note `sh -c` (dash)
ignores `BASH_ENV`, so a PATH entry alone is not sufficient in the general case.

---

## 10. Deferred: visual unification

The wizard and the shell were designed in different passes and do not share a visual
language. A single design pass should unify them before the first public release. Not
scheduled; recorded so it is not rediscovered.
