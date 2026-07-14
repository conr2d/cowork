use cowork_errors::{Code, Envelope, Stage};
use cowork_host::protocol::HostEvent;
use cowork_host::session::{
    KNOWN_AGENTS, agent_theme_args, classify_agent_theme, classify_session_check,
    classify_session_uuid, session_check_args, session_uuid_args, validate_agent, validate_theme,
};

fn session_uuid(uuid: Option<&str>) -> HostEvent {
    HostEvent::SessionUuid {
        agent: "codex".to_string(),
        uuid: uuid.map(str::to_string),
    }
}

#[test]
fn session_uuid_args_match_guest_subcommand() {
    assert_eq!(
        session_uuid_args("codex", "app", 123),
        [
            "session-uuid",
            "--agent",
            "codex",
            "--slug",
            "app",
            "--since-ms",
            "123"
        ]
        .map(str::to_string)
    );
}

#[test]
fn agent_theme_args_match_guest_subcommand() {
    assert_eq!(
        agent_theme_args("light"),
        ["agent-theme", "--theme", "light"].map(str::to_string)
    );
}

#[test]
fn session_check_args_match_guest_subcommand() {
    assert_eq!(
        session_check_args("claude", "u1"),
        ["session-check", "--agent", "claude", "--uuid", "u1"].map(str::to_string)
    );
}

#[test]
fn validate_theme_accepts_light_dark_and_rejects_unknown() {
    validate_theme("light").expect("light");
    validate_theme("dark").expect("dark");
    let env = validate_theme("blue").unwrap_err();
    assert_eq!(env.code, Code::AgentThemeSyncFailed);
    assert_eq!(
        env.context.get("agent").map(String::as_str),
        Some("antigravity")
    );
}

#[test]
fn classify_session_uuid_passthrough_some_and_none() {
    assert_eq!(
        classify_session_uuid("codex", &[session_uuid(Some("u1"))], 0)
            .unwrap()
            .as_deref(),
        Some("u1")
    );
    assert_eq!(
        classify_session_uuid("codex", &[session_uuid(None)], 0).unwrap(),
        None
    );
}

#[test]
fn classify_session_uuid_surfaces_guest_error() {
    let env = Envelope::new(Code::WorkspaceInvalidName, Stage::Workspace);
    let events = vec![HostEvent::GuestError(env.clone()), session_uuid(Some("u1"))];
    assert_eq!(
        classify_session_uuid("codex", &events, 1).unwrap_err().code,
        env.code
    );
}

#[test]
fn classify_session_uuid_no_event_nonzero_exit_is_crashed() {
    assert_eq!(
        classify_session_uuid("codex", &[], 2).unwrap_err().code,
        Code::GuestCliCrashed
    );
}

#[test]
fn classify_session_uuid_no_event_zero_exit_is_capture_failed() {
    assert_eq!(
        classify_session_uuid("codex", &[], 0).unwrap_err().code,
        Code::SessionUuidCaptureFailed
    );
}

#[test]
fn classify_session_check_maps_some_and_none_to_bool() {
    assert!(classify_session_check(&[session_uuid(Some("u1"))], 0).unwrap());
    assert!(!classify_session_check(&[session_uuid(None)], 0).unwrap());
}

#[test]
fn classify_session_check_surfaces_guest_error() {
    let env = Envelope::new(Code::WorkspaceInvalidName, Stage::Workspace);
    let events = vec![HostEvent::GuestError(env.clone()), session_uuid(Some("u1"))];
    assert_eq!(
        classify_session_check(&events, 1).unwrap_err().code,
        env.code
    );
}

#[test]
fn classify_agent_theme_done_ok() {
    assert!(
        classify_agent_theme(
            &[HostEvent::Done {
                stage: Stage::Workspace
            }],
            0
        )
        .is_ok()
    );
}

#[test]
fn classify_agent_theme_no_done_zero_exit_is_theme_sync_failed() {
    assert_eq!(
        classify_agent_theme(&[], 0).unwrap_err().code,
        Code::AgentThemeSyncFailed
    );
}

#[test]
fn validate_agent_accepts_every_known_id() {
    for agent in KNOWN_AGENTS {
        assert!(validate_agent(agent).is_ok(), "{agent} must be accepted");
    }
}

#[test]
fn validate_agent_rejects_an_unknown_id() {
    let env = validate_agent("gemini").unwrap_err();
    assert_eq!(env.code, Code::AuthAgentNotFound);
    assert_eq!(env.context.get("agent").map(String::as_str), Some("gemini"));
}
