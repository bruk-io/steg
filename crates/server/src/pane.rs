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
//! **A known gap**, unreachable by any client today (dispatch calls none
//! of [`start`]/[`PaneHandle::shutdown`]/`restart` yet) and tracked for
//! the task that will make it reachable, DRAFT-4 ("Server: start a
//! pane's program from the API, and end it on a forced close"), whose
//! own AC #4 already names it: "with `--force` the program is ended, its
//! exit is observed, and its relay thread ends, for a program that is
//! producing output and for one that prints nothing". [`PaneHandle::shutdown`]
//! does **not** reliably do either today, confirmed by probing both: a
//! program that would finish on its own within a second, and one that
//! never writes anything at all, both end up with `child.wait()` blocked
//! forever in the relay thread once `shutdown` has dropped the worker's
//! `Running` - not just the silent case. What *is* fixed, and what the
//! name "shutdown" in this module's own tests should not be read as more
//! than: a failed forward (the worker is gone, independent of whether
//! its fds were ever dropped) no longer skips `child.wait()` for a
//! program that ends on its own regardless. See `spawn_relay`'s own
//! comment for why dropping those handles does not deliver a reliable
//! end, and for a way to actually end a program without `unsafe`.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use steg_kernel::{CommandMark, Terminal};
use steg_sdk::{PaneId, Screen};

use crate::error::Error;
use crate::pty::{LaunchSpec, ProgramLauncher};

/// doc-9 R15's default scrollback, in lines. Not exposed as a parameter
/// here: nothing in this task's acceptance criteria needs a different
/// value, and TASK-1.74 (scrollback identity and paging) is where that
/// would be revisited.
const DEFAULT_SCROLLBACK_LINES: usize = 10_000;

/// A new pane's size when nothing has ever declared one for its tab
/// (TASK-1.76 AC #3) - no client has reported a real size yet (no
/// `client` dispatch method exists), so this is a stated literal, not a
/// computed guess.
pub const DEFAULT_PANE_SIZE: (u16, u16) = (80, 24);

/// How long [`PaneHandle::cwd`] waits for the pane's worker thread to
/// answer before giving up (TASK-1.76 §2) - bounded, so a worker mid-
/// `vt_write` on a large flood (R19's own worst case) never blocks a
/// caller holding `Runtime`'s one lock indefinitely.
const CWD_LOOKUP_TIMEOUT: Duration = Duration::from_millis(200);

/// How long [`PaneHandle::stop`] waits for a running program to actually
/// end, once asked, before giving up on *this* pane and moving on
/// (review finding 3; `Runtime::stop`, the server stopping, not
/// `--force` on one pane). A program that ignores the one signal sent -
/// `SIGHUP` by pid, via `clone_killer()` - is not covered here: no grace
/// period, no escalation to a stronger signal. That is exactly TASK-1.78's
/// own job (a single pane's forced close, with its own grace period and
/// what a client is told); this bound exists only so one unresponsive
/// pane cannot keep the whole server from finishing a stop.
pub(crate) const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// How long the worker waits to join a run's relay thread once its own
/// `Exited` has arrived - short, because by that point the relay thread
/// is already on its way out (`Exited` is the last thing it sends before
/// returning): this is a hard guarantee, not a real wait for anything to
/// happen.
const RELAY_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Joins `handle` with a bound, rather than waiting on it forever -
/// `thread::JoinHandle` has no built-in timeout, so a watchdog thread does
/// the blocking join and this blocks on a bounded `recv_timeout` for it
/// instead, never a sleep loop. Production code (not test-only): both
/// [`PaneHandle::stop`] and the worker's own handling of
/// [`PaneCommand::Exited`] need a bounded join, for the same reason this
/// module's own tests already did before either existed.
pub(crate) fn join_with_bound(handle: thread::JoinHandle<()>, timeout: Duration) -> bool {
    let (done_tx, done_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = handle.join();
        let _ = done_tx.send(());
    });
    done_rx.recv_timeout(timeout).is_ok()
}

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
    /// A typed command started producing output in `pane` (OSC 133 `C`;
    /// [`CommandMark::OutputStarted`]; TASK-1.84).
    CommandOutputStarted {
        /// The pane whose terminal reported the mark.
        pane: PaneId,
    },
    /// A typed command ended in `pane` (OSC 133 `D`; `CommandMark::Ended`;
    /// TASK-1.84), with `exit_code` exactly as the mark carried it (`None`
    /// for a bare `D`, or one the engine's own parser could not read).
    CommandEnded {
        /// The pane whose terminal reported the mark.
        pane: PaneId,
        /// The command's exit code, if the shell reported one.
        exit_code: Option<i32>,
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
    /// Read the terminal's live working directory, as last reported by a
    /// real OSC 7 (doc-9 R16), back through this reply channel - `None`
    /// if nothing has reported one yet (TASK-1.76 §2, rule B).
    Cwd(Sender<Option<String>>),
    /// Resume with a freshly spawned program (a restart): the pieces
    /// [`SpawnedProgram`] splits apart, already spawned on the caller's
    /// thread by [`PaneHandle::restart`].
    Resume {
        /// Writes to the new program's standard input.
        writer: Box<dyn Write + Send>,
        /// Resizes the new pseudo-terminal.
        resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
        /// Signals the new run's process independently of the thread
        /// blocked in its `child.wait()` (see [`Running`]'s own doc).
        killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
        /// The new run's relay thread, joined once its own `Exited`
        /// arrives.
        relay_handle: thread::JoinHandle<()>,
    },
    /// End the worker thread, without touching whatever is running
    /// (`PaneHandle::shutdown`'s own long-named gap: this alone does not
    /// reliably end a still-running program).
    Shutdown,
    /// End the worker thread *and* whatever program is running, reaping
    /// it and joining its relay thread before replying - review finding
    /// 3, `PaneHandle::stop`'s own command.
    Stop(Sender<()>),
    /// Bytes read from the current run's pseudo-terminal (internal: sent
    /// by this run's relay thread, never by a caller).
    PtyBytes(Vec<u8>),
    /// The current run's program ended with this exit code (internal:
    /// sent by this run's relay thread, never by a caller).
    Exited(i32),
}

