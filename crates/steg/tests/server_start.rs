//! `steg server start [--on-demand]` (TASK-1.77), proved through the real
//! binary this task adds - each test is named for the acceptance criterion
//! clause it proves, mapped in full in the task's own notes.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde_json::{Map, json};
use steg_sdk::{
    ClientKind, Connection, Incoming, Response, WireRole as Role, description, parse_server_hello,
};

#[path = "support/mod.rs"]
mod support;
use support::scoped_socket_path;

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

/// Kills and reaps its child on drop, including when a panicking assertion
/// unwinds past it (the same pattern `crates/server/tests/race.rs`'s own
/// `ReapOnDrop` already uses) - except this one is also usable to end a
/// process *mid-test*, via `kill_now`, which marks itself done so `Drop`
/// does not repeat an already-finished kill/wait a second time.
struct KillOnDrop {
    child: Child,
    done: bool,
}

impl KillOnDrop {
    fn new(child: Child) -> Self {
        Self { child, done: false }
    }

    /// Kills and reaps now, for a test that needs the process gone *before*
    /// the end of the function, not only at teardown.
    fn kill_now(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait(); // bounded: SIGKILL is not caught
        self.done = true;
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Finds the pid holding `socket_path`'s arbiter lock open, via `lsof -t` -
/// **the lock file, not the socket path itself.** Found necessary by
/// running this, not assumed: `bind_with_mode` (`crates/server/src/bind.rs`)
/// binds the real socket under a *temporary* sibling name and only renames
/// it onto `socket_path` afterward, and on this platform `lsof` reports a
/// Unix domain socket's name as whatever path it was bound to originally -
/// the rename never updates that - so `lsof -t socket_path` reliably finds
/// nothing, confirmed directly (`lsof -p <pid>` on a real leftover server
/// showed its socket fd still named `.steg-tmp-<pid>-<n>`, never the final
/// path). The lock file has no such step - `arbiter::try_lock` creates it
/// directly at its final name - so `lsof -t` on *it* works, also confirmed
/// directly before relying on it here.
///
/// **Loud, not quiet, on anything other than "exactly nothing" or "exactly
/// one pid" - a review nit found the earlier version of this function
/// went quiet in exactly the cases that matter most.** `None` (nothing
/// holds the lock) is a genuine, non-fatal answer - the server has
/// already gone. Anything else that is not exactly one pid is not: a
/// machine with no `lsof` at all must not look the same as "nothing to
/// clean up," since that is precisely how this guard would go back to
/// silently leaking servers, which is the bug this whole mechanism exists
/// to fix.
fn find_server_pid(socket_path: &std::path::Path) -> Option<u32> {
    let mut lock_path = socket_path.as_os_str().to_owned();
    lock_path.push(".lock");
    let output = Command::new("lsof")
        .arg("-t")
        .arg(&lock_path)
        .output()
        .expect(
            "lsof should be runnable on this machine - without it this guard cannot find a \
             server to clean up, and must fail loudly rather than silently leak one",
        );
    let text = String::from_utf8_lossy(&output.stdout);
    let pids: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    match pids.as_slice() {
        [] => None,
        [one] => Some(
            one.parse()
                .unwrap_or_else(|err| panic!("lsof reported a non-numeric pid {one:?}: {err}")),
        ),
        many => panic!(
            "expected lsof to report at most one pid holding {lock_path:?}, got {many:?} - \
             this guard cannot safely pick one to kill"
        ),
    }
}

/// Kills, on drop, whatever process `lsof` finds holding `socket_path`
/// open. Needed whenever a test causes a server to be spawned *indirectly*
/// (through the sdk's own `connect()`, via the `autostart_client` fixture):
/// `connect()` spawns and drops its own `Child` immediately (TASK-1.70's
/// documented behaviour), so no `Child` value this test owns ever refers to
/// that server, and once it has detached (`setsid`) it has no parent this
/// test could `wait()` on even if it somehow still held one - `setsid` only
/// changes session/group membership, never reparents a process away from
/// whoever actually forked it, but there is no fork in this picture at all
/// for *this* server: the sdk's own spawn is what created it, and the sdk
/// never hands that `Child` back. `lsof` is the one way left to find it
/// from outside. On every path, including a panicking assertion - the same
/// reason `KillOnDrop` exists for a directly-held `Child`.
///
/// Confirmed empirically, not assumed, that this is actually needed: a
/// review of this task found exactly these two tests (the only ones that
/// reach a server through this fixture rather than a directly-held
/// `Child`) each leaking one real, permanently-running `steg server start
/// --on-demand` process per run - up to 37 found alive, parent 1 (orphaned,
/// reparented once this process's own ancestor exited), since an on-demand
/// server here never stops by itself (no idle rule exists yet, TASK-1.86's)
/// and nothing was ever waiting to reap one once it detached.
struct ServerAtSocket(PathBuf);

impl Drop for ServerAtSocket {
    fn drop(&mut self) {
        if let Some(pid) = find_server_pid(&self.0) {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

/// Polls `path` repeatedly over `timeout`, failing the instant a connect
/// ever fails within that window - not a single, immediate check. Signal
/// delivery (SIGKILL, SIGHUP) is not synchronous with the syscall that sends
/// it returning, so a single check right after sending one races ahead of
/// whatever the signal was going to do; this is what actually proves
/// "stays reachable," not merely "was still reachable at one instant."
fn assert_stays_reachable(path: &std::path::Path, window: Duration, message: &str) {
    let deadline = Instant::now() + window;
    loop {
        assert!(UnixStream::connect(path).is_ok(), "{message}");
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// This piece's one named exception to "no sleeps as synchronisation": a
/// socket appearing at a path is a filesystem change this process has no
/// notification for, so every multi-process test here waits for one with a
/// short, bounded poll - the same shape `crates/server/tests/race.rs`'s own
/// `wait_until_accepting` and the sdk's own `connect()` retry loop already
/// use for the identical reason.
fn wait_for_socket(path: &std::path::Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
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

/// Waits for `child` to exit, bounded by polling `try_wait` - a review nit
/// found two places in this file that waited on a process expected to have
/// already been ended by something *other* than this function calling
/// `kill()` itself first (a signal sent elsewhere, or a resource limit this
/// process hit on its own): on a machine where that expectation does not
/// hold, a bare, unbounded `wait()` would hang the test instead of failing
/// it with a message saying what did not happen.
fn wait_bounded(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait should not fail") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the process did not exit within the bound"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn spawn_steg_server_start(socket_path: &std::path::Path, on_demand: bool) -> Child {
    let mut args = vec!["server", "start"];
    if on_demand {
        args.push("--on-demand");
    }
    Command::new(steg_binary())
        .args(&args)
        .env("STEG_SERVER", socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the real steg binary should spawn")
}

fn spawn_steg_server_start_capturing_stdout(
    socket_path: &std::path::Path,
    on_demand: bool,
) -> Child {
    let mut args = vec!["server", "start"];
    if on_demand {
        args.push("--on-demand");
    }
    Command::new(steg_binary())
        .args(&args)
        .env("STEG_SERVER", socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the real steg binary should spawn")
}

/// Reads one line from `child`'s own piped stdout, bounded by `timeout` -
/// never a bare, unbounded `read`/`BufRead::lines().next()`, since a
/// process that never prints (a bug in the code under test) must not hang
/// this test instead of failing it promptly.
fn read_one_line_bounded<R: Read + Send + 'static>(reader: R, timeout: Duration) -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    // `BufReader::read_line` itself has no timeout - the bound comes from
    // not waiting past `timeout` for this channel, not from the read call
    // itself, which is left blocked on its own thread if it never returns
    // (acceptable here: that thread ends with the test process, and the
    // alternative - a raw fd-level read timeout - would need platform
    // -specific socket/pipe options this is not worth adding for a test).
    let mut owned = BufReader::new(reader);
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = owned.read_line(&mut line);
        let _ = tx.send(line);
    });
    rx.recv_timeout(timeout)
        .expect("the process should print a line within the bound")
        .trim()
        .to_owned()
}

/// AC #1's "explicit stays up with no clients" - a real, biting assertion,
/// not an appeal to construction: nothing in `run_server_start`'s explicit
/// path *could* stop this process (a fresh `Stopper` nobody else holds), and
/// this proves that remains true by actually waiting and checking, rather
/// than reading the code for it.
#[test]
fn explicit_start_stays_up_with_no_client_ever_connecting() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut server = KillOnDrop::new(spawn_steg_server_start(&socket_path, false));
    wait_for_socket(&socket_path, Duration::from_secs(5));

    // A short, bounded wait with *no client ever connecting* - if anything
    // in the explicit path could stop itself with no clients, this would
    // either exit on its own (caught by `try_wait`) or stop accepting
    // (caught by the follow-up connect).
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        server
            .child
            .try_wait()
            .expect("try_wait should not fail")
            .is_none(),
        "an explicitly-started server with no clients should still be running"
    );
    assert!(
        UnixStream::connect(&socket_path).is_ok(),
        "the socket should still accept a connection"
    );

    server.kill_now();
}

/// AC #5's own exit-status/stdout table, proved by printing and timing: the
/// moment the "started" line arrives, a single, unretried connect attempt
/// must already succeed - `Arbitration::Became` is only ever returned once
/// `bind_with_mode` has already succeeded, so this would fail intermittently
/// if that ordering were ever reversed.
#[test]
fn started_is_printed_only_after_the_socket_already_accepts() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut server = KillOnDrop::new(spawn_steg_server_start_capturing_stdout(
        &socket_path,
        false,
    ));
    let stdout = server.child.stdout.take().expect("stdout was piped");
    let first_line = read_one_line_bounded(stdout, Duration::from_secs(5));
    assert_eq!(first_line, "started a new server");

    // No retry, no wait at all - the connect either already succeeds, right
    // now, or this assertion fails.
    assert!(
        UnixStream::connect(&socket_path).is_ok(),
        "the socket should already accept a connection the instant \"started\" is printed"
    );

    server.kill_now();
}

/// AC #1's "an interrupt leaves no socket file or lock behind," proved
/// through the real binary rather than only at the library level
/// (`crates/server/tests/race.rs`'s own
/// `a_socket_file_left_by_a_real_dead_process_is_replaced` already proves
/// the underlying mechanism; this is the same mechanism exercised by a real
/// `steg server start` instead of the `race-attempt` fixture).
#[test]
fn a_fresh_start_replaces_the_socket_of_a_server_killed_outright() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut first = KillOnDrop::new(spawn_steg_server_start(&socket_path, false));
    wait_for_socket(&socket_path, Duration::from_secs(5));
    first.kill_now();

    let mut second = KillOnDrop::new(spawn_steg_server_start_capturing_stdout(
        &socket_path,
        false,
    ));
    let stdout = second.child.stdout.take().expect("stdout was piped");
    let first_line = read_one_line_bounded(stdout, Duration::from_secs(5));
    assert_eq!(
        first_line, "started a new server",
        "a fresh start should replace the stale socket, not mistake it for a live one"
    );

    second.kill_now();
}

/// AC #5's exit-status table, the refusal row: an existing, untrusted file
/// at the socket's own path (`arbiter::decide`'s `NotASocket` branch,
/// already proven at the library level by `crates/server/src/paths.rs`'s
/// own tests) reaches status 4 and names the reason on stderr, through the
/// real binary - a row the review found untested, reachable with no new
/// mechanism (just a plain file where a socket is expected).
#[test]
fn an_untrusted_existing_file_refuses_with_status_4_and_names_the_reason() {
    let (_socket_dir, not_a_socket) = scoped_socket_path("s.sock");
    std::fs::write(&not_a_socket, b"not a socket").expect("writing the plain file should succeed");

    let output = Command::new(steg_binary())
        .args(["server", "start"])
        .env("STEG_SERVER", &not_a_socket)
        .output()
        .expect("the real steg binary should run to completion");

    assert_eq!(
        output.status.code(),
        Some(4),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not start a server"),
        "expected the reason to be named on stderr, got {stderr:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "a refusal should print nothing on stdout, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );

    let _ = std::fs::remove_file(&not_a_socket);
}

/// Real handshake over a real socket, with the sdk's own `Connection` -
/// the shape every real client (CLI, app, plugin) actually uses, not a
/// hand-rolled stand-in. Duplicated from `crates/server/tests/routing.rs`'s
/// own helper of the same name and shape: test helpers are not shared
/// across crates, and this file has never needed one before.
fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream =
        UnixStream::connect(socket_path).expect("connecting to the server should succeed");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("setting a read timeout on a fresh connection should not fail");
    let hello_bytes = Connection::send_hello("server-start-test-client", ClientKind::Cli, None);
    stream
        .write_all(&hello_bytes)
        .expect("writing the hello should not fail");

    let mut connection = Connection::new(Role::Client, description());
    let hello_reply = loop {
        if let Some(incoming) = connection.poll() {
            match incoming {
                Incoming::Hello(payload) => break payload,
                other @ (Incoming::Request { .. }
                | Incoming::Reply { .. }
                | Incoming::Event { .. }
                | Incoming::UnknownEvent { .. }
                | Incoming::StreamControl { .. }
                | Incoming::Data { .. }
                | Incoming::UnknownStream { .. }
                | Incoming::Malformed(_)) => {
                    panic!("expected the server's hello reply, got {other:?}")
                }
            }
        }
        let mut buf = [0_u8; 4096];
        let n = stream
            .read(&mut buf)
            .expect("reading the server's hello reply");
        assert!(n > 0, "server closed before replying to the hello");
        connection.receive(&buf[..n]);
    };
    parse_server_hello(&hello_reply).expect("a matching protocol version should be accepted");
    (stream, connection)
}

/// Writes a validated call and reads back its matching reply - the same
/// shape `routing.rs`'s own `call_and_read_reply` uses.
fn call_and_read_reply(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, serde_json::Value>,
) -> Response {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    stream
        .write_all(&bytes)
        .expect("writing the call should not fail");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(Incoming::Reply {
            id: reply_id,
            response,
        }) = connection.poll()
        {
            assert_eq!(reply_id, id, "the reply must carry the id this call minted");
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for a reply to {method}"
        );
        let mut buf = [0_u8; 65536];
        let n = stream.read(&mut buf).expect("reading the reply");
        assert!(n > 0, "server closed before replying");
        connection.receive(&buf[..n]);
    }
}

/// `lsof -p pid`'s own open-fd count, minus the header row it always
/// prints - `0` once the process is gone (an empty `lsof` result), never
/// an error: a caller polling this across a process's own death is
/// expected to see that, not panic over it.
fn open_fd_count(pid: u32) -> u64 {
    let output = Command::new("lsof")
        .arg("-p")
        .arg(pid.to_string())
        .output()
        .expect("lsof should be runnable on this machine");
    (String::from_utf8_lossy(&output.stdout).lines().count() as u64).saturating_sub(1)
}

/// Polls `pid`'s own open-fd count (`lsof`) until it reaches `limit`,
/// within a small margin - confirming descriptor exhaustion actually
/// happened, rather than assumed from the `-n` value and how many
/// connections a test happens to hold (a review of this task found the
/// test below asserting survival without this: 18 of 20 runs with the
/// exhaustion-handling arms disabled still passed, because most of them
/// never drove the server anywhere near its own limit at all). The same
/// bounded-poll-on-an-external-fact shape `wait_for_socket` already uses
/// in this file, for the identical reason: there is no notification for
/// an OS-level fact like this, only a fact to poll for. Fails with a
/// clear message, not a silent pass, if the limit is never reached within
/// `bound` - a runner with a different baseline then fails loudly here,
/// rather than letting the rest of this test assert things it never
/// actually exercised.
fn wait_for_descriptor_exhaustion(pid: u32, limit: u64, bound: Duration) -> u64 {
    const MARGIN: u64 = 5;
    let deadline = Instant::now() + bound;
    loop {
        let count = open_fd_count(pid);
        if count + MARGIN >= limit {
            return count;
        }
        assert!(
            Instant::now() < deadline,
            "the server's own descriptor count never reached its configured limit \
             ({limit}) within the bound - last observed {count} - exhaustion was never \
             established, so the rest of this test would not be proving anything"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// TASK-1.98 AC #3, replacing `running_out_of_file_descriptors_exits_1`
/// (named here, as the task's own notes require, since this reverses that
/// test's own expectation): that test proved the server used to die the
/// instant `accept()` failed under a lowered descriptor limit - this one
/// proves the opposite is now true for the recoverable kinds AC #1 names.
/// Descriptor exhaustion (`EMFILE`/`ENFILE`) is one of them, so the exact
/// same lowered-`ulimit`-and-hold-connections technique that test used
/// (kept for the same isolation reason: the limit must apply to the real
/// server's own *child* process, never this test binary's own) now shows
/// the server surviving it instead.
///
/// The `-n` value (180) only has to be low enough to *reach* within
/// `wait_for_descriptor_exhaustion`'s own bound, below - it is not, on its
/// own, what proves exhaustion happened (a review of this task found the
/// opposite: at the first chosen value, 128, with the exhaustion-handling
/// arms disabled by a scratch edit, 18 of 20 runs still passed, because
/// `try_wait`'s own "is the server still running" check happened before
/// the server had taken enough of its own backlog to actually hit its
/// limit - the test's own arithmetic about what *should* happen was never
/// itself a confirmation that it *did*). `wait_for_descriptor_exhaustion`
/// is that confirmation, polling the real, running server's own `lsof`
/// -reported fd count until it reaches `RLIMIT_NOFILE`, and failing this
/// test loudly, with a clear message, if it never does - so a runner with
/// a different baseline fails here instead of passing by accident. 180
/// itself is chosen with real margin over this server's own measured cost
/// (a real `server start`, steady state, with one session and one
/// client, holds 15 open descriptors on this machine, `lsof -p <pid>`
/// against an unrestricted run; TASK-1.87's own real login-shell capture,
/// which runs before `run`'s own accept loop is even reached, adds a
/// transient cost on top of that - empirically, `-n 90` already fails the
/// very first handshake, `-n 120` always passes), not chosen to be the
/// proof itself.
#[test]
fn surviving_descriptor_exhaustion_still_serves_existing_connections_and_recovers() {
    const RLIMIT_NOFILE: u64 = 180;
    const HELD_CONNECTIONS: usize = 100;

    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    // Same `sh -c 'ulimit -n N; exec ...'` wrapper the old test used, and
    // for the same reason: the lowered limit has to apply to the real
    // server process this `exec`s into, never to a subshell `sh` itself.
    let mut server = KillOnDrop::new(
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "ulimit -n {RLIMIT_NOFILE}; exec {} server start",
                steg_binary().display()
            ))
            .env("STEG_SERVER", &socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the shell wrapper should spawn"),
    );
    wait_for_socket(&socket_path, Duration::from_secs(5));

    // One real, handshaken client, held open through the whole exhaustion
    // window - its own `session.new` is the live session/program AC #1's
    // own "every session and program is untouched" and AC #3's "existing
    // connections still get replies" are both checked against, below.
    let (mut client, mut connection) = connect_and_handshake(&socket_path);
    let session = match call_and_read_reply(
        &mut client,
        &mut connection,
        "session.new",
        json!({"program": ["sh", "-c", "sleep 100"]})
            .as_object()
            .unwrap()
            .clone(),
    ) {
        Response::Ok(value) => value,
        Response::Error(error) => panic!("session.new failed: {error:?}"),
    };

    // Real connections, left open (never handshaken, never read from) so
    // each keeps consuming the server's own file descriptors rather than
    // being reaped - the same technique `running_out_of_file_descriptors_
    // exits_1` used. Ignoring a failed connect here is deliberate: once
    // descriptors are genuinely exhausted, later attempts in this same
    // loop are expected to fail too, and the real assertions are below,
    // not how many connections this loop itself manages to open.
    let mut held = Vec::new();
    for _ in 0..HELD_CONNECTIONS {
        if let Ok(stream) = UnixStream::connect(&socket_path) {
            held.push(stream);
        }
    }

    // Confirmed, not assumed: the server's own descriptor count has
    // actually reached its configured limit - this is what the rest of
    // this test's own assertions depend on meaning anything (this file's
    // own `wait_for_descriptor_exhaustion`, above).
    let pid = server.child.id();
    wait_for_descriptor_exhaustion(pid, RLIMIT_NOFILE, Duration::from_secs(5));

    // The server is still running, not dead the instant accept() first
    // failed (the whole point of this test, replacing the old one's own
    // opposite expectation).
    assert!(
        server
            .child
            .try_wait()
            .expect("try_wait should not fail")
            .is_none(),
        "the server should still be running through the exhaustion window, not dead"
    );

    // "Existing connections still get replies": the already-handshaken
    // client from before the storm can still complete a real round trip -
    // `session.list` needs no new fd of its own, so this proves the
    // connection/dispatch path survived, independent of whatever `pane.
    // split`-style fd pressure a new program start would add.
    match call_and_read_reply(&mut client, &mut connection, "session.list", Map::new()) {
        Response::Ok(value) => assert_eq!(
            value.as_array().map(Vec::len),
            Some(1),
            "the session started before the storm should still be listed, got {value:?}"
        ),
        Response::Error(error) => {
            panic!("session.list failed during the exhaustion window: {error:?}")
        }
    }

    // "Every session and program is untouched": the same session's own
    // pane, by a fully-qualified target (doc-2 section 4's own
    // `"session/tab/pane"` grammar - R94 guarantees a freshly-minted
    // session has exactly one tab and one pane, each at position 1, so
    // this needs no connection-level default context at all), still shows
    // its program running, not merely that *a* session exists.
    let session_id = session["id"].as_str().expect("session.new returns an id");
    match call_and_read_reply(
        &mut client,
        &mut connection,
        "pane.show",
        json!({"target": format!("{session_id}/1/1")})
            .as_object()
            .unwrap()
            .clone(),
    ) {
        Response::Ok(value) => {
            assert_eq!(
                value.get("program_status"),
                Some(&json!("running")),
                "the session's program should still be running, got {value:?}"
            );
        }
        Response::Error(error) => {
            panic!("pane.show on the session started before the storm failed: {error:?}")
        }
    }

    // Freeing roughly half the held connections is enough to let a fresh
    // accept() succeed again - more than that is not needed to prove
    // recovery, only that it happens at all once some descriptor frees.
    held.truncate(HELD_CONNECTIONS / 2);

    // "A new client connects once descriptors are freed": a fresh
    // handshake completes within a bound - this is also the backlog-
    // queued-client case (AC #3's own notes): this `connect()` may well
    // have been queued *during* the storm above, by the kernel, and only
    // actually accepted now that a descriptor is free - `connect_and_
    // handshake` itself blocks until that happens, bounded by its own
    // read timeout.
    let (mut fresh_client, mut fresh_connection) = connect_and_handshake(&socket_path);
    match call_and_read_reply(
        &mut fresh_client,
        &mut fresh_connection,
        "session.list",
        Map::new(),
    ) {
        Response::Ok(_) => {}
        Response::Error(error) => panic!("a fresh client's own session.list failed: {error:?}"),
    }

    // The server does not die on its own - that is the point of this
    // test - so it is ended explicitly, same as any other test here.
    server.kill_now();

    drop(held);
    drop(client);
    drop(fresh_client);
    let _ = std::fs::remove_file(&socket_path);
    let mut lock_path = socket_path.into_os_string();
    lock_path.push(".lock");
    let _ = std::fs::remove_file(lock_path);
}

/// The review's nit, taken: an unrecognised variant of `server start`
/// (extra or unknown arguments) gets doc-2 section 6's bad-usage status
/// (2), not the generic "not yet implemented" placeholder's status 0 -
/// `server start` itself is no longer unimplemented, so silently
/// succeeding on a typo'd invocation of it would be a real gap now, not
/// merely a pre-existing one.
#[test]
fn an_unrecognised_server_start_variant_exits_2() {
    let output = Command::new(steg_binary())
        .args(["server", "start", "--bogus"])
        .output()
        .expect("the real steg binary should run to completion");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

/// AC #3: several real clients starting a server at the same moment end up
/// connected to the same one. Four real, separate `steg server start
/// --on-demand` processes, test-owned so the test can distinguish winner
/// from losers deterministically (the sdk's own `connect()` never returns a
/// `Child` to do this with, and this piece has no idle-stop to lean on for
/// cleanup either - TASK-1.86's).
#[test]
fn four_simultaneous_starts_converge_on_one_server() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let socket_path = socket_path.clone();
        let barrier = std::sync::Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait(); // as close together as four threads can manage
            spawn_steg_server_start(&socket_path, true)
        }));
    }
    let mut children: Vec<KillOnDrop> = handles
        .into_iter()
        .map(|h| KillOnDrop::new(h.join().expect("spawning thread should not panic")))
        .collect();

