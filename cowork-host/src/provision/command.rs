//! `wsl.exe` argument vectors for distro provisioning, the failure→envelope
//! mappings, and the rootfs checksum check. All pure: building a command,
//! classifying an exit code, comparing two hex digests.

use cowork_errors::{Code, Envelope, Stage};

use super::{DISTRO_NAME, FALLBACK_STORE_DISTRO};

/// `wsl --import Cowork <install_location> <rootfs_path> --version 2` — the
/// primary path: import the pinned vanilla rootfs as a dedicated distro.
pub fn import_args(install_location: &str, rootfs_path: &str) -> Vec<String> {
    vec![
        "--import".to_string(),
        DISTRO_NAME.to_string(),
        install_location.to_string(),
        rootfs_path.to_string(),
        "--version".to_string(),
        "2".to_string(),
    ]
}

/// `wsl --install Ubuntu-24.04 --name Cowork --no-launch` — the Store fallback
/// when the pinned rootfs mirror is unreachable. `--name` isolates it from any
/// existing `Ubuntu`; `--no-launch` keeps us in control of first boot.
pub fn install_named_args() -> Vec<String> {
    vec![
        "--install".to_string(),
        FALLBACK_STORE_DISTRO.to_string(),
        "--name".to_string(),
        DISTRO_NAME.to_string(),
        "--no-launch".to_string(),
    ]
}

/// `wsl --unregister Cowork` — uninstall: drop the dedicated distro and its data.
pub fn unregister_args() -> Vec<String> {
    vec!["--unregister".to_string(), DISTRO_NAME.to_string()]
}

/// `distro.import_failed` (Internal) — `--import` returned nonzero.
pub fn import_failed_envelope(exit_code: i32) -> Envelope {
    Envelope::new(Code::DistroImportFailed, Stage::Provision)
        .with_context("exitCode", exit_code.to_string())
}

/// `distro.install_failed` (Transient) — the Store `--install` returned nonzero.
pub fn install_failed_envelope(exit_code: i32) -> Envelope {
    Envelope::new(Code::DistroInstallFailed, Stage::Provision)
        .with_context("exitCode", exit_code.to_string())
}

/// `distro.unregister_failed` (Internal) — `--unregister` returned nonzero.
pub fn unregister_failed_envelope(exit_code: i32) -> Envelope {
    Envelope::new(Code::DistroUnregisterFailed, Stage::Provision)
        .with_context("exitCode", exit_code.to_string())
}

/// `distro.rootfs_fetch_failed` (Transient) — the pinned rootfs download failed.
/// `http_status` is 0 when no HTTP response was received (connection failure).
pub fn rootfs_fetch_failed_envelope(url: &str, http_status: u16) -> Envelope {
    Envelope::new(Code::DistroRootfsFetchFailed, Stage::Provision)
        .with_context("url", url)
        .with_context("httpStatus", http_status.to_string())
}

/// `distro.already_exists` (NeedsUserAction) — a distro named `Cowork` is present.
pub fn already_exists_envelope() -> Envelope {
    Envelope::new(Code::DistroAlreadyExists, Stage::Provision).with_context("name", DISTRO_NAME)
}

/// `distro.user_create_failed` (Internal) — firstboot user creation failed.
pub fn user_create_failed_envelope(detail: &str) -> Envelope {
    Envelope::new(Code::DistroUserCreateFailed, Stage::Provision).with_context("detail", detail)
}

/// Verify a downloaded rootfs against the pinned digest. Comparison is
/// ASCII-case-insensitive (hex). A mismatch is `distro.checksum_mismatch`
/// (Internal) — a corruption / MITM signal; the caller must STOP, never fall
/// back to another source.
pub fn verify_checksum(expected_hex: &str, actual_hex: &str) -> Result<(), Envelope> {
    if expected_hex.eq_ignore_ascii_case(actual_hex) {
        Ok(())
    } else {
        Err(
            Envelope::new(Code::DistroChecksumMismatch, Stage::Provision)
                .with_context("expected", expected_hex)
                .with_context("actual", actual_hex),
        )
    }
}