/// The writer, resize handle, killer and relay thread of whichever
/// program is currently running in a pane, if any. `killer` is
/// `child.clone_killer()` (`portable-pty`'s own safe code, confirmed
/// from its 0.9.0 source: `Child: ChildKiller`, and the clone is
/// `Send + Sync`, independent of every pty descriptor), taken *before*
/// `child` itself moves into [`spawn_relay`] - the only way to signal
/// the process from the pane's own worker thread while the relay thread
/// may be blocked in `child.wait()` on the same `child`. `relay_handle`
/// is that run's relay thread, joined (bounded, [`join_with_bound`])
/// once its own [`PaneCommand::Exited`] arrives - not discarded, so a
/// caller can know every thread this run started has actually ended,
/// not merely that a message arrived.
struct Running {
    writer: Box<dyn Write + Send>,
    resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
    killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
    relay_handle: thread::JoinHandle<()>,
}

/// A handle to a pane's worker thread: the pane's whole program-lifecycle
/// surface for later tasks. `send_input`/`snapshot` are what an attached
/// stream calls (TASK-1.47); `resize` is the one call that keeps the
/// kernel `Terminal` and the pseudo-terminal in step (TASK-1.42).
/// `worker` is the worker thread's own handle, joined by
/// [`PaneHandle::stop`] - not by [`PaneHandle::shutdown`], which stays
/// fire-and-forget exactly as it already was.
pub struct PaneHandle {
    commands: Sender<PaneCommand>,
    launcher: Arc<dyn ProgramLauncher>,
    worker: thread::JoinHandle<()>,
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