    wait_for_socket(&socket_path, Duration::from_secs(5));

    // Bounded, repeated `try_wait()` - never a bare `wait()`, which would
    // block forever on the winner. A short window is given for the three
    // losers to have printed and exited; the winner's own `run()` never
    // returns on its own in this piece (no idle-stop), so it is never
    // mistaken for a loser by waiting longer.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut statuses = vec![None; children.len()];
    while Instant::now() < deadline && statuses.iter().any(Option::is_none) {
        for (child, status) in children.iter_mut().zip(statuses.iter_mut()) {
            if status.is_none() {
                *status = child.child.try_wait().expect("try_wait should not fail");
            }
        }
        if statuses.iter().filter(|s| s.is_some()).count() == 3 {
            break; // all three losers accounted for; the fourth is the winner
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let losers = statuses.iter().filter(|s| s.is_some()).count();
    let winners = statuses.iter().filter(|s| s.is_none()).count();
    assert_eq!(
        winners, 1,
        "exactly one of four should still be running, statuses: {statuses:?}"
    );
    assert_eq!(losers, 3, "statuses: {statuses:?}");
    for status in statuses.iter().flatten() {
        assert!(
            status.success(),
            "a losing racer should exit 0 (already running), got {status:?}"
        );
    }

    for child in &mut children {
        child.kill_now();
    }
}

/// AC #6's "what the client is left holding," proved by test, not only
/// stated: the sdk's own `connect()` never `wait()`s on a spawn it loses the
/// race with, so that spawn's dropped `Child` is a real zombie until
/// something reaps it. Reproduced with a test-owned `Command` (so the test
/// can inspect the pid directly) rather than through the sdk, which hides
/// the pid entirely.
#[test]
fn a_losing_racers_dropped_child_is_a_zombie_until_reaped() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut winner = KillOnDrop::new(spawn_steg_server_start(&socket_path, true));
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let mut loser = spawn_steg_server_start(&socket_path, true);
    let pid = loser.id();
    // *Not yet waited on* - reproducing exactly what the sdk's own
    // `connect()` does with a losing spawn (it drops the `Child` without
    // ever calling `wait()`). Holding the `Child` value itself does not
    // reap it; only `wait()`/`try_wait()` does, and neither is called
    // until after the zombie check below - std gives no other way to
    // reap a specific pid, so this test needs no new dependency for it.
    // Give the loser a moment to actually finish exiting (it should,
    // almost immediately, once it finds the socket already answering).
    std::thread::sleep(Duration::from_millis(300));

    let ps_output = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("ps should run");
    // An empty result - no row at all - is what `ps` gives for a pid
    // already reaped; asserted explicitly so that case fails loudly rather
    // than being silently read as "not a zombie."
    assert!(
        !ps_output.stdout.is_empty(),
        "expected ps to still report a row for pid {pid} (a zombie), got no output at all \
         (status {:?}) - meaning it was reaped before this check ran, which would make this \
         test prove nothing",
        ps_output.status
    );
    let stat = String::from_utf8_lossy(&ps_output.stdout);
    assert!(
        stat.contains('Z'),
        "expected pid {pid} to be a zombie (stat containing 'Z'), got {stat:?}"
    );

    // Test hygiene, not something production code does: reap the zombie
    // this test itself created, via plain `std::process::Child::wait`,
    // rather than leaving it for the test process's own eventual exit.
    loser.wait().expect("reaping the loser should succeed");

    winner.kill_now();
}

/// AC #2, the first of two tests that prove different things: `SIGKILL`
/// sent to the client's own pid never reaches the server it spawned - true
/// "for free" from how Unix process spawning works (a signal to one pid
/// does not cascade to its children), not from anything this task's own
/// `detach()` does. Named as such so it is not mistaken for proof of
/// detaching.
#[test]
fn the_client_being_sigkilled_does_not_end_the_server() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    // Ends the server this test causes to be spawned indirectly, on every
    // path including a panic - declared before the client that causes it
    // to exist, so this guard's own `Drop` runs *after* `client`'s own
    // (drop order is declaration order, reversed) and the server is still
    // there, reachable by `lsof`, when this guard's own drop runs.
    let _server_guard = ServerAtSocket(socket_path.clone());
    let mut client = KillOnDrop::new(
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_autostart_client")))
            .env("STEG_SERVER", &socket_path)
            .env("STEG_TEST_BINARY", steg_binary())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the autostart-client fixture should spawn"),
    );
    wait_for_socket(&socket_path, Duration::from_secs(5));

