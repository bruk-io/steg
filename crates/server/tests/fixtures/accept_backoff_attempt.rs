//! `crates/server/tests/accept_backoff.rs`'s own fixture (TASK-1.98 AC #1/
//! #2/#4): a real server, in its own dedicated process, with its own
//! `RLIMIT_NOFILE` lowered so a real `EMFILE`/`ENFILE` happens when the
//! test holds enough connections open - never shared with any other test,
//! which is what makes lowering a *process-wide* limit safe here (unlike
//! lowering it for the shared `cargo test` harness process). Reports each
//! recovered accept error over stdout, the same "print a line the real
//! test process reads via `BufReader`" shape `race_attempt.rs` already
//! uses, since the test process has no other way to see what happened
//! inside this one's own memory.
//!
//! Two optional behaviours, each driven by how many `AcceptRetried` events
//! this fixture has itself observed so far, chosen by which env var (if
//! either) the test sets:
//! - `STOP_AFTER_RETRIES`: calls its own `Stopper::stop()` once that many
//!   have arrived (AC #1's stop-path fix: proves a stop asked mid-storm
//!   ends this process promptly, not only once the storm resolves on its
//!   own).
//! - `DUP2_AFTER_RETRIES`: `dup2`s an open `/dev/null` over the listener's
//!   own raw fd once that many have arrived (AC #2: the next `accept()`
//!   then sees a non-socket and fails with `ENOTSOCK`, a real, fatal error
//!   through the real loop - safe specifically because the test keeps
//!   every exhausting connection open right up to this moment, so the loop
//!   is provably mid-backoff, never blocked in a live `accept()`, when this
//!   runs).

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use nix::sys::resource::{Resource, rlim_t, setrlimit};
use steg_server::{Arbitration, ConnectionEvent, ServerConfig, Stopper, attempt, run};

/// `FIXTURE_RLIMIT_NOFILE`'s own default - comfortably above what binding
/// the socket, holding its lock, and one real session's own pty cost, so
/// `attempt`'s bind below does not itself fail; the test's own held-open
/// connections are what push past it, not this fixture's own baseline use.
const DEFAULT_RLIMIT_NOFILE: rlim_t = 48;

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
}

