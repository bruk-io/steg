//! Shared bounded-wait helpers for this crate's own unit tests
//! (TASK-1.113) - `#[cfg(test)]` only, so none of this is compiled into a
//! real build. `crates/server/tests/support/mod.rs` has the same shape
//! for the integration tests under `tests/`: a separate module, not
//! shared from here - an integration test file is its own compilation
//! unit and cannot import a crate-private module, which is why this is
//! duplicated rather than hidden behind one shared path.

use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::outbound::Outbound;

/// Runs `body` on its own thread and waits up to `bound` for it to
/// finish, panicking - naming `what` and `bound` - if it does not. Not a
/// sleep loop: the wait is a channel receive with a timeout
/// (`.claude/rules/testing-rust.md`: "no test sleeps to wait for
/// something; it waits on the event"). If `body` never returns, the
/// spawned thread is simply abandoned once this function panics - Rust
/// has no way to force a thread to stop - but that costs nothing once
/// this test binary's own process exits.
pub(crate) fn run_bounded<T, F>(what: &str, bound: Duration, body: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(body());
    });
    rx.recv_timeout(bound)
        .unwrap_or_else(|_| panic!("timed out after {bound:?} waiting for: {what}"))
}

/// Ends an `Outbound`'s writer thread on every exit path from the test
/// that owns it - a normal return, a failing assertion, or a panic
/// unwinding through it - the same reason `connection.rs`'s own
/// `ConnectionCleanup` has a `Drop` in production: a test that only
/// calls `close()`/`writer.join()` after its own assertions leaves the
/// writer thread parked forever if one of those assertions panics
/// first. Hygiene, not a fix for any particular hang - nothing here
/// changes what ended this crate's own sighted test (TASK-1.113's
/// notes).
///
/// `Drop` can run mid-unwind, so this never panics itself: a join that
/// somehow still times out only logs to stderr, it does not turn an
/// already-failing test into a double panic (which would abort the
/// whole process instead of reporting one clean failure).
pub(crate) struct ClosingWriterGuard {
    outbound: Arc<Outbound>,
    writer: Option<JoinHandle<()>>,
}

impl ClosingWriterGuard {
    pub(crate) fn new(outbound: Arc<Outbound>, writer: JoinHandle<()>) -> Self {
        Self {
            outbound,
            writer: Some(writer),
        }
    }
}

impl Drop for ClosingWriterGuard {
    fn drop(&mut self) {
        self.outbound.close();
        let Some(writer) = self.writer.take() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(writer.join());
        });
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => {}
            Ok(Err(_)) => eprintln!("ClosingWriterGuard::drop: the writer thread panicked"),
            Err(_) => {
                eprintln!(
                    "ClosingWriterGuard::drop: the writer thread did not end within 5s of close()"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run_bounded;
    use std::time::Duration;

    #[test]
    fn a_body_that_returns_promptly_is_not_delayed() {
        assert_eq!(run_bounded("a quick body", Duration::from_secs(5), || 7), 7);
    }

    /// AC #3's own negative control for the bound mechanism itself: a
    /// body that never returns must fail the test within the bound,
    /// naming what it waited for, rather than hang - proved here with no
    /// socket or fork involved at all, independent of anything specific
    /// to `outbound.rs`.
    #[test]
    #[should_panic(expected = "timed out after 200ms waiting for: a body that never returns")]
    fn a_body_that_never_returns_fails_fast_naming_what_it_waited_for() {
        run_bounded(
            "a body that never returns",
            Duration::from_millis(200),
            || loop {
                std::thread::park();
            },
        );
    }
}