    client.kill_now(); // SIGKILL, the client's own pid only

    assert_stays_reachable(
        &socket_path,
        Duration::from_secs(2),
        "the server should still be answering after its client was SIGKILLed",
    );

    // `_server_guard`'s own `Drop` (declared above, before `client`) finds
    // and kills the real server this test caused to be spawned, once
    // `client` (dropped first) has already ended.
}

/// AC #2, the second test: `SIGHUP` sent to the client's *whole process
/// group* is what `detach()` actually has to survive - the controlling
/// -terminal-closing case R99 is really about. The client fixture is given
/// its own, fresh process group (`.process_group(0)`, a safe std API) so
/// this signal cannot also reach the test harness itself - a detail the
/// plan's own prose did not need to spell out, but the test cannot be
/// written safely without it, so it is recorded here as the one decision
/// this build made that the plan did not.
#[test]
fn the_clients_process_group_being_sighupd_does_not_end_the_server() {
    use std::os::unix::process::CommandExt;

    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    // See the SIGKILL test's own comment on `_server_guard`'s declaration
    // order: this one matters even more here, since the whole point of
    // this test is that the server *survives* the SIGHUP - it is still
    // there, needing this guard, at the very end.
    let _server_guard = ServerAtSocket(socket_path.clone());
    let mut client = KillOnDrop::new(
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_autostart_client")))
            .env("STEG_SERVER", &socket_path)
            .env("STEG_TEST_BINARY", steg_binary())
            .process_group(0) // its own group, so SIGHUP to -pgid cannot hit this test
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the autostart-client fixture should spawn"),
    );
    let pgid = client.child.id(); // == this pid, since `process_group(0)` made it the leader
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let status = Command::new("kill")
        .args(["-HUP", &format!("-{pgid}")])
        .status()
        .expect("kill should run");
    assert!(
        status.success(),
        "sending SIGHUP to the client's own group should succeed"
    );

    // Signal delivery is not synchronous with `kill()` returning - a single,
    // immediate connect attempt would race ahead of the server actually
    // dying if it were going to. Polled for a bounded window instead,
    // failing the instant a connect ever fails within it, so a server that
    // dies even a little after the signal is still caught.
    assert_stays_reachable(
        &socket_path,
        Duration::from_secs(2),
        "the server should still be answering after its client's whole process group was SIGHUPed",
    );

    // Waited on, bounded, rather than just marked done: SIGHUP's default
    // disposition should already have ended the client, but that is an
    // expectation about the signal, not a guarantee this function itself
    // enforced the way `kill_now`'s own `kill()` does - a review nit
    // found this left a zombie (nothing ever reaped it) until the whole
    // test binary exited.
    let status = wait_bounded(&mut client.child, Duration::from_secs(5));
    client.done = true;
    assert!(
        !status.success(),
        "the client should have been ended by the SIGHUP, not exited cleanly on its own"
    );
}

