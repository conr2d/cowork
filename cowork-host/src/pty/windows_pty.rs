//! cfg(windows) ConPTY glue: turns a pure `PtyCommand` into a live
//! `portable-pty` session (the Windows `NativePtySystem` is ConPTY). Thin glue
//! verified at the WP10 e2e gate + the windows-gnu cross-check, NOT by unit
//! tests — all decision logic is in `super::command`. The Tauri command/Channel
//! layer that pumps the reader to xterm.js lives in `src-tauri`; this crate
//! stays Tauri-free.

use std::io::{Read, Write};
use std::thread;
use std::time::{Duration, Instant};

use cowork_errors::{Envelope, Stage};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use super::command::{PtyCommand, pty_bridge_failed_envelope, pty_spawn_failed_envelope};

/// A live ConPTY session: the master (for resize), the input writer, a
/// once-takeable output reader, and the child handle (for kill). Stored behind
/// a `Mutex` in the Tauri command layer.
pub struct WindowsPtySession {
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    reader: Option<Box<dyn Read + Send>>,
    child: Box<dyn Child + Send + Sync>,
}

impl WindowsPtySession {
    const EXIT_GRACE: Duration = Duration::from_secs(2);
    const EXIT_POLL: Duration = Duration::from_millis(50);

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
            master: Some(pair.master),
            writer: Some(writer),
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
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| pty_bridge_failed_envelope(stage, "pty writer unavailable"))?;
        writer
            .write_all(data)
            .and_then(|()| writer.flush())
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))
    }

    /// Resize the ConPTY (xterm `onResize`).
    pub fn resize(&self, rows: u16, cols: u16, stage: Stage) -> Result<(), Envelope> {
        self.master
            .as_ref()
            .ok_or_else(|| pty_bridge_failed_envelope(stage, "pty master unavailable"))?
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))
    }

    /// Close the PTY first so the shell sees HUP/EOF, then force-kill after a
    /// short bounded grace period if the child still has not exited.
    pub fn kill(&mut self, stage: Stage) -> Result<(), Envelope> {
        self.reader.take();
        self.writer.take();
        self.master.take();

        let deadline = Instant::now() + Self::EXIT_GRACE;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return Ok(()),
                Ok(None) if Instant::now() < deadline => thread::sleep(Self::EXIT_POLL),
                Ok(None) => break,
                Err(e) => return Err(pty_bridge_failed_envelope(stage, &e.to_string())),
            }
        }

        self.child
            .kill()
            .map_err(|e| pty_bridge_failed_envelope(stage, &e.to_string()))
    }
}
