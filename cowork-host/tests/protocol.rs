use cowork_errors::protocol::{AgentAuthStatus, Message, PROTOCOL_VERSION};
use cowork_errors::{Code, Envelope, Kind, Stage};
use cowork_host::protocol::{HostEvent, StreamParser, parse_stream};

/// Destructure a `ProtocolError`, assert its code/kind/stage, return its envelope.
fn expect_protocol_error(event: &HostEvent, code: Code, stage: Stage) -> &Envelope {
    let HostEvent::ProtocolError(env) = event else {
        panic!("expected ProtocolError, got {event:?}");
    };
    assert_eq!(env.code, code);
    assert_eq!(env.kind, Kind::Internal);
    assert_eq!(env.stage, stage);
    env
}

#[test]
fn hello_matching_version_is_consumed() {
    let mut parser = StreamParser::new(Stage::Provision);
    let line = format!(r#"{{"type":"hello","protocolVersion":{PROTOCOL_VERSION}}}"#);
    assert!(parser.push_line(&line).is_none());
}

#[test]
fn hello_version_mismatch_emits_envelope() {
    let mut parser = StreamParser::new(Stage::Provision);
    let guest = PROTOCOL_VERSION + 41;
    let line = format!(r#"{{"type":"hello","protocolVersion":{guest}}}"#);
    let event = parser.push_line(&line).expect("mismatch yields an event");
    let env = expect_protocol_error(&event, Code::ProtocolVersionMismatch, Stage::Provision);

    let host_str = PROTOCOL_VERSION.to_string();
    let guest_str = guest.to_string();
    assert_eq!(env.context.get("hostVersion"), Some(&host_str));
    assert_eq!(env.context.get("guestVersion"), Some(&guest_str));
}

#[test]
fn progress_message_parses() {
    let mut parser = StreamParser::new(Stage::Toolchain);
    let event = parser
        .push_line(r#"{"type":"progress","stage":"toolchain","step":"brew_install"}"#)
        .expect("progress yields an event");
    let HostEvent::Progress { stage, step } = event else {
        panic!("expected Progress, got {event:?}");
    };
    assert_eq!(stage, Stage::Toolchain);
    assert_eq!(step, "brew_install");
}

#[test]
fn error_message_carries_envelope() {
    let mut parser = StreamParser::new(Stage::Toolchain);
    let line = r#"{"type":"error","envelope":{"code":"toolchain.brew_install_failed","kind":"Transient","stage":"toolchain","context":{"exitCode":"1"}}}"#;
    let event = parser.push_line(line).expect("error yields an event");
    let HostEvent::GuestError(env) = event else {
        panic!("expected GuestError, got {event:?}");
    };
    assert_eq!(env.code, Code::ToolchainBrewInstallFailed);
    assert_eq!(env.kind, Kind::Transient);
    assert_eq!(env.stage, Stage::Toolchain);
    assert_eq!(env.context.get("exitCode").map(String::as_str), Some("1"));
}

#[test]
fn done_message_parses() {
    let mut parser = StreamParser::new(Stage::Provision);
    let event = parser
        .push_line(r#"{"type":"done","stage":"provision"}"#)
        .expect("done yields an event");
    let HostEvent::Done { stage } = event else {
        panic!("expected Done, got {event:?}");
    };
    assert_eq!(stage, Stage::Provision);
}

#[test]
fn auth_status_message_parses() {
    let mut parser = StreamParser::new(Stage::Auth);
    let event = parser
        .push_line(r#"{"type":"auth_status","agent":"claude","status":"Valid"}"#)
        .expect("auth status yields an event");
    let HostEvent::AuthStatus { agent, status } = event else {
        panic!("expected AuthStatus, got {event:?}");
    };
    assert_eq!(agent, "claude");
    assert_eq!(status, AgentAuthStatus::Valid);
}

#[test]
fn blank_lines_are_ignored() {
    let mut parser = StreamParser::new(Stage::Provision);
    assert!(parser.push_line("").is_none());
    assert!(parser.push_line("   ").is_none());
    assert!(parser.push_line("\t").is_none());
}

#[test]
fn garbage_line_is_parse_error() {
    let mut parser = StreamParser::new(Stage::Provision);
    let event = parser
        .push_line("{ not json")
        .expect("garbage yields an event");
    let env = expect_protocol_error(&event, Code::ProtocolParseError, Stage::Provision);
    assert_eq!(
        env.context.get("line").map(String::as_str),
        Some("{ not json")
    );
}

#[test]
fn unknown_message_type_is_parse_error() {
    let mut parser = StreamParser::new(Stage::Provision);
    let event = parser
        .push_line(r#"{"type":"bogus","x":1}"#)
        .expect("unknown type yields an event");
    expect_protocol_error(&event, Code::ProtocolParseError, Stage::Provision);
}

#[test]
fn parse_stream_happy_path_consumes_hello() {
    let hello = format!(r#"{{"type":"hello","protocolVersion":{PROTOCOL_VERSION}}}"#);
    let lines = [
        hello.as_str(),
        r#"{"type":"progress","stage":"provision","step":"import_rootfs"}"#,
        r#"{"type":"progress","stage":"provision","step":"create_user"}"#,
        r#"{"type":"done","stage":"provision"}"#,
    ];
    let stream = lines.join("\n");
    let events = parse_stream(Stage::Provision, &stream);
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], HostEvent::Progress { .. }));
    assert!(matches!(events[1], HostEvent::Progress { .. }));
    assert!(matches!(events[2], HostEvent::Done { .. }));
}

#[test]
fn parse_stream_surfaces_version_mismatch_and_parse_error() {
    let guest = PROTOCOL_VERSION + 7;
    let hello = format!(r#"{{"type":"hello","protocolVersion":{guest}}}"#);
    let lines = [
        hello.as_str(),
        "garbage line not json",
        r#"{"type":"progress","stage":"provision","step":"import_rootfs"}"#,
    ];
    let stream = lines.join("\n");
    let events = parse_stream(Stage::Provision, &stream);
    assert_eq!(events.len(), 3);
    expect_protocol_error(&events[0], Code::ProtocolVersionMismatch, Stage::Provision);
    expect_protocol_error(&events[1], Code::ProtocolParseError, Stage::Provision);
    assert!(matches!(events[2], HostEvent::Progress { .. }));
}

#[test]
fn hello_serializes_with_camel_case_field() {
    let json = serde_json::to_string(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    })
    .expect("serialize hello");
    assert!(json.contains(r#""type":"hello""#), "got {json}");
    assert!(json.contains(r#""protocolVersion""#), "got {json}");
}

#[test]
fn progress_serializes_with_snake_case_tag() {
    let json = serde_json::to_string(&Message::Progress {
        stage: Stage::Toolchain,
        step: "brew_install".to_string(),
    })
    .expect("serialize progress");
    assert!(json.contains(r#""type":"progress""#), "got {json}");
    assert!(json.contains(r#""stage":"toolchain""#), "got {json}");
}

#[test]
fn auth_status_serializes_with_snake_case_tag_and_pascal_status() {
    let json = serde_json::to_string(&Message::AuthStatus {
        agent: "codex".to_string(),
        status: AgentAuthStatus::Missing,
    })
    .expect("serialize auth status");
    assert!(json.contains(r#""type":"auth_status""#), "got {json}");
    assert!(json.contains(r#""status":"Missing""#), "got {json}");
}
