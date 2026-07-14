use cowork_errors::{Envelope, Stage};

use crate::provision::run_guest_events;

use super::{
    agent_theme_args, classify_agent_theme, classify_session_check, classify_session_uuid,
    session_check_args, session_uuid_args, validate_agent, validate_theme,
};

pub fn capture_session_uuid(
    agent: &str,
    slug: &str,
    since_ms: u64,
) -> Result<Option<String>, Envelope> {
    validate_agent(agent)?;
    let (events, exit_code) = run_guest_events(
        Stage::Workspace,
        &session_uuid_args(agent, slug, since_ms),
        &mut |_, _| {},
    )?;
    classify_session_uuid(agent, &events, exit_code)
}

pub fn session_check(agent: &str, uuid: &str) -> Result<bool, Envelope> {
    validate_agent(agent)?;
    let (events, exit_code) = run_guest_events(
        Stage::Workspace,
        &session_check_args(agent, uuid),
        &mut |_, _| {},
    )?;
    classify_session_check(&events, exit_code)
}

pub fn sync_agent_theme(theme: &str) -> Result<(), Envelope> {
    validate_theme(theme)?;
    let (events, exit_code) =
        run_guest_events(Stage::Workspace, &agent_theme_args(theme), &mut |_, _| {})?;
    classify_agent_theme(&events, exit_code)
}
