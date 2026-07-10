//! The side-effect seam for agent install. The pure orchestration in `super`
//! drives a `&mut dyn AgentOps`; the real Linux impl ([`LinuxAgentOps`]) runs
//! installers (with a hang-guard timeout, stdin closed) and touches the
//! filesystem, while tests substitute a mock. Like
//! [`crate::bootstrap::ops::LinuxOps`], [`LinuxAgentOps`] is thin glue verified
//! at the WP10 e2e gate, not by unit tests.

use std::io::Read;
use std::os::unix::process::CommandExt;
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
}

/// The real Linux implementation (runs inside the WSL distro).
pub struct LinuxAgentOps;

fn run_with_optional_timeout(cmd: &Cmd, timeout: Option<Duration>) -> InstallOutcome {
    let mut command = Command::new(&cmd.program);
    command
        .args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Put the installer in its own process group (leader pid == child pid) so
        // we can later signal the whole group, not just the direct child.
        .process_group(0);
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
    let pid = child.id();

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
                            break true;
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => break true,
                }
            }
        }
    };

    // Kill the whole process group before joining the drain threads. An installer
    // can leave a grandchild (a stray daemon) that inherited the stdout/stderr
    // pipes; that holder keeps the pipes open, so the drain threads' read never
    // sees EOF and the join below would otherwise block forever — defeating the
    // timeout (observed with codex's installer under real WSL2). Signalling the
    // group (a negative pid) reaps such strays whether we timed out or the direct
    // child already exited; with no strays the group is already empty and the
    // signal is a harmless no-op. The v0.1 agent installers do not daemonize at
    // install time, so nothing we intend to keep is killed.
    kill_process_group(pid);
    let _ = child.wait();

    let mut output = out_handle.join().unwrap_or_default();
    output.push_str(&err_handle.join().unwrap_or_default());

    if timed_out {
        return InstallOutcome::TimedOut;
    }
    // The child has exited; obtain its (cached) status without blocking on pipes.
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

/// SIGKILL an entire process group. A negative pid targets the group whose id is
/// `pid`; the installer was spawned as its own group leader (`process_group(0)`),
/// so this reaps any grandchild it left holding the output pipes. See the call site.
fn kill_process_group(pid: u32) {
    // `pid` comes from `Child::id()` and is a valid, in-range Linux PID.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // A child that itself runs past the limit must be timed out and killed, and
    // the call must return shortly after the limit (not hang).
    #[test]
    fn timeout_kills_a_slow_child() {
        let cmd = Cmd::new("sleep", &["60"]);
        let start = Instant::now();
        let outcome = run_with_optional_timeout(&cmd, Some(Duration::from_secs(2)));
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "must return shortly after the timeout, not hang"
        );
        assert!(matches!(outcome, InstallOutcome::TimedOut));
    }

    // The regression: the direct child exits immediately but backgrounds a process
    // that inherits the stdout pipe and outlives it. Before the process-group kill,
    // the stdout drain thread never saw EOF and join() hung until the stray exited.
    // The call must now return promptly with the child's own exit status.
    #[test]
    fn does_not_hang_when_a_grandchild_holds_the_pipe() {
        let cmd = Cmd::new("sh", &["-c", "sleep 60 & exit 0"]);
        let start = Instant::now();
        let outcome = run_with_optional_timeout(&cmd, Some(Duration::from_secs(5)));
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "must not hang on the pipe held by the backgrounded process"
        );
        assert!(matches!(
            outcome,
            InstallOutcome::Completed { exit_code: 0, .. }
        ));
    }
}
