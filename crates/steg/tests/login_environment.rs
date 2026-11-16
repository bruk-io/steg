//! AC #1 (doc-8 R99): a pane's program starts with a login shell's own
//! environment, not the client's - proved end to end, through the real
//! `steg` binary, a real on-demand server, and a real client
//! (`tests/fixtures/env_leak_client.rs`).
//!
//! **One departure from the brief's own "throwaway home directory"
//! phrasing, stated plainly rather than silently approximated.**
//! `crate::login_env::resolve_login_identity` reads the password database
//! for this process's own real effective uid - by design, since reading
//! `$HOME`/`$SHELL` from the environment instead is exactly what R99
//! forbids. That means this end-to-end test cannot redirect identity
//! resolution to a fabricated shell/home the way `login_env.rs`'s own
//! `#[cfg(test)]` module already does (there, `capture_login_environment`
//! takes `shell`/`home` as plain parameters - no production code path
//! reads them from anywhere ambient). Two ways to get a throwaway home
//! into this real, whole-binary path were considered and rejected:
//!
//! - **Mutating this real account's own `~/.zprofile`** (temporarily,
//!   restored after) - rejected outright: a crashed test leaves a
//!   developer's real shell configuration corrupted, and a concurrently
//!   open terminal sourcing the same file mid-write is a real hazard, not
//!   a theoretical one.
//! - **Seeding `ZDOTDIR`** - confirmed empirically that zsh honours it for
//!   dotfile lookup while leaving `$HOME` itself untouched (`env -i
//!   HOME=<real> ZDOTDIR=<throwaway> ... zsh -l -c env` sources the
//!   throwaway directory's own `.zprofile`), which would have let this
//!   test use a real throwaway profile with no dotfile mutation at all -
//!   but the only way to reach it from this real binary is either reading
//!   `ZDOTDIR` out of the *server's own* ambient environment (which is
//!   exactly the kind of client-set variable reaching a login shell's own
//!   profile-sourcing step that R99 exists to prevent - a client could set
//!   `ZDOTDIR` to point at an attacker-controlled directory), or a
//!   test-only hook in the shipped binary (this team's own convention
//!   disfavours one, `TASK-1.77`'s own AC #5). Neither is acceptable for
//!   production code reachable this way.
//!
//! What this test proves instead, safely, through the real account's own
//! real identity: **the leak-absence half of AC #1** (a variable set only
//! in the launching client's own environment is absent from the pane,
//! `CARGO_PKG_NAME`-style - here, a value unique to this test run,
//! inherited by the real detached server from the real fixture client
//! that autostarted it), and **that a real capture actually happened, not
//! merely the fallback** (the pane's own `PATH` carries a `path_helper`
//! -specific entry only a real macOS login shell's own profile produces,
//! never `FALLBACK_PATH`'s plain four directories) and **AC #3 through the
//! real binary** (`TERM=xterm-256color`, whatever the login shell's own
//! profile might otherwise export). The throwaway-home-specific proof -
//! that a login shell's own start-up *file* is what is sourced, not merely
//! "the real one happens to work" - is `crates/server/src/login_env.rs`'s
//! own `capture_login_environment_sources_the_login_profile_not_the_
//! interactive_one` test, with a fabricated shell/home pair no production
//! code path can reach ambiently.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-le{suffix}.sock", std::process::id()))
}

fn unique_output_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "steg-login-env-e2e-{}-{suffix}.out",
        std::process::id()
    ))
}

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

fn env_leak_client_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_env_leak_client"))
}

/// Kills and reaps its child on drop, including when a panicking assertion
/// unwinds past it - `crates/steg/tests/server_start.rs`'s own
/// `KillOnDrop`, duplicated here rather than shared across test binaries.
struct KillOnDrop {
    child: Child,
    done: bool,
}

