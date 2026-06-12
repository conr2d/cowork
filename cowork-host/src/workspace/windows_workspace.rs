use cowork_errors::Stage;

use crate::provision::{RunOutcome, run_guest};

/// Production guest runner: shells out to the injected guest CLI via wsl.exe.
pub struct WindowsWorkspaceOps;

impl super::WorkspaceGuestOps for WindowsWorkspaceOps {
    fn run(&self, extra: &[String]) -> RunOutcome {
        run_guest(Stage::Workspace, extra, &mut |_, _| {})
    }
}
