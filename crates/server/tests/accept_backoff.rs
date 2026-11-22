//! TASK-1.98's own proof, through the real `accept-backoff-attempt`
//! fixture (`tests/fixtures/accept_backoff_attempt.rs`) under a real,
//! lowered `RLIMIT_NOFILE` in its own dedicated process - genuine
//! `EMFILE`/`ENFILE`, never a mock of `accept()` itself:
//!
//! - AC #4: the loop cannot spin at full speed while descriptors stay
//!   exhausted, with a permanent negative control (the same fixture, no
//!   backoff, proving this harness can actually tell a spin from a
//!   backoff).
//! - AC #1's stop-path fix: a `stop()` asked mid-storm ends the server
//!   promptly, not only once the storm resolves on its own.
//! - AC #2: a genuinely fatal `accept()` error (`ENOTSOCK`, induced by
//!   `dup2`ing a non-socket over the listener's own fd while the loop is
//!   provably mid-backoff) still ends the server with status 1 and still
//!   runs the same cleanup (socket file removed, lock released) as any
//!   other stop.
//!
//! **Every wait in this file is bounded** (a join, a read, a connect, a
//! child's own exit) - a review of this task found a real, reachable
//! unbounded one in an earlier version. **What is actually confirmed, not
//! merely theorised**: `CountReader`'s own `thread.join()` had no bound,
//! and the fixture's own stdout only closes once its process exits - so
//! any run where the fixture did not exit (this file's own next doc
//! paragraph names the reason that actually happened: the exhaustion
//! condition ending quietly, with the background supply thread's own
//! stop-join then also running unbounded) hung this file's own join, with
//! no timeout anywhere to end it. Reproduced directly, both on this
//! platform's own CI runner from a fresh `git push` and independently in
//! a second engineer's own reproduction of this same PR.
//!
//! **What this file's first review round *suspected*, and what a direct
//! check since then found instead**: that earlier version's background
//! supply thread kept calling `UnixStream::connect` with nothing capping
//! it against this platform's own listen backlog (128,
//! `UnixListener::bind`'s own hardcoded value on macOS - confirmed by
//! reading `library/std/src/os/unix/net/listener.rs`, not assumed), and
//! the working theory at the time was that a `connect()` against a full
//! backlog would itself block (`UnixStream` has no `connect_timeout` at
//! all) once that thread's own held connections reached it. **A direct
//! probe since found that is not what this platform does**: binding a
//! listener that never once calls `accept()` at all - the most adversarial
//! case there is - and then calling `connect()` against it 300 times in a
//! row, every single attempt either succeeded immediately or failed
//! immediately with `ECONNREFUSED`; none ever blocked. A second engineer's
//! own independent reproduction of this same question found the same
//! thing. That theory is retracted, not merely left unstated: under what
//! condition (if any) a Unix-domain `connect()` *does* block on this
//! platform remains unestablished. The fix below does not depend on
//! settling that - bounding every wait is correct regardless of which
//! specific call might someday be the slow one - which is why it stays,
//! unchanged, rather than being narrowed to only the confirmed mechanism.
//!
//! Every test in this file uses `ConnectionSupply`, a background thread
//! that keeps replenishing held connections until told to stop, rather
//! than one fixed batch: a fixed batch, tried first while building this
//! task, was found - measured, not assumed - to only ever produce a brief
//! burst before the fixture's own accept loop drained it and went idle
//! waiting for a connection that would never arrive, which both (a) made
//! AC #4's own rate tests measure nothing over their own 500ms window once
//! nothing throttled how fast the loop could consume it (the zero-backoff
//! case), and (b), more subtly, meant AC #1/#2's own stop/fatal tests could
//! pass even with their own fix *removed* - not because the fix worked, but
//! because the storm had already quietly ended on its own by the time
//! `STOP_AFTER_RETRIES`/`DUP2_AFTER_RETRIES` fired, letting `stop()`'s own
//! ordinary wakeup (a real `connect()`, on the loop's already-unmodified
//! success path) win regardless. `ConnectionSupply` keeps the condition
//! genuinely live until each test says otherwise, so every one of them
//! tests what it claims to. Every individual `connect()` it makes is
//! itself bounded (`connect_bounded`, spawned on its own thread and joined
//! with `join_with_bound`), so even if the backlog genuinely does fill
//! (the slow-draining, real-backoff case), no single attempt can block
//! this thread past its own bound, and stopping it is bounded the same
//! way.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nix::sys::resource::{Resource, getrlimit, setrlimit};
use steg_server::{Arbitration, ServerConfig, attempt};

