//! Cowork guest CLI — host-agnostic bootstrap agent run inside WSL.
//!
//! v0.1: the `bootstrap` subcommand performs the toolchain bootstrap (WP6) —
//! apt prerequisites, Linuxbrew, mise, shellrc wiring, an optional pinned Node
//! toolchain, locales, and the default workspace — emitting the guest→host
//! JSON-lines protocol on stdout. The crate is deliberately **host-agnostic**:
//! it must never depend on Windows APIs (a conformance gate enforces this) so it
//! ports as-is to any future host driver.

mod bootstrap;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use bootstrap::{BootstrapOutcome, Config, LinuxOps, StdoutSink, run_bootstrap};

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
    Bootstrap {
        /// Install the pinned Node toolchain. Set by the host iff `codex` is
        /// among the selected agents (codex is the only Node-dependent agent).
        #[arg(long = "with-node")]
        with_node: bool,
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
        Some(Command::Bootstrap { with_node }) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let mut ops = LinuxOps;
            let mut sink = StdoutSink;
            let config = Config { home, with_node };
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
        None => {
            // Bare invocation is a no-op; clap handles `--version` / `--help`.
            ExitCode::SUCCESS
        }
    }
}
