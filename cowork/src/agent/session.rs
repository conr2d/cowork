//! Agent session UUID capture (v0.2 WP4d): scan durable agent session artifacts
//! for the newest conversation tied to a workspace cwd and spawn time.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::UNIX_EPOCH;

use cowork_errors::protocol::{Message, PROTOCOL_VERSION};
use cowork_errors::{Envelope, Stage};
use uuid::Uuid;

use crate::sink::ProgressSink;
use crate::workspace::valid_slug;

use super::command::{self, Agent};

/// `{home}/workspaces/{slug}` — the spawn cwd the agents key their sessions on.
pub fn workspace_cwd(home: &str, slug: &str) -> String {
    format!("{home}/workspaces/{slug}")
}

pub fn valid_session_uuid(uuid: &str) -> bool {
    Uuid::parse_str(uuid).is_ok()
}

/// Newest codex rollout in `codex_home/sessions/**` whose first-line
/// `payload.cwd` == cwd and mtime > since_ms. Returns `payload.id`.
pub fn codex_session_uuid(codex_home: &Path, cwd: &str, since_ms: u64) -> Option<String> {
    let root = codex_home.join("sessions");
    let mut best: Option<(u64, String, String)> = None;
    visit_codex_rollouts(&root, cwd, since_ms, &mut best);
    best.map(|(_, _, uuid)| uuid)
}

fn visit_codex_rollouts(
    dir: &Path,
    cwd: &str,
    since_ms: u64,
    best: &mut Option<(u64, String, String)>,
) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_codex_rollouts(&path, cwd, since_ms, best);
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.starts_with("rollout-") || !file_name.ends_with(".jsonl") {
            continue;
        }
        let Some(mtime_ms) = modified_ms(&path) else {
            continue;
        };
        if mtime_ms <= since_ms {
            continue;
        }
        let Some(uuid) = codex_rollout_uuid(&path, cwd) else {
            continue;
        };
        let candidate = (mtime_ms, file_name.to_string(), uuid);
        if best.as_ref().is_none_or(|current| candidate > *current) {
            *best = Some(candidate);
        }
    }
}

fn codex_rollout_uuid(path: &Path, cwd: &str) -> Option<String> {
    let value = rollout_first_line_json(path)?;
    if value["payload"]["cwd"].as_str() != Some(cwd) {
        return None;
    }
    value["payload"]["id"].as_str().map(str::to_string)
}

fn codex_rollout_any_uuid(path: &Path) -> Option<String> {
    let value = rollout_first_line_json(path)?;
    value["payload"]["id"].as_str().map(str::to_string)
}

fn rollout_first_line_json(path: &Path) -> Option<serde_json::Value> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    serde_json::from_str::<serde_json::Value>(&line).ok()
}

/// Newest `conversations/*.db` under agy_root whose raw bytes contain cwd and
/// mtime > since_ms. Returns the file stem (the conversation UUID).
pub fn agy_session_uuid(agy_root: &Path, cwd: &str, since_ms: u64) -> Option<String> {
    let conversations = agy_root.join("conversations");
    let Ok(entries) = fs::read_dir(conversations) else {
        return None;
    };
    let mut best: Option<(u64, String, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("db") {
            continue;
        }
        let Some(mtime_ms) = modified_ms(&path) else {
            continue;
        };
        if mtime_ms <= since_ms {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        if !contains_subslice(&bytes, cwd.as_bytes()) {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let candidate = (mtime_ms, file_name.to_string(), stem.to_string());
        if best.as_ref().is_none_or(|current| candidate > *current) {
            best = Some(candidate);
        }
    }
    best.map(|(_, _, uuid)| uuid)
}

pub fn claude_session_exists(claude_root: &Path, uuid: &str) -> bool {
    path_named_exists(&claude_root.join("projects"), &format!("{uuid}.jsonl"))
}

pub fn codex_session_exists(codex_home: &Path, uuid: &str) -> bool {
    rollout_with_uuid_exists(&codex_home.join("sessions"), uuid)
}

pub fn agy_session_exists(agy_root: &Path, uuid: &str) -> bool {
    agy_root
        .join("conversations")
        .join(format!("{uuid}.db"))
        .is_file()
}

fn path_named_exists(dir: &Path, file_name: &str) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path_named_exists(&path, file_name) {
                return true;
            }
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some(file_name) {
            return true;
        }
    }
    false
}