impl KillOnDrop {
    fn new(child: Child) -> Self {
        Self { child, done: false }
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

/// Finds the pid holding `socket_path`'s arbiter lock open, via `lsof -t`
/// on the lock file - `server_start.rs`'s own `find_server_pid`,
/// duplicated here for the same reason `KillOnDrop` is.
fn find_server_pid(socket_path: &std::path::Path) -> Option<u32> {
    let mut lock_path = socket_path.as_os_str().to_owned();
    lock_path.push(".lock");
    let output = Command::new("lsof")
        .arg("-t")
        .arg(&lock_path)
        .output()
        .expect("lsof should be runnable on this machine");
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
        many => {
            panic!("expected lsof to report at most one pid holding {lock_path:?}, got {many:?}")
        }
    }
}

/// Kills, on drop, whatever process `lsof` finds holding `socket_path`
/// open - the real server this test's own fixture caused to be spawned
/// (and detached) indirectly, never held as a `Child` this test owns
/// directly. `server_start.rs`'s own `ServerAtSocket`.
struct ServerAtSocket(PathBuf);

impl Drop for ServerAtSocket {
    fn drop(&mut self) {
        if let Some(pid) = find_server_pid(&self.0) {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

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

/// Polls for `path` to exist and be non-empty - the real program inside
/// the real pane needs a moment to start and write it, and there is no
/// event this process can wait on instead (the same reasoning this
/// workspace's own multi-process tests already give for polling a
/// socket's own appearance).
fn wait_for_output(path: &std::path::Path, timeout: Duration) -> String {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && !text.is_empty()
        {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "the pane's own program never wrote {path:?} within {timeout:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_pane_started_by_a_real_on_demand_server_never_inherits_the_launching_clients_own_environment()
{
    let socket_path = unique_socket_path();
    let output_path = unique_output_path();
    let _ = std::fs::remove_file(&output_path);
    // Ends the real, detached server this test's own fixture causes to
    // exist, on every path including a panic - declared before the
    // fixture client itself, so this guard's own `Drop` runs *after* the
    // fixture's (drop order is declaration order, reversed), with the
    // server still there, reachable by `lsof`, when this guard's own
    // drop runs.
    let _server_guard = ServerAtSocket(socket_path.clone());

    // Unique to this test run, never seeded by anything
    // `capture_login_environment` sets (`HOME`/`USER`/`LOGNAME`/`PATH`) -
    // the stand-in here for whatever a real client's own environment
    // would otherwise leak through the server it autostarts.
    let marker_value = format!("steg-test-leak-marker-{}", std::process::id());

    let mut client = KillOnDrop::new(
        Command::new(env_leak_client_binary())
            .env("STEG_SOCKET_PATH", &socket_path)
            .env("STEG_TEST_BINARY", steg_binary())
            .env("STEG_TEST_OUTPUT_PATH", &output_path)
            .env("STEG_TEST_LEAK_MARKER", &marker_value)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the env_leak_client fixture should spawn"),
    );

    // The fixture itself exits once its own `session.new` call succeeds -
    // waited on, bounded, so this test does not race ahead of the real
    // request actually having been accepted. `KillOnDrop` still applies
    // if it never does, on any path out of this function.
    let status = wait_bounded(&mut client.child, Duration::from_secs(15));
    client.done = true;
    assert!(
        status.success(),
        "the env_leak_client fixture should have exited 0 after a successful session.new"
    );

    wait_for_socket(&socket_path, Duration::from_secs(5));

    let text = wait_for_output(&output_path, Duration::from_secs(10));

    assert!(
        !text.contains(&marker_value),
        "a variable set only in the launching client's own environment must not reach the \
         pane: {text:?}"
    );
    assert!(
        text.contains("TERM=xterm-256color"),
        "AC #3 through the real binary: TERM must be set by the server: {text:?}"
    );
    // A `path_helper`-specific entry (verified present on this machine's
    // own real macOS login shell by running `capture_login_environment`'s
    // own mechanism directly, §1 of this task's plan) - present only when
    // a real capture happened, never when `fallback_env`'s plain
    // `/usr/bin:/bin:/usr/sbin:/sbin` was used instead. A bare
    // `contains("/usr/bin")` would pass on the fallback too, so this
    // checks something only the real profile produces.
    assert!(
        text.contains("/System/Cryptexes/App/usr/bin"),
        "the pane's own PATH must carry the real login shell's own path_helper entries, not \
         the fallback: {text:?}"
    );

    let _ = std::fs::remove_file(&output_path);
}

/// Waits for `child` to exit, bounded by polling `try_wait`.
fn wait_bounded(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait should not itself fail") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the fixture never exited within {timeout:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
