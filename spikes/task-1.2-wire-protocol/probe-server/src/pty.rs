//! The pseudo-terminal. The same code serves both data candidates, so the comparison is of what
//! goes on the wire and not of pty plumbing.

use std::io::{Read as _, Write};
use std::sync::mpsc::Sender;

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::term::TermMsg;

pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
}

impl Pty {
    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
}

/// Open a pty, run `program` in it, and start the thread that reads its output.
///
/// # Errors
/// Anything the platform refuses: no pty available, or the program not found.
pub fn open(
    program: &str,
    args: &[String],
    cols: u16,
    rows: u16,
    to_term: Sender<TermMsg>,
) -> std::io::Result<Pty> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| std::io::Error::other(err.to_string()))?;

    let mut command = CommandBuilder::new(program);
    command.args(args);
    command.env("TERM", "xterm-256color");
    command.env("COLUMNS", cols.to_string());
    command.env("LINES", rows.to_string());
    if let Ok(cwd) = std::env::current_dir() {
        command.cwd(cwd);
    }
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err| std::io::Error::other(err.to_string()))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|err| std::io::Error::other(err.to_string()))?;

    let exit_sender = to_term.clone();
    std::thread::Builder::new()
        .name(format!("pty-read-{program}"))
        .spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(read) => {
                        if exit_sender
                            .send(TermMsg::PtyBytes(buf[..read].to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            // The program ended. The pane and its screen stay: that is R2, and it is what the
            // reattach demonstration compares against after the output stops.
            let _ = child.wait();
        })?;

    Ok(Pty {
        master: pair.master,
        writer,
    })
}
