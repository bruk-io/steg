//! AC #2 ("several servers started at the same moment for one socket
//! leave exactly one listening; the others find it and exit without
//! error. A socket file left by a dead server is replaced"), proved
//! with real, separate processes - the race itself cannot be proved any
//! other way, since the whole point is several processes genuinely
//! racing each other's syscalls, not this test's own scheduling of
//! threads within one process.

use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[path = "support/mod.rs"]
mod support;
use support::wait_child_bounded;

/// Bounded wait for an already-`kill()`ed child to actually exit -
/// generous, since `kill()` already ran: this is a backstop against the
/// OS somehow not reaping a `SIGKILL`ed process promptly, not a wait for
/// the process to do anything further.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// `label` names the test in a comment or a variable, never in the path
/// itself: kept short against `sockaddr_un`'s 104-byte `sun_path`
/// budget (`crates/server/src/bind.rs`'s own doc comment found this
/// matters - a first version of this helper embedded the label and
/// failed with exactly that error, not assumed safe).
fn unique_socket_path(_label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-{suffix}.sock", std::process::id()))
}

fn fixture_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_race-attempt"))
}

/// Kills and reaps its child on drop, including when a panicking
/// assertion unwinds past it - found necessary while writing this test,
/// not assumed: a winning fixture blocks forever in `run`'s accept loop,
/// and explicit cleanup written only at the end of a test function never
/// runs if an `assert_eq!` above it fails first, which orphans a real,
/// still-running process. Every spawned child in this file is wrapped
/// in this guard so "every spawned child is waited on along every path"
/// holds on the panicking path too, not only the happy one.
struct ReapOnDrop(Child);

