//! Start and stop and idle (TASK-1.131 seam: "start and stop and idle" -
//! `Runtime`'s own construction/teardown and connection accounting, not
//! starting a pane's program).

use std::sync::Arc;
use std::sync::mpsc::Sender;

use steg_sdk::{ClientId, ClientKind};

use super::close::force_end;
use super::{FORCE_JOIN_TIMEOUT, Runtime, apply_size_changed_locked};
use crate::connection::ConnectionEvent;

impl Runtime {
    /// Mints a real, tree-tracked client (TASK-1.45 §1): `connection::
    /// handle` calls this once, right after a successful handshake, before
    /// the `ServerHello` is built - replacing `connection.rs`'s own
    /// previous `next_local_client_id()`, a local counter with no kernel
    /// backing at all (its own doc named wiring a real model in as this
    /// task's job). Self-locking, same shape as [`Runtime::apply_size_changed`].
    pub fn connect_client(&self, kind: ClientKind, label: Option<String>) -> ClientId {
        self.lock().tree.connect_client(kind, label)
    }

    /// Disconnects a client this `Runtime` minted (TASK-1.45 §1), applying
    /// every `SizeChanged` the kernel reports as a consequence (a
    /// disconnecting client may have been the latest declarer for some
    /// tab's size, decision-6) - the same `apply_size_changed_locked` every
    /// other mutation already reapplies through. Called once, on every
    /// path out of `connection::handle` (ordinary close, a protocol
    /// violation, a panic unwinding past it).
    ///
    /// # Panics
    /// Never in practice: the only way `Tree::disconnect_client` can fail
    /// is an id this `Runtime` never minted, which `connection::handle`
    /// never passes (it only ever passes back the id its own, prior
    /// `connect_client` call returned).
    pub fn disconnect_client(&self, id: ClientId) {
        let mut state = self.lock();
        let changes = state
            .tree
            .disconnect_client(id)
            .expect("connection::handle only ever disconnects a client it minted itself");
        for change in &changes {
            apply_size_changed_locked(&mut state, change);
        }
    }

