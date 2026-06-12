//! Agent auth-status orchestration (v0.2 WP4): local credential probes for the
//! selected agent, emitting the guest→host JSON-lines protocol.
//!
//! The decision flow is pure against [`AgentOps`]. Claude exposes
//! `auth status` with a `loggedIn` JSON field, Codex exposes `login status`
//! with exit-code semantics, and Antigravity has no measured local status
//! subcommand, so it reports `Unknown` without side effects.

use std::time::Duration;

use cowork_errors::protocol::{AgentAuthStatus, Message, PROTOCOL_VERSION};
use cowork_errors::{Code, Envelope, Stage};

use crate::cmd::Cmd;
use crate::sink::ProgressSink;

use super::command::{self, Agent};
use super::ops::{AgentOps, InstallOutcome};

const STATUS_TIMEOUT: Duration = Duration::from_secs(30);

pub fn status_cmd(agent: Agent, home: &str) -> Option<Cmd> {
    let args = match agent {
        Agent::Claude => ["auth", "status"],
        Agent::Codex => ["login", "status"],
        Agent::Antigravity => return None,
    };
    let mut cmd = Cmd::new(&command::bin_path(agent, home), &args);
    if let Some(var) = agent.creds_env_var() {
        cmd = cmd.with_env(var, &command::creds_dir(agent, home));
    }
    Some(cmd)
}

fn parse_claude_logged_in(output: &str) -> Option<bool> {
    let start = output.find('{')?;
    let end = output.rfind('}')?;
    if end < start {
        return None;
    }
    let value = serde_json::from_str::<serde_json::Value>(&output[start..=end]).ok()?;
    value.get("loggedIn")?.as_bool()
}

fn classify(agent: Agent, exit_code: i32, output: &str) -> AgentAuthStatus {
    match agent {
        Agent::Claude => match parse_claude_logged_in(output) {
            Some(true) => AgentAuthStatus::Valid,
            Some(false) => AgentAuthStatus::Missing,
            None if exit_code == 0 => AgentAuthStatus::Valid,
            None => AgentAuthStatus::Missing,
        },
        Agent::Codex => {
            if exit_code == 0 {
                AgentAuthStatus::Valid
            } else {
                AgentAuthStatus::Missing
            }
        }
        Agent::Antigravity => AgentAuthStatus::Unknown,
    }
}

fn agent_not_found(agent: Agent, bin: &str) -> Envelope {
    Envelope::new(Code::AuthAgentNotFound, Stage::Auth)
        .with_context("agent", agent.id())
        .with_cause(bin)
}

fn probe_failed(agent: Agent, cause: &str) -> Envelope {
    Envelope::new(Code::AuthStatusProbeFailed, Stage::Auth)
        .with_context("agent", agent.id())
        .with_cause(cause)
}

/// Outcome of [`run_auth_status`]. NOTE: no `PartialEq`/`Eq` — `Failed` carries `Envelope`.
#[derive(Debug, Clone)]
pub enum AuthStatusOutcome {
    Done,
    Failed(Envelope),
}

