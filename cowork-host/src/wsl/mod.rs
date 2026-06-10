//! WSL enable/update orchestration. The pure decision flow lives here; the
//! actual side effects (running `wsl.exe`, UAC elevation, RunOnce) sit behind
//! the [`WslOps`] seam so the flow is unit-testable off-Windows. The
//! `#[cfg(windows)]` implementation is `windows_exec::WindowsWslOps`.

mod command;
mod resume;
mod version;
#[cfg(windows)]
mod windows_exec;

pub use command::{WslOp, inbox_unsupported_envelope, is_inbox_unsupported, run_failure_envelope};
pub use resume::{
    RESUME_SCHEMA_VERSION, ResumeStage, ResumeState, clear_resume_state, load_resume_state,
    save_resume_state,
};
pub use version::{MIN_WSL_VERSION, WslVersion, parse_wsl_version, version_too_old_envelope};
#[cfg(windows)]
pub use windows_exec::{WindowsWslOps, arm_resume, disarm_resume};

use cowork_errors::{Code, Envelope, Stage};

/// Result of one `wsl.exe` invocation through the [`WslOps`] seam.
/// NOTE: do NOT derive `PartialEq`/`Eq` — a variant carries no `Envelope`, but
/// keep the derive set identical to `WslEnableOutcome` (just `Debug, Clone`) for
/// consistency; tests never compare `WslRun` by `==`.
#[derive(Debug, Clone)]
pub enum WslRun {
    /// Process ran to completion. `output` is decoded stdout+stderr (used for
    /// `--version` parsing and inbox detection; empty for elevated runs, which
    /// cannot capture the child's output).
    Completed { exit_code: i32, output: String },
    /// An elevated launch was declined at the UAC prompt (retryable).
    ElevationDeclined,
    /// The launch itself failed (binary missing, OS refused to start it).
    LaunchFailed { detail: String },
}

/// The side effects WSL enablement needs. Implemented by `WindowsWslOps` on
/// Windows and by mocks in tests.
pub trait WslOps {
    /// Run `wsl.exe` for `op`; `op.needs_elevation()` decides whether to elevate.
    fn run(&self, op: WslOp) -> WslRun;

    /// Probe general internet connectivity (proxy-aware). Used to disambiguate a
    /// network-dependent failure (`wsl --install`/`--update` downloads from
    /// Microsoft) from a WSL-specific one: if this returns `false`, the cause is
    /// the user's connection, not WSL. Best-effort; returns `true` when unsure.
    fn is_online(&self) -> bool;
}

/// Outcome of [`enable_wsl`].
/// NOTE: do NOT derive `PartialEq`/`Eq` — `Envelope` (in `cowork-errors`) does
/// not implement them, and we must NOT modify that crate. Tests assert outcomes
/// by destructuring + `matches!`, exactly like `tests/preflight.rs::assert_fail`
/// (which reads `env.code`/`env.kind`/`env.stage`/`env.context`).
#[derive(Debug, Clone)]
pub enum WslEnableOutcome {
    /// WSL is installed and meets the minimum version; proceed to provisioning.
    Ready,
    /// WSL was installed/updated and the machine must reboot. The caller
    /// persists resume state + arms RunOnce, then triggers the reboot.
    RebootRequired,
    /// A step failed. Present `0` (the envelope) by its kind.
    Failed(Envelope),
}

/// `wsl.elevation_denied` (NeedsUserAction, retryable).
fn elevation_denied_envelope() -> Envelope {
    Envelope::new(Code::WslElevationDenied, Stage::WslEnable)
}

/// `host.wsl_not_found` (Internal) — wsl.exe could not be launched at all.
fn wsl_not_found_envelope() -> Envelope {
    Envelope::new(Code::HostWslNotFound, Stage::WslEnable)
}

/// `wsl.reboot_required` (NeedsUserAction) — provided for the UI/caller to
/// render the reboot affordance that accompanies `WslEnableOutcome::RebootRequired`.
pub fn reboot_required_envelope() -> Envelope {
    Envelope::new(Code::WslRebootRequired, Stage::WslEnable)
}

