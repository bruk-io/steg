//! One pseudo-terminal, the same for both candidates, so the comparison is of emulators and not
//! of pty plumbing. `portable-pty` (MIT) was chosen over `alacritty_terminal::tty` precisely
//! because the latter would drag one candidate's crate into the other candidate's measurement.

use std::io::{Read as _, Write as _};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::thread;
use std::time::Duration;

use anyhow::{Context as _, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// A running program on a pseudo-terminal, with its output arriving on a channel so the caller
/// never blocks on a read.
pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    output: Receiver<Vec<u8>>,
    child: Box<dyn Child + Send + Sync>,
}

impl Pty {
    /// Start a program on a new pseudo-terminal of the given size.
    ///
    /// # Errors
    /// If the pseudo-terminal cannot be opened or the program cannot be started.
    pub fn spawn(command: CommandBuilder, cols: u16, rows: u16) -> Result<Self> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("open a pseudo-terminal")?;
        let child = pair
            .slave
            .spawn_command(command)
            .context("start the program")?;
        // The slave handle has to go, or the master never sees end of file.
        drop(pair.slave);

        let writer = pair.master.take_writer().context("take the pty writer")?;
        let mut reader = pair.master.try_clone_reader().context("clone the reader")?;
        let (sender, output) = channel();
        thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                let mut buffer = vec![0_u8; 64 * 1024];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(count) => {
                            if sender.send(buffer[..count].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .context("start the pty reader thread")?;

        Ok(Self {
            master: pair.master,
            writer,
            output,
            child,
        })
    }

    /// Start the user's login shell, with a predictable environment.
    ///
    /// # Errors
    /// As [`Pty::spawn`].
    pub fn spawn_shell(cols: u16, rows: u16) -> Result<Self> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_owned());
        let mut command = CommandBuilder::new(shell);
        command.env("TERM", "xterm-256color");
        command.env("LANG", "en_US.UTF-8");
        Self::spawn(command, cols, rows)
    }

    /// Send bytes to the program, as typing does.
    ///
    /// # Errors
    /// If the pseudo-terminal has gone away.
    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes).context("write to the pty")?;
        self.writer.flush().context("flush the pty")
    }

    /// Every chunk of output waiting right now. Never blocks.
    pub fn drain(&mut self) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();
        loop {
            match self.output.try_recv() {
                Ok(chunk) => chunks.push(chunk),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return chunks,
            }
        }
    }

    /// Wait up to `timeout` for the next chunk of output.
    pub fn next_chunk(&mut self, timeout: Duration) -> Option<Vec<u8>> {
        self.output.recv_timeout(timeout).ok()
    }

    /// Resize the pseudo-terminal, which signals the program.
    ///
    /// # Errors
    /// If the ioctl fails.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("resize the pty")
    }

    /// Whether the program has exited.
    pub fn finished(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
    }

    /// Stop the program.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}
