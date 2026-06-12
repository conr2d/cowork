use cowork_errors::protocol::AgentAuthStatus;
use cowork_errors::{Envelope, Stage};

use crate::provision::run_guest_events;

use super::{auth_status_args, classify_auth_probe, validate_agent};

/// Probe `agent`'s local credential validity via the injected guest CLI.
pub fn verify_agent_auth(agent: &str) -> Result<AgentAuthStatus, Envelope> {
    validate_agent(agent)?;
    let (events, exit_code) =
        run_guest_events(Stage::Auth, &auth_status_args(agent), &mut |_, _| {})?;
    classify_auth_probe(agent, &events, exit_code)
}
