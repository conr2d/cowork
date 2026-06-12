use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use cowork_errors::{Code, Envelope, Stage};
use cowork_host::provision::RunOutcome;
use cowork_host::workspace::metadata::MetadataStore;
use cowork_host::workspace::slug::slug_from_name;
use cowork_host::workspace::{
    CreateRequest, SessionMeta, WorkspaceGuestOps, WorkspaceMeta, WorkspacePatch, create_workspace,
    delete_workspace, update_workspace,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cowork-host-workspace-test-{nanos}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn store(path: PathBuf) -> MetadataStore {
    MetadataStore::new(path)
}

fn meta(slug: &str) -> WorkspaceMeta {
    WorkspaceMeta {
        name: slug.to_string(),
        slug: slug.to_string(),
        created_ms: 1,
        pinned: true,
        pin_order: Some(2),
        last_used_ms: 3,
        default_agent: "claude".to_string(),
        preset: "blank".to_string(),
        sessions: vec![SessionMeta {
            id: "s1".to_string(),
            agent: "codex".to_string(),
            agent_session_uuid: Some("uuid".to_string()),
            title: "Session".to_string(),
            order: 1,
        }],
        default_provider: None,
    }
}

fn guest_error(code: Code) -> Envelope {
    Envelope::new(code, Stage::Workspace)
}

struct MockOps {
    calls: RefCell<Vec<Vec<String>>>,
    outcome: RunOutcome,
}

impl MockOps {
    fn done() -> Self {
        Self {
            calls: RefCell::new(vec![]),
            outcome: RunOutcome::Done {
                stage: Stage::Workspace,
            },
        }
    }

    fn failing(env: Envelope) -> Self {
        Self {
            calls: RefCell::new(vec![]),
            outcome: RunOutcome::GuestFailed(env),
        }
    }
}

impl WorkspaceGuestOps for MockOps {
    fn run(&self, extra: &[String]) -> RunOutcome {
        self.calls.borrow_mut().push(extra.to_vec());
        self.outcome.clone()
    }
}

#[test]
fn slug_derives_expected_values() {
    assert_eq!(
        slug_from_name("PDF Translate", &[]).unwrap(),
        "pdf-translate"
    );
    assert_eq!(slug_from_name("번역 작업", &[]).unwrap(), "번역-작업");
    assert_eq!(slug_from_name("  a   b  ", &[]).unwrap(), "a-b");
    assert_eq!(slug_from_name("!!!", &[]).unwrap(), "workspace");
    assert_eq!(
        slug_from_name("", &[]).unwrap_err().code,
        Code::WorkspaceInvalidName
    );
    assert_eq!(
        slug_from_name("   ", &[]).unwrap_err().code,
        Code::WorkspaceInvalidName
    );
    assert_eq!(
        slug_from_name("abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuv", &[]).unwrap(),
        "abcdefghijklmnopqrstuvwxyzabcdefghijklmn"
    );
    assert_eq!(
        slug_from_name("Report", &["report".to_string()]).unwrap(),
        "report-2"
    );
    assert_eq!(
        slug_from_name("Report", &["report".to_string(), "report-2".to_string()]).unwrap(),
        "report-3"
    );
}

#[test]
fn metadata_missing_loads_empty() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    assert_eq!(store.load().unwrap(), Vec::<WorkspaceMeta>::new());
}

#[test]
fn metadata_save_load_round_trip_preserves_fields() {
    let temp = TempDir::new();
    let path = temp.path.join("nested/workspaces.json");
    let store = store(path.clone());
    let all = vec![meta("default")];
    store.save(&all).unwrap();
    assert_eq!(store.load().unwrap(), all);
    assert!(!path.with_extension("json.tmp").exists());
}

#[test]
fn metadata_corrupt_returns_corrupt_code() {
    let temp = TempDir::new();
    let path = temp.path.join("workspaces.json");
    fs::write(&path, "{nope").unwrap();
    let env = store(path).load().unwrap_err();
    assert_eq!(env.code, Code::WorkspaceMetadataCorrupt);
}