    /// Reads the pane's live working directory - the directory its own
    /// shell last reported with a real OSC 7 (doc-9 R16), which can
    /// differ from the directory it *started* in once the program `cd`s
    /// (TASK-1.76 §2, rule B). `Ok(None)` means nothing has reported one
    /// yet, not that the pane is gone.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneGone`] if the pane's worker thread is no
    /// longer there, or did not answer within
    /// [`CWD_LOOKUP_TIMEOUT`] - treated the same way by every caller
    /// today (TASK-1.76's `Runtime` falls back to the login shell's home
    /// directory either way), but returned as its own error rather than
    /// folded into `Ok(None)`, so a caller that ever needs to tell "no
    /// cwd yet" apart from "the worker didn't answer" still can.
    pub fn cwd(&self) -> Result<Option<String>, Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(PaneCommand::Cwd(reply_tx))
            .map_err(|_| Error::PaneGone)?;
        reply_rx
            .recv_timeout(CWD_LOOKUP_TIMEOUT)
            .map_err(|_| Error::PaneGone)
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
        let killer = spawned.child.clone_killer();
        let relay_handle = spawn_relay(spawned.reader, spawned.child, self.commands.clone());
        self.commands
            .send(PaneCommand::Resume {
                writer: spawned.writer,
                resize: spawned.resize,
                killer,
                relay_handle,
            })
            .map_err(|_| Error::PaneGone)
    }

    /// Ends the pane's worker thread. Whatever is currently running loses
    /// its standard input and resize handle when this is dropped, but
    /// that alone does not reliably end a still-running program on this
    /// platform (see the module doc's known gap and `spawn_relay`'s own
    /// comment) - a program that has already ended on its own is reaped
    /// cleanly; one that has not keeps running, with its relay thread
    /// blocked for as long as it does. Killing the process outright
    /// (`--force`) is [`PaneHandle::stop`]'s job when the whole server is
    /// stopping, and TASK-1.78's for one pane.
    pub fn shutdown(self) {
        let _ = self.commands.send(PaneCommand::Shutdown);
    }

    /// Ends whatever program is running (a real `SIGHUP` by pid, via the
    /// run's own `clone_killer` - safe code, no escalation, see
    /// [`STOP_TIMEOUT`]'s own doc), reaps it, joins its relay thread, then
    /// ends and joins the worker thread itself - review finding 3. Used by
    /// `Runtime::stop` (the server stopping), not by a single pane's
    /// forced close (TASK-1.78).
    ///
    /// Bounded: if the running program ignores the signal, this still
    /// returns within [`STOP_TIMEOUT`] plus the time a bounded join needs,
    /// rather than blocking the caller forever - at the cost of leaving
    /// that one pane's worker and relay threads running, named here rather
    /// than hidden, since nothing *of this task's own* can end a program
    /// that traps the one signal it sends.
    pub fn stop(self) {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self.commands.send(PaneCommand::Stop(reply_tx)).is_err() {
            // The worker was already gone - nothing to join.
            return;
        }
        let _ = reply_rx.recv_timeout(STOP_TIMEOUT);
        let _ = join_with_bound(self.worker, STOP_TIMEOUT);
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
    // The killer is cloned *before* `child` moves into `spawn_relay`,
    // which is what makes a later `stop`/`restart` able to signal the
    // process independently of the relay thread that will block in
    // `child.wait()` (see `Running`'s own doc; review finding 3).
    let killer = spawned.child.clone_killer();
    let relay_handle = spawn_relay(spawned.reader, spawned.child, commands_tx.clone());

    let initial = Running {
        writer: spawned.writer,
        resize: spawned.resize,
        killer,
        relay_handle,
    };

    let worker = thread::Builder::new()
        .name(format!("steg-pane-{pane}"))
        .spawn(move || worker_loop(commands_rx, cols, rows, Some(initial), pane, events))
        .map_err(Error::Io)?;

    Ok(PaneHandle {
        commands: commands_tx,
        launcher,
        worker,
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
    // Set only while a `Stop` is waiting for the current run to actually
    // end (review finding 3) - `None` the rest of the time, including
    // whenever nothing is running at all.
    let mut stopping: Option<Sender<()>> = None;

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
            PaneCommand::Cwd(reply) => {
                let _ = reply.send(terminal.cwd().map(str::to_string));
            }
            PaneCommand::Resume {
                writer,
                resize,
                killer,
                relay_handle,
            } => {
                current = Some(Running {
                    writer,
                    resize,
                    killer,
                    relay_handle,
                });
            }
            PaneCommand::PtyBytes(bytes) => {
                terminal.write(&bytes);
                drain_command_marks(&mut terminal, pane, &events);
            }
            PaneCommand::Exited(exit_code) => {
                if let Some(running) = current.take() {
                    // Bounded, not a bare `join`: by the time this run's
                    // own relay thread sent this message, it is already
                    // on its way out (this is the very last thing
                    // `spawn_relay`'s closure does before returning) -
                    // this just makes that a hard guarantee rather than
                    // a timing assumption, for every exit, not only
                    // during `stop`.
                    let _ = join_with_bound(running.relay_handle, RELAY_JOIN_TIMEOUT);
                }
                let _ = events.send(PaneEvent::Exited { pane, exit_code });
                if let Some(reply) = stopping.take() {
                    let _ = reply.send(());
                    break;
                }
            }
            PaneCommand::Shutdown => break,
            PaneCommand::Stop(reply) => {
                if let Some(running) = &mut current {
                    // Safe code inside `portable-pty`, not this crate:
                    // `ProcessSignaller::kill` sends `SIGHUP` by pid
                    // (confirmed from its 0.9.0 source), independent of
                    // every pty descriptor. No escalation - see
                    // `STOP_TIMEOUT`'s own doc for the line this draws.
                    let _ = running.killer.kill();
                    stopping = Some(reply);
                } else {
                    let _ = reply.send(());
                    break;
                }
            }
        }
    }
}

