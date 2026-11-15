//! A pane's program: the thread that owns its terminal and runs its
//! program, for the pane's whole life (TASK-1.40).
//!
//! One OS thread per pane, created once (by [`start`]) and alive until
//! [`PaneHandle::shutdown`] - not one thread per program run, because the
//! kernel's `Terminal` is created on it, must never move
//! (decision-3 section 7: every engine handle is `!Send + !Sync`), and
//! must outlive a restart (R97: "keeps what it printed"). While a program
//! is running, one more short-lived relay thread is spawned for that run
//! alone: it blocks reading the pseudo-terminal, forwards bytes to the
//! pane's own thread as they arrive, and - on end-of-file - waits for the
//! exit code and forwards that too, then ends (the same shape
//! `spikes/task-1.2-wire-protocol/probe-server/src/pty.rs` already uses).
//!
//! Opening the pseudo-terminal and spawning the process both happen
//! synchronously, on whichever thread calls [`start`] or
//! [`PaneHandle::restart`] - never inside a spawned thread - so a bad
//! command is a plain `Result` to that caller, not something that has to
//! escape a thread it can't join synchronously.
//!
//! `Tree` (the kernel) lives on the server's own control thread, a
//! different thread entirely: the two sides talk only over
//! [`PaneHandle`]'s commands and the [`PaneEvent`]s this module sends out,
//! both `Send`, so the `!Send` engine never has to cross a boundary.
//!
//! **A known gap**, found and left for the task that builds forced close
//! (`--force`), not this one: [`PaneHandle::shutdown`] ends a program
//! that has produced *some* output, but a program that never writes
//! anything at all, such as a plain `sleep`, leaves its relay thread
//! blocked in `read` for as long as it lives - see `spawn_relay`'s own
//! comment and this module's tests for why the pseudo-terminal's natural
//! hang-up does not reach it on this platform.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;

use steg_kernel::Terminal;
use steg_sdk::{PaneId, Screen};

use crate::error::Error;
use crate::pty::{LaunchSpec, ProgramLauncher};

/// doc-9 R15's default scrollback, in lines. Not exposed as a parameter
/// here: nothing in this task's acceptance criteria needs a different
/// value, and TASK-1.74 (scrollback identity and paging) is where that
/// would be revisited.
const DEFAULT_SCROLLBACK_LINES: usize = 10_000;

/// What happened to a pane's program, reported out of its worker thread.
/// TASK-1.41 reads this to compute richer status; TASK-1.46 forwards it as
/// an event to clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneEvent {
    /// The program running in `pane` ended with `exit_code`.
    Exited {
        /// The pane whose program ended.
        pane: PaneId,
        /// The exit code the program ended with.
        exit_code: i32,
    },
}

/// A command sent to a pane's worker thread. Private: [`PaneHandle`] is
/// the only public surface this module offers.
enum PaneCommand {
    /// Bytes for the program's standard input.
    Input(Vec<u8>),
    /// A new size for the terminal and, if a program is running, the
    /// pseudo-terminal underneath it.
    Resize(u16, u16),
    /// Read the terminal's current screen back through this reply
    /// channel.
    Snapshot(Sender<Result<Screen, Error>>),
    /// Resume with a freshly spawned program (a restart): the pieces
    /// [`SpawnedProgram`] splits apart, already spawned on the caller's
    /// thread by [`PaneHandle::restart`].
    Resume {
        /// Writes to the new program's standard input.
        writer: Box<dyn Write + Send>,
        /// Resizes the new pseudo-terminal.
        resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
    },
    /// End the worker thread.
    Shutdown,
    /// Bytes read from the current run's pseudo-terminal (internal: sent
    /// by this run's relay thread, never by a caller).
    PtyBytes(Vec<u8>),
    /// The current run's program ended with this exit code (internal:
    /// sent by this run's relay thread, never by a caller).
    Exited(i32),
}

/// The writer and resize handle of whichever program is currently
/// running in a pane, if any.
struct Running {
    writer: Box<dyn Write + Send>,
    resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
}

/// A handle to a pane's worker thread: the pane's whole program-lifecycle
/// surface for later tasks. `send_input`/`snapshot` are what an attached
/// stream calls (TASK-1.47); `resize` is the one call that keeps the
/// kernel `Terminal` and the pseudo-terminal in step (TASK-1.42).
pub struct PaneHandle {
    commands: Sender<PaneCommand>,
    launcher: Arc<dyn ProgramLauncher>,
}

