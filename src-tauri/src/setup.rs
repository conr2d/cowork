//! WP9①: the thin Tauri command surface over the sealed `cowork-host` setup
//! phases (preflight, wsl-enable, provision + guest inject, guest bootstrap /
//! agent-install, reboot-resume, uninstall). Glue only — every decision lives in
//! `cowork-host`; here we adapt it to Tauri commands, stream guest progress over
//! an `ipc::Channel`, and surface failures as the frontend's `Envelope`. Like
//! `pty.rs`, this is compiled and clippy'd only on the windows-latest CI host job
//! (src-tauri does not build off-Windows), so it calls cowork-host's
//! `#[cfg(windows)]` items without `cfg` gating.

use std::path::PathBuf;

use cowork_errors::{Code, Envelope, Stage};
use cowork_host::preflight::{PreflightReport, WindowsProbe, run_preflight};
use cowork_host::provision::{
    ProvisionOutcome, RunOutcome, WindowsProvisionOps, inject_guest, provision, remove_cowork,
    run_guest, setup_firstboot_user,
};
use cowork_host::setup_marker::{clear_setup_marker, is_setup_complete, mark_setup_complete};
use cowork_host::wsl::{
    ResumeStage, ResumeState, WindowsWslOps, WslEnableOutcome, arm_resume, clear_resume_state,
    disarm_resume, enable_wsl, load_resume_state, save_resume_state,
};
use serde::Serialize;
use tauri::ipc::Channel;

/// Non-error result of [`wsl_enable`]. A failure is the `Err(Envelope)` arm.
#[derive(Debug, Clone, Serialize)]
pub enum WslEnableDto {
    /// WSL is installed and current; proceed to provisioning.
    Ready,
    /// WSL was installed/updated; the machine must reboot. Resume state is saved
    /// and RunOnce is armed; the frontend shows the restart affordance.
    RebootRequired,
}

/// Non-error result of [`provision_run`].
#[derive(Debug, Clone, Serialize)]
pub enum ProvisionDto {
    /// A fresh `Cowork` distro was created and the guest CLI injected.
    Ready,
    /// A `Cowork` distro already existed (idempotent re-use); guest CLI injected.
    AlreadyExists,
}

/// Persisted wizard resume state, exposed to the frontend on a `--resume` launch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeDto {
    pub stage: ResumeStage,
    pub selected_agents: Vec<String>,
}

/// One live guest progress update forwarded over the `ipc::Channel`. Terminal
/// outcomes (done / error / protocol fault) arrive as the command's `Result`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub stage: Stage,
    pub step: String,
}

/// `%LOCALAPPDATA%\Cowork\resume.json` — the per-user reboot-resume state file.
/// Creates the `Cowork` directory if missing.
fn resume_state_path() -> Result<PathBuf, Envelope> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        Envelope::new(Code::HostResumeStateCorrupt, Stage::WslEnable)
            .with_context("detail", "LOCALAPPDATA is not set")
    })?;
    let mut dir = PathBuf::from(base);
    dir.push("Cowork");
    std::fs::create_dir_all(&dir).map_err(|e| {
        Envelope::new(Code::HostResumeStateCorrupt, Stage::WslEnable)
            .with_context("detail", format!("create {}: {e}", dir.display()))
    })?;
    dir.push("resume.json");
    Ok(dir)
}

/// `%LOCALAPPDATA%\Cowork\setup-complete.json` — written when the wizard
/// finishes; its presence boots the app into the shell.
fn setup_marker_path() -> Result<PathBuf, Envelope> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        Envelope::new(Code::HostSetupMarkerFailed, Stage::Done)
            .with_context("detail", "LOCALAPPDATA is not set")
    })?;
    let mut dir = PathBuf::from(base);
    dir.push("Cowork");
    dir.push("setup-complete.json");
    Ok(dir)
}

/// The host-side directory the embedded guest binary is extracted into
/// (`%LOCALAPPDATA%\Cowork\guest`). Created if missing.
#[cfg(feature = "embed-guest")]
fn guest_extract_dir() -> Result<PathBuf, Envelope> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
        Envelope::new(Code::GuestCliInjectFailed, Stage::Provision)
            .with_context("detail", "LOCALAPPDATA is not set")
    })?;
    let mut dir = PathBuf::from(base);
    dir.push("Cowork");
    dir.push("guest");
    std::fs::create_dir_all(&dir).map_err(|e| {
        Envelope::new(Code::GuestCliInjectFailed, Stage::Provision)
            .with_context("detail", format!("create {}: {e}", dir.display()))
    })?;
    Ok(dir)
}

