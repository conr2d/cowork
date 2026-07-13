//! v0.2 WP2a: the setup-complete marker. Written once the wizard finishes;
//! its presence routes the app into the shell instead of the wizard at boot.
//! Presence is the whole signal; the marker file has no body.

use std::path::Path;

use cowork_errors::{Code, Envelope, Stage};

/// True if setup has completed on this machine (the marker file exists).
pub fn is_setup_complete(path: &Path) -> bool {
    path.exists()
}

/// Write the marker atomically (temp file + rename, same pattern as
/// `workspace::metadata`). Idempotent.
pub fn mark_setup_complete(path: &Path) -> Result<(), Envelope> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| setup_marker_error(format!("create {}: {e}", parent.display())))?;
    }

    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, b"")
        .map_err(|e| setup_marker_error(format!("write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        setup_marker_error(format!(
            "rename {} to {}: {e}",
            tmp.display(),
            path.display()
        ))
    })?;
    Ok(())
}

/// Best-effort removal (uninstall path). Missing file is fine.
pub fn clear_setup_marker(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn setup_marker_error(detail: String) -> Envelope {
    Envelope::new(Code::HostSetupMarkerFailed, Stage::Done).with_context("detail", detail)
}
