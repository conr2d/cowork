//! Agent-install orchestration (WP7): the ordered sequence for the selected
//! agents, emitting the guest→host JSON-lines protocol.
//!
//! For each agent the sequence is: install (hang-guarded) → verify the binary.
//! Agents keep their own default config paths inside the distro. The first
//! failure emits a structured `Error` envelope and stops. This mirrors
//! [`crate::bootstrap`]: all decision logic is pure and unit-tested against a
//! mock [`AgentOps`]; the real process/filesystem glue is [`LinuxAgentOps`].

mod command;
mod ops;
mod session;
mod theme;

use std::time::Duration;

use cowork_errors::Envelope;
use cowork_errors::Stage;
use cowork_errors::protocol::{Message, PROTOCOL_VERSION};

pub use command::Agent;
pub use ops::{AgentOps, InstallOutcome, LinuxAgentOps};
pub use session::{SessionUuidOutcome, run_session_uuid};
pub use theme::{AgyThemeOutcome, AppTheme, run_agy_theme};

use crate::sink::ProgressSink;

/// What the agent-install run was asked to do.
pub struct AgentConfig {
    /// The invoking user's home directory (resolved from `$HOME` by the caller).
    pub home: String,
    /// The agents to install, in order (host enforces "at least one").
    pub agents: Vec<Agent>,
}

/// Outcome of [`run_agent_install`].
///
/// NOTE: no `PartialEq`/`Eq` — `Failed` carries [`Envelope`].
#[derive(Debug, Clone)]
pub enum AgentInstallOutcome {
    Done,
    Failed(Envelope),
}

/// Hang-guard: an installer exceeding this is killed and reported as
/// `agent.installer_hang`.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);
/// Max chars of captured output attached as an envelope `cause`.
const CAUSE_TAIL: usize = 1500;

/// Run agent install, emitting the JSON-lines protocol through `sink`.
pub fn run_agent_install(
    ops: &mut dyn AgentOps,
    sink: &mut dyn ProgressSink,
    config: &AgentConfig,
) -> AgentInstallOutcome {
    sink.emit(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    });

    for &agent in &config.agents {
        if let Err(env) = install_one(ops, sink, agent, &config.home) {
            sink.emit(&Message::Error {
                envelope: env.clone(),
            });
            return AgentInstallOutcome::Failed(env);
        }
    }

    sink.emit(&Message::Done {
        stage: Stage::AgentInstall,
    });
    AgentInstallOutcome::Done
}

fn install_one(
    ops: &mut dyn AgentOps,
    sink: &mut dyn ProgressSink,
    agent: Agent,
    home: &str,
) -> Result<(), Envelope> {
    // 1. install (hang-guarded).
    progress(sink, &command::install_step(agent));
    match ops.run_installer(&command::install_cmd(agent), INSTALL_TIMEOUT) {
        InstallOutcome::Completed { exit_code: 0, .. } => {}
        InstallOutcome::Completed { exit_code, output } => {
            return Err(command::install_failed_envelope(agent, exit_code)
                .with_cause(&tail(&output, CAUSE_TAIL)));
        }
        InstallOutcome::TimedOut => return Err(command::installer_hang_envelope(agent)),
        InstallOutcome::LaunchFailed { detail } => {
            return Err(command::install_failed_envelope(agent, -1).with_cause(&detail));
        }
    }

    // 2. verify the installed binary.
    progress(sink, &command::verify_step(agent));
    let bin = command::bin_path(agent, home);
    if !ops.path_exists(&bin) {
        return Err(command::binary_not_found_envelope(agent, &bin));
    }
    match ops.run_check(&command::verify_cmd(agent, home)) {
        InstallOutcome::Completed { exit_code: 0, .. } => {}
        InstallOutcome::Completed { .. } | InstallOutcome::TimedOut => {
            return Err(command::integrity_check_failed_envelope(agent));
        }
        InstallOutcome::LaunchFailed { .. } => {
            return Err(command::binary_not_found_envelope(agent, &bin));
        }
    }

    Ok(())
}