/// Resolve the host-side guest binary to inject into the distro.
///
/// PROVENANCE: with the `embed-guest` feature the cross-compiled musl guest
/// `cowork` binary is embedded at build time (build.rs stages it into OUT_DIR)
/// and extracted here to `%LOCALAPPDATA%\Cowork\guest\cowork`; `inject_guest`
/// then copies it into the distro. An explicit `COWORK_GUEST_BIN` path always
/// wins (used by clean-room/e2e harnesses). Without the feature and without the
/// override, this reports `guest.cli_inject_failed`.
#[cfg(feature = "embed-guest")]
fn resolve_guest_binary() -> Result<String, Envelope> {
    if let Some(p) = std::env::var_os("COWORK_GUEST_BIN") {
        return Ok(p.to_string_lossy().into_owned());
    }
    const GUEST_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cowork-guest"));
    let dest = guest_extract_dir()?.join("cowork");
    std::fs::write(&dest, GUEST_BIN).map_err(|e| {
        Envelope::new(Code::GuestCliInjectFailed, Stage::Provision).with_context(
            "detail",
            format!("write guest binary {}: {e}", dest.display()),
        )
    })?;
    Ok(dest.to_string_lossy().into_owned())
}

/// See the `embed-guest` variant above. Without that feature, only an explicit
/// `COWORK_GUEST_BIN` override provides a guest binary.
#[cfg(not(feature = "embed-guest"))]
fn resolve_guest_binary() -> Result<String, Envelope> {
    if let Some(p) = std::env::var_os("COWORK_GUEST_BIN") {
        return Ok(p.to_string_lossy().into_owned());
    }
    Err(
        Envelope::new(Code::GuestCliInjectFailed, Stage::Provision).with_context(
            "detail",
            "guest binary not embedded (build with --features embed-guest)",
        ),
    )
}

/// Map a finished guest [`RunOutcome`] to the command result the frontend reads.
fn run_outcome_to_result(outcome: RunOutcome) -> Result<(), Envelope> {
    match outcome {
        RunOutcome::Done { .. } => Ok(()),
        RunOutcome::GuestFailed(env)
        | RunOutcome::ProtocolFault(env)
        | RunOutcome::Crashed(env)
        | RunOutcome::LaunchFailed(env) => Err(env),
    }
}

/// Run all preflight checks. Never fails as a command — the report's per-check
/// outcomes (and `can_proceed`) carry any failing envelopes for the UI.
#[tauri::command]
pub fn preflight_run() -> PreflightReport {
    run_preflight(&WindowsProbe)
}

/// Enable/update WSL. On `RebootRequired` this persists resume state (carrying
/// `selected_agents` so the post-reboot relaunch keeps the user's selection) and
/// arms RunOnce, then returns so the frontend can present the restart step.
#[tauri::command]
pub async fn wsl_enable(selected_agents: Vec<String>) -> Result<WslEnableDto, Envelope> {
    tauri::async_runtime::spawn_blocking(move || match enable_wsl(&WindowsWslOps) {
        WslEnableOutcome::Ready => Ok(WslEnableDto::Ready),
        WslEnableOutcome::RebootRequired => {
            let path = resume_state_path()?;
            save_resume_state(
                &path,
                &ResumeState::new(ResumeStage::WslReady, selected_agents),
            )?;
            let exe = std::env::current_exe().map_err(|e| {
                Envelope::new(Code::InternalUnknown, Stage::WslEnable)
                    .with_context("detail", format!("current_exe: {e}"))
            })?;
            arm_resume(&exe.to_string_lossy())?;
            Ok(WslEnableDto::RebootRequired)
        }
        WslEnableOutcome::Failed(env) => Err(env),
    })
    .await
    .map_err(join_envelope(Stage::WslEnable))?
}

