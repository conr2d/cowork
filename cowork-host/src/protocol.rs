//! Host-side parser for the guest JSON-lines stream. Wraps the shared wire
//! types in [`cowork_errors::protocol`] with the host's compatibility check and
//! folds malformed lines / version skew into `protocol.*` envelopes.
//!
//! Pure and Tauri-independent: it consumes `&str` lines (the `cfg(windows)` PTY
//! plumbing that produces those lines lives elsewhere), so the state machine is
//! fully unit-testable off-Windows.

use cowork_errors::protocol::{
    AgentAuthStatus, Message, PROTOCOL_VERSION, parse_error_envelope, version_mismatch_envelope,
};
use cowork_errors::{Envelope, Stage};

/// What the host should act on for one consumed line. A blank line and a
/// matching `Hello` handshake produce no event (`None` from
/// [`StreamParser::push_line`]).
///
/// NOTE: no `PartialEq`/`Eq` — `GuestError`/`ProtocolError` carry [`Envelope`].
/// Consumers destructure + `matches!`.
#[derive(Debug, Clone)]
pub enum HostEvent {
    /// A guest progress update for `stage`/`step`.
    Progress { stage: Stage, step: String },
    /// The guest emitted a structured error envelope.
    GuestError(Envelope),
    /// The guest reported `stage` complete.
    Done { stage: Stage },
    /// The guest reported an agent auth-status probe result (v0.2 WP4).
    AuthStatus {
        agent: String,
        status: AgentAuthStatus,
    },
    /// The host detected a protocol fault (`protocol.version_mismatch` or
    /// `protocol.parse_error`) while reading the stream.
    ProtocolError(Envelope),
}

/// Incremental parser for the guest stream. Construct once per guest invocation
/// with the [`Stage`] the stream belongs to (used to stamp protocol-fault
/// envelopes); it checks guest hellos against the host's [`PROTOCOL_VERSION`].
pub struct StreamParser {
    host_version: u32,
    stage: Stage,
}

impl StreamParser {
    /// New parser stamping protocol faults with `stage`, checking guest hellos
    /// against the host's own [`PROTOCOL_VERSION`].
    pub fn new(stage: Stage) -> Self {
        Self {
            host_version: PROTOCOL_VERSION,
            stage,
        }
    }

    /// Feed one raw line. Returns:
    /// - `None` for a blank line or a matching `Hello` handshake (consumed),
    /// - `Some(HostEvent::ProtocolError(..))` for a version mismatch or an
    ///   unparseable line,
    /// - `Some(HostEvent::{Progress,GuestError,Done})` for a valid message.
    pub fn push_line(&mut self, line: &str) -> Option<HostEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        match serde_json::from_str::<Message>(trimmed) {
            Ok(Message::Hello { protocol_version }) => {
                if protocol_version == self.host_version {
                    None
                } else {
                    Some(HostEvent::ProtocolError(version_mismatch_envelope(
                        self.stage,
                        self.host_version,
                        protocol_version,
                    )))
                }
            }
            Ok(Message::Progress { stage, step }) => Some(HostEvent::Progress { stage, step }),
            Ok(Message::Error { envelope }) => Some(HostEvent::GuestError(envelope)),
            Ok(Message::Done { stage }) => Some(HostEvent::Done { stage }),
            Ok(Message::AuthStatus { agent, status }) => {
                Some(HostEvent::AuthStatus { agent, status })
            }
            Err(_) => Some(HostEvent::ProtocolError(parse_error_envelope(
                self.stage, trimmed,
            ))),
        }
    }
}

/// Parse a whole buffered stream: split `input` on line boundaries and collect
/// every emitted [`HostEvent`] in order. Convenience over
/// [`StreamParser::push_line`] for buffered input and tests.
pub fn parse_stream(stage: Stage, input: &str) -> Vec<HostEvent> {
    let mut parser = StreamParser::new(stage);
    input
        .lines()
        .filter_map(|line| parser.push_line(line))
        .collect()
}
