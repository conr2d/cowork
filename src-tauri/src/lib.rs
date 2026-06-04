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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