/// Provision the dedicated `Cowork` distro, then inject the guest CLI into it.
#[tauri::command]
pub async fn provision_run() -> Result<ProvisionDto, Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let dto = match provision(&WindowsProvisionOps) {
            ProvisionOutcome::Ready => ProvisionDto::Ready,
            ProvisionOutcome::AlreadyExists => ProvisionDto::AlreadyExists,
            ProvisionOutcome::Failed(env) => return Err(env),
        };
        let src = resolve_guest_binary()?;
        inject_guest(&src)?;
        setup_firstboot_user()?;
        Ok(dto)
    })
    .await
    .map_err(join_envelope(Stage::Provision))?
}

/// Run the guest toolchain bootstrap (WP6), forwarding progress over `on_progress`.
#[tauri::command]
pub async fn guest_bootstrap(on_progress: Channel<ProgressEvent>) -> Result<(), Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let outcome = run_guest(
            Stage::Toolchain,
            &["bootstrap".to_string()],
            &mut |stage, step| {
                let _ = on_progress.send(ProgressEvent {
                    stage,
                    step: step.to_string(),
                });
            },
        );
        run_outcome_to_result(outcome)
    })
    .await
    .map_err(join_envelope(Stage::Toolchain))?
}

/// Install the selected coding agents (WP7), forwarding progress over `on_progress`.
/// Agent slugs (`claude|codex|antigravity`) are validated guest-side by clap.
#[tauri::command]
pub async fn guest_agent_install(
    agents: Vec<String>,
    on_progress: Channel<ProgressEvent>,
) -> Result<(), Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut extra = vec!["agent-install".to_string()];
        for agent in &agents {
            extra.push("--agent".to_string());
            extra.push(agent.clone());
        }
        let outcome = run_guest(Stage::AgentInstall, &extra, &mut |stage, step| {
            let _ = on_progress.send(ProgressEvent {
                stage,
                step: step.to_string(),
            });
        });
        run_outcome_to_result(outcome)
    })
    .await
    .map_err(join_envelope(Stage::AgentInstall))?
}

/// Uninstall: unregister the `Cowork` distro and clear host-side resume state.
#[tauri::command]
pub async fn remove_cowork_distro() -> Result<(), Envelope> {
    tauri::async_runtime::spawn_blocking(move || {
        remove_cowork(&WindowsProvisionOps)?;
        if let Ok(path) = resume_state_path() {
            let _ = clear_resume_state(&path);
        }
        if let Ok(path) = setup_marker_path() {
            clear_setup_marker(&path);
        }
        let _ = disarm_resume();
        Ok(())
    })
    .await
    .map_err(join_envelope(Stage::Provision))?
}

/// True if setup has completed on this machine.
#[tauri::command]
pub fn setup_is_complete() -> bool {
    setup_marker_path()
        .map(|p| is_setup_complete(&p))
        .unwrap_or(false)
}

/// Mark setup complete (wizard finish). Idempotent.
#[tauri::command]
pub fn setup_mark_complete() -> Result<(), Envelope> {
    mark_setup_complete(&setup_marker_path()?)
}

/// True if the process was relaunched by RunOnce after a reboot (`--resume`).
#[tauri::command]
pub fn is_resume_launch() -> bool {
    std::env::args().any(|a| a == "--resume")
}

/// Read persisted resume state, if any. `Ok(None)` when there is no pending
/// resume; `Err(host.resume_state_corrupt)` when the file exists but is invalid.
#[tauri::command]
pub fn get_resume_state() -> Result<Option<ResumeDto>, Envelope> {
    let path = resume_state_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let state = load_resume_state(&path)?;
    Ok(Some(ResumeDto {
        stage: state.stage,
        selected_agents: state.selected_agents,
    }))
}

/// Clear resume state + disarm RunOnce (called once the resume is consumed, or on
/// abort). Idempotent.
#[tauri::command]
pub fn clear_resume() -> Result<(), Envelope> {
    let path = resume_state_path()?;
    let _ = clear_resume_state(&path);
    let _ = disarm_resume();
    Ok(())
}

/// Build a closure that maps a `spawn_blocking` `JoinError` to an
/// `internal.unknown` envelope stamped at `stage`.
fn join_envelope(stage: Stage) -> impl FnOnce(tauri::Error) -> Envelope {
    move |e| {
        Envelope::new(Code::InternalUnknown, stage).with_context("detail", format!("join: {e}"))
    }
}
