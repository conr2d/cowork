//! Progress emission seam. The orchestration emits [`Message`]s through a
//! `&mut dyn ProgressSink`; the real sink serializes one JSON object per line to
//! stdout (the guest→host wire protocol), while tests collect them.

use cowork_errors::protocol::Message;

/// Where the guest emits its JSON-lines protocol.
pub trait ProgressSink {
    /// Emit one protocol message.
    fn emit(&mut self, message: &Message);
}

/// The real sink: one JSON object per line on stdout.
pub struct StdoutSink;

impl ProgressSink for StdoutSink {
    fn emit(&mut self, message: &Message) {
        // A `Message` always serializes (its fields are plain data); on the
        // impossible serialization error there is nothing useful to emit.
        if let Ok(line) = serde_json::to_string(message) {
            println!("{line}");
        }
    }
}
