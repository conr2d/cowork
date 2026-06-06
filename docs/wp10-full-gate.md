# WP10 — Full gate runbook (v0.1 definition of done)

The **authoritative** v0.1 acceptance gate. It is run **manually / semi-automated** on a
real Windows clean-room because the final step (agent OAuth login) is interactive and
cannot be fully automated in CI. Everything cheaply automatable already runs per-PR in
`ci.yml` (frontend, guest unit tests, host compile, host/guest conformance, the
docker `guest-e2e` real-installer smoke, and the real-WSL2 `wsl-integration` slice).
This gate covers what those cannot:

- the **real Windows host driver** end to end (WebView2 wizard, `wsl.exe`, UAC elevation,
  ConPTY↔`wsl.exe` PTY bridge, registry `RunOnce`);
- a **real reboot** with `RunOnce` auto-resume;
- **real interactive OAuth** with the host-browser handoff over WSLInterop;
- the **from-nothing** path (WSL never enabled) for **every agent selection**;
- the **induced-failure** matrix with the correct localized message + recovery affordance;
- **idempotency** and **Remove Cowork** teardown;
- **codex install on a real (single-user) IP** — deliberately *not* CI-tested because the
  codex installer resolves versions via unauthenticated `api.github.com`, which is HTTP-403
  rate-limited on shared CI runner IPs (a single user IP is well under the 60 req/h limit).

> Clean-room decision (locked): a **Hyper-V VM (Windows 11 guest) with nested
> virtualization + checkpoints**. Windows Sandbox is unusable (no nested virtualization).
> A spare physical Win11 laptop is an acceptable alternative for the parts a checkpoint
> would otherwise revert, but the VM is preferred for fast, deterministic reverts.

---

## 1. Prerequisites

| # | Item | How to get it | Status check |
|---|------|---------------|--------------|
| 1 | **Fresh `Cowork.exe` (NSIS installer) built from the current `main`** | `gh workflow run release.yml --ref main`, then download the `cowork-windows-x64` artifact | The build's `headSha` must equal `git rev-parse main`. ⚠️ A stale installer embeds an old guest binary (without the `~/workspaces/default` layout and the `killpg` install-timeout fix). |
| 2 | **Published rootfs** | already live: release tag `rootfs-ubuntu-24.04`, asset `cowork-ubuntu-24.04-rootfs.tar.gz` | `gh release view rootfs-ubuntu-24.04` returns the asset; its SHA-256 equals `ROOTFS_SHA256` in `cowork-host/src/provision/mod.rs` |
| 3 | **Hyper-V host** | Windows 10/11 Pro/Enterprise with Hyper-V enabled | `Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V` = Enabled |
| 4 | **Real agent subscriptions** for OAuth | Anthropic (claude), OpenAI (codex), Google (antigravity) accounts the tester can log into | tester is signed out / has credentials ready |
| 5 | **A second, pre-existing distro to protect** | inside the VM, install a stock `Ubuntu` distro **before** the gate (see §3.4) | `wsl -l -v` shows `Ubuntu` — used to prove Cowork never touches it |

Do **not** start until item 1's `headSha` matches `main`.

---

## 2. Hyper-V VM setup (run on the Hyper-V host, elevated PowerShell)

Nested virtualization is required (WSL2 = a VM inside the guest) and is **incompatible with
Dynamic Memory** — use static RAM. Size the disk so the happy path clears the 32 GiB
*recommended* preflight bar, leaving room to later shrink free space below the 16 GiB *hard*
bar for the induced-failure case.

```powershell
$VM   = 'Cowork-FullGate'
$VHD  = "C:\HyperV\$VM.vhdx"
$ISO  = 'C:\iso\Win11.iso'        # a Win11 install ISO

New-VM -Name $VM -Generation 2 -MemoryStartupBytes 8GB -NewVHDPath $VHD -NewVHDSizeBytes 80GB
Set-VM -Name $VM -StaticMemory                                   # nested virt: no Dynamic Memory
Set-VMProcessor -VMName $VM -Count 4 -ExposeVirtualizationExtensions $true   # nested virt ON
Set-VMMemory  -VMName $VM -StartupBytes 8GB
Add-VMDvdDrive -VMName $VM -Path $ISO
# Generation-2 needs Secure Boot tuned for the OS; leave default MS UEFI cert for Win11.
Start-VM -Name $VM
```

