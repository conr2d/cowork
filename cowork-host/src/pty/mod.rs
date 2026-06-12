//! Embedded-terminal PTY bridge (WP8). The pure launch-command + tuning-env
//! construction ([`command`]) is unit-tested off-Windows; the cfg(windows)
//! ConPTY session ([`windows_pty`]) is thin glue over `portable-pty`, verified
//! at the WP10 e2e gate. The Tauri command/Channel layer that pumps the session
//! reader to xterm.js lives in `src-tauri`, keeping this crate Tauri-free.
//! The keyed session registry is generic and unit-tested off-Windows.

mod command;
mod registry;
#[cfg(windows)]
mod windows_pty;

pub use command::{
    COLORTERM, PtyCommand, TERM, TERM_PROGRAM, locale_to_lang, pty_bridge_failed_envelope,
    pty_spawn_failed_envelope, terminal_launch,
};
pub use registry::PtyRegistry;
#[cfg(windows)]
pub use windows_pty::WindowsPtySession;
