//! The side-effect seam for the toolchain bootstrap. The pure orchestration in
//! `super` drives a `&mut dyn BootstrapOps`; the real Linux impl ([`LinuxOps`])
//! runs processes and touches the filesystem, while tests substitute a mock.
//! Like the host's `windows_provision`, [`LinuxOps`] is thin glue verified at the
//! WP10 e2e gate, not by unit tests — all decision logic lives in the pure
//! modules.

use std::io::Write;

use crate::cmd::Cmd;

/// Result of running one [`Cmd`].
#[derive(Debug, Clone)]
pub enum ExecOutcome {
    /// The process ran to completion. `output` is combined stdout+stderr.
    Completed { exit_code: i32, output: String },
    /// The process could not be launched (program missing / OS refused).
    LaunchFailed { detail: String },
}

/// The side effects the bootstrap needs. Implemented for real by [`LinuxOps`]
/// and by a mock in `super`'s tests.
pub trait BootstrapOps {
    /// Run `cmd`, capturing its exit code and combined output.
    fn run(&mut self, cmd: &Cmd) -> ExecOutcome;
    /// Whether `path` exists (idempotency probe for brew/mise).
    fn path_exists(&self, path: &str) -> bool;
    /// Read a file to a string, or `None` if it does not exist / cannot be read.
    fn read_to_string(&self, path: &str) -> Option<String>;
    /// Append `line` (a trailing newline is added) to `path`, creating it if
    /// absent. `Err` carries a short diagnostic string.
    fn append_line(&mut self, path: &str, line: &str) -> Result<(), String>;
    /// Overwrite `path` with `contents` (used to strip legacy activation lines).
    /// `Err` carries a short diagnostic string.
    fn write_file(&mut self, path: &str, contents: &str) -> Result<(), String>;
    /// Create `path` and all parents (idempotent, like `mkdir -p`). `Err` carries
    /// a short diagnostic string.
    fn create_dir_all(&mut self, path: &str) -> Result<(), String>;
}

/// The real Linux implementation (runs inside the WSL distro).
pub struct LinuxOps;

impl BootstrapOps for LinuxOps {
    fn run(&mut self, cmd: &Cmd) -> ExecOutcome {
        let mut command = std::process::Command::new(&cmd.program);
        command.args(&cmd.args);
        for (key, value) in &cmd.env {
            command.env(key, value);
        }
        match command.output() {
            Ok(out) => {
                let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
                output.push_str(&String::from_utf8_lossy(&out.stderr));
                ExecOutcome::Completed {
                    exit_code: out.status.code().unwrap_or(-1),
                    output,
                }
            }
            Err(e) => ExecOutcome::LaunchFailed {
                detail: e.to_string(),
            },
        }
    }

    fn path_exists(&self, path: &str) -> bool {
        std::path::Path::new(path).exists()
    }

    fn read_to_string(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn append_line(&mut self, path: &str, line: &str) -> Result<(), String> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{line}").map_err(|e| e.to_string())
    }

    fn write_file(&mut self, path: &str, contents: &str) -> Result<(), String> {
        std::fs::write(path, contents).map_err(|e| e.to_string())
    }

    fn create_dir_all(&mut self, path: &str) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())
    }
}
