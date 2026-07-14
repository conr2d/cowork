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
    /// Canonical lowercase id, used as the `agent` context value.
    pub fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Antigravity => "antigravity",
        }
    }

    /// Installed binary name.
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

    /// Extra environment to run the installer unattended (skip interactive prompts).
    /// codex's installer otherwise prompts `Start Codex now? [y/N]` by reading
    /// `/dev/tty` directly (a closed stdin does not stop it), which hangs a headless
    /// install; `CODEX_NON_INTERACTIVE=1` makes it take the default and continue.
    pub fn installer_unattended_env(self) -> Option<(&'static str, &'static str)> {
        match self {
            Agent::Codex => Some(("CODEX_NON_INTERACTIVE", "1")),
            Agent::Claude | Agent::Antigravity => None,
        }
    }
}

/// Absolute binary path where Cowork's own installer puts `agent`; this is only
/// the path we name when resolution fails after an install.
pub fn bin_path(agent: Agent, home: &str) -> String {
    format!("{home}/.local/bin/{}", agent.binary())
}

/// The agent's own default config/credentials directory.
///
/// Cowork does not redirect credentials. The distro is already the isolation
/// boundary; a second boundary inside it bought nothing, and the env-var exports
/// that implemented the redirect went into `~/.bashrc`, which a non-interactive
/// shell never reads — so it only ever applied to an interactive session anyway.
pub fn config_dir(agent: Agent, home: &str) -> String {
    match agent {
        Agent::Claude => format!("{home}/.claude"),
        Agent::Codex => format!("{home}/.codex"),
        Agent::Antigravity => format!("{home}/.gemini/antigravity-cli"),
    }
}

/// Curl-pipe-shell installer for `agent`.
pub fn install_cmd(agent: Agent) -> Cmd {
    let script = pipe_install_script(agent.installer_url(), agent.installer_runner());
    let mut cmd = Cmd::new("bash", &["-c", &script]);
    if let Some((var, val)) = agent.installer_unattended_env() {
        cmd = cmd.with_env(var, val);
    }
    cmd
}

/// `bash -lc 'command -v <binary>'` — ask a **login** shell where the agent's
/// binary is. A login shell sources `~/.profile`, which puts `~/.local/bin`, the
/// Homebrew prefix and mise's shims on `PATH` (see `bootstrap::command::profile_lines`);
/// this process's own `PATH` has none of them. `command -v` is a shell builtin, so
/// it also honours anything the user put on `PATH` themselves.
pub fn resolve_cmd(agent: Agent) -> Cmd {
    Cmd::new("bash", &["-lc", &format!("command -v {}", agent.binary())])
}

/// The absolute path `command -v` reported, or `None`.
///
/// `output` is the combined stdout+stderr of [`resolve_cmd`], and a login shell may
/// print to either (a `~/.profile` that echoes, a MOTD). So: take the **last**
/// line that is an absolute path. `command -v` prints a bare name for a shell
/// builtin, function or alias — none of which we can exec — so anything that is
/// not absolute is not a hit.
pub fn parse_resolved_bin(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with('/'))
        .map(str::to_string)
}

/// `--version` verification command for the resolved agent binary.
pub fn verify_cmd(bin: &str) -> Cmd {
    Cmd::new(bin, &["--version"])
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

/// Stable install step key for `agent`.
pub fn install_step(agent: Agent) -> String {
    format!("install-{}", agent.id())
}

/// Stable resolve step key for `agent`.
pub fn resolve_step(agent: Agent) -> String {
    format!("resolve-{}", agent.id())
}

/// Stable verify step key for `agent`.
pub fn verify_step(agent: Agent) -> String {
    format!("verify-{}", agent.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_install_cmd_is_curl_pipe_bash() {
        let c = install_cmd(Agent::Claude);
        assert_eq!(c.program, "bash");
        assert_eq!(c.args[0], "-c");
        assert!(c.args[1].contains("set -o pipefail"));
        assert!(c.args[1].contains("https://claude.ai/install.sh"));
        assert!(c.args[1].ends_with("| bash"));
        assert!(c.env.is_empty());
    }

    #[test]
    fn codex_install_cmd_pipes_to_sh_and_sets_non_interactive_env() {
        let c = install_cmd(Agent::Codex);
        assert!(c.args[1].contains("https://chatgpt.com/codex/install.sh"));
        assert!(c.args[1].ends_with("| sh"));
        assert_eq!(
            c.env,
            vec![("CODEX_NON_INTERACTIVE".to_string(), "1".to_string())]
        );
    }

    #[test]
    fn antigravity_install_cmd_has_no_env() {
        let c = install_cmd(Agent::Antigravity);
        assert!(c.args[1].contains("https://antigravity.google/cli/install.sh"));
        assert!(c.args[1].ends_with("| bash"));
        assert!(c.env.is_empty());
    }

    #[test]
    fn only_codex_install_cmd_sets_non_interactive_env() {
        for agent in [Agent::Claude, Agent::Antigravity] {
            let c = install_cmd(agent);
            assert!(!c.env.iter().any(|(key, _)| key == "CODEX_NON_INTERACTIVE"));
        }
    }

    #[test]
    fn resolve_cmd_uses_login_shell_command_v() {
        let c = resolve_cmd(Agent::Claude);
        assert_eq!(c.program, "bash");
        assert_eq!(
            c.args,
            vec!["-lc".to_string(), "command -v claude".to_string()]
        );
        assert!(c.env.is_empty());
    }

    #[test]
    fn parse_resolved_bin_accepts_absolute_path() {
        assert_eq!(
            parse_resolved_bin("/home/u/.local/bin/claude\n"),
            Some("/home/u/.local/bin/claude".to_string())
        );
    }

    #[test]
    fn parse_resolved_bin_rejects_empty_output() {
        assert_eq!(parse_resolved_bin(""), None);
    }

    #[test]
    fn parse_resolved_bin_rejects_bare_builtin_name() {
        assert_eq!(parse_resolved_bin("claude\n"), None);
    }

    #[test]
    fn parse_resolved_bin_ignores_noise_before_answer() {
        assert_eq!(
            parse_resolved_bin("some banner\n/usr/local/bin/codex\n"),
            Some("/usr/local/bin/codex".to_string())
        );
    }

    #[test]
    fn parse_resolved_bin_takes_last_absolute_line() {
        assert_eq!(
            parse_resolved_bin("/first/claude\n/second/claude\n"),
            Some("/second/claude".to_string())
        );
    }

    #[test]
    fn parse_resolved_bin_trims_path_whitespace() {
        assert_eq!(
            parse_resolved_bin("  /home/u/.local/bin/agy  \n"),
            Some("/home/u/.local/bin/agy".to_string())
        );
    }

    #[test]
    fn verify_cmd_runs_versioned_resolved_binary() {
        let c = verify_cmd("/usr/local/bin/claude");
        assert_eq!(c.program, "/usr/local/bin/claude");
        assert_eq!(c.args, vec!["--version".to_string()]);
        assert!(c.env.is_empty());
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
    fn config_dirs_are_agent_defaults() {
        assert_eq!(config_dir(Agent::Claude, "/home/u"), "/home/u/.claude");
        assert_eq!(config_dir(Agent::Codex, "/home/u"), "/home/u/.codex");
        assert_eq!(
            config_dir(Agent::Antigravity, "/home/u"),
            "/home/u/.gemini/antigravity-cli"
        );
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
        assert_eq!(resolve_step(Agent::Claude), "resolve-claude");
        assert_eq!(install_step(Agent::Claude), "install-claude");
        assert_eq!(verify_step(Agent::Codex), "verify-codex");
    }
}