pub fn run_auth_status(
    ops: &mut dyn AgentOps,
    sink: &mut dyn ProgressSink,
    agent: Agent,
    home: &str,
) -> AuthStatusOutcome {
    sink.emit(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    });

    let Some(cmd) = status_cmd(agent, home) else {
        sink.emit(&Message::AuthStatus {
            agent: agent.id().to_string(),
            status: AgentAuthStatus::Unknown,
        });
        sink.emit(&Message::Done { stage: Stage::Auth });
        return AuthStatusOutcome::Done;
    };

    let bin = command::bin_path(agent, home);
    if !ops.path_exists(&bin) {
        let env = agent_not_found(agent, &bin);
        sink.emit(&Message::Error {
            envelope: env.clone(),
        });
        return AuthStatusOutcome::Failed(env);
    }

    let status = match ops.run_installer(&cmd, STATUS_TIMEOUT) {
        InstallOutcome::Completed { exit_code, output } => classify(agent, exit_code, &output),
        InstallOutcome::TimedOut => {
            let env = probe_failed(agent, "status probe exceeded 30s");
            sink.emit(&Message::Error {
                envelope: env.clone(),
            });
            return AuthStatusOutcome::Failed(env);
        }
        InstallOutcome::LaunchFailed { detail } => {
            let env = probe_failed(agent, &detail);
            sink.emit(&Message::Error {
                envelope: env.clone(),
            });
            return AuthStatusOutcome::Failed(env);
        }
    };

    sink.emit(&Message::AuthStatus {
        agent: agent.id().to_string(),
        status,
    });
    sink.emit(&Message::Done { stage: Stage::Auth });
    AuthStatusOutcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct CollectingSink {
        messages: Vec<Message>,
    }

    impl ProgressSink for CollectingSink {
        fn emit(&mut self, message: &Message) {
            self.messages.push(message.clone());
        }
    }

    struct MockAgentOps {
        path_exists: bool,
        outcomes: Vec<InstallOutcome>,
        runs: Vec<Cmd>,
    }

    impl MockAgentOps {
        fn new(path_exists: bool, outcomes: Vec<InstallOutcome>) -> Self {
            Self {
                path_exists,
                outcomes,
                runs: vec![],
            }
        }
    }

    impl AgentOps for MockAgentOps {
        fn run_installer(&mut self, cmd: &Cmd, _timeout: Duration) -> InstallOutcome {
            self.runs.push(cmd.clone());
            self.outcomes.remove(0)
        }

        fn run_check(&mut self, _cmd: &Cmd) -> InstallOutcome {
            unreachable!("auth status must not run checks")
        }

        fn path_exists(&self, _path: &str) -> bool {
            self.path_exists
        }

        fn read_to_string(&self, _path: &str) -> Option<String> {
            unreachable!("auth status must not read files")
        }

        fn append_line(&mut self, _path: &str, _line: &str) -> Result<(), String> {
            unreachable!("auth status must not write shellrc")
        }

        fn create_dir_all(&mut self, _path: &str) -> Result<(), String> {
            unreachable!("auth status must not create dirs")
        }

        fn symlink(&mut self, _target: &str, _link: &str) -> Result<(), String> {
            unreachable!("auth status must not create symlinks")
        }
    }

    fn completed(exit_code: i32, output: &str) -> InstallOutcome {
        InstallOutcome::Completed {
            exit_code,
            output: output.to_string(),
        }
    }

    fn auth_status(messages: &[Message]) -> AgentAuthStatus {
        messages
            .iter()
            .find_map(|message| match message {
                Message::AuthStatus { status, .. } => Some(*status),
                _ => None,
            })
            .expect("auth status message")
    }

    fn assert_done_messages(messages: &[Message], expected: AgentAuthStatus) {
        assert!(matches!(messages[0], Message::Hello { .. }));
        assert!(matches!(
            messages[1],
            Message::AuthStatus {
                status,
                ..
            } if status == expected
        ));
        assert!(matches!(messages[2], Message::Done { stage: Stage::Auth }));
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn status_cmd_shapes() {
        let claude = status_cmd(Agent::Claude, "/home/u").expect("claude status cmd");
        assert_eq!(claude.program, "/home/u/.local/bin/claude");
        assert_eq!(claude.args, ["auth", "status"].map(str::to_string));
        assert_eq!(
            claude.env,
            vec![(
                "CLAUDE_CONFIG_DIR".to_string(),
                "/home/u/.cowork/creds/claude".to_string()
            )]
        );

        let codex = status_cmd(Agent::Codex, "/home/u").expect("codex status cmd");
        assert_eq!(codex.program, "/home/u/.local/bin/codex");
        assert_eq!(codex.args, ["login", "status"].map(str::to_string));
        assert_eq!(
            codex.env,
            vec![(
                "CODEX_HOME".to_string(),
                "/home/u/.cowork/creds/codex".to_string()
            )]
        );

        assert!(status_cmd(Agent::Antigravity, "/home/u").is_none());
    }

    #[test]
    fn antigravity_reports_unknown_without_ops_calls() {
        struct PanickingOps;
        impl AgentOps for PanickingOps {
            fn run_installer(&mut self, _cmd: &Cmd, _timeout: Duration) -> InstallOutcome {
                unreachable!("antigravity must not run a status command")
            }

            fn run_check(&mut self, _cmd: &Cmd) -> InstallOutcome {
                unreachable!("antigravity must not run checks")
            }

            fn path_exists(&self, _path: &str) -> bool {
                unreachable!("antigravity must not check paths")
            }

            fn read_to_string(&self, _path: &str) -> Option<String> {
                unreachable!("antigravity must not read files")
            }

            fn append_line(&mut self, _path: &str, _line: &str) -> Result<(), String> {
                unreachable!("antigravity must not write files")
            }

            fn create_dir_all(&mut self, _path: &str) -> Result<(), String> {
                unreachable!("antigravity must not create dirs")
            }

            fn symlink(&mut self, _target: &str, _link: &str) -> Result<(), String> {
                unreachable!("antigravity must not create symlinks")
            }
        }

        let mut ops = PanickingOps;
        let mut sink = CollectingSink::default();
        let out = run_auth_status(&mut ops, &mut sink, Agent::Antigravity, "/home/u");
        assert!(matches!(out, AuthStatusOutcome::Done));
        assert_done_messages(&sink.messages, AgentAuthStatus::Unknown);
    }

    #[test]
    fn claude_binary_missing_emits_agent_not_found() {
        let mut ops = MockAgentOps::new(false, vec![]);
        let mut sink = CollectingSink::default();
        let out = run_auth_status(&mut ops, &mut sink, Agent::Claude, "/home/u");
        let AuthStatusOutcome::Failed(env) = out else {
            panic!("expected failure");
        };
        assert_eq!(env.code, Code::AuthAgentNotFound);
        assert_eq!(env.context.get("agent").map(String::as_str), Some("claude"));
        let Message::Error { envelope } = &sink.messages[1] else {
            panic!("expected error message");
        };
        assert_eq!(envelope.code, Code::AuthAgentNotFound);
        assert_eq!(
            envelope.context.get("agent").map(String::as_str),
            Some("claude")
        );
    }

    #[test]
    fn claude_logged_in_reports_valid() {
        let mut ops = MockAgentOps::new(true, vec![completed(0, r#"{"loggedIn": true}"#)]);
        let mut sink = CollectingSink::default();
        let out = run_auth_status(&mut ops, &mut sink, Agent::Claude, "/home/u");
        assert!(matches!(out, AuthStatusOutcome::Done));
        assert_eq!(auth_status(&sink.messages), AgentAuthStatus::Valid);
    }

    #[test]
    fn claude_logged_out_reports_missing() {
        let mut ops = MockAgentOps::new(true, vec![completed(1, r#"{"loggedIn": false}"#)]);
        let mut sink = CollectingSink::default();
        let out = run_auth_status(&mut ops, &mut sink, Agent::Claude, "/home/u");
        assert!(matches!(out, AuthStatusOutcome::Done));
        assert_eq!(auth_status(&sink.messages), AgentAuthStatus::Missing);
    }

    #[test]
    fn claude_json_surrounded_by_noise_still_parses() {
        let mut ops = MockAgentOps::new(
            true,
            vec![completed(1, "WARNING: x\n{ \"loggedIn\": true }\ntrailing")],
        );
        let mut sink = CollectingSink::default();
        let out = run_auth_status(&mut ops, &mut sink, Agent::Claude, "/home/u");
        assert!(matches!(out, AuthStatusOutcome::Done));
        assert_eq!(auth_status(&sink.messages), AgentAuthStatus::Valid);
    }

    #[test]
    fn claude_unparsable_output_falls_back_to_exit_code() {
        for (exit_code, expected) in [(0, AgentAuthStatus::Valid), (1, AgentAuthStatus::Missing)] {
            let mut ops = MockAgentOps::new(true, vec![completed(exit_code, "garbage")]);
            let mut sink = CollectingSink::default();
            let out = run_auth_status(&mut ops, &mut sink, Agent::Claude, "/home/u");
            assert!(matches!(out, AuthStatusOutcome::Done));
            assert_eq!(auth_status(&sink.messages), expected);
        }
    }

    #[test]
    fn codex_classifies_by_exit_code() {
        for (exit_code, output, expected) in [
            (0, "Logged in", AgentAuthStatus::Valid),
            (1, "WARNING: x\nNot logged in", AgentAuthStatus::Missing),
        ] {
            let mut ops = MockAgentOps::new(true, vec![completed(exit_code, output)]);
            let mut sink = CollectingSink::default();
            let out = run_auth_status(&mut ops, &mut sink, Agent::Codex, "/home/u");
            assert!(matches!(out, AuthStatusOutcome::Done));
            assert_eq!(auth_status(&sink.messages), expected);
        }
    }

    #[test]
    fn timeout_maps_to_status_probe_failed() {
        let mut ops = MockAgentOps::new(true, vec![InstallOutcome::TimedOut]);
        let mut sink = CollectingSink::default();
        let out = run_auth_status(&mut ops, &mut sink, Agent::Claude, "/home/u");
        let AuthStatusOutcome::Failed(env) = out else {
            panic!("expected failure");
        };
        assert_eq!(env.code, Code::AuthStatusProbeFailed);
    }
}
