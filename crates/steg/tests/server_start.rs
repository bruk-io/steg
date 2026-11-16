//! `steg server start [--on-demand]` (TASK-1.77), proved through the real
//! binary this task adds - each test is named for the acceptance criterion
//! clause it proves, mapped in full in the task's own notes.

use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Kept short against `sockaddr_un`'s 104-byte `sun_path` budget
/// (`crates/server/src/bind.rs`'s own doc comment found this matters).
fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-s{suffix}.sock", std::process::id()))
}

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
fn find_server_pid(socket_path: &std::path::Path) -> Option<u32> {
    let mut lock_path = socket_path.as_os_str().to_owned();
    lock_path.push(".lock");
    let output = Command::new("lsof")
        .arg("-t")
        .arg(&lock_path)
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()
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

fn spawn_steg_server_start(socket_path: &std::path::Path, on_demand: bool) -> Child {
    let mut args = vec!["server", "start"];
    if on_demand {
        args.push("--on-demand");
    }
    Command::new(steg_binary())
        .args(&args)
        .env("STEG_SOCKET_PATH", socket_path)
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
        .env("STEG_SOCKET_PATH", socket_path)
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
    let socket_path = unique_socket_path();
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
    let socket_path = unique_socket_path();
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
    let socket_path = unique_socket_path();
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
    let not_a_socket = unique_socket_path();
    std::fs::write(&not_a_socket, b"not a socket").expect("writing the plain file should succeed");

    let output = Command::new(steg_binary())
        .args(["server", "start"])
        .env("STEG_SOCKET_PATH", &not_a_socket)
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

/// AC #5's exit-status table, the "ran and failed" row: `run`'s own accept
/// loop returns `Err` (and `main` maps that to status 1) when `accept`
/// itself fails - reached here by starting the server under a lowered
/// open-file limit and then opening real connections until it runs out,
/// confirmed empirically before writing this test (a scratch shell/Python
/// experiment, deleted after): a handful of connections against a limit of
/// 16 reliably exhausts it on this machine, varying exactly which
/// connection trips it run to run (scheduling, not a bug) but always
/// tripping within the first twenty - the real connection count here (40)
/// is deliberately generous past that observed range, not tuned to the
/// exact number.
#[test]
fn running_out_of_file_descriptors_exits_1() {
    let socket_path = unique_socket_path();
    // `ulimit` is a shell builtin, not a program - the lowered limit has to
    // be set in the same shell that then `exec`s the real binary, so it
    // actually applies to *that* process rather than only to a subshell
    // `sh` itself, which `exec` here replaces in place.
    let mut server = KillOnDrop::new(
        Command::new("sh")
            .arg("-c")
            .arg(format!(
                "ulimit -n 16; exec {} server start",
                steg_binary().display()
            ))
            .env("STEG_SOCKET_PATH", &socket_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the shell wrapper should spawn"),
    );
    wait_for_socket(&socket_path, Duration::from_secs(5));

    // Real connections, left open (never read from, never sent a hello) so
    // each keeps consuming the server's own file descriptors rather than
    // being reaped - exactly what drives `accept`'s own fd allocation to
    // fail. Ignoring a failed connect here is deliberate: once the server
    // dies, later attempts in this same loop are expected to fail too, and
    // the real assertion is the server's own exit status below, not how
    // many connections this loop itself manages to open.
    let mut held = Vec::new();
    for _ in 0..40 {
        if let Ok(stream) = UnixStream::connect(&socket_path) {
            held.push(stream);
        }
    }

    let status = server
        .child
        .wait()
        .expect("the server should exit on its own once accept fails");
    server.done = true;
    assert_eq!(
        status.code(),
        Some(1),
        "expected the server to exit 1 once its accept loop failed, got {status:?}"
    );

    drop(held);
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
    let socket_path = unique_socket_path();
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
    let socket_path = unique_socket_path();
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
    let socket_path = unique_socket_path();
    // Ends the server this test causes to be spawned indirectly, on every
    // path including a panic - declared before the client that causes it
    // to exist, so this guard's own `Drop` runs *after* `client`'s own
    // (drop order is declaration order, reversed) and the server is still
    // there, reachable by `lsof`, when this guard's own drop runs.
    let _server_guard = ServerAtSocket(socket_path.clone());
    let mut client = KillOnDrop::new(
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_autostart_client")))
            .env("STEG_SOCKET_PATH", &socket_path)
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

    let socket_path = unique_socket_path();
    // See the SIGKILL test's own comment on `_server_guard`'s declaration
    // order: this one matters even more here, since the whole point of
    // this test is that the server *survives* the SIGHUP - it is still
    // there, needing this guard, at the very end.
    let _server_guard = ServerAtSocket(socket_path.clone());
    let mut client = KillOnDrop::new(
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_autostart_client")))
            .env("STEG_SOCKET_PATH", &socket_path)
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

    client.done = true; // SIGHUP already ended it (default disposition); nothing to kill
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
    let socket_path = unique_socket_path();
    let mut fixture = KillOnDrop::new(
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_already_a_group_leader")))
            .env("STEG_SOCKET_PATH", &socket_path)
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