/// Section 3's own decided behaviour: typing `--on-demand` directly at an
/// interactive shell prompt makes this process already a process-group
/// leader (the same state a job-control shell's own `setpgid` produces),
/// so `detach()`'s own `setsid` fails with `EPERM` - and the decision is to
/// carry on attached with a stated message, not to refuse. Proved with the
/// `already_a_group_leader` fixture, which sets up the exact precondition
/// via a safe `exec()` in place rather than guessing at it.
#[test]
fn typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut fixture = KillOnDrop::new(
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_already_a_group_leader")))
            .env("STEG_SERVER", &socket_path)
            .env("STEG_TEST_BINARY", steg_binary())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the fixture should spawn"),
    );
    let stderr = fixture.child.stderr.take().expect("stderr was piped");
    let message = read_one_line_bounded(stderr, Duration::from_secs(5));
    assert!(
        message.contains("could not detach") && message.contains("continuing attached"),
        "expected the carry-on-attached message, got {message:?}"
    );

    let stdout = fixture.child.stdout.take().expect("stdout was piped");
    let started_line = read_one_line_bounded(stdout, Duration::from_secs(5));
    assert_eq!(
        started_line, "started a new server",
        "the server should still start - carry on, not refuse"
    );
    assert!(
        UnixStream::connect(&socket_path).is_ok(),
        "the server should be answering despite not having detached"
    );

    fixture.kill_now();
}

