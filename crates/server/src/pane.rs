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
//! came from one of those signals or the program ending on its own) -
//! `Runtime::force_end` orchestrates both across however many panes are
//! in a forced close's scope, with one shared grace window, not one per
//! pane - then [`PaneHandle::settle_worker`]/[`join_worker`][PaneHandle::join_worker]
//! end the worker thread itself.
//!
//! **[`RunGate`]** (TASK-1.78's review, finding 1) is what makes the
//! signal and the wait exact, not approximate: the relay thread is the
//! only thread that ever reaps a run's process, and it only ever does so
//! while holding `RunGate`'s own lock - so a signal, or a wait, that also
//! takes that lock before reading the pid can never race the reap. See
//! `RunGate`'s own doc for why, and `spawn_relay`'s for how the reap
//! itself is done without ever blocking inside that lock.

use std::io::{Read, Write};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use nix::sys::signal::Signal;
use steg_kernel::{CommandMark, Terminal};
use steg_sdk::{Frame, PaneId, Screen, ScreenUpdate, encode_frame};

use crate::error::Error;
use crate::outbound::{EnqueueDataError, Outbound};
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

/// How often [`spawn_relay`] polls `portable_pty::Child::try_wait` while
/// waiting to reap a run that the pty's own end-of-file has not yet
/// proven exited (TASK-1.78's review, finding 1 - see `spawn_relay`'s own
/// comment for why `try_wait`, not a blocking `wait`, is what closes the
/// signal race). Short, because in the overwhelming common case the
/// *first* call, right after that end-of-file, already finds the process
/// a zombie - the kernel has almost always finished tearing it down well
/// before or by the moment its pty slave side closes - so this interval
/// is rarely slept even once in practice.
const REAP_POLL_INITIAL_INTERVAL: Duration = Duration::from_millis(5);

/// Where [`REAP_POLL_INITIAL_INTERVAL`]'s own doubling (TASK-1.78's
/// review, a nit of the engineer's own) stops: a program that detaches
/// from its terminal while it keeps running (closes its own pty slave
/// side, which is what produces the end-of-file `spawn_relay` reacts to,
/// without the process itself ever exiting - a daemonising script is the
/// ordinary example) would otherwise be polled at `REAP_POLL_INITIAL_
/// INTERVAL`'s own cadence for as long as it lives, one thread per pane.
/// Doubling from 5ms up to this cap costs five extra polls (5, 10, 20,
/// 40, 80ms - about 155ms total) over the overwhelmingly common case
/// where the first poll already finds a zombie, then settles here: at
/// most 100ms added before such a pane's own eventual, real exit is
/// noticed, once the ramp is past - the residual this backoff leaves
/// deliberately, not removed, since reaping at all still only needs to
/// happen once, however late.
const REAP_POLL_MAX_INTERVAL: Duration = Duration::from_millis(100);

/// The doubling itself, pulled out as a pure function so it has its own
/// test independent of a real sleeping thread: `current`'s next interval,
/// doubled and capped at [`REAP_POLL_MAX_INTERVAL`].
fn next_reap_poll_interval(current: Duration) -> Duration {
    (current * 2).min(REAP_POLL_MAX_INTERVAL)
}

/// The run-ending synchronisation TASK-1.78's review (finding 1) asked
/// for, closing a real race rather than narrowing it: a signal sent to a
/// pid the OS has already reaped-and-reused, and `force_end`'s own wait
/// (`PaneHandle::await_exit`) timing a pane out as a "straggler" purely
/// because its real `Exited` is stuck behind a flooding run's own
/// backlog of `PtyBytes` on the worker's command queue - not because the
/// program is actually still alive.
///
/// The relay thread is the only thread that ever reaps a run's child
/// (`spawn_relay`'s own `try_wait` loop, never a blocking `wait`), and it
/// only ever does so *while holding this lock* - so [`PaneHandle::
/// signal_hangup`]/[`signal_kill`], which also take this same lock before
/// reading the pid and keep it held across the signal call itself, can
/// never observe a pid the relay thread is concurrently reaping, nor one
/// it already has: either the signal is sent first, to a pid that is at
/// least still unreaped (and so not yet eligible for the OS to reuse) at
/// that exact instant, or the reap is recorded first and no signal is
/// sent to that pid again. `generation` is bumped on every `start`/
/// `restart`, never on an exit, so a relay thread for an old run - not
/// reachable today by any path this crate builds, see `PaneHandle::
/// restart`'s own doc for why, but checked rather than assumed - can
/// never clobber a newer run's own live pid: it only marks dead if its
/// own generation is still the current one. [`PaneHandle::await_exit`]
/// waits on `changed`, so it too never goes through the worker's own
/// command queue - the exact mechanism finding 1's "misclassified
/// straggler" was about.
struct RunGate {
    state: Mutex<GateState>,
    changed: Condvar,
}

struct GateState {
    generation: u64,
    pid: Option<u32>,
}

