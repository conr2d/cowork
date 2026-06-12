//! Thin Tauri command surface for v0.2 workspace metadata and guest filesystem
//! operations. Decision logic lives in `cowork-host::workspace`; this module
//! only resolves the per-user metadata path and runs blocking filesystem/WSL
//! work off the async command thread.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use cowork_errors::{Code, Envelope, Stage};
use cowork_host::provision::{DISTRO_NAME, WSL_USER};
use cowork_host::workspace::metadata::MetadataStore;
use cowork_host::workspace::{
    CreateRequest, WindowsWorkspaceOps, WorkspaceMeta, WorkspacePatch, create_workspace,
    delete_workspace, list_workspaces, open_files_target, slug, update_workspace,
};

fn metadata_path() -> Result<PathBuf, Envelope> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        Envelope::new(Code::WorkspaceMetadataIoFailed, Stage::Workspace)
            .with_context("op", "read")
            .with_cause("LOCALAPPDATA is not set")
    })?;
    let mut dir = PathBuf::from(base);
    dir.push("Cowork");
    std::fs::create_dir_all(&dir).map_err(|e| {
        Envelope::new(Code::WorkspaceMetadataIoFailed, Stage::Workspace)
            .with_context("op", "write")
            .with_cause(&format!("create {}: {e}", dir.display()))
    })?;
    dir.push("workspaces.json");
    Ok(dir)
}

#[tauri::command]
pub async fn workspace_create(
    name: String,
    default_agent: String,
    preset: String,
) -> Result<WorkspaceMeta, Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let store = MetadataStore::new(metadata_path()?);
        let req = CreateRequest {
            name,
            default_agent,
            preset,
            now_ms,
        };
        create_workspace(&WindowsWorkspaceOps, &store, &req)
    })
    .await
    .map_err(join_envelope(Stage::Workspace))?
}

#[tauri::command]
pub async fn workspace_list() -> Result<Vec<WorkspaceMeta>, Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = MetadataStore::new(metadata_path()?);
        list_workspaces(&store)
    })
    .await
    .map_err(join_envelope(Stage::Workspace))?
}

#[tauri::command]
pub async fn workspace_update(
    slug: String,
    patch: WorkspacePatch,
) -> Result<WorkspaceMeta, Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = MetadataStore::new(metadata_path()?);
        update_workspace(&store, &slug, &patch)
    })
    .await
    .map_err(join_envelope(Stage::Workspace))?
}

#[tauri::command]
pub async fn workspace_delete(slug: String) -> Result<(), Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = MetadataStore::new(metadata_path()?);
        delete_workspace(&WindowsWorkspaceOps, &store, &slug)
    })
    .await
    .map_err(join_envelope(Stage::Workspace))?
}

#[tauri::command]
pub async fn workspace_slug_preview(name: String) -> Result<String, Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = MetadataStore::new(metadata_path()?);
        let all = store.load()?;
        let existing = all.iter().map(|m| m.slug.clone()).collect::<Vec<_>>();
        slug::slug_from_name(&name, &existing)
    })
    .await
    .map_err(join_envelope(Stage::Workspace))?
}

/// Open the workspace's `files/` directory in Windows Explorer over the
/// `\\wsl.localhost` share (accessing the share starts the distro on demand).
#[tauri::command]
pub async fn workspace_open_files(slug: String) -> Result<(), Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = MetadataStore::new(metadata_path()?);
        let target = open_files_target(&store, DISTRO_NAME, WSL_USER, &slug)?;
        tauri_plugin_opener::open_path(&target, None::<&str>).map_err(|e| {
            Envelope::new(Code::WorkspaceOpenFilesFailed, Stage::Workspace)
                .with_context("slug", &slug)
                .with_cause(&e.to_string())
        })
    })
    .await
    .map_err(join_envelope(Stage::Workspace))?
}

fn join_envelope(stage: Stage) -> impl FnOnce(tauri::Error) -> Envelope {
    move |e| {
        Envelope::new(Code::InternalUnknown, stage).with_context("detail", format!("join: {e}"))
    }
}
