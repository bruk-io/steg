//! What can go wrong running a pane's program.

use std::fmt;

/// An error from the server's half of TASK-1.40.
#[derive(Debug)]
pub enum Error {
    /// A kernel operation failed (the pane's own state: starting,
    /// recording an exit, or restarting its program).
    Kernel(steg_kernel::Error),
    /// The pseudo-terminal could not be opened, or the program could not
    /// be spawned in it (for instance, the command does not exist).
    Pty(anyhow::Error),
    /// Reading from, or writing to, the pseudo-terminal failed.
    Io(std::io::Error),
    /// A pane's worker thread is no longer there to answer - the pane was
    /// already shut down, or its thread panicked.
    PaneGone,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Kernel(cause) => write!(f, "the kernel refused it: {cause}"),
            Error::Pty(cause) => write!(f, "the pseudo-terminal failed: {cause}"),
            Error::Io(cause) => write!(f, "{cause}"),
            Error::PaneGone => write!(f, "this pane's worker thread is no longer there"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Kernel(cause) => Some(cause),
            Error::Io(cause) => Some(cause),
            // `anyhow::Error` does not implement `std::error::Error` itself
            // (by design, so `?` composes without specialisation
            // conflicts); its own `Display` already carries the full
            // chain, which is what the line above prints.
            Error::Pty(_) | Error::PaneGone => None,
        }
    }
}

impl From<steg_kernel::Error> for Error {
    fn from(cause: steg_kernel::Error) -> Self {
        Error::Kernel(cause)
    }
}

impl From<std::io::Error> for Error {
    fn from(cause: std::io::Error) -> Self {
        Error::Io(cause)
    }
}