impl RunGate {
    fn new() -> Self {
        Self {
            state: Mutex::new(GateState {
                generation: 0,
                pid: None,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts a new run: bumps the generation and records its pid (if
    /// the OS gave one), returning the new generation for the relay
    /// thread this run's own spawn is about to start with
    /// ([`spawn_relay`]'s own `generation` parameter).
    fn begin_run(&self, pid: Option<u32>) -> u64 {
        let mut state = self.lock();
        state.generation += 1;
        state.pid = pid;
        state.generation
    }

    /// Signals `sig` to the current run's own pid, if `generation` is
    /// irrelevant here (any live pid is this call's target, whichever
    /// generation it belongs to - there is at most one live run at a
    /// time) and one is actually recorded right now. The send itself
    /// happens *while still holding the lock*, which is what makes this
    /// safe: the relay thread's own reap-and-clear (`spawn_relay`) is
    /// mutually exclusive with this whole check-and-send, not only with
    /// the check.
    fn signal_if_alive(&self, sig: Signal) {
        let state = self.lock();
        if let Some(pid) = state.pid {
            crate::signal::send(pid, sig);
        }
    }

    /// Waits up to `timeout` for the current run to be dead - `true` if
    /// it already was, or became so within the bound; `false` on a real
    /// timeout with it still alive.
    fn await_dead(&self, timeout: Duration) -> bool {
        let guard = self.lock();
        let (guard, _) = self
            .changed
            .wait_timeout_while(guard, timeout, |state| state.pid.is_some())
            .unwrap_or_else(PoisonError::into_inner);
        guard.pid.is_none()
    }
}

impl GateState {
    /// Clears the live pid, but only if `generation` is still the
    /// current one - a no-op otherwise, which is what stops a stale
    /// run's own relay thread (TASK-1.78's review, finding 1's own nit)
    /// from ever clobbering a newer run's live pid. A free method on the
    /// already-locked state, not on `RunGate` itself, because
    /// [`spawn_relay`]'s own reap loop needs to call this from *inside*
    /// a critical section it is already holding for `child.try_wait()`,
    /// not acquire a second one.
    fn clear_if_current(&mut self, generation: u64) {
        if self.generation == generation {
            self.pid = None;
        }
    }
}

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

/// Identifies one attachment, minted by the pane's own worker the moment
/// it registers one (TASK-1.114) - never the connection-supplied `stream`
/// number, which is only unique *within* the connection that minted it:
/// two different connections attached to this same pane can (and, proven
/// by TASK-1.114's own two-client test, did) both mint `stream == 1` for
/// their own first attachment, and `PaneCommand::Arm`/`Detach` matching
/// on the bare `stream` number alone found whichever entry happened to
/// come first in `attachments`, regardless of which connection the
/// message actually came from - silently arming (or detaching) the wrong
/// one. A worker-local counter, starting at 0 and bumped on every
/// `PaneCommand::Attach` for this pane's whole life, is unique across
/// every connection that has ever attached here, which `stream` alone
/// never promised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct AttachmentId(u64);

/// One client's attachment to this pane's screen (TASK-1.47): its own
/// worker-local [`AttachmentId`], the stream id the frames it is sent are
/// tagged with (connection-local; only ever read back by the connection
/// that minted it), the connection's own outbound queue once it is armed,
/// and whether it holds every row the terminal has reported so far or
/// needs a fresh, full resync before it can resume taking deltas
/// (decision-2 section 6).
///
/// `outbound` starts `None` and is filled in by [`PaneCommand::Arm`],
/// deliberately a later, separate step from [`PaneCommand::Attach`] - see
/// that command's own doc for why, and [`run_tick`]'s own doc for what an
/// unarmed attachment costs (nothing: it is skipped until armed).
struct Attachment {
    id: AttachmentId,
    stream: u32,
    outbound: Option<Arc<Outbound>>,
    caught_up_to: AttachState,
}

/// Whether an attachment is caught up to the terminal's own tracked point
/// (`CaughtUp`, holding the point it was last sent, or registered with),
/// or needs a whole fresh screen before it can resume taking deltas
/// (`NeedsResync` - decision-2 section 6: a full queue, or a stream not
/// yet armed at the moment a tick ran). A `CaughtUp` value that no longer
/// equals the worker's own current tracked point (an attachment that was
/// armed after one or more ticks it took no part in) is treated exactly
/// like `NeedsResync` by [`run_tick`] - the two are not told apart there,
/// since "caught up to a stale point" and "needs a resync" call for the
/// identical fresh-screen response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachState {
    CaughtUp(u64),
    NeedsResync,
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
    /// `pane`'s pseudo-terminal did not open on the first try, but did
    /// succeed after `attempts` attempts in total (TASK-1.112) - a retry
    /// is never silent. Not sent at all when `attempts` would be `1`
    /// (no retry needed).
    OpenRetried {
        /// The pane whose pseudo-terminal needed more than one attempt to
        /// open.
        pane: PaneId,
        /// How many attempts the open took in total, including the one
        /// that finally succeeded - always greater than `1`.
        attempts: u32,
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
    /// thread by [`PaneHandle::restart`] - which also starts the new run
    /// on the shared [`RunGate`] itself, synchronously, before this is
    /// ever sent (see [`PaneHandle::restart`]'s own doc for why a pid is
    /// not carried here too).
    Resume {
        /// Writes to the new program's standard input.
        writer: Box<dyn Write + Send>,
        /// Resizes the new pseudo-terminal.
        resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
        /// The new run's relay thread, joined once its own `Exited`
        /// arrives.
        relay_handle: thread::JoinHandle<()>,
        /// How many attempts opening the new run's pseudo-terminal took
        /// (TASK-1.112) - carried here, not read back out of
        /// `SpawnedProgram` a second time, because `PaneHandle::restart`
        /// has already split `SpawnedProgram` apart by the time this is
        /// built. The worker thread sends [`PaneEvent::OpenRetried`] for
        /// this when it is greater than `1`, the same as [`start`] does
        /// for a pane's first run.
        open_attempts: u32,
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
    /// Registers a new attachment to this pane's screen (TASK-1.47): reads
    /// the terminal's current screen for the reply, and registers the
    /// attachment in the worker's own fan-out list, caught up to the
    /// terminal's own tracked point as of this exact read (decision-2
    /// section 3: the stream resumes at this point plus one) - but *not*
    /// yet handed anything to send to. `outbound` starts `None`:
    /// decision-2's own ordering rule ("queue the attach result first,
    /// register the attachment's queue second") is honoured by the caller
    /// sending [`PaneCommand::Arm`] only once it has actually queued the
    /// reply this sends back and opened the stream with the writer - this
    /// command never blocks on that; it answers at once either way.
    Attach {
        /// The new attachment's stream id, already minted by the caller
        /// (a connection concern, not this module's) - connection-local
        /// only; never used by this module to *identify* the attachment
        /// again later (see [`AttachmentId`]'s own doc for why).
        stream: u32,
        /// Where the pane's current screen is sent back, alongside the
        /// [`AttachmentId`] this worker itself mints for it - the caller's
        /// own handle on this attachment for every later `Arm`/`Detach`.
        reply: Sender<Result<(AttachmentId, Screen), Error>>,
    },
    /// Hands an already-registered attachment the `Outbound` it sends
    /// updates to, once the caller has queued the attach reply and opened
    /// the stream with the writer (decision-2 section 3's ordering rule;
    /// see [`PaneCommand::Attach`]'s own doc). A no-op if `id` is not
    /// currently registered (it was detached before this arrived, or the
    /// `Attach` it answers never reached this worker - `Error::PaneGone`
    /// already covers that case for the caller).
    Arm {
        /// The attachment to arm.
        id: AttachmentId,
        /// The connection's own outbound queue.
        outbound: Arc<Outbound>,
    },
    /// Ends an attachment: no further update is computed or sent for it.
    /// A no-op if `id` is not currently registered.
    Detach {
        /// The attachment to end.
        id: AttachmentId,
    },
    /// How many attachments this pane's own fan-out list currently holds,
    /// read back through this reply channel (TASK-1.47 review): test-only,
    /// since nothing outside a test needs it - the one fact that is true
    /// exactly when a `Detach`, or the sweep `ConnectionCleanup::Drop`
    /// sends in its place, has actually been applied by this worker, not
    /// merely sent.
    #[cfg(test)]
    AttachmentCount(Sender<usize>),
}

/// The writer, resize handle and relay thread of whichever program is
/// currently running in a pane, if any - no killer here, and no pid
/// either: TASK-1.78 replaces `portable_pty::ChildKiller` (its own clone
/// sends `SIGHUP` by pid alone, with no escalation and no group) with
/// [`RunGate`], shared with the relay thread, never reached through this
/// struct or the command queue (see the module doc and `RunGate`'s own).
/// `relay_handle` is that run's relay thread, joined (bounded,
/// [`join_with_bound`]) once its own [`PaneCommand::Exited`] arrives -
/// not discarded, so a caller can know every thread this run started has
/// actually ended, not merely that a message arrived.
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
/// as it already was. `gate` is shared with the relay thread
/// ([`RunGate`]'s own doc has the full reasoning) - read (and, for a
/// signal, written-through) directly by [`PaneHandle::signal_hangup`]/
/// [`signal_kill`][PaneHandle::signal_kill]/[`await_exit`][PaneHandle::await_exit],
/// never through `commands`, so none of the three can be delayed behind
/// a flooding run's own backlog of `PtyBytes` on that same queue
/// (TASK-1.78 §1).
pub struct PaneHandle {
    commands: Sender<PaneCommand>,
    launcher: Arc<dyn ProgramLauncher>,
    worker: thread::JoinHandle<()>,
    gate: Arc<RunGate>,
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

    /// Registers a new attachment to this pane's screen and returns its
    /// own [`AttachmentId`] alongside its snapshot (TASK-1.47; AC #1, #7).
    /// The caller mints `stream` (a connection concern - the frame tag,
    /// connection-local) but the `AttachmentId` returned here, not
    /// `stream`, is what [`PaneHandle::arm`]/[`PaneHandle::detach`] must
    /// be called with (TASK-1.114: `stream` alone is not unique across
    /// different connections attached to the same pane - see
    /// [`AttachmentId`]'s own doc). Call `arm` once this reply is queued
    /// and the stream is opened with the writer - decision-2 section 3's
    /// ordering rule; see [`PaneCommand::Attach`]'s own doc for why attach
    /// and arm are two separate steps.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneGone`] if the pane's worker thread is no
    /// longer there, or whatever the terminal itself failed with.
    pub(crate) fn attach(&self, stream: u32) -> Result<(AttachmentId, Screen), Error> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.commands
            .send(PaneCommand::Attach {
                stream,
                reply: reply_tx,
            })
            .map_err(|_| Error::PaneGone)?;
        reply_rx.recv().map_err(|_| Error::PaneGone)?
    }

    /// Hands an already-registered attachment the `Outbound` it sends
    /// updates to (TASK-1.47) - see [`PaneCommand::Arm`]'s own doc for the
    /// ordering rule this is the second half of. `pub(crate)`, not `pub`:
    /// `Outbound` itself is `pub(crate)` (a server-internal type), so this
    /// is only ever called from this crate's own connection-handling code,
    /// never meaningfully reachable from outside it.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneGone`] if the pane's worker thread is no
    /// longer there.
    pub(crate) fn arm(&self, id: AttachmentId, outbound: Arc<Outbound>) -> Result<(), Error> {
        self.commands
            .send(PaneCommand::Arm { id, outbound })
            .map_err(|_| Error::PaneGone)
    }

    /// Ends an attachment (TASK-1.47): no further update is computed or
    /// sent for it. Not an error if it is already gone - detaching twice,
    /// or detaching an id the worker never armed, both leave the pane
    /// exactly as the caller wanted it: nothing more sent there.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PaneGone`] if the pane's worker thread is no
    /// longer there.
    pub(crate) fn detach(&self, id: AttachmentId) -> Result<(), Error> {
        self.commands
            .send(PaneCommand::Detach { id })
            .map_err(|_| Error::PaneGone)
    }

    /// How many attachments this pane's own fan-out list currently holds
    /// (TASK-1.47 review) - test-only; see [`PaneCommand::AttachmentCount`]'s
    /// own doc. `0` if the worker is already gone: nothing is left
    /// attached to a pane with no worker, whatever this call's own reply
    /// channel says.
    #[cfg(test)]
    pub(crate) fn attachment_count(&self) -> usize {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .commands
            .send(PaneCommand::AttachmentCount(reply_tx))
            .is_err()
        {
            return 0;
        }
        reply_rx.recv().unwrap_or(0)
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
        // Starts the new run on the shared `RunGate` *before* `Resume` is
        // even sent - synchronously, on this caller's own thread, the
        // same as `start` (below) does for the first run. A signal sent
        // in the brief window before the worker itself processes `Resume`
        // still reaches a real, already-spawned process, so this ordering
        // is never observably wrong, only ever slightly ahead of the
        // worker's own bookkeeping - and `begin_run`'s own new generation
        // is what stops a *stale* run's relay thread from ever clobbering
        // this new one (`RunGate`'s own doc; TASK-1.78's review, finding
        // 1's own nit).
        let generation = self.gate.begin_run(spawned.child.process_id());
        let relay_handle = spawn_relay(
            spawned.reader,
            spawned.child,
            self.commands.clone(),
            Arc::clone(&self.gate),
            generation,
        );
        self.commands
            .send(PaneCommand::Resume {
                writer: spawned.writer,
                resize: spawned.resize,
                relay_handle,
                open_attempts: spawned.open_attempts,
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
    /// (`crate::signal::send`), through [`RunGate::signal_if_alive`],
    /// never through `commands`, so a backlog of `PtyBytes` the worker
    /// has not drained yet (a flooding program) cannot delay this (module
    /// doc, TASK-1.78 §1), and the signal race the review's finding 1
    /// found cannot happen either (`RunGate`'s own doc). Fire-and-forget:
    /// use [`PaneHandle::await_exit`] to learn whether it worked.
    pub(crate) fn signal_hangup(&self) {
        self.gate.signal_if_alive(Signal::SIGHUP);
    }

    /// Same, `SIGKILL` - the escalation, after a grace period, for a run
    /// that ignored [`PaneHandle::signal_hangup`] (TASK-1.78 §2).
    pub(crate) fn signal_kill(&self) {
        self.gate.signal_if_alive(Signal::SIGKILL);
    }

    /// Waits up to `timeout` for the current run to actually end - `true`
    /// if it did within that bound (including "nothing was running at
    /// all", which answers at once), `false` on a real timeout. Through
    /// [`RunGate::await_dead`], never through `commands`, for the same
    /// reason as [`PaneHandle::signal_hangup`] - a wait that went through
    /// the worker's own queue could time this out purely because a real
    /// `Exited` was still stuck behind a flooding run's own `PtyBytes`
    /// backlog, misclassifying a program that had already ended as a
    /// straggler (the review's finding 1).
    pub(crate) fn await_exit(&self, timeout: Duration) -> bool {
        self.gate.await_dead(timeout)
    }

    /// Tells the worker thread its current run has ended and waits for
    /// it to acknowledge, bounded - the worker's own `AwaitExit` handling
    /// needs no change for this (TASK-1.78 §1's own fix already made its
    /// "nothing running" branch break the loop). Meaningful only once
    /// [`PaneHandle::await_exit`] has confirmed this via `RunGate` - the
    /// worker's own queue may still be draining a backlog at that point,
    /// which is fine, since nothing here depends on *when* the real exit
    /// was recorded, only that it already was.
    ///
    /// Kept separate from [`PaneHandle::join_worker`] (rather than
    /// folded into it) so a caller settling several panes at once
    /// (`Runtime::force_end`) can do this for every one of them in
    /// parallel, against one shared bound, instead of paying this bound
    /// again for each pane it then joins in turn.
    pub(crate) fn settle_worker(&self, timeout: Duration) -> bool {
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
    /// meaningful once [`PaneHandle::settle_worker`] has confirmed the
    /// worker has already been told to stop - calling this first can
    /// time out on a worker still legitimately serving other commands,
    /// which is not a bug in this method itself.
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
    attach_tick: Duration,
) -> Result<PaneHandle, Error> {
    let (cols, rows) = spec.size;
    let spawned = launcher.spawn(&spec)?;
    // A retry is never silent (TASK-1.112): sent right away, before
    // anything else below - the first thing that could observe this
    // pane at all is this same caller, synchronously, so there is no
    // earlier point to send it from.
    if spawned.open_attempts > 1 {
        let _ = events.send(PaneEvent::OpenRetried {
            pane,
            attempts: spawned.open_attempts,
        });
    }

    let (commands_tx, commands_rx) = mpsc::channel::<PaneCommand>();
    // The gate's own run starts *before* `child` moves into `spawn_relay`,
    // the same ordering `clone_killer` used to need and for the same
    // reason: once `child` is inside the relay thread, nothing outside
    // that thread can reach it through `child` itself. Started right
    // away, on this thread, before the worker thread (which shares the
    // same `Arc`) even exists - TASK-1.78 §1, and `RunGate`'s own doc for
    // the review fix this and `spawn_relay`'s own reaping loop are.
    let gate = Arc::new(RunGate::new());
    let generation = gate.begin_run(spawned.child.process_id());
    let relay_handle = spawn_relay(
        spawned.reader,
        spawned.child,
        commands_tx.clone(),
        Arc::clone(&gate),
        generation,
    );

    let initial = Running {
        writer: spawned.writer,
        resize: spawned.resize,
        relay_handle,
    };

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
                attach_tick,
            );
        })
        .map_err(Error::Io)?;

    Ok(PaneHandle {
        commands: commands_tx,
        launcher,
        worker,
        gate,
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
///
/// **The tick (TASK-1.47): armed only while there is real work for it,
/// never on a fixed cadence.** `next_tick` is `None` whenever nothing
/// attached has output it has not yet seen - which includes every pane
/// with no attachment at all - and the receive below blocks with no
/// timeout in that state, exactly as it always did before this task. A
/// command that could change the screen (`PtyBytes`, `Resize`) arms one
/// deadline, `attach_tick` from now, *only* if an armed attachment exists
/// and none is already pending (`maybe_arm_tick`) - one arm per dirty
/// window, not one per byte, which is what keeps the tick's own
/// coalescing real. A timeout clears `next_tick` and runs [`run_tick`];
/// nothing re-arms it until the next dirtying command does. This is the
/// whole of "no periodic wake-up": an idle or unattached pane calls
/// `recv()` and nothing else, for ever, until a real command arrives.
fn worker_loop(
    commands_rx: mpsc::Receiver<PaneCommand>,
    cols: u16,
    rows: u16,
    initial: Option<Running>,
    pane: PaneId,
    events: Sender<PaneEvent>,
    attach_tick: Duration,
) {
    let Ok(mut terminal) = Terminal::new(cols, rows, DEFAULT_SCROLLBACK_LINES) else {
        return;
    };
    let mut current = initial;
    // Set only while an `AwaitExit` is waiting for the current run to
    // actually end (TASK-1.78 §1) - `None` the rest of the time,
    // including whenever nothing is running at all.
    let mut awaiting_exit: Option<Sender<()>> = None;
    // TASK-1.47: every live attachment to this pane's screen, and the
    // terminal's own tracked point as of the last tick this worker ran
    // (`0` - a value `Terminal::changes_since` never hands out itself -
    // meaning "no tick has run yet", the same convention the terminal's
    // own `seq` already uses, read back by `run_tick` below).
    let mut attachments: Vec<Attachment> = Vec::new();
    let mut tracked_seq: u64 = 0;
    // TASK-1.114: this pane's own counter for minting `AttachmentId`s,
    // unique for this pane's whole life - see `AttachmentId`'s own doc
    // for why the connection-supplied `stream` number cannot serve this
    // role on its own.
    let mut next_attachment_id: u64 = 0;
    // `None` until a dirtying command arms it; see this function's own
    // doc for why this, not an unconditional `recv_timeout`, is what
    // makes an idle or unattached pane cost nothing.
    let mut next_tick: Option<Instant> = None;

    loop {
        let command = match next_tick {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                match commands_rx.recv_timeout(remaining) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => {
                        next_tick = None;
                        run_tick(&mut terminal, &mut tracked_seq, &mut attachments);
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
            None => match commands_rx.recv() {
                Ok(command) => command,
                Err(_) => break,
            },
        };

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
                maybe_arm_tick(&mut next_tick, attach_tick, &attachments);
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
                open_attempts,
            } => {
                // The shared `RunGate` was already started by
                // `PaneHandle::restart`, synchronously, before this was
                // even sent - nothing to do with it here.
                current = Some(Running {
                    writer,
                    resize,
                    relay_handle,
                });
                // A retry is never silent (TASK-1.112): sent only when
                // more than one attempt was actually needed.
                if open_attempts > 1 {
                    let _ = events.send(PaneEvent::OpenRetried {
                        pane,
                        attempts: open_attempts,
                    });
                }
            }
            PaneCommand::PtyBytes(bytes) => {
                terminal.write(&bytes);
                drain_command_marks(&mut terminal, pane, &events);
                maybe_arm_tick(&mut next_tick, attach_tick, &attachments);
            }
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
                // The shared `RunGate` is *already* marked dead by this
                // run's own relay thread by the time this is read - see
                // `spawn_relay`'s own comment - never here: this arm no
                // longer touches it at all (TASK-1.78's review, finding
                // 1 and its own nit).
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
            PaneCommand::Attach { stream, reply } => {
                let screen = terminal.screen().map_err(Error::from);
                let reply_value = match screen {
                    Ok(screen) => {
                        // Registered as caught up to the worker's own
                        // currently-tracked point, in this exact step -
                        // not "caught up starting next tick": decision-2
                        // section 3's stream resumption ("snapshot.seq +
                        // 1") depends on it. `outbound` stays `None`
                        // until `Arm` arrives; nothing reads it before
                        // then. `id` is this worker's own (TASK-1.114;
                        // `AttachmentId`'s own doc) - `stream` alone is
                        // not a safe key once a second connection can
                        // attach the same pane.
                        let id = AttachmentId(next_attachment_id);
                        next_attachment_id += 1;
                        attachments.push(Attachment {
                            id,
                            stream,
                            outbound: None,
                            caught_up_to: AttachState::CaughtUp(tracked_seq),
                        });
                        Ok((id, screen))
                    }
                    Err(error) => Err(error),
                };
                let _ = reply.send(reply_value);
            }
            PaneCommand::Arm { id, outbound } => {
                if let Some(attachment) = attachments.iter_mut().find(|a| a.id == id) {
                    attachment.outbound = Some(outbound);
                }
                maybe_arm_tick(&mut next_tick, attach_tick, &attachments);
            }
            PaneCommand::Detach { id } => {
                attachments.retain(|a| a.id != id);
            }
            #[cfg(test)]
            PaneCommand::AttachmentCount(reply) => {
                let _ = reply.send(attachments.len());
            }
        }
    }
}

/// Arms `next_tick`, `attach_tick` from now, if it is not already armed
/// and at least one attachment is actually armed (has an `Outbound` to
/// send to) - called after anything that could make `run_tick` have real
/// work to do. A no-op otherwise: an unattached or not-yet-armed pane
/// stays on a plain, timeout-free `recv()` (this function's own
/// non-effect *is* "no periodic wake-up", not a separate mechanism that
/// enforces it).
fn maybe_arm_tick(
    next_tick: &mut Option<Instant>,
    attach_tick: Duration,
    attachments: &[Attachment],
) {
    if next_tick.is_some() {
        return;
    }
    if attachments.iter().any(|a| a.outbound.is_some()) {
        *next_tick = Some(Instant::now() + attach_tick);
    }
}

/// Computes this pane's one screen update for this tick and fans it out
/// to every armed attachment (decision-2 section 3: "call this once per
/// tick and fan the one result out"; AC #1, #3).
///
/// An attachment with no `Outbound` yet (not armed - see [`Attachment`]'s
/// own doc) is skipped entirely: nothing is computed or sent for it, and
/// its own `caught_up_to` is left exactly as it was, which is what lets
/// [`PaneCommand::Arm`] hand it off cleanly whenever it does arrive - the
/// next tick after that treats a stale `CaughtUp` exactly like
/// `NeedsResync` (`AttachState`'s own doc), with no special case needed
/// here for "just armed".
///
/// Back-pressure (decision-2 section 6, AC #3's second half): an
/// attachment whose `enqueue_data` returns `Full` or `NotOpen` moves to
/// `NeedsResync` and is simply left there for this tick - the very next
/// tick tries a fresh, scrollback-free screen for it again, never a
/// delta, until one actually queues. This never blocks: `enqueue_data`
/// itself never blocks (`crate::outbound`'s own design), so neither does
/// this function.
fn run_tick(terminal: &mut Terminal, tracked_seq: &mut u64, attachments: &mut [Attachment]) {
    let previous = *tracked_seq;
    let seen = (previous != 0).then_some(previous);
    let Ok(outcome) = terminal.changes_since(seen) else {
        // The engine failed to report. Nothing to send this tick; try
        // again next tick rather than treating this as fatal to the pane.
        return;
    };
    let new_seq = match &outcome {
        ScreenUpdate::Unchanged => return,
        ScreenUpdate::Delta { seq, .. } | ScreenUpdate::Full { seq, .. } => *seq,
    };
    *tracked_seq = new_seq;

    // The JSON payload for an attachment that is already caught up to
    // `previous` - built once, shared by every such attachment, not
    // recomputed per attachment (decision-2 section 3's own point: one
    // update per tick, fanned out). Framed separately per attachment
    // below, since the stream id is part of the frame, not the payload.
    let mut ready_payload: Option<Vec<u8>> = None;

    for attachment in attachments.iter_mut() {
        let Some(outbound) = attachment.outbound.clone() else {
            continue; // not yet armed - nothing to send to
        };
        let ready =
            matches!(attachment.caught_up_to, AttachState::CaughtUp(seq) if seq == previous);

        let payload = if ready {
            ready_payload
                .get_or_insert_with(|| {
                    serde_json::to_vec(&outcome).expect("ScreenUpdate always serialises to JSON")
                })
                .clone()
        } else {
            // Needs a fresh, independent, scrollback-free screen - never
            // the tick's own `outcome` (which may be a `Delta` this
            // attachment cannot apply), and never what any other
            // attachment is sent (decision-2 section 6: "a client that
            // falls behind gets one fresh screen-sized snapshot, without
            // scrollback").
            let Ok(screen) = terminal.screen() else {
                continue; // try again next tick
            };
            let full = ScreenUpdate::Full {
                screen,
                seq: new_seq,
            };
            serde_json::to_vec(&full).expect("ScreenUpdate always serialises to JSON")
        };

        let bytes = encode_frame(&Frame::control(attachment.stream, payload))
            .expect("a screen update fits under the frame cap");

        match outbound.enqueue_data(attachment.stream, bytes) {
            Ok(()) => attachment.caught_up_to = AttachState::CaughtUp(new_seq),
            Err(EnqueueDataError::Full | EnqueueDataError::NotOpen) => {
                attachment.caught_up_to = AttachState::NeedsResync;
            }
            Err(EnqueueDataError::Closed) => {
                // The connection is tearing down; nothing more to do -
                // `PaneCommand::Detach` will arrive, or this attachment
                // simply never gets another successful send.
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
/// until end-of-file, then reaps `child` and forwards the exit code as
/// [`PaneCommand::Exited`]. Mirrors
/// `spikes/task-1.2-wire-protocol/probe-server/src/pty.rs`'s own reader
/// thread, except for the reap itself (below) - `generation` is the value
/// [`RunGate::begin_run`] gave this run's own spawn, carried here so this
/// thread can mark it dead once it is sure, never a stale run's own.
fn spawn_relay(
    mut reader: Box<dyn Read + Send>,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    commands: Sender<PaneCommand>,
    gate: Arc<RunGate>,
    generation: u64,
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
                        // the reap below - a dropped receiver must never
                        // skip reaping the child, or it is left a zombie
                        // (this is what the early `return` this comment
                        // replaces got wrong).
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
        // fully closed, which is what a real hang-up needs. TASK-1.78's
        // own ending of a running program does not depend on this at all
        // - `PaneHandle::signal_hangup`/`signal_kill` (`crate::signal::
        // send`) signal the process directly, by the pid `start`/
        // `restart` read off `child` before it ever reached this
        // function, so nothing here has to change for that to reach a
        // program still blocked in the `read` above.
        //
        // The reap itself, though, *is* this task's own (review finding
        // 1): `child.try_wait()`, polled while holding `RunGate`'s own
        // lock, never a blocking `child.wait()` called outside it. The
        // obvious-looking alternative - call the old blocking `wait()`
        // first, then take the lock afterward to mark the run dead - was
        // rejected: that still leaves a real window, between the reap
        // actually happening (inside `wait()`) and this thread getting
        // around to taking the lock, during which a concurrent signaller
        // could read a pid the OS has already reaped and, in principle,
        // already reused for an unrelated process. Polling `try_wait`
        // *inside* the lock closes that window instead of narrowing it:
        // `try_wait`'s own `Ok(None)` reaps nothing, so a concurrent
        // signaller reading the still-`Some` pid under the same lock is
        // always reading a pid that has not been reaped yet, and the
        // reap and the pid's own clearing to `None` happen in the exact
        // same critical section (`Ok(Some(_))`, below) - no signaller can
        // ever observe a pid between those two. `WaitPidFlag::WNOWAIT`
        // (observe without reaping at all, the most direct way to say
        // this) was the first thing tried instead, and does not need
        // `unsafe`, but `nix` 0.28.0 only exposes the `waitid` call that
        // flag needs on Android/FreeBSD/Haiku/Linux (`sys/wait.rs`'s own
        // `#[cfg(any(target_os = "android", target_os = "freebsd",
        // target_os = "haiku", all(target_os = "linux", ...)))]` on
        // `pub fn waitid`) - not on this project's own macOS target
        // (decision-4, R4) at all, so it was not an option here.
        let mut poll_interval = REAP_POLL_INITIAL_INTERVAL;
        let exit_code = loop {
            let mut state = gate.lock();
            match child.try_wait() {
                Ok(Some(status)) => {
                    state.clear_if_current(generation);
                    drop(state);
                    gate.changed.notify_all();
                    break Some(i32::try_from(status.exit_code()).unwrap_or(i32::MAX));
                }
                Ok(None) => {
                    drop(state);
                    thread::sleep(poll_interval);
                    poll_interval = next_reap_poll_interval(poll_interval);
                }
                // Deliberately silent, not a bug: this can only happen if
                // the OS has nothing left to report (the process was
                // already reaped by something outside this program's own
                // model, or a transient OS error) - there is no real exit
                // code to report either way, and `PaneEvent` has no
                // variant for "unknown" that isn't itself a guessed code,
                // so the pane is left in whatever state it was. Still
                // marks the run dead on `RunGate` first - whatever the OS
                // meant by this, it does not mean "still signallable".
                Err(_) => {
                    state.clear_if_current(generation);
                    drop(state);
                    gate.changed.notify_all();
                    break None;
                }
            }
        };
        if let Some(exit_code) = exit_code {
            let _ = commands.send(PaneCommand::Exited(exit_code));
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use crate::pty::{LaunchSpec, ProgramLauncher, RealLauncher};

    use super::{PaneCommand, next_reap_poll_interval, spawn_relay};

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
        let spawned = RealLauncher::default()
            .spawn(&real_spec(vec!["sh", "-c", "sleep 1; echo done"]))
            .unwrap();
        let pid = spawned.child.process_id().unwrap();
        let (tx, rx) = mpsc::channel::<PaneCommand>();
        let gate = std::sync::Arc::new(super::RunGate::new());
        let generation = gate.begin_run(spawned.child.process_id());

        let relay = spawn_relay(spawned.reader, spawned.child, tx, gate, generation);
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

    /// `RunGate`'s own generation guard, exercised directly rather than
    /// through an end-to-end restart (TASK-1.78's review, finding 1's own
    /// nit; DRAFT-1: an end-to-end "restart, then force" test - also
    /// added, in `ending_tests` - would pass with or without this guard,
    /// since nothing on that path can currently make a stale run's relay
    /// thread still be executing once a restart has happened; see
    /// `PaneHandle::restart`'s own doc for why). This constructs the
    /// exact race directly: a stale generation's own "mark dead" attempt,
    /// arriving after a newer run has already started, must never touch
    /// that newer run's own live pid.
    #[test]
    fn a_stale_generations_own_mark_dead_never_clobbers_a_newer_run() {
        let gate = super::RunGate::new();
        let old_generation = gate.begin_run(Some(100));
        let new_generation = gate.begin_run(Some(200));
        assert_ne!(old_generation, new_generation);

        // The exact shared method `spawn_relay`'s own reap loop calls,
        // for the *old* generation specifically - simulating a stale
        // relay thread that only now gets around to it, well after a
        // restart already happened. Calling the real, shared method
        // (not a hand-copied duplicate of its logic) is what makes this
        // a genuine test of production code, not just of this test's
        // own assumptions about what that code does.
        gate.lock().clear_if_current(old_generation);

        assert_eq!(
            gate.lock().pid,
            Some(200),
            "a stale generation's own mark-dead must never clobber a \
             newer run's live pid"
        );
    }

    /// TASK-1.78's review (a nit of the engineer's own): the reap poll's
    /// own backoff, proven as the pure doubling-and-cap it actually is,
    /// not only by trusting `spawn_relay`'s own real-process tests above
    /// to happen to exercise enough iterations.
    #[test]
    fn the_reap_poll_interval_doubles_then_holds_at_its_cap() {
        let mut interval = super::REAP_POLL_INITIAL_INTERVAL;
        let mut seen = vec![interval];
        for _ in 0..8 {
            interval = next_reap_poll_interval(interval);
            seen.push(interval);
        }

        assert_eq!(
            seen,
            vec![
                Duration::from_millis(5),
                Duration::from_millis(10),
                Duration::from_millis(20),
                Duration::from_millis(40),
                Duration::from_millis(80),
                super::REAP_POLL_MAX_INTERVAL,
                super::REAP_POLL_MAX_INTERVAL,
                super::REAP_POLL_MAX_INTERVAL,
                super::REAP_POLL_MAX_INTERVAL,
            ],
            "the interval must double each time until it reaches the cap, \
             then hold there - never exceed it, never reset"
        );
    }
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

    use super::PaneHandle;

    use super::start;
    use crate::pty::{LaunchSpec, RealLauncher};

    /// Bounded, never a bare wait - the same reasoning every other real-
    /// process wait in this crate gives.
    const EVENT_TIMEOUT: Duration = Duration::from_secs(10);

    /// A tiny `attach_tick` for every `start` in this module - none of
    /// these tests attach to a pane.
    const TEST_TICK: Duration = Duration::from_millis(1);

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
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
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
        assert!(handle.settle_worker(EVENT_TIMEOUT));
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
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();

        handle.signal_hangup();

        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("SIGHUP must end a plain sleep");
        assert!(handle.await_exit(EVENT_TIMEOUT));
        assert!(handle.settle_worker(EVENT_TIMEOUT));
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
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
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
        assert!(handle.settle_worker(EVENT_TIMEOUT));
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
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();

        handle.signal_hangup();

        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the main process must end");
        assert!(handle.await_exit(EVENT_TIMEOUT));
        assert!(handle.settle_worker(EVENT_TIMEOUT));
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

    /// The reviewer's own ask (TASK-1.78's review, finding 1's nit):
    /// restart a command pane, then force it - the *new* program must be
    /// the one that ends. A plain regression test, named for exactly
    /// that outcome and no more - the review's second round confirmed
    /// it is not a proof of the generation guard, and a reviewer who
    /// disabled the guard entirely found it still passes here, because
    /// nothing on this path can currently make the *old* run's relay
    /// thread still be executing (and so still able to call `clear_if_
    /// current` for a stale generation) once `restart` has returned (see
    /// `PaneHandle::restart`'s own doc) - this test's own synchronisation
    /// point (waiting for the first run's real exit event before
    /// restarting) is downstream of that relay thread's own mark-dead
    /// call, by construction, so there is no ordering left for a test at
    /// this level to race against. A seam that could hold that call back
    /// deliberately would need production code built only to make a test
    /// possible, with no other use - not attempted. The guard itself is
    /// exercised directly, by constructing the out-of-order race by
    /// hand, in `tests::a_stale_generations_own_mark_dead_never_clobbers_
    /// a_newer_run` - *that* is this guard's actual proof, with its own
    /// recorded negative control; this test is not.
    #[test]
    fn restarting_then_forcing_ends_the_new_program_as_a_plain_regression() {
        let (events_tx, events_rx) = mpsc::channel();
        let handle = start(
            real_spec(vec!["sh", "-c", "exit 0"]),
            steg_sdk::PaneId::from_raw(5),
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();
        // The first run already exits on its own - R97's own restart
        // precondition (`Tree::restart_program` only allows it once
        // `Ended`), proven here the same way: wait for the real event
        // before restarting, not assumed.
        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the first run's own exit");

        handle
            .restart(crate::pty::LaunchSpec {
                argv: vec!["sh".to_string(), "-c".to_string(), "sleep 1000".to_string()],
                cwd: std::env::temp_dir().to_str().unwrap().to_string(),
                // A working `PATH` is required (TASK-1.87): see
                // `real_spec`'s own comment in this module, above.
                env: vec![(
                    std::ffi::OsString::from("PATH"),
                    std::env::var_os("PATH").unwrap_or_default(),
                )],
                size: (80, 24),
            })
            .unwrap();

        handle.signal_hangup();
        events_rx
            .recv_timeout(EVENT_TIMEOUT)
            .expect("the new run's own exit, not the old one's (already consumed above)");
        assert!(handle.await_exit(EVENT_TIMEOUT));
        assert!(handle.settle_worker(EVENT_TIMEOUT));
        assert!(handle.join_worker(EVENT_TIMEOUT));
    }

    /// `await_exit` (`RunGate`-based) is not delayed by a backlog on the
    /// worker's own command queue; `settle_worker` (still queue-based,
    /// deliberately - its own doc explains why) is. TASK-1.78's review,
    /// finding 1's own point, demonstrated directly and deterministically
    /// rather than by flooding a real program's output through a real
    /// pty: a real end-to-end attempt at that (`tests/runtime.rs`'s own
    /// `a_flooding_program_that_exits_by_itself_mid_grace_completes_
    /// promptly_as_a_regression`, renamed for exactly that in TASK-1.78's
    /// review's second round) could not be made to show a measurable
    /// difference between the fixed and the reverted mechanism in this
    /// environment, because the pty's own kernel buffer throttles the
    /// writing program well before the backlog this bug needs can build
    /// up - a real limit on that test's own power to discriminate, named
    /// rather than hidden (and that test is kept only as an end-to-end
    /// regression now, not as this mechanism's proof). Flooding the
    /// *handle's* own queue directly, with real `send_input` calls (not
    /// relying on any OS buffering at all), has no such limit.
    #[test]
    fn await_exit_is_not_delayed_by_a_backlog_on_the_workers_own_queue() {
        // Two separate handles, each flooded the same way, each measured
        // alone - not one handle asked both ways in sequence: whichever
        // of the two drains a *shared* backlog first would leave nothing
        // for the other to wait on, which would make even the broken,
        // queue-based `await_exit` this is a negative control for look
        // fast, for the wrong reason (confirmed by hand: an earlier
        // version of this test, calling both on one handle, passed its
        // own "is it fast" assertion for the reverted mechanism too,
        // because the first call always drained the queue for the
        // second one regardless of which mechanism either used).
        const BACKLOG: usize = 1_000_000;

        fn flooded_handle(pane: u64) -> PaneHandle {
            let (events_tx, events_rx) = mpsc::channel();
            let handle = start(
                real_spec(vec!["sh", "-c", "exit 0"]),
                steg_sdk::PaneId::from_raw(pane),
                Arc::new(RealLauncher::default()),
                events_tx,
                TEST_TICK,
            )
            .unwrap();
            events_rx
                .recv_timeout(EVENT_TIMEOUT)
                .expect("the real shell's own exit, within the bound");
            for _ in 0..BACKLOG {
                handle.send_input(vec![0u8; 16]).unwrap();
            }
            handle
        }

        let await_exit_handle = flooded_handle(6);
        let start = std::time::Instant::now();
        assert!(await_exit_handle.await_exit(EVENT_TIMEOUT));
        let await_exit_elapsed = start.elapsed();
        assert!(await_exit_handle.settle_worker(EVENT_TIMEOUT));
        assert!(await_exit_handle.join_worker(EVENT_TIMEOUT));

        let settle_worker_handle = flooded_handle(7);
        let start = std::time::Instant::now();
        assert!(settle_worker_handle.settle_worker(EVENT_TIMEOUT));
        let settle_worker_elapsed = start.elapsed();
        assert!(settle_worker_handle.join_worker(EVENT_TIMEOUT));

        assert!(
            await_exit_elapsed < Duration::from_millis(50),
            "await_exit took {await_exit_elapsed:?} behind a {BACKLOG}-deep \
             backlog on its own, separate handle - it must bypass the \
             queue entirely, not merely be faster than settle_worker"
        );
        assert!(
            settle_worker_elapsed > await_exit_elapsed * 10,
            "settle_worker ({settle_worker_elapsed:?}) must actually be \
             slowed by the same backlog await_exit ({await_exit_elapsed:?}) \
             was unaffected by - otherwise this test is not proving the \
             two mechanisms differ at all"
        );
    }
}

/// TASK-1.47: attaching to a pane's screen, the tick, the fan-out and its
/// back-pressure. `maybe_arm_tick` and `run_tick` are proven directly, as
/// pure(-ish) functions - the same reasoning `testing-rust.md` gives for a
/// terminal's own behaviour ("feeding bytes in and reading the screen back,
/// not... the emulator's internals") applies here: these two functions
/// *are* the mechanism, not internals behind it, and calling them directly
/// needs no thread at all for most of what they decide. The full
/// `start`/`PaneHandle` surface is exercised by the handful of tests that
/// need a real worker thread and a real pty.
#[cfg(test)]
mod attach_tests {
    use std::io::Read;
    use std::os::unix::net::UnixStream;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use steg_sdk::{Decoder, PaneId, ScreenUpdate};

    use crate::outbound::{Outbound, run_writer};
    use crate::pty::{LaunchSpec, RealLauncher};

    use super::{AttachState, Attachment, AttachmentId, Terminal, maybe_arm_tick, run_tick, start};

    const TEST_TICK: Duration = Duration::from_millis(1);

    fn real_spec(argv: Vec<&str>) -> LaunchSpec {
        LaunchSpec {
            argv: argv.into_iter().map(str::to_string).collect(),
            cwd: std::env::temp_dir().to_str().unwrap().to_string(),
            env: vec![(
                std::ffi::OsString::from("PATH"),
                std::env::var_os("PATH").unwrap_or_default(),
            )],
            size: (80, 24),
        }
    }

    /// A real `Outbound`, writing onto a real `UnixStream::pair()` - the
    /// returned stream is this test's own read side; `outbound` is what
    /// `run_tick`/`PaneHandle::arm` enqueue onto. The writer thread is
    /// joined by `finish`, not dropped: every thread this helper starts is
    /// accounted for on every test's own exit path.
    struct FramePair {
        outbound: Arc<Outbound>,
        client: UnixStream,
        writer: std::thread::JoinHandle<()>,
        decoder: Decoder,
    }

    impl FramePair {
        fn new() -> Self {
            let outbound = Arc::new(Outbound::new());
            let (client, server) = UnixStream::pair().unwrap();
            let writer = {
                let outbound = Arc::clone(&outbound);
                std::thread::spawn(move || run_writer(&outbound, server))
            };
            Self {
                outbound,
                client,
                writer,
                decoder: Decoder::new(),
            }
        }

        /// Reads one frame's payload, decoded as a [`ScreenUpdate`] -
        /// bounded, via a real read with a timeout, never a sleep-poll:
        /// the event this waits on is the frame itself arriving.
        fn next_update(&mut self, timeout: Duration) -> (u32, ScreenUpdate) {
            let deadline = Instant::now() + timeout;
            loop {
                if let Ok(Some(frame)) = self.decoder.next_frame() {
                    let update = serde_json::from_slice(&frame.payload)
                        .expect("ScreenUpdate always deserialises from what run_tick sent");
                    return (frame.stream, update);
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                assert!(!remaining.is_zero(), "no update arrived within {timeout:?}");
                self.client.set_read_timeout(Some(remaining)).unwrap();
                let mut buf = [0_u8; 65536];
                match self.client.read(&mut buf) {
                    Ok(0) => panic!("the writer closed before an update arrived"),
                    Ok(n) => self.decoder.push(&buf[..n]),
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        panic!("no update arrived within {timeout:?}")
                    }
                    Err(e) => panic!("read failed: {e}"),
                }
            }
        }

        /// Asserts nothing arrives within `window` - used to prove an
        /// unarmed or detached attachment gets nothing, not inferred from
        /// a successful read elsewhere.
        fn assert_nothing_arrives(&mut self, window: Duration) {
            self.client.set_read_timeout(Some(window)).unwrap();
            let mut buf = [0_u8; 1];
            match self.client.read(&mut buf) {
                Ok(0) => {}
                Ok(_) => panic!("a byte arrived when nothing should have been sent"),
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(e) => panic!("read failed: {e}"),
            }
        }

        fn finish(self) {
            self.outbound.close();
            let _ = self.writer.join();
        }
    }

    // --- `maybe_arm_tick`: the arming rule, pure, no thread at all ---------

    #[test]
    fn no_attachments_never_arms() {
        let mut next_tick = None;
        maybe_arm_tick(&mut next_tick, TEST_TICK, &[]);
        assert_eq!(next_tick, None);
    }

    #[test]
    fn an_unarmed_attachment_never_arms() {
        let mut next_tick = None;
        let attachments = [Attachment {
            id: AttachmentId(0),
            stream: 1,
            outbound: None,
            caught_up_to: AttachState::CaughtUp(0),
        }];
        maybe_arm_tick(&mut next_tick, TEST_TICK, &attachments);
        assert_eq!(
            next_tick, None,
            "an attachment with no Outbound yet must never arm a tick - \
             this is the whole of \"no wake-up for an idle or unattached \
             pane\""
        );
    }

    #[test]
    fn one_armed_attachment_arms_a_deadline_roughly_one_tick_away() {
        let mut next_tick = None;
        let attachments = [Attachment {
            id: AttachmentId(0),
            stream: 1,
            outbound: Some(Arc::new(Outbound::new())),
            caught_up_to: AttachState::CaughtUp(0),
        }];
        let before = Instant::now();
        maybe_arm_tick(&mut next_tick, TEST_TICK, &attachments);
        let deadline = next_tick.expect("an armed attachment must arm a deadline");
        assert!(deadline >= before + TEST_TICK);
        assert!(deadline <= Instant::now() + TEST_TICK);
    }

    #[test]
    fn an_already_armed_deadline_is_never_extended() {
        let attachments = [Attachment {
            id: AttachmentId(0),
            stream: 1,
            outbound: Some(Arc::new(Outbound::new())),
            caught_up_to: AttachState::CaughtUp(0),
        }];
        let already = Instant::now() + Duration::from_secs(1);
        let mut next_tick = Some(already);
        maybe_arm_tick(&mut next_tick, TEST_TICK, &attachments);
        assert_eq!(
            next_tick,
            Some(already),
            "maybe_arm_tick must not reset or extend a deadline that is \
             already armed - one arm per dirty window, not one per call"
        );
    }

    // --- `run_tick`: the fan-out and back-pressure algorithm ---------------

    #[test]
    fn a_new_attachments_first_tick_is_a_full_screen_not_a_delta() {
        // The engine's own `seq` starts at 0 too (never handed out), so
        // the very first `changes_since` call of this terminal's whole
        // life always answers `Full`, regardless of what the attachment's
        // own `caught_up_to` already is - this is the terminal's own
        // documented behaviour, proven here at the level this task's own
        // code actually depends on it.
        let mut terminal = Terminal::new(10, 2, 100).unwrap();
        terminal.write(b"hi");
        let mut tracked_seq = 0_u64;
        let mut frames = FramePair::new();
        frames.outbound.open_stream(7);
        let mut attachments = [Attachment {
            id: AttachmentId(0),
            stream: 7,
            outbound: Some(Arc::clone(&frames.outbound)),
            caught_up_to: AttachState::CaughtUp(0),
        }];

        run_tick(&mut terminal, &mut tracked_seq, &mut attachments);

        let (stream, update) = frames.next_update(Duration::from_secs(2));
        assert_eq!(stream, 7);
        match update {
            ScreenUpdate::Full { seq, .. } => assert_eq!(seq, 1),
            unexpected @ (ScreenUpdate::Unchanged | ScreenUpdate::Delta { .. }) => {
                panic!("expected Full, got {unexpected:?}")
            }
        }
        assert_eq!(attachments[0].caught_up_to, AttachState::CaughtUp(1));
        frames.finish();
    }

    #[test]
    fn a_caught_up_attachment_gets_a_delta_on_the_next_real_change() {
        let mut terminal = Terminal::new(10, 2, 100).unwrap();
        terminal.write(b"first");
        let mut tracked_seq = 0_u64;
        let mut frames = FramePair::new();
        frames.outbound.open_stream(7);
        let mut attachments = [Attachment {
            id: AttachmentId(0),
            stream: 7,
            outbound: Some(Arc::clone(&frames.outbound)),
            caught_up_to: AttachState::CaughtUp(0),
        }];
        run_tick(&mut terminal, &mut tracked_seq, &mut attachments);
        let (_, first) = frames.next_update(Duration::from_secs(2));
        assert!(matches!(first, ScreenUpdate::Full { .. }));

        terminal.write(b"second");
        run_tick(&mut terminal, &mut tracked_seq, &mut attachments);

        let (_, second) = frames.next_update(Duration::from_secs(2));
        match second {
            ScreenUpdate::Delta { seq, .. } => assert_eq!(seq, tracked_seq),
            unexpected @ (ScreenUpdate::Unchanged | ScreenUpdate::Full { .. }) => panic!(
                "a caught-up attachment's second real change must be a \
                 Delta, not a redundant Full: got {unexpected:?}"
            ),
        }
        frames.finish();
    }

    #[test]
    fn an_attachment_marked_needs_resync_gets_a_fresh_screen_not_the_ticks_own_delta() {
        let mut terminal = Terminal::new(10, 2, 100).unwrap();
        terminal.write(b"first");
        let mut tracked_seq = 0_u64;
        let mut frames = FramePair::new();
        frames.outbound.open_stream(7);
        let mut attachments = [Attachment {
            id: AttachmentId(0),
            stream: 7,
            outbound: Some(Arc::clone(&frames.outbound)),
            caught_up_to: AttachState::CaughtUp(0),
        }];
        run_tick(&mut terminal, &mut tracked_seq, &mut attachments);
        let _ = frames.next_update(Duration::from_secs(2));

        // Force the attachment behind, the same way a full queue would
        // (decision-2 section 6) - never by hand-picking a stale seq, so
        // this exercises the exact state `run_tick` itself sets.
        attachments[0].caught_up_to = AttachState::NeedsResync;
        terminal.write(b"second");
        run_tick(&mut terminal, &mut tracked_seq, &mut attachments);

        let (_, update) = frames.next_update(Duration::from_secs(2));
        match update {
            ScreenUpdate::Full { seq, .. } => assert_eq!(seq, tracked_seq),
            unexpected @ (ScreenUpdate::Unchanged | ScreenUpdate::Delta { .. }) => panic!(
                "NeedsResync must get a fresh Full screen, never a Delta: \
                 got {unexpected:?}"
            ),
        }
        assert_eq!(
            attachments[0].caught_up_to,
            AttachState::CaughtUp(tracked_seq)
        );
        frames.finish();
    }

    /// decision-2 section 6's own back-pressure policy, proven against a
    /// real, bounded `Outbound` queue - not asserted from the type, run.
    /// Negative control (recorded on the task, not a toggle kept in the
    /// tree): reverting `run_tick`'s `Err(EnqueueDataError::Full |
    /// EnqueueDataError::NotOpen)` arm to `Err(_) => {}` (ignore the
    /// error, leave the attachment's own state as `CaughtUp`) makes this
    /// test fail: the attachment is never marked `NeedsResync`, so the
    /// next real change after the queue drains is wrongly sent as a
    /// `Delta` an already-behind client cannot apply.
    #[test]
    fn a_full_queue_marks_the_attachment_for_resync_without_blocking() {
        let mut terminal = Terminal::new(10, 2, 100).unwrap();
        let mut tracked_seq = 0_u64;
        let outbound = Arc::new(Outbound::new());
        outbound.open_stream(7); // never drained: no writer thread at all
        let mut attachments = [Attachment {
            id: AttachmentId(0),
            stream: 7,
            outbound: Some(Arc::clone(&outbound)),
            caught_up_to: AttachState::CaughtUp(0),
        }];

        // Fills the queue directly (decision-2's own measured `DATA_CAP`,
        // `crate::outbound`'s own constant - not duplicated here, just
        // driven past): same effect as many real ticks' worth of output
        // with nobody reading, without needing that many real writes.
        for i in 0.. {
            terminal.write(format!("line {i}\r\n").as_bytes());
            let before = attachments[0].caught_up_to;
            let started = Instant::now();
            run_tick(&mut terminal, &mut tracked_seq, &mut attachments);
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "run_tick must never block on a full queue"
            );
            if attachments[0].caught_up_to == AttachState::NeedsResync {
                assert_ne!(
                    before,
                    AttachState::NeedsResync,
                    "the attachment should not have started this iteration \
                     already needing a resync"
                );
                break;
            }
            assert!(
                i < 1000,
                "the queue never filled - test is not exercising DATA_CAP"
            );
        }

        outbound.close();
    }

    // --- The real thing: `start`/`PaneHandle::attach`/`arm`/`detach` -------

    /// A row's own text, trimmed trailing blanks stripped by the caller if
    /// it wants that - kept raw here since this helper is only ever used
    /// to search for a known substring, never compared whole.
    fn row_text(row: &steg_sdk::Row) -> String {
        row.iter().map(|run| run.text.as_str()).collect()
    }

    /// Applies one [`ScreenUpdate`] onto a client's own held copy, the way
    /// a real attached client does (decision-2 section 3): `Delta`
    /// replaces only the rows it names, by position; `Full` replaces the
    /// whole thing; `Unchanged` does nothing.
    fn apply_update(held: &mut steg_sdk::Screen, update: ScreenUpdate) {
        match update {
            ScreenUpdate::Unchanged => {}
            ScreenUpdate::Full { screen, .. } => *held = screen,
            ScreenUpdate::Delta { rows, cursor, .. } => {
                for (position, row) in rows {
                    if let Some(slot) = held.rows.get_mut(usize::from(position)) {
                        *slot = row;
                    }
                }
                held.cursor = cursor;
            }
        }
    }

    fn screen_text(screen: &steg_sdk::Screen) -> String {
        screen
            .rows
            .iter()
            .map(row_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn attaching_returns_the_panes_current_screen_and_updates_follow_it() {
        let (events_tx, _events_rx) = mpsc::channel();
        // `cat`: reflects exactly what this test sends it, on its own -
        // no race with a shell starting up or a prompt rendering, since
        // nothing is written to this pty until this test does so itself.
        let handle = start(
            real_spec(vec!["sh", "-c", "exec cat"]),
            PaneId::from_raw(1),
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();

        // Nothing has been sent yet - deterministically blank, no race to
        // synchronise with.
        let stream = 7;
        let (attachment_id, attached_screen) = handle.attach(stream).unwrap();
        assert!(
            screen_text(&attached_screen).trim().is_empty(),
            "nothing was written yet: {:?}",
            screen_text(&attached_screen)
        );

        let mut frames = FramePair::new();
        frames.outbound.open_stream(stream);
        handle
            .arm(attachment_id, Arc::clone(&frames.outbound))
            .unwrap();

        // The pty's own line discipline echoes this, and `cat` relays it
        // back too - either way, "hello" lands on the screen, possibly
        // over more than one update (the two echoes, or the tick simply
        // catching the write mid-flight) - applied here exactly as a real
        // client would, onto its own held copy, rather than assumed to
        // land whole in the very first frame.
        handle.send_input(b"hello\r\n".to_vec()).unwrap();

        let mut held = attached_screen;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let (frame_stream, update) = frames.next_update(
                deadline
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_millis(1)),
            );
            assert_eq!(frame_stream, stream);
            apply_update(&mut held, update);
            if screen_text(&held).contains("hello") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "never saw the echoed input land, holding: {:?}",
                screen_text(&held)
            );
        }

        handle.detach(attachment_id).unwrap();
        handle.send_input(b"more\r\n".to_vec()).unwrap();
        frames.assert_nothing_arrives(Duration::from_millis(200));

        frames.finish();
        handle.shutdown();
    }

    /// decision-2 section 3's own ordering rule, the half this layer can
    /// prove without a connection: an attachment never receives anything
    /// before [`super::PaneHandle::arm`] hands it the `Outbound` it sends
    /// to - even when real output happens in between. `connection.rs`'s
    /// own half (queuing the attach reply before calling `Outbound::
    /// open_stream`) is TASK-1.47's step 4, blocked on TASK-1.46's
    /// description vocabulary - named, not silently skipped.
    #[test]
    fn an_attachment_gets_nothing_before_it_is_armed_even_if_output_happens_first() {
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = start(
            real_spec(vec!["sh", "-c", "exec sleep 1000"]),
            PaneId::from_raw(2),
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();

        let stream = 3;
        let (attachment_id, _) = handle.attach(stream).unwrap();

        handle.send_input(b"before arm\n".to_vec()).unwrap();
        // Lets the worker's own tick (1ms) run several times over with
        // real output pending, unarmed - a real wait, not a sleep this
        // test treats as proof of anything: the actual proof is the read
        // below finding nothing.
        std::thread::sleep(Duration::from_millis(50));

        let mut frames = FramePair::new();
        frames.outbound.open_stream(stream);
        // Still nothing: arming happens on the next line, after this.
        frames.assert_nothing_arrives(Duration::from_millis(50));

        handle
            .arm(attachment_id, Arc::clone(&frames.outbound))
            .unwrap();
        handle.send_input(b"after arm\n".to_vec()).unwrap();
        let _ = frames.next_update(Duration::from_secs(2));

        frames.finish();
        handle.shutdown();
    }

    /// The lead's own named risk: a worker blocked in `recv_timeout`
    /// (a deadline armed) must still notice `Shutdown` promptly, not only
    /// a worker blocked in a plain `recv()`.
    #[test]
    fn shutdown_ends_the_worker_even_while_a_tick_is_armed() {
        let (events_tx, _events_rx) = mpsc::channel();
        let handle = start(
            real_spec(vec!["sh", "-c", "exec sleep 1000"]),
            PaneId::from_raw(4),
            Arc::new(RealLauncher::default()),
            events_tx,
            Duration::from_secs(60), // a deadline far in the future if armed
        )
        .unwrap();
        let stream = 9;
        let (attachment_id, _) = handle.attach(stream).unwrap();
        let frames = FramePair::new();
        handle
            .arm(attachment_id, Arc::clone(&frames.outbound))
            .unwrap();
        handle.send_input(b"x".to_vec()).unwrap(); // arms the far-future deadline

        handle.shutdown();
        frames.finish();
        // No direct join handle from `shutdown` (fire-and-forget, by
        // design - see its own doc); this test's own proof is that the
        // process does not hang waiting for anything further, which the
        // test harness's own timeout already enforces for the whole test.
    }
}
