//! The pseudo-terminal and the process running in it (doc-16 R1: "the
//! server is where I/O meets the kernel"; TASK-1.40).
//!
//! [`ProgramLauncher`] is the interface the brief means by "an interface
//! the server supplies": real I/O behind it ([`RealLauncher`], over
//! `portable-pty`), so the orchestration in `crate::pane` is driven by a
//! fake at the boundary in tests instead of a real OS process every time
//! (`.claude/rules/testing-rust.md`). Opening the pty and spawning the
//! process both happen inside one call to [`ProgramLauncher::spawn`], on
//! whichever thread calls it - `portable-pty`'s own master, writer and
//! child handles are `Send` (decision-3 section 7's probe: only the
//! terminal engine is pinned to its own thread), so a bad command is a
//! plain `Result` here, never an error that has to escape a spawned
//! thread.

use std::ffi::OsString;
use std::io::{Read, Write};

use crate::error::Error;

/// What to run, and in what, for one pseudo-terminal.
///
/// Built by the caller from a pane's stored `steg_sdk::Program` (resolving
/// a plain shell to a concrete `argv` is the caller's job - see
/// `crate::pane::start`), its environment (`crate::env::pane_environment`)
/// and its size - never computed inside [`ProgramLauncher::spawn`] itself,
/// so a functional test can hand a `FakeLauncher` a `LaunchSpec` with no
/// real socket, no real installed binary and no real OS process at all.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    /// The program and its arguments, in the order given on the command
    /// line: `argv[0]` is the program to run.
    pub argv: Vec<String>,
    /// Its working directory.
    pub cwd: String,
    /// Environment variables to set or override on top of whatever this
    /// process already inherits - `portable_pty::CommandBuilder` seeds
    /// from the current process's own environment on its own, so this is
    /// only ever the overrides (the five `STEG_*` variables and the new
    /// `PATH`), never the whole environment restated.
    pub env: Vec<(OsString, OsString)>,
    /// The pseudo-terminal's size, in columns and rows.
    pub size: (u16, u16),
}

/// What spawning a [`LaunchSpec`] gives back: the pieces a pane's worker
/// thread needs, already split so each can move to a different thread -
/// the writer and the resize closure stay on the pane's own thread, the
/// reader and the child move to the short relay thread `crate::pane`
/// spawns for this one run.
pub struct SpawnedProgram {
    /// Writes to the program's standard input.
    pub writer: Box<dyn Write + Send>,
    /// Reads the program's output. Reaches end-of-file once the program
    /// has exited and nothing else holds the pty's slave side open.
    pub reader: Box<dyn Read + Send>,
    /// Resizes the pseudo-terminal.
    pub resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send>,
    /// A handle to wait for the program's exit code, or to kill it.
    pub child: Box<dyn portable_pty::Child + Send + Sync>,
}

/// Starts a program in a pseudo-terminal (doc-9 R13).
pub trait ProgramLauncher: Send + Sync {
    /// # Errors
    ///
    /// Returns [`Error::Pty`] if the pseudo-terminal could not be opened,
    /// or the program could not be spawned in it (for instance, the
    /// command does not exist).
    fn spawn(&self, spec: &LaunchSpec) -> Result<SpawnedProgram, Error>;
}

/// The real implementation, over `portable-pty`. Mirrors
/// `spikes/task-1.2-wire-protocol/probe-server/src/pty.rs`'s own shape,
/// which already proved this reads cleanly against this exact problem.
#[derive(Debug, Default)]
pub struct RealLauncher;

impl ProgramLauncher for RealLauncher {
    fn spawn(&self, spec: &LaunchSpec) -> Result<SpawnedProgram, Error> {
        let (cols, rows) = spec.size;
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system
            .openpty(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|cause| Error::Pty(cause.into()))?;

        let mut command =
            portable_pty::CommandBuilder::from_argv(spec.argv.iter().map(OsString::from).collect());
        command.cwd(&spec.cwd);
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|cause| Error::Pty(cause.into()))?;
        // Dropping the slave is what lets the program's own exit reach the
        // reader as end-of-file: the pty's slave side only closes once
        // nothing holds it open, and this process's own copy is the thing
        // standing in the way (mirrors the spike's `pty.rs`: "The
        // program ended. The pane and its screen stay: that is R2").
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|cause| Error::Pty(cause.into()))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|cause| Error::Pty(cause.into()))?;
        let master = pair.master;
        let resize: Box<dyn Fn(u16, u16) -> Result<(), Error> + Send> =
            Box::new(move |cols, rows| {
                master
                    .resize(portable_pty::PtySize {
                        rows,
                        cols,
                        pixel_width: 0,
                        pixel_height: 0,
                    })
                    .map_err(|cause| Error::Pty(cause.into()))
            });

        Ok(SpawnedProgram {
            writer,
            reader,
            resize,
            child,
        })
    }
}
