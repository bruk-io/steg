//! Resolving the person's login identity from the password database, and
//! capturing the environment a real login shell exports (doc-8 R99: "takes
//! its environment from a login shell and not from that client").
//!
//! Two separate, bounded steps, kept apart because they have different
//! costs: [`resolve_login_identity`] is a local passwd lookup (TASK-1.77's
//! own design, reused verbatim - no child process, so an abandoned worker
//! thread on timeout is harmless); [`capture_login_environment`] spawns a
//! real shell and is the one that can be genuinely slow (a hanging profile
//! script, a stalled directory service). Neither ever touches this
//! process's own environment table (`std::env::set_var` is unsafe and is
//! not used anywhere in this module); both hand back plain data.
//!
//! **Accepted, engineer-confirmed limit: a login shell run this way never
//! sources `.zshrc`.** `-l` (login) without `-i` (interactive) is
//! deliberate (a login shell's own profile, never an interactive prompt
//! that would need a real tty - see [`capture_login_environment`]'s own
//! doc) - but on zsh that also means only `.zprofile` is sourced, never
//! `.zshrc` (confirmed empirically, and proved here by
//! `capture_login_environment_sources_the_login_profile_not_the_
//! interactive_one`). A command pane's own program therefore never sees
//! anything a person set only in `.zshrc` - most macOS users' own `PATH`
//! edits, among other things. Accepted as a real gap, not worked around:
//! widening this to source `.zshrc` too would mean treating every pane's
//! own program as an interactive session, which it is not.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nix::unistd::User;

/// The resolved identity of the account the server is running as - never
/// the client's own `$SHELL`/`$HOME`/`$USER` (R99 forbids exactly that),
/// always the password database's own entry for this process's real
/// effective uid.
#[derive(Debug, Clone)]
pub struct LoginIdentity {
    /// The account's own login shell, e.g. `/bin/zsh`.
    pub shell: PathBuf,
    /// The account's own home directory.
    pub home: PathBuf,
    /// The account's own login name. `None` only when the password
    /// database has no entry for this process's own effective uid, or
    /// the lookup failed or timed out - a case this machine cannot
    /// reproduce for real (every real account has one), so the fallback
    /// path below is proven with a fabricated identity instead.
    pub user: Option<String>,
}

/// The literal fallback shell/home this process used before this task
/// (`LoginShell::from_process_env`'s own values) - kept as named constants
/// rather than re-invented, for the one case [`resolve_login_identity`]
/// itself cannot answer at all.
const DEFAULT_SHELL: &str = "/bin/zsh";
const DEFAULT_HOME: &str = "/";

/// The environment every pane falls back to when capturing the real login
/// shell's own environment fails for any reason - never derived from this
/// process's own inherited environment (R99's "not from that client",
/// read as "never derived from it at all," the stronger of two readings
/// AC #2's wording allows). Fixed and named so a test can assert on it
/// exactly, not merely "did not panic."
const FALLBACK_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

/// Resolves the login identity of the account this process is running as,
/// from the password database - never from `$SHELL`/`$HOME`/`$USER` in
/// this process's own (possibly client-inherited) environment, which is
/// exactly what R99 forbids. Bounded: on no passwd entry, a real `errno`,
/// or the timeout itself, falls back to [`DEFAULT_SHELL`]/[`DEFAULT_HOME`]
/// with `user: None`.
///
/// The worker thread is not joined on the timeout path - joining it would
/// reintroduce the exact unbounded hang this bound exists to prevent.
/// Harmless to abandon: no child process is ever spawned here, so the
/// thread holds nothing open but itself, which the OS reclaims once the
/// syscall it is blocked in eventually returns.
#[must_use]
pub fn resolve_login_identity(timeout: Duration) -> LoginIdentity {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = User::from_uid(nix::unistd::geteuid());
        // Dropped silently if the receiver already gave up (the timeout
        // path below) - there is no one left to tell.
        let _ = tx.send(result);
    });
    match rx.recv_timeout(timeout) {
        Ok(Ok(Some(user))) => LoginIdentity {
            shell: user.shell,
            home: user.dir,
            user: Some(user.name),
        },
        Ok(Ok(None)) | Ok(Err(_)) | Err(_) => LoginIdentity {
            shell: PathBuf::from(DEFAULT_SHELL),
            home: PathBuf::from(DEFAULT_HOME),
            user: None,
        },
    }
}

