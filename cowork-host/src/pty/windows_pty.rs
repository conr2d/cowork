//! cfg(windows) ConPTY glue: turns a pure `PtyCommand` into a live
//! `portable-pty` session (the Windows `NativePtySystem` is ConPTY). Thin glue
//! verified at the WP10 e2e gate + the windows-gnu cross-check, NOT by unit
//! tests — all decision logic is in `super::command`. The Tauri command/Channel
//! layer that pumps the reader to xterm.js lives in `src-tauri`; this crate
//! stays Tauri-free.

use std::io::{Read, Write};

use cowork_errors::{Envelope, Stage};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use super::command::{PtyCommand, pty_bridge_failed_envelope, pty_spawn_failed_envelope};

/// A live ConPTY session: the master (for resize), the input writer, a
/// once-takeable output reader, and the child handle (for kill). Stored behind
/// a `Mutex` in the Tauri command layer.
pub struct WindowsPtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    reader: Option<Box<dyn Read + Send>>,
    child: Box<dyn Child + Send + Sync>,
}

impl WindowsPtySession {
    /// Open a ConPTY of `rows` x `cols` and spawn `cmd` into it. `stage` tags any
    /// emitted envelope (the terminal runs during auth and at done).
    pub fn spawn(cmd: &PtyCommand, rows: u16, cols: u16, stage: Stage) -> Result<Self, Envelope> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| pty_spawn_failed_envelope(stage, &e.to_string()))?;

        let mut builder = CommandBuilder::new(&cmd.program);
        builder.args(&cmd.args);

        let child = pair
            .slave
            .spawn_command(builder)
            .map_err(|e| pty_spawn_failed_envelope(stage, &e.to_string()))?;
        // The slave handle is no longer needed once the child holds it; dropping
        // it lets the master observe EOF when the child exits.
        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))?;
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))?;

        Ok(Self {
            master: pair.master,
            writer,
            reader: Some(reader),
            child,
        })
    }

    /// Take the output reader (once) to move into the streaming pump thread.
    pub fn take_reader(&mut self) -> Option<Box<dyn Read + Send>> {
        self.reader.take()
    }

    /// Write user input (bytes from xterm `onData`) to the PTY.
    pub fn write_input(&mut self, data: &[u8], stage: Stage) -> Result<(), Envelope> {
        self.writer
            .write_all(data)
            .and_then(|()| self.writer.flush())
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))
    }

    /// Resize the ConPTY (xterm `onResize`).
    pub fn resize(&self, rows: u16, cols: u16, stage: Stage) -> Result<(), Envelope> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))
    }

    /// Kill the child (window closed / session ended).
    pub fn kill(&mut self, stage: Stage) -> Result<(), Envelope> {
        self.child
            .kill()
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))
    }
}
