# Isolation and platforms

**Status:** accepted (design), 2026-07-10; D4 and §7 revised the same day. D5 and §8 revised
2026-07-14 with measurements (L1 rejected, L3 confirmed feasible). Supersedes the implicit
isolation claim of v0.1.
**Scope:** what Cowork isolates, how, and what that implies for macOS. Feeds the **v0.4**
Isolation work package — deferred from v0.3 because the product is single-user and
single-machine and no harm has been observed. Nothing here is implemented yet, except §9.

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

**What we keep from it:** nothing, in the end. Security-scoped bookmarks looked like the
right way to express B1 on macOS — they would govern what the VM may see of the host. D4
removes the need: the VM is given **no host filesystem at all**, so there is nothing to
scope. The bookmark idea is recorded here only so it is not rediscovered as an oversight.

### D3 — Rejected: Apple `container`

Per-container VMs give strong isolation and fast cold start, but require macOS 26 on Apple
silicon, and would make B2 a VM boundary on macOS while it stays bubblewrap on Windows.
Two isolation designs is the outcome we are trying to avoid.

### D4 — File model: everything lives in the environment; the host browses it

```
~/workspaces/<slug>/          # in the distro / VM disk, on ext4 — the agent's work tree
  AGENTS.md, CLAUDE.md        # preset instructions (v0.2)
  files/                      # what the user drags in and takes out
```

**No host filesystem is ever mounted into the environment.** The agent has zero writable host
paths. The exposure runs the other way: the host — a trusted party — mounts the guest for
browsing.

| platform | reverse mount | provided by |
|---|---|---|
| Windows | `\\wsl.localhost\Cowork\…` (9p) | WSL, free |
| macOS | `smb://<guest>/cowork` via a `smbd` in the guest | us |

The mechanisms differ; the model does not. Neither the user, the guest CLI, nor the L3
sandbox policy can tell them apart. That asymmetry is far cheaper than letting the file
*layout* differ per platform.