fn progress(sink: &mut dyn ProgressSink, step: &str) {
    sink.emit(&Message::Progress {
        stage: Stage::AgentInstall,
        step: step.to_string(),
    });
}

/// Last `n` chars of `s` (char-boundary safe). Copy WP6's `bootstrap::tail`.
fn tail(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_string();
    }
    s.chars().skip(count - n).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    use crate::cmd::Cmd;

    /// Records emitted messages as `(tag, detail)` pairs for order assertions:
    /// `("hello", "")`, `("progress", step)`, `("error", "<code-debug>")`,
    /// `("done", "")`.
    #[derive(Default)]
    struct CollectingSink {
        events: Vec<(String, String)>,
    }

    impl ProgressSink for CollectingSink {
        fn emit(&mut self, message: &Message) {
            let pair = match message {
                Message::Hello { .. } => ("hello".to_string(), String::new()),
                Message::Progress { step, .. } => ("progress".to_string(), step.clone()),
                Message::Error { envelope } => {
                    ("error".to_string(), format!("{:?}", envelope.code))
                }
                Message::Done { .. } => ("done".to_string(), String::new()),
                Message::SessionUuid { .. } => ("session_uuid".to_string(), String::new()),
            };
            self.events.push(pair);
        }
    }

    struct MockAgentOps {
        installed: HashSet<String>,
        install_fail: HashMap<String, i32>,
        install_timeout: HashSet<String>,
        verify_fail: HashSet<String>,
        verify_launch_fail: HashSet<String>,
        missing_binary: HashSet<String>,
        installer_runs: Vec<Cmd>,
        checks: Vec<Cmd>,
    }

    impl MockAgentOps {
        fn new() -> Self {
            Self {
                installed: HashSet::new(),
                install_fail: HashMap::new(),
                install_timeout: HashSet::new(),
                verify_fail: HashSet::new(),
                verify_launch_fail: HashSet::new(),
                missing_binary: HashSet::new(),
                installer_runs: Vec::new(),
                checks: Vec::new(),
            }
        }
    }

    impl AgentOps for MockAgentOps {
        fn run_installer(&mut self, cmd: &Cmd, _timeout: Duration) -> InstallOutcome {
            self.installer_runs.push(cmd.clone());
            let agent = agent_from_install(cmd);
            if self.install_timeout.contains(agent.id()) {
                return InstallOutcome::TimedOut;
            }
            match self.install_fail.get(agent.id()) {
                Some(&code) => InstallOutcome::Completed {
                    exit_code: code,
                    output: format!("{} failed", agent.id()),
                },
                None => InstallOutcome::Completed {
                    exit_code: 0,
                    output: String::new(),
                },
            }
        }

        fn run_check(&mut self, cmd: &Cmd) -> InstallOutcome {
            self.checks.push(cmd.clone());
            let agent = agent_from_check(cmd);
            if self.verify_launch_fail.contains(agent.id()) {
                return InstallOutcome::LaunchFailed {
                    detail: "missing".to_string(),
                };
            }
            if self.verify_fail.contains(agent.id()) {
                return InstallOutcome::Completed {
                    exit_code: 1,
                    output: "bad version".to_string(),
                };
            }
            InstallOutcome::Completed {
                exit_code: 0,
                output: String::new(),
            }
        }

        fn path_exists(&self, path: &str) -> bool {
            for agent in [Agent::Claude, Agent::Codex, Agent::Antigravity] {
                if path == command::bin_path(agent, "/home/u")
                    && self.missing_binary.contains(agent.id())
                {
                    return false;
                }
            }
            self.installed.contains(path)
        }
    }

    fn agent_from_install(cmd: &Cmd) -> Agent {
        let script = cmd.args.get(1).cloned().unwrap_or_default();
        if script.contains("claude.ai") {
            Agent::Claude
        } else if script.contains("chatgpt.com/codex") {
            Agent::Codex
        } else {
            Agent::Antigravity
        }
    }

    fn agent_from_check(cmd: &Cmd) -> Agent {
        if cmd.program.ends_with("/claude") {
            Agent::Claude
        } else if cmd.program.ends_with("/codex") {
            Agent::Codex
        } else {
            Agent::Antigravity
        }
    }

    fn config(agents: Vec<Agent>) -> AgentConfig {
        AgentConfig {
            home: "/home/u".to_string(),
            agents,
        }
    }

    fn steps(sink: &CollectingSink) -> Vec<String> {
        sink.events
            .iter()
            .filter(|(tag, _)| tag == "progress")
            .map(|(_, step)| step.clone())
            .collect()
    }

    fn assert_failed_with(out: AgentInstallOutcome, expected: cowork_errors::Code) {
        match out {
            AgentInstallOutcome::Failed(env) => assert_eq!(env.code, expected),
            AgentInstallOutcome::Done => panic!("expected Failed, got Done"),
        }
    }

    #[test]
    fn happy_path_single_agent_emits_hello_progress_done() {
        let mut ops = MockAgentOps::new();
        ops.installed
            .insert(command::bin_path(Agent::Claude, "/home/u"));
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert!(matches!(out, AgentInstallOutcome::Done));

        assert_eq!(sink.events.first().map(|(t, _)| t.as_str()), Some("hello"));
        assert_eq!(sink.events.last().map(|(t, _)| t.as_str()), Some("done"));
        assert_eq!(steps(&sink), vec!["install-claude", "verify-claude"]);
        assert_eq!(ops.installer_runs.len(), 1);
    }

    #[test]
    fn multi_agent_runs_all_in_order() {
        let mut ops = MockAgentOps::new();
        for agent in [Agent::Claude, Agent::Codex, Agent::Antigravity] {
            ops.installed.insert(command::bin_path(agent, "/home/u"));
        }
        let mut sink = CollectingSink::default();
        let out = run_agent_install(
            &mut ops,
            &mut sink,
            &config(vec![Agent::Claude, Agent::Codex, Agent::Antigravity]),
        );
        assert!(matches!(out, AgentInstallOutcome::Done));
        assert_eq!(
            steps(&sink),
            vec![
                "install-claude",
                "verify-claude",
                "install-codex",
                "verify-codex",
                "install-antigravity",
                "verify-antigravity",
            ]
        );
    }

    #[test]
    fn install_nonzero_maps_to_install_failed() {
        let mut ops = MockAgentOps::new();
        ops.install_fail.insert("codex".to_string(), 1);
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Codex]));
        assert_failed_with(out, cowork_errors::Code::AgentInstallFailed);
        assert_eq!(steps(&sink), vec!["install-codex"]);
        assert!(ops.checks.is_empty());
    }

    #[test]
    fn install_timeout_maps_to_installer_hang() {
        let mut ops = MockAgentOps::new();
        ops.install_timeout.insert("claude".to_string());
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert_failed_with(out, cowork_errors::Code::AgentInstallerHang);
    }

    #[test]
    fn missing_binary_maps_to_binary_not_found() {
        let mut ops = MockAgentOps::new();
        ops.installed
            .insert(command::bin_path(Agent::Claude, "/home/u"));
        ops.missing_binary.insert("claude".to_string());
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert_failed_with(out, cowork_errors::Code::AgentBinaryNotFound);
        assert_eq!(steps(&sink), vec!["install-claude", "verify-claude"]);
    }

    #[test]
    fn version_nonzero_maps_to_integrity_check_failed() {
        let mut ops = MockAgentOps::new();
        ops.installed
            .insert(command::bin_path(Agent::Claude, "/home/u"));
        ops.verify_fail.insert("claude".to_string());
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert_failed_with(out, cowork_errors::Code::AgentIntegrityCheckFailed);
    }

    #[test]
    fn verify_launch_fail_maps_to_binary_not_found() {
        let mut ops = MockAgentOps::new();
        ops.installed
            .insert(command::bin_path(Agent::Claude, "/home/u"));
        ops.verify_launch_fail.insert("claude".to_string());
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert_failed_with(out, cowork_errors::Code::AgentBinaryNotFound);
    }
}
