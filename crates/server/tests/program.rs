//! Functional tests for running a pane's program (TASK-1.40): the outside
//! world faked at the boundary (`.claude/rules/testing-rust.md`), so the
//! orchestration - starting, ending, restarting, resizing - is proven
//! fast and without a real OS process. `real_pty.rs` pairs this with the
//! few tests that need the real thing.

use std::io::Write as _;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use steg_sdk::{PaneId, Screen};

use steg_server::{Error, LaunchSpec, PaneEvent, ProgramLauncher, SpawnedProgram};

/// What a `FakeLauncher`'s `spawn` call hands back to the test driving it:
/// the write end of the pipe that stands in for the fake program's
/// output, and the exit code it ends with.
struct FakeDriver {
    output: std::io::PipeWriter,
    exit: mpsc::Sender<i32>,
}

impl FakeDriver {
    fn print(&mut self, bytes: &[u8]) {
        self.output
            .write_all(bytes)
            .expect("the pipe is still open: nothing has called `end` yet");
        self.output
            .flush()
            .expect("the pipe is still open: nothing has called `end` yet");
    }

    /// Ends the fake program: closes its output (so the relay thread's
    /// read loop sees end-of-file) and reports the exit code.
    fn end(self, exit_code: i32) {
        drop(self.output);
        let _ = self.exit.send(exit_code);
    }
}

#[derive(Debug)]
struct FakeChild {
    // `portable_pty::Child` must be `Send + Sync`, and `mpsc::Receiver`
    // is only `Send` (it allows a single consumer) - the `Mutex` is what
    // makes this fake satisfy that bound, not a sign of real concurrent
    // access (`&mut self` already keeps `try_wait`/`wait` exclusive).
    exit_code: Mutex<mpsc::Receiver<i32>>,
}

impl portable_pty::Child for FakeChild {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        match self
            .exit_code
            .lock()
            .expect("not poisoned: &mut self already keeps this exclusive")
            .try_recv()
        {
            Ok(code) => Ok(Some(exit_status(code))),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Ok(None),
        }
    }

    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        self.exit_code
            .lock()
            .expect("not poisoned: &mut self already keeps this exclusive")
            .recv()
            .map(exit_status)
            .map_err(|_| std::io::Error::other("the fake program's exit was never reported"))
    }

    fn process_id(&self) -> Option<u32> {
        None
    }
}

fn exit_status(code: i32) -> portable_pty::ExitStatus {
    #[expect(
        clippy::cast_sign_loss,
        reason = "a real exit code is 0..=255; this fake is never fed a negative one"
    )]
    portable_pty::ExitStatus::with_exit_code(code as u32)
}

impl portable_pty::ChildKiller for FakeChild {
    fn kill(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        // Not exercised by these tests: killing a fake program independently
        // of waiting on it is not part of TASK-1.40's scope (`--force` is
        // named as a deliberate exclusion in the task's plan).
        let (_tx, rx) = mpsc::channel();
        Box::new(FakeChild {
            exit_code: Mutex::new(rx),
        })
    }
}

/// A [`ProgramLauncher`] with no real pseudo-terminal and no real OS
/// process: each `spawn` call hands the test a [`FakeDriver`] (through the
/// receiver [`FakeLauncher::new`] returns) instead of actually running
/// anything.
struct FakeLauncher {
    drivers: Mutex<mpsc::Sender<FakeDriver>>,
}

impl FakeLauncher {
    fn new() -> (Self, mpsc::Receiver<FakeDriver>) {
        let (tx, rx) = mpsc::channel();
        (
            Self {
                drivers: Mutex::new(tx),
            },
            rx,
        )
    }
}

