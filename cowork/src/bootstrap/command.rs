//! Pure command/path construction and failure→envelope mapping for the
//! toolchain bootstrap. No I/O and no global state: every function here returns
//! a value computed solely from its inputs, so the whole module is unit-tested
//! off the real environment. The side effects (running a [`Cmd`], filesystem
//! probes/writes) live behind the [`super::ops::BootstrapOps`] seam.

use cowork_errors::{Code, Envelope, Stage};

use crate::cmd::Cmd;

/// Homebrew's official default install prefix on Linux x86_64.
pub const BREW_PREFIX: &str = "/home/linuxbrew/.linuxbrew";
/// Absolute path of the `brew` binary once installed; used as an idempotency probe.
pub const BREW_BIN: &str = "/home/linuxbrew/.linuxbrew/bin/brew";
/// Homebrew's official unattended installer URL.
pub const BREW_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh";
/// mise's official short installer URL (redirects to the hosted install script).
pub const MISE_INSTALL_URL: &str = "https://mise.run";
/// apt prerequisites: Homebrew's Linux build dependencies, plus `locales` (which
/// provides `locale-gen` — not guaranteed present on a minimal base image).
pub const APT_PACKAGES: &[&str] = &[
    "build-essential",
    "procps",
    "curl",
    "file",
    "git",
    "locales",
];
/// Locales generated for the ja/ko/en trilingual product.
pub const LOCALES: &[&str] = &["ja_JP.UTF-8", "ko_KR.UTF-8", "en_US.UTF-8"];

/// Stable step identifiers emitted as `Progress { step }`. The frontend localizes
/// these keys; they are NEVER localized text.
pub mod step {
    pub const APT_PREREQS: &str = "apt-prereqs";
    pub const BREW: &str = "brew-install";
    pub const MISE: &str = "mise-install";
    pub const SHELLRC: &str = "shellrc";
    pub const LOCALES: &str = "locale-gen";
    pub const WORKSPACE: &str = "workspace";
}

/// `sudo apt-get update` — refresh package lists before installing prerequisites.
pub fn apt_update_cmd() -> Cmd {
    Cmd::new("sudo", &["apt-get", "update"])
}

/// `sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y <APT_PACKAGES>`.
/// `sudo env …` keeps the noninteractive setting across sudo's environment reset.
pub fn apt_install_cmd() -> Cmd {
    let mut args = vec![
        "env".to_string(),
        "DEBIAN_FRONTEND=noninteractive".to_string(),
        "apt-get".to_string(),
        "install".to_string(),
        "-y".to_string(),
    ];
    args.extend(APT_PACKAGES.iter().map(|p| p.to_string()));
    Cmd {
        program: "sudo".to_string(),
        args,
        env: Vec::new(),
    }
}

/// Homebrew's unattended install. `NONINTERACTIVE=1` suppresses all prompts;
/// `set -o pipefail` makes a failed `curl` (not just the piped shell) fail the
/// command.
pub fn brew_install_cmd() -> Cmd {
    Cmd::new(
        "bash",
        &["-c", &pipe_install_script(BREW_INSTALL_URL, "bash")],
    )
    .with_env("NONINTERACTIVE", "1")
}

/// mise's standalone install (`curl https://mise.run | sh`).
pub fn mise_install_cmd() -> Cmd {
    Cmd::new(
        "bash",
        &["-c", &pipe_install_script(MISE_INSTALL_URL, "sh")],
    )
}

/// Absolute path of the mise binary for `home`; used as an idempotency probe and
/// to invoke mise without relying on an activated shell.
pub fn mise_bin(home: &str) -> String {
    format!("{home}/.local/bin/mise")
}

/// `sudo locale-gen <LOCALES>`.
pub fn locale_gen_cmd() -> Cmd {
    let mut args = vec!["locale-gen".to_string()];
    args.extend(LOCALES.iter().map(|l| l.to_string()));
    Cmd {
        program: "sudo".to_string(),
        args,
        env: Vec::new(),
    }
}

/// `<home>/.bashrc` — the shellrc the activation lines are appended to.
pub fn shellrc_path(home: &str) -> String {
    format!("{home}/.bashrc")
}

/// The default workspace directory created during bootstrap: `<home>/workspaces/default`.
/// Workspaces live under a `workspaces/` container (one per agent session, created
/// repeatedly); `default` is the single workspace seeded in v0.1. The container shape
/// is the seed for v0.2 per-workspace isolation — siblings are added with no migration.
pub fn workspace_path(home: &str) -> String {
    format!("{home}/workspaces/default")
}

