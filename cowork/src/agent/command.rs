//! Pure command/path construction and failure→envelope mapping for the
//! agent-install stage (WP7), mirroring [`crate::bootstrap::command`]. No I/O
//! and no global state: the agent name is carried as a context value so the
//! error model is vendor-neutral (a new CLI = a new [`Agent`] variant, no
//! error-model change).

use clap::ValueEnum;

use cowork_errors::{Code, Envelope, Stage};

use crate::cmd::Cmd;

/// A coding agent Cowork can install. Vendor-neutral: adding a CLI is a new
/// variant + its installer URL / runner / binary name; the error model is
/// unchanged because install failures carry the agent id in `context`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
    Antigravity,
}

impl Agent {
    /// Canonical lowercase id, used as the `agent` context value and creds dir.
    pub fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Antigravity => "antigravity",
        }
    }

    /// Installed binary name under `~/.local/bin`.
    pub fn binary(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Antigravity => "agy",
        }
    }

    /// Official installer URL for the agent CLI.
    pub fn installer_url(self) -> &'static str {
        match self {
            Agent::Claude => "https://claude.ai/install.sh",
            Agent::Codex => "https://chatgpt.com/codex/install.sh",
            Agent::Antigravity => "https://antigravity.google/cli/install.sh",
        }
    }

    /// Shell used to run the downloaded installer.
    pub fn installer_runner(self) -> &'static str {
        match self {
            Agent::Codex => "sh",
            Agent::Claude | Agent::Antigravity => "bash",
        }
    }

    /// Environment variable that redirects the agent's config/creds home.
    pub fn creds_env_var(self) -> Option<&'static str> {
        match self {
            Agent::Claude => Some("CLAUDE_CONFIG_DIR"),
            Agent::Codex => Some("CODEX_HOME"),
            Agent::Antigravity => None,
        }
    }
}

/// Absolute installed binary path for `agent`.
pub fn bin_path(agent: Agent, home: &str) -> String {
    format!("{home}/.local/bin/{}", agent.binary())
}

/// Root directory for all routed agent credentials.
pub fn creds_root(home: &str) -> String {
    format!("{home}/.cowork/creds")
}

/// Routed credentials directory for `agent`.
pub fn creds_dir(agent: Agent, home: &str) -> String {
    format!("{}/{}", creds_root(home), agent.id())
}

/// Antigravity config path that reads `credentials.enc`.
pub fn antigravity_link_path(home: &str) -> String {
    format!("{home}/.gemini/antigravity-cli")
}

/// Parent directory for the Antigravity config symlink.
pub fn antigravity_link_parent(home: &str) -> String {
    format!("{home}/.gemini")
}

/// Curl-pipe-shell installer for `agent`, with credentials redirected when supported.
pub fn install_cmd(agent: Agent, home: &str) -> Cmd {
    let script = pipe_install_script(agent.installer_url(), agent.installer_runner());
    let mut cmd = Cmd::new("bash", &["-c", &script]);
    if let Some(var) = agent.creds_env_var() {
        cmd = cmd.with_env(var, &creds_dir(agent, home));
    }
    cmd
}

/// `--version` verification command for the installed agent binary.
pub fn verify_cmd(agent: Agent, home: &str) -> Cmd {
    let mut cmd = Cmd::new(&bin_path(agent, home), &["--version"]);
    if let Some(var) = agent.creds_env_var() {
        cmd = cmd.with_env(var, &creds_dir(agent, home));
    }
    cmd
}

