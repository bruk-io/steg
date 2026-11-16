//! Sending a real, unignorable OS signal to a run's whole process group
//! (TASK-1.78 §0/§1).
//!
//! `pane::Running` no longer holds a `portable_pty::ChildKiller` (that
//! clone's own `kill()` sends `SIGHUP` by pid alone, with no escalation and
//! no group — confirmed by reading `portable-pty` 0.9.0's `lib.rs:293-338`,
//! not `unix.rs` as TASK-1.40/1.76's own notes said). This module is the
//! replacement: one call, straight to the OS, using the pid `pane::start`/
//! `restart` already reads off the spawned child before it moves into the
//! relay thread.
//!
//! The *process group*, not the pid alone, is what is signalled:
//! `portable-pty`'s own `pre_exec` (`unix.rs`) calls `setsid()` before
//! `exec`, which makes the child's pgid equal its own pid at that moment —
//! so `killpg` with that same number as the group id reaches a pipeline or
//! a background job under a non-job-control shell too (TASK-1.78 §0's own
//! experiment), not only the one process a plain `kill` would. An
//! *interactive* shell's own job control reassigns a backgrounded job to a
//! new process group, which this does not reach — named, not fixed,
//! TASK-1.78's own plan, §0.

use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;

/// Sends `signal` to the process group `pid` leads. Never errors to its
/// caller: `ESRCH` (the process is already gone) is not a failure here —
/// every caller confirms the real outcome by waiting for the program's own
/// exit, never by this call's own return value (there is nothing useful to
/// return: a `Result` whose only interesting case is "it was already dead"
/// would just be checked and discarded at every call site).
pub(crate) fn send(pid: u32, signal: Signal) {
    let _ = killpg(Pid::from_raw(pid as i32), signal);
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    use std::time::Duration;

    use nix::sys::signal::Signal;

    use super::send;

    /// Guarantees a real pid is dead by the time this value drops,
    /// including when a panic unwinds through it - a test's own fixture
    /// must never depend on the very mechanism it is testing to clean up
    /// after itself, or a real bug in `send` would leave a `sleep 1000`
    /// running past the end of the test. Takes only the pid, never the
    /// `Child` itself, so it never has to fight Rust's "can't partially
    /// move a field out of a type with its own `Drop`" rule: a plain
    /// numeric guard beside the `Child` a test still owns and waits on
    /// directly, not a wrapper around it.
    ///
    /// Every child this module's tests spawn is also built with
    /// `Stdio::null()`/`Stdio::piped()` on every stream, never inherited -
    /// this is the fix for the actual incident this comment is about: an
    /// earlier round of this task's own build (not committed) left a
    /// `sleep 1000` with this test binary's own stdout inherited; once
    /// that `sleep` outlived the test (the exact gap `send` was still a
    /// no-op stub for, at that point in the build), it kept the harness's
    /// stdout pipe open, which hung `asdlc record-check`'s own blocking
    /// read of it for several minutes, not the compiler. A guard that only
    /// signals the process cannot fix a pipe already held open by
    /// inherited stdio - redirecting it in the first place is what does.
    struct KillOnDrop(u32);

    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(self.0 as i32),
                Signal::SIGKILL,
            );
        }
    }

    /// Joins a real `std::process::Child` with a bound, the same reasoning
    /// as `crate::pane::join_with_bound`: `Child::wait` has no built-in
    /// timeout, so a watchdog thread does the blocking wait and this
    /// blocks on a bounded `recv_timeout` for it instead of a bare, maybe-
    /// forever wait.
    fn wait_within(
        mut child: std::process::Child,
        timeout: Duration,
    ) -> Option<std::process::ExitStatus> {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let status = child.wait();
            let _ = done_tx.send(status);
        });
        done_rx.recv_timeout(timeout).ok().and_then(Result::ok)
    }

    /// `send` reaches a real process, not just a well-formed call: a
    /// `sleep` with no trap dies on the polite signal alone (TASK-1.78 §0's
    /// own experiment, reproduced here through this module's own public
    /// surface rather than cited only from a scratch script).
    ///
    /// `process_group(0)` (std, stable, no `unsafe`) is what makes this
    /// child's own pgid equal its pid - the same thing `portable-pty`'s own
    /// `pre_exec` gets from calling `setsid()` (§0), and the precondition
    /// `send`'s own doc states outright. Without it, `sh` here would
    /// inherit *this test binary's* pgid, `killpg(pid, ...)` would name a
    /// process group nothing belongs to, and the signal would reach
    /// nothing - caught by running this test with that call removed: it
    /// fails exactly the way the first TDD round's stub did, for a
    /// different reason (not "two different bugs with the same symptom"
    /// glossed over, but the real cause found and named, not guessed).
    #[test]
    fn send_reaches_a_real_process_with_the_named_signal() {
        let child = std::process::Command::new("sh")
            .args(["-c", "exec sleep 1000"])
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sh is on PATH on every platform this runs on");
        let pid = child.id();
        let _guard = KillOnDrop(pid);

        send(pid, Signal::SIGHUP);

        let status =
            wait_within(child, Duration::from_secs(5)).expect("the signalled process must exit");
        assert!(
            !status.success(),
            "a program killed by a signal did not exit 0"
        );
    }

    /// The group, not only the pid, is what is reached: a backgrounded
    /// child of the same shell (sharing its pgid, since nothing here calls
    /// `setpgid`) dies too, proving `killpg` is really what runs, not a
    /// `kill` that happens to look similar.
    #[test]
    fn send_reaches_the_whole_process_group_not_only_the_leader() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "sleep 1000 & echo $! ; wait"])
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("sh is on PATH on every platform this runs on");
        let pid = child.id();
        let _guard = KillOnDrop(pid); // signals the whole group, so this alone covers the background pid too

        let mut stdout = std::io::BufRead::lines(std::io::BufReader::new(
            child.stdout.take().expect("stdout was piped"),
        ));
        let background_pid: u32 = stdout
            .next()
            .expect("the shell's own echo of $!")
            .expect("a readable line")
            .trim()
            .parse()
            .expect("a pid is a plain number");

        send(pid, Signal::SIGKILL);

        let status =
            wait_within(child, Duration::from_secs(5)).expect("the signalled shell must exit");
        assert!(!status.success());
        assert!(
            !process_exists(background_pid),
            "the backgrounded child, in the same process group, must be gone too"
        );
    }

    /// Whether a process with this id still exists at all (reaped or not)
    /// — the same `kill -0` probe `crate::pane`'s own tests already use.
    fn process_exists(pid: u32) -> bool {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .expect("kill(1) is on PATH on every platform this runs on")
            .success()
    }
}
