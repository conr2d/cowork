//! Host workspace bridge (v0.2 WP1): metadata, slug derivation, and the pure
//! decision flow for creating, listing, updating, and deleting workspaces. The
//! real Windows runner is isolated behind [`WorkspaceGuestOps`]; tests exercise
//! this module off-Windows with scripted guest outcomes.

pub mod metadata;
pub mod slug;
#[cfg(windows)]
mod windows_workspace;
#[cfg(windows)]
pub use windows_workspace::WindowsWorkspaceOps;

use cowork_errors::{Code, Envelope, Stage};

use crate::provision::RunOutcome;

pub const WORKSPACES_DIR: &str = "workspaces";
pub const FILES_DIR: &str = "files";
pub const DEFAULT_WORKSPACE_SLUG: &str = "default";

/// The Windows-side UNC path of a workspace's `files/` I/O zone
/// (`\\wsl.localhost\<distro>\home\<user>\workspaces\<slug>\files`). Single
/// source of the Explorer-facing path convention.
pub fn files_unc_path(distro: &str, user: &str, slug: &str) -> String {
    format!(r"\\wsl.localhost\{distro}\home\{user}\{WORKSPACES_DIR}\{slug}\{FILES_DIR}")
}

/// Resolve the Explorer target for `slug`: the workspace must exist in the
/// metadata store (stale UI must not open arbitrary paths). Returns the UNC path.
pub fn open_files_target(
    store: &metadata::MetadataStore,
    distro: &str,
    user: &str,
    slug: &str,
) -> Result<String, Envelope> {
    let all = store.load()?;
    if !all.iter().any(|m| m.slug == slug) {
        return Err(not_found(slug));
    }
    Ok(files_unc_path(distro, user, slug))
}

/// Runs the injected guest CLI with the given extra args. Trait seam so flows
/// are unit-testable off-Windows (mirror of provision's ops seams).
pub trait WorkspaceGuestOps {
    fn run(&self, extra: &[String]) -> RunOutcome;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub agent: String,
    #[serde(default)]
    pub agent_session_uuid: Option<String>,
    pub title: String,
    pub order: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMeta {
    pub name: String,
    pub slug: String,
    pub created_ms: u64,
    pub pinned: bool,
    #[serde(default)]
    pub pin_order: Option<u32>,
    pub last_used_ms: u64,
    pub default_agent: String,
    pub preset: String,
    #[serde(default)]
    pub sessions: Vec<SessionMeta>,
    /// Reserved for v0.3+ provider switching; always None in v0.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePatch {
    pub name: Option<String>,
    pub pinned: Option<bool>,
    /// Some(None) cannot be expressed over the wire; a patch sets or leaves it.
    pub pin_order: Option<Option<u32>>,
    pub last_used_ms: Option<u64>,
    pub default_agent: Option<String>,
    pub preset: Option<String>,
    /// Whole-array replace; the frontend owns tab order/titles. None = leave as is.
    pub sessions: Option<Vec<SessionMeta>>,
}

pub struct CreateRequest {
    pub name: String,
    pub default_agent: String,
    pub preset: String,
    pub now_ms: u64,
}

pub fn create_workspace(
    ops: &dyn WorkspaceGuestOps,
    store: &metadata::MetadataStore,
    req: &CreateRequest,
) -> Result<WorkspaceMeta, Envelope> {
    let mut all = store.load()?;
    let existing_slugs = all.iter().map(|m| m.slug.clone()).collect::<Vec<_>>();
    let slug = slug::slug_from_name(&req.name, &existing_slugs)?;
    let extra = vec![
        "workspace".to_string(),
        "create".to_string(),
        "--slug".to_string(),
        slug.clone(),
        "--preset".to_string(),
        req.preset.clone(),
    ];
    match ops.run(&extra) {
        RunOutcome::Done { .. } => {}
        RunOutcome::GuestFailed(env)
        | RunOutcome::ProtocolFault(env)
        | RunOutcome::Crashed(env)
        | RunOutcome::LaunchFailed(env) => return Err(env),
    }
    let meta = WorkspaceMeta {
        name: req.name.trim().to_string(),
        slug,
        created_ms: req.now_ms,
        pinned: false,
        pin_order: None,
        last_used_ms: req.now_ms,
        default_agent: req.default_agent.clone(),
        preset: req.preset.clone(),
        sessions: vec![],
        default_provider: None,
    };
    all.push(meta.clone());
    store.save(&all)?;
    Ok(meta)
}

pub fn list_workspaces(store: &metadata::MetadataStore) -> Result<Vec<WorkspaceMeta>, Envelope> {
    store.load()
}

pub fn update_workspace(
    store: &metadata::MetadataStore,
    slug: &str,
    patch: &WorkspacePatch,
) -> Result<WorkspaceMeta, Envelope> {
    let mut all = store.load()?;
    let meta = all
        .iter_mut()
        .find(|m| m.slug == slug)
        .ok_or_else(|| not_found(slug))?;
    if let Some(name) = &patch.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(Envelope::new(Code::WorkspaceInvalidName, Stage::Workspace)
                .with_context("name", name));
        }
        meta.name = trimmed.to_string();
    }
    if let Some(pinned) = patch.pinned {
        meta.pinned = pinned;
    }
    if let Some(pin_order) = patch.pin_order {
        meta.pin_order = pin_order;
    }
    if let Some(last_used_ms) = patch.last_used_ms {
        meta.last_used_ms = last_used_ms;
    }
    if let Some(default_agent) = &patch.default_agent {
        meta.default_agent = default_agent.clone();
    }
    if let Some(preset) = &patch.preset {
        meta.preset = preset.clone();
    }
    if let Some(sessions) = &patch.sessions {
        meta.sessions = sessions.clone();
    }
    let updated = meta.clone();
    store.save(&all)?;
    Ok(updated)
}

pub fn delete_workspace(
    ops: &dyn WorkspaceGuestOps,
    store: &metadata::MetadataStore,
    slug: &str,
) -> Result<(), Envelope> {
    let extra = vec![
        "workspace".to_string(),
        "remove".to_string(),
        "--slug".to_string(),
        slug.to_string(),
    ];
    match ops.run(&extra) {
        RunOutcome::Done { .. } => {}
        RunOutcome::GuestFailed(env)
        | RunOutcome::ProtocolFault(env)
        | RunOutcome::Crashed(env)
        | RunOutcome::LaunchFailed(env) => return Err(env),
    }
    let mut all = store.load()?;
    all.retain(|m| m.slug != slug);
    store.save(&all)
}

fn not_found(slug: &str) -> Envelope {
    Envelope::new(Code::WorkspaceNotFound, Stage::Workspace).with_context("slug", slug)
}
