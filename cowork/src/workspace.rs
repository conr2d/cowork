//! Workspace filesystem operations inside the guest distro (v0.2 WP1).
//! The host owns metadata; the guest only creates/removes the workspace tree and
//! emits the standard JSON-lines protocol.

use std::fs;
use std::io;

use cowork_errors::protocol::{Message, PROTOCOL_VERSION};
use cowork_errors::{Code, Envelope, Stage};

use crate::sink::ProgressSink;

pub enum Action {
    Create { slug: String },
    Remove { slug: String },
}

pub enum WorkspaceOutcome {
    Done,
    Failed,
}

pub fn run_workspace(action: &Action, home: &str, sink: &mut dyn ProgressSink) -> WorkspaceOutcome {
    sink.emit(&Message::Hello {
        protocol_version: PROTOCOL_VERSION,
    });

    let slug = match action {
        Action::Create { slug } | Action::Remove { slug } => slug,
    };
    if !valid_slug(slug) {
        sink.emit(&Message::Error {
            envelope: Envelope::new(Code::WorkspaceInvalidName, Stage::Workspace)
                .with_context("name", slug),
        });
        return WorkspaceOutcome::Failed;
    }

    let result = match action {
        Action::Create { slug } => create_workspace(home, slug),
        Action::Remove { slug } => remove_workspace(home, slug),
    };
    if let Err(env) = result {
        sink.emit(&Message::Error { envelope: env });
        return WorkspaceOutcome::Failed;
    }

    sink.emit(&Message::Done {
        stage: Stage::Workspace,
    });
    WorkspaceOutcome::Done
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug != "."
        && slug != ".."
        && !slug.starts_with(['-', '.'])
        && !slug
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_whitespace())
}

fn create_workspace(home: &str, slug: &str) -> Result<(), Envelope> {
    fs::create_dir_all(format!("{home}/workspaces/{slug}/files")).map_err(|e| {
        Envelope::new(Code::WorkspaceCreateFailed, Stage::Workspace)
            .with_context("slug", slug)
            .with_cause(&e.to_string())
    })
}

fn remove_workspace(home: &str, slug: &str) -> Result<(), Envelope> {
    match fs::remove_dir_all(format!("{home}/workspaces/{slug}")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Envelope::new(Code::WorkspaceDeleteFailed, Stage::Workspace)
            .with_context("slug", slug)
            .with_cause(&e.to_string())),
    }
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
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock must be after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("cowork-workspace-test-{nanos}"));
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

    #[test]
    fn create_makes_files_dir_and_is_idempotent() {
        let home = TempHome::new();
        let mut sink = CollectingSink::default();
        let action = Action::Create {
            slug: "pdf-translate".to_string(),
        };
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        assert!(home.path.join("workspaces/pdf-translate/files").is_dir());
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
    }

    #[test]
    fn remove_deletes_tree_and_missing_is_done() {
        let home = TempHome::new();
        fs::create_dir_all(home.path.join("workspaces/report/files")).expect("seed workspace");
        let action = Action::Remove {
            slug: "report".to_string(),
        };
        let mut sink = CollectingSink::default();
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        assert!(!home.path.join("workspaces/report").exists());
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
    }

    #[test]
    fn invalid_slugs_emit_invalid_name() {
        let home = TempHome::new();
        let invalid = [
            "",
            "a/b",
            "a\\b",
            "a b",
            ".",
            "..",
            "-a",
            ".a",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ];
        for slug in invalid {
            let mut sink = CollectingSink::default();
            let action = Action::Create {
                slug: slug.to_string(),
            };
            assert!(matches!(
                run_workspace(&action, home.as_str(), &mut sink),
                WorkspaceOutcome::Failed
            ));
            let Message::Error { envelope } = &sink.messages[1] else {
                panic!("expected error message");
            };
            assert_eq!(envelope.code, Code::WorkspaceInvalidName);
        }
    }

    #[test]
    fn success_messages_are_hello_then_done() {
        let home = TempHome::new();
        let mut sink = CollectingSink::default();
        let action = Action::Create {
            slug: "ok".to_string(),
        };
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        assert!(matches!(sink.messages[0], Message::Hello { .. }));
        assert!(matches!(
            sink.messages[1],
            Message::Done {
                stage: Stage::Workspace
            }
        ));
    }
}
