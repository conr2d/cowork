//! Pure `wsl.exe` argument construction, inbox-`wsl.exe` detection, and the
//! mapping from a completed run's exit code to an error Envelope.

use cowork_errors::{Code, Envelope, Stage};

/// A wsl.exe operation this unit invokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WslOp {
    /// `wsl --install --no-distribution` — enable the WSL components only (we
    /// provision our dedicated distro later, in WP5).
    Install,
    /// `wsl --update` — update the WSL app to a modern version.
    Update,
    /// `wsl --version` — probe the installed WSL app version.
    Version,
}

impl WslOp {
    pub fn args(&self) -> Vec<String> {
        match self {
            WslOp::Install => vec!["--install".to_string(), "--no-distribution".to_string()],
            WslOp::Update => vec!["--update".to_string()],
            WslOp::Version => vec!["--version".to_string()],
        }
    }

    /// Install/Update modify the system and must run elevated (UAC). Version is
    /// a read-only probe.
    pub fn needs_elevation(&self) -> bool {
        matches!(self, WslOp::Install | WslOp::Update)
    }
}

/// Map a *nonzero* completed run to its Transient failure envelope. Returns
/// `None` for `WslOp::Version` (a failing version probe is not itself an error;
/// the orchestration interprets it as "absent/inbox").
pub fn run_failure_envelope(op: WslOp, exit_code: i32) -> Option<Envelope> {
    let code = match op {
        WslOp::Install => Code::WslInstallFailed,
        WslOp::Update => Code::WslUpdateFailed,
        WslOp::Version => return None,
    };
    Some(Envelope::new(code, Stage::WslEnable).with_context("exitCode", exit_code.to_string()))
}

/// `wsl.update_unsupported_inbox` (NeedsUserAction).
pub fn inbox_unsupported_envelope() -> Envelope {
    Envelope::new(Code::WslUpdateUnsupportedInbox, Stage::WslEnable)
}

/// Heuristic: the build-19041 inbox `wsl.exe` predates modern flags
/// (`--update`, `--version`); invoking them yields a nonzero exit plus
/// usage / "invalid option" text instead of acting. Detect that signature so
/// the wizard routes the user to a Store/MSI update rather than retrying a
/// command the inbox binary cannot understand.
///
/// NOTE (known-fragile): these markers are English and best-effort; the
/// authoritative validation is the WP10 clean-room. The orchestration only
/// consults this on the captured (non-elevated) `--version` probe output.
pub fn is_inbox_unsupported(exit_code: i32, output: &str) -> bool {
    if exit_code == 0 {
        return false;
    }
    let lower = output.to_lowercase();
    INBOX_MARKERS.iter().any(|marker| lower.contains(marker))
}

const INBOX_MARKERS: &[&str] = &[
    "invalid command line option",
    "usage: wsl",
    "the requested operation could not be completed",
];