fn rollout_with_uuid_exists(dir: &Path, uuid: &str) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if rollout_with_uuid_exists(&path, uuid) {
                return true;
            }
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.starts_with("rollout-") || !file_name.ends_with(".jsonl") {
            continue;
        }
        let Some(found_uuid) = codex_rollout_any_uuid(&path) else {
            continue;
        };
        if found_uuid == uuid {
            return true;
        }
    }
    false
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty()
        || haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn modified_ms(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()?
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}

#[derive(Debug, Clone)]
pub enum SessionUuidOutcome {
    Done,
    Failed(Envelope),
}

pub fn run_session_uuid(
    sink: &mut dyn ProgressSink,
    agent: Agent,
    home: &str,
    slug: &str,
    since_ms: u64,
) -> SessionUuidOutcome {
    sink.emit(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    });

    if !valid_slug(slug) {
        let env = Envelope::new(cowork_errors::Code::WorkspaceInvalidName, Stage::Workspace)
            .with_context("name", slug);
        sink.emit(&Message::Error {
            envelope: env.clone(),
        });
        return SessionUuidOutcome::Failed(env);
    }

    let cwd = workspace_cwd(home, slug);
    let uuid = match agent {
        Agent::Codex => codex_session_uuid(
            Path::new(&command::config_dir(Agent::Codex, home)),
            &cwd,
            since_ms,
        ),
        Agent::Antigravity => agy_session_uuid(
            Path::new(&command::config_dir(Agent::Antigravity, home)),
            &cwd,
            since_ms,
        ),
        Agent::Claude => None,
    };

    sink.emit(&Message::SessionUuid {
        agent: agent.id().to_string(),
        uuid,
    });
    sink.emit(&Message::Done {
        stage: Stage::Workspace,
    });
    SessionUuidOutcome::Done
}

