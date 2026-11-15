//! AC #3 ("a client and server whose protocol versions differ get a
//! message naming both versions") and AC #4 ("a connection that never
//! completes its handshake is closed after a timeout; one that has
//! completed it is not closed for being idle"), proved against this
//! task's own `serve`, in the foreground, in this test's own process -
//! reusing `steg_sdk::wire::Connection` exactly as TASK-1.70 built it.
//! Nothing here adds a line to the sdk; this proves the *server*
//! delivers its decisions over a real socket, not that the decisions
//! themselves are right, which is already the sdk's own proof.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use steg_sdk::{
    ClientHello, ClientKind, Decoder, Frame, PROTOCOL_VERSION, encode_frame, parse_server_hello,
};
use steg_server::{ConnectionEvent, ServerConfig, Stopper, serve, stop_and_wake};

/// Reads one whole frame's payload off `stream`, looping reads until the
/// sdk's own `Decoder` has a complete frame - the length-prefixed
/// framing is self-delimiting but not guaranteed to arrive in one
/// `read()` call, so this does not assume it does.
fn read_one_frame_payload(stream: &mut UnixStream) -> Vec<u8> {
    let mut decoder = Decoder::new();
    loop {
        if let Some(frame) = decoder
            .next_frame()
            .expect("a well-formed frame from the server")
        {
            return frame.payload;
        }
        let mut buf = [0_u8; 4096];
        let n = stream
            .read(&mut buf)
            .expect("reading the server's reply should not fail");
        assert!(n > 0, "the server closed before a complete frame arrived");
        decoder.push(&buf[..n]);
    }
}

/// Kept short against `sockaddr_un`'s 104-byte `sun_path` budget
/// (`crates/server/src/bind.rs`'s own doc comment found this matters).
fn unique_socket_path() -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}-{suffix}.sock", std::process::id()))
}

/// Starts `serve` on a background thread with a short, injected
/// handshake timeout, and waits (on the socket actually accepting, not a
/// sleep) until it is up. Returns the socket path, the `Stopper` to end
/// it with, and the thread's join handle.
fn start_server(handshake_timeout: Duration) -> (PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let socket_path = unique_socket_path();
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout,
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let handle = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, None);
        })
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if UnixStream::connect(&socket_path).is_ok() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "server never started accepting at {socket_path:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    (socket_path, stop, handle)
}

fn stop_server(socket_path: &std::path::Path, stop: &Stopper, handle: std::thread::JoinHandle<()>) {
    stop_and_wake(stop, socket_path);
    handle.join().expect("the server thread should not panic");
}

#[test]
fn a_version_mismatch_names_both_versions_over_a_real_connection() {
    let (socket_path, stop, handle) = start_server(Duration::from_secs(5));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let wrong_protocol = PROTOCOL_VERSION + 1;
    let hello = ClientHello::new("test-client", wrong_protocol, ClientKind::Cli, None);
    let payload = serde_json::to_vec(&hello).unwrap();
    let bytes = encode_frame(&Frame::hello(payload)).unwrap();
    stream.write_all(&bytes).unwrap();

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    let err = parse_server_hello(&frame_payload).unwrap_err();
    assert!(
        err.contains(&wrong_protocol.to_string()),
        "message should name the client's version {wrong_protocol}: {err}"
    );
    assert!(
        err.contains(&PROTOCOL_VERSION.to_string()),
        "message should name the server's version {PROTOCOL_VERSION}: {err}"
    );

    stop_server(&socket_path, &stop, handle);
}

#[test]
fn a_connection_that_never_sends_a_hello_is_closed_after_the_timeout() {
    let (socket_path, stop, handle) = start_server(Duration::from_millis(100));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    // Sends nothing. The server's own timeout is 100ms; this read waits
    // a bound comfortably over it for the close to arrive - waiting on
    // the event (the close), not sleeping first.
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buf = [0_u8; 16];
    let result = stream.read(&mut buf);
    match result {
        Ok(0) => {} // a clean close
        Ok(n) => panic!("expected the connection to close with nothing sent, got {n} bytes"),
        Err(err) => {
            // A reset is also a legitimate close, depending on how the
            // peer's own close races with this read.
            assert_ne!(
                err.kind(),
                std::io::ErrorKind::WouldBlock,
                "the connection should have closed well within the 5s bound, not still be open"
            );
        }
    }

    stop_server(&socket_path, &stop, handle);
}

#[test]
fn a_completed_handshake_is_not_closed_for_sitting_idle_past_the_timeout() {
    let handshake_timeout = Duration::from_millis(100);
    let (socket_path, stop, handle) = start_server(handshake_timeout);

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let hello = ClientHello::new("test-client", PROTOCOL_VERSION, ClientKind::Cli, None);
    let payload = serde_json::to_vec(&hello).unwrap();
    let bytes = encode_frame(&Frame::hello(payload)).unwrap();
    stream.write_all(&bytes).unwrap();

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    parse_server_hello(&frame_payload).expect("a matching protocol version should be accepted");

    // Idle for well past the handshake timeout, then prove the
    // connection is still open: a short read times out (`WouldBlock`),
    // rather than observing a close - the one case this task names as
    // the exception to "no sleeps as synchronisation" (a negative
    // proof has no event to wait on).
    std::thread::sleep(handshake_timeout * 5);
    stream
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let mut buf = [0_u8; 16];
    let result = stream.read(&mut buf);
    match result {
        Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
        Err(err) => {
            panic!("expected the read to time out (still open), got a different error: {err}")
        }
        Ok(n) => panic!("expected no bytes and no close, got {n} bytes"),
    }

    stop_server(&socket_path, &stop, handle);
}

/// A connection that finishes its handshake counts as a client the
/// instant `accept()` returns it, before any byte is read
/// (`connection::handle`'s own doc). Proved here with a real channel
/// rather than asserted: `ConnectionEvent::Connected` arrives before
/// the client has sent anything at all.
#[test]
fn a_connection_is_reported_connected_before_it_sends_its_hello() {
    let socket_path = unique_socket_path();
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        ..ServerConfig::default()
    };
    let stop = Stopper::new();
    let (tx, rx) = mpsc::channel::<ConnectionEvent>();
    let handle = {
        let config = config.clone();
        let stop = stop.clone();
        std::thread::spawn(move || {
            let _ = serve(&config, &stop, Some(tx));
        })
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if UnixStream::connect(&socket_path).is_ok() {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "server never started");
        std::thread::sleep(Duration::from_millis(5));
    }

    let _stream = UnixStream::connect(&socket_path).unwrap(); // sends nothing
    let event = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("Connected should arrive");
    assert_eq!(event, ConnectionEvent::Connected);

    stop_server(&socket_path, &stop, handle);
}