#[path = "support/mod.rs"]
mod support;
use support::scoped_socket_path;

/// Comfortably above what binding the socket and holding its lock cost on
/// their own (confirmed empirically while building this task, not
/// guessed).
const FIXTURE_RLIMIT_NOFILE: u64 = 48;

/// Raises this test *process's* own soft `RLIMIT_NOFILE` to its own hard
/// limit - cheap defensive margin against `ConnectionSupply`'s own real
/// connections landing on a shell with an unusually low default (256 is
/// common on this platform). Raising only the soft limit, never the hard
/// one, is the same safe, standard idiom documented on `setrlimit(2)` for
/// a process widening its own budget - nothing is lowered here, unlike the
/// fixture's own, deliberately lowered limit.
fn raise_own_rlimit() {
    let (_soft, hard) =
        getrlimit(Resource::RLIMIT_NOFILE).expect("getrlimit should succeed on this platform");
    setrlimit(Resource::RLIMIT_NOFILE, hard, hard)
        .expect("raising this test's own soft limit to its own hard limit should succeed");
}

fn lock_path_for(socket_path: &std::path::Path) -> PathBuf {
    let mut p = socket_path.as_os_str().to_owned();
    p.push(".lock");
    PathBuf::from(p)
}

fn fixture_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_accept-backoff-attempt"))
}

/// Bounded join, the shape `crates/server/tests/shutdown.rs`'s own
/// `join_within` already uses: a watcher thread does the real,
/// potentially-unbounded join, and this function only ever waits on a
/// channel with a bound - so a thread that somehow never ends (this
/// file's own module doc names the one way that used to happen) fails
/// this call with `None`, rather than hanging it. The watcher itself is
/// never joined - if the real thread never ends, neither does the
/// watcher, but nothing here ever waits on *that*, so it costs this
/// function nothing beyond a thread that outlives it, acceptable on a
/// test's own, rare, timeout path.
fn join_with_bound<T: Send + 'static>(handle: JoinHandle<T>, bound: Duration) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = handle.join();
        let _ = tx.send(result);
    });
    match rx.recv_timeout(bound) {
        Ok(Ok(value)) => Some(value),
        Ok(Err(_)) => None, // the joined thread panicked
        Err(_) => None,     // timed out
    }
}

/// One `connect()` attempt, bounded - spawned on its own thread so a
/// `connect()` that would otherwise block (this platform's own listen
/// backlog, 128, full - this file's own module doc) cannot block the
/// caller past `bound`. A timed-out attempt's own thread is abandoned, not
/// force-stopped (`connect()` itself has no cancellation): it either
/// eventually succeeds, whose result nobody ever reads, and ends, or never
/// does and outlives this test binary - acceptable, since nothing here
/// ever joins it again either way.
fn connect_bounded(path: &Path, bound: Duration) -> Option<UnixStream> {
    let path = path.to_path_buf();
    let handle = std::thread::spawn(move || UnixStream::connect(&path));
    join_with_bound(handle, bound).and_then(Result::ok)
}

/// Kills and reaps its child on drop, including when a panicking assertion
/// unwinds past it - the same guard `race.rs`'s own `ReapOnDrop` already
/// uses, so a failing assertion here never leaves a real process behind.
/// The wait after `kill()` is bounded (`try_wait`, polled), not a bare
/// `wait()`: a review of this task required every wait here to be bounded,
/// including this one, and a `Drop` must never panic, so this gives up
/// silently after the bound rather than asserting.
struct ReapOnDrop(Child);

impl Drop for ReapOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match self.0.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => return,
            }
        }
        // Gave up waiting for a `SIGKILL`'d child to be reaped within the
        // bound - never expected in practice (a killed process does not
        // stay around), but a `Drop` does not get to hang the test
        // process, which is itself likely mid-panic, to find out why.
    }
}

