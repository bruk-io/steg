//! `steg server show`/`steg server stop` (TASK-1.88), through the real
//! binary end to end - the one cheap, real-process proof the task's own
//! orchestrator brief asks for, alongside the much more thorough
//! socket-level coverage in `crates/server/tests/server_stop.rs`. Proves
//! the *wiring* (the real CLI -> a real socket -> the real dispatch arms),
//! not the stop mechanism's own semantics a second time.
//!
//! Every helper below is duplicated from `server_start.rs`/
//! `cli_exit_statuses.rs` rather than shared, the convention every file in
//! this crate's `tests/` already follows.

use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

#[path = "support/mod.rs"]
mod support;
use support::scoped_socket_path;

fn steg_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_steg"))
}

/// Kills, on drop, whatever process `lsof` finds holding `socket_path`'s
/// lock open (`server_start.rs`'s own `ServerAtSocket`) - a safety net
/// for a failed assertion that leaves this test's own on-demand server
/// still running, since it detaches and this file never holds a `Child`
/// for it directly.
struct ServerAtSocket(PathBuf);

impl Drop for ServerAtSocket {
    fn drop(&mut self) {
        if let Some(pid) = find_server_pid(&self.0) {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

fn find_server_pid(socket_path: &std::path::Path) -> Option<u32> {
    let mut lock_path = socket_path.as_os_str().to_owned();
    lock_path.push(".lock");
    let output = Command::new("lsof")
        .arg("-t")
        .arg(&lock_path)
        .output()
        .expect("lsof should be runnable on this machine");
    let text = String::from_utf8_lossy(&output.stdout);
    let pids: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    match pids.as_slice() {
        [] => None,
        [one] => Some(
            one.parse()
                .unwrap_or_else(|err| panic!("lsof reported a non-numeric pid {one:?}: {err}")),
        ),
        many => panic!("expected at most one pid holding {lock_path:?}, got {many:?}"),
    }
}

fn wait_for_socket(path: &std::path::Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if UnixStream::connect(path).is_ok() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "nothing ever answered at {path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_bounded(child: &mut Child, timeout: Duration) -> std::process::ExitStatus {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait should not fail") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the process did not exit within the bound"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn spawn_steg_server_start_on_demand(socket_path: &std::path::Path) -> Child {
    Command::new(steg_binary())
        .args(["server", "start", "--on-demand"])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SOCKET_PATH", socket_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawning the real steg binary")
}

/// Runs the real binary to completion against `socket` - a minimal,
/// explicit environment (SAFETY: this machine holds real credentials;
/// this test's own process environment is never handed to a child).
fn run_steg(args: &[&str], socket: &std::path::Path) -> Output {
    Command::new(steg_binary())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("STEG_SOCKET_PATH", socket)
        .output()
        .expect("the real steg binary should run to completion")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `steg server show` reports real data, and `steg server stop` actually
/// ends the server, both carried end to end by the CLI's own generic
/// routing (`connect.rs::run`) through the real binary - never a
/// hand-written case for either, unlike `server start`.
#[test]
fn server_show_and_server_stop_work_through_the_real_binary() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
    // The process that *becomes* the server (there is only one racer
    // here, so it always does) never exits on its own: `main.rs`'s own
    // `run_server_start` detaches it from the controlling terminal
    // (`setsid`) and then runs `listen::run` in that same process, which
    // only returns once something stops it - this `Child` *is* the real
    // server process, not a launcher that hands off to a separate one.
    let mut starter = spawn_steg_server_start_on_demand(&socket_path);
    let _cleanup = ServerAtSocket(socket_path.clone());
    wait_for_socket(&socket_path, Duration::from_secs(5));

    let show = run_steg(&["server", "show", "--json"], &socket_path);
    assert_eq!(show.status.code(), Some(0), "{show:?}");
    let value: serde_json::Value =
        serde_json::from_str(stdout_of(&show).trim()).expect("server show --json prints JSON");
    assert!(
        value.get("version").is_some_and(|v| v.is_string()),
        "{value}"
    );
    assert!(value.get("uptime").is_some_and(|v| v.is_u64()), "{value}");
    assert_eq!(value["sessions"], serde_json::json!(0));
    // At least this very call's own connection counts as a client - not
    // asserted as exactly 1, since nothing here rules out another
    // process on this machine also having one open at the same instant.
    assert!(
        value["clients"]
            .as_u64()
            .is_some_and(|clients| clients >= 1),
        "{value}"
    );

    let stop = run_steg(&["server", "stop"], &socket_path);
    assert_eq!(stop.status.code(), Some(0), "{stop:?}");

    // The real server process - `starter`, not a separate detached one -
    // actually returns from `run()` and exits, bounded: the direct proof
    // that `server.stop` reached the real process, not only that this
    // one connection stopped seeing it.
    let status = wait_bounded(&mut starter, Duration::from_secs(10));
    assert_eq!(status.code(), Some(0), "{status:?}");

    // The server itself is now gone - a further call with no autostart
    // finds nothing to talk to (doc-2 section 6's status 4).
    let show_after = run_steg(&["server", "show", "--no-autostart"], &socket_path);
    assert_eq!(show_after.status.code(), Some(4), "{show_after:?}");
}