Install Windows 11 in the guest, finish OOBE, then **inside the guest** confirm the clean
baseline (WSL must be *absent*, not merely disabled):

```powershell
wsl --status        # expect: not installed / no default distro
wsl -l -v           # expect: error or empty
Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Windows-Subsystem-Linux   # Disabled
Get-WindowsOptionalFeature -Online -FeatureName VirtualMachinePlatform              # Disabled
```

Take the baseline checkpoint **before** anything Cowork-related is installed:

```powershell
# on the host
Checkpoint-VM -Name 'Cowork-FullGate' -SnapshotName 'clean-wsl-never-enabled'
```

Revert between runs with:

```powershell
Restore-VMCheckpoint -VMName 'Cowork-FullGate' -Name 'clean-wsl-never-enabled' -Confirm:$false
```

---

## 3. Standing setup inside the clean checkpoint

Do these once, then take a second checkpoint `ready-to-install` so each agent run starts
from an identical, reproducible point that still has WSL *absent*.

1. Copy the fresh `Cowork.exe` (prereq 1) into the guest.
2. Sign the tester out of any browser sessions for the agents (so OAuth is a genuine login).
3. Set the guest's Windows display language to exercise i18n (see §7): run one pass each in
   **en**, **ja**, **ko**.
4. **Protect-an-existing-distro check (do once):** install a stock Ubuntu so the gate can
   prove isolation. `wsl --install -d Ubuntu` *after* enabling WSL — but since the baseline
   has WSL absent, instead install it during a dedicated isolation run (see §5, row
   "existing Ubuntu untouched") rather than in the baseline.

```powershell
Checkpoint-VM -Name 'Cowork-FullGate' -SnapshotName 'ready-to-install'
```

---

## 4. Happy path (run per agent selection — §5)

From a freshly reverted `ready-to-install` checkpoint:

| Step | Action | Expected (PASS) |
|------|--------|-----------------|
| 4.1 | Launch `Cowork.exe` | Wizard opens; language matches the guest locale (en/ja/ko); language selector present on the first screen |
| 4.2 | **Agent selection** screen | Can select 1+ of claude / codex / antigravity; selection drives step 4.7 |
| 4.3 | **Preflight** | All checks pass on this VM; each renders localized; proceeds |
| 4.4 | **WSL enable/update** | UAC prompt → accept; `wsl --install --no-distribution` + `wsl --update`; WSL app ≥ 2.4.4 |
| 4.5 | **Reboot + auto-resume** | Wizard writes `Cowork.exe --resume` to `HKCU\...\RunOnce`, reboots, **auto-relaunches after login with no user action**, resumes at the right step; the `RunOnce` value is consumed (not left to loop) |
| 4.6 | **Provision distro** | A distro named **`Cowork`** is created from the published rootfs (`wsl --import`, new `.tar.gz` format); `wsl -l -v` shows `Cowork` |
| 4.7 | **Toolchain + agent install** | brew + mise present; for each selected agent the binary installs (`claude`/`codex`/`agy`); creds routed under `~/.cowork/creds/...` |
| 4.8 | **Auth (real OAuth)** | Running the agent's login from the embedded terminal **opens the host Windows browser via WSLInterop**, completes OAuth; with Interop disabled, the printed-URL/device-code fallback completes login; the post-login token lands under (or is redirected into) `~/.cowork/creds/<agent>` |
| 4.9 | **Done** | Dropped into the embedded terminal at **`~/workspaces/default/`**; terminal is interactive (keystroke echo, resize reflow, 256-color) |

---

## 5. Per-agent-selection matrix

Each row = one full §4 run from a reverted checkpoint. All must pass for v0.1 DoD.

| Selection | claude login | codex login | antigravity login | `~/workspaces/default` terminal | Notes |
|-----------|:---:|:---:|:---:|:---:|-------|
| claude only | ☐ | — | — | ☐ | |
| codex only | — | ☐ | — | ☐ | confirms codex installs on a **real IP** (no api.github.com 403) |
| antigravity only | — | — | ☐ | ☐ | first-run browser OAuth, token at `~/.gemini/antigravity-cli/...` → routed |
| all three (multi) | ☐ | ☐ | ☐ | ☐ | install-domain isolation: `~/.local/bin/{claude,codex,agy}` distinct |

Also, on **one** of these runs, verify **idempotency**: re-run the wizard → it **re-uses** the
existing `Cowork` distro (no duplicate in `wsl -l -v`), and re-running bootstrap/agent-install
is a no-op (no errors).