/// `common.network_failed` (Transient) — a network-dependent WSL step failed and
/// the connectivity probe found no internet, so the cause is the user's
/// connection. `httpStatus` 0 means no HTTP response was obtained (offline),
/// matching the provision download convention.
fn network_failed_envelope() -> Envelope {
    Envelope::new(Code::CommonNetworkFailed, Stage::WslEnable).with_context("httpStatus", "0")
}

/// What the `--version` probe told us about the installed WSL app.
enum VersionProbe {
    /// Installed; version parsed.
    Present(WslVersion),
    /// The inbox wsl.exe that predates modern flags.
    Inbox,
    /// Not installed (or unparseable / un-launchable).
    Absent,
}

/// Drive WSL to an enabled, modern state.
///
/// Flow:
/// 1. Probe `--version`.
/// 2. Present & meets minimum → `Ready`.
/// 3. Present but too old → `--update` (elevated); success → `Ready`.
/// 4. Inbox wsl.exe → `Failed(update_unsupported_inbox)`.
/// 5. Absent → `--install --no-distribution` (elevated); success → `RebootRequired`.
///
/// Elevation declined at any elevated step → `Failed(elevation_denied)`.
pub fn enable_wsl(ops: &dyn WslOps) -> WslEnableOutcome {
    match probe_version(ops) {
        VersionProbe::Present(v) if v.meets_minimum() => WslEnableOutcome::Ready,
        VersionProbe::Present(_) => run_update(ops),
        VersionProbe::Inbox => WslEnableOutcome::Failed(inbox_unsupported_envelope()),
        VersionProbe::Absent => run_install(ops),
    }
}

fn probe_version(ops: &dyn WslOps) -> VersionProbe {
    match ops.run(WslOp::Version) {
        WslRun::Completed {
            exit_code: 0,
            output,
        } => match parse_wsl_version(&output) {
            Some(v) => VersionProbe::Present(v),
            None => VersionProbe::Absent,
        },
        WslRun::Completed { exit_code, output } => {
            if is_inbox_unsupported(exit_code, &output) {
                VersionProbe::Inbox
            } else {
                VersionProbe::Absent
            }
        }
        // Version is read-only; these branches are defensive.
        WslRun::ElevationDeclined | WslRun::LaunchFailed { .. } => VersionProbe::Absent,
    }
}

fn run_update(ops: &dyn WslOps) -> WslEnableOutcome {
    match ops.run(WslOp::Update) {
        WslRun::Completed { exit_code: 0, .. } => WslEnableOutcome::Ready,
        WslRun::Completed { exit_code, output } => {
            if is_inbox_unsupported(exit_code, &output) {
                WslEnableOutcome::Failed(inbox_unsupported_envelope())
            } else if !ops.is_online() {
                WslEnableOutcome::Failed(network_failed_envelope())
            } else {
                WslEnableOutcome::Failed(
                    run_failure_envelope(WslOp::Update, exit_code)
                        .expect("Update always maps to a failure envelope"),
                )
            }
        }
        WslRun::ElevationDeclined => WslEnableOutcome::Failed(elevation_denied_envelope()),
        WslRun::LaunchFailed { .. } => WslEnableOutcome::Failed(wsl_not_found_envelope()),
    }
}

fn run_install(ops: &dyn WslOps) -> WslEnableOutcome {
    match ops.run(WslOp::Install) {
        WslRun::Completed { exit_code: 0, .. } => WslEnableOutcome::RebootRequired,
        WslRun::Completed { exit_code, output } => {
            if is_inbox_unsupported(exit_code, &output) {
                WslEnableOutcome::Failed(inbox_unsupported_envelope())
            } else if !ops.is_online() {
                WslEnableOutcome::Failed(network_failed_envelope())
            } else {
                WslEnableOutcome::Failed(
                    run_failure_envelope(WslOp::Install, exit_code)
                        .expect("Install always maps to a failure envelope"),
                )
            }
        }
        WslRun::ElevationDeclined => WslEnableOutcome::Failed(elevation_denied_envelope()),
        WslRun::LaunchFailed { .. } => WslEnableOutcome::Failed(wsl_not_found_envelope()),
    }
}
