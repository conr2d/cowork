//! Workspace filesystem operations inside the guest distro (v0.2 WP1).
//! The host owns metadata; the guest only creates/removes the workspace tree and
//! emits the standard JSON-lines protocol.

use std::fs;
use std::io;
use std::os::unix::fs::symlink;
use std::path::Path;

use cowork_errors::protocol::{Message, PROTOCOL_VERSION};
use cowork_errors::{Code, Envelope, Stage};

use crate::sink::ProgressSink;

pub enum Action {
    Create { slug: String, preset: String },
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
        Action::Create { slug, .. } | Action::Remove { slug } => slug,
    };
    if !valid_slug(slug) {
        sink.emit(&Message::Error {
            envelope: Envelope::new(Code::WorkspaceInvalidName, Stage::Workspace)
                .with_context("name", slug),
        });
        return WorkspaceOutcome::Failed;
    }

    if let Action::Create { preset, .. } = action {
        if !crate::preset::KNOWN_PRESETS.contains(&preset.as_str()) {
            let env = Envelope::new(Code::WorkspaceInvalidPreset, Stage::Workspace)
                .with_context("preset", preset);
            sink.emit(&Message::Error {
                envelope: env.clone(),
            });
            return WorkspaceOutcome::Failed;
        }
    }

    let result = match action {
        Action::Create { slug, preset } => create_workspace(home, slug, preset),
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

pub(crate) fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 64
        && slug != "."
        && slug != ".."
        && !slug.starts_with(['-', '.'])
        && !slug
            .chars()
            .any(|c| c == '/' || c == '\\' || c.is_whitespace())
}

fn create_workspace(home: &str, slug: &str, preset: &str) -> Result<(), Envelope> {
    let dir = Path::new(home).join("workspaces").join(slug);
    fs::create_dir_all(dir.join("files")).map_err(|e| create_failed(slug, e))?;

    if let Some(body) = crate::preset::template(preset) {
        fs::write(dir.join("AGENTS.md"), body).map_err(|e| create_failed(slug, e))?;
        match symlink("AGENTS.md", dir.join("CLAUDE.md")) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(create_failed(slug, e)),
        }
    }

    Ok(())
}

fn create_failed(slug: &str, e: io::Error) -> Envelope {
    Envelope::new(Code::WorkspaceCreateFailed, Stage::Workspace)
        .with_context("slug", slug)
        .with_cause(&e.to_string())
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
            preset: "blank".to_string(),
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
    fn create_with_pdf_preset_writes_agents_and_claude_symlink() {
        let home = TempHome::new();
        let mut sink = CollectingSink::default();
        let action = Action::Create {
            slug: "pdf-translate".to_string(),
            preset: "pdf".to_string(),
        };
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        assert!(home.path.join("workspaces/pdf-translate/files").is_dir());
        let agents = fs::read_to_string(home.path.join("workspaces/pdf-translate/AGENTS.md"))
            .expect("read agents template");
        assert!(agents.starts_with("# Workspace: Document Translation"));
        assert_eq!(
            fs::read_link(home.path.join("workspaces/pdf-translate/CLAUDE.md"))
                .expect("read claude symlink"),
            Path::new("AGENTS.md")
        );
    }

    #[test]
    fn create_with_proposal_preset_writes_proposal_template() {
        let home = TempHome::new();
        let mut sink = CollectingSink::default();
        let action = Action::Create {
            slug: "proposal".to_string(),
            preset: "proposal".to_string(),
        };
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        let agents = fs::read_to_string(home.path.join("workspaces/proposal/AGENTS.md"))
            .expect("read agents template");
        assert!(agents.starts_with("# Workspace: Proposal Drafting"));
    }

    #[test]
    fn create_with_blank_preset_writes_no_instruction_files() {
        let home = TempHome::new();
        let mut sink = CollectingSink::default();
        let action = Action::Create {
            slug: "blank".to_string(),
            preset: "blank".to_string(),
        };
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        assert!(home.path.join("workspaces/blank/files").is_dir());
        assert!(!home.path.join("workspaces/blank/AGENTS.md").exists());
        assert!(!home.path.join("workspaces/blank/CLAUDE.md").exists());
    }

    #[test]
    fn invalid_preset_fails_before_creating_workspace() {
        let home = TempHome::new();
        let mut sink = CollectingSink::default();
        let action = Action::Create {
            slug: "bad-preset".to_string(),
            preset: "garbage".to_string(),
        };
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Failed
        ));
        let Message::Error { envelope } = &sink.messages[1] else {
            panic!("expected error message");
        };
        assert_eq!(envelope.code, Code::WorkspaceInvalidPreset);
        assert!(!home.path.join("workspaces/bad-preset").exists());
    }

    #[test]
    fn create_with_pdf_preset_is_idempotent_when_symlink_exists() {
        let home = TempHome::new();
        let mut sink = CollectingSink::default();
        let action = Action::Create {
            slug: "pdf-translate".to_string(),
            preset: "pdf".to_string(),
        };
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        assert!(matches!(
            run_workspace(&action, home.as_str(), &mut sink),
            WorkspaceOutcome::Done
        ));
        assert_eq!(
            fs::read_link(home.path.join("workspaces/pdf-translate/CLAUDE.md"))
                .expect("read claude symlink"),
            Path::new("AGENTS.md")
        );
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
                preset: "blank".to_string(),
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
            preset: "blank".to_string(),
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
