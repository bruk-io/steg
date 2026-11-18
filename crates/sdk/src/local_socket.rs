//! The local-socket provider (R103): finds a running server, or with none
//! running starts one and waits for its socket, with an option not to
//! start one. The one place in this crate allowed to do real I/O
//! (`.claude/rules/sdk.md`) - std only, blocking, no dependency.
//!
//! What this hands TASK-1.44: the spawn step always runs exactly `<binary>
//! server start --on-demand` (doc-2's own `server start` command, widened
//! with the `on_demand` switch `description::methods` now declares - see
//! that method's own `details` for why `server.start`/`.stop`/`.show`
//! cannot themselves be how a process with nothing to connect to becomes
//! one). What `server start --on-demand` does once spawned is not this
//! module's: try to connect first, and become the server - bind, detach,
//! serve, with R99's on-demand lifecycle - only on failure, never
//! recursing into this module's own autostart path.
//!
//! The lock half of "the socket's path and lock" is explicitly not this
//! module's either: it only spawns and waits for the socket to accept a
//! connection. Resolving two simultaneous starts to one survivor (R99) is
//! invisible here - this module only ever sees "the socket now accepts,
//! or it still doesn't."

#![expect(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "R103: the local-socket provider is the sdk's one permitted exception to no \
              I/O of its own (doc-16 R102/R110) - it finds or starts a local server using \
              std's process and Unix-socket APIs directly, blocking, with its own retry \
              deadline. `.claude/rules/sdk.md` scopes this expectation to this module alone."
)]

use std::env;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::client_kind::ClientKind;
use crate::paths::socket_path;
use crate::wire::{Connection, Decoder, Kind};

/// How to reach, or start, the local server.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// What to run to become a server when none is found. No default
    /// anywhere in this crate - the CLI (`steg` package) sets this to
    /// `std::env::current_exe()`, correct only because the server and the
    /// CLI ship as one binary (R61/R98); the app is a separate program
    /// (doc-16 level 2) and must supply wherever it finds its own copy,
    /// which this crate does not decide.
    pub binary: PathBuf,
    /// `None` means "compute it from the real environment"
    /// (`XDG_RUNTIME_DIR`/`TMPDIR`/`STEG_SOCKET_PATH`/`USER`, read here).
    /// `Some(path)` is an explicit override - a test's temp directory, or
    /// later a person's own debugging - used directly, and set as
    /// `STEG_SOCKET_PATH` on a spawned child's environment so it resolves
    /// the identical path through the same `paths::socket_path` call.
    pub socket_path: Option<PathBuf>,
    /// Start a server when none is found. `false` is what doc-2's
    /// `--no-autostart` becomes: [`ConnectError::NoServerRunning`], which
    /// a consumer (TASK-1.49) maps to exit status 4.
    pub autostart: bool,
    /// How long to wait for a freshly spawned server's socket to answer
    /// before giving up with [`ConnectError::Timeout`].
    pub connect_timeout: Duration,
}

/// Why [`connect`] could not return a connected socket, or why
/// [`connect_handshaken`] could not return a connected, handshaken one.
#[derive(Debug)]
pub enum ConnectError {
    /// Nothing was listening, and `autostart` was `false`.
    NoServerRunning,
    /// `binary` could not be spawned at all.
    Spawn(std::io::Error),
    /// A server was spawned but its socket never answered within
    /// `connect_timeout`.
    Timeout,
    /// [`connect_handshaken`] only: the connection was accepted and then
    /// closed with nothing written, with no complete hello ever decoded -
    /// twice in a row (TASK-1.119 AC #4). The first time this happens,
    /// [`connect_handshaken`] retries the whole arbitration (a fresh
    /// `connect()`) once on its own and this variant is never seen by a
    /// caller; it is returned only once that retry also ends this way.
    ClosedBeforeHello,
    /// [`connect_handshaken`] only: writing the hello, or reading for its
    /// reply, failed outright (not a clean EOF - see
    /// [`ConnectError::ClosedBeforeHello`] for that) once a connection was
    /// already in hand.
    Io(std::io::Error),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoServerRunning => write!(f, "no steg server is running"),
            Self::Spawn(err) => write!(f, "could not start a steg server: {err}"),
            Self::Timeout => write!(f, "timed out waiting for the steg server's socket"),
            Self::ClosedBeforeHello => write!(
                f,
                "the steg server accepted the connection and closed it before a hello arrived, twice"
            ),
            Self::Io(err) => write!(f, "talking to the steg server failed: {err}"),
        }
    }
}

