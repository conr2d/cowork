//! WP8②: the thin Tauri command/event layer over cowork-host PTY sessions.
//! All ConPTY spawn/IO/resize/kill logic lives in `cowork_host::pty`; here we
//! expose it as Tauri commands and pump PTY output to xterm.js over an
//! `ipc::Channel`. Sessions live in a keyed registry so terminals persist across
//! workspace switches; they are killed explicitly or when the window is
//! destroyed. No PTY logic lives here — this is glue, compiled and
//! clippy'd on the windows-latest CI host job (src-tauri does not build
//! off-Windows).

use std::io::Read;
use std::sync::{Arc, Mutex};

use base64::Engine;
use cowork_errors::{Code, Envelope, Stage};
use cowork_host::pty::{
    PtyRegistry, WindowsPtySession, pty_bridge_failed_envelope, terminal_launch,
};
use tauri::State;
use tauri::ipc::Channel;
use uuid::Uuid;

pub type PtyState = PtyRegistry<WindowsPtySession>;

/// The embedded terminal runs in the wizard's agent-login (auth) step.
const STAGE: Stage = Stage::Auth;

/// Bytes read from the PTY per chunk before base64-framing onto the channel.
const READ_CHUNK: usize = 4096;

/// Spawn the embedded terminal: an interactive login shell in the `Cowork`
/// distro at `workspace`. The frontend creates `onData` (a `Channel<String>`)
/// and passes the already-fitted `rows`/`cols`, so the ConPTY is the correct
/// size from the first byte (never hardcode a default — that corrupts the first
/// frame of full-screen TUIs). Optional `autorun` stays attached to the spawn
/// request so retries replay the launch command without a frontend-side race,
/// but the actual write is deferred until the shell emits its first output.
/// Each `onData` message is base64 of one raw PTY output chunk; the frontend
/// base64-decodes to bytes and writes them to xterm.
// A Tauri command's parameter list IS the IPC payload; grouping these into a
// struct would only hide the same fields behind an indirection the frontend
// would still have to spell out.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn pty_spawn(
    state: State<'_, PtyState>,
    id: String,
    on_data: Channel<String>,
    distro: String,
    workspace: String,
    locale: String,
    rows: u16,
    cols: u16,
    autorun: Option<String>,
) -> Result<(), Envelope> {
    validate_session_id(&id, STAGE)?;
    let cmd = terminal_launch(&distro, &workspace, &locale);
    let mut session = WindowsPtySession::spawn(&cmd, rows, cols, STAGE)?;
    let reader = match session.take_reader() {
        Some(reader) => reader,
        None => {
            let error = pty_bridge_failed_envelope(STAGE, "pty reader unavailable");
            let _ = session.kill(STAGE);
            return Err(error);
        }
    };

    if let Some(previous) = state.insert(id.clone(), session) {
        let _ = previous
            .lock()
            .expect("PtyRegistry session mutex poisoned")
            .kill(STAGE);
    }
    // Not `expect`: kill_all runs off the window-destroy handler, so the session
    // we just inserted can legitimately be gone by now. Losing an autorun is not
    // worth panicking a Tauri command over — the pump still streams whatever the
    // shell prints.
    let session = state.get(&id);

    // Pump PTY output → channel on a dedicated thread. The reader is an
    // independent handle from the writer/master kept in state, so the pump never
    // contends on the state mutex. The first output chunk marks the shell as
    // ready to read, so that is when we replay the deferred autorun exactly
    // once.
    std::thread::spawn(move || pump(reader, on_data, session, autorun));
    Ok(())
}

/// Read loop: base64-frame each chunk onto the channel until EOF or the channel
/// is dropped by the frontend.
fn pump(
    mut reader: Box<dyn Read + Send>,
    on_data: Channel<String>,
    session: Option<Arc<Mutex<WindowsPtySession>>>,
    mut autorun: Option<String>,
) {
    let mut buf = [0u8; READ_CHUNK];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                if on_data.send(encoded).is_err() {
                    break;
                }
                // The first chunk is the shell's prompt: it is now reading stdin.
                // Writing the launch command any earlier lets the tty echo it once
                // and readline echo it again when bash finally reads the buffer.
                if let (Some(session), Some(autorun)) = (session.as_ref(), autorun.take()) {
                    if session
                        .lock()
                        .expect("PtyRegistry session mutex poisoned")
                        .write_input(format!("{autorun}\n").as_bytes(), STAGE)
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    }
    // EOF or read failure: the child is gone. An empty chunk can never come from
    // a real read (n > 0 always), so it doubles as the exit sentinel the
    // frontend's status glyphs key off.
    let _ = on_data.send(String::new());
}

/// Forward user keystrokes (xterm `onData`, a UTF-8 string) to the PTY.
#[tauri::command]
pub fn pty_write(state: State<'_, PtyState>, id: String, data: String) -> Result<(), Envelope> {
    validate_session_id(&id, STAGE)?;
    match state.get(&id) {
        Some(session) => session
            .lock()
            .expect("PtyRegistry session mutex poisoned")
            .write_input(data.as_bytes(), STAGE),
        None => Ok(()),
    }
}

/// Resize the ConPTY when xterm reflows (`onResize`).
#[tauri::command]
pub fn pty_resize(
    state: State<'_, PtyState>,
    id: String,
    rows: u16,
    cols: u16,
) -> Result<(), Envelope> {
    validate_session_id(&id, STAGE)?;
    match state.get(&id) {
        Some(session) => session
            .lock()
            .expect("PtyRegistry session mutex poisoned")
            .resize(rows, cols, STAGE),
        None => Ok(()),
    }
}

/// Kill the session. Unknown/already-removed ids are a no-op.
#[tauri::command]
pub fn pty_kill(state: State<'_, PtyState>, id: String) -> Result<(), Envelope> {
    validate_session_id(&id, STAGE)?;
    match state.remove(&id) {
        Some(session) => session
            .lock()
            .expect("PtyRegistry session mutex poisoned")
            .kill(STAGE),
        None => Ok(()),
    }
}

/// Kill every live session (window destroyed). Errors are ignored: the children
/// are being torn down with the app and there is no surface left to report to.
pub fn kill_all(state: &PtyState) {
    let mut joins = Vec::new();
    for session in state.drain() {
        joins.push(std::thread::spawn(move || {
            let _ = session
                .lock()
                .expect("PtyRegistry session mutex poisoned")
                .kill(STAGE);
        }));
    }
    for join in joins {
        let _ = join.join();
    }
}

fn validate_session_id(id: &str, stage: Stage) -> Result<(), Envelope> {
    if Uuid::parse_str(id).is_ok() {
        Ok(())
    } else {
        Err(Envelope::new(Code::HostPtyBridgeFailed, stage).with_context("id", id.to_string()))
    }
}