// --- the review finding: one parser for `server start`, not a second,
// hand-written grammar (TASK-1.49). `--on-demand`/`--no-on-demand` are an
// ordinary switch pair (doc-2 section 5): both always accepted, whichever
// is given last wins. The `already_a_group_leader` fixture (above) is the
// existing, established signal for "did this process actually try to
// detach": on-demand always attempts it (and, under this fixture's own
// precondition, always fails with the "could not detach" message);
// explicit never attempts it at all, message or not. -------------------

fn spawn_already_a_group_leader(socket_path: &Path, flags: &[&str]) -> KillOnDrop {
    KillOnDrop::new(
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_already_a_group_leader")))
            .args(flags)
            .env("STEG_SERVER", socket_path)
            .env("STEG_TEST_BINARY", steg_binary())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the fixture should spawn"),
    )
}

/// Asserts `stream` never produces a line within `window` - the same
/// bounded-wait-for-an-absence shape `crates/steg/tests/cli_no_server.rs`'s
/// own `assert_never_accepted` already uses, for the same reason: there is
/// no event to wait on instead of a timeout when proving something never
/// happens.
fn assert_stream_stays_silent(stream: impl Read + Send + 'static, window: Duration) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let mut reader = BufReader::new(stream);
        let _ = reader.read_line(&mut line);
        let _ = tx.send(line);
    });
    match rx.recv_timeout(window) {
        Err(RecvTimeoutError::Timeout) => {} // never produced a line - the expected case
        Ok(line) if line.is_empty() => {}    // the stream closed with nothing ever written
        Ok(line) => panic!("expected no message, got {line:?}"),
        Err(err) => panic!("unexpected error waiting for silence: {err:?}"),
    }
}

