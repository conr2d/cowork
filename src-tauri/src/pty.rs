//! WP8②: the thin Tauri command/event layer over cowork-host PTY sessions.
//! All ConPTY spawn/IO/resize/kill logic lives in `cowork_host::pty`; here we
//! expose it as Tauri commands and pump PTY output to xterm.js over an
//! `ipc::Channel`. Sessions live in a keyed registry so terminals persist across
//! workspace switches; they are killed explicitly or when the window is
//! destroyed. No PTY logic lives here — this is glue, compiled and
//! clippy'd on the windows-latest CI host job (src-tauri does not build
//! off-Windows).

use std::io::Read;

use base64::Engine;
use cowork_errors::{Envelope, Stage};
use cowork_host::pty::{
    PtyRegistry, WindowsPtySession, pty_bridge_failed_envelope, terminal_launch,
};
use tauri::State;
use tauri::ipc::Channel;

pub type PtyState = PtyRegistry<WindowsPtySession>;

/// The embedded terminal runs in the wizard's agent-login (auth) step.
const STAGE: Stage = Stage::Auth;

/// Bytes read from the PTY per chunk before base64-framing onto the channel.
const READ_CHUNK: usize = 4096;

/// Spawn the embedded terminal: an interactive login shell in the `Cowork`
/// distro at `workspace`. The frontend creates `onData` (a `Channel<String>`)
/// and passes the already-fitted `rows`/`cols`, so the ConPTY is the correct
/// size from the first byte (never hardcode a default — that corrupts the first
/// frame of full-screen TUIs). Each `onData` message is base64 of one raw PTY
/// output chunk; the frontend base64-decodes to bytes and writes them to xterm.
#[tauri::command]
pub fn pty_spawn(
    state: State<'_, PtyState>,
    on_data: Channel<String>,
    distro: String,
    workspace: String,
    locale: String,
    rows: u16,
    cols: u16,
) -> Result<u64, Envelope> {
    let cmd = terminal_launch(&distro, &workspace, &locale);
    let mut session = WindowsPtySession::spawn(&cmd, rows, cols, STAGE)?;
    let reader = session
        .take_reader()
        .ok_or_else(|| pty_bridge_failed_envelope(STAGE, "pty reader unavailable"))?;

    // Pump PTY output → channel on a dedicated thread. The reader is an
    // independent handle from the writer/master kept in state, so the pump never
    // contends on the state mutex.
    std::thread::spawn(move || pump(reader, on_data));

    Ok(state.insert(session))
}

/// Read loop: base64-frame each chunk onto the channel until EOF or the channel
/// is dropped by the frontend.
fn pump(mut reader: Box<dyn Read + Send>, on_data: Channel<String>) {
    let mut buf = [0u8; READ_CHUNK];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                if on_data.send(encoded).is_err() {
                    break;
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
pub fn pty_write(state: State<'_, PtyState>, id: u64, data: String) -> Result<(), Envelope> {
    match state.get(id) {
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
    id: u64,
    rows: u16,
    cols: u16,
) -> Result<(), Envelope> {
    match state.get(id) {
        Some(session) => session
            .lock()
            .expect("PtyRegistry session mutex poisoned")
            .resize(rows, cols, STAGE),
        None => Ok(()),
    }
}

/// Kill the session. Unknown/already-removed ids are a no-op.
#[tauri::command]
pub fn pty_kill(state: State<'_, PtyState>, id: u64) -> Result<(), Envelope> {
    match state.remove(id) {
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
    for session in state.drain() {
        let _ = session
            .lock()
            .expect("PtyRegistry session mutex poisoned")
            .kill(STAGE);
    }
}
