//! The shutdown ordering part of AC #5 ("the server runs in the
//! foreground... so this part is testable alone") and the seam part B's
//! idle checker will use (`Stopper`): stop accepting, close the
//! listener, remove the socket file, release the lock last - proved
//! directly, not assumed, and bounded by a channel rather than a sleep.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use steg_sdk::{ClientHello, ClientKind, Decoder, Frame, PROTOCOL_VERSION, encode_frame};
use steg_server::{ConnectionEvent, ServeOutcome, ServerConfig, Stopper, serve};

/// Kept short against `sockaddr_un`'s 104-byte `sun_path` budget
/// (`crates/server/src/bind.rs`'s own doc comment found this matters).
fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-{suffix}.sock", std::process::id()))
}

fn wait_until_accepting(path: &std::path::Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if UnixStream::connect(path).is_ok() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "server never started accepting at {path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Starts `serve` on a joined-on-cleanup background thread with the
/// given `handshake_timeout`, waiting on the socket actually accepting
/// (not a sleep) before returning. `events`, when given, is wired
/// through to `serve` so a test can count connections and
/// disconnections as they happen rather than guessing at timing.
fn start_server(
    handshake_timeout: Duration,
    events: Option<mpsc::Sender<ConnectionEvent>>,
) -> (PathBuf, ServerConfig, Stopper, std::thread::JoinHandle<()>) {
    let socket_path = unique_socket_path();
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout,
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let server_thread = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, events);
        })
    };
    wait_until_accepting(&socket_path);
    (socket_path, config, stop, server_thread)
}

/// Starts a server exactly like `start_server`, then additionally
/// drains the one connection `wait_until_accepting`'s own probe leaves
/// queued, so `run`'s accept loop has genuinely cycled back to a fresh,
/// empty, blocking `accept()` call before this returns.
///
/// **Without this, a test proving `stop()` wakes a genuinely idle
/// server does not actually prove that** - found while writing these
/// tests, not assumed. A successful `connect()` only means the kernel
/// queued the connection; it does not require `run`'s own `accept()` to
/// have consumed it yet. So `run`'s *first* `accept()` call can return
/// that leftover probe connection regardless of whether `stop()`
/// properly wakes a blocked `accept()` at all - and once that first
/// `accept()` returns anything, `run` re-checks `should_stop()`
/// immediately, which `set_stop_flag()` alone already makes true. A
/// test that calls `start_server` then `stop()` would pass even with a
/// `stop()` that never wakes anything, purely because of this leftover
/// connection - exactly the bug a review of this task found, masked by
/// the test's own setup rather than caught by it. Draining the probe's
/// own `Connected`/`Disconnected` pair confirms `run`'s loop has
/// already spawned a handler for it and gone back to `accept()` a
/// second time - which, with nothing else queued, can only be blocking.
fn start_truly_idle_server(
    handshake_timeout: Duration,
) -> (PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let (events_tx, events_rx) = mpsc::channel();
    let socket_path = unique_socket_path();
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout,
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let server_thread = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, Some(events_tx));
        })
    };
    wait_until_accepting(&socket_path);
    events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the startup probe's Connected event should arrive");
    events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the startup probe's Disconnected event should arrive");
    (socket_path, stop, server_thread)
}

/// Joins `server_thread` within `bound`, failing the test rather than
/// hanging the suite if `run` has not returned by then - every test
/// that proves "stop returns promptly" is bounded this way, never by an
/// un-timed `join()`.
fn join_within(server_thread: std::thread::JoinHandle<()>, bound: Duration) {
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let waiter = std::thread::spawn(move || {
        server_thread
            .join()
            .expect("the server thread should not panic");
        let _ = done_tx.send(());
    });
    done_rx
        .recv_timeout(bound)
        .unwrap_or_else(|_| panic!("run should have returned within {bound:?} of being stopped"));
    waiter.join().expect("the waiter thread should not panic");
}

/// Sends a well-formed hello over `stream` and reads back the server's
/// reply far enough to know the handshake completed - this test only
/// needs "accepted", not the reply's exact contents (already proved by
/// `handshake.rs`).
fn complete_handshake(stream: &mut UnixStream) {
    let hello = ClientHello::new("test-client", PROTOCOL_VERSION, ClientKind::Cli, None);
    let payload = serde_json::to_vec(&hello).expect("a ClientHello should serialise");
    let bytes = encode_frame(&Frame::hello(payload)).expect("a hello frame should encode");
    stream
        .write_all(&bytes)
        .expect("writing the hello should not fail");

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("setting a read timeout on a fresh connection should not fail");
    let mut decoder = Decoder::new();
    loop {
        if decoder
            .next_frame()
            .expect("a well-formed frame from the server")
            .is_some()
        {
            return;
        }
        let mut buf = [0_u8; 4096];
        let n = stream
            .read(&mut buf)
            .expect("reading the server's reply should not fail");
        assert!(n > 0, "the server closed before replying to the hello");
        decoder.push(&buf[..n]);
    }
}

