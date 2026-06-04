//! The side-effect seam for agent install. The pure orchestration in `super`
//! drives a `&mut dyn AgentOps`; the real Linux impl ([`LinuxAgentOps`]) runs
//! installers (with a hang-guard timeout, stdin closed) and touches the
//! filesystem, while tests substitute a mock. Like
//! [`crate::bootstrap::ops::LinuxOps`], [`LinuxAgentOps`] is thin glue verified
//! at the WP10 e2e gate, not by unit tests.

use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::cmd::Cmd;

/// Result of running one agent-install command.
#[derive(Debug, Clone)]
pub enum InstallOutcome {
    /// Ran to completion. `output` is combined stdout+stderr.
    Completed { exit_code: i32, output: String },
    /// Could not be launched (program missing / OS refused).
    LaunchFailed { detail: String },
    /// Exceeded the hang-guard timeout and was killed.
    TimedOut,
}

pub trait AgentOps {
    /// Run an installer with a hang-guard `timeout` (stdin is closed so it can
    /// never block on a prompt). Returns `TimedOut` if it exceeds `timeout`.
    fn run_installer(&mut self, cmd: &Cmd, timeout: Duration) -> InstallOutcome;
    /// Run a short verification command (e.g. `--version`).
    fn run_check(&mut self, cmd: &Cmd) -> InstallOutcome;
    /// Whether `path` exists (post-install binary probe).
    fn path_exists(&self, path: &str) -> bool;
    /// Read a file to a string, or `None` if it does not exist / cannot be read.
    fn read_to_string(&self, path: &str) -> Option<String>;
    /// Append `line` (a trailing newline is added) to `path`, creating it if
    /// absent. `Err` carries a short diagnostic string.
    fn append_line(&mut self, path: &str, line: &str) -> Result<(), String>;
    /// Create `path` and all parents (idempotent, like `mkdir -p`).
    fn create_dir_all(&mut self, path: &str) -> Result<(), String>;
    /// Create a symlink at `link` pointing to `target`. Idempotent: if `link`
    /// already resolves to `target`, succeed; if `link` exists as something
    /// else, `Err` (never clobber existing data).
    fn symlink(&mut self, target: &str, link: &str) -> Result<(), String>;
}

/// The real Linux implementation (runs inside the WSL distro).
pub struct LinuxAgentOps;

fn run_with_optional_timeout(cmd: &Cmd, timeout: Option<Duration>) -> InstallOutcome {
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &cmd.env {
        command.env(key, value);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return InstallOutcome::LaunchFailed {
                detail: e.to_string(),
            };
        }
    };

    // Drain both pipes on dedicated threads to avoid a full-buffer deadlock.
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_handle = thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });
    let err_handle = thread::spawn(move || {
        let mut buf = String::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_string(&mut buf);
        }
        buf
    });

    let timed_out = match timeout {
        None => {
            let _ = child.wait();
            false
        }
        Some(limit) => {
            let start = Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break false,
                    Ok(None) => {
                        if start.elapsed() >= limit {
                            let _ = child.kill();
                            let _ = child.wait();
                            break true;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break true;
                    }
                }
            }
        }
    };

    let mut output = out_handle.join().unwrap_or_default();
    output.push_str(&err_handle.join().unwrap_or_default());

    if timed_out {
        return InstallOutcome::TimedOut;
    }
    // The child has exited; obtain its status without blocking on pipes again.
    match child.wait() {
        Ok(status) => InstallOutcome::Completed {
            exit_code: status.code().unwrap_or(-1),
            output,
        },
        Err(e) => InstallOutcome::LaunchFailed {
            detail: e.to_string(),
        },
    }
}

impl AgentOps for LinuxAgentOps {
    fn run_installer(&mut self, cmd: &Cmd, timeout: Duration) -> InstallOutcome {
        run_with_optional_timeout(cmd, Some(timeout))
    }

    fn run_check(&mut self, cmd: &Cmd) -> InstallOutcome {
        run_with_optional_timeout(cmd, None)
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

    fn create_dir_all(&mut self, path: &str) -> Result<(), String> {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())
    }

    fn symlink(&mut self, target: &str, link: &str) -> Result<(), String> {
        let link_path = std::path::Path::new(link);
        if link_path.exists() || link_path.is_symlink() {
            match std::fs::read_link(link_path) {
                Ok(existing) if existing == std::path::Path::new(target) => return Ok(()),
                _ => return Err(format!("{link} already exists")),
            }
        }
        std::os::unix::fs::symlink(target, link).map_err(|e| e.to_string())
    }
}
