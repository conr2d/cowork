use std::fs;
use std::path::PathBuf;

use cowork_errors::{Code, Envelope, Stage};

use super::WorkspaceMeta;

pub struct MetadataStore {
    path: PathBuf,
}

impl MetadataStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Missing file -> Ok(vec![]). Unreadable -> workspace.metadata_io_failed.
    /// Unparsable JSON -> workspace.metadata_corrupt.
    pub fn load(&self) -> Result<Vec<WorkspaceMeta>, Envelope> {
        match fs::read_to_string(&self.path) {
            Ok(raw) => serde_json::from_str(&raw).map_err(|e| {
                Envelope::new(Code::WorkspaceMetadataCorrupt, Stage::Workspace)
                    .with_cause(&e.to_string())
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(
                Envelope::new(Code::WorkspaceMetadataIoFailed, Stage::Workspace)
                    .with_context("op", "read")
                    .with_cause(&e.to_string()),
            ),
        }
    }

    /// Atomic save via `<path with extension "json.tmp">`, then rename.
    pub fn save(&self, all: &[WorkspaceMeta]) -> Result<(), Envelope> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| write_error(e.to_string()))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let raw = serde_json::to_string_pretty(all).map_err(|e| write_error(e.to_string()))?;
        fs::write(&tmp, raw).map_err(|e| write_error(e.to_string()))?;
        fs::rename(&tmp, &self.path).map_err(|e| write_error(e.to_string()))?;
        Ok(())
    }
}

fn write_error(cause: String) -> Envelope {
    Envelope::new(Code::WorkspaceMetadataIoFailed, Stage::Workspace)
        .with_context("op", "write")
        .with_cause(&cause)
}