#[test]
fn stopping_closes_the_listener_removes_the_socket_and_releases_the_lock() {
    let socket_path = unique_socket_path();
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let (done_tx, done_rx) = mpsc::channel();
    let server_thread = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let outcome = serve(&config, &stop, None);
            let _ = done_tx.send(outcome.map(|o| matches!(o, ServeOutcome::Became)));
        })
    };
    wait_until_accepting(&socket_path);

    stop.stop();

    // Bounded on a real event (`serve` returning, sent over the
    // channel), not a sleep.
    let became = done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("serve should return promptly once stopped")
        .expect("serve should not itself fail");
    assert!(
        became,
        "this process should have been the one that became the server"
    );
    // Joined, not detached: the team rule this task picked up mid-build
    // (every spawned thread joined on every path) applies to this
    // test's own server thread too, not only to `run`'s connection
    // threads.
    server_thread
        .join()
        .expect("the server thread should not panic");

    assert!(
        !socket_path.exists(),
        "the socket file should be removed on shutdown"
    );

    // A client connecting now sees exactly what it would see against a
    // server that never existed - `ConnectionRefused` or `NotFound`,
    // both indistinguishable from "nothing is listening", which is the
    // correct, safe reading (the sdk's own `local_socket::connect`
    // treats either the same way: try to start one).
    let err = UnixStream::connect(&socket_path).unwrap_err();
    assert!(
        matches!(
            err.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
        ),
        "expected NotFound or ConnectionRefused, got {err:?}"
    );

    // The lock is released too: a fresh attempt at the same path should
    // not find it contended.
    let second = steg_server::attempt(&config, &socket_path).unwrap();
    assert!(
        matches!(second, steg_server::Arbitration::Became(_)),
        "the lock should have been released, letting a fresh attempt become the server"
    );
    if let steg_server::Arbitration::Became(handle) = second {
        drop(handle);
    }
    let _ = std::fs::remove_file(&socket_path);
    let _ = std::fs::remove_file(&lock_path);
}

/// AC #4's second clause ("one that has completed [the handshake] is
/// not closed for being idle") meets the shutdown path here: a client
/// that finished its handshake and is now sitting idle still has to let
/// `stop` win. Without `run` shutting down and joining every open
/// connection (confirmed by removing that step and recording the
/// failure, then restoring it - see this task's notes), that client's
/// thread blocks forever in its hold-open read and `run` never returns.
#[test]
fn a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop() {
    let (socket_path, _config, stop, server_thread) = start_server(Duration::from_secs(30), None);

    let mut client = UnixStream::connect(&socket_path).unwrap();
    complete_handshake(&mut client);
    // Set before `stop`, not after: on this platform, setting a read
    // timeout on a stream whose peer has already shut down its side
    // fails with `EINVAL` (confirmed by a standalone reproduction, not
    // assumed) - stopping first and setting the timeout afterwards
    // would make this assertion flaky on exactly the prompt-shutdown
    // behaviour this test means to prove.
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    stop.stop();
    join_within(server_thread, Duration::from_secs(5));

    // The client sees its connection closed, not merely a slow server:
    // a bounded read observes `Ok(0)` (or a reset, the same legitimate
    // close `handshake.rs` already allows for).
    let mut buf = [0_u8; 16];
    match client.read(&mut buf) {
        Ok(0) => {}
        Ok(n) => panic!("expected the connection to close, got {n} bytes"),
        Err(err) => assert_ne!(
            err.kind(),
            std::io::ErrorKind::WouldBlock,
            "the client should see its connection closed well within the 5s bound"
        ),
    }

    let _ = std::fs::remove_file(&socket_path);
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let _ = std::fs::remove_file(&lock_path);
}

/// A stop issued while a connection is mid-handshake must not wait out
/// that connection's own (here, deliberately long) handshake timeout -
/// `run` has to shut it down directly. The 30s timeout only guards
/// against the opposite regression (joining a stalled handshake thread
/// without first shutting its stream down, which would make this test
/// time out instead of passing quickly); `handshake.rs`'s own tests are
/// what prove the timeout itself closes a stalled handshake when stop
/// is never called at all.
#[test]
fn a_stalled_handshake_is_closed_and_run_returns_promptly_on_stop() {
    let (socket_path, _config, stop, server_thread) = start_server(Duration::from_secs(30), None);

    let client = UnixStream::connect(&socket_path).unwrap();
    // Sends nothing - this is deliberately the stalled-handshake case,
    // not the completed one above.

    stop.stop();
    join_within(server_thread, Duration::from_secs(5));

    drop(client);
    let _ = std::fs::remove_file(&socket_path);
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let _ = std::fs::remove_file(&lock_path);
}

