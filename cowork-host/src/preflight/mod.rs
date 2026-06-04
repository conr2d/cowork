//! Cowork preflight checks. A probe gathers raw system facts, then the pure
//! decision seam maps those facts to a preflight report.

mod decide;
mod probe;
#[cfg(windows)]
mod windows_probe;

pub use decide::{CheckId, CheckOutcome, CheckStatus, PreflightReport, decide};
pub use probe::{ARCH_ARM64, ARCH_X64, ElevationFacts, RawFacts, StaticProbe, SystemProbe};
#[cfg(windows)]
pub use windows_probe::WindowsProbe;

/// Gather facts through a probe and evaluate the preflight checks.
pub fn run_preflight(probe: &dyn SystemProbe) -> PreflightReport {
    decide(&probe.gather())
}
