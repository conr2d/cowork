//! Thin Tauri command surface for v0.2 session UUID capture and agy theme sync.
//! Decision logic lives in `cowork-host::session`; this module only runs
//! blocking WSL work off the async command thread.

use cowork_errors::{Code, Envelope, Stage};

#[tauri::command]
pub async fn capture_session_uuid(
    agent: String,
    slug: String,
    since_ms: u64,
) -> Result<Option<String>, Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        cowork_host::session::capture_session_uuid(&agent, &slug, since_ms)
    })
    .await
    .map_err(join_envelope(Stage::Workspace))?
}

#[tauri::command]
pub async fn session_check(agent: String, uuid: String) -> Result<bool, Envelope> {
    tauri::async_runtime::spawn_blocking(move || cowork_host::session::session_check(&agent, &uuid))
        .await
        .map_err(join_envelope(Stage::Workspace))?
}

#[tauri::command]
pub async fn agent_theme_sync(theme: String) -> Result<(), Envelope> {
    tauri::async_runtime::spawn_blocking(move || cowork_host::session::sync_agent_theme(&theme))
        .await
        .map_err(join_envelope(Stage::Workspace))?
}

fn join_envelope(stage: Stage) -> impl FnOnce(tauri::Error) -> Envelope {
    move |e| {
        Envelope::new(Code::InternalUnknown, stage).with_context("detail", format!("join: {e}"))
    }
}
