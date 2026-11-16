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
//! **Ending a running program** (TASK-1.78, closing the gap TASK-1.40/1.76
//! left named): [`PaneHandle::signal_hangup`]/[`signal_kill`][PaneHandle::signal_kill]
//! send a real `SIGHUP`/`SIGKILL` to the run's whole process group,
//! straight to the OS (`crate::signal::send`) via the pid [`start`]/
//! [`PaneHandle::restart`] read off the child before it moved into
//! [`spawn_relay`] - not through this module's own command queue, and not
//! through `portable_pty::ChildKiller` (its own clone sends `SIGHUP` by
//! pid alone, with no escalation and no group - confirmed from
//! `portable-pty` 0.9.0's `lib.rs`, correcting both TASK-1.40's and
//! TASK-1.76's notes, which cited `unix.rs`). [`PaneHandle::await_exit`]
//! is the other half: it waits for the run's own real exit (whether it
//! came from one of those signals or the program ending on its own) and,
//! once it has, ends the worker thread too - `Runtime::force_end`
//! orchestrates all three across however many panes are in a forced
//! close's scope, with one shared grace window, not one per pane.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use nix::sys::signal::Signal;
use steg_kernel::Terminal;
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

/// How long the worker waits to join a run's relay thread once its own
/// `Exited` has arrived - short, because by that point the relay thread
/// is already on its way out (`Exited` is the last thing it sends before
/// returning): this is a hard guarantee, not a real wait for anything to
/// happen.
const RELAY_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

/// `0` is never a real pid (the kernel reserves it), so it is the shared
/// atomic's own sentinel for "no program is running right now" -
/// `PaneHandle::signal_hangup`/`signal_kill` read it directly, with no
/// channel round-trip through the worker thread's own command queue (see
/// the module doc): that queue is the same one a run's relay thread pushes
/// `PtyBytes` into, and a flooding program (`yes`) could otherwise delay a
/// signal command behind its own backlog indefinitely.
const NO_PROGRAM: u64 = 0;

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
    /// thread by [`PaneHandle::restart`] - which also updates the shared
    /// pid atomic itself, synchronously, before this is ever sent (see
    /// [`PaneHandle::restart`]'s own doc for why a pid is not carried
    /// here too).
    Resume {
        /// Writes to the new program's standard input.
        writer: Box<dyn Write + Send>,
        /// Resizes the new pseudo-terminal.
        resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
        /// The new run's relay thread, joined once its own `Exited`
        /// arrives.
        relay_handle: thread::JoinHandle<()>,
    },
    /// End the worker thread, without touching whatever is running
    /// (`PaneHandle::shutdown`'s own long-named gap: this alone does not
    /// reliably end a still-running program).
    Shutdown,
    /// Replies once the current run has actually ended - immediately,
    /// **and ends the worker thread too**, if nothing is running right
    /// now (TASK-1.78 §1: the immediate-reply branch must still break the
    /// loop, not only the branch that waits for `Exited`, or a run that
    /// ends on its own before this is even read leaves the worker thread
    /// never told to stop). Sends no signal of its own: `PaneHandle::
    /// signal_hangup`/`signal_kill` are separate, so a caller forcing
    /// several panes at once can signal all of them before waiting on
    /// any (`Runtime::force_end`).
    AwaitExit(Sender<()>),
    /// Bytes read from the current run's pseudo-terminal (internal: sent
    /// by this run's relay thread, never by a caller).
    PtyBytes(Vec<u8>),
    /// The current run's program ended with this exit code (internal:
    /// sent by this run's relay thread, never by a caller).
    Exited(i32),
}

/// The writer, resize handle and relay thread of whichever program is
/// currently running in a pane, if any - no killer here: TASK-1.78
/// replaces `portable_pty::ChildKiller` (its own clone sends `SIGHUP` by
/// pid alone, with no escalation and no group) with the pid in
/// `PaneHandle`/`worker_loop`'s own shared atomic, read directly by
/// `crate::signal::send`, never through this struct or the command queue
/// (see the module doc). `relay_handle` is that run's relay thread, joined
/// (bounded, [`join_with_bound`]) once its own [`PaneCommand::Exited`]
/// arrives - not discarded, so a caller can know every thread this run
/// started has actually ended, not merely that a message arrived.
struct Running {
    writer: Box<dyn Write + Send>,
    resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
    relay_handle: thread::JoinHandle<()>,
}