/// Shellrc export line for agents that support an env-based creds redirect.
pub fn creds_export_line(agent: Agent, home: &str) -> Option<String> {
    agent
        .creds_env_var()
        .map(|var| format!(r#"export {var}="{}""#, creds_dir(agent, home)))
}

/// `set -o pipefail; curl … | <runner>` — a curl-pipe-shell installer that fails
/// if curl fails. `runner` is `bash` or `sh`.
fn pipe_install_script(url: &str, runner: &str) -> String {
    format!("set -o pipefail; curl -fsSL {url} | {runner}")
}

// --- failure → envelope constructors (context only; the caller attaches cause) ---

/// `agent.install_failed` — the installer exited nonzero or failed to launch.
pub fn install_failed_envelope(agent: Agent, exit_code: i32) -> Envelope {
    Envelope::new(Code::AgentInstallFailed, Stage::AgentInstall)
        .with_context("agent", agent.id())
        .with_context("exitCode", exit_code.to_string())
}

/// `agent.installer_hang` — the installer exceeded the hang-guard timeout.
pub fn installer_hang_envelope(agent: Agent) -> Envelope {
    Envelope::new(Code::AgentInstallerHang, Stage::AgentInstall).with_context("agent", agent.id())
}

/// `agent.binary_not_found` — the expected post-install binary was absent.
pub fn binary_not_found_envelope(agent: Agent, expected_path: &str) -> Envelope {
    Envelope::new(Code::AgentBinaryNotFound, Stage::AgentInstall)
        .with_context("agent", agent.id())
        .with_context("expectedPath", expected_path)
}

/// `agent.integrity_check_failed` — the installed binary failed `--version`.
pub fn integrity_check_failed_envelope(agent: Agent) -> Envelope {
    Envelope::new(Code::AgentIntegrityCheckFailed, Stage::AgentInstall)
        .with_context("agent", agent.id())
}

/// `internal.unknown` for AgentInstall-stage filesystem/routing failures.
pub fn internal_unknown_envelope(detail: &str) -> Envelope {
    Envelope::new(Code::InternalUnknown, Stage::AgentInstall).with_context("detail", detail)
}

/// Stable install step key for `agent`.
pub fn install_step(agent: Agent) -> String {
    format!("install-{}", agent.id())
}

/// Stable verify step key for `agent`.
pub fn verify_step(agent: Agent) -> String {
    format!("verify-{}", agent.id())
}

/// Stable credential-routing step key for `agent`.
pub fn creds_step(agent: Agent) -> String {
    format!("creds-{}", agent.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_install_cmd_is_curl_pipe_bash() {
        let c = install_cmd(Agent::Claude, "/home/u");
        assert_eq!(c.program, "bash");
        assert_eq!(c.args[0], "-c");
        assert!(c.args[1].contains("set -o pipefail"));
        assert!(c.args[1].contains("https://claude.ai/install.sh"));
        assert!(c.args[1].ends_with("| bash"));
        assert_eq!(
            c.env,
            vec![(
                "CLAUDE_CONFIG_DIR".to_string(),
                "/home/u/.cowork/creds/claude".to_string()
            )]
        );
    }

    #[test]
    fn codex_install_cmd_pipes_to_sh_and_sets_codex_home() {
        let c = install_cmd(Agent::Codex, "/home/u");
        assert!(c.args[1].contains("https://chatgpt.com/codex/install.sh"));
        assert!(c.args[1].ends_with("| sh"));
        assert_eq!(
            c.env,
            vec![(
                "CODEX_HOME".to_string(),
                "/home/u/.cowork/creds/codex".to_string()
            )]
        );
    }

    #[test]
    fn antigravity_install_cmd_has_no_creds_env() {
        let c = install_cmd(Agent::Antigravity, "/home/u");
        assert!(c.args[1].contains("https://antigravity.google/cli/install.sh"));
        assert!(c.args[1].ends_with("| bash"));
        assert!(c.env.is_empty());
    }

    #[test]
    fn verify_cmd_runs_versioned_binary_at_local_bin() {
        for agent in [Agent::Claude, Agent::Codex, Agent::Antigravity] {
            let c = verify_cmd(agent, "/home/u");
            assert_eq!(c.program, bin_path(agent, "/home/u"));
            assert_eq!(c.args, vec!["--version".to_string()]);
            match agent {
                Agent::Claude => assert_eq!(
                    c.env,
                    vec![(
                        "CLAUDE_CONFIG_DIR".to_string(),
                        "/home/u/.cowork/creds/claude".to_string()
                    )]
                ),
                Agent::Codex => assert_eq!(
                    c.env,
                    vec![(
                        "CODEX_HOME".to_string(),
                        "/home/u/.cowork/creds/codex".to_string()
                    )]
                ),
                Agent::Antigravity => assert!(c.env.is_empty()),
            }
        }
    }

    #[test]
    fn bin_paths_are_local_bin() {
        assert_eq!(
            bin_path(Agent::Claude, "/home/u"),
            "/home/u/.local/bin/claude"
        );
        assert_eq!(
            bin_path(Agent::Codex, "/home/u"),
            "/home/u/.local/bin/codex"
        );
        assert_eq!(
            bin_path(Agent::Antigravity, "/home/u"),
            "/home/u/.local/bin/agy"
        );
    }

    #[test]
    fn creds_paths() {
        assert_eq!(creds_root("/home/u"), "/home/u/.cowork/creds");
        assert_eq!(
            creds_dir(Agent::Codex, "/home/u"),
            "/home/u/.cowork/creds/codex"
        );
        assert_eq!(
            antigravity_link_path("/home/u"),
            "/home/u/.gemini/antigravity-cli"
        );
    }

    #[test]
    fn creds_export_line_only_for_env_agents() {
        assert_eq!(
            creds_export_line(Agent::Claude, "/home/u"),
            Some(r#"export CLAUDE_CONFIG_DIR="/home/u/.cowork/creds/claude""#.to_string())
        );
        assert_eq!(
            creds_export_line(Agent::Codex, "/home/u"),
            Some(r#"export CODEX_HOME="/home/u/.cowork/creds/codex""#.to_string())
        );
        assert_eq!(creds_export_line(Agent::Antigravity, "/home/u"), None);
    }

    #[test]
    fn envelopes_carry_agent_context() {
        let e = install_failed_envelope(Agent::Codex, 7);
        assert_eq!(e.code, Code::AgentInstallFailed);
        assert_eq!(e.context.get("agent").map(String::as_str), Some("codex"));
        assert_eq!(e.context.get("exitCode").map(String::as_str), Some("7"));

        let e = binary_not_found_envelope(Agent::Claude, "/x");
        assert_eq!(e.context.get("agent").map(String::as_str), Some("claude"));
        assert_eq!(
            e.context.get("expectedPath").map(String::as_str),
            Some("/x")
        );

        let e = integrity_check_failed_envelope(Agent::Antigravity);
        assert_eq!(
            e.context.get("agent").map(String::as_str),
            Some("antigravity")
        );
    }

    #[test]
    fn step_keys() {
        assert_eq!(install_step(Agent::Claude), "install-claude");
        assert_eq!(verify_step(Agent::Codex), "verify-codex");
        assert_eq!(creds_step(Agent::Antigravity), "creds-antigravity");
    }
}
