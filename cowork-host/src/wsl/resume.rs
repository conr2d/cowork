//! Reboot-resume state. Enabling WSL may require a reboot; before rebooting the
//! host persists this state and arms RunOnce (see `windows_exec`) so
//! `Cowork.exe --resume` relaunches the wizard at the right step. On resume the
//! state is validated — anything unparseable, schema-mismatched, or carrying an
//! unknown field is `host.resume_state_corrupt`, never a silent wrong resume.

use std::path::Path;

use cowork_errors::{Code, Envelope, Stage};
use serde::{Deserialize, Serialize};

/// Bump when the on-disk shape changes. A mismatch is treated as corrupt; we
/// never half-read a foreign/old schema.
pub const RESUME_SCHEMA_VERSION: u32 = 1;

/// Where to resume the wizard after an interrupting reboot. v0.1 only reboots
/// after the WSL-enable step, so there is a single resume point today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResumeStage {
    /// Re-verify WSL after the post-enable reboot, then continue to provisioning.
    WslReady,
}

/// Persisted wizard state across a reboot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeState {
    pub schema_version: u32,
    pub stage: ResumeStage,
    /// Agent slugs the user selected pre-reboot (opaque here; validated by later
    /// WPs). Preserved so the wizard resumes with the same selection.
    pub selected_agents: Vec<String>,
}

impl ResumeState {
    pub fn new(stage: ResumeStage, selected_agents: Vec<String>) -> Self {
        Self {
            schema_version: RESUME_SCHEMA_VERSION,
            stage,
            selected_agents,
        }
    }
}

/// Serialize and write the resume state to `path`.
pub fn save_resume_state(path: &Path, state: &ResumeState) -> Result<(), Envelope> {
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| corrupt(format!("serialize resume state: {e}")))?;
    std::fs::write(path, json)
        .map_err(|e| corrupt(format!("write resume state to {}: {e}", path.display())))?;
    Ok(())
}

/// Read and validate the resume state from `path`. Any read/parse failure,
/// schema-version mismatch, or unknown field yields `host.resume_state_corrupt`.
pub fn load_resume_state(path: &Path) -> Result<ResumeState, Envelope> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| corrupt(format!("read resume state from {}: {e}", path.display())))?;
    let state: ResumeState =
        serde_json::from_str(&raw).map_err(|e| corrupt(format!("parse resume state: {e}")))?;
    if state.schema_version != RESUME_SCHEMA_VERSION {
        return Err(corrupt(format!(
            "schema_version {} != expected {}",
            state.schema_version, RESUME_SCHEMA_VERSION
        )));
    }
    Ok(state)
}

/// Remove the resume state file if present. A missing file is success
/// (idempotent cleanup).
pub fn clear_resume_state(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn corrupt(detail: String) -> Envelope {
    Envelope::new(Code::HostResumeStateCorrupt, Stage::WslEnable).with_context("detail", detail)
}
