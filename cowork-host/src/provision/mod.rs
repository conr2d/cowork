//! Distro provisioning (WP5): create the dedicated `Cowork` distro without
//! touching an existing `Ubuntu`, inject the guest CLI, and tear down on
//! uninstall. Pure decision logic lives in these submodules; the
//! `#[cfg(windows)]` `wsl.exe` / download / file-copy impl (added later) is the
//! only OS-specific part.

pub mod list;