/// A handle to a pane's worker thread: the pane's whole program-lifecycle
/// surface for later tasks. `send_input`/`snapshot` are what an attached
/// stream calls (TASK-1.47); `resize` is the one call that keeps the
/// kernel `Terminal` and the pseudo-terminal in step (TASK-1.42). `worker`
/// is the worker thread's own handle, joined by [`PaneHandle::join_worker`],
/// not by [`PaneHandle::shutdown`], which stays fire-and-forget exactly
/// as it already was. `pid` is shared with the worker thread (`Arc` over
/// one `AtomicU64`, [`NO_PROGRAM`] meaning nothing is running), read
/// directly by [`PaneHandle::signal_hangup`]/[`signal_kill`][PaneHandle::signal_kill],
/// never through `commands`, so neither can be delayed behind a flooding
/// run's own backlog of `PtyBytes` on that same queue (TASK-1.78 §1).
pub struct PaneHandle {
    commands: Sender<PaneCommand>,
    launcher: Arc<dyn ProgramLauncher>,
    worker: thread::JoinHandle<()>,
    pid: Arc<AtomicU64>,
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
        // Updates the shared atomic *before* `Resume` is even sent -
        // synchronously, on this caller's own thread, the same as `start`
        // (below) sets the initial value before the worker thread exists
        // at all. A signal sent in the brief window before the worker
        // itself processes `Resume` still reaches a real, already-spawned
        // process, so this ordering is never observably wrong, only ever
        // slightly ahead of the worker's own bookkeeping.
        self.pid.store(
            u64::from(spawned.child.process_id().unwrap_or(0)),
            Ordering::SeqCst,
        );
        let relay_handle = spawn_relay(spawned.reader, spawned.child, self.commands.clone());
        self.commands
            .send(PaneCommand::Resume {
                writer: spawned.writer,
                resize: spawned.resize,
                relay_handle,
            })
            .map_err(|_| Error::PaneGone)
    }

    /// Ends the pane's worker thread. Whatever is currently running loses
    /// its standard input and resize handle when this is dropped, but
    /// that alone does not reliably end a still-running program on this
    /// platform (see `spawn_relay`'s own comment) - a program that has
    /// already ended on its own is reaped cleanly; one that has not keeps
    /// running, with its relay thread blocked for as long as it does.
    /// Ending it outright is [`PaneHandle::signal_hangup`]/`signal_kill`'s
    /// job (TASK-1.78).
    pub fn shutdown(self) {
        let _ = self.commands.send(PaneCommand::Shutdown);
    }

    /// Signals the run's own process group (if one is running right now)
    /// with the polite signal, `SIGHUP` - straight to the OS
    /// (`crate::signal::send`), reading the shared pid atomic directly, so
    /// a backlog of `PtyBytes` the worker has not drained yet (a flooding
    /// program) cannot delay this (module doc, TASK-1.78 §1). Fire-and-
    /// forget: use [`PaneHandle::await_exit`] to learn whether it worked.
    pub(crate) fn signal_hangup(&self) {
        if let Some(pid) = self.live_pid() {
            crate::signal::send(pid, Signal::SIGHUP);
        }
    }

    /// Same, `SIGKILL` - the escalation, after a grace period, for a run
    /// that ignored [`PaneHandle::signal_hangup`] (TASK-1.78 §2).
    pub(crate) fn signal_kill(&self) {
        if let Some(pid) = self.live_pid() {
            crate::signal::send(pid, Signal::SIGKILL);
        }
    }

    /// The pid of whichever program is running right now, if any - `None`
    /// when the shared atomic reads [`NO_PROGRAM`] (nothing running, or
    /// the worker has already cleared it on `Exited`).
    fn live_pid(&self) -> Option<u32> {
        let pid = self.pid.load(Ordering::SeqCst);
        (pid != NO_PROGRAM).then(|| u32::try_from(pid).unwrap_or(0))
    }

    /// Waits up to `timeout` for the current run to actually end - `true`
    /// if it did within that bound (including "nothing was running at
    /// all", which answers at once), `false` on a real timeout. A send
    /// failure (the worker is already gone) counts as "ended" too, not a
    /// timeout: there is nothing left to wait for either way.
    pub(crate) fn await_exit(&self, timeout: Duration) -> bool {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .commands
            .send(PaneCommand::AwaitExit(reply_tx))
            .is_err()
        {
            return true;
        }
        reply_rx.recv_timeout(timeout).is_ok()
    }

    /// Consumes the handle: joins its worker thread, bounded. Only
    /// meaningful once [`PaneHandle::await_exit`] has returned `true` for
    /// this handle - the worker's own loop only breaks once it has
    /// answered an `AwaitExit` (module doc, TASK-1.78 §1), so calling this
    /// first can time out on a worker still legitimately serving other
    /// commands, which is not a bug in this method itself.
    pub(crate) fn join_worker(self, timeout: Duration) -> bool {
        join_with_bound(self.worker, timeout)
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
    // The pid is read *before* `child` moves into `spawn_relay`, the same
    // ordering `clone_killer` used to need and for the same reason: once
    // `child` is inside the relay thread's own `child.wait()`, nothing
    // outside that thread can reach it through `child` itself. Stored in
    // the atomic right away, on this thread, before the worker thread
    // (which shares the same `Arc`) even exists - TASK-1.78 §1.
    let pid = Arc::new(AtomicU64::new(u64::from(
        spawned.child.process_id().unwrap_or(0),
    )));
    let relay_handle = spawn_relay(spawned.reader, spawned.child, commands_tx.clone());

    let initial = Running {
        writer: spawned.writer,
        resize: spawned.resize,
        relay_handle,
    };

    let worker_pid = Arc::clone(&pid);
    let worker = thread::Builder::new()
        .name(format!("steg-pane-{pane}"))
        .spawn(move || {
            worker_loop(
                commands_rx,
                cols,
                rows,
                Some(initial),
                pane,
                events,
                worker_pid,
            )
        })
        .map_err(Error::Io)?;

    Ok(PaneHandle {
        commands: commands_tx,
        launcher,
        worker,
        pid,
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
    pid: Arc<AtomicU64>,
) {
    let Ok(mut terminal) = Terminal::new(cols, rows, DEFAULT_SCROLLBACK_LINES) else {
        return;
    };
    let mut current = initial;
    // Set only while an `AwaitExit` is waiting for the current run to
    // actually end (TASK-1.78 §1) - `None` the rest of the time,
    // including whenever nothing is running at all.
    let mut awaiting_exit: Option<Sender<()>> = None;

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
                relay_handle,
            } => {
                // The shared `pid` atomic was already updated by
                // `PaneHandle::restart`, synchronously, before this was
                // even sent - nothing to do with it here.
                current = Some(Running {
                    writer,
                    resize,
                    relay_handle,
                });
            }
            PaneCommand::PtyBytes(bytes) => terminal.write(&bytes),
            PaneCommand::Exited(exit_code) => {
                if let Some(running) = current.take() {
                    // Bounded, not a bare `join`: by the time this run's
                    // own relay thread sent this message, it is already
                    // on its way out (this is the very last thing
                    // `spawn_relay`'s closure does before returning) -
                    // this just makes that a hard guarantee rather than
                    // a timing assumption, for every exit, not only
                    // while something is awaiting it.
                    let _ = join_with_bound(running.relay_handle, RELAY_JOIN_TIMEOUT);
                }
                // Cleared *before* the event goes out: a stale signal
                // sent after this point (nothing left to reach the pid
                // it last knew about, in case the OS ever reused it) can
                // only ever hit nothing, never a program this pane no
                // longer owns (TASK-1.78 §1).
                pid.store(NO_PROGRAM, Ordering::SeqCst);
                let _ = events.send(PaneEvent::Exited { pane, exit_code });
                if let Some(reply) = awaiting_exit.take() {
                    let _ = reply.send(());
                    break;
                }
            }
            PaneCommand::Shutdown => break,
            PaneCommand::AwaitExit(reply) => {
                if current.is_some() {
                    awaiting_exit = Some(reply);
                } else {
                    // Nothing running right now: reply at once, *and*
                    // break the loop - the fix TASK-1.78 §1 is specifically
                    // about. Without this, a run that ended on its own
                    // before this command was even read leaves the worker
                    // thread with nothing left to tell it to stop, and a
                    // caller's own `PaneHandle::join_worker` would time
                    // out on a thread that is never coming back.
                    let _ = reply.send(());
                    break;
                }
            }
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
        // to. That much holds regardless of why the receiver is gone, but
        // reaching this line at all still depends on the program ending
        // on its own - which is why ending a still-running program is not
        // this function's own job. Probed directly, not assumed (TASK-
        // 1.40/1.76): dropping the worker thread's `Running` (its writer
        // and the resize closure's `MasterPty`, which is what `PaneHandle::
        // shutdown` does) does not reliably hang this platform's pty up,
        // whether the program writes nothing at all or was about to
        // finish within a second anyway - the relay's own cloned reader
        // (a genuinely separate descriptor from `MasterPty::
        // try_clone_reader`) keeps the pty's master side from ever being
        // fully closed, which is what a real hang-up needs. TASK-1.78
        // does not fix *this* - it ends a running program a different
        // way entirely, independent of any pty descriptor: `PaneHandle::
        // signal_hangup`/`signal_kill` (`crate::signal::send`) signal the
        // process directly, by the pid `start`/`restart` read off `child`
        // before it ever reached this function, so nothing here has to
        // change for that to reach a program still blocked in the `read`
        // above or, having reached this point, in `child.wait()` itself.
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
    // it on its own - its relay thread blocked in `read` for as long as
    // the program lives, holding the pty's last master-side descriptor
    // open - *through this path*. `PaneHandle::shutdown` alone still does
    // not end a running program. TASK-1.78 ends one a different way
    // entirely, bypassing this problem rather than solving it: `signal_
    // hangup`/`signal_kill` (`crate::signal::send`) signal the process
    // directly by pid, independent of every pty descriptor, so the relay
    // thread's own open reader clone - the exact thing that defeats the
    // natural hang-up above - never matters to it. No fd-ownership rework
    // of `RealLauncher` was needed after all, and no `unsafe`.
}

/// `PaneHandle`'s own ending surface (TASK-1.78): `signal_hangup`/
/// `signal_kill`/`await_exit`/`join_worker`, through the real public API,
/// with real processes - `pane::tests` above covers `spawn_relay` directly
/// (a focused check beneath `PaneHandle`); this covers the handle itself,
/// the layer `Runtime::force_end` actually calls.
#[cfg(test)]
mod ending_tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::start;
    use crate::pty::{LaunchSpec, RealLauncher};

    /// Bounded, never a bare wait - the same reasoning every other real-
    /// process wait in this crate gives.
    const EVENT_TIMEOUT: Duration = Duration::from_secs(10);

    fn real_spec(argv: Vec<&str>) -> LaunchSpec {
        LaunchSpec {
            argv: argv.into_iter().map(str::to_string).collect(),
            cwd: std::env::temp_dir().to_str().unwrap().to_string(),
            env: Vec::new(),
            size: (80, 24),
        }
    }

    /// The fix TASK-1.78 §1 is specifically about: a run that ends on its
    /// own, with nothing yet asking `await_exit` about it, must still
    /// leave the worker thread able to end once something does -
    /// `AwaitExit`'s own "nothing is running, reply at once" branch has to
    /// break the loop too, not only the branch that was already waiting.
    /// Proven the strong way: not just that `await_exit` returns `true`,
    /// but that the worker thread *actually ends* (`join_worker` succeeds,
    /// bounded) - a weaker test asserting only the reply would have missed
    /// exactly the bug this is named for, which left a leaked worker
    /// thread behind a successful-looking reply.
    ///
    /// Negative control, run by hand against this exact test (not
    /// committed as a toggle): removing the `break` from `AwaitExit`'s
    /// immediate-reply arm in `worker_loop` makes `join_worker` below time
    /// out (`false`) instead of succeeding - recorded on the task.
    #[test]
    fn await_exit_after_the_program_already_ended_still_ends_the_worker_thread() {
        let (events_tx, events_rx) = mpsc::channel();
        let handle = start(
            real_spec(vec!["sh", "-c", "exit 0"]),
            steg_sdk::PaneId::from_raw(1),
            Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();

        // Wait for the real, natural exit first - by the time this
        // arrives, the worker has already processed `Exited` and set
        // `current` back to `None`, which is the exact precondition
        // `AwaitExit`'s immediate-reply branch needs to be exercised by
        // the call below, rather than its "something is running" branch.
        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the real shell's own exit, within the bound");

        assert!(
            handle.await_exit(EVENT_TIMEOUT),
            "await_exit must reply once nothing is running, not time out"
        );
        assert!(
            handle.join_worker(EVENT_TIMEOUT),
            "the worker thread must actually end, not just reply - the bug \
             this test is named for left a worker thread running forever \
             behind a reply that looked correct on its own"
        );
    }

    /// `signal_hangup` ends a real, non-trapping program on its own -
    /// TASK-1.78 §0's own silent/continuous-output experiments, through
    /// `PaneHandle`'s real public surface rather than a scratch script.
    #[test]
    fn signal_hangup_ends_a_real_program_through_the_public_handle() {
        let (events_tx, events_rx) = mpsc::channel();
        let handle = start(
            real_spec(vec!["sh", "-c", "exec sleep 1000"]),
            steg_sdk::PaneId::from_raw(2),
            Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();

        handle.signal_hangup();

        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("SIGHUP must end a plain sleep");
        assert!(handle.await_exit(EVENT_TIMEOUT));
        assert!(handle.join_worker(EVENT_TIMEOUT));
    }

    /// `signal_hangup` alone does not end a program that traps it, but
    /// `signal_kill` does - TASK-1.78 §0's own trapping-program
    /// experiment, reproduced here through the public handle. Proves the
    /// escalation is real, not merely documented.
    ///
    /// Waits for a real marker file the shell writes *after* installing
    /// its own trap, before sending anything - found necessary by probing,
    /// not assumed: without it, this test raced `signal_hangup` against
    /// the newly forked shell's own first few instructions and, under
    /// heavy CPU contention (confirmed by running it in a tight loop, not
    /// just once), the signal could arrive *before* the shell had reached
    /// its own `trap` builtin at all - hitting the inherited default
    /// disposition (terminate) instead of the trap this test means to
    /// exercise. A real, timing-sensitive race in the test's own fixture,
    /// not in `signal_hangup`/`signal_kill` themselves.
    #[test]
    fn signal_kill_ends_a_program_that_traps_the_polite_signal() {
        let marker = std::env::temp_dir().join(format!(
            "steg-signal-trap-ready-{}-{}.marker",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_file(&marker);
        let (events_tx, events_rx) = mpsc::channel();
        let handle = start(
            real_spec(vec![
                "sh",
                "-c",
                &format!("trap '' HUP; touch {}; exec sleep 1000", marker.display()),
            ]),
            steg_sdk::PaneId::from_raw(3),
            Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();

        let start_wait = std::time::Instant::now();
        while !marker.exists() {
            assert!(
                start_wait.elapsed() < Duration::from_secs(10),
                "the shell never reached its own trap+marker"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = std::fs::remove_file(&marker);

        handle.signal_hangup();
        assert!(
            !handle.await_exit(Duration::from_millis(500)),
            "a program trapping SIGHUP must not end on that signal alone"
        );

        handle.signal_kill();
        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("SIGKILL cannot be trapped");
        assert!(handle.await_exit(EVENT_TIMEOUT));
        assert!(handle.join_worker(EVENT_TIMEOUT));
    }

    /// `signal_hangup` reaches the whole process group, not only the pane's
    /// own direct child - TASK-1.78 §0's own grandchild experiment, through
    /// the public handle: a backgrounded child of a non-interactive shell
    /// shares its pgid (nothing here calls `setpgid`), so the one signal
    /// this sends to the pane's own pid (which `portable-pty`'s own
    /// `setsid()` in `pre_exec` makes equal to that pgid) ends it too.
    #[test]
    fn signal_hangup_reaches_a_backgrounded_grandchild_too() {
        let marker = std::env::temp_dir().join(format!(
            "steg-signal-grandchild-{}-{}.marker",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_file(&marker);
        let (events_tx, events_rx) = mpsc::channel();
        let handle = start(
            real_spec(vec![
                "sh",
                "-c",
                &format!("(sleep 1000; touch {}) & exec sleep 1000", marker.display()),
            ]),
            steg_sdk::PaneId::from_raw(4),
            Arc::new(RealLauncher),
            events_tx,
        )
        .unwrap();

        handle.signal_hangup();

        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the main process must end");
        assert!(handle.await_exit(EVENT_TIMEOUT));
        assert!(handle.join_worker(EVENT_TIMEOUT));

        // Not a race against the `touch`: the backgrounded job is itself
        // `sleep 1000`, so if it survived the group signal, it would not
        // reach its own `touch` for another ~1000s regardless of how
        // long this waits - a killed job never reaches it, at all, not
        // "not yet". This short, bounded sleep is only to let the OS
        // finish actually delivering/acting on the signal before the
        // check below reads the filesystem, the same margin `runtime.rs`'s
        // own `stop_is_a_no_op_for_a_pane_whose_program_has_already_ended`
        // test gives for the same reason.
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            !marker.exists(),
            "the backgrounded grandchild must have been signalled too, \
             never reaching its own touch"
        );
        let _ = std::fs::remove_file(&marker);
    }
}
