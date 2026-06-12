//! Guest↔host wire protocol (host-agnostic, single source of truth). The guest
//! `cowork` CLI emits one JSON object per line on stdout; the host
//! (`cowork-host`) parses the stream. Both sides share these wire types so the
//! format cannot drift.
//!
//! Compatibility gate: the guest's first line is a [`Message::Hello`] declaring
//! its [`PROTOCOL_VERSION`]; the host compares it to its own and emits
//! `protocol.version_mismatch` on disagreement. Within one protocol version the
//! message shape is fixed — bump [`PROTOCOL_VERSION`] for ANY wire change.

use serde::{Deserialize, Serialize};

use crate::{Code, Envelope, Stage};

/// Wire protocol version. Bump on ANY change to [`Message`]'s shape.
pub const PROTOCOL_VERSION: u32 = 2;

/// One line of the guest→host stream. Internally tagged by `type`.
///
/// NOTE: no `PartialEq`/`Eq` — the `Error` variant carries [`Envelope`], which
/// intentionally implements neither. Consumers destructure + `matches!`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// First line of the stream: the guest declares its protocol version.
    Hello {
        #[serde(rename = "protocolVersion")]
        protocol_version: u32,
    },
    /// Progress for a setup step. `step` is a stable identifier key (the
    /// frontend localizes it) — never localized text.
    Progress { stage: Stage, step: String },
    /// The guest hit an error; it carries a full structured envelope.
    Error { envelope: Envelope },
    /// The guest finished the work for `stage` successfully.
    Done { stage: Stage },
    /// Result of an `auth-status` probe (v0.2 WP4) for `agent`
    /// (canonical lowercase id: "claude" | "codex" | "antigravity").
    AuthStatus {
        agent: String,
        status: AgentAuthStatus,
    },
}

/// Result of an agent auth-status probe (v0.2 WP4). `Valid`/`Missing` reflect
/// *local* credential validity (presence + not expired) — server-side revocation
/// is not detectable here and is absorbed by lazy re-auth on first agent call.
/// `Unknown` = the agent has no local probe (antigravity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentAuthStatus {
    Valid,
    Missing,
    Unknown,
}

/// `protocol.version_mismatch` (Internal) — the guest's declared protocol
/// version differs from the host's. `stage` is the stage the stream belongs to.
pub fn version_mismatch_envelope(stage: Stage, host_version: u32, guest_version: u32) -> Envelope {
    Envelope::new(Code::ProtocolVersionMismatch, stage)
        .with_context("hostVersion", host_version.to_string())
        .with_context("guestVersion", guest_version.to_string())
}

/// `protocol.parse_error` (Internal) — a stream line was not a valid [`Message`].
/// `line` is the offending (already-trimmed) line, stored verbatim for diagnosis.
pub fn parse_error_envelope(stage: Stage, line: &str) -> Envelope {
    Envelope::new(Code::ProtocolParseError, stage).with_context("line", line)
}
