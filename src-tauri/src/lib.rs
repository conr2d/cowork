//! Cowork — Windows host driver (Tauri v2).
//!
//! v0.1 WP0 scaffold: this launches an empty shell window only. The real host
//! driver lands across WP3–WP8:
//!   - preflight queries (build/arch/virtualization/disk/elevation/policy),
//!   - the `wsl.exe` driver + UAC elevation + `RunOnce` reboot-resume,
//!   - the ConPTY <-> `wsl.exe` PTY bridge to the xterm.js terminal,
//!   - guest-CLI injection and JSON-lines progress parsing.
//!
//! Windows-specific code lives **only** in this crate. The `cowork` guest CLI
//! stays host-agnostic (enforced by the host/guest-separation conformance gate)
//! so a future Mac/Linux host is "write a new host driver", not a rewrite.

mod pty;
mod setup;
mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .manage(pty::PtyState::default())
        .invoke_handler(tauri::generate_handler![
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            setup::preflight_run,
            setup::wsl_enable,
            setup::provision_run,
            setup::guest_bootstrap,
            setup::guest_agent_install,
            setup::remove_cowork_distro,
            setup::is_resume_launch,
            setup::get_resume_state,
            setup::clear_resume,
            setup::setup_is_complete,
            setup::setup_mark_complete,
            workspace::workspace_create,
            workspace::workspace_list,
            workspace::workspace_update,
            workspace::workspace_delete,
            workspace::workspace_slug_preview,
            workspace::workspace_open_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