/// Why [`capture_login_environment`] fell back to [`fallback_env`],
/// carried out so a caller can report it (this module returns data, never
/// prints - `steg-server` does no I/O of its own kind here, matching this
/// crate's existing functional-core/imperative-shell boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackReason {
    /// The shell could not be spawned at all (missing, not executable).
    SpawnFailed,
    /// The shell ran and exited, but with a non-zero status.
    NonZeroExit,
    /// The shell did not finish within the bound.
    TimedOut,
}

/// What [`capture_login_environment`] actually produced.
#[derive(Debug, Clone)]
pub enum Capture {
    /// The login shell's own environment, exactly as it exported it.
    Captured(Vec<(OsString, OsString)>),
    /// Capturing failed; the carried [`Vec`] ([`fallback_env`]'s own
    /// result, already built from the same `identity` this call was
    /// given) is what every pane gets instead - carried here, not
    /// rebuilt by the caller, so a caller never has to reconstruct a
    /// [`LoginIdentity`] by hand just to ask for the fallback a second
    /// time (a round trip where a future field could silently be
    /// dropped).
    Fallback(FallbackReason, Vec<(OsString, OsString)>),
}

/// Spawns `identity.shell` as a login shell (`-l`, deliberately without
/// `-i`: a login shell's own profile, never an interactive prompt that
/// would need a real tty) with a cleared environment, re-seeded with
/// exactly four variables - `HOME`, `USER`, `LOGNAME` (from `identity`,
/// already resolved by [`resolve_login_identity`] - zero client
/// provenance), and a minimal `PATH` (profile scripts may assume one
/// exists, even though macOS's own `/etc/zprofile` rebuilds it from
/// scratch via `path_helper` regardless) - then reads its own exported
/// environment back.
///
/// Bounded: a reader thread owns the whole `Child`, reads its stdout to
/// completion *and* `wait()`s it, sending `(bytes, status)` back together,
/// so one `recv_timeout` on the caller's side covers both steps as a
/// single bound, never just the read (a child that backgrounds a process
/// of its own, e.g. a profile script that forks with `&`, could otherwise
/// let the read finish while the shell itself has not yet exited, leaving
/// an unbounded `wait()` after the bound already fired). On timeout, the
/// caller kills the whole *process group* (`kill -9 -<pid>`, the pid read
/// before the `Child` moved into the thread), via a plain `kill`
/// subprocess, the same technique this repository's own tests already use
/// (`crates/steg/tests/server_start.rs`), not a new `nix` feature.
///
/// **A backgrounded grandchild that inherits the stdout pipe is the
/// reason this kills the group, not just the shell's own pid.** A profile
/// that backgrounds a process of its own (`sleep 100 &`) and then exits
/// leaves that grandchild running, still holding the pipe's write end
/// open - found by testing exactly this, not assumed: the shell's own
/// exit alone does not make `read_to_end` see end-of-file, so the reader
/// thread (and this function's own bound) would otherwise depend on when
/// that unrelated process happens to end, which is never guaranteed.
/// `.process_group(0)` below puts the shell in its own, fresh process
/// group (the same technique `crates/steg/tests/server_start.rs`'s own
/// SIGHUP test already uses) so that killing *that* group reaches a
/// backgrounded grandchild too, as long as it never changes its own
/// group (the one remaining, accepted gap, stated here rather than
/// silently assumed covered: a profile that deliberately re-parents its
/// own background job into a different process group would still
/// outlive this bound). Once the group is killed, the grandchild's own
/// close of the pipe lets the abandoned reader thread's `read_to_end`
/// return and the thread exit on its own; not joined here, for the same
/// reason [`resolve_login_identity`]'s own worker thread is not joined
/// on its timeout path.
///
/// **The success path kills nothing, on purpose - stated plainly, not
/// left to be found by reading only the timeout path above.** A profile
/// that backgrounds a process which does *not* hold the stdout pipe open
/// (redirected elsewhere, or itself closed before backgrounding) lets
/// `read_to_end` see end-of-file as soon as the shell itself exits, well
/// within the bound - a genuine success, `Capture::Captured` returned
/// promptly - while that backgrounded process keeps running, entirely
/// unsupervised by this function, for as long as it likes. Accepted: a
/// real login shell's own profile is allowed to background a legitimate
/// job the person's session is meant to outlive (an agent, a daemon they
/// start on purpose) - this capture's job is reading the environment
/// back within a bound, not auditing or owning everything a login
/// shell's own start-up might spawn. Only the *timeout* path kills the
/// group, because only there is this function already giving up on the
/// shell itself; a clean exit earns no such sweep.
///
/// `stderr` is discarded (`Stdio::null()`): a profile's own warnings are
/// not this capture's concern, and keeping it unpiped avoids a full pipe
/// ever blocking the child while this function only ever reads stdout.
///
/// # Panics
///
/// Never in practice: `.stdout` is `Some` because `.stdout(Stdio::piped())`
/// is set on this same `Command` a few lines above, unconditionally, with
/// nothing in between that could clear it.
#[must_use]
pub fn capture_login_environment(identity: &LoginIdentity, timeout: Duration) -> Capture {
    use std::os::unix::process::CommandExt;

    let fallback = |reason: FallbackReason| Capture::Fallback(reason, fallback_env(identity));
    let marker = capture_marker();

    let mut command = Command::new(&identity.shell);
    command
        .arg("-l")
        .arg("-c")
        .arg(format!("echo {marker}; exec /usr/bin/env -0"))
        .env_clear()
        .env("HOME", &identity.home)
        .env("PATH", FALLBACK_PATH)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        // Its own, fresh process group - see this function's own doc for
        // why: killing it on timeout has to reach a backgrounded
        // grandchild too, not only this direct child's own pid.
        .process_group(0);
    if let Some(user) = &identity.user {
        command.env("USER", user).env("LOGNAME", user);
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return fallback(FallbackReason::SpawnFailed),
    };
    // `process_group(0)` makes this child its own group leader, so its
    // own pid *is* the group's pid - confirmed directly (`ps -o pid,pgid`
    // on a spawned child), not assumed from the API's own doc alone.
    let pgid = child.id();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let outcome = child
            .stdout
            .take()
            .expect("stdout was piped above, so this is always Some")
            .read_to_end(&mut buf)
            .and_then(|_| child.wait().map(|status| (buf, status)));
        let _ = tx.send(outcome);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok((bytes, status))) => {
            if !status.success() {
                return fallback(FallbackReason::NonZeroExit);
            }
            Capture::Captured(parse_env_dump(&bytes, &marker))
        }
        Ok(Err(_)) => fallback(FallbackReason::SpawnFailed),
        Err(_) => {
            // The `Child` itself is owned by the thread above (not this
            // stack), so killing it means killing by pid - the *group's*
            // pid, with the leading `-` `kill` reads as "a process
            // group, not a process" - bounded, since `kill`'s own
            // subprocess runs and exits near-instantly.
            let _ = Command::new("kill")
                .arg("-9")
                .arg(format!("-{pgid}"))
                .status();
            fallback(FallbackReason::TimedOut)
        }
    }
}

