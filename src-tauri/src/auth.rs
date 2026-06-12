//! Thin Tauri command surface for v0.2 agent auth-status probes. Decision logic
//! lives in `cowork-host::auth`; this module only runs blocking WSL work off the
//! async command thread.

use cowork_errors::protocol::AgentAuthStatus;
use cowork_errors::{Code, Envelope, Stage};

#[tauri::command]
pub async fn verify_agent_auth(agent: String) -> Result<AgentAuthStatus, Envelope> {
    tauri::async_runtime::spawn_blocking(move || cowork_host::auth::verify_agent_auth(&agent))
        .await
        .map_err(join_envelope(Stage::Auth))?
}

fn join_envelope(stage: Stage) -> impl FnOnce(tauri::Error) -> Envelope {
    move |e| {
        Envelope::new(Code::InternalUnknown, stage).with_context("detail", format!("join: {e}"))
    }
}