impl PaneHandle {
    /// Sends bytes to the program's standard input. A no-op, not an
    /// error, if no program is currently running (between an exit and a
    /// restart).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneGone`] if the pane's worker thread is no
    /// longer there.
    pub fn send_input(&self, bytes: Vec<u8>) -> Result<(), Error> {
        self.commands
            .send(PaneCommand::Input(bytes))
            .map_err(|_| Error::PaneGone)
    }

    /// Resizes the pane's terminal and, if a program is running, the
    /// pseudo-terminal underneath it, together (TASK-1.42).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneGone`] if the pane's worker thread is no
    /// longer there.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Error> {
        self.commands
            .send(PaneCommand::Resize(cols, rows))
            .map_err(|_| Error::PaneGone)
    }

    /// Reads the pane's current screen (TASK-1.47 attaches here).
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneGone`] if the pane's worker thread is no
    /// longer there, or whatever the terminal itself failed with.
    pub fn snapshot(&self) -> Result<Screen, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(PaneCommand::Snapshot(reply_tx))
            .map_err(|_| Error::PaneGone)?;
        reply_rx.recv().map_err(|_| Error::PaneGone)?
    }

    /// Runs the pane's program again (R97): a new pseudo-terminal and a
    /// new process, spawned synchronously on this call - a bad command is
    /// a plain `Err` right here - but the *same* `Terminal`, which keeps
    /// what it already printed. The caller is responsible for having
    /// already asked the kernel (`steg_kernel::Tree::restart_program`)
    /// first; this is only the server's half.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pty`] if the new pseudo-terminal could not be
    /// opened or the program could not be spawned, or [`Error::PaneGone`]
    /// if the pane's worker thread is no longer there.
    pub fn restart(&self, spec: LaunchSpec) -> Result<(), Error> {
        let spawned = self.launcher.spawn(&spec)?;
        let _relay = spawn_relay(spawned.reader, spawned.child, self.commands.clone());
        self.commands
            .send(PaneCommand::Resume {
                writer: spawned.writer,
                resize: spawned.resize,
            })
            .map_err(|_| Error::PaneGone)
    }

    /// Ends the pane's worker thread. Whatever is currently running loses
    /// its standard input and resize handle when this is dropped; killing
    /// the process outright (`--force`) is not this task's job.
    pub fn shutdown(self) {
        let _ = self.commands.send(PaneCommand::Shutdown);
    }
}

/// Starts a pane's program (doc-9 R13): opens its pseudo-terminal and
/// spawns the process on the calling thread, then creates the pane's own
/// worker thread, which creates the `Terminal` and owns it from here on.
///
/// # Errors
///
/// Returns [`Error::Pty`] if the pseudo-terminal could not be opened or
/// the program could not be spawned, or [`Error::Io`] if the pane's own
/// worker thread could not be created.
pub fn start(
    spec: LaunchSpec,
    pane: PaneId,
    launcher: Arc<dyn ProgramLauncher>,
    events: Sender<PaneEvent>,
) -> Result<PaneHandle, Error> {
    let (cols, rows) = spec.size;
    let spawned = launcher.spawn(&spec)?;

    let (commands_tx, commands_rx) = mpsc::channel::<PaneCommand>();
    let _relay = spawn_relay(spawned.reader, spawned.child, commands_tx.clone());

    let initial = Running {
        writer: spawned.writer,
        resize: spawned.resize,
    };

    thread::Builder::new()
        .name(format!("steg-pane-{pane}"))
        .spawn(move || worker_loop(commands_rx, cols, rows, Some(initial), pane, events))
        .map_err(Error::Io)?;

    Ok(PaneHandle {
        commands: commands_tx,
        launcher,
    })
}

