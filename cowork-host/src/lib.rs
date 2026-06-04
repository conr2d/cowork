//! Cowork host-side setup logic. Tauri-independent so each setup phase's
//! decision logic is unit-testable off-Windows; the `cfg(windows)` parts (real
//! `wsl.exe` / registry / Win32 probes) are the only OS-specific code. The thin
//! Tauri command layer (`src-tauri`) depends on this crate.
//!
//! One module per setup phase. WP3: [`preflight`].

pub mod preflight;
