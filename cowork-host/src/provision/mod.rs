//! Distro provisioning (WP5): create the dedicated `Cowork` distro without
//! touching an existing `Ubuntu`, inject the guest CLI, and tear down on
//! uninstall. Pure decision logic lives here and in the submodules behind the
//! [`ProvisionOps`] seam; the `#[cfg(windows)]` `wsl.exe` / WinHTTP-download
//! impl (`windows_provision`) is the only OS-specific part.

mod command;
pub mod list;
#[cfg(any(windows, test))]
mod url;
#[cfg(windows)]
mod windows_provision;

pub use command::{
    already_exists_envelope, import_args, import_failed_envelope, install_failed_envelope,
    install_named_args, rootfs_fetch_failed_envelope, unregister_args, unregister_failed_envelope,
    user_create_failed_envelope, verify_checksum,
};
#[cfg(windows)]
pub use windows_provision::WindowsProvisionOps;

use cowork_errors::{Code, Envelope, Stage};

/// The dedicated distro name. Never collides with a user's `Ubuntu`.
pub const DISTRO_NAME: &str = "Cowork";
/// The Store image used only for the fallback `--install --name` path.
pub const FALLBACK_STORE_DISTRO: &str = "Ubuntu-24.04";
/// Pinned vanilla Ubuntu 24.04 rootfs on the Cowork GitHub Releases mirror.
/// TODO: replace with the real release asset URL once it is uploaded.
pub const ROOTFS_URL: &str = "https://github.com/conr2d/cowork/releases/download/rootfs-ubuntu-24.04/cowork-ubuntu-24.04-rootfs.tar.gz";
/// SHA-256 of the pinned rootfs (hex).
/// TODO: replace with the real digest once the asset is uploaded.
pub const ROOTFS_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Result of one `wsl.exe` invocation in provisioning. Unlike WSL enablement,
/// these run unelevated (WSL is already enabled by WP4), so there is no
/// elevation-declined variant.
#[derive(Debug, Clone)]
pub enum ExecResult {
    /// Process ran to completion. `output` is decoded stdout+stderr.
    Completed { exit_code: i32, output: String },
    /// The launch itself failed (wsl.exe missing / OS refused to start it).
    LaunchFailed { detail: String },
}

/// Outcome of fetching the pinned rootfs.
#[derive(Debug, Clone)]
pub enum FetchResult {
    /// Downloaded to `rootfs_path`; `sha256` is its computed digest (hex).
    Fetched { rootfs_path: String, sha256: String },
    /// Download failed. `http_status` is 0 when no response was received.
    Failed { http_status: u16 },
}

/// The side effects provisioning needs. Implemented for real on Windows
/// (`wsl.exe` + HTTPS download) and by mocks in tests.
pub trait ProvisionOps {
    /// `wsl --list --verbose`, output decoded to UTF-8.
    fn list_distros(&self) -> ExecResult;
    /// Download the pinned rootfs ([`ROOTFS_URL`]) and compute its SHA-256.
    fn fetch_rootfs(&self) -> FetchResult;
    /// `wsl --import Cowork <install_location> <rootfs_path> --version 2`.
    /// The impl chooses the install location.
    fn import(&self, rootfs_path: &str) -> ExecResult;
    /// `wsl --install Ubuntu-24.04 --name Cowork --no-launch` (Store fallback).
    fn install_named(&self) -> ExecResult;
    /// `wsl --unregister Cowork`.
    fn unregister(&self) -> ExecResult;
}

/// Outcome of [`provision`].
#[derive(Debug, Clone)]
pub enum ProvisionOutcome {
    /// A fresh `Cowork` distro is registered (via import or the Store fallback).
    Ready,
    /// A `Cowork` distro already existed; left as-is for idempotent re-use. The
    /// caller may surface [`already_exists_envelope`] to offer re-provisioning.
    AlreadyExists,
    /// Provisioning failed; present `0` by its kind.
    Failed(Envelope),
}

/// `host.wsl_not_found` (Internal) stamped for the provision stage.
fn wsl_not_found_envelope(detail: String) -> Envelope {
    Envelope::new(Code::HostWslNotFound, Stage::Provision).with_cause(&detail)
}

/// Provision the dedicated `Cowork` distro.
///
/// Flow:
/// 1. If a `Cowork` distro already exists → [`ProvisionOutcome::AlreadyExists`]
///    (idempotent; never re-imports over it). A failed list probe is non-fatal —
///    provisioning proceeds and surfaces its own errors.
/// 2. Primary path: fetch the pinned rootfs, verify its checksum, `--import`.
///    - checksum mismatch → `Failed(distro.checksum_mismatch)` and STOP (a
///      corruption / MITM signal must never silently fall back to the Store).
///    - import returned nonzero → `Failed(distro.import_failed)` (a verified
///      rootfs that WSL rejected is a real fault worth surfacing; the Store path
///      shares the same WSL subsystem and would not help).
///    - import success → `Ready`.
/// 3. If the rootfs mirror is unreachable (fetch failed) → Store fallback
///    `--install --name` (its only purpose: avoid a hard Microsoft-Store
///    dependency *when the mirror works*).
/// 4. A failed launch of `wsl.exe` at any step → `Failed(host.wsl_not_found)`.
pub fn provision(ops: &dyn ProvisionOps) -> ProvisionOutcome {
    if let ExecResult::Completed {
        exit_code: 0,
        output,
    } = ops.list_distros()
    {
        let entries = list::parse_distro_list(&output);
        if list::distro_present(&entries, DISTRO_NAME) {
            return ProvisionOutcome::AlreadyExists;
        }
    }

    match ops.fetch_rootfs() {
        FetchResult::Fetched {
            rootfs_path,
            sha256,
        } => {
            if let Err(env) = verify_checksum(ROOTFS_SHA256, &sha256) {
                return ProvisionOutcome::Failed(env);
            }
            match ops.import(&rootfs_path) {
                ExecResult::Completed { exit_code: 0, .. } => ProvisionOutcome::Ready,
                ExecResult::Completed { exit_code, .. } => {
                    ProvisionOutcome::Failed(import_failed_envelope(exit_code))
                }
                ExecResult::LaunchFailed { detail } => {
                    ProvisionOutcome::Failed(wsl_not_found_envelope(detail))
                }
            }
        }
        FetchResult::Failed { .. } => fallback_install(ops),
    }
}

/// Store fallback: `wsl --install Ubuntu-24.04 --name Cowork --no-launch`.
fn fallback_install(ops: &dyn ProvisionOps) -> ProvisionOutcome {
    match ops.install_named() {
        ExecResult::Completed { exit_code: 0, .. } => ProvisionOutcome::Ready,
        ExecResult::Completed { exit_code, .. } => {
            ProvisionOutcome::Failed(install_failed_envelope(exit_code))
        }
        ExecResult::LaunchFailed { detail } => {
            ProvisionOutcome::Failed(wsl_not_found_envelope(detail))
        }
    }
}

/// Uninstall: `wsl --unregister Cowork`. Clean precisely because the install is
/// isolated to a dedicated distro — other distros are untouched. Host-side state
/// cleanup (RunOnce, wizard state, `~/.cowork`) is the caller's responsibility.
pub fn remove_cowork(ops: &dyn ProvisionOps) -> Result<(), Envelope> {
    match ops.unregister() {
        ExecResult::Completed { exit_code: 0, .. } => Ok(()),
        ExecResult::Completed { exit_code, .. } => Err(unregister_failed_envelope(exit_code)),
        ExecResult::LaunchFailed { detail } => Err(wsl_not_found_envelope(detail)),
    }
}
