//! A real pseudo-terminal running a real short program (`.claude/rules/
//! testing-rust.md`: "a real socket... is not a double") - the few
//! things only a real process can prove: a real working directory and
//! environment actually reaching it, and a real exit code read back from
//! a real `wait()`. Orchestration (restart, resize, the no-program state)
//! is proven fast and without doubles in `program.rs`.
//!
//! No test here sleeps to wait for anything: every wait is a `recv` -
//! bounded, never unbounded, so a real program that never ends fails the
//! test with a message instead of hanging the job - on the event channel
//! the real program's own end (or the real pty reaching end-of-file)
//! triggers.

use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use steg_kernel::{ExitOutcome, Moment, Tree};
use steg_sdk::{PaneId, Program, Screen};

use steg_server::{Error, LaunchSpec, PaneEvent, ProgramLauncher, RealLauncher};

/// How long any one real-pty test waits for an event before failing with
/// a message, rather than hanging indefinitely on a misbehaving child
/// (generous: these are real, short programs on a real pty, which should
/// each take well under a second).
const EVENT_TIMEOUT: Duration = Duration::from_secs(20);

/// Blocks for one [`PaneEvent`], bounded by [`EVENT_TIMEOUT`].
fn recv_event(events_rx: &mpsc::Receiver<PaneEvent>) -> PaneEvent {
    events_rx
        .recv_timeout(EVENT_TIMEOUT)
        .unwrap_or_else(|_| panic!("no PaneEvent within {EVENT_TIMEOUT:?}"))
}