---

## 6. Induced-failure matrix

Each row: revert to a checkpoint, induce the condition, run the wizard, and confirm the exact
error **code + kind**, a **localized** message (not a raw code, not English in ja/ko), and the
**recovery affordance** for that kind (Blocker = stop+explain; NeedsUserAction =
instructions + Retry; Transient = auto-retry/backoff + manual Retry; Internal = report +
log path). Codes are from `errors.json` (`docs/v0.1.md`).

| # | Scenario | How to induce | Expected code | kind | Affordance |
|---|----------|---------------|---------------|------|------------|
| 6.1 | Virtualization disabled in firmware | host: `Set-VMProcessor -ExposeVirtualizationExtensions $false`; revert WSL to clean | `preflight.virtualization_disabled` | NeedsUserAction | enable-virtualization instructions + Retry; does **not** proceed |
| 6.2 | Insufficient disk | shrink guest free space below 16 GiB (fill a dummy file) | `preflight.insufficient_disk` | NeedsUserAction | shows required vs available; blocks until freed |
| 6.3 | UAC declined mid-flow | click **No** on the elevation prompt at step 4.4 | `wsl.elevation_denied` | NeedsUserAction | retryable (not a hard Blocker) |
| 6.4 | Network drop during install | disconnect the VM NIC during step 4.7 | `common.network_failed` (or endpoint-specific) | Transient | auto-retry/backoff, then manual Retry; reconnect → succeeds |
| 6.5 | Existing `Cowork` distro conflict | run with a `Cowork` distro already present | `distro.already_exists` | NeedsUserAction | offers re-use vs re-provision |
| 6.6 | **Existing `Ubuntu` left untouched** | install stock `Ubuntu` first, then run the full happy path | (no error) | — | `wsl -l -v` shows **both**; the pre-existing `Ubuntu`'s files/state are unchanged |
| 6.7 | Remove Cowork (uninstall) | run **Remove Cowork** after a successful install | (no error; `distro.unregister_failed` only on failure) | — | only the `Cowork` distro is unregistered; other distros intact; host state (`RunOnce`, wizard state, `~/.cowork` host config) cleared |
| 6.8 | *(optional)* Enterprise policy block | set the WSL-blocking Group Policy/MDM registry key | `preflight.wsl_blocked_by_policy` | Blocker | stop + explain; cannot bypass |

> codex-specific reminder: if codex install fails with a 403 here, check the VM's egress —
> a shared corporate NAT can collectively exhaust the unauthenticated `api.github.com` quota
> even for a single VM. On a normal home/single-user IP it installs cleanly. (Tracked for WP7:
> surface the 403 explicitly + retry/backoff.)

---

## 7. i18n spot-check

- Boot the guest in **ja** and **ko** Windows display language → wizard first screen renders
  in that language; the language selector overrides it; an unknown key would fall back to **en**
  (not show the raw key).
- During an induced failure (§6), confirm the error body is localized in the active language.

---

## 8. Pass/fail recording

Record one row per run; attach screenshots of the OAuth handoff and the final terminal.

```
Date | Build headSha | Locale | Agent selection | Result (PASS/FAIL) | Notes / code seen
-----+---------------+--------+-----------------+--------------------+------------------
```

**v0.1 is DONE when:** every §5 row passes for every agent selection, every §6 row shows the
correct localized code + recovery, idempotency and Remove Cowork hold, and §7 passes in all
three locales — all from a reverted clean checkpoint.

---

## 9. Notes / open items carried into this gate

- **codex install** is validated here (real IP) because CI cannot (api.github.com 403 on
  shared runner IPs; the installer has no token support). See `docs/v0.1.md` and project
  memory `cowork-wp7-installer-deadlock`.
- **New-format `wsl --import`** of the `.tar.gz` rootfs is already proven on WSL ≥ 2.4.4 by
  the CI `wsl-integration` slice; this gate re-confirms it on the full from-nothing path.
- **Install-timeout robustness** (`killpg` on the installer process group) is in the embedded
  guest as of the build in prereq 1; an installer that strands a daemon can no longer hang the
  install past its timeout.
- **WP4 floor caveat:** if a clean Win11 image ships an inbox `wsl.exe` older than 2.4.4 and
  `wsl --update` cannot reach the Store/MSI, expect `wsl.update_unsupported_inbox` — note the
  inbox version observed and confirm `--update` lifts it to ≥ 2.4.4.