Performance falls out correctly. The agent's heavy I/O — greps, builds, `git status` — is
local ext4. The network share is traversed only when a human drags a file in Finder or
Explorer, where latency is irrelevant. The numbers that would have mattered under a
bind-mount design ([virtioblk ~2–3× native, virtiofs a further ~3× on top of
that](https://github.com/apple/container/discussions/1516)) never enter the agent's path.
CPU overhead on `Virtualization.framework` with an arm64 guest is under ~5% and is not a
consideration.

macOS specifics (see S9): Lima's `vzNAT` gives the guest a host-reachable IP
[without `socket_vmnet`, a sudoers entry, or a root helper](https://lima-vm.io/docs/config/network/vmnet/),
so `smbd` binds port 445 directly. That matters: Finder handles `smb://host:port`
[poorly](https://discussions.apple.com/thread/553264), so a port-forwarding design would have
died here. A random password is minted at provisioning and stored in the host Keychain;
the app mounts via `NetFSMountURLSync` with no prompt (never `mount_smbfs` with the password
in `argv` — it appears in `ps`). `smbd` binds the `vzNAT` interface only; a share reachable
from the LAN is an incident.

Rejected alternatives for the macOS reverse mount: **NFS** (no authentication, so host-only
binding is mandatory; uid mapping is messy; wants a privileged port), **File Provider
Extension** (Apple's native answer and the prettiest Finder integration, but enumeration,
transfer and conflict handling are weeks of work), **FUSE-T / macFUSE** (third-party
dependency). Samba is the balance point.

This supersedes `\\wsl.localhost` only as a *concept name*: on Windows it remains exactly the
mechanism, and `cowork-host/src/workspace/mod.rs::files_unc_path` stays. On Windows,
`automount=false` simply kills `/mnt/c` outright — nothing needs to be mounted back.

### D5 — Hardening ladder

| | Change | Status | Residual risk / what breaks |
|---|---|---|---|
| **L1** | `wsl.conf`: `[automount] enabled=false`, `[interop] enabled=false, appendWindowsPath=false` (Lima: no host mounts at all) | **Rejected on Windows** (2026-07-14) | See below — the cost is sign-in, the benefit is against a threat we have not observed |
| **L2** | Drop `/etc/sudoers.d/cowork` after provisioning; installs run as root at provision time | Open | brew and mise self-update still work (they own their prefixes); `apt` becomes app-mediated |
| **L3** | bubblewrap per session: workspace RW, that agent's config dir RW, toolchain RO, rest of `$HOME` invisible, private `/tmp` | **Feasible, measured** | Network stays open (§2). Agents that apply their own sandbox nest a second one — unmeasured |

**L1 is rejected, and the risk it would have covered is accepted.** `interop=true` lets the
guest execute Windows binaries **as the Windows user, with no sandbox and no UAC prompt** —
`powershell.exe -c "Get-Content $env:USERPROFILE\.ssh\id_rsa"` works from inside the distro.
That is the largest B1 hole there is, and it also makes `automount=false` nearly moot on its
own: killing `/mnt/c` buys little while `powershell.exe` still reads and writes all of `C:`.

We accept it anyway. Turning interop off breaks the agents' automatic browser launch, leaving
the printed-URL path (§ "Open sign-in page") as the *only* way to sign in — a path already
fragile enough that claude hard-wraps its own URL. The realistic threat is not a malicious
agent but a **prompt-injected** one: an agent reads a hostile README or web page and runs
`powershell.exe`. For a single-user, single-machine product with no observed incident, that
trade goes to sign-in that works. **This is an accepted risk. It is not a safe configuration,
and must never be described as one.** Revisit if the product ever gains a multi-user or
untrusted-input surface.

L2 makes a **sudo-free package manager mandatory**, which is why brew exists (§6).

**L3 works on WSL2 with no AppArmor work at all** (measured 2026-07-14; kernel
6.18.33.2-microsoft-standard-WSL2). The Microsoft kernel compiles AppArmor in but does not
enable it — no `security=apparmor`, no securityfs, and so Ubuntu 24.04's
`kernel.apparmor_restrict_unprivileged_userns=1` does not exist. apt's `bubblewrap` 0.9.0
creates an unprivileged user namespace as the `cowork` user and exits 0. No profile, no
sysctl override, no setuid binary.

Two consequences worth carrying:

- **Use apt's `/usr/bin/bwrap`, not brew's.** The path is stable; brew's real binary sits at
  `Cellar/bubblewrap/<version>/bin/bwrap`, so any path-matched policy detaches on upgrade.
  brew exists so *agents* can install packages without sudo (§6); bwrap is our infrastructure,
  not an agent-facing package.
- **There is no LSM backstop.** On stock Ubuntu, the shipped `bwrap-userns-restrict` profile
  confines bwrap's children with `audit deny capability`, so bwrap cannot become a general
  userns-restriction bypass. On WSL2 that machinery is absent, so the isolation rests
  **entirely** on bwrap's own flags and mount namespace. Get `--unshare-all` or a bind wrong
  and nothing catches it. On macOS/Lima, where AppArmor *is* enforcing, that profile becomes
  necessary again — see #7 for the version to use.

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

**A checkpoint is the state immediately before the user sends the agent a new instruction.**

Not "whenever output goes quiet". A timestamped list of quiet moments is unusable: those are
our internal events, not events the user remembers. The one moment a user *can* place is
"before I asked it to do that", so undo means *go back to before this instruction* — and one
instruction yields exactly one checkpoint, instead of one per lull in the agent's output.

We already own that moment. `Terminal.svelte` proxies every keystroke through
`term.onData(… invoke('pty_write') …)`, so the user pressing Enter on a new instruction
passes through our code. The working→idle transition in `src/lib/shell/sessions.svelte.ts`
(output quiet for `WORKING_QUIET_MS`) remains as the **fallback** for an agent running long
without user input. Also on session close and app quit. A no-change snapshot is skipped.

Whether Enter means *send* or *newline* differs per agent — spike S8. That accuracy is what
the trigger rests on.

**The user never picks from a list of checkpoints.** Two surfaces, in this order:

- **L0 — Undo / Redo.** No list. One press goes back one checkpoint, another goes back
  further. It is `Cmd+Z`, the only undo a non-developer already knows. This covers the
  common case ("cancel what it just did") and asks the user to choose nothing.
- **L1 — per-file previous versions.** The accident a user actually suffers is *"this file is
  ruined"*, not *"something went wrong at some time"*. Right-click a file → previous versions
  by time. The selection is made by **looking, not by reading a label**, which is why Time
  Machine, Dropbox and Google Docs version history work at all. It is nearly free for us:
  extract the old blob to a temp path and hand it to `tauri-plugin-opener`, which opens it in
  the host's default application. No viewer to build.
- **L2 — a workspace-wide checkpoint list.** Probably never. Decide only after L0 and L1 are
  in use and we can see what they fail to cover.

**Labels.** L0 and L1 need only a timestamp; the preview does the label's job. If L2 ever
happens, three ways to name a checkpoint, none free: buffering keystrokes until Enter
(vendor-neutral, but TUI line editing, paste, and per-agent send keys make it fragile);
parsing the agent's own session file (we already read those for session-uuid capture, so the
schema-drift risk is one we accept — but reading the user's prose is a deeper reach);
generating a summary with a headless agent call (robust, but latency, tokens, and a
credential dependency for a background task). Whichever is chosen, a pasted secret must never
land in a commit message inside `~/.cowork/history/<slug>.git` — store labels in a sidecar,
truncated.

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

`files/` is inside the snapshot. D4 puts it on guest ext4, so snapshotting it is cheap and
per-file version history applies uniformly across the workspace — which matters, because the
file a user wants restored is usually a deliverable, and deliverables live in `files/`. Under
the superseded bind-mount design this was an open trade-off; it no longer is.

Nested repositories the user clones into a workspace are recorded as gitlinks and their
contents are not snapshotted. They carry their own history. Accepted.

---

## 8. Open questions and spikes

Each is stated so a result decides something.

| # | Spike | Decides |
|---|---|---|
| ~~S1~~ | ~~With `interop=false`, do claude / codex / agy print the OAuth URL to stdout?~~ | **Dropped 2026-07-14.** It existed only to gate L1, and L1 is rejected (D5) |
| ~~S2a~~ | ~~Does `kernel.apparmor_restrict_unprivileged_userns` block bubblewrap inside the WSL2 kernel?~~ | **Answered 2026-07-14: no.** AppArmor is not enabled in the WSL2 kernel; apt's bwrap 0.9.0 runs unprivileged. L3 is bubblewrap (D5) |
| **S2b** | Does an agent's own sandbox (codex ships one) nest inside ours, or conflict? | Whether L3 must special-case an agent. Cheap; do it while building L3 |
| **S4** | Cold-boot time of Lima + our rootfs on Apple silicon | D6: lazy start vs start-at-login default |
| **S5** | Snapshot latency of `git add -A` over a realistic workspace | Whether the checkpoint trigger needs debouncing |
| **S6** | `brew install poppler pandoc` on aarch64 Ubuntu 24.04 — bottles, or source builds? | Confirms §5/§6 |
| **S8** | For each agent, does Enter send the message or insert a newline? | Accuracy of the §7 checkpoint trigger |
| **S9** | Lima `vzNAT` + guest `smbd` on 445 + `NetFSMountURLSync` with a Keychain credential — mounts with no prompt? | Whether D4's macOS half stands |
| **S10** | After the VM dies unexpectedly, what cleans up the stale `/Volumes/Cowork` mount? | D4 operational cost |

S3 (whether a single host directory can be drvfs-mounted with `automount=false`) was dropped:
D4 mounts no host directory into the environment.

S7 was resolved; §9 records the reproduced shell-startup behavior and the resulting fix.

---

## 9. Resolved: profile activation

Older bootstraps appended two activation lines to `~/.bashrc`:

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

Measured on Ubuntu by appending a marker line to each file and running
`env -i HOME=… <invocation>`:

| invocation | marker in `~/.bashrc` | marker in `~/.profile` |
|---|---|---|
| `bash -li` (our PTY) | reached | reached |
| `bash -lc` (headless wrappers) | **MISSING** | reached |
| `sh -lc` (dash) | **MISSING** | reached |
| `bash -c` / `sh -c` | MISSING | MISSING |

The fix is to append toolchain activation to `~/.profile`, after Ubuntu's existing
`~/.local/bin` PATH block, so every login shell sees Homebrew and mise. The block is POSIX
`sh`: Homebrew is loaded with `brew shellenv sh`, and mise contributes its shims directory
instead of `mise activate`, which is a prompt hook and does nothing useful outside an
interactive shell.

The invariant to adopt: **anything an arbitrary non-interactive shell must find belongs on
the default `PATH` (`/usr/local/bin`), not behind a shell hook** — the rule the injected guest
binary already follows. Non-login shells (`bash -c`, `sh -c`) still read no startup file and
inherit `PATH` from their parent; that behavior is correct and remains out of scope for
profile activation.

**The same pattern survives one place more.** `cowork/src/agent/mod.rs::ensure_shellrc_line`
appends `export CLAUDE_CONFIG_DIR=…` / `export CODEX_HOME=…` to `~/.bashrc`, so credential
routing has only ever taken effect in an interactive shell either. It is left alone here
because the credentials-at-default-paths change deletes that code outright — see the
`~/.cowork/creds` decision. If that change is ever abandoned, this line moves to
`~/.profile` too.

---

## 10. Deferred: visual unification

The wizard and the shell were designed in different passes and do not share a visual
language. A single design pass should unify them before the first public release. Not
scheduled; recorded so it is not rediscovered.