fn screen_text(screen: &Screen) -> String {
    screen
        .rows
        .iter()
        .map(|row| row.iter().map(|run| run.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn spec(
    argv: Vec<&str>,
    cwd: &str,
    env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> LaunchSpec {
    sized_spec(argv, cwd, env, (80, 24))
}

fn sized_spec(
    argv: Vec<&str>,
    cwd: &str,
    env: Vec<(std::ffi::OsString, std::ffi::OsString)>,
    size: (u16, u16),
) -> LaunchSpec {
    LaunchSpec {
        argv: argv.into_iter().map(str::to_string).collect(),
        cwd: cwd.to_string(),
        env,
        size,
    }
}

#[test]
fn a_real_program_runs_in_the_given_working_directory() {
    let cwd = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(
        spec(vec!["sh", "-c", "pwd"], cwd.to_str().unwrap(), Vec::new()),
        pane,
        Arc::new(RealLauncher),
        events_tx,
    )
    .unwrap();

    assert_eq!(
        recv_event(&events_rx),
        PaneEvent::Exited { pane, exit_code: 0 }
    );

    let screen = handle.snapshot().unwrap();
    let reported = std::fs::canonicalize(screen_text(&screen).trim()).unwrap();
    assert_eq!(reported, cwd);
    handle.shutdown();
}

#[test]
fn a_real_programs_environment_carries_the_five_variables_and_path() {
    let cwd = std::env::temp_dir();
    let env = steg_server::pane_environment(
        "/tmp/steg-test.sock",
        steg_sdk::SessionId::from_raw(7),
        steg_sdk::TabId::from_raw(8),
        PaneId::from_raw(9),
        std::path::Path::new("/usr/local/steg/bin"),
        std::env::var_os("PATH").as_deref(),
    );
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(9);

    let handle = steg_server::start(
        // Tall and wide enough that a real process's whole real
        // environment - which can run to a hundred or more variables on a
        // developer's own machine - fits the viewport with nothing
        // scrolled out of it; `screen_text` only reads the current
        // screen, never scrollback.
        sized_spec(
            vec!["sh", "-c", "env"],
            cwd.to_str().unwrap(),
            env,
            (200, 4000),
        ),
        pane,
        Arc::new(RealLauncher),
        events_tx,
    )
    .unwrap();

    recv_event(&events_rx);

    let screen = handle.snapshot().unwrap();
    let text = screen_text(&screen);
    assert!(text.contains("STEG_SERVER=/tmp/steg-test.sock"), "{text}");
    assert!(
        text.contains(&format!(
            "STEG_SESSION={}",
            steg_sdk::SessionId::from_raw(7)
        )),
        "{text}"
    );
    assert!(
        text.contains(&format!("STEG_TAB={}", steg_sdk::TabId::from_raw(8))),
        "{text}"
    );
    assert!(
        text.contains(&format!("STEG_PANE={}", PaneId::from_raw(9))),
        "{text}"
    );
    assert!(text.contains("STEG_BIN_DIR=/usr/local/steg/bin"), "{text}");
    assert!(text.contains("PATH=/usr/local/steg/bin:"), "{text}");
    handle.shutdown();
}

#[test]
fn a_command_panes_real_exit_code_is_known_it_stays_and_can_run_again() {
    let cwd = std::env::temp_dir();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);
    let command_spec = spec(
        vec!["sh", "-c", "printf hi; exit 3"],
        cwd.to_str().unwrap(),
        Vec::new(),
    );

    let handle = steg_server::start(
        command_spec.clone(),
        pane,
        Arc::new(RealLauncher),
        events_tx,
    )
    .unwrap();

    assert_eq!(
        recv_event(&events_rx),
        PaneEvent::Exited { pane, exit_code: 3 }
    );
    let screen = handle.snapshot().unwrap();
    assert!(
        screen_text(&screen).contains("hi"),
        "{}",
        screen_text(&screen)
    );

    // The kernel's own state (no doubles): a command pane's exit stays,
    // and the real exit code the server just reported is exactly what
    // the kernel records.
    let mut tree = Tree::new();
    let session_id = tree.new_session(None).unwrap();
    let kernel_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
    tree.start_program(
        kernel_pane,
        Program::Command {
            argv: vec![
                "sh".to_string(),
                "-c".to_string(),
                "printf hi; exit 3".to_string(),
            ],
            cwd: cwd.to_str().unwrap().to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        tree.record_exit(kernel_pane, 3, Moment::new(Duration::ZERO))
            .unwrap(),
        ExitOutcome::Stayed
    );

    // Restarting the server's own pane runs the same command again.
    handle.restart(command_spec).unwrap();
    assert_eq!(
        recv_event(&events_rx),
        PaneEvent::Exited { pane, exit_code: 3 }
    );
    handle.shutdown();
}

#[test]
fn a_shell_panes_real_exit_reaches_the_kernels_cascade() {
    let cwd = std::env::temp_dir();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(
        spec(vec!["sh"], cwd.to_str().unwrap(), Vec::new()),
        pane,
        Arc::new(RealLauncher),
        events_tx,
    )
    .unwrap();

    handle.send_input(b"exit 7\n".to_vec()).unwrap();

    assert_eq!(
        recv_event(&events_rx),
        PaneEvent::Exited { pane, exit_code: 7 }
    );

    // The kernel's own cascade (no doubles): a shell pane's real exit
    // code closes it, the same cascade a manual close runs.
    let mut tree = Tree::new();
    let session_id = tree.new_session(None).unwrap();
    let kernel_pane = tree.session(session_id).unwrap().tabs()[0].panes()[0].id();
    tree.start_program(
        kernel_pane,
        Program::Shell {
            cwd: cwd.to_str().unwrap().to_string(),
        },
    )
    .unwrap();

    let outcome = tree
        .record_exit(kernel_pane, 7, Moment::new(Duration::ZERO))
        .unwrap();
    let ExitOutcome::Closed(closed) = outcome else {
        panic!("expected the shell pane's real exit to close it, got {outcome:?}");
    };
    assert_eq!(closed.pane, kernel_pane);
    assert_eq!(closed.session, Some(session_id));

    handle.shutdown();
}

/// `AC #1`'s "a program that cannot be started" is engaged by a bad
/// command (the next test), never by a bad `cwd` - confirmed by the
/// experiment this is the committed record of, not assumed. Read
/// `portable-pty` 0.9.0's own source
/// (`src/cmdbuilder.rs:498-507,560-576`): `CommandBuilder::as_command`
/// silently substitutes the user's home directory for any `cwd` that
/// isn't a real, existing directory, and spawns there - there is no
/// error, no child-side `chdir` failure, nothing for this task's
/// `ENOENT`-shaped rollback (§2 of the plan) to catch, because the
/// program genuinely does start, successfully, just not where it was
/// asked to. `get_home_dir()` reads the env var `HOME` from the
/// `CommandBuilder`'s own map, which is seeded from this process's real
/// environment at construction (`get_base_env()`) and never overridden by
/// anything `pane_environment` sets (the five `STEG_*` vars and `PATH`
/// only) - the same source `std::env::var("HOME")` reads, so asserting
/// equality with it is a real claim, not a guess at portable-pty's
/// internals.
///
/// **Answered (review finding 4, TASK-1.76)**: refused, not silently
/// redirected - but not here, and not by `portable-pty`, since this
/// experiment is exactly why that is the wrong layer to fix it at (no
/// error reaches this call at all). `Runtime::start_and_dispatch`
/// (`crates/server/src/runtime.rs`) checks `program.cwd()` itself,
/// before calling down to this function, and refuses with
/// `ErrorCode::ProgramFailed` rather than ever reaching this silent
/// substitution; see `crates/server/tests/runtime.rs`'s
/// `*_with_a_nonexistent_cwd_*` tests. This test stays exactly as it
/// was: the committed record of what this lower layer does on its own,
/// which is still true and still the reason the check exists above it.
#[test]
fn a_nonexistent_cwd_is_not_a_failure_to_start_portable_pty_silently_uses_home() {
    let home = std::fs::canonicalize(std::env::var("HOME").unwrap()).unwrap();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(
        spec(
            vec!["sh", "-c", "pwd"],
            "/this/directory/does/not/exist/anywhere/at/all",
            Vec::new(),
        ),
        pane,
        Arc::new(RealLauncher),
        events_tx,
    )
    .expect("a bad cwd alone does not fail start() - portable-pty substitutes home and spawns");

    assert_eq!(
        recv_event(&events_rx),
        PaneEvent::Exited { pane, exit_code: 0 }
    );
    let screen = handle.snapshot().unwrap();
    let reported = std::fs::canonicalize(screen_text(&screen).trim()).unwrap();
    assert_eq!(reported, home, "expected the program to have run in $HOME");
    handle.shutdown();
}

#[test]
fn a_command_that_does_not_exist_fails_synchronously_with_no_thread_to_join() {
    let (events_tx, _events_rx) = mpsc::channel();
    let cwd = std::env::temp_dir();

    let result = steg_server::start(
        spec(
            vec!["steg-this-program-does-not-exist-anywhere-on-path"],
            cwd.to_str().unwrap(),
            Vec::new(),
        ),
        PaneId::from_raw(1),
        Arc::new(RealLauncher),
        events_tx,
    );

    assert!(
        matches!(result, Err(Error::Pty(_))),
        "expected Err(Error::Pty(_)) synchronously from start(), got {}",
        match result {
            Ok(_) => "Ok(_)".to_string(),
            Err(e) => e.to_string(),
        }
    );
}

/// How many threads call `RealLauncher::spawn` at once, each round -
/// "at least 28" per the review: this machine's own default `cargo test`
/// parallelism, the exact concurrency that first exposed the race.
const CONCURRENT_OPENS: usize = 32;

/// How many times [`many_threads_opening_a_real_pty_at_once_never_fails`]
/// repeats the all-at-once open, inside one test run - enough that the
/// negative control (the lock removed) fails on a near-certain fraction
/// of local runs, recorded on the task alongside how many rounds that
/// took.
const ROUNDS: usize = 10;

/// TASK-1.76 review (second round): `openpty()` itself is not safe to
/// call from several threads of one process at once on this platform -
/// measured directly, isolated from this crate's own code (a bare libc
/// call from many threads, no Rust, no `portable-pty`, still raced) and
/// recorded on the task. `RealLauncher::spawn` now serialises its own
/// `openpty` call behind `pty::OPENPTY_LOCK` for exactly this reason;
/// this test drives that real contention directly - many real threads,
/// each opening a real pty through the real launcher, at once, repeated
/// enough rounds that the race is no longer a matter of luck.
///
/// Every spawned program exits at once and is waited on before the next
/// round, rather than left running: proving the open itself is safe
/// under contention does not need every pty kept alive at once, and
/// reaping each keeps 32 * 10 rounds of this test from leaking real
/// processes.
#[test]
fn many_threads_opening_a_real_pty_at_once_never_fails() {
    let cwd = std::env::temp_dir();

    for round in 0..ROUNDS {
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(CONCURRENT_OPENS));
        let handles: Vec<_> = (0..CONCURRENT_OPENS)
            .map(|thread| {
                let barrier = std::sync::Arc::clone(&barrier);
                let cwd = cwd.clone();
                std::thread::spawn(move || {
                    let launcher = RealLauncher;
                    let spec = spec(vec!["sh", "-c", "true"], cwd.to_str().unwrap(), Vec::new());
                    // Every thread opens at the same instant, not
                    // whenever it happens to get scheduled: the race
                    // this is proving against is concurrent calls, not
                    // merely overlapping ones.
                    barrier.wait();
                    (thread, launcher.spawn(&spec))
                })
            })
            .collect();

        for handle in handles {
            let (thread, result) = handle.join().expect("the spawning thread itself panicked");
            match result {
                Ok(mut spawned) => {
                    let _ = spawned.child.wait();
                }
                Err(error) => panic!(
                    "round {round}/{ROUNDS}, thread {thread}/{CONCURRENT_OPENS}: \
                     RealLauncher::spawn failed under concurrent opens: {error}"
                ),
            }
        }
    }
}