fn main() {
    // First, before anything else opens a single fd - the same ordering
    // AC #3's own real-binary test uses (`ulimit -n N; exec steg server
    // start`), so this fixture's own baseline cost (the socket, its lock)
    // is paid *inside* the lowered limit, not before it.
    let rlimit_nofile = env_u64("FIXTURE_RLIMIT_NOFILE").unwrap_or(DEFAULT_RLIMIT_NOFILE);
    setrlimit(Resource::RLIMIT_NOFILE, rlimit_nofile, rlimit_nofile)
        .expect("accept-backoff-attempt: setrlimit should succeed on this platform");

    let socket_path = PathBuf::from(
        std::env::var_os("STEG_SOCKET_PATH")
            .expect("accept-backoff-attempt: STEG_SOCKET_PATH must be set"),
    );
    let accept_error_backoff = env_u64("ACCEPT_BACKOFF_MS")
        .map(Duration::from_millis)
        .unwrap_or(steg_server::DEFAULT_ACCEPT_ERROR_BACKOFF);
    let stop_after_retries = env_u64("STOP_AFTER_RETRIES");
    let dup2_after_retries = env_u64("DUP2_AFTER_RETRIES");

    // Opened now, before any exhaustion - found necessary, not assumed:
    // opening this *after* the storm has already started (inside the
    // reporter thread, at the moment of injection) raced the very
    // condition this fixture exists to simulate, since a new fd is a new
    // fd regardless of which thread asks for it, and a process already at
    // `RLIMIT_NOFILE` fails to open *anything* new, including this dummy
    // file - which silently wedged the whole fixture (the reporter thread
    // panicked on the failed `open`, `run`'s own listener was never
    // touched, and the loop kept retrying against the real, undisturbed
    // listener forever). Pre-opening spends this fd's own slot while
    // budget still allows it; the later `dup2` only ever redirects the
    // listener's *existing* slot, never allocates a new one, so it stays
    // safe to do even once the process is genuinely exhausted.
    let dup2_target_fd = dup2_after_retries.map(|_| {
        let dummy = File::open("/dev/null")
            .expect("accept-backoff-attempt: /dev/null should open before any exhaustion");
        let fd = std::os::fd::AsRawFd::as_raw_fd(&dummy);
        // Leaked deliberately: closing `dummy` here would immediately
        // invalidate the fd this fixture is about to hold onto until the
        // `dup2` call uses it, arbitrarily later.
        std::mem::forget(dummy);
        fd
    });

    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        accept_error_backoff,
        // Long and explicit, not the production default
        // (`DEFAULT_HANDSHAKE_TIMEOUT`, 10s): what actually keeps
        // exhaustion sustained for the whole of a test's own measurement
        // window is every *accepted*, never-handshaken connection's own
        // two fds staying held until this fires - a value this fixture
        // states for itself, not one it inherits from a constant another
        // task owns and could change independently of this one.
        handshake_timeout: Duration::from_secs(60),
        ..ServerConfig::default()
    };

    let handle = match attempt(&config, &socket_path) {
        Ok(Arbitration::Became(handle)) => handle,
        other => {
            // Never expected in this fixture's own tests (each uses a
            // fresh, unique socket path) - printed rather than panicking
            // so a test that does hit this sees why in its own captured
            // stdout, not a bare process exit code.
            println!("did_not_become:{other:?}");
            std::io::stdout().flush().ok();
            return;
        }
    };

    // Captured before `handle` moves into `run`, below - `ServerHandle`'s
    // own fields are otherwise `pub(crate)` (`arbiter.rs`'s own doc for
    // why), this accessor is the one, feature-gated exception this task
    // adds for exactly this fixture.
    let listener_raw_fd = handle.listener_raw_fd();

    let stopper = Stopper::new();
    let (events_tx, events_rx) = mpsc::channel::<ConnectionEvent>();

    // How often the reporter thread below prints, at most - independent of
    // how often an `AcceptRetried` event itself arrives. Found necessary
    // while building this task, not assumed: printing a flushed line on
    // *every* event makes the real syscall cost of that print the actual
    // bottleneck on the count this fixture reports, which collapses the
    // very distinction `accept_backoff.rs`'s own bound/spin tests exist to
    // measure (a first version of this fixture printed unconditionally and
    // reported a near-identical, heavily-throttled count whether
    // `ACCEPT_BACKOFF_MS` was 0 or 20). `retries` itself still increments
    // once per event, immediately, on the fast path below - only the
    // printing is paced.
    const REPORT_INTERVAL: Duration = Duration::from_millis(20);

    let reporter = {
        let stopper = stopper.clone();
        std::thread::spawn(move || {
            let mut retries: u64 = 0;
            let mut last_report = std::time::Instant::now();
            for event in events_rx {
                if let ConnectionEvent::AcceptRetried(_) = event {
                    retries += 1;

                    if last_report.elapsed() >= REPORT_INTERVAL {
                        println!("count {retries}");
                        std::io::stdout()
                            .flush()
                            .expect("stdout is a real pipe in this fixture's own test");
                        last_report = std::time::Instant::now();
                    }

                    if stop_after_retries == Some(retries) {
                        stopper.stop();
                    }
                    if dup2_after_retries == Some(retries) {
                        // The next `accept()` call on this exact fd number
                        // now operates on a regular file, which fails with
                        // `ENOTSOCK` immediately (confirmed in a
                        // standalone probe during planning, not assumed) -
                        // never blocked, so this is safe to do from this
                        // side thread regardless of exactly when the main
                        // thread's loop next calls `accept()`. Redirects
                        // the listener's own, already-allocated slot;
                        // allocates nothing new of its own (see
                        // `dup2_target_fd`'s own doc, above, for why that
                        // distinction is load-bearing here).
                        nix::unistd::dup2(
                            dup2_target_fd.expect("set above whenever DUP2_AFTER_RETRIES is"),
                            listener_raw_fd,
                        )
                        .expect(
                            "accept-backoff-attempt: dup2 onto the listener's own fd should succeed",
                        );
                    }
                }
            }
            // A final report once the channel closes (`run` returning, in
            // practice) - without this, a run shorter than one
            // `REPORT_INTERVAL` (every test below waits for at least one
            // line, via `wait_for_first`) would report nothing at all.
            println!("count {retries}");
            std::io::stdout().flush().ok();
        })
    };

    let outcome = run(handle, &socket_path, &config, &stopper, Some(events_tx));

    // Joined before this process exits, on this, the only path out of
    // `main` - `reporter`'s own loop ends on its own once `run` drops
    // every clone of `events_tx` it was given (part of its own normal
    // shutdown, not something this fixture has to force).
    reporter
        .join()
        .expect("the reporter thread should not panic");

    match outcome {
        Ok(()) => std::process::exit(0),
        Err(_) => std::process::exit(1),
    }
}
