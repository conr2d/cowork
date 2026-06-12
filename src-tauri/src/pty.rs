//! WP8②: the thin Tauri command/event layer over the cowork-host PTY session.
//! All ConPTY spawn/IO/resize/kill logic lives in `cowork_host::pty`; here we
//! expose it as Tauri commands and pump the PTY's output to xterm.js over an
//! `ipc::Channel`. No PTY logic lives here — this is glue, compiled and
//! clippy'd on the windows-latest CI host job (src-tauri does not build
//! off-Windows).

use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use base64::Engine;
use cowork_errors::{Envelope, Stage};
use cowork_host::pty::{pty_bridge_failed_envelope, terminal_launch, WindowsPtySession};
use tauri::ipc::Channel;
use tauri::State;

/// The single embedded-terminal session. `None` until `pty_spawn`; a fresh spawn
/// kills and replaces any prior session. The generation lets stale frontend
/// cleanup skip killing a newer session after rapid workspace switches.
#[derive(Default)]
pub struct PtyState {
    session: Mutex<Option<(u64, WindowsPtySession)>>,
    next_generation: AtomicU64,
}

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

    // Install the new session, killing any prior one (so its wsl.exe child and
    // its pump thread terminate).
    let generation = state.next_generation.fetch_add(1, Ordering::Relaxed) + 1;
    let mut guard = state.session.lock().expect("PtyState mutex poisoned");
    if let Some((_, mut old)) = guard.take() {
        let _ = old.kill(STAGE);
    }
    *guard = Some((generation, session));
    Ok(generation)
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
}

/// Forward user keystrokes (xterm `onData`, a UTF-8 string) to the PTY.
#[tauri::command]
pub fn pty_write(state: State<'_, PtyState>, data: String) -> Result<(), Envelope> {
    let mut guard = state.session.lock().expect("PtyState mutex poisoned");
    match guard.as_mut() {
        Some((_, session)) => session.write_input(data.as_bytes(), STAGE),
        None => Ok(()),
    }
}

/// Resize the ConPTY when xterm reflows (`onResize`).
#[tauri::command]
pub fn pty_resize(state: State<'_, PtyState>, rows: u16, cols: u16) -> Result<(), Envelope> {
    let guard = state.session.lock().expect("PtyState mutex poisoned");
    match guard.as_ref() {
        Some((_, session)) => session.resize(rows, cols, STAGE),
        None => Ok(()),
    }
}

/// Kill the session (window closing / wizard restart). When a generation is
/// supplied, stale cleanup from a replaced terminal is a no-op.
#[tauri::command]
pub fn pty_kill(state: State<'_, PtyState>, generation: Option<u64>) -> Result<(), Envelope> {
    let mut guard = state.session.lock().expect("PtyState mutex poisoned");
    if let (Some(expected), Some((current, _))) = (generation, guard.as_ref()) {
        if *current != expected {
            return Ok(());
        }
    }
    match guard.take() {
        Some((_, mut session)) => session.kill(STAGE),
        None => Ok(()),
    }
}
