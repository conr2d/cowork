//! Host auth-status bridge (v0.2 WP4): validate probe inputs and classify guest
//! auth-status results. The pure classification is unit-tested off-Windows; the
//! real guest runner is isolated in `cfg(windows)` glue.

#[cfg(windows)]
mod windows_auth;
#[cfg(windows)]
pub use windows_auth::verify_agent_auth;

use cowork_errors::protocol::AgentAuthStatus;
use cowork_errors::{Code, Envelope, Stage};

use crate::protocol::HostEvent;
use crate::provision::cli_crashed_envelope;

/// Canonical agent ids the probe accepts (matches the guest clap ValueEnum).
pub const KNOWN_AGENTS: [&str; 3] = ["claude", "codex", "antigravity"];

/// `auth.agent_not_found` for an id outside [`KNOWN_AGENTS`] (stale/foreign frontend value).
pub fn validate_agent(agent: &str) -> Result<(), Envelope> {
    if KNOWN_AGENTS.contains(&agent) {
        Ok(())
    } else {
        Err(Envelope::new(Code::AuthAgentNotFound, Stage::Auth).with_context("agent", agent))
    }
}

/// The guest CLI args for the probe: `auth-status --agent <id>`.
pub fn auth_status_args(agent: &str) -> Vec<String> {
    vec![
        "auth-status".to_string(),
        "--agent".to_string(),
        agent.to_string(),
    ]
}

/// Fold a finished probe run into a status: first `AuthStatus` event wins; a guest
/// or protocol error surfaces as-is; no status event at all is a crashed/stale guest
/// (nonzero exit) or a probe fault.
pub fn classify_auth_probe(
    agent: &str,
    events: &[HostEvent],
    exit_code: i32,
) -> Result<AgentAuthStatus, Envelope> {
    for event in events {
        match event {
            HostEvent::AuthStatus { status, .. } => return Ok(*status),
            HostEvent::GuestError(env) | HostEvent::ProtocolError(env) => return Err(env.clone()),
            HostEvent::Progress { .. } | HostEvent::Done { .. } | HostEvent::SessionUuid { .. } => {
            }
        }
    }
    if exit_code != 0 {
        Err(cli_crashed_envelope(exit_code, Stage::Auth))
    } else {
        Err(Envelope::new(Code::AuthStatusProbeFailed, Stage::Auth)
            .with_context("agent", agent)
            .with_cause("guest emitted no auth status"))
    }
}
