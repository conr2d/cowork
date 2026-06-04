//! Cowork error model (host-agnostic). The `Code` enum is generated from the
//! repo-root `errors.json` by build.rs; `Kind`, `Stage`, `Envelope`, and
//! `redact` are defined here. Backend code emits these CODES only — never
//! localized strings (the frontend maps codes to text).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/codes.rs"));

/// How the UI must treat an error. `Cancelled` is user cancellation (no error
/// UI) — it maps to errors.json's `common.cancelled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Blocker,
    NeedsUserAction,
    Transient,
    Internal,
    Cancelled,
}

/// The setup stage an envelope was emitted at (a runtime property of the
/// envelope, not stored per-code in errors.json).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    #[serde(rename = "preflight")]
    Preflight,
    #[serde(rename = "wsl-enable")]
    WslEnable,
    #[serde(rename = "provision")]
    Provision,
    #[serde(rename = "toolchain")]
    Toolchain,
    #[serde(rename = "agent-install")]
    AgentInstall,
    #[serde(rename = "auth")]
    Auth,
    #[serde(rename = "done")]
    Done,
}

/// The one structured signal the backend sends the frontend. Never carries
/// localized text. `cause` is an optional English diagnostic, already redacted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub code: Code,
    pub kind: Kind,
    pub stage: Stage,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

impl Envelope {
    /// Build an envelope; `kind` is taken from the code, `context` empty, no cause.
    pub fn new(code: Code, stage: Stage) -> Self {
        Self {
            code,
            kind: code.kind(),
            stage,
            context: BTreeMap::new(),
            cause: None,
        }
    }

    /// Add one ICU-interpolation context value (camelCase key).
    pub fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Attach a diagnostic cause, REDACTED before storage.
    pub fn with_cause(mut self, raw: &str) -> Self {
        self.cause = Some(redact(raw));
        self
    }
}

/// Strip usernames / home paths / URLs / tokens / OAuth artifacts from a raw
/// diagnostic string before it is logged or shown. Order matters (URL before
/// generic long-token rule). Applied left-to-right.
pub fn redact(input: &str) -> String {
    // (key, replacement). Compiled once.
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        vec![
            (Regex::new(r#"(?i)\bhttps?://[^\s'"]+"#).expect("static redaction regex must compile"), "<url>"),
            (Regex::new(r#"\\\\wsl[^\s'"]*"#).expect("static redaction regex must compile"), "<wsl-path>"),
            (
                Regex::new(r#"(?i)C:\\Users\\[^\\\s'"]+"#).expect("static redaction regex must compile"),
                r"C:\Users\<user>",
            ),
            (Regex::new(r#"/home/[^/\s'"]+"#).expect("static redaction regex must compile"), "/home/<user>"),
            (Regex::new(r#"/Users/[^/\s'"]+"#).expect("static redaction regex must compile"), "/Users/<user>"),
            (
                Regex::new(r#"(?i)\b(token|bearer|authorization|api[_-]?key|access_token|refresh_token|client_secret|code)\b\s*[:=]\s*\S+"#).expect("static redaction regex must compile"),
                "${1}=<redacted>",
            ),
            (Regex::new(r"\b[A-Za-z0-9_\-]{32,}\b").expect("static redaction regex must compile"), "<redacted>"),
        ]
    });
    let mut out = input.to_string();
    for (re, rep) in rules.iter() {
        out = re.replace_all(&out, *rep).into_owned();
    }
    out
}