impl ProgramLauncher for FakeLauncher {
    fn spawn(&self, _spec: &LaunchSpec) -> Result<SpawnedProgram, Error> {
        let (reader, writer) = std::io::pipe()?;
        let (exit_tx, exit_rx) = mpsc::channel();

        let driver = FakeDriver {
            output: writer,
            exit: exit_tx,
        };
        self.drivers
            .lock()
            .expect("not poisoned: nothing else locks this")
            .send(driver)
            .expect("the test still holds the matching receiver");

        Ok(SpawnedProgram {
            writer: Box::new(std::io::sink()),
            reader: Box::new(reader),
            resize: Box::new(|_cols, _rows| Ok(())),
            child: Box::new(FakeChild {
                exit_code: Mutex::new(exit_rx),
            }),
            // No real open to retry here (TASK-1.112) - this fake never
            // fails its own "open", so it always took exactly one attempt.
            open_attempts: 1,
        })
    }
}

fn spec() -> LaunchSpec {
    LaunchSpec {
        argv: vec!["fake".to_string()],
        cwd: "/".to_string(),
        env: Vec::new(),
        size: (20, 5),
    }
}

fn screen_text(screen: &Screen) -> String {
    screen
        .rows
        .iter()
        .flat_map(|row| row.iter().map(|run| run.text.as_str()))
        .collect()
}

#[test]
fn a_programs_exit_code_reaches_the_event_channel() {
    let (launcher, drivers) = FakeLauncher::new();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(spec(), pane, Arc::new(launcher), events_tx).unwrap();
    let driver = drivers.recv().unwrap();

    driver.end(3);

    assert_eq!(
        events_rx.recv().unwrap(),
        PaneEvent::Exited { pane, exit_code: 3 }
    );
    handle.shutdown();
}

#[test]
fn output_printed_before_exit_is_in_the_snapshot() {
    let (launcher, drivers) = FakeLauncher::new();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(spec(), pane, Arc::new(launcher), events_tx).unwrap();
    let mut driver = drivers.recv().unwrap();

    driver.print(b"hello");
    driver.end(0);
    events_rx.recv().unwrap();

    let screen = handle.snapshot().unwrap();
    assert!(
        screen_text(&screen).contains("hello"),
        "{:?}",
        screen_text(&screen)
    );
    handle.shutdown();
}

#[test]
fn restarting_keeps_the_same_terminal_so_old_output_survives_a_new_run() {
    let (launcher, drivers) = FakeLauncher::new();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(spec(), pane, Arc::new(launcher), events_tx).unwrap();
    let mut first_driver = drivers.recv().unwrap();
    first_driver.print(b"first-run");
    first_driver.end(3);
    events_rx.recv().unwrap();

    handle.restart(spec()).unwrap();
    let mut second_driver = drivers.recv().unwrap();
    second_driver.print(b" second-run");
    second_driver.end(0);
    events_rx.recv().unwrap();

    let screen = handle.snapshot().unwrap();
    let text = screen_text(&screen);
    assert!(text.contains("first-run"), "{text:?}");
    assert!(text.contains("second-run"), "{text:?}");
    handle.shutdown();
}

#[test]
fn resize_reaches_the_terminal_even_with_no_program_running() {
    let (launcher, drivers) = FakeLauncher::new();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(spec(), pane, Arc::new(launcher), events_tx).unwrap();
    let driver = drivers.recv().unwrap();
    driver.end(0);
    events_rx.recv().unwrap();

    // No program is running (between an exit and a restart) - resizing
    // must still succeed, since the terminal itself (and its scrollback)
    // is still there to resize.
    handle.resize(10, 3).unwrap();

    handle.shutdown();
}

#[test]
fn a_pane_with_no_program_running_accepts_input_as_a_no_op() {
    let (launcher, drivers) = FakeLauncher::new();
    let (events_tx, events_rx) = mpsc::channel();
    let pane = PaneId::from_raw(1);

    let handle = steg_server::start(spec(), pane, Arc::new(launcher), events_tx).unwrap();
    let driver = drivers.recv().unwrap();
    driver.end(0);
    events_rx.recv().unwrap();

    handle.send_input(b"ignored".to_vec()).unwrap();

    handle.shutdown();
}
