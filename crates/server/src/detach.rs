//! Leaving the session and process group inherited from whatever started this
//! process (R98: detaching is the server's own code, never the CLI's).

/// Starts a new session, detaching from any controlling terminal and the process
/// group inherited from whatever spawned this process. `Err` only if this process
/// is already a process-group leader (typically: invoked directly at an
/// interactive shell prompt, never through the sdk's own spawn path -
/// `crates/steg`'s own tests cover both). Not fatal to a caller - `crates/steg`'s
/// own handling continues attached and says so.
///
/// # Errors
/// Whatever `setsid(2)` fails with, converted from `nix::errno::Errno` via its own
/// `impl From<Errno> for io::Error` (confirmed by reading `nix-0.28.0`'s own
/// `errno.rs`, the same conversion `error.rs`'s existing `Io` variant already
/// relies on elsewhere in this crate) - kept as a plain `io::Result` rather than
/// wrapped in this crate's own `Error`, since nothing here needs `Error`'s richer
/// shape for a value a caller only ever displays, never pattern-matches.
pub fn detach() -> std::io::Result<()> {
    nix::unistd::setsid()
        .map(|_pid| ())
        .map_err(std::io::Error::from)
}