/// A marker unguessable by anything this capture did not itself
/// generate, combining this process's own pid, a monotonic counter and
/// the current time, none of which a login shell's own profile could
/// predict in advance. Generated once per call, never a fixed constant:
/// a fixed one would let a profile that happens to echo the exact same
/// literal text corrupt [`parse_env_dump`]'s own search for it (an
/// unlikely coincidence for a *fixed* string, but not one worth leaving
/// possible when a cheap, per-call value closes it outright).
fn capture_marker() -> String {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let counter = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!(
        "__steg_env_capture_{}_{counter}_{nanos}__",
        std::process::id()
    )
}

/// Parses `/usr/bin/env -0`'s own NUL-delimited output, discarding every
/// byte up to and including `marker`'s own trailing newline, the one
/// place a profile script's startup noise (always printed *before* the
/// marker, since login-profile sourcing finishes before the `-c`
/// string's own commands run) could otherwise corrupt the dump that
/// follows. `marker` is [`capture_marker`]'s own fresh, per-call value,
/// never a fixed string a profile could happen to echo itself and so
/// corrupt this search - the risk a fixed marker would carry, closed by
/// construction rather than by searching for the *last* match instead. A
/// line with no `=` is skipped (malformed, not a crash); a value
/// containing its own `=` is split on the first one only.
#[must_use]
fn parse_env_dump(bytes: &[u8], marker: &str) -> Vec<(OsString, OsString)> {
    let marker_line = format!("{marker}\n");
    let dump_start = bytes
        .windows(marker_line.len())
        .position(|window| window == marker_line.as_bytes())
        .map(|pos| pos + marker_line.len())
        .unwrap_or(0);

    bytes[dump_start..]
        .split(|&byte| byte == 0)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            use std::os::unix::ffi::OsStrExt;
            let pos = entry.iter().position(|&byte| byte == b'=')?;
            let key = OsStr::from_bytes(&entry[..pos]).to_os_string();
            let value = OsStr::from_bytes(&entry[pos + 1..]).to_os_string();
            Some((key, value))
        })
        .collect()
}

