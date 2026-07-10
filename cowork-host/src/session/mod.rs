//! Host session bridge (v0.2 WP4d): validate inputs and classify guest
//! session/theme probe results. Windows execution glue is isolated behind cfg.

#[cfg(windows)]
mod windows_session;
#[cfg(windows)]
pub use windows_session::{capture_session_uuid, sync_agent_theme};

use cowork_errors::{Code, Envelope, Stage};

use crate::protocol::HostEvent;
use crate::provision::cli_crashed_envelope;

pub fn session_uuid_args(agent: &str, slug: &str, since_ms: u64) -> Vec<String> {
    vec![
        "session-uuid".to_string(),
        "--agent".to_string(),
        agent.to_string(),
        "--slug".to_string(),
        slug.to_string(),
        "--since-ms".to_string(),
        since_ms.to_string(),
    ]
}

pub fn agent_theme_args(theme: &str) -> Vec<String> {
    vec![
        "agent-theme".to_string(),
        "--theme".to_string(),
        theme.to_string(),
    ]
}

/// Canonical agent ids the guest accepts (matches its clap `ValueEnum`).
pub const KNOWN_AGENTS: [&str; 3] = ["claude", "codex", "antigravity"];

/// `auth.agent_not_found` for an id outside [`KNOWN_AGENTS`] (stale/foreign frontend value).
pub fn validate_agent(agent: &str) -> Result<(), Envelope> {
    if KNOWN_AGENTS.contains(&agent) {
        Ok(())
    } else {
        Err(Envelope::new(Code::AuthAgentNotFound, Stage::Auth).with_context("agent", agent))
    }
}

/// `agent.theme_sync_failed` for a theme outside {"light","dark"} (stale frontend value).
pub fn validate_theme(theme: &str) -> Result<(), Envelope> {
    if matches!(theme, "light" | "dark") {
        Ok(())
    } else {
        Err(Envelope::new(Code::AgentThemeSyncFailed, Stage::Workspace)
            .with_context("agent", "antigravity")
            .with_cause("unknown app theme"))
    }
}

pub fn classify_session_uuid(
    agent: &str,
    events: &[HostEvent],
    exit_code: i32,
) -> Result<Option<String>, Envelope> {
    for event in events {
        match event {
            HostEvent::SessionUuid { uuid, .. } => return Ok(uuid.clone()),
            HostEvent::GuestError(env) | HostEvent::ProtocolError(env) => return Err(env.clone()),
            HostEvent::Progress { .. } | HostEvent::Done { .. } => {}
        }
    }
    if exit_code != 0 {
        Err(cli_crashed_envelope(exit_code, Stage::Workspace))
    } else {
        Err(
            Envelope::new(Code::SessionUuidCaptureFailed, Stage::Workspace)
                .with_context("agent", agent)
                .with_cause("guest emitted no session uuid"),
        )
    }
}

pub fn classify_agent_theme(events: &[HostEvent], exit_code: i32) -> Result<(), Envelope> {
    for event in events {
        match event {
            HostEvent::Done { .. } => return Ok(()),
            HostEvent::GuestError(env) | HostEvent::ProtocolError(env) => return Err(env.clone()),
            HostEvent::Progress { .. } | HostEvent::SessionUuid { .. } => {}
        }
    }
    if exit_code != 0 {
        Err(cli_crashed_envelope(exit_code, Stage::Workspace))
    } else {
        Err(Envelope::new(Code::AgentThemeSyncFailed, Stage::Workspace)
            .with_context("agent", "antigravity")
            .with_cause("guest emitted no done"))
    }
}
