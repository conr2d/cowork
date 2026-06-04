//! Cowork guest CLI — host-agnostic bootstrap agent run inside WSL.
//!
//! v0.1: the `bootstrap` subcommand performs the toolchain bootstrap (WP6) —
//! apt prerequisites, Linuxbrew, mise, shellrc wiring, an optional pinned Node
//! toolchain, locales, and the default workspace — and the `agent-install`
//! subcommand installs selected coding agents (WP7), emitting the guest→host
//! JSON-lines protocol on stdout. The crate is deliberately **host-agnostic**:
//! it must never depend on Windows APIs (a conformance gate enforces this) so it
//! ports as-is to any future host driver.

mod agent;
mod bootstrap;
mod cmd;
mod sink;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use agent::{Agent, AgentConfig, AgentInstallOutcome, LinuxAgentOps, run_agent_install};
use bootstrap::{BootstrapOutcome, Config, LinuxOps, run_bootstrap};
use sink::StdoutSink;

#[derive(Parser)]
#[command(
    name = "cowork",
    version,
    about = "Cowork guest CLI (runs inside WSL)",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print the wire protocol version used for the host/guest handshake.
    Protocol,
    /// Run the toolchain bootstrap inside the distro (WP6), emitting JSON-lines
    /// progress on stdout.
    Bootstrap,
    /// Install the selected coding agents inside the distro (WP7), emitting
    /// JSON-lines progress on stdout. Pass `--agent` once per agent.
    AgentInstall {
        /// An agent to install (repeatable): claude | codex | antigravity.
        #[arg(long = "agent", required = true)]
        agents: Vec<Agent>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Protocol) => {
            // The host compares this against its own build to detect a stale
            // guest binary (`protocol.version_mismatch`).
            println!("{}", cowork_errors::protocol::PROTOCOL_VERSION);
            ExitCode::SUCCESS
        }
        Some(Command::Bootstrap) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let mut ops = LinuxOps;
            let mut sink = StdoutSink;
            let config = Config { home };
            match run_bootstrap(&mut ops, &mut sink, &config) {
                BootstrapOutcome::Done => ExitCode::SUCCESS,
                BootstrapOutcome::Failed(env) => {
                    // The structured error was already emitted on stdout (the
                    // host acts on that). Leave a human breadcrumb on stderr too
                    // for local debugging; the host discards stderr, so this does
                    // not pollute the protocol.
                    eprintln!("cowork: bootstrap failed ({:?})", env.code);
                    ExitCode::FAILURE
                }
            }
        }
        Some(Command::AgentInstall { agents }) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let mut ops = LinuxAgentOps;
            let mut sink = StdoutSink;
            let config = AgentConfig { home, agents };
            match run_agent_install(&mut ops, &mut sink, &config) {
                AgentInstallOutcome::Done => ExitCode::SUCCESS,
                AgentInstallOutcome::Failed(env) => {
                    // The structured error was already emitted on stdout; leave a
                    // human breadcrumb on stderr (the host discards stderr).
                    eprintln!("cowork: agent install failed ({:?})", env.code);
                    ExitCode::FAILURE
                }
            }
        }
        None => {
            // Bare invocation is a no-op; clap handles `--version` / `--help`.
            ExitCode::SUCCESS
        }
    }
}
