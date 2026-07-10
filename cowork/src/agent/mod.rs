//! Agent-install orchestration (WP7): the ordered sequence for the selected
//! agents, emitting the guest→host JSON-lines protocol.
//!
//! For each agent the sequence is: resolve → install if absent (hang-guarded) →
//! verify the resolved binary. An agent the user already has is used as-is.
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
    // 1. resolve: an agent the user (or an earlier run) already installed is used
    //    as-is. Cowork never upgrades an agent it did not install.
    progress(sink, &command::resolve_step(agent));
    let mut bin = resolve_bin(ops, agent);

    // 2. install (hang-guarded) only when it is absent.
    if bin.is_none() {
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
        bin = resolve_bin(ops, agent);
    }

    // 3. verify whatever we ended up with.
    progress(sink, &command::verify_step(agent));
    let Some(bin) = bin else {
        return Err(command::binary_not_found_envelope(
            agent,
            &command::bin_path(agent, home),
        ));
    };
    match ops.run_check(&command::verify_cmd(&bin)) {
        InstallOutcome::Completed { exit_code: 0, .. } => Ok(()),
        InstallOutcome::Completed { .. } | InstallOutcome::TimedOut => {
            Err(command::integrity_check_failed_envelope(agent))
        }
        InstallOutcome::LaunchFailed { .. } => Err(command::binary_not_found_envelope(agent, &bin)),
    }
}