/// `--no-on-demand` alone resolves to the explicit kind: `run_server_start`
/// only ever attempts `detach()` when `on_demand` is true, so - unlike the
/// existing `--on-demand` test above - this process never attempts to
/// detach at all, precondition or not, and so never prints the "could not
/// detach" message either.
#[test]
fn server_start_no_on_demand_resolves_to_the_explicit_kind_and_never_attempts_to_detach() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut fixture = spawn_already_a_group_leader(&socket_path, &["--no-on-demand"]);

    let stderr = fixture.child.stderr.take().expect("stderr was piped");
    assert_stream_stays_silent(stderr, Duration::from_millis(500));

    let stdout = fixture.child.stdout.take().expect("stdout was piped");
    let started_line = read_one_line_bounded(stdout, Duration::from_secs(5));
    assert_eq!(started_line, "started a new server");
    assert!(
        UnixStream::connect(&socket_path).is_ok(),
        "the explicit server should be answering"
    );

    fixture.kill_now();
}

/// `--on-demand` given after `--no-on-demand` wins (doc-2 section 5): the
/// same on-demand behaviour the existing
/// `typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message`
/// test proves for `--on-demand` alone, proved again here for the
/// "started off, then turned back on" order, which only a single parser
/// reading the whole switch pair - not matching `server start`'s literal
/// argv - can resolve correctly.
#[test]
fn on_demand_given_after_no_on_demand_wins() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut fixture =
        spawn_already_a_group_leader(&socket_path, &["--no-on-demand", "--on-demand"]);

    let stderr = fixture.child.stderr.take().expect("stderr was piped");
    let message = read_one_line_bounded(stderr, Duration::from_secs(5));
    assert!(
        message.contains("could not detach") && message.contains("continuing attached"),
        "expected the carry-on-attached message (on-demand should have won and attempted to \
         detach), got {message:?}"
    );

    let stdout = fixture.child.stdout.take().expect("stdout was piped");
    let started_line = read_one_line_bounded(stdout, Duration::from_secs(5));
    assert_eq!(started_line, "started a new server");

    fixture.kill_now();
}

