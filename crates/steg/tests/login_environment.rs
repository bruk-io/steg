//! AC #1 (doc-8 R99): a pane's program starts with a login shell's own
//! environment, not the client's - proved end to end, through the real
//! server code, a real on-demand server, and a real client
//! (`tests/fixtures/env_leak_client.rs`), with a *fabricated* login
//! identity (`tests/fixtures/fixture_server.rs`) rather than this
//! machine's real one.
//!
//! **Review finding, resolved: this test's own identity is never resolved
//! from the real password database.** An earlier version of this test let
//! the real `steg` binary resolve its own login identity, which meant the
//! server it autostarted ran *this machine's* real login shell against
//! *this machine's* real `~/.zprofile`/`/etc/zprofile` - a result that
//! depends on the speed and content of whoever happens to run the test
//! (a slow `.zprofile` step could push the capture past its bound and
//! silently flip to the fallback, failing an assertion about a real
//! system path for reasons unrelated to any regression), and executes a
//! real person's start-up files as a side effect of running tests, which
//! this team's own convention forbids.
//!
//! The fix follows a convention this crate already has, rather than
//! inventing a new one: `crates/steg/Cargo.toml` already ships three
//! `fixtures`-feature-gated `[[bin]]`s compiled out of the shipped
//! binary entirely. `fixture_server` is a fourth: the real server code
//! (`attempt`, `detach`, `build_login_shell`, `run` - everything
//! `crates/steg/src/main.rs`'s own `run_server_start(true)` calls), given
//! a `LoginIdentity` from its own environment instead of
//! `resolve_login_identity`'s real password-database lookup. Two
//! alternatives were considered first and rejected (mutating a real
//! account's own `~/.zprofile`; threading `ZDOTDIR` through to
//! production code - both for reasons that still apply and are kept
//! here for the record): mutating a real account's own dotfiles risks
//! corrupting them if the test crashes mid-write, and a client-settable
//! `ZDOTDIR` reaching production code would be a new way for a client to
//! redirect a login shell's own profile-sourcing step, exactly the kind
//! of thing R99 exists to prevent. Neither is needed once the identity
//! itself can simply be given to a fixture-only entry point instead.
//!
//! What this test proves, now against a throwaway home with its own
//! `.zprofile` and nothing about this machine: **the leak-absence half
//! of AC #1** (a variable set only in the launching client's own
//! environment is absent from the pane), **that a real capture actually
//! happened, not merely the fallback** (the throwaway `.zprofile`'s own
//! marker and `PATH` addition are both present - specific to this test's
//! own profile, never `fallback_env`'s fixed four variables), and **AC
//! #3 through the real binary** (`TERM=xterm-256color`, whatever the
//! login shell's own profile exports). The one thing this test does not
//! prove - that `-l` without `-i` means only `.zprofile` is sourced,
//! never `.zshrc` - is `crates/server/src/login_env.rs`'s own
//! `capture_login_environment_sources_the_login_profile_not_the_
//! interactive_one`, which already uses a fabricated shell/home pair at
//! the function level; keeping one small test of real identity
//! resolution alone (no fixture, no side effect - it reads the password
//! database and returns a home/shell/user that exist) is
//! `login_env.rs`'s own `resolve_login_identity_never_panics_and_
//! always_names_a_real_shell_and_home`, strengthened by this same
//! review round to check the returned paths are real.

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

fn fixture_server_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fixture_server"))
}

fn env_leak_client_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_env_leak_client"))
}

/// A throwaway home directory with its own `.zprofile` exporting a
/// marker and a `PATH` addition - never this machine's own home, so this
/// test never reads this machine's own real start-up files.
/// `crates/server/src/login_env.rs`'s own `ThrowawayHome`, duplicated
/// here rather than shared across crates.
struct ThrowawayHome {
    path: PathBuf,
}

impl ThrowawayHome {
    fn with_zprofile(body: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "steg-login-env-e2e-home-{}-{suffix}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("creating the throwaway home should succeed");
        std::fs::write(path.join(".zprofile"), body).expect("writing .zprofile should succeed");
        Self { path }
    }
}

impl Drop for ThrowawayHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
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
    // A throwaway home with its own `.zprofile` - unique to this test
    // run, read by nothing but the fixture server this test itself
    // causes to exist. The marker it exports and the `PATH` it adds are
    // both specific to *this* profile, never anything this machine's
    // own real account might also happen to have.
    let marker_in_zprofile = format!("steg-login-env-e2e-zprofile-marker-{}", std::process::id());
    let home = ThrowawayHome::with_zprofile(&format!(
        "export MARK_ZPROFILE={marker_in_zprofile}\nexport PATH=\"$HOME/custom-bin:$PATH\"\n"
    ));

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
            .env("STEG_TEST_BINARY", fixture_server_binary())
            .env("STEG_TEST_OUTPUT_PATH", &output_path)
            .env("STEG_TEST_LEAK_MARKER", &marker_value)
            .env("STEG_TEST_LOGIN_SHELL", "/bin/zsh")
            .env("STEG_TEST_LOGIN_HOME", &home.path)
            .env("STEG_TEST_LOGIN_USER", "steg-test-user")
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
    // A real capture happened, not merely the fallback: the throwaway
    // profile's own marker and `PATH` addition are both present - never
    // `fallback_env`'s fixed four variables, and nothing about this
    // machine's own real account.
    assert!(
        text.contains(&format!("MARK_ZPROFILE={marker_in_zprofile}")),
        "the throwaway .zprofile's own marker must reach the pane: {text:?}"
    );
    assert!(
        text.contains(&format!("{}/custom-bin", home.path.display())),
        "the throwaway .zprofile's own PATH addition must reach the pane: {text:?}"
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
