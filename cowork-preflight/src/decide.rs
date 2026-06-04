use cowork_errors::{Code, Envelope, Stage};
use serde::{Deserialize, Serialize};

use crate::probe::{ARCH_ARM64, ARCH_X64, RawFacts};

/// The 9 preflight checks, in stable display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckId {
    WindowsBuild,
    Arch,
    Virtualization,
    HypervisorConflict,
    Disk,
    Elevation,
    WslPolicy,
    Store,
    ControlledFolderAccess,
}

/// Outcome of one check. `Unknown` = a fact could not be determined; recorded
/// but non-blocking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Fail(Envelope),
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub check: CheckId,
    pub status: CheckStatus,
}

/// The full preflight result. `can_proceed` is true iff NO check failed
/// (Pass/Unknown only). The UI uses each failing envelope's `kind` to decide
/// how to present it (Blocker vs NeedsUserAction).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightReport {
    pub outcomes: Vec<CheckOutcome>,
    pub can_proceed: bool,
}

/// Minimum supported Windows build (Win10 2004 / 19041).
const MIN_BUILD: u32 = 19041;
/// Hard disk floor: 16 GiB.
const MIN_FREE_BYTES: u64 = 16 * 1024 * 1024 * 1024; // 17179869184

pub fn decide(f: &RawFacts) -> PreflightReport {
    let outcomes = vec![
        CheckOutcome {
            check: CheckId::WindowsBuild,
            status: check_build(f),
        },
        CheckOutcome {
            check: CheckId::Arch,
            status: check_arch(f),
        },
        CheckOutcome {
            check: CheckId::Virtualization,
            status: check_virtualization(f),
        },
        CheckOutcome {
            check: CheckId::HypervisorConflict,
            status: check_hypervisor_conflict(f),
        },
        CheckOutcome {
            check: CheckId::Disk,
            status: check_disk(f),
        },
        CheckOutcome {
            check: CheckId::Elevation,
            status: check_elevation(f),
        },
        CheckOutcome {
            check: CheckId::WslPolicy,
            status: check_wsl_policy(f),
        },
        CheckOutcome {
            check: CheckId::Store,
            status: check_store(f),
        },
        CheckOutcome {
            check: CheckId::ControlledFolderAccess,
            status: check_cfa(f),
        },
    ];
    let can_proceed = outcomes
        .iter()
        .all(|o| !matches!(o.status, CheckStatus::Fail(_)));
    PreflightReport {
        outcomes,
        can_proceed,
    }
}

fn check_build(f: &RawFacts) -> CheckStatus {
    match f.build_number {
        Some(b) if b >= MIN_BUILD => CheckStatus::Pass,
        Some(b) => CheckStatus::Fail(
            Envelope::new(Code::PreflightWindowsBuildUnsupported, Stage::Preflight)
                .with_context("build", b.to_string())
                .with_context("minBuild", MIN_BUILD.to_string()),
        ),
        None => CheckStatus::Unknown,
    }
}

fn check_arch(f: &RawFacts) -> CheckStatus {
    if f.arch == ARCH_X64 || f.arch == ARCH_ARM64 {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail(
            Envelope::new(Code::PreflightArchUnsupported, Stage::Preflight)
                .with_context("arch", f.arch.to_string()),
        )
    }
}

fn check_virtualization(f: &RawFacts) -> CheckStatus {
    let hv = f.hypervisor_present == Some(true);
    if f.vm_monitor_mode_extensions == Some(false) && !hv {
        CheckStatus::Fail(Envelope::new(
            Code::PreflightVirtualizationUnsupported,
            Stage::Preflight,
        ))
    } else {
        let virt_ok =
            hv || f.virtualization_firmware_enabled == Some(true) || f.virt_feature_present;
        if virt_ok {
            CheckStatus::Pass
        } else {
            CheckStatus::Fail(Envelope::new(
                Code::PreflightVirtualizationDisabled,
                Stage::Preflight,
            ))
        }
    }
}

fn check_hypervisor_conflict(f: &RawFacts) -> CheckStatus {
    if f.hypervisor_present == Some(false) && !f.known_vmm_services.is_empty() {
        CheckStatus::Fail(
            Envelope::new(Code::PreflightHypervisorConflict, Stage::Preflight).with_context(
                "detail",
                format!(
                    "Third-party VMM detected ({}); the Windows Hypervisor Platform is not active. Update VMware (16+) / VirtualBox (6+) or disable it.",
                    f.known_vmm_services.join(", ")
                ),
            ),
        )
    } else {
        CheckStatus::Pass
    }
}

fn check_disk(f: &RawFacts) -> CheckStatus {
    match f.free_bytes_available {
        Some(b) if b >= MIN_FREE_BYTES => CheckStatus::Pass,
        Some(b) => CheckStatus::Fail(
            Envelope::new(Code::PreflightInsufficientDisk, Stage::Preflight)
                .with_context("requiredBytes", MIN_FREE_BYTES.to_string())
                .with_context("availableBytes", b.to_string()),
        ),
        None => CheckStatus::Unknown,
    }
}

fn check_elevation(f: &RawFacts) -> CheckStatus {
    match f.elevation.elevation_type {
        2 => CheckStatus::Pass,
        3 => CheckStatus::Pass,
        1 => {
            if f.elevation.is_member_filtered {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail(Envelope::new(
                    Code::PreflightElevationUnavailable,
                    Stage::Preflight,
                ))
            }
        }
        _ => CheckStatus::Unknown,
    }
}

fn check_wsl_policy(f: &RawFacts) -> CheckStatus {
    if f.wsl_blocked {
        CheckStatus::Fail(
            Envelope::new(Code::PreflightWslBlockedByPolicy, Stage::Preflight)
                .with_context("detail", "Group Policy AllowWSL=0"),
        )
    } else {
        CheckStatus::Pass
    }
}

fn check_store(f: &RawFacts) -> CheckStatus {
    if f.store_disabled {
        CheckStatus::Fail(Envelope::new(
            Code::PreflightStoreDisabled,
            Stage::Preflight,
        ))
    } else {
        CheckStatus::Pass
    }
}

fn check_cfa(f: &RawFacts) -> CheckStatus {
    match f.cfa_mode {
        Some(1) => CheckStatus::Fail(
            Envelope::new(Code::PreflightControlledFolderAccess, Stage::Preflight).with_context(
                "path",
                f.cfa_protected_path
                    .clone()
                    .unwrap_or_else(|| "%LOCALAPPDATA%".to_string()),
            ),
        ),
        Some(_) => CheckStatus::Pass,
        None => CheckStatus::Unknown,
    }
}