/// `--no-on-demand` given after `--on-demand` wins: the explicit kind,
/// never attempting to detach - the mirror of the test above, proving the
/// same "last one wins" rule from the other starting order.
#[test]
fn no_on_demand_given_after_on_demand_wins() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let mut fixture =
        spawn_already_a_group_leader(&socket_path, &["--on-demand", "--no-on-demand"]);

    let stderr = fixture.child.stderr.take().expect("stderr was piped");
    assert_stream_stays_silent(stderr, Duration::from_millis(500));

    let stdout = fixture.child.stdout.take().expect("stdout was piped");
    let started_line = read_one_line_bounded(stdout, Duration::from_secs(5));
    assert_eq!(started_line, "started a new server");
    assert!(
        UnixStream::connect(&socket_path).is_ok(),
        "the explicit server should be answering"
    );

    fixture.kill_now();
}

/// `server start --help` prints the tree's own generated help (built from
/// the description, the same as every other command) and starts nothing -
/// doc-2 section 5: `--help` "contacts no server", checked here for the
/// one command this task's generated tree hands off to hand-written
/// behaviour, not only for the commands that stay on the placeholder.
#[test]
fn server_start_help_prints_the_generated_help_and_starts_nothing() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    let output = Command::new(steg_binary())
        .args(["server", "start", "--help"])
        .env("STEG_SERVER", &socket_path)
        .output()
        .expect("the real steg binary should run to completion");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--on-demand") && stdout.contains("--no-on-demand"),
        "expected the generated help to list both switch flags, got {stdout:?}"
    );
    assert!(
        UnixStream::connect(&socket_path).is_err(),
        "--help must start nothing"
    );
}