/// The pane's own thread: creates the `Terminal` once, then serves
/// commands until [`PaneCommand::Shutdown`].
///
/// If the `Terminal` cannot be created (only `ghostty-vt` construction
/// failing - no observed case in practice, see `crates/kernel/src/
/// terminal.rs`'s own tests), this returns immediately without serving
/// anything: `commands_rx` is dropped, so the very next call through the
/// `PaneHandle` this pane was given fails with [`Error::PaneGone`] rather
/// than hanging - no panic, and no silent half-working pane.
fn worker_loop(
    commands_rx: mpsc::Receiver<PaneCommand>,
    cols: u16,
    rows: u16,
    initial: Option<Running>,
    pane: PaneId,
    events: Sender<PaneEvent>,
) {
    let Ok(mut terminal) = Terminal::new(cols, rows, DEFAULT_SCROLLBACK_LINES) else {
        return;
    };
    let mut current = initial;

    for command in commands_rx {
        match command {
            PaneCommand::Input(bytes) => {
                if let Some(running) = &mut current {
                    let _ = running.writer.write_all(&bytes);
                }
            }
            PaneCommand::Resize(cols, rows) => {
                let _ = terminal.resize(cols, rows);
                if let Some(running) = &current {
                    let _ = (running.resize)(cols, rows);
                }
            }
            PaneCommand::Snapshot(reply) => {
                let _ = reply.send(terminal.screen().map_err(Error::from));
            }
            PaneCommand::Resume { writer, resize } => {
                current = Some(Running { writer, resize });
            }
            PaneCommand::PtyBytes(bytes) => terminal.write(&bytes),
            PaneCommand::Exited(exit_code) => {
                current = None;
                let _ = events.send(PaneEvent::Exited { pane, exit_code });
            }
            PaneCommand::Shutdown => break,
        }
    }
}

