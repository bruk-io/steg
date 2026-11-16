//! Sets up the one precondition `server_start.rs`'s own
//! `typing_on_demand_at_a_shell_prompt_carries_on_attached_with_a_message`
//! test needs: a process that is *itself* already a process-group leader
//! before it ever calls `steg_server::detach()` for real, the same state a
//! job-control shell's own `setpgid` puts a directly-typed `steg server
//! start --on-demand` into. No `unsafe`, no new dependency: reuses
//! `steg_server::detach()` itself (rather than calling `nix::unistd::setsid`
//! a second time) to become the leader, then replaces its own image with
//! the real `steg` binary via `std::os::unix::process::CommandExt::exec` -
//! safe, a direct `execve` with no fork, so there is nothing half-forked for
//! the compiler's unsafety rules to worry about.
//!
//! The flags after `server start` are this fixture's own argv (passed
//! through unchanged), defaulting to `--on-demand` so the original test
//! above needs no change - the same precondition also proves, with
//! different flags, that `server start --no-on-demand` never attempts to
//! detach at all (TASK-1.49's review finding: one parser for every
//! `--on-demand`/`--no-on-demand` combination, doc-2 section 5's "last one
//! wins", not a second hand-written grammar).

use std::os::unix::process::CommandExt;

fn main() {
    steg_server::detach().expect("the fixture must become its own leader to set up the case");
    // `CARGO_BIN_EXE_steg` only resolves via `env!` inside a `[[test]]`
    // target, not a `[[bin]]` one like this fixture - `server_start.rs`
    // (where it does resolve) passes it along as a plain environment
    // variable instead, test-only plumbing between two test artifacts,
    // not a hook in the shipped binary (AC #5/#6).
    let steg_binary =
        std::env::var("STEG_TEST_BINARY").expect("the test should set STEG_TEST_BINARY");
    let given_args: Vec<String> = std::env::args().skip(1).collect();
    let flags = if given_args.is_empty() {
        vec!["--on-demand".to_owned()]
    } else {
        given_args
    };
    let err = std::process::Command::new(steg_binary)
        .args(["server", "start"])
        .args(&flags)
        .exec();
    // `exec` returns an `io::Error` only on failure - on success this
    // process's image is already gone, so this line is unreachable on the
    // success path, not a missing branch.
    panic!("exec failed: {err}");
}