fn spawn_fixture(socket_path: &std::path::Path, extra_env: &[(&str, &str)]) -> ReapOnDrop {
    let mut command = Command::new(fixture_binary());
    command
        .env("STEG_SERVER", socket_path)
        .env("FIXTURE_RLIMIT_NOFILE", FIXTURE_RLIMIT_NOFILE.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (key, value) in extra_env {
        command.env(key, value);
    }
    ReapOnDrop(command.spawn().expect("the fixture binary should spawn"))
}

fn wait_until_accepting(path: &std::path::Path, deadline: Instant) {
    loop {
        // Bounded (`connect_bounded`), not a bare `connect()` - this
        // file's own module doc says every wait here is bounded, and this
        // one is no exception just because it is expected to resolve fast
        // in practice (nothing has connected yet at this point in every
        // test that calls this, so the backlog this file's own module doc
        // worries about is nowhere near relevant) - the outer `deadline`
        // only bounds how many attempts this loop makes, not any single
        // one of them.
        if connect_bounded(path, Duration::from_millis(500)).is_some() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "nothing ever answered at {path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Keeps replenishing held connections against `socket_path` on a
/// background thread until told to stop, pushing the fixture's own
/// lowered `RLIMIT_NOFILE` past its limit and keeping it there for as long
/// as a test needs: used by every test in this file, not only AC #4's own
/// rate tests, for the reason this file's own module doc gives (a fixed
/// batch let the condition end quietly on its own, which could make even
/// the stop/fatal tests pass for the wrong reason). Each individual
/// `connect()` is itself bounded (`connect_bounded`), so this can run
/// indefinitely with no risk of the one real hang a review of this task
/// found: a thread that could itself block forever on a full backlog.
/// Ignoring a failed `connect()` (a `connect_bounded` timeout) is
/// deliberate, the same reasoning `crates/steg/tests/server_start.rs`'s
/// own `running_out_of_file_descriptors_exits_1` used for its own ignored
/// `connect()` failures: once descriptors are genuinely exhausted, later
/// attempts are expected to fail (or, here, time out) too, and this is a
/// setup step, not the test's own assertion.
struct ConnectionSupply {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Vec<UnixStream>>>,
}

impl ConnectionSupply {
    fn start(socket_path: &Path) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let path = socket_path.to_path_buf();
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let mut held = Vec::new();
            while !thread_stop.load(Ordering::Relaxed) {
                match connect_bounded(&path, Duration::from_millis(100)) {
                    Some(stream) => held.push(stream),
                    // Only reached if the backlog is genuinely already
                    // full (this file's own module doc) - a short pause
                    // before trying again, rather than immediately
                    // spawning another attempt that would likely also
                    // need its own bound to expire.
                    None => std::thread::sleep(Duration::from_millis(20)),
                }
            }
            held
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }

    /// Stops the supply and hands back every connection it made, still
    /// open - bounded (`join_with_bound`): the supply thread's own loop
    /// checks `stop` between attempts, each of which is itself bounded at
    /// 100ms, so it is expected back well within this bound in practice;
    /// an empty `Vec` on a timeout (never observed while building this
    /// task) is still a safe answer; a stuck supply thread at that point
    /// has not blocked anything else, since nothing after this call
    /// depends on recovering its particular connections.
    fn stop_and_collect(mut self) -> Vec<UnixStream> {
        self.stop.store(true, Ordering::Relaxed);
        join_with_bound(
            self.thread
                .take()
                .expect("stop_and_collect is only ever called once"),
            Duration::from_secs(5),
        )
        .unwrap_or_default()
    }
}

impl Drop for ConnectionSupply {
    /// A backstop for the panicking path only - every test below calls
    /// `stop_and_collect` itself on the happy path. Bounded the same way.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = join_with_bound(thread, Duration::from_secs(5));
        }
    }
}

/// Reads the fixture's own `"count {n}"` lines from its stdout on a side
/// thread, parsed into the numbers alone. Joined with a bound (`drop`,
/// below) rather than assumed to end promptly: the thread's own loop ends
/// once the fixture process exits and closes its stdout, which every test
/// below guarantees (through `ReapOnDrop`, or a bounded wait on the
/// fixture exiting by itself) before dropping this - but a bound here too
/// is what turns "the fixture somehow did not actually exit" into a clear
/// failure rather than a second way to hang.
struct CountReader {
    rx: mpsc::Receiver<u64>,
    thread: Option<JoinHandle<()>>,
}

