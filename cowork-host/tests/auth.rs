use cowork_errors::protocol::AgentAuthStatus;
use cowork_errors::{Code, Envelope, Stage};
use cowork_host::auth::{KNOWN_AGENTS, auth_status_args, classify_auth_probe, validate_agent};
use cowork_host::protocol::HostEvent;

fn status(status: AgentAuthStatus) -> HostEvent {
    HostEvent::AuthStatus {
        agent: "claude".to_string(),
        status,
    }
}

#[test]
fn auth_status_args_match_guest_subcommand() {
    assert_eq!(
        auth_status_args("claude"),
        ["auth-status", "--agent", "claude"].map(str::to_string)
    );
}

#[test]
fn validate_agent_accepts_known_ids_and_rejects_unknown() {
    for agent in KNOWN_AGENTS {
        validate_agent(agent).expect("known agent");
    }
    let env = validate_agent("gemini").unwrap_err();
    assert_eq!(env.code, Code::AuthAgentNotFound);
    assert_eq!(env.context.get("agent").map(String::as_str), Some("gemini"));
}

#[test]
fn classify_auth_probe_returns_first_status_among_noise() {
    for expected in [
        AgentAuthStatus::Valid,
        AgentAuthStatus::Missing,
        AgentAuthStatus::Unknown,
    ] {
        let events = vec![
            HostEvent::Progress {
                stage: Stage::Auth,
                step: "x".to_string(),
            },
            status(expected),
            HostEvent::Done { stage: Stage::Auth },
        ];
        assert_eq!(classify_auth_probe("claude", &events, 0).unwrap(), expected);
    }
}

#[test]
fn classify_auth_probe_surfaces_guest_error_before_status() {
    let env = Envelope::new(Code::AuthAgentNotFound, Stage::Auth);
    let events = vec![
        HostEvent::GuestError(env.clone()),
        status(AgentAuthStatus::Valid),
    ];
    assert_eq!(
        classify_auth_probe("claude", &events, 1).unwrap_err().code,
        env.code
    );
}

#[test]
fn no_events_and_nonzero_exit_is_guest_cli_crashed() {
    assert_eq!(
        classify_auth_probe("claude", &[], 2).unwrap_err().code,
        Code::GuestCliCrashed
    );
}

#[test]
fn no_events_and_zero_exit_is_status_probe_failed() {
    assert_eq!(
        classify_auth_probe("claude", &[], 0).unwrap_err().code,
        Code::AuthStatusProbeFailed
    );
}
