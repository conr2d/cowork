//! Cowork guest CLI — host-agnostic bootstrap agent run inside WSL.
//!
//! v0.1 WP0 scaffold: this wires the CLI surface and `--version` only.
//! The bootstrap pipeline (apt prereqs → Linuxbrew → mise → Node → agent
//! install, with JSON-lines progress and `~/.cowork/creds` routing) lands in
//! WP6/WP7. The structured error envelope and its `errors.json` codegen land
//! in WP2. This crate is deliberately **host-agnostic**: it must never depend
//! on Windows APIs (a conformance gate enforces this) so it ports as-is to any
//! future host driver.

use clap::{Parser, Subcommand};

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
    /// Print the guest-CLI protocol version used for the host/guest handshake.
    Protocol,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Protocol) => {
            // The host pins this against its own build to detect a stale guest
            // binary (`protocol.version_mismatch`, wired in WP5).
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        None => {
            // Bare invocation is a no-op in the scaffold; clap already handles
            // `--version` / `--help`. WP6 wires the real bootstrap entrypoint.
        }
    }
}