impl CountReader {
    fn spawn(child: &mut ReapOnDrop) -> Self {
        let stdout = child.0.stdout.take().expect("stdout was piped");
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let Some(count) = line
                    .strip_prefix("count ")
                    .and_then(|rest| rest.trim().parse::<u64>().ok())
                else {
                    continue;
                };
                if tx.send(count).is_err() {
                    break;
                }
            }
        });
        Self {
            rx,
            thread: Some(thread),
        }
    }

    /// Blocks until at least one count has arrived - confirms the fixture
    /// is genuinely retrying (not coincidentally idle) before a test does
    /// anything that depends on that, never a sleep standing in for it.
    fn wait_for_first(&self, bound: Duration) -> u64 {
        self.rx
            .recv_timeout(bound)
            .expect("the fixture should report at least one retry within the bound")
    }

    /// The highest count observed within `window`, measured from the call
    /// to this method, not from the fixture's own start - every test below
    /// calls `wait_for_first` first, so the loop is already confirmed
    /// retrying before this window starts.
    fn max_within(&self, window: Duration) -> u64 {
        let deadline = Instant::now() + window;
        let mut last = 0;
        loop {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return last;
            };
            match self.rx.recv_timeout(remaining) {
                Ok(count) => last = count,
                Err(_) => return last,
            }
        }
    }
}

impl Drop for CountReader {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            // Bounded (`join_with_bound`, above), not a bare `join()` - a
            // review of this task required every wait here to be bounded;
            // a `Drop` must not panic, so a timeout here is silently given
            // up on rather than asserted.
            let _ = join_with_bound(thread, Duration::from_secs(5));
        }
    }
}

/// Proves the lock was actually released, not merely that some file is
/// gone: a `flock` stays released regardless of whether its own backing
/// path still exists on disk, so the real contract (`shutdown.rs`'s own
/// `stopping_closes_the_listener_removes_the_socket_and_releases_the_lock`
/// proves the identical thing) is "a fresh attempt at the same path can
/// become the server", not "the `.lock` file is gone" - a lock file left
/// behind, unlocked, is not a bug.
fn assert_lock_released(socket_path: &std::path::Path) {
    let config = ServerConfig {
        socket_path: Some(socket_path.to_path_buf()),
        ..ServerConfig::default()
    };
    let result = attempt(&config, socket_path).expect("a fresh attempt should not itself error");
    match result {
        Arbitration::Became(handle) => drop(handle),
        other @ (Arbitration::AlreadyRunning | Arbitration::Refuse(_)) => panic!(
            "expected a fresh attempt to become the server (proving the old lock was \
             released), got {other:?}"
        ),
    }
}

