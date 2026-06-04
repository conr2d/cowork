//! Pure PTY launch-command + terminal-tuning env construction for the embedded
//! terminal (WP8). No portable-pty/Windows dependency lives here, so it
//! unit-tests off-Windows and stays clear of the host/guest gate; the
//! cfg(windows) ConPTY glue is in `super::windows_pty`. The tunings encoded here
//! (truecolor + TERM + locale-matched LANG/LC_ALL) come from the embedded
//! terminal design: correct CJK width + agent TUI colour.

use cowork_errors::{Code, Envelope, Stage};

/// truecolor so agent TUIs (e.g. Claude Code) emit full colour.
pub const COLORTERM: &str = "truecolor";
/// 256-colour terminfo the agents expect.
pub const TERM: &str = "xterm-256color";
/// Program identity some tools key off.
pub const TERM_PROGRAM: &str = "Cowork";

/// A fully-specified host command to run inside a PTY: program + args. Pure
/// data — `super::windows_pty` turns it into a portable-pty `CommandBuilder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PtyCommand {
    pub program: String,
    pub args: Vec<String>,
}

/// Map an app locale tag (`en`/`ja`/`ko`) to the distro UTF-8 locale the
/// toolchain bootstrap generated (WP6). Unknown → the base locale `en_US.UTF-8`.
pub fn locale_to_lang(locale: &str) -> &'static str {
    match locale {
        "ja" => "ja_JP.UTF-8",
        "ko" => "ko_KR.UTF-8",
        _ => "en_US.UTF-8",
    }
}

/// The embedded-terminal launch: an interactive login `bash` in `distro` at
/// `workspace`, with the terminal tuning env set *inside* WSL via `env` (this is
/// deterministic and avoids WSLENV forwarding subtleties). `LANG`/`LC_ALL` come
/// from `locale` so CJK cell width and agent output match the UI language.
pub fn terminal_launch(distro: &str, workspace: &str, locale: &str) -> PtyCommand {
    let lang = locale_to_lang(locale);
    PtyCommand {
        program: "wsl.exe".to_string(),
        args: vec![
            "-d".to_string(),
            distro.to_string(),
            "--cd".to_string(),
            workspace.to_string(),
            "--".to_string(),
            "env".to_string(),
            format!("COLORTERM={COLORTERM}"),
            format!("TERM={TERM}"),
            format!("TERM_PROGRAM={TERM_PROGRAM}"),
            format!("LANG={lang}"),
            format!("LC_ALL={lang}"),
            "bash".to_string(),
            "-li".to_string(),
        ],
    }
}

/// `host.pty_spawn_failed` (Internal) — the ConPTY or child could not be created.
pub fn pty_spawn_failed_envelope(stage: Stage, detail: &str) -> Envelope {
    Envelope::new(Code::HostPtySpawnFailed, stage).with_context("detail", detail)
}

/// `host.pty_bridge_failed` (Internal) — a read/write/resize/kill on a live PTY failed.
pub fn pty_bridge_failed_envelope(stage: Stage, detail: &str) -> Envelope {
    Envelope::new(Code::HostPtyBridgeFailed, stage).with_context("detail", detail)
}