/// `None` when the agent is not on a login shell's PATH. A failed probe (launch
/// failure, timeout, nonzero exit) is "not installed", not an error: the install
/// step that follows is the real test.
fn resolve_bin(ops: &mut dyn AgentOps, agent: Agent) -> Option<String> {
    match ops.run_check(&command::resolve_cmd(agent)) {
        InstallOutcome::Completed {
            exit_code: 0,
            output,
        } => command::parse_resolved_bin(&output),
        _ => None,
    }
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
    use std::collections::{HashMap, VecDeque};

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
        install_fail: HashMap<String, i32>,
        install_timeout: Vec<String>,
        checks_by_program: HashMap<String, VecDeque<InstallOutcome>>,
        installer_runs: Vec<Cmd>,
        checks: Vec<Cmd>,
    }

    impl MockAgentOps {
        fn new() -> Self {
            Self {
                install_fail: HashMap::new(),
                install_timeout: Vec::new(),
                checks_by_program: HashMap::new(),
                installer_runs: Vec::new(),
                checks: Vec::new(),
            }
        }

        fn queue_check(&mut self, program: &str, outcome: InstallOutcome) {
            self.checks_by_program
                .entry(program.to_string())
                .or_default()
                .push_back(outcome);
        }

        fn queue_resolve(&mut self, path: Option<&str>) {
            let outcome = match path {
                Some(path) => InstallOutcome::Completed {
                    exit_code: 0,
                    output: format!("{path}\n"),
                },
                None => InstallOutcome::Completed {
                    exit_code: 1,
                    output: String::new(),
                },
            };
            self.queue_check("bash", outcome);
        }

        fn queue_verify_ok(&mut self, bin: &str) {
            self.queue_check(
                bin,
                InstallOutcome::Completed {
                    exit_code: 0,
                    output: String::new(),
                },
            );
        }

        fn queue_verify_nonzero(&mut self, bin: &str) {
            self.queue_check(
                bin,
                InstallOutcome::Completed {
                    exit_code: 1,
                    output: "bad version".to_string(),
                },
            );
        }

        fn queue_verify_launch_failed(&mut self, bin: &str) {
            self.queue_check(
                bin,
                InstallOutcome::LaunchFailed {
                    detail: "missing".to_string(),
                },
            );
        }
    }

    impl AgentOps for MockAgentOps {
        fn run_installer(&mut self, cmd: &Cmd, _timeout: Duration) -> InstallOutcome {
            self.installer_runs.push(cmd.clone());
            let agent = agent_from_install(cmd);
            if self.install_timeout.iter().any(|id| id == agent.id()) {
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
            self.checks_by_program
                .get_mut(&cmd.program)
                .and_then(VecDeque::pop_front)
                .unwrap_or(InstallOutcome::Completed {
                    exit_code: 0,
                    output: String::new(),
                })
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
        ops.queue_resolve(None);
        ops.queue_resolve(Some("/home/u/.local/bin/claude"));
        ops.queue_verify_ok("/home/u/.local/bin/claude");
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert!(matches!(out, AgentInstallOutcome::Done));

        assert_eq!(sink.events.first().map(|(t, _)| t.as_str()), Some("hello"));
        assert_eq!(sink.events.last().map(|(t, _)| t.as_str()), Some("done"));
        assert_eq!(
            steps(&sink),
            vec!["resolve-claude", "install-claude", "verify-claude"]
        );
        assert_eq!(ops.installer_runs.len(), 1);
    }

    #[test]
    fn multi_agent_runs_all_in_order() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(Some("/home/u/.local/bin/claude"));
        ops.queue_resolve(Some("/home/u/.local/bin/codex"));
        ops.queue_resolve(Some("/home/u/.local/bin/agy"));
        ops.queue_verify_ok("/home/u/.local/bin/claude");
        ops.queue_verify_ok("/home/u/.local/bin/codex");
        ops.queue_verify_ok("/home/u/.local/bin/agy");
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
                "resolve-claude",
                "verify-claude",
                "resolve-codex",
                "verify-codex",
                "resolve-antigravity",
                "verify-antigravity",
            ]
        );
    }

    #[test]
    fn install_nonzero_maps_to_install_failed() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(None);
        ops.install_fail.insert("codex".to_string(), 1);
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Codex]));
        assert_failed_with(out, cowork_errors::Code::AgentInstallFailed);
        assert_eq!(steps(&sink), vec!["resolve-codex", "install-codex"]);
        assert_eq!(ops.checks.len(), 1);
    }

    #[test]
    fn install_timeout_maps_to_installer_hang() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(None);
        ops.install_timeout.push("claude".to_string());
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert_failed_with(out, cowork_errors::Code::AgentInstallerHang);
    }

    #[test]
    fn missing_binary_maps_to_binary_not_found() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(None);
        ops.queue_resolve(None);
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        match out {
            AgentInstallOutcome::Failed(env) => {
                assert_eq!(env.code, cowork_errors::Code::AgentBinaryNotFound);
                assert_eq!(
                    env.context.get("expectedPath").map(String::as_str),
                    Some("/home/u/.local/bin/claude")
                );
            }
            AgentInstallOutcome::Done => panic!("expected Failed, got Done"),
        }
        assert_eq!(
            steps(&sink),
            vec!["resolve-claude", "install-claude", "verify-claude"]
        );
    }

    #[test]
    fn version_nonzero_maps_to_integrity_check_failed() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(Some("/home/u/.local/bin/claude"));
        ops.queue_verify_nonzero("/home/u/.local/bin/claude");
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert_failed_with(out, cowork_errors::Code::AgentIntegrityCheckFailed);
    }

    #[test]
    fn verify_launch_fail_maps_to_binary_not_found() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(Some("/home/u/.local/bin/claude"));
        ops.queue_verify_launch_failed("/home/u/.local/bin/claude");
        let mut sink = CollectingSink::default();
        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));
        assert_failed_with(out, cowork_errors::Code::AgentBinaryNotFound);
    }

    #[test]
    fn already_resolved_agent_skips_installer_and_verifies_resolved_path() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(Some("/opt/bin/claude"));
        ops.queue_verify_ok("/opt/bin/claude");
        let mut sink = CollectingSink::default();

        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Claude]));

        assert!(matches!(out, AgentInstallOutcome::Done));
        assert!(ops.installer_runs.is_empty());
        assert_eq!(steps(&sink), vec!["resolve-claude", "verify-claude"]);
        assert_eq!(ops.checks.len(), 2);
        assert_eq!(ops.checks[1].program, "/opt/bin/claude");
    }

    #[test]
    fn unresolved_agent_installs_then_verifies_second_resolved_path() {
        let mut ops = MockAgentOps::new();
        ops.queue_resolve(None);
        ops.queue_resolve(Some("/home/u/.local/bin/codex"));
        ops.queue_verify_ok("/home/u/.local/bin/codex");
        let mut sink = CollectingSink::default();

        let out = run_agent_install(&mut ops, &mut sink, &config(vec![Agent::Codex]));

        assert!(matches!(out, AgentInstallOutcome::Done));
        assert_eq!(ops.installer_runs.len(), 1);
        assert_eq!(
            steps(&sink),
            vec!["resolve-codex", "install-codex", "verify-codex"]
        );
        assert_eq!(ops.checks.len(), 3);
        assert_eq!(ops.checks[2].program, "/home/u/.local/bin/codex");
    }
}