fn wait_bounded(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait should not fail") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the fixture process did not exit within the bound"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// AC #4: with a real, short backoff, the loop's own attempt rate while
/// descriptors stay exhausted is bounded, not a spin. Measured, not
/// guessed, with `ConnectionSupply`'s own bounded replenishment (not the
/// unbounded, review-found-broken design this file's own module doc
/// names): `ACCEPT_BACKOFF_MS=20` over a 500ms window observes about 22;
/// `ACCEPT_BACKOFF_MS=0`
/// (`accept_errors_spin_without_a_backoff_negative_control`, below)
/// observes on the order of ten thousand in the same window - the bound
/// below (150) leaves nearly seven times the observed backoff rate for
/// scheduling jitter, while staying two orders of magnitude under the
/// observed spin rate.
#[test]
fn accept_errors_back_off_instead_of_spinning() {
    const BOUND: u64 = 150;

    raise_own_rlimit();
    let (_socket_dir, socket_path) = scoped_socket_path("backoff.sock");
    let mut fixture = spawn_fixture(&socket_path, &[("ACCEPT_BACKOFF_MS", "20")]);
    wait_until_accepting(&socket_path, Instant::now() + Duration::from_secs(5));

    let supply = ConnectionSupply::start(&socket_path);
    let counts = CountReader::spawn(&mut fixture);
    counts.wait_for_first(Duration::from_secs(5));
    let observed = counts.max_within(Duration::from_millis(500));
    let held = supply.stop_and_collect();

    assert!(
        observed < BOUND,
        "expected a bounded retry rate with a real backoff, observed {observed} attempts in 500ms"
    );

    // `fixture` first: its `Drop` kills and waits (bounded) the process,
    // closing its stdout - only then is `counts`'s own reader thread
    // guaranteed to see EOF and return.
    drop(fixture);
    drop(counts);
    drop(held);
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(lock_path_for(&socket_path));
}

/// The negative control for the test above, run for real against the same
/// fixture rather than asserted from memory: with no backoff at all, the
/// same 500ms window produces far more attempts, proving this harness can
/// actually distinguish a spin from a backoff.
#[test]
fn accept_errors_spin_without_a_backoff_negative_control() {
    const BOUND: u64 = 150;

    raise_own_rlimit();
    let (_socket_dir, socket_path) = scoped_socket_path("spin.sock");
    let mut fixture = spawn_fixture(&socket_path, &[("ACCEPT_BACKOFF_MS", "0")]);
    wait_until_accepting(&socket_path, Instant::now() + Duration::from_secs(5));

    let supply = ConnectionSupply::start(&socket_path);
    let counts = CountReader::spawn(&mut fixture);
    counts.wait_for_first(Duration::from_secs(5));
    let observed = counts.max_within(Duration::from_millis(500));
    let held = supply.stop_and_collect();

    assert!(
        observed > BOUND * 3,
        "expected a spin to blow well past the bounded test's own threshold, observed only \
         {observed} attempts in 500ms - the bounded test's own pass would not be meaningful \
         if this harness could not tell the difference"
    );

    drop(fixture);
    drop(counts);
    drop(held);
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(lock_path_for(&socket_path));
}

/// AC #1's stop-path fix: `stop()` asked while descriptors stay exhausted
/// must not wait out the storm - it has to win within about one backoff
/// interval, the same as any other stop. The fixture calls its own
/// `Stopper::stop()` once it has seen one `AcceptRetried` event (confirmed
/// genuinely mid-storm, not coincidentally idle), so this test only has to
/// wait for the process to exit, bounded well above what one 20ms backoff
/// interval needs but far below "the storm would have to resolve on its
/// own first".
///
/// **Negative control (a scratch edit, run once during this task's own
/// build and reverted, not kept as code)**: with the `if stop.should_stop()
/// { break Ok(()); }` check removed from both recoverable-error arms in
/// `listen.rs`, this same test does not pass within its bound - the
/// fixture keeps retrying against the exhaustion, oblivious to the flag,
/// until this test's own bound fails it.
#[test]
fn a_stop_asked_mid_exhaustion_ends_the_server_promptly() {
    raise_own_rlimit();
    let (_socket_dir, socket_path) = scoped_socket_path("stop.sock");
    let mut fixture = spawn_fixture(
        &socket_path,
        &[("ACCEPT_BACKOFF_MS", "20"), ("STOP_AFTER_RETRIES", "1")],
    );
    wait_until_accepting(&socket_path, Instant::now() + Duration::from_secs(5));

    let supply = ConnectionSupply::start(&socket_path);

    let status = wait_bounded(&mut fixture.0, Duration::from_secs(5));
    let held = supply.stop_and_collect();
    fixture.0.stdout.take(); // nothing left to read; avoid a pointless pipe wait
    assert!(
        status.success(),
        "a stop asked mid-storm should still exit 0, got {status:?}"
    );

    assert!(
        !socket_path.exists(),
        "the socket file should be removed even when stop raced an exhaustion storm"
    );
    assert_lock_released(&socket_path);

    drop(held);
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(lock_path_for(&socket_path));
}

/// AC #2, proven for real: a genuinely fatal `accept()` error
/// (`ENOTSOCK`, from the fixture `dup2`ing an open `/dev/null` over its
/// own listener's raw fd once it has seen one recoverable retry - safe
/// specifically because this test keeps every exhausting connection open,
/// unchanged, right up through the fixture's own exit, so the loop
/// provably never returns to a blocking `accept()` before the injection
/// lands) still ends the server with status 1 and still releases its
/// socket and lock, the same cleanup as any other stop.
///
/// **Negative control (a scratch edit, run once during this task's own
/// build and reverted, not kept as code)**: with `classify_accept_error`
/// changed to also return `Some(..)` for `ENOTSOCK`, the fixture never
/// exits - it keeps retrying against the dummy fd - and this test's own
/// bound fails it.
#[test]
fn a_fatal_accept_error_ends_the_server_and_cleans_up() {
    raise_own_rlimit();
    let (_socket_dir, socket_path) = scoped_socket_path("fatal.sock");
    let mut fixture = spawn_fixture(
        &socket_path,
        &[("ACCEPT_BACKOFF_MS", "20"), ("DUP2_AFTER_RETRIES", "1")],
    );
    wait_until_accepting(&socket_path, Instant::now() + Duration::from_secs(5));

    let supply = ConnectionSupply::start(&socket_path);

    let status = wait_bounded(&mut fixture.0, Duration::from_secs(5));
    let held = supply.stop_and_collect();
    fixture.0.stdout.take();
    assert_eq!(
        status.code(),
        Some(1),
        "a genuinely fatal accept() error should exit 1, got {status:?}"
    );

    assert!(
        !socket_path.exists(),
        "the socket file should be removed on the fatal path, same as any other stop"
    );
    assert_lock_released(&socket_path);

    drop(held);
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(lock_path_for(&socket_path));
}
