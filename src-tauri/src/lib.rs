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

mod build_info;
mod pty;
mod session;
mod setup;
mod window_theme;
mod workspace;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .manage(pty::PtyState::default())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                // First run lands on the fixed-light setup wizard; a provisioned
                // machine opens the shell (dark by default). Paint the caption to
                // match before the window shows, so there is no system-grey flash
                // and no dark-caption-over-light-wizard mismatch on first launch.
                // The shell's own effect re-asserts the user's saved theme after
                // mount; the backend cannot read that localStorage preference here.
                let theme = if setup::setup_is_complete() {
                    "dark"
                } else {
                    "light"
                };
                let _ = window_theme::set_window_theme(window, theme.into());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                // Terminals are no longer torn down by frontend navigation; the
                // window closing is the backstop that kills every wsl.exe child.
                pty::kill_all(&window.state::<pty::PtyState>());
            }
        })
        .invoke_handler(tauri::generate_handler![
            build_info::app_build,
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            setup::preflight_run,
            setup::wsl_enable,
            setup::provision_run,
            setup::guest_sync,
            setup::guest_bootstrap,
            setup::guest_agent_install,
            setup::remove_cowork_distro,
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
            session::capture_session_uuid,
            session::session_check,
            session::agent_theme_sync,
            window_theme::set_window_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
