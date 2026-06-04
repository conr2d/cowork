//! Raw system facts and the probe seam. `RawFacts` is the entire input to the
//! pure decision logic; a probe's only job is to populate it.

use serde::{Deserialize, Serialize};

/// PROCESSOR_ARCHITECTURE_AMD64 (x64).
pub const ARCH_X64: u16 = 9;
/// PROCESSOR_ARCHITECTURE_ARM64.
pub const ARCH_ARM64: u16 = 12;

/// Elevation facts derived from the process token.
/// `elevation_type` is the Win32 TOKEN_ELEVATION_TYPE: 1 = Default (no split
/// token / standard user), 2 = Full (already elevated), 3 = Limited (admin
/// under UAC, can elevate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElevationFacts {
    pub is_member_filtered: bool,
    pub elevation_type: u32,
}

/// Every raw fact the preflight decision needs. `Option::None` means "could not
/// determine" (a probe failure), which the decision logic treats as non-fatal
/// (`CheckStatus::Unknown`) unless stated otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawFacts {
    pub build_number: Option<u32>,
    pub ubr: Option<u32>,
    pub arch: u16,
    pub virt_feature_present: bool,
    pub vm_monitor_mode_extensions: Option<bool>,
    pub virtualization_firmware_enabled: Option<bool>,
    pub hypervisor_present: Option<bool>,
    pub known_vmm_services: Vec<String>,
    pub free_bytes_available: Option<u64>,
    pub elevation: ElevationFacts,
    pub wsl_blocked: bool,
    pub inbox_wsl_blocked: bool,
    pub store_disabled: bool,
    pub cfa_mode: Option<u32>,
    pub cfa_protected_path: Option<String>,
}

/// Gathers all raw facts in one call.
pub trait SystemProbe {
    fn gather(&self) -> RawFacts;
}

/// A probe that returns preset facts (for tests and the decision seam).
pub struct StaticProbe {
    pub facts: RawFacts,
}

impl SystemProbe for StaticProbe {
    fn gather(&self) -> RawFacts {
        self.facts.clone()
    }
}