/// Counts open file descriptors in this test's own process - `serve`
/// runs on a thread inside it, so `run`'s tracked `connections` leaking
/// a `try_clone`d stream per past connection shows up here directly,
/// with no accessor added to shipped code for the purpose (rejected
/// elsewhere on this task as a test hook in production code).
fn open_fd_count() -> usize {
    std::fs::read_dir("/dev/fd")
        .expect("/dev/fd should be readable on this platform")
        .count()
}

/// After many connections have each completed a handshake and closed on
/// their own, the set `run` tracks internally does not grow with the
/// server's whole history - only with how many are concurrently open.
/// Proved indirectly, through this process's own descriptor count (the
/// only honest window into `run`'s private state without an accessor):
/// run many short-lived connections, drain their `Disconnected` events
/// so each is known finished before counting, make one more connection
/// to trigger a reap pass inside `run` (reaping happens on accept, so
/// the last batch is only collected once another connection arrives),
/// then compare. `listen.rs`'s own `reap_finished` unit tests cover the
/// removal logic directly; this is the end-to-end case that the real
/// loop actually calls it.
#[test]
fn many_short_connections_do_not_grow_the_tracked_set() {
    let (events_tx, events_rx) = mpsc::channel();
    let (socket_path, _config, stop, server_thread) =
        start_server(Duration::from_secs(5), Some(events_tx));

    let before = open_fd_count();

    const ROUNDS: usize = 200;
    for _ in 0..ROUNDS {
        let mut client = UnixStream::connect(&socket_path).unwrap();
        complete_handshake(&mut client);
        drop(client); // closes on its own - the server's thread notices and exits
        events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Connected should arrive"); // Connected
        events_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Disconnected should arrive"); // Disconnected
    }

    // One more connection: the reap pass inside `run`'s loop runs right
    // after this `accept()` returns, collecting every one of the 200
    // that finished above.
    let mut trigger = UnixStream::connect(&socket_path).unwrap();
    complete_handshake(&mut trigger);
    events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Connected should arrive for the trigger connection");
    drop(trigger);
    events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Disconnected should arrive for the trigger connection");

    let after = open_fd_count();

    stop.stop();
    join_within(server_thread, Duration::from_secs(5));

    assert!(
        after < before + 10,
        "fd count should not grow with the number of past connections: before={before}, after={after}"
    );

    let _ = std::fs::remove_file(&socket_path);
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let _ = std::fs::remove_file(&lock_path);
}

/// The regression a review of this task found: a genuinely idle
/// server, meaning no connection ever arriving on its own to make
/// `accept()` return and notice the flag, used to leave `run` blocked
/// forever on a bare `Stopper::stop()`. Uses `start_truly_idle_server`,
/// not `start_server` (see its own doc for why that distinction is
/// load-bearing here): the 30s handshake timeout given is deliberately
/// far longer than this test's 5s bound, so a pass cannot be explained
/// by anything but `stop()` itself waking the accept loop.
#[test]
fn stop_on_a_genuinely_idle_server_returns_from_run_within_a_bound() {
    let (socket_path, stop, server_thread) = start_truly_idle_server(Duration::from_secs(30));

    stop.stop();
    join_within(server_thread, Duration::from_secs(5));

    let _ = std::fs::remove_file(&socket_path);
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let _ = std::fs::remove_file(&lock_path);
}

/// `Stopper::stop`'s own doc promises idempotence, including
/// concurrently - two threads calling it at once on the same idle
/// server should be no different from one. `start_truly_idle_server`,
/// not `start_server`, for the same reason as the test above.
#[test]
fn stop_called_twice_concurrently_on_an_idle_server_is_harmless() {
    let (socket_path, stop, server_thread) = start_truly_idle_server(Duration::from_secs(30));

    let first = stop.clone();
    let second = stop.clone();
    let a = std::thread::spawn(move || first.stop());
    let b = std::thread::spawn(move || second.stop());
    a.join().expect("the first stop() should not panic");
    b.join().expect("the second stop() should not panic");

    join_within(server_thread, Duration::from_secs(5));

    let _ = std::fs::remove_file(&socket_path);
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let _ = std::fs::remove_file(&lock_path);
}

