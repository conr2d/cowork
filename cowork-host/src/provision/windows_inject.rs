//! `#[cfg(windows)]` guest-CLI injection + run: write the binary into the distro
//! as root over `wsl.exe` stdin, set its executable bit, then spawn it via
//! `wsl.exe` and stream its JSON-lines stdout through the host parser. Not
//! compiled on Linux/CI-ubuntu; verified by the windows-gnu cross-check, the
//! windows-latest runner, and the WP10 e2e gate. All decision logic it relies on
//! is the pure, unit-tested `provision/inject.rs`.

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use cowork_errors::{Envelope, Stage};
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

use crate::protocol::{HostEvent, StreamParser};

use super::inject::{
    RunOutcome, chmod_args, classify_run, cli_inject_failed_envelope, cli_launch_failed_envelope,
    events_repairable, firstboot_setup_args, launch_args, needs_inject, repair_and_retry,
    terminate_args, unc_inject_path, write_args,
};
use super::user_create_failed_envelope;

/// Host-side path of the guest binary last injected, cached so a reactive repair
/// (see [`run_guest_events`]) can re-inject without threading the path through
/// every call site. First write wins; the path is stable per process.
static GUEST_SRC: OnceLock<String> = OnceLock::new();

/// Inject the host-side guest binary at `src_binary` into the `Cowork` distro:
/// stream it to `/usr/local/bin/cowork` as root, then `chmod +x` it inside the
/// distro (the piped write does not carry the executable bit).
pub fn inject_guest(src_binary: &str) -> Result<(), Envelope> {
    let _ = GUEST_SRC.set(src_binary.to_string());
    let mut child = Command::new("wsl.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(write_args())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| cli_inject_failed_envelope(&format!("launch write: {e}")))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| cli_inject_failed_envelope("launch write: missing stdin pipe"))?;
    let mut src = std::fs::File::open(src_binary)
        .map_err(|e| cli_inject_failed_envelope(&format!("read shipped {src_binary}: {e}")))?;
    if let Err(e) = std::io::copy(&mut src, &mut stdin) {
        drop(stdin);
        let _ = child.wait();
        return Err(cli_inject_failed_envelope(&format!(
            "write shipped {src_binary} -> root stdin: {e}"
        )));
    }
    if let Err(e) = stdin.flush() {
        drop(stdin);
        let _ = child.wait();
        return Err(cli_inject_failed_envelope(&format!(
            "flush shipped {src_binary} -> root stdin: {e}"
        )));
    }
    drop(stdin);

    match child.wait() {
        Ok(status) if status.success() => {}
        Ok(status) => {
            return Err(cli_inject_failed_envelope(&format!(
                "write exited {}",
                status.code().unwrap_or(-1)
            )));
        }
        Err(e) => return Err(cli_inject_failed_envelope(&format!("wait write: {e}"))),
    }

    match Command::new("wsl.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(chmod_args())
        .output()
    {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(cli_inject_failed_envelope(&format!(
            "chmod exited {}",
            out.status.code().unwrap_or(-1)
        ))),
        Err(e) => Err(cli_inject_failed_envelope(&format!("launch chmod: {e}"))),
    }
}

/// Re-inject the guest binary from the cached source path, for reactive repair.
/// If no source was ever cached (no injection this process — should not happen
/// post-provision), repair is impossible; report it so the caller surfaces the
/// original failure.
fn repair_guest() -> Result<(), Envelope> {
    match GUEST_SRC.get() {
        Some(src) => inject_guest(src),
        None => Err(cli_inject_failed_envelope(
            "reactive repair: no cached guest source path",
        )),
    }
}

/// Shell-boot guest sync: re-inject the guest CLI when the installed binary
/// differs from the shipped one. This is how rebuilt app bytes reach an
/// already-provisioned distro, and how a missing/corrupt installed guest
/// self-heals. Returns whether an injection ran. An unreadable installed
/// binary counts as stale; a failed read of the shipped binary is a real error.
pub fn sync_guest(src_binary: &str) -> Result<bool, Envelope> {
    let _ = GUEST_SRC.set(src_binary.to_string()); // cache every boot for reactive repair, even when bytes match
    let shipped = std::fs::read(src_binary)
        .map_err(|e| cli_inject_failed_envelope(&format!("read shipped {src_binary}: {e}")))?;
    let installed = std::fs::read(unc_inject_path()).ok();
    if !needs_inject(&shipped, installed.as_deref()) {
        return Ok(false);
    }
    inject_guest(src_binary)?;
    Ok(true)
}

/// Create the non-root firstboot user, set it as the distro default, and
/// `wsl --terminate` so the new default applies. Called after `inject_guest`, before
/// bootstrap. Without this, the import default user (root) runs bootstrap and
/// Homebrew aborts ("Don't run this as root!"). Idempotent on a re-provisioned distro.
pub fn setup_firstboot_user() -> Result<(), Envelope> {
    run_root_wsl(&firstboot_setup_args(), "firstboot setup")?;
    run_root_wsl(&terminate_args(), "terminate")?;
    Ok(())
}

/// Run `wsl.exe <args>` to completion, mapping any failure to
/// `distro.user_create_failed`. `what` labels the step in the (redacted) cause.
fn run_root_wsl(args: &[String], what: &str) -> Result<(), Envelope> {
    match Command::new("wsl.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(args)
        .output()
    {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(user_create_failed_envelope(&format!(
            "{what} exited {}",
            out.status.code().unwrap_or(-1)
        ))),
        Err(e) => Err(user_create_failed_envelope(&format!("launch {what}: {e}"))),
    }
}

/// Run the injected guest with `extra` args, streaming its stdout JSON-lines
/// through [`StreamParser`] (stamped with `stage`), forwarding each `Progress`
/// event live to `on_progress`, and classifying the finished run. stderr is
/// human-readable noise and is discarded; the protocol is stdout-only.
///
/// `stage` is the stage this run belongs to (`Toolchain` for `bootstrap`,
/// `AgentInstall` for `agent-install`); it stamps protocol-fault envelopes and is
/// the `classify_run` fallback. (WP5 originally hardcoded `Provision`; threading
/// it here is the forward-dependency resolution noted in the plan.)
pub fn run_guest(
    stage: Stage,
    extra: &[String],
    on_progress: &mut dyn FnMut(Stage, &str),
) -> RunOutcome {
    match run_guest_events(stage, extra, on_progress) {
        Err(env) => RunOutcome::LaunchFailed(env),
        Ok((events, exit_code)) => classify_run(&events, exit_code, stage),
    }
}

/// One guest invocation, no repair: stream stdout through the parser (stamped
/// `stage`), forward `Progress` to `on_progress`, and return ALL parsed events
/// plus the exit code. `Err` = the process could not be launched.
fn run_guest_events_once(
    stage: Stage,
    extra: &[String],
    on_progress: &mut dyn FnMut(Stage, &str),
) -> Result<(Vec<HostEvent>, i32), Envelope> {
    let mut child = match Command::new("wsl.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(launch_args(extra))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return Err(cli_launch_failed_envelope(&e.to_string())),
    };

    let mut events = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let mut parser = StreamParser::new(stage);
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(ev) = parser.push_line(&line) {
                if let crate::protocol::HostEvent::Progress { stage, step } = &ev {
                    on_progress(*stage, step);
                }
                events.push(ev);
            }
        }
    }

    let exit_code = match child.wait() {
        Ok(status) => status.code().unwrap_or(-1),
        Err(_) => -1,
    };
    Ok((events, exit_code))
}

/// Run the injected guest, with one reactive repair: if the first attempt fails
/// to launch or crashes, re-inject the binary once and retry exactly once. This
/// is the shared chokepoint for `run_guest` and direct event-stream callers. On
/// a clean run (the happy path) nothing is re-injected. See issue #38.
pub fn run_guest_events(
    stage: Stage,
    extra: &[String],
    on_progress: &mut dyn FnMut(Stage, &str),
) -> Result<(Vec<HostEvent>, i32), Envelope> {
    let first = run_guest_events_once(stage, extra, on_progress);
    repair_and_retry(
        first,
        |result| events_repairable(result, stage),
        repair_guest,
        // must be _once (not run_guest_events) — calling the repairing variant would recurse
        || run_guest_events_once(stage, extra, on_progress),
    )
}
