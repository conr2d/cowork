//! Cowork host-side setup logic. Tauri-independent so each setup phase's
//! decision logic is unit-testable off-Windows; the `cfg(windows)` parts (real
//! `wsl.exe` / registry / Win32 probes) are the only OS-specific code. The thin
//! Tauri command layer (`src-tauri`) depends on this crate.
//!
//! One module per setup phase. WP3: [`preflight`]. WP4: [`wsl`]. WP5:
//! [`protocol`] (guest JSON-lines stream parser) + [`provision`] (distro
//! provisioning). WP8: [`pty`] (embedded-terminal ConPTY bridge).

pub mod preflight;
pub mod protocol;
pub mod provision;
pub mod pty;
pub mod session;
pub mod setup_marker;
pub mod workspace;
pub mod wsl;