/// Spawns the short-lived relay thread for one program run: blocks
/// reading `reader` and forwards each chunk as [`PaneCommand::PtyBytes`]
/// until end-of-file, then blocks on `child.wait()` and forwards the exit
/// code as [`PaneCommand::Exited`]. Mirrors
/// `spikes/task-1.2-wire-protocol/probe-server/src/pty.rs`'s own reader
/// thread.
fn spawn_relay(
    mut reader: Box<dyn Read + Send>,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    commands: Sender<PaneCommand>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(read) => {
                    if commands
                        .send(PaneCommand::PtyBytes(buf[..read].to_vec()))
                        .is_err()
                    {
                        // The pane's worker thread is gone - most often
                        // `PaneHandle::shutdown` while this run's program
                        // was still producing output. Stop forwarding:
                        // nothing is listening. But still fall through to
                        // `child.wait()` below - a dropped receiver must
                        // never skip reaping the child, or it is left a
                        // zombie (this is what the early `return` this
                        // comment replaces got wrong).
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        // Reached by a normal end-of-file, or by the send failure above -
        // a dropped receiver (the pane's worker thread is gone) must stop
        // forwarding, never skip reaping the child it's still attached
        // to. This is only reached at all for a program that produces
        // *something* to read, eventually: a program that never writes
        // anything, such as a plain `sleep`, leaves this thread blocked
        // in the `read` above for as long as it lives, even after
        // `PaneHandle::shutdown` drops the worker thread's `Running`
        // (its writer and the resize closure's `MasterPty`) - probed
        // directly (see this module's test module) rather than assumed:
        // on this platform, that drop alone does not deliver the pty's
        // hang-up, because the relay's own cloned reader (a genuinely
        // separate descriptor from `MasterPty::try_clone_reader`) keeps
        // the pty's master side from ever being fully closed, which is
        // what the hang-up condition actually requires. Ending that case
        // needs either a kill - `--force`, left to the task that builds
        // it - or giving this thread a descriptor it can close itself,
        // neither of which this task's plan covers.
        let exit_code = match child.wait() {
            Ok(status) => i32::try_from(status.exit_code()).unwrap_or(i32::MAX),
            // The child could not be waited on at all - no exit code to
            // report; the pane stays in whatever state it was, which is
            // observable (no `Exited` event ever arrives) rather than a
            // guessed code.
            Err(_) => return,
        };
        let _ = commands.send(PaneCommand::Exited(exit_code));
    })
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use crate::pty::{LaunchSpec, ProgramLauncher, RealLauncher};

    use super::{PaneCommand, spawn_relay};

    fn real_spec(argv: Vec<&str>) -> LaunchSpec {
        LaunchSpec {
            argv: argv.into_iter().map(str::to_string).collect(),
            cwd: std::env::temp_dir().to_str().unwrap().to_string(),
            env: Vec::new(),
            size: (80, 24),
        }
    }

    /// Joins `handle` with a bound, rather than waiting on it forever -
    /// `thread::JoinHandle` has no built-in timeout, so a watchdog thread
    /// does the blocking join and this blocks on a bounded `recv_timeout`
    /// for it instead, never a sleep loop.
    fn join_within(handle: thread::JoinHandle<()>, timeout: Duration) -> bool {
        let (done_tx, done_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = handle.join();
            let _ = done_tx.send(());
        });
        done_rx.recv_timeout(timeout).is_ok()
    }

    /// Whether a process with this id still exists, reaped or not -
    /// `kill -0` (a real subprocess, not a double, and no new dependency:
    /// std's own `Command` is enough to probe this) delivers no signal
    /// and only reports whether the pid could be signalled at all. A
    /// zombie - reaped or not - behaves identically to this probe only
    /// while unreaped; once `child.wait()` runs, the pid stops existing
    /// at all and this reports `false`.
    fn process_exists(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("kill(1) is on PATH on every platform this runs on")
            .success()
    }

    #[test]
    fn shutdown_mid_run_still_reaps_a_program_that_was_still_producing_output() {
        // Reproduces the review's first scenario directly: the pane's
        // worker is "gone" (its receiver dropped) while
        // `sh -c 'sleep 1; echo done'` is still running - the exact
        // condition under which `spawn_relay`'s send used to fail and
        // return early, skipping `child.wait()` and leaving a zombie.
        let spawned = RealLauncher
            .spawn(&real_spec(vec!["sh", "-c", "sleep 1; echo done"]))
            .unwrap();
        let pid = spawned.child.process_id().unwrap();
        let (tx, rx) = mpsc::channel::<PaneCommand>();

        let relay = spawn_relay(spawned.reader, spawned.child, tx);
        drop(rx); // the pane's worker is "gone" before the program ends

        assert!(
            join_within(relay, Duration::from_secs(10)),
            "the relay thread never ended"
        );
        assert!(
            !process_exists(pid),
            "pid {pid} still exists: child.wait() was skipped, leaving a zombie"
        );
    }

    // What this probed, directly, rather than assumed: whether dropping
    // the handles a real shutdown drops - the writer and the resize
    // closure's `MasterPty` - delivers the pseudo-terminal's hang-up to
    // a program with *nothing else to read*, the way the review asked
    // for. It does not, on this platform, and the reason is specific
    // enough to write down rather than re-discover: dropping only those
    // two still leaves a third master-side reference open - the relay
    // thread's own cloned reader (`MasterPty::try_clone_reader`, a
    // genuinely separate file descriptor) - and the kernel's hang-up
    // condition is "every master-side descriptor closed", not "the
    // writer and the original master". Isolated scratch probes (not
    // committed; reproducible by spawning a real pty, a real `sleep
    // 1000`, and dropping only the writer and the original master while
    // a cloned reader stays open elsewhere) show exactly two effects of
    // that drop: `UnixMasterWriter::drop` writes `\n` plus the line
    // discipline's EOF character into the pty, which a canonical-mode,
    // echoing terminal echoes straight back as a few bytes with no
    // bearing on the child at all (`sleep` never reads its stdin); and
    // nothing else. The child does not receive `SIGHUP`, because the
    // relay's own reader clone is still open. A probe that drops *every*
    // master-side descriptor, including that clone, does see `SIGHUP`
    // reliably and reaps cleanly - but the relay thread cannot drop its
    // own clone before `read` returns, which is exactly the thread this
    // gap is about.
    //
    // So: the mid-run case above is fixed (a failed send no longer skips
    // `child.wait()`), but a pane shut down while its program has
    // produced nothing at all, ever, stays exactly as the review found
    // it - its relay thread blocked in `read` for as long as the program
    // lives, holding the pty's last master-side descriptor open. No
    // explicit kill is added here: this task's plan already leaves
    // killing a running program to the task that builds forced close
    // (`--force`), and that is the only remaining way to end this case,
    // since the natural hang-up this review asked for turns out not to
    // reach a program with an open reader clone standing in its way. A
    // fix would need the relay to own a descriptor it can close itself
    // (dropping `try_clone_reader` for a raw descriptor taken from
    // `MasterPty::as_raw_fd` and reconstructing resize without the
    // trait object) - real surgery on `RealLauncher`'s ownership shape,
    // not a one-line change, and `unsafe` this crate does not use
    // anywhere else today.
}