/// The environment every pane falls back to when [`capture_login_environment`]
/// could not capture a real one - a fixed, minimal `PATH`
/// ([`FALLBACK_PATH`]) plus whatever of `HOME`/`USER`/`LOGNAME` the
/// identity already resolved (never anything from this process's own
/// inherited environment, on purpose - see this module's own doc).
#[must_use]
pub fn fallback_env(identity: &LoginIdentity) -> Vec<(OsString, OsString)> {
    let mut env = vec![
        (OsString::from("PATH"), OsString::from(FALLBACK_PATH)),
        (
            OsString::from("HOME"),
            identity.home.clone().into_os_string(),
        ),
    ];
    if let Some(user) = &identity.user {
        env.push((OsString::from("USER"), OsString::from(user)));
        env.push((OsString::from("LOGNAME"), OsString::from(user)));
    }
    env
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{
        Capture, FallbackReason, LoginIdentity, capture_login_environment, fallback_env,
        parse_env_dump, resolve_login_identity,
    };

    fn find<'a>(env: &'a [(OsString, OsString)], key: &str) -> Option<&'a std::ffi::OsStr> {
        env.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_os_str())
    }

    /// A unique suffix for a throwaway path - `line!()` alone is not
    /// enough here, since both `ThrowawayHome::with_zprofile` and
    /// `FakeShell::script` call it from inside *their own* function body,
    /// not from each call site, so every call from the same test binary
    /// would otherwise expand to the exact same line number; combined
    /// with `cargo test`'s own default parallelism (many tests, one
    /// process, one pid), that collided two tests onto the same file path
    /// and had one clobber the other mid-run - caught by running this
    /// module's own tests repeatedly, not assumed safe from the pattern
    /// alone.
    fn unique_suffix() -> u64 {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    /// A throwaway home directory with its own `.zprofile` exporting a
    /// marker and a `PATH` addition - never this machine's own home, so
    /// this test never reads this machine's own real start-up files.
    /// Removed on drop, including when a panicking assertion unwinds past
    /// it.
    struct ThrowawayHome {
        path: PathBuf,
    }

    impl ThrowawayHome {
        fn with_zprofile(body: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "steg-login-env-test-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join(".zprofile"), body).unwrap();
            Self { path }
        }
    }

    impl Drop for ThrowawayHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// A fake "shell" standing in for a real login shell, the same role
    /// TASK-1.44's own (now-stale) draft plan proposed - a real,
    /// executable file, never a mock: it is run exactly as a real shell
    /// would be, with the same `-l -c <string>` argv this module's own
    /// `capture_login_environment` always passes.
    struct FakeShell {
        path: PathBuf,
    }

    impl FakeShell {
        fn script(body: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "steg-fake-shell-{}-{}",
                std::process::id(),
                unique_suffix()
            ));
            let mut file = fs::File::create(&path).unwrap();
            writeln!(file, "#!/bin/sh").unwrap();
            writeln!(file, "{body}").unwrap();
            drop(file);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self { path }
        }
    }

    impl Drop for FakeShell {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    /// A fabricated identity for a test to hand `capture_login_environment`
    /// directly - every test in this module always gives the same `user`,
    /// so this is the one place that literal lives.
    fn test_identity(shell: &std::path::Path, home: &std::path::Path) -> LoginIdentity {
        LoginIdentity {
            shell: shell.to_path_buf(),
            home: home.to_path_buf(),
            user: Some("steg-test-user".to_string()),
        }
    }

    /// Cheap and side-effect-free (a local passwd lookup, no child
    /// process, no file written) - not a claim about this machine's own
    /// account beyond what every real account's passwd entry already
    /// guarantees: the fallback branch is proven separately, by
    /// fabrication, below. Checks the returned shell/home/user actually
    /// *exist*, not merely that they are non-empty strings.
    #[test]
    fn resolve_login_identity_never_panics_and_returns_a_home_a_shell_and_a_user_that_exist() {
        let identity = resolve_login_identity(Duration::from_secs(5));
        assert!(
            identity.shell.is_file(),
            "the resolved shell should be a real, existing file: {identity:?}"
        );
        assert!(
            identity.home.is_dir(),
            "the resolved home should be a real, existing directory: {identity:?}"
        );
        assert!(
            identity.user.is_some_and(|user| !user.is_empty()),
            "the resolved user should be a real, non-empty login name"
        );
    }

    #[test]
    fn capture_login_environment_sources_the_login_profile_not_the_interactive_one() {
        let home = ThrowawayHome::with_zprofile("export MARK_ZPROFILE=1\n");
        fs::write(home.path.join(".zshrc"), "export MARK_ZSHRC=1\n").unwrap();

        let capture = capture_login_environment(
            &test_identity(std::path::Path::new("/bin/zsh"), &home.path),
            Duration::from_secs(20),
        );

        let Capture::Captured(env) = capture else {
            panic!("expected a real capture, got {capture:?}");
        };
        assert_eq!(find(&env, "MARK_ZPROFILE"), Some(std::ffi::OsStr::new("1")));
        assert_eq!(
            find(&env, "MARK_ZSHRC"),
            None,
            "a login, non-interactive shell (-l without -i) must not source .zshrc"
        );
    }

    #[test]
    fn capture_login_environment_never_leaks_a_variable_set_only_in_the_callers_own_environment() {
        // `CARGO_PKG_NAME` is set by `cargo test` itself in *this test
        // process's own* ambient environment (confirmed by running a
        // probe test printing `std::env::vars()`, not assumed from
        // memory) - never seeded by `capture_login_environment`'s own
        // four variables, and never mutated here, so this needs no
        // `std::env::set_var` (unsafe, and process-global, which would
        // race every other test in this binary under `cargo test`'s own
        // default parallelism - the task's own description forbids
        // `set_var` for exactly this kind of reason).
        let home = ThrowawayHome::with_zprofile("");

        let capture = capture_login_environment(
            &test_identity(std::path::Path::new("/bin/zsh"), &home.path),
            Duration::from_secs(20),
        );

        let Capture::Captured(env) = capture else {
            panic!("expected a real capture, got {capture:?}");
        };
        assert_eq!(
            find(&env, "CARGO_PKG_NAME"),
            None,
            "a variable set only in the calling process's own environment must not reach \
             the captured login shell's environment"
        );
    }

    #[test]
    fn capture_login_environment_falls_back_on_a_shell_that_never_finishes_in_time() {
        let fake = FakeShell::script("sleep 100");

        let capture = capture_login_environment(
            &test_identity(&fake.path, std::path::Path::new("/tmp")),
            Duration::from_millis(200),
        );

        assert!(
            matches!(capture, Capture::Fallback(FallbackReason::TimedOut, _)),
            "expected a timeout fallback, got {capture:?}"
        );
    }

    #[test]
    fn capture_login_environment_falls_back_on_a_shell_that_exits_non_zero() {
        let fake = FakeShell::script("exit 7");

        let capture = capture_login_environment(
            &test_identity(&fake.path, std::path::Path::new("/tmp")),
            Duration::from_secs(20),
        );

        assert!(
            matches!(capture, Capture::Fallback(FallbackReason::NonZeroExit, _)),
            "expected a non-zero-exit fallback, got {capture:?}"
        );
    }

    #[test]
    fn capture_login_environment_falls_back_on_a_shell_that_does_not_exist() {
        let identity = test_identity(
            std::path::Path::new("/nonexistent/steg-test-shell"),
            std::path::Path::new("/tmp"),
        );
        let capture = capture_login_environment(&identity, Duration::from_secs(5));

        let Capture::Fallback(reason, env) = capture else {
            panic!("expected a spawn-failed fallback, got {capture:?}");
        };
        assert_eq!(reason, FallbackReason::SpawnFailed);
        // The carried environment is `fallback_env(&identity)` exactly -
        // this is the whole point of `Capture::Fallback` carrying it
        // itself rather than a caller reconstructing `identity` by hand
        // to ask for it a second time.
        assert_eq!(env, fallback_env(&identity));
    }

    #[test]
    fn capture_login_environment_prints_noise_before_the_marker_without_corrupting_the_dump() {
        // A profile that echoes a banner to stdout, *then* actually runs
        // the real `-c` command `capture_login_environment` passed it -
        // `$3` is that string (`-l`, `-c` are `$1`/`$2`; see this file's
        // own shebang-script reasoning elsewhere), carrying this run's
        // own fresh marker, which this test cannot predict in advance
        // and so cannot hardcode - proving the marker, not luck, is what
        // keeps the dump parseable, against the real mechanism rather
        // than a stand-in for it.
        let fake = FakeShell::script("echo 'Welcome to a noisy profile!'\neval \"$3\"");

        let capture = capture_login_environment(
            &test_identity(&fake.path, std::path::Path::new("/tmp")),
            Duration::from_secs(20),
        );

        let Capture::Captured(env) = capture else {
            panic!("expected a real capture, got {capture:?}");
        };
        // The fake shell's own `exec /usr/bin/env -0` still ran and
        // produced a well-formed dump - every entry parsed has a `=`,
        // none of them is the banner line itself.
        assert!(
            env.iter()
                .all(|(key, _)| !key.to_string_lossy().contains("Welcome")),
            "the banner printed before the marker must never appear as a parsed key: {env:?}"
        );
        assert!(
            !env.is_empty(),
            "the real dump after the marker must still parse: {env:?}"
        );
    }

    /// A review finding, reproduced directly: a profile that backgrounds
    /// a process of its own and then exits leaves that grandchild
    /// running, still holding the pipe's write end open - without
    /// killing the whole *group* on timeout, the reader thread (and, in
    /// a long-running server, the grandchild itself) would be left
    /// unbounded. Proves both halves: this function still returns within
    /// its own stated bound, *and* the grandchild is actually gone
    /// shortly after, not merely unreported.
    #[test]
    fn capture_login_environment_kills_a_backgrounded_grandchild_that_held_the_pipe_open() {
        let grandchild_pid_file = std::env::temp_dir().join(format!(
            "steg-grandchild-pid-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let _ = fs::remove_file(&grandchild_pid_file);
        let fake = FakeShell::script(&format!(
            "sleep 100 & echo $! > {}; exit 0",
            grandchild_pid_file.display()
        ));

        // 5s, not 1s (TASK-1.143): a full second was not enough margin for
        // the shell to actually finish forking `sleep 100 &` *and* writing
        // its pid to a file before this capture's own functional timeout
        // fired - found to matter under real load, not assumed: this
        // exact test flaked organically, twice in a row, in this task's
        // own recorded `just check` baseline, always the same way (this
        // assertion's own fallback check still passed - the kill always
        // fires - but the pid file below never appeared, because the
        // kill landed before the shell had written it). 5s gives fork/exec
        // real margin under contention while staying an order of
        // magnitude below the 15s/30s liveness bounds below, which this
        // task does not touch - those were never the problem; see
        // `capture_login_environment_tolerates_a_delayed_pid_file_write_
        // from_a_backgrounded_grandchild` below for the deterministic,
        // forced control proving this specific margin, rather than relying
        // on load to reproduce it again.
        // 15s (TASK-1.123): this only has to prove
        // `capture_login_environment` returns *at all*, not race a tight
        // deadline against this machine's own, unrelated CPU contention -
        // the same "liveness bound, not a timing-precision one" reasoning
        // `attach.rs`'s own bounds already settled on for the identical
        // reason.
        let started = std::time::Instant::now();
        let capture = capture_login_environment(
            &test_identity(&fake.path, std::path::Path::new("/tmp")),
            Duration::from_secs(5),
        );
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "capture_login_environment must still return within a bound close to its own \
             timeout, not hang on the grandchild: took {:?}",
            started.elapsed()
        );
        assert!(
            matches!(capture, Capture::Fallback(FallbackReason::TimedOut, _)),
            "expected a timeout fallback, got {capture:?}"
        );

        // 30s from the same `started`, not 10s (TASK-1.123, the actual
        // sighting this bound was widened for - found while building
        // this task's own class (b) fix, not synthesised: this exact
        // assertion failed once, organically, under this machine's own
        // heavy concurrent test load, recorded on the task). This is a
        // cross-process liveness poll with no in-process event to
        // subscribe to instead - the fake shell's own fork-then-echo-
        // then-write sequence is scheduled by the OS, not signalled to
        // this test - so the fix here is a wider margin, not a
        // different wait mechanism.
        let grandchild_pid = loop {
            if let Ok(text) = fs::read_to_string(&grandchild_pid_file)
                && let Ok(pid) = text.trim().parse::<u32>()
            {
                break pid;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the backgrounded grandchild's own pid file never appeared"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let alive = std::process::Command::new("kill")
                .arg("-0")
                .arg(grandchild_pid.to_string())
                .status()
                .expect("kill -0 should run")
                .success();
            if !alive {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the backgrounded grandchild (pid {grandchild_pid}) was never killed"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        let _ = fs::remove_file(&grandchild_pid_file);
    }

    /// TASK-1.143's own deterministic control for the margin raised above:
    /// forces the exact race a loaded machine triggers only sometimes
    /// (the shell's own fork-then-write delayed past this capture's
    /// functional timeout), rather than waiting for load to reproduce it
    /// again. The `sleep 2` between the fork and the pid-file write is a
    /// fixture's own deliberate delay standing in for that fork/exec
    /// scheduling delay - the one sleep in this module's tests that is
    /// not a poll for an event, but the independent variable this control
    /// is built to force.
    ///
    /// At this test's own committed timeout (5s), the 2s delay finishes
    /// comfortably before the kill fires, so this passes at the head this
    /// test is committed at. Built, and once, temporarily, read back with
    /// this test's own timeout argument lowered to the old `1s` bound
    /// (reverted immediately after, never committed that way) to force and
    /// record the identical failure the test above flaked on organically -
    /// same panic, same message, same ~30s shape (the kill lands before
    /// the write, so the pid-file poll below exhausts its own 30s bound) -
    /// proving the margin raised above is both necessary (fails at 1s) and
    /// sufficient (passes at 5s) by construction, not by chance.
    #[test]
    fn capture_login_environment_tolerates_a_delayed_pid_file_write_from_a_backgrounded_grandchild()
     {
        let grandchild_pid_file = std::env::temp_dir().join(format!(
            "steg-grandchild-pid-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        let _ = fs::remove_file(&grandchild_pid_file);
        let fake = FakeShell::script(&format!(
            "sleep 100 & sleep 2; echo $! > {}; exit 0",
            grandchild_pid_file.display()
        ));

        let started = std::time::Instant::now();
        let capture = capture_login_environment(
            &test_identity(&fake.path, std::path::Path::new("/tmp")),
            Duration::from_secs(5),
        );
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "capture_login_environment must still return within a bound close to its own \
             timeout, not hang on the grandchild: took {:?}",
            started.elapsed()
        );
        assert!(
            matches!(capture, Capture::Fallback(FallbackReason::TimedOut, _)),
            "expected a timeout fallback, got {capture:?}"
        );

        let grandchild_pid = loop {
            if let Ok(text) = fs::read_to_string(&grandchild_pid_file)
                && let Ok(pid) = text.trim().parse::<u32>()
            {
                break pid;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the backgrounded grandchild's own pid file never appeared"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            let alive = std::process::Command::new("kill")
                .arg("-0")
                .arg(grandchild_pid.to_string())
                .status()
                .expect("kill -0 should run")
                .success();
            if !alive {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the backgrounded grandchild (pid {grandchild_pid}) was never killed"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        let _ = fs::remove_file(&grandchild_pid_file);
    }

    #[test]
    fn fallback_env_is_exactly_the_named_constant_set() {
        let identity = LoginIdentity {
            shell: PathBuf::from("/bin/zsh"),
            home: PathBuf::from("/Users/steg-test"),
            user: Some("steg-test".to_string()),
        };

        let env = fallback_env(&identity);

        assert_eq!(
            find(&env, "PATH"),
            Some(std::ffi::OsStr::new("/usr/bin:/bin:/usr/sbin:/sbin"))
        );
        assert_eq!(
            find(&env, "HOME"),
            Some(std::ffi::OsStr::new("/Users/steg-test"))
        );
        assert_eq!(find(&env, "USER"), Some(std::ffi::OsStr::new("steg-test")));
        assert_eq!(
            find(&env, "LOGNAME"),
            Some(std::ffi::OsStr::new("steg-test"))
        );
        assert_eq!(
            env.len(),
            4,
            "exactly these four, nothing from the real environment: {env:?}"
        );
    }

    #[test]
    fn fallback_env_omits_user_and_logname_when_the_identity_itself_has_none() {
        let identity = LoginIdentity {
            shell: PathBuf::from("/bin/zsh"),
            home: PathBuf::from("/"),
            user: None,
        };

        let env = fallback_env(&identity);

        assert_eq!(find(&env, "USER"), None);
        assert_eq!(find(&env, "LOGNAME"), None);
        assert_eq!(env.len(), 2);
    }

    /// `parse_env_dump` itself takes the marker as a plain parameter
    /// (never a global constant, since the real one is fresh per call,
    /// `capture_marker`'s own doc) - this test's own marker is any fixed
    /// string, since this function's own logic does not care what the
    /// marker's value is, only that it matches.
    const TEST_MARKER: &str = "__test_marker__";

    #[test]
    fn parse_env_dump_skips_a_malformed_entry_with_no_equals_sign() {
        let mut bytes = format!("{TEST_MARKER}\n").into_bytes();
        bytes.extend_from_slice(b"GOOD=value\0malformed\0ALSO_GOOD=ok\0");

        let env = parse_env_dump(&bytes, TEST_MARKER);

        assert_eq!(find(&env, "GOOD"), Some(std::ffi::OsStr::new("value")));
        assert_eq!(find(&env, "ALSO_GOOD"), Some(std::ffi::OsStr::new("ok")));
        assert_eq!(
            env.len(),
            2,
            "the malformed entry must be skipped, not crash: {env:?}"
        );
    }

    #[test]
    fn parse_env_dump_splits_a_value_containing_its_own_equals_sign_on_the_first_one_only() {
        let mut bytes = format!("{TEST_MARKER}\n").into_bytes();
        bytes.extend_from_slice(b"KEY=a=b=c\0");

        let env = parse_env_dump(&bytes, TEST_MARKER);

        assert_eq!(find(&env, "KEY"), Some(std::ffi::OsStr::new("a=b=c")));
    }
}