#[test]
fn create_runs_guest_and_appends_metadata() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    let ops = MockOps::done();
    let req = CreateRequest {
        name: "PDF Translate".to_string(),
        default_agent: "codex".to_string(),
        preset: "blank".to_string(),
        now_ms: 42,
    };
    let created = create_workspace(&ops, &store, &req).unwrap();
    assert_eq!(
        ops.calls.borrow()[0],
        ["workspace", "create", "--slug", "pdf-translate"].map(str::to_string)
    );
    assert_eq!(created.name, "PDF Translate");
    assert_eq!(created.slug, "pdf-translate");
    assert!(!created.pinned);
    assert!(created.sessions.is_empty());
    assert_eq!(created.created_ms, 42);
    assert_eq!(created.last_used_ms, 42);
    assert_eq!(store.load().unwrap(), vec![created]);
}

#[test]
fn create_guest_failure_leaves_store_unchanged() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    store.save(&[meta("default")]).unwrap();
    let env = guest_error(Code::WorkspaceCreateFailed);
    let ops = MockOps::failing(env.clone());
    let req = CreateRequest {
        name: "Report".to_string(),
        default_agent: "codex".to_string(),
        preset: "blank".to_string(),
        now_ms: 42,
    };
    assert_eq!(
        create_workspace(&ops, &store, &req).unwrap_err().code,
        env.code
    );
    assert_eq!(store.load().unwrap(), vec![meta("default")]);
}

#[test]
fn delete_runs_guest_and_removes_metadata() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    store.save(&[meta("default"), meta("report")]).unwrap();
    let ops = MockOps::done();
    delete_workspace(&ops, &store, "report").unwrap();
    assert_eq!(
        ops.calls.borrow()[0],
        ["workspace", "remove", "--slug", "report"].map(str::to_string)
    );
    assert_eq!(store.load().unwrap(), vec![meta("default")]);
}

#[test]
fn delete_guest_failure_keeps_metadata() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    store.save(&[meta("default")]).unwrap();
    let ops = MockOps::failing(guest_error(Code::WorkspaceDeleteFailed));
    assert!(delete_workspace(&ops, &store, "default").is_err());
    assert_eq!(store.load().unwrap(), vec![meta("default")]);
}

#[test]
fn delete_absent_metadata_after_guest_done_is_ok() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    let ops = MockOps::done();
    delete_workspace(&ops, &store, "missing").unwrap();
    assert_eq!(store.load().unwrap(), Vec::<WorkspaceMeta>::new());
}

#[test]
fn update_applies_patch_and_keeps_slug() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    store.save(&[meta("default")]).unwrap();
    let updated = update_workspace(
        &store,
        "default",
        &WorkspacePatch {
            name: Some("  Renamed  ".to_string()),
            pinned: Some(false),
            pin_order: Some(Some(3)),
            last_used_ms: Some(99),
            default_agent: Some("codex".to_string()),
            preset: Some("repo".to_string()),
        },
    )
    .unwrap();
    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.slug, "default");
    assert_eq!(updated.pin_order, Some(3));
    assert_eq!(updated.last_used_ms, 99);
}

#[test]
fn update_unknown_slug_and_empty_rename_fail() {
    let temp = TempDir::new();
    let store = store(temp.path.join("workspaces.json"));
    store.save(&[meta("default")]).unwrap();
    assert_eq!(
        update_workspace(&store, "missing", &WorkspacePatch::default())
            .unwrap_err()
            .code,
        Code::WorkspaceNotFound
    );
    assert_eq!(
        update_workspace(
            &store,
            "default",
            &WorkspacePatch {
                name: Some(" ".to_string()),
                ..WorkspacePatch::default()
            },
        )
        .unwrap_err()
        .code,
        Code::WorkspaceInvalidName
    );
}