/// Turns every [`CommandMark`] [`Terminal::take_command_marks`] has queued
/// since the last drain into a [`PaneEvent`], sent on the same `events`
/// channel [`PaneEvent::Exited`] already uses (TASK-1.84 §1) - not a new
/// command `Runtime` would have to poll for (the shape
/// [`PaneCommand::Cwd`] uses), which would give a mark no ordering
/// relationship to this pane's own eventual `Exited` at all. Because this
/// pane's one worker thread is the sole sender of every `PaneEvent` it
/// ever produces, and [`spawn_relay`] only ever sends `Exited` after its
/// own `PtyBytes` forwarding has ended, mpsc's per-sender FIFO order means
/// a mark drained here can never reach the other end of the channel after
/// that pane's own exit.
///
/// Called only from the `PtyBytes` arm, after [`Terminal::write`] - never
/// after [`Terminal::resize`], even though `resize` also calls the
/// engine's own event drain internally: `resize` only ever queues
/// `Event::PtyWrite` (an in-band size report), never `Event::
/// SemanticPrompt`, which only ever comes from parsing a program's
/// *written* bytes. So a resize can never produce a `CommandMark`, and
/// any mark a preceding `write` produced is already drained by the very
/// next call here regardless of whether a resize runs in between.
fn drain_command_marks(terminal: &mut Terminal, pane: PaneId, events: &Sender<PaneEvent>) {
    for mark in terminal.take_command_marks() {
        let event = match mark {
            CommandMark::OutputStarted => PaneEvent::CommandOutputStarted { pane },
            CommandMark::Ended { exit_code } => PaneEvent::CommandEnded { pane, exit_code },
        };
        let _ = events.send(event);
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
        // to. That much is fixed and holds regardless of why the receiver
        // is gone. But reaching this line at all still depends on the
        // program ending on its own: probed directly (not assumed, and
        // not only for a silent program - see this module's test module),
        // dropping the worker thread's `Running` (its writer and the
        // resize closure's `MasterPty`, which is what `PaneHandle::shutdown`
        // does) does not reliably end a program that is still running,
        // whether it writes nothing at all or was about to finish within
        // a second anyway - both leave this thread blocked in the `read`
        // above, or (having reached here) blocked in `child.wait()`
        // itself, for as long as the program lives. The pty's hang-up
        // does not reach it on this platform: the relay's own cloned
        // reader (a genuinely separate descriptor from
        // `MasterPty::try_clone_reader`) keeps the pty's master side from
        // ever being fully closed, which is what the hang-up condition
        // actually requires - and even where that drop does produce a
        // visible effect (a terminal's own line discipline echoing the
        // `\n` plus EOF character `UnixMasterWriter::drop` writes), it is
        // not the hang-up and does not end the program. This task's plan
        // covers neither a kill nor giving this thread a descriptor it
        // can close itself; DRAFT-4 ("Server: start a pane's program from
        // the API, and end it on a forced close") AC #4 already names
        // ending a program "producing output" and one that "prints
        // nothing" as one requirement, which is exactly this. Confirmed
        // by reading `portable-pty` 0.9.0's own source (not just its
        // docs): `Child: ChildKiller`, and on Unix `Child::clone_killer`
        // hands back a `ProcessSignaller` whose own `kill` sends `SIGHUP`
        // by pid (`libc::kill`, inside `portable-pty`, not this crate) -
        // independent of every pty descriptor, so it would end a program
        // that does not trap `SIGHUP` without the fd-ownership rework
        // above, and with no `unsafe` of this crate's own (unlike
        // `std::process::Child`'s own `ChildKiller::kill`, this clone
        // does not escalate to a stronger signal if `SIGHUP` alone is
        // not enough). Taking that clone before handing `child` to this
        // function is DRAFT-4's to add, not this task's: killing a
        // running program is this plan's own named exclusion.
        let exit_code = match child.wait() {
            Ok(status) => i32::try_from(status.exit_code()).unwrap_or(i32::MAX),
            // Deliberately silent, not a bug: `wait()` on a `Child` this
            // thread itself holds the only handle to can fail only if the
            // OS has nothing left to report (the process was already
            // reaped by something outside this program's own model, or a
            // transient OS error) - there is no real exit code to report
            // either way, and `PaneEvent` has no variant for "unknown"
            // that isn't itself a guessed code, so the pane is left in
            // whatever state it was. Observable as an absence (no
            // `Exited` event ever arrives for this run) rather than a
            // fabricated one.
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
            // A working `PATH` is required (TASK-1.87):
            // `RealLauncher::spawn`'s own `.env_clear()` means an empty
            // `env` cannot resolve a relative program name like `"sh"` -
            // this test's own ambient `PATH` is fine to read here (a
            // test, not production code).
            env: vec![(
                std::ffi::OsString::from("PATH"),
                std::env::var_os("PATH").unwrap_or_default(),
            )],
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
    fn a_forward_failing_once_the_worker_is_gone_still_reaps_a_program_that_ends_on_its_own() {
        // Reproduces the review's first scenario directly: the pane's
        // worker is "gone" (its receiver dropped, which is all this
        // simulates - not `PaneHandle::shutdown`'s own fd-dropping,
        // which this module's doc comment covers separately and which
        // this test does not exercise) while `sh -c 'sleep 1; echo
        // done'` is still running - the exact condition under which
        // `spawn_relay`'s send used to fail and return early, skipping
        // `child.wait()` and leaving a zombie. This program ends on its
        // own a second later either way, which is what lets this assert
        // cleanly.
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