    /// A cheap, repeatable way to read the live session count, usable
    /// from a thread that outlives any borrow of `self` (`listen::run`'s
    /// idle checker, TASK-1.86, which must outlive the `&Runtime` every
    /// connection thread borrows inside `thread::scope`). `pub(crate)`:
    /// nothing outside this crate needs it.
    pub(crate) fn session_counter(&self) -> impl Fn() -> usize + Send + 'static {
        let state = Arc::clone(&self.state);
        move || {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tree
                .sessions()
                .len()
        }
    }

    /// Told once, by `listen::run`, only when idle-checking is on
    /// (TASK-1.86) - every pane-exit cascade that closes a session (R94)
    /// sends `ConnectionEvent::SessionCountChanged` on this afterward
    /// (`apply_event_locked`'s own `PaneEvent::Exited` arm, below). Safe
    /// to call before any pane can possibly exist: `listen::run` calls
    /// this immediately after constructing the `Runtime`, before its
    /// accept loop starts accepting anything, and a pane is only ever
    /// minted by a request on an already-accepted, already-handshaken
    /// connection.
    pub(crate) fn set_idle_notifier(&self, notify: Sender<ConnectionEvent>) {
        self.lock().notify_session_count_changed = Some(notify);
    }

    /// Lets a later `server.stop` request (TASK-1.88, `connection.rs`'s
    /// own interception) reach the same mechanism an idle server already
    /// stops itself through - `Stopper::stop()`, the one public way to
    /// end `listen::run`'s accept loop. Called once, unconditionally, by
    /// `listen::run`, for every server (explicit or on-demand) - unlike
    /// [`Runtime::set_idle_notifier`], which an explicit start never
    /// calls (R99: it never idle-stops), an explicit start still needs
    /// this: R99's other half, "a server started explicitly stops only
    /// when told to", is answered by `server.stop`, which needs the same
    /// `Stopper` every server is already given by its own caller today
    /// (`run_server_start` constructs one and passes `&stopper` into
    /// `steg_server::run` on both paths) - nothing downstream could reach
    /// it at all until this.
    pub(crate) fn set_stopper(&self, stopper: crate::listen::Stopper) {
        self.lock().stopper = Some(stopper);
    }

    /// Records the real address this server's socket is reachable at
    /// (TASK-1.146, doc-2 section 4's `STEG_SERVER`) - called once,
    /// unconditionally, by `listen::run`, for every server (explicit or
    /// on-demand), with the same `socket_path` its own listener is
    /// already bound to (`arbiter::attempt`'s own caller resolves that
    /// path once, in `listen::serve`, and hands the identical value to
    /// both). `runtime/start.rs::launch_spec`'s own caller reads this
    /// under the same lock that minted the pane, so a pane's own
    /// `STEG_SERVER` is never a placeholder once this has run.
    ///
    /// `to_string_lossy().into_owned()`, the same conversion `runtime/
    /// login.rs` already uses for a login shell's own `shell`/`home` -
    /// `pane_environment`'s own `server` parameter is a plain `&str`
    /// (`.claude/rules/architecture.md`: no new type for a value nothing
    /// downstream parses back).
    pub(crate) fn bind_server_address(&self, address: &std::path::Path) {
        self.lock().server_address = Some(address.to_string_lossy().into_owned());
    }

    /// How many sessions this server holds right now (TASK-1.88): the
    /// refusal check `server.stop` makes before anything else - a plain,
    /// one-off read, not [`Runtime::session_counter`]'s long-lived
    /// closure (that one exists for the idle checker's thread, which
    /// outlives any `&Runtime` borrow - a different need, unrelated here).
    pub(crate) fn session_count(&self) -> usize {
        self.lock().tree.sessions().len()
    }

    /// A clone of the `Stopper` [`Runtime::set_stopper`] recorded, if any -
    /// `None` only in a test `Runtime` nothing ever wired one into (every
    /// real server, explicit or on-demand, has one by the time it can
    /// accept a request at all).
    pub(crate) fn stopper(&self) -> Option<crate::listen::Stopper> {
        self.lock().stopper.clone()
    }

    /// Stops this `Runtime`: ends every running program, reaps each one,
    /// joins every worker thread, then joins the one drain thread too
    /// (review finding 3; rewritten for TASK-1.78 §4 to reuse [`force_end`],
    /// the same mechanism a single pane's forced close uses, not the
    /// old `PaneHandle::stop`'s pid-only, unescalated `SIGHUP`, which
    /// TASK-1.76's own reviewer confirmed left a program trapping that
    /// signal running forever even once the whole server tried to stop).
    /// A behaviour change, named rather than folded in silently: this used
    /// to be serial (`N * STOP_TIMEOUT` worst case, 5s per pane); now it is
    /// `force_grace + ESCALATE_CONFIRM_TIMEOUT + FORCE_JOIN_TIMEOUT` for
    /// *any* number of panes, reaching a trapping program and a
    /// non-job-control grandchild (§0.4) the old mechanism could not. This
    /// is the method `listen::run` calls, once every connection thread
    /// has already been joined (TASK-1.45 §1 - `&mut self` is what makes
    /// that a property the compiler itself enforces there, not merely
    /// documented).
    ///
    /// Idempotent: every field this empties is `Option`-wrapped, so a
    /// second call (including the one `Drop` makes) finds nothing left to
    /// do and returns at once. Every request this `Runtime` is given
    /// after this call panics (`self.events`'s own `.expect()` inside
    /// `start_and_dispatch`) rather than silently starting a program this
    /// `Runtime` can no longer track - stopping is meant to be the last
    /// thing done with one.
    pub fn stop(&mut self) {
        let panes = {
            let mut state = self.lock();
            // Drops this `Runtime`'s own clone of the idle checker's
            // notifier (TASK-1.86), if any - one of the senders
            // `listen::run`'s own join bound (section 6 of this task's
            // plan) is derived against: a checker blocked with no
            // timeout at all (a session still open when something other
            // than its own decision calls `stop`) can only be woken by
            // its channel disconnecting, which needs every sender into
            // it gone, this one included.
            state.notify_session_count_changed = None;
            std::mem::take(&mut state.panes)
        };
        force_end(panes.into_iter().collect(), self.force_grace);

        // Dropping every clone of `events` (this one, the last) is what
        // lets the drain thread's own `for event in events_rx` end -
        // every pane's own clone already went with its worker thread,
        // stopped just above.
        drop(self.events.take());
        if let Some(drain) = self.drain.take() {
            // The same hard-guarantee reasoning as `FORCE_JOIN_TIMEOUT`:
            // every pane's own clone of `events` is already gone by this
            // point (dropped above, inside `force_end`'s own joins), so
            // the drain thread's loop has nothing left to read and is
            // already on its way out.
            let _ = crate::pane::join_with_bound(drain, FORCE_JOIN_TIMEOUT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use crate::pty::RealLauncher;
    use crate::runtime::testing::{TEST_GRACE, TEST_TICK, login};

    /// `Runtime::session_count` (TASK-1.88): tracks real sessions minted
    /// and closed through `Runtime::handle` itself, not a re-derivation -
    /// the one fact `server.stop`'s own refusal check reads.
    #[test]
    fn session_count_tracks_sessions_created_and_closed_through_handle() {
        use steg_sdk::{Request, Response};

        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let description = steg_sdk::description();
        let context = crate::dispatch::Context::none();
        assert_eq!(runtime.session_count(), 0);

        let session = match runtime.handle(
            &description,
            &context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "sleep 100"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            Response::Ok(value) => value,
            Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        assert_eq!(runtime.session_count(), 1);

        let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();
        match runtime.handle(
            &description,
            &context,
            &Request {
                method: "session.close".to_string(),
                params: serde_json::json!({"target": session_id.to_string(), "force": true})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            Response::Ok(_) => {}
            Response::Error(error) => panic!("session.close failed: {error:?}"),
        }
        assert_eq!(runtime.session_count(), 0);
    }

    /// `Runtime::set_stopper`/`stopper` (TASK-1.88): a thin, self-locking
    /// wrapper proof - a `Runtime` with none set reports `None`, and
    /// setting one makes it reachable again, by whatever its own `Clone`
    /// equality would show (`Stopper` carries no `PartialEq`, so this
    /// only checks presence, not identity - the real behavioural proof,
    /// that the clone returned actually stops the same `run`, is
    /// `server_stop.rs`'s own socket-level test, which alone can observe
    /// a `Stopper` doing anything).
    #[test]
    fn stopper_is_none_until_set_then_some() {
        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        assert!(runtime.stopper().is_none());

        runtime.set_stopper(crate::listen::Stopper::new());

        assert!(runtime.stopper().is_some());
    }

    /// `Runtime::connect_client`/`disconnect_client` (TASK-1.45 §1): a
    /// thin, self-locking wrapper proof, not a re-proof of the kernel's
    /// own cascade (`crates/kernel/src/tree.rs`'s own
    /// `latest_falls_back_to_the_next_most_recently_active_remaining_viewer_on_departure`
    /// already proves `Tree::disconnect_client` itself). What this proves
    /// is that `Runtime`'s own wrapper actually calls through to the real
    /// tree and reapplies whatever `SizeChanged`s it gets back - checked
    /// on a real pane's real pty, the same way every other
    /// `apply_size_changed` test in this module already does.
    #[test]
    fn disconnect_client_reapplies_the_next_latest_declared_size() {
        use steg_sdk::{ClientKind, Request, Size};

        let runtime = Runtime::new(
            std::sync::Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let description = steg_sdk::description();
        let no_context = crate::dispatch::Context::none();
        let session = match runtime.handle(
            &description,
            &no_context,
            &Request {
                method: "session.new".to_string(),
                params: serde_json::json!({"program": ["sh", "-c", "sleep 100"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ) {
            steg_sdk::Response::Ok(value) => value,
            steg_sdk::Response::Error(error) => panic!("session.new failed: {error:?}"),
        };
        let session_id: steg_sdk::SessionId = session["id"].as_str().unwrap().parse().unwrap();
        let (pane, tab_id) = {
            let state = runtime.state.lock().unwrap();
            let tab = &state.tree.session(session_id).unwrap().tabs()[0];
            (tab.panes()[0].id(), tab.id())
        };

        // `a` and `b` are minted through the public wrapper this test is
        // about; `view_pane`/`declare_size` have no `Runtime`-level
        // wrapper of their own (no dispatch method reaches them yet,
        // TASK-1.42's own gap), so those two calls reach the tree
        // directly, the same way this module's other size tests already
        // do.
        let a = runtime.connect_client(ClientKind::App, None);
        let b = runtime.connect_client(ClientKind::Cli, None);
        let first_change = {
            let mut state = runtime.state.lock().unwrap();
            state.tree.view_pane(a, pane).unwrap();
            state.tree.view_pane(b, pane).unwrap();
            state
                .tree
                .declare_size(a, tab_id, Size::new(80, 24))
                .unwrap();
            state
                .tree
                .declare_size(b, tab_id, Size::new(40, 10))
                .unwrap()
        };
        runtime.apply_size_changed(&first_change);
        assert_eq!(
            runtime.state.lock().unwrap().panes[&pane]
                .snapshot()
                .unwrap()
                .rows
                .len(),
            10,
            "b, the latest, declared 40x10 last"
        );

        runtime.disconnect_client(b);

        assert_eq!(
            runtime.state.lock().unwrap().panes[&pane]
                .snapshot()
                .unwrap()
                .rows
                .len(),
            24,
            "a's own 80x24 must be reapplied once b (the latest) disconnects"
        );
    }
}

#[cfg(test)]
mod stop_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use steg_sdk::{PaneId, Program};

    use super::Runtime;
    use crate::pane::PaneHandle;
    use crate::pty::{LaunchSpec, RealLauncher};

    /// How long these tests poll for a real program's own printed pid
    /// before giving up - bounded, the same reasoning every other
    /// real-process wait in this crate gives.
    const POLL_TIMEOUT: Duration = Duration::from_secs(10);

    /// A working `PATH` is required (TASK-1.87): see `mod tests`'s own
    /// `test_base_env` for why - duplicated here rather than shared
    /// across modules, matching this file's own existing style of each
    /// test module carrying its own small `login`-style helper.
    fn test_base_env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
        vec![(
            std::ffi::OsString::from("PATH"),
            std::env::var_os("PATH").unwrap_or_default(),
        )]
    }

    /// A short, real `force_grace` for this module's own tests - unlike
    /// `tests::TEST_GRACE`, every test here actually exercises `force_end`
    /// through `Runtime::stop`, so this has to be long enough for a
    /// trapping program's test to prove the escalation really happens
    /// after the polite signal is given a real chance, not so long that
    /// the whole module's run time grows needlessly.
    const TEST_GRACE: Duration = Duration::from_millis(200);

    /// A tiny `attach_tick` for every `Runtime::new` in this module - same
    /// reasoning as `tests::TEST_TICK`: nothing here attaches to a pane.
    const TEST_TICK: Duration = Duration::from_millis(1);

    fn login() -> crate::runtime::LoginShell {
        crate::runtime::LoginShell {
            argv0: "/bin/sh".to_string(),
            home: "/tmp".to_string(),
            user: None,
            base_env: test_base_env(),
        }
    }

    /// Whether a process with this id still exists, reaped or not - the
    /// same `kill -0` probe `crate::pane`'s own tests already use.
    fn process_exists(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("kill(1) is on PATH on every platform this runs on")
            .success()
    }

    /// Starts a real pane whose own program prints its pid with `$$`
    /// then `exec`s into `then` - keeping the same pid, so the one
    /// number this reads back from the screen is the pid of whatever
    /// ends up actually running, with no new accessor needed on
    /// `PaneHandle` for a test to learn it. Returns the handle's own
    /// `PaneEvent` receiver too (review nit): `Exited` is a real event a
    /// caller that cares when the program ends should wait on instead of
    /// polling, unlike the pid file below, which has no event at all.
    fn start_pane_reporting_its_pid(
        runtime: &Runtime,
        then: &str,
    ) -> (
        PaneId,
        PaneHandle,
        u32,
        std::sync::mpsc::Receiver<crate::pane::PaneEvent>,
    ) {
        let pane = {
            let mut state = runtime.state.lock().unwrap();
            let session = state.tree.new_session(None).unwrap();
            let pane = state.tree.session(session).unwrap().tabs()[0].panes()[0].id();
            state
                .tree
                .start_program(
                    pane,
                    Program::Command {
                        argv: vec!["sh".to_string()],
                        cwd: "/tmp".to_string(),
                    },
                )
                .unwrap();
            pane
        };

        // The pid is written to its own file, not the screen: a program
        // like `yes` floods the screen fast enough to scroll a printed
        // pid line out of the visible area before a poll loop can ever
        // read it back, which is a race against the test's own reading,
        // not against the program - a file has no such race. There is
        // still no event for "a file now has this content" (review nit),
        // unlike the process's own exit below, so this one stays a
        // bounded poll.
        //
        // Named with a per-call counter, not just `pane`: every test in
        // this module builds its own fresh `Tree`, so `pane` alone is not
        // unique across tests - under this crate's default, parallel
        // `cargo test` (unlike the `--test-threads=1` these tests were
        // first proven stable under) two of them really did collide on
        // the same file (`just check`'s own recorded run caught this).
        static UNIQUE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = UNIQUE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid_file = std::env::temp_dir().join(format!(
            "steg-stop-test-{}-{pane}-{unique}.pid",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&pid_file);
        let (events_tx, events_rx) = std::sync::mpsc::channel();
        let handle = crate::pane::start(
            LaunchSpec {
                argv: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    format!("echo $$ > {} ; exec {then}", pid_file.display()),
                ],
                cwd: "/tmp".to_string(),
                env: test_base_env(),
                size: (80, 24),
            },
            pane,
            Arc::new(RealLauncher::default()),
            events_tx,
            TEST_TICK,
        )
        .unwrap();

        let start = std::time::Instant::now();
        let pid = loop {
            if let Ok(text) = std::fs::read_to_string(&pid_file)
                && let Ok(pid) = text.trim().parse()
            {
                break pid;
            }
            assert!(
                start.elapsed() < POLL_TIMEOUT,
                "never saw a pid written to {}",
                pid_file.display()
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        let _ = std::fs::remove_file(&pid_file);

        (pane, handle, pid, events_rx)
    }

    #[test]
    fn stop_ends_a_program_producing_output() {
        let mut runtime = Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (pane, handle, pid, _events_rx) = start_pane_reporting_its_pid(&runtime, "yes");
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        assert!(
            process_exists(pid),
            "the program must be running before stop"
        );

        runtime.stop();

        assert!(!process_exists(pid), "stop must end the program");
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    #[test]
    fn stop_ends_a_silent_program() {
        let mut runtime = Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        let (pane, handle, pid, _events_rx) = start_pane_reporting_its_pid(&runtime, "sleep 1000");
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        assert!(
            process_exists(pid),
            "the program must be running before stop"
        );

        runtime.stop();

        assert!(!process_exists(pid), "stop must end the program too");
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    #[test]
    fn stop_is_a_no_op_for_a_pane_whose_program_has_already_ended() {
        let mut runtime = Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        // `true` exits at once, no `exec sleep` keeping it alive - by the
        // time this pid is known at all (printed, then read back), the
        // process behind it is already gone or going; this test only
        // needs that to have fully happened, confirmed the same way the
        // other `stop` tests confirm a process ended.
        let (pane, handle, pid, events_rx) = start_pane_reporting_its_pid(&runtime, "true");
        // An event wait first, not a bare sleep-poll (review nit):
        // `Exited` is a real event here (unlike the pid file above,
        // which has none), so this waits on it rather than polling
        // `process_exists` from the start. It is not the *whole*
        // answer, though: `Exited` only means our own wait() on the
        // child has returned, not that the OS has finished reaping it
        // (observed empirically - `process_exists` can still report
        // `true` for a moment right after `Exited` fires) - there is no
        // second event for "the process table caught up", so the short
        // remaining gap is still a bounded poll, now with far less of it
        // to cover than polling from the start would.
        events_rx
            .recv_timeout(POLL_TIMEOUT)
            .expect("the real `true` should have exited within the bound");
        let start = std::time::Instant::now();
        while process_exists(pid) {
            assert!(
                start.elapsed() < POLL_TIMEOUT,
                "the process was never actually reaped after its own Exited event"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        runtime.state.lock().unwrap().panes.insert(pane, handle);

        runtime.stop(); // must not hang or panic on an already-ended program

        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    #[test]
    fn stop_with_no_panes_at_all_returns_at_once() {
        let mut runtime = Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );
        runtime.stop();
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }

    /// TASK-1.78 §4's own real behaviour change: the old `PaneHandle::
    /// stop` sent only an unescalated `SIGHUP` by pid, so a program
    /// trapping it survived `Runtime::stop`/`Drop` indefinitely (confirmed
    /// by this task's own reviewer, in a scratch test against the
    /// previous mechanism). Reusing `force_end` fixes this as a direct
    /// consequence, not a special case - proven here rather than only
    /// argued.
    #[test]
    fn stop_ends_a_program_that_traps_the_polite_signal() {
        let mut runtime = Runtime::new(
            Arc::new(RealLauncher::default()),
            login(),
            TEST_GRACE,
            TEST_TICK,
        );

        // Waits for a real marker the nested shell writes *after*
        // installing its own trap, before calling `stop` - found
        // necessary by probing, not assumed: without it, `stop` can race
        // the newly forked shell's own first few instructions and, under
        // heavy CPU contention, send its signal before the shell has
        // reached its own `trap` builtin at all (the same race `pane::
        // ending_tests`'s own trapping test found and fixed the same
        // way). `start_pane_reporting_its_pid`'s own `exec {then}` means
        // `then` here is a nested `sh -c` so the trap can be installed
        // *inside* the process that then execs into `sleep`, not applied
        // to `trap` itself (which has no external binary to exec).
        let ready = std::env::temp_dir().join(format!(
            "steg-stop-trap-ready-{}-{}.marker",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_file(&ready);
        let (pane, handle, pid, _events_rx) = start_pane_reporting_its_pid(
            &runtime,
            &format!(
                "sh -c \"trap '' HUP; touch {}; exec sleep 1000\"",
                ready.display()
            ),
        );
        runtime.state.lock().unwrap().panes.insert(pane, handle);
        assert!(
            process_exists(pid),
            "the program must be running before stop"
        );
        let start = std::time::Instant::now();
        while !ready.exists() {
            assert!(
                start.elapsed() < POLL_TIMEOUT,
                "the nested shell never reached its own trap+marker"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = std::fs::remove_file(&ready);

        runtime.stop();

        assert!(
            !process_exists(pid),
            "stop must still end a program that traps the polite signal, \
             by escalating - the old mechanism could not"
        );
        assert!(runtime.state.lock().unwrap().panes.is_empty());
    }
}