/// The two PATH-activation lines appended to the shellrc (brew first, then mise).
/// Each is matched verbatim for idempotency, so keep the exact text stable.
pub fn shellrc_lines(home: &str) -> Vec<String> {
    vec![
        format!(r#"eval "$({BREW_PREFIX}/bin/brew shellenv)""#),
        format!(r#"eval "$({home}/.local/bin/mise activate bash)""#),
    ]
}

/// `set -o pipefail; curl … | <runner>` — a curl-pipe-shell installer that fails
/// if curl fails. `runner` is `bash` (Homebrew) or `sh` (mise).
fn pipe_install_script(url: &str, runner: &str) -> String {
    format!("set -o pipefail; curl -fsSL {url} | {runner}")
}

// --- failure → envelope constructors (context only; the caller attaches cause) ---

/// `toolchain.prereq_apt_failed` (Transient) — apt update or install failed.
pub fn prereq_apt_failed_envelope() -> Envelope {
    Envelope::new(Code::ToolchainPrereqAptFailed, Stage::Toolchain)
        .with_context("packages", APT_PACKAGES.join(" "))
}

/// `toolchain.brew_install_failed` (Transient).
pub fn brew_install_failed_envelope(exit_code: i32) -> Envelope {
    Envelope::new(Code::ToolchainBrewInstallFailed, Stage::Toolchain)
        .with_context("exitCode", exit_code.to_string())
}

/// `toolchain.mise_install_failed` (Transient).
pub fn mise_install_failed_envelope(exit_code: i32) -> Envelope {
    Envelope::new(Code::ToolchainMiseInstallFailed, Stage::Toolchain)
        .with_context("exitCode", exit_code.to_string())
}

/// `toolchain.shellrc_write_failed` (Internal).
pub fn shellrc_write_failed_envelope(file: &str) -> Envelope {
    Envelope::new(Code::ToolchainShellrcWriteFailed, Stage::Toolchain).with_context("file", file)
}

/// `internal.unknown` (Internal) — used for the locale-gen and workspace steps,
/// which have no dedicated code in the locked v0.1 error model. `detail` is the
/// short diagnostic; it is redacted by `with_context` consumers downstream only
/// via `cause`, so keep `detail` free of secrets (it never is here).
pub fn internal_unknown_envelope(detail: &str) -> Envelope {
    Envelope::new(Code::InternalUnknown, Stage::Toolchain).with_context("detail", detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apt_update_is_sudo_apt_get_update() {
        let c = apt_update_cmd();
        assert_eq!(c.program, "sudo");
        assert_eq!(c.args, vec!["apt-get".to_string(), "update".to_string()]);
        assert!(c.env.is_empty());
    }

    #[test]
    fn apt_install_is_noninteractive_and_lists_all_packages() {
        let c = apt_install_cmd();
        assert_eq!(c.program, "sudo");
        assert_eq!(c.args[0], "env");
        assert_eq!(c.args[1], "DEBIAN_FRONTEND=noninteractive");
        assert_eq!(&c.args[2..5], &["apt-get", "install", "-y"]);
        for pkg in APT_PACKAGES {
            assert!(c.args.iter().any(|a| a == pkg), "missing package {pkg}");
        }
    }

    #[test]
    fn brew_install_pipefails_and_is_noninteractive() {
        let c = brew_install_cmd();
        assert_eq!(c.program, "bash");
        assert_eq!(c.args[0], "-c");
        assert!(c.args[1].contains("set -o pipefail"));
        assert!(c.args[1].contains(BREW_INSTALL_URL));
        assert!(c.args[1].ends_with("| bash"));
        assert_eq!(c.env, vec![("NONINTERACTIVE".to_string(), "1".to_string())]);
    }

    #[test]
    fn mise_install_pipefails_to_sh() {
        let c = mise_install_cmd();
        assert_eq!(c.program, "bash");
        assert!(c.args[1].contains("set -o pipefail"));
        assert!(c.args[1].contains(MISE_INSTALL_URL));
        assert!(c.args[1].ends_with("| sh"));
        assert!(c.env.is_empty());
    }

    #[test]
    fn locale_gen_lists_three_locales() {
        let c = locale_gen_cmd();
        assert_eq!(c.program, "sudo");
        assert_eq!(c.args[0], "locale-gen");
        assert_eq!(&c.args[1..], LOCALES);
    }

    #[test]
    fn shellrc_lines_are_brew_then_mise_with_home() {
        let lines = shellrc_lines("/home/u");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("/home/linuxbrew/.linuxbrew/bin/brew shellenv"));
        assert!(lines[1].contains("/home/u/.local/bin/mise activate bash"));
    }

    #[test]
    fn paths_are_home_relative() {
        assert_eq!(mise_bin("/home/u"), "/home/u/.local/bin/mise");
        assert_eq!(shellrc_path("/home/u"), "/home/u/.bashrc");
        assert_eq!(workspace_path("/home/u"), "/home/u/workspaces/default");
    }

    #[test]
    fn prereq_envelope_carries_packages() {
        let e = prereq_apt_failed_envelope();
        assert_eq!(e.code, Code::ToolchainPrereqAptFailed);
        assert_eq!(
            e.context.get("packages").map(String::as_str),
            Some("build-essential procps curl file git locales")
        );
    }

    #[test]
    fn brew_envelope_carries_exit_code() {
        let e = brew_install_failed_envelope(42);
        assert_eq!(e.code, Code::ToolchainBrewInstallFailed);
        assert_eq!(e.context.get("exitCode").map(String::as_str), Some("42"));
    }

    #[test]
    fn mise_envelope_carries_exit_code() {
        let e = mise_install_failed_envelope(7);
        assert_eq!(e.code, Code::ToolchainMiseInstallFailed);
        assert_eq!(e.context.get("exitCode").map(String::as_str), Some("7"));
    }

    #[test]
    fn shellrc_envelope_carries_file() {
        let e = shellrc_write_failed_envelope("/home/u/.bashrc");
        assert_eq!(e.code, Code::ToolchainShellrcWriteFailed);
        assert_eq!(
            e.context.get("file").map(String::as_str),
            Some("/home/u/.bashrc")
        );
    }

    #[test]
    fn internal_unknown_envelope_carries_detail() {
        let e = internal_unknown_envelope("locale-gen exited 1");
        assert_eq!(e.code, Code::InternalUnknown);
        assert_eq!(
            e.context.get("detail").map(String::as_str),
            Some("locale-gen exited 1")
        );
    }
}