impl std::error::Error for ConnectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoServerRunning | Self::Timeout | Self::ClosedBeforeHello => None,
            Self::Spawn(err) | Self::Io(err) => Some(err),
        }
    }
}

fn resolve_path(options: &ConnectOptions) -> PathBuf {
    if let Some(path) = &options.socket_path {
        return path.clone();
    }
    let explicit = env::var_os("STEG_SOCKET_PATH").map(PathBuf::from);
    let xdg_runtime_dir = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let tmp_dir = env::var_os("TMPDIR").map(PathBuf::from);
    let user = env::var("USER").ok();
    socket_path(
        explicit.as_deref(),
        xdg_runtime_dir.as_deref(),
        tmp_dir.as_deref(),
        user.as_deref(),
    )
}

/// Connects to a running server, or - with `autostart` and none running -
/// spawns one and waits for its socket.
///
/// This never waits on the spawned process (below): a bad `binary` - one
/// that does not exist, or exists but cannot be executed - is reported as
/// [`ConnectError::Spawn`] as soon as the spawn itself fails, without
/// waiting out `connect_timeout`. A consumer that calls this with
/// `autostart` many times over a long life (the app, reconnecting) can
/// accumulate a process-table entry per call until it itself exits, for
/// as long as the real server takes to detach and exit the process this
/// spawned - entirely TASK-1.44's to keep short, not a cost this function
/// can avoid by waiting, which R99 forbids it from doing.
///
/// # Errors
/// [`ConnectError::NoServerRunning`] when nothing is listening and
/// `autostart` is `false`; [`ConnectError::Spawn`] when `binary` could not
/// be run at all; [`ConnectError::Timeout`] when a spawned server's socket
/// never answers within `connect_timeout`.
pub fn connect(options: &ConnectOptions) -> Result<UnixStream, ConnectError> {
    let path = resolve_path(options);

    if let Ok(stream) = UnixStream::connect(&path) {
        return Ok(stream);
    }

    if !options.autostart {
        return Err(ConnectError::NoServerRunning);
    }

    let mut command = Command::new(&options.binary);
    command
        .args(["server", "start", "--on-demand"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(socket_path) = &options.socket_path {
        command.env("STEG_SOCKET_PATH", socket_path);
    }
    // R99's detaching, re-parenting or double-fork, and stopping are the
    // server's own code; this drops the `Child` handle once spawn
    // succeeds and never wait()s on it. `spawn()` itself still fails
    // synchronously when `binary` cannot be exec'd at all (missing, or
    // not executable) - the OS reports that before this call returns, so
    // `?` here returns `ConnectError::Spawn` immediately, before the
    // retry loop and its deadline below are ever reached.
    command.spawn().map_err(ConnectError::Spawn)?;

    let deadline = Instant::now() + options.connect_timeout;
    loop {
        if let Ok(stream) = UnixStream::connect(&path) {
            return Ok(stream);
        }
        if Instant::now() >= deadline {
            return Err(ConnectError::Timeout);
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// [`connect`], then the handshake's own first half: sends the hello and
/// reads for its reply (TASK-1.119 AC #4). Returns the raw hello-reply
/// payload, undecoded - this function's job is the connection and the
/// retry below, not interpreting the hello, which stays the caller's,
/// exactly as it is for a caller of [`connect`] today (`parse_server_hello`
/// is a plain function over bytes, with nothing of this module's own
/// state).
///
/// **A connection accepted and then closed with nothing written, before
/// any complete hello is decoded, is treated identically to "nothing was
/// listening": the whole arbitration above - a fresh [`connect`], which
/// itself autostarts again if nothing answers a direct connect - is
/// retried exactly once.** This is the sdk's own fix for a real race
/// (TASK-1.77's own finding): a connection the accept loop takes in the
/// instant it sees a stop flag is dropped with no reply, so the client
/// sees connect succeed and then end of file - a third outcome beside
/// refused and not found, and one `connect()` alone cannot tell apart
/// from a server that is simply slow to speak. A second such closure, on
/// the retry, is not retried again: [`ConnectError::ClosedBeforeHello`] is
/// returned, bounded by this function's own one retry rather than an open
/// loop.
///
/// `client`/`kind`/`label` are [`Connection::send_hello`]'s own parameters,
/// passed straight through.
///
/// # Errors
/// Anything [`connect`] itself can return; [`ConnectError::ClosedBeforeHello`]
/// when the connection closed with no hello twice in a row;
/// [`ConnectError::Io`] when writing the hello or reading for its reply
/// failed outright, once a connection was already in hand.
pub fn connect_handshaken(
    options: &ConnectOptions,
    client: &str,
    kind: ClientKind,
    label: Option<&str>,
) -> Result<(UnixStream, Vec<u8>), ConnectError> {
    match one_handshake_attempt(options, client, kind, label)? {
        Some(result) => Ok(result),
        None => match one_handshake_attempt(options, client, kind, label)? {
            Some(result) => Ok(result),
            None => Err(ConnectError::ClosedBeforeHello),
        },
    }
}

/// One `connect()` plus one hello round trip. `Ok(None)` is the
/// EOF-before-hello condition [`connect_handshaken`] retries once; every
/// other outcome (success, or a real error) is returned as is and ends
/// the whole call.
fn one_handshake_attempt(
    options: &ConnectOptions,
    client: &str,
    kind: ClientKind,
    label: Option<&str>,
) -> Result<Option<(UnixStream, Vec<u8>)>, ConnectError> {
    let mut stream = connect(options)?;
    let hello = Connection::send_hello(client, kind, label);

    // From here on, every operation is on a stream this function just
    // connected itself, with a fixed, well-formed buffer and a valid
    // timeout - there is no genuine bug this code could have that would
    // make any of `write_all`/`set_read_timeout`/`read` fail; the only
    // real-world cause is a connection closed the instant it was
    // accepted (TASK-1.77's own race). Found by this module's own test,
    // not assumed: which precise errno the OS reports for that depends
    // on exactly when the peer's close is noticed relative to which
    // call, which neither side controls - a broken pipe, "not
    // connected", and even "invalid argument" (`set_read_timeout` on a
    // socket whose peer has already gone, observed directly on this
    // platform under enough concurrent load to land in a narrower
    // window) have all been seen for the identical fixture. So every
    // error below but the read's own explicit timeout is read the same
    // way: `Ok(None)`, the same EOF-before-hello signal as a zero-byte
    // read.
    if stream.write_all(&hello).is_err() {
        return Ok(None);
    }

    // A timeout on this read is a bound, not polling (the same reasoning
    // `crates/app/src/startup.rs`'s own module doc gives for its
    // equivalent read): a server that is there but silent for longer than
    // `connect_timeout` is indistinguishable, for this function's
    // purposes, from one that never answers at all.
    if stream
        .set_read_timeout(Some(options.connect_timeout))
        .is_err()
    {
        return Ok(None);
    }

    let mut decoder = Decoder::new();
    let mut buf = [0_u8; 65536];
    loop {
        match decoder.next_frame() {
            Ok(Some(frame)) if frame.kind == Kind::Hello => {
                if stream.set_read_timeout(None).is_err() {
                    return Ok(None);
                }
                return Ok(Some((stream, frame.payload)));
            }
            // A frame that is not the hello, arriving before it ever
            // could in a correct server - ignored rather than treated as
            // fatal, since this function only cares about the hello.
            Ok(Some(_)) => continue,
            Ok(None) => {} // need more bytes
            // Malformed bytes arriving before any complete hello - the
            // same treatment as every other failure in this window.
            Err(_) => return Ok(None),
        }
        match stream.read(&mut buf) {
            Ok(0) => return Ok(None), // EOF before any complete hello
            Ok(n) => decoder.push(&buf[..n]),
            // The read's own, explicit way of saying "nothing arrived
            // within `connect_timeout`" - the one outcome in this whole
            // function that is not itself a sign the peer is already
            // gone, so the one error this function still reports as a
            // real `ConnectError` rather than folding into a retry.
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(ConnectError::Timeout);
            }
            Err(_) => return Ok(None),
        }
    }
}

/// Test support only: the whole of `tests/fixtures/fake_server.rs`'s
/// `main` is one call into this. Lives here, under this module's own
/// `#![expect]`, deliberately - a `[[bin]]` fixture needs real I/O to
/// stand in for "a server now accepts there" at all, and
/// `.claude/rules/sdk.md` is explicit that nowhere else in the crate
/// gets this expectation, so the fixture's own file calls this instead
/// of carrying a second one.
///
/// Resolves the socket path exactly as [`connect`]'s spawn step expects a
/// real server to: from its own inherited environment, via the same
/// `paths::socket_path`, with no path ever passed on its argv (which this
/// function, and a real `steg server start --on-demand`, both ignore).
///
/// # Panics
/// If the socket cannot be bound - there is nothing a test fixture can
/// usefully do instead.
#[doc(hidden)]
pub fn run_fake_server_for_tests() {
    let explicit = env::var_os("STEG_SOCKET_PATH").map(PathBuf::from);
    let xdg_runtime_dir = env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let tmp_dir = env::var_os("TMPDIR").map(PathBuf::from);
    let user = env::var("USER").ok();
    let path = socket_path(
        explicit.as_deref(),
        xdg_runtime_dir.as_deref(),
        tmp_dir.as_deref(),
        user.as_deref(),
    );
    let listener = UnixListener::bind(&path).expect("fake-server-for-tests: bind failed");
    thread::sleep(Duration::from_secs(30));
    drop(listener);
}

// AC #3's own tests live in `tests/local_socket.rs`, not here: they need
// the `fake-server-for-tests` fixture binary, and `CARGO_BIN_EXE_<name>`
// is only set for an integration test, not for this module's own
// `#[cfg(test)]` (a lib unit test binary) - found while building this,
// not assumed.
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc;

    use super::{ConnectError, ConnectOptions, connect_handshaken};
    use crate::client_kind::ClientKind;
    use crate::ids::ClientId;
    use crate::wire::{Connection, Decoder, Kind};

    #[test]
    fn display_names_the_binary_error_and_the_timeout() {
        assert!(format!("{}", ConnectError::NoServerRunning).contains("no steg server"));
        assert!(format!("{}", ConnectError::Timeout).contains("timed out"));
        let spawn_err = ConnectError::Spawn(std::io::Error::other("boom"));
        assert!(format!("{spawn_err}").contains("boom"));
        assert!(format!("{}", ConnectError::ClosedBeforeHello).contains("twice"));
        let io_err = ConnectError::Io(std::io::Error::other("severed"));
        assert!(format!("{io_err}").contains("severed"));
    }

    fn unique_test_socket_path(label: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "{}-connect-handshaken-{label}-{suffix}.sock",
            std::process::id()
        ))
    }

    /// Reads off `stream` until a complete hello frame is decoded - the
    /// server-side counterpart to [`one_handshake_attempt`]'s own client
    /// -side read, written out again here rather than shared, since this
    /// is test-only and the two sides read for different things.
    fn read_one_hello(stream: &mut std::os::unix::net::UnixStream) -> Vec<u8> {
        let mut decoder = Decoder::new();
        let mut buf = [0_u8; 4096];
        loop {
            match decoder.next_frame() {
                Ok(Some(frame)) if frame.kind == Kind::Hello => return frame.payload,
                Ok(Some(_)) => continue,
                Ok(None) => {}
                Err(err) => panic!("malformed hello: {err}"),
            }
            let n = stream.read(&mut buf).expect("reading the client's hello");
            assert_ne!(n, 0, "the client closed before sending a hello");
            decoder.push(&buf[..n]);
        }
    }

    /// Binds a real `UnixListener` at `path` on its own thread and accepts
    /// exactly `behaviours.len()` connections, one behaviour per accept -
    /// `true` reads the client's real hello and answers with a real
    /// server one (`Connection::accept_or_refuse`); `false` drops the
    /// connection immediately with nothing written at all, reproducing
    /// TASK-1.77's own race. Returns the shared accept counter (every
    /// increment happens strictly before that accept's own behaviour runs,
    /// so by the time `connect_handshaken` returns - synchronously, after
    /// observing either a reply or an EOF for its *last* attempt - every
    /// accept that led to that outcome has already been counted: no wait
    /// is needed after the call returns) and a receiver that fires once
    /// the bind has completed, so a test can synchronise on "the listener
    /// is ready" without spending one of its own accepts on a throwaway
    /// probe connection.
    fn spawn_listener(
        path: std::path::PathBuf,
        behaviours: Vec<bool>,
    ) -> (Arc<AtomicU32>, mpsc::Receiver<()>) {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_for_thread = Arc::clone(&counter);
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let listener = UnixListener::bind(&path).expect("binding the test listener");
            ready_tx.send(()).expect("the test is still waiting");
            for cooperative in behaviours {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                counter_for_thread.fetch_add(1, Ordering::SeqCst);
                if cooperative {
                    let payload = read_one_hello(&mut stream);
                    let (_, bytes) = Connection::accept_or_refuse(
                        &payload,
                        "steg-test-server",
                        "1.0",
                        ClientId::from_raw(1),
                    );
                    let _ = stream.write_all(&bytes);
                    // Held open briefly so the client's own read of this
                    // reply never itself races this thread's own teardown
                    // of the connection.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                // `cooperative == false`: `stream` drops here, closing the
                // connection with nothing ever written to it.
            }
        });
        (counter, ready_rx)
    }

    fn options_for(path: std::path::PathBuf) -> ConnectOptions {
        ConnectOptions {
            // Never spawned: `autostart: false` below means `connect()`
            // never reaches for this, and a wrong path here would be a
            // loud failure (`Spawn`), not a silent one, if that ever
            // stopped being true.
            binary: std::path::PathBuf::from("/steg-test-should-never-spawn-this"),
            socket_path: Some(path),
            autostart: false,
            connect_timeout: std::time::Duration::from_secs(2),
        }
    }

    /// The sharpest part of AC #4: a connection closed with no hello,
    /// twice in a row, is retried *exactly* once - not zero times (it
    /// would fail on the first EOF with nothing to show for the retry)
    /// and not two or more times (which this counter would also catch).
    #[test]
    fn eof_before_hello_is_retried_exactly_once_then_fails() {
        let path = unique_test_socket_path("mode-a");
        let (counter, ready) = spawn_listener(path.clone(), vec![false, false]);
        ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("the test listener should bind within the bound");

        let result = connect_handshaken(
            &options_for(path.clone()),
            "steg-test",
            ClientKind::Cli,
            None,
        );

        assert!(
            matches!(result, Err(ConnectError::ClosedBeforeHello)),
            "{result:?}"
        );
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "exactly two connection attempts: the first EOF, then one retry - not one, not three or more"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The other half of "exactly once": a first attempt that closes with
    /// no hello and a second that completes a real handshake succeeds,
    /// having made exactly two attempts, not given up after the first.
    #[test]
    fn a_hello_completed_on_the_retry_succeeds_after_exactly_two_attempts() {
        let path = unique_test_socket_path("mode-b");
        let (counter, ready) = spawn_listener(path.clone(), vec![false, true]);
        ready
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("the test listener should bind within the bound");

        let result = connect_handshaken(
            &options_for(path.clone()),
            "steg-test",
            ClientKind::Cli,
            None,
        );

        let (stream, payload) = result.expect("the retry's own hello should succeed");
        let server_hello =
            crate::wire::parse_server_hello(&payload).expect("a real server hello was decoded");
        assert_eq!(server_hello.client, ClientId::from_raw(1));
        assert_eq!(
            counter.load(Ordering::SeqCst),
            2,
            "exactly two connection attempts: the first EOF, then the retry that succeeded"
        );
        drop(stream);
        let _ = std::fs::remove_file(&path);
    }
}