impl ReapOnDrop {
    fn spawn(socket_path: &std::path::Path) -> Self {
        let child = Command::new(fixture_binary())
            .env("STEG_SOCKET_PATH", socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the fixture binary should spawn");
        Self(child)
    }
}

impl Drop for ReapOnDrop {
    fn drop(&mut self) {
        // Not `wait_child_bounded` here on purpose: that panics on a
        // timeout, and a panic inside `Drop` while already unwinding
        // from another panic aborts the whole process rather than
        // reporting one clean failure. `kill()` already ran, so the
        // `wait()` below is a backstop for the OS reaping an already
        // -`SIGKILL`ed process, not a wait for it to do anything
        // further.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Reads the fixture's one outcome line (`became`, `already_running`, or
/// `refuse:...`) - it prints and flushes this before it would otherwise
/// block in `run`, so a line is always there to read even for the
/// winner, which is now parked in `accept()`.
fn read_outcome_line(child: &mut ReapOnDrop) -> String {
    let stdout = child.0.stdout.take().expect("stdout was piped");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("reading the fixture's outcome line should not fail");
    line.trim().to_owned()
}

fn wait_until_accepting(path: &std::path::Path, deadline: Instant) {
    loop {
        if UnixStream::connect(path).is_ok() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "nothing ever answered at {path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Eight real processes, started at once against the same socket path:
/// exactly one becomes the server, the rest exit cleanly, and every one
/// of them exits with status 0 (the losing branch is not an error for
/// an on-demand-style caller - this task has no on-demand/explicit
/// distinction, but the arbiter's own contract is that losing the race
/// is never itself a failure).
#[test]
fn eight_simultaneous_starts_leave_exactly_one_server() {
    let socket_path = unique_socket_path("eight-way");
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };

    // `ReapOnDrop` guarantees every one of the eight is killed and
    // waited on when this vector drops - including on the panicking
    // path below, if the assertions ever fail, so a broken arbiter's
    // winners (plural, in that case) are never left running past this
    // test (found necessary while writing this test: an early version
    // cleaned up only after the assertions and left real orphaned
    // processes behind on exactly that failing path).
    let mut children: Vec<ReapOnDrop> = (0..8).map(|_| ReapOnDrop::spawn(&socket_path)).collect();
    let outcomes: Vec<String> = children.iter_mut().map(read_outcome_line).collect();

    let became = outcomes
        .iter()
        .filter(|line| line.as_str() == "became")
        .count();
    let already_running = outcomes
        .iter()
        .filter(|line| line.as_str() == "already_running")
        .count();
    assert_eq!(
        became, 1,
        "expected exactly one winner, got outcomes: {outcomes:?}"
    );
    assert_eq!(
        already_running, 7,
        "expected the other seven to find it already running, got outcomes: {outcomes:?}"
    );

    // Still waited on explicitly, with their real exit statuses checked,
    // on the happy path - `ReapOnDrop` is the backstop for the
    // panicking path, not a replacement for checking these.
    for (child, outcome) in children.iter_mut().zip(outcomes.iter()) {
        if outcome == "became" {
            child.0.kill().expect("killing the winner should succeed");
            let status = wait_child_bounded("the killed winner", &mut child.0, STOP_TIMEOUT);
            assert!(!status.success(), "a killed process does not exit 0");
        } else {
            let status = wait_child_bounded("a losing racer", &mut child.0, STOP_TIMEOUT);
            assert!(
                status.success(),
                "a losing racer should exit 0, got {status:?}"
            );
        }
    }
    drop(children);

    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&lock_path);
}

/// The clause AC #2 names distinctly from a mere stale file: the lock is
/// actually held by a real, now-dead process, and a fresh attempt wins
/// it because the kernel released it on that process's death - not
/// because anything in this task detected the death itself.
#[test]
fn a_socket_file_left_by_a_real_dead_process_is_replaced() {
    let socket_path = unique_socket_path("dead-holder");
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };

    let mut holder = ReapOnDrop::spawn(&socket_path);
    let outcome = read_outcome_line(&mut holder);
    assert_eq!(
        outcome, "became",
        "the first fixture should have become the server"
    );
    wait_until_accepting(&socket_path, Instant::now() + Duration::from_secs(5));

    holder.0.kill().expect("killing the holder should succeed");
    wait_child_bounded("the killed holder", &mut holder.0, STOP_TIMEOUT);
    drop(holder); // ReapOnDrop's own backstop re-wait is a harmless no-op here

    let config = steg_server::ServerConfig {
        socket_path: Some(socket_path.clone()),
        ..steg_server::ServerConfig::default()
    };
    let result = steg_server::attempt(&config, &socket_path).unwrap();
    match result {
        steg_server::Arbitration::Became(handle) => drop(handle),
        other
        @ (steg_server::Arbitration::AlreadyRunning | steg_server::Arbitration::Refuse(_)) => {
            panic!("expected a fresh attempt to become the server, got {other:?}")
        }
    }

    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&lock_path);
}

/// The branch `decide`'s own unit tests cannot reach on their own: the
/// lock stays genuinely contended (held by a second real file handle on
/// the same path, confirmed earlier to produce a real `EAGAIN`) for
/// longer than a short, injected `arbitration_timeout`, with nothing
/// ever answering - `attempt` must give up inside that bound, not by
/// waiting out the production default.
#[test]
fn attempt_times_out_when_the_lock_stays_contended_and_nothing_answers() {
    let socket_path = unique_socket_path("timeout");
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };

    let held = std::fs::File::create(&lock_path).unwrap();
    let _guard = nix::fcntl::Flock::lock(held, nix::fcntl::FlockArg::LockExclusiveNonblock)
        .expect("this test should be the first to lock its own unique path");

    let config = steg_server::ServerConfig {
        socket_path: Some(socket_path.clone()),
        arbitration_timeout: Duration::from_millis(100),
        ..steg_server::ServerConfig::default()
    };
    let started = Instant::now();
    let result = steg_server::attempt(&config, &socket_path).unwrap();
    let elapsed = started.elapsed();

    assert!(
        matches!(
            result,
            steg_server::Arbitration::Refuse(steg_server::ArbitrationReason::ArbitrationTimedOut)
        ),
        "expected Refuse(ArbitrationTimedOut), got {result:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "attempt should give up close to its 100ms deadline, took {elapsed:?}"
    );

    let _ = std::fs::remove_file(&lock_path);
}