/// `stop()` racing a real client's `connect()` - whichever one the
/// kernel schedules first, `run` must still return within the bound.
/// Run several times (not once) because a single run only ever
/// exercises whichever interleaving the scheduler happens to produce;
/// looping gives both orderings a real chance to occur rather than
/// relying on luck once.
///
/// **This does not, on its own, prove `stop()`'s wakeup mechanism
/// matters** - found while writing the idle tests above: as long as
/// the listener is still open (which it is, for as long as `run`
/// hasn't yet returned), the connector thread's own `connect()`
/// succeeds and queues a connection regardless of whether `stop()`
/// woke anything, and that connection, once consumed by `accept()`,
/// would let `run` notice an already-set flag and return even under a
/// `stop()` with no wakeup at all - the same masking the idle tests
/// above exist to avoid. What this test proves instead, honestly: that
/// `stop()` and a real, concurrently connecting client do not
/// deadlock, panic, or leave anything orphaned, in either ordering -
/// a real property, just not this review finding's own regression,
/// which the two tests above are what actually cover.
#[test]
fn stop_racing_a_client_connecting_still_returns_within_a_bound() {
    const ROUNDS: usize = 20;
    for round in 0..ROUNDS {
        let (socket_path, _config, stop, server_thread) =
            start_server(Duration::from_secs(30), None);

        let connect_path = socket_path.clone();
        let connector = std::thread::spawn(move || {
            // Whether this lands before or after `stop` wins the race
            // is exactly what this test does not control - either
            // way, `run` must still return within the bound below.
            // Both an `Ok` (accepted, then closed by the shutdown) and
            // an `Err` (refused, because the listener had already
            // closed) are fine outcomes for this thread; only `run`
            // itself hanging would fail this test.
            let _ = UnixStream::connect(&connect_path);
        });
        stop.stop();
        connector
            .join()
            .unwrap_or_else(|_| panic!("the connector thread should not panic on round {round}"));

        join_within(server_thread, Duration::from_secs(5));

        let _ = std::fs::remove_file(&socket_path);
        let lock_path = {
            let mut p = socket_path.as_os_str().to_owned();
            p.push(".lock");
            PathBuf::from(p)
        };
        let _ = std::fs::remove_file(&lock_path);
    }
}

/// A trickling, mid-handshake client (`handshake.rs`'s own
/// `a_client_trickling_one_byte_at_a_time_is_still_closed_at_the_deadline`
/// proves the handshake deadline itself closes it) must not also be
/// able to make a *stop* wait on it. This is a guard, not a new
/// discovery: the mechanism - `shutdown(Both)` on the tracked clone
/// unblocking the thread's own blocked `read()` regardless of what
/// read timeout is set - is the same one
/// `a_completed_idle_connection_is_closed_and_run_returns_promptly_on_stop`
/// already proved, and a trickling connection's thread is blocked in
/// exactly that kind of `read()` call (just with a shrinking deadline
/// instead of no timeout at all) when `stop` is called. Driven off the
/// events channel rather than a sleep, so `stop()` is called only once
/// the trickler is confirmed connected and mid-handshake, not merely
/// "probably by now".
#[test]
fn stopping_while_a_trickling_handshake_is_in_progress_still_returns_within_a_bound() {
    let (events_tx, events_rx) = mpsc::channel();
    // Long enough that the handshake's own deadline is not what ends
    // this test - only `stop()` is.
    let (socket_path, _config, stop, server_thread) =
        start_server(Duration::from_secs(30), Some(events_tx));

    let mut trickler = UnixStream::connect(&socket_path).unwrap();
    events_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Connected should arrive for the trickler");

    let trickler_thread = std::thread::spawn(move || {
        // A declared length `Decoder::next_frame` accepts but never
        // completes, the same shape `handshake.rs`'s own trickle test
        // uses - this connection must still be genuinely mid-handshake
        // (nothing decoded yet) when `stop()` below fires.
        let declared_len: u32 = 1_000_000;
        let mut bytes = declared_len.to_be_bytes().to_vec();
        bytes.extend(std::iter::repeat_n(0_u8, 20));
        for byte in &bytes {
            if trickler.write_all(std::slice::from_ref(byte)).is_err() {
                break; // the server already closed its side - stop() won
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    });

    stop.stop();
    join_within(server_thread, Duration::from_secs(5));
    trickler_thread
        .join()
        .expect("the trickler thread should not panic");

    let _ = std::fs::remove_file(&socket_path);
    let lock_path = {
        let mut p = socket_path.as_os_str().to_owned();
        p.push(".lock");
        PathBuf::from(p)
    };
    let _ = std::fs::remove_file(&lock_path);
}
