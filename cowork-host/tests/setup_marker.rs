use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use cowork_host::setup_marker::{clear_setup_marker, is_setup_complete, mark_setup_complete};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cowork-host-setup-marker-test-{nanos}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn fresh_path_is_not_setup_complete() {
    let temp = TempDir::new();
    let path = temp.path.join("setup-complete");

    assert!(!is_setup_complete(&path));
}

#[test]
fn mark_setup_complete_writes_empty_marker() {
    let temp = TempDir::new();
    let path = temp.path.join("setup-complete");

    mark_setup_complete(&path).expect("write setup marker");

    assert!(path.exists());
    assert!(is_setup_complete(&path));
    let raw = fs::read_to_string(&path).expect("read setup marker");
    assert!(raw.is_empty());
}

#[test]
fn mark_setup_complete_is_idempotent() {
    let temp = TempDir::new();
    let path = temp.path.join("setup-complete");

    mark_setup_complete(&path).expect("first write succeeds");
    mark_setup_complete(&path).expect("second write succeeds");

    assert!(is_setup_complete(&path));
}

#[test]
fn clear_setup_marker_removes_marker_idempotently() {
    let temp = TempDir::new();
    let path = temp.path.join("setup-complete");
    mark_setup_complete(&path).expect("write setup marker");

    clear_setup_marker(&path);
    assert!(!is_setup_complete(&path));

    clear_setup_marker(&path);
    assert!(!is_setup_complete(&path));
}

#[test]
fn mark_setup_complete_reports_marker_error_when_parent_is_file() {
    let temp = TempDir::new();
    let parent = temp.path.join("not-a-dir");
    fs::write(&parent, "file").expect("write parent file");
    let path = parent.join("setup-complete");

    let err = mark_setup_complete(&path).expect_err("parent file should fail");

    assert_eq!(err.code.as_str(), "host.setup_marker_failed");
}