pub fn run_session_check(
    sink: &mut dyn ProgressSink,
    agent: Agent,
    home: &str,
    uuid: &str,
) -> SessionUuidOutcome {
    sink.emit(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    });

    if !valid_session_uuid(uuid) {
        let env = Envelope::new(
            cowork_errors::Code::SessionUuidCaptureFailed,
            Stage::Workspace,
        )
        .with_context("agent", agent.id())
        .with_cause("invalid session uuid");
        sink.emit(&Message::Error {
            envelope: env.clone(),
        });
        return SessionUuidOutcome::Failed(env);
    }

    let exists = match agent {
        Agent::Claude => {
            claude_session_exists(Path::new(&command::config_dir(Agent::Claude, home)), uuid)
        }
        Agent::Codex => {
            codex_session_exists(Path::new(&command::config_dir(Agent::Codex, home)), uuid)
        }
        Agent::Antigravity => agy_session_exists(
            Path::new(&command::config_dir(Agent::Antigravity, home)),
            uuid,
        ),
    };

    sink.emit(&Message::SessionUuid {
        agent: agent.id().to_string(),
        uuid: exists.then(|| uuid.to_string()),
    });
    sink.emit(&Message::Done {
        stage: Stage::Workspace,
    });
    SessionUuidOutcome::Done
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Default)]
    struct CollectingSink {
        messages: Vec<Message>,
    }

    impl ProgressSink for CollectingSink {
        fn emit(&mut self, message: &Message) {
            self.messages.push(message.clone());
        }
    }

    struct TempHome {
        path: PathBuf,
    }

    impl TempHome {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock must be after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("cowork-session-{name}-{nanos}"));
            fs::create_dir_all(&path).expect("create temp home");
            Self { path }
        }

        fn as_str(&self) -> &str {
            self.path.to_str().expect("temp path must be utf-8")
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_rollout(root: &Path, name: &str, cwd: &str, uuid: &str) {
        let path = root.join("sessions/2026/06/12");
        fs::create_dir_all(&path).expect("create rollout dir");
        fs::write(
            path.join(name),
            format!(r#"{{"type":"session_meta","payload":{{"id":"{uuid}","cwd":"{cwd}"}}}}"#),
        )
        .expect("write rollout");
    }

    fn write_claude_conversation(root: &Path, dir: &str, uuid: &str) {
        let path = root.join(format!("projects/{dir}"));
        fs::create_dir_all(&path).expect("create claude dir");
        fs::write(path.join(format!("{uuid}.jsonl")), b"{}\n").expect("write claude convo");
    }

    #[test]
    fn workspace_cwd_formats_home_workspace_slug() {
        assert_eq!(
            workspace_cwd("/home/cowork", "demo"),
            "/home/cowork/workspaces/demo"
        );
    }

    #[test]
    fn valid_session_uuid_accepts_and_rejects_expected_shapes() {
        assert!(valid_session_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!valid_session_uuid("not-a-uuid"));
    }

    #[test]
    fn codex_rollout_matching_cwd_returns_payload_id() {
        let home = TempHome::new("codex-match");
        let cwd = "/home/u/workspaces/app";
        write_rollout(&home.path, "rollout-2026-a-u1.jsonl", cwd, "u1");
        assert_eq!(
            codex_session_uuid(&home.path, cwd, 0).as_deref(),
            Some("u1")
        );
    }

    #[test]
    fn codex_rollout_skips_different_cwd_and_garbage() {
        let home = TempHome::new("codex-skip");
        let cwd = "/home/u/workspaces/app";
        write_rollout(
            &home.path,
            "rollout-2026-a-other.jsonl",
            "/home/u/workspaces/other",
            "other",
        );
        fs::write(
            home.path
                .join("sessions/2026/06/12/rollout-2026-b-garbage.jsonl"),
            "not json\n",
        )
        .expect("write garbage");
        assert_eq!(codex_session_uuid(&home.path, cwd, 0), None);
    }

    #[test]
    fn codex_rollout_skips_mtime_at_or_before_since() {
        let home = TempHome::new("codex-old");
        let cwd = "/home/u/workspaces/app";
        write_rollout(&home.path, "rollout-2026-c-old.jsonl", cwd, "old");
        assert_eq!(codex_session_uuid(&home.path, cwd, u64::MAX), None);
    }

    #[test]
    fn codex_rollout_later_filename_wins_when_mtimes_collide() {
        let home = TempHome::new("codex-order");
        let cwd = "/home/u/workspaces/app";
        write_rollout(&home.path, "rollout-2026-a-u1.jsonl", cwd, "u1");
        write_rollout(&home.path, "rollout-2026-b-u2.jsonl", cwd, "u2");
        assert_eq!(
            codex_session_uuid(&home.path, cwd, 0).as_deref(),
            Some("u2")
        );
    }

    #[test]
    fn agy_db_matching_cwd_returns_stem() {
        let home = TempHome::new("agy-match");
        let conversations = home.path.join("conversations");
        fs::create_dir_all(&conversations).expect("create conversations");
        fs::write(
            conversations.join("u1.db"),
            b"prefix /home/u/workspaces/app suffix",
        )
        .expect("write db");
        assert_eq!(
            agy_session_uuid(&home.path, "/home/u/workspaces/app", 0).as_deref(),
            Some("u1")
        );
    }

    #[test]
    fn agy_skips_non_matching_sidecars_and_missing_dir() {
        let home = TempHome::new("agy-skip");
        let conversations = home.path.join("conversations");
        fs::create_dir_all(&conversations).expect("create conversations");
        fs::write(conversations.join("nope.db"), b"other cwd").expect("write db");
        fs::write(conversations.join("u1.db-shm"), b"/home/u/workspaces/app")
            .expect("write sidecar");
        fs::write(conversations.join("u1.db-wal"), b"/home/u/workspaces/app")
            .expect("write sidecar");
        assert_eq!(
            agy_session_uuid(&home.path, "/home/u/workspaces/app", 0),
            None
        );
        let missing = TempHome::new("agy-missing");
        assert_eq!(
            agy_session_uuid(&missing.path, "/home/u/workspaces/app", 0),
            None
        );
    }

    #[test]
    fn claude_session_exists_finds_recursive_match_only_by_exact_filename() {
        let home = TempHome::new("claude-exists");
        write_claude_conversation(&home.path, "proj-a", "550e8400-e29b-41d4-a716-446655440000");
        write_claude_conversation(&home.path, "proj-b", "550e8400-e29b-41d4-a716-446655440001");
        fs::write(
            home.path
                .join("projects/proj-b/550e8400-e29b-41d4-a716-446655440000.jsonl.bak"),
            b"{}\n",
        )
        .expect("write unrelated file");
        assert!(claude_session_exists(
            &home.path,
            "550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(!claude_session_exists(
            &home.path,
            "550e8400-e29b-41d4-a716-446655440099"
        ));
    }

    #[test]
    fn codex_session_exists_matches_payload_id_only() {
        let home = TempHome::new("codex-exists");
        write_rollout(
            &home.path,
            "rollout-2026-a-u1.jsonl",
            "/home/u/workspaces/app",
            "550e8400-e29b-41d4-a716-446655440000",
        );
        fs::write(
            home.path
                .join("sessions/2026/06/12/rollout-2026-b-garbage.jsonl"),
            "not json\n",
        )
        .expect("write garbage");
        assert!(codex_session_exists(
            &home.path,
            "550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(!codex_session_exists(
            &home.path,
            "550e8400-e29b-41d4-a716-446655440099"
        ));
    }

    #[test]
    fn agy_session_exists_matches_db_file_only() {
        let home = TempHome::new("agy-exists");
        let conversations = home.path.join("conversations");
        fs::create_dir_all(&conversations).expect("create conversations");
        fs::write(
            conversations.join("550e8400-e29b-41d4-a716-446655440000.db"),
            b"db",
        )
        .expect("write db");
        fs::write(
            conversations.join("550e8400-e29b-41d4-a716-446655440000.db-wal"),
            b"db",
        )
        .expect("write wal");
        assert!(agy_session_exists(
            &home.path,
            "550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(!agy_session_exists(
            &home.path,
            "550e8400-e29b-41d4-a716-446655440099"
        ));
    }

    #[test]
    fn run_session_uuid_emits_hello_session_uuid_done_for_empty_codex_home() {
        let home = TempHome::new("run-codex");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_session_uuid(&mut sink, Agent::Codex, home.as_str(), "app", 0),
            SessionUuidOutcome::Done
        ));
        assert!(matches!(sink.messages[0], Message::Hello { .. }));
        assert!(matches!(
            &sink.messages[1],
            Message::SessionUuid { agent, uuid } if agent == "codex" && uuid.is_none()
        ));
        assert!(matches!(
            sink.messages[2],
            Message::Done {
                stage: Stage::Workspace
            }
        ));
    }

    #[test]
    fn run_session_uuid_invalid_slug_emits_workspace_error() {
        let home = TempHome::new("run-invalid");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_session_uuid(&mut sink, Agent::Codex, home.as_str(), "../x", 0),
            SessionUuidOutcome::Failed(_)
        ));
        assert!(matches!(sink.messages[0], Message::Hello { .. }));
        let Message::Error { envelope } = &sink.messages[1] else {
            panic!("expected error");
        };
        assert_eq!(envelope.code, cowork_errors::Code::WorkspaceInvalidName);
    }

    #[test]
    fn run_session_uuid_claude_returns_none() {
        let home = TempHome::new("run-claude");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_session_uuid(&mut sink, Agent::Claude, home.as_str(), "app", 0),
            SessionUuidOutcome::Done
        ));
        assert!(matches!(
            &sink.messages[1],
            Message::SessionUuid { agent, uuid } if agent == "claude" && uuid.is_none()
        ));
    }

    #[test]
    fn run_session_check_claude_reports_found_uuid() {
        let home = TempHome::new("check-claude");
        write_claude_conversation(
            &home.path.join(".claude"),
            "proj-a",
            "550e8400-e29b-41d4-a716-446655440000",
        );
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_session_check(
                &mut sink,
                Agent::Claude,
                home.as_str(),
                "550e8400-e29b-41d4-a716-446655440000"
            ),
            SessionUuidOutcome::Done
        ));
        assert!(matches!(
            &sink.messages[1],
            Message::SessionUuid { agent, uuid }
                if agent == "claude"
                    && uuid.as_deref() == Some("550e8400-e29b-41d4-a716-446655440000")
        ));
    }

    #[test]
    fn run_session_check_missing_uuid_reports_none() {
        let home = TempHome::new("check-missing");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_session_check(
                &mut sink,
                Agent::Antigravity,
                home.as_str(),
                "550e8400-e29b-41d4-a716-446655440000"
            ),
            SessionUuidOutcome::Done
        ));
        assert!(matches!(
            &sink.messages[1],
            Message::SessionUuid { agent, uuid } if agent == "antigravity" && uuid.is_none()
        ));
    }

    #[test]
    fn run_session_check_invalid_uuid_emits_error() {
        let home = TempHome::new("check-invalid");
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_session_check(&mut sink, Agent::Claude, home.as_str(), "not-a-uuid"),
            SessionUuidOutcome::Failed(_)
        ));
        let Message::Error { envelope } = &sink.messages[1] else {
            panic!("expected error");
        };
        assert_eq!(envelope.code, cowork_errors::Code::SessionUuidCaptureFailed);
        assert_eq!(
            envelope.context.get("agent").map(String::as_str),
            Some("claude")
        );
    }
}
