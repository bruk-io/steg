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
use std::sync::mpsc;
use std::time::{Duration, Instant};

use steg_sdk::{
    CONTROL_STREAM, ClientHello, ClientKind, Control, Decoder, Frame, PROTOCOL_VERSION,
    encode_frame, parse_server_hello,
};
use steg_server::{ConnectionEvent, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, run_bounded, scoped_socket_path};

/// This file's own bound for its two bare joins (TASK-1.137): generous
/// against the trickler's own up-to-`36 * 40ms` write cadence plus the
/// handshake timeout it races against, without being open-ended.
const JOIN_DEADLINE: Duration = Duration::from_secs(10);

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

/// Starts `serve` on a background thread with a short, injected
/// handshake timeout, and waits (on the socket actually accepting, not a
/// sleep) until it is up. Returns the directory the socket lives in (kept
/// alive for as long as the caller needs the socket), the socket path,
/// the `Stopper` to end it with, and the thread's join handle.
fn start_server(
    handshake_timeout: Duration,
) -> (ScopedTempDir, PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let (socket_dir, socket_path) = scoped_socket_path("s.sock");
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
    (socket_dir, socket_path, stop, handle)
}

fn stop_server(stop: &Stopper, handle: std::thread::JoinHandle<()>) {
    stop.stop();
    run_bounded("the server thread to end after stop()", JOIN_DEADLINE, move || {
        handle.join().expect("the server thread should not panic");
    });
}

#[test]
fn a_version_mismatch_names_both_versions_over_a_real_connection() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_secs(5));

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

    stop_server(&stop, handle);
}

#[test]
fn a_connection_that_never_sends_a_hello_is_closed_after_the_timeout() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_millis(100));

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

    stop_server(&stop, handle);
}

#[test]
fn a_completed_handshake_is_not_closed_for_sitting_idle_past_the_timeout() {
    let handshake_timeout = Duration::from_millis(100);
    let (_socket_dir, socket_path, stop, handle) = start_server(handshake_timeout);

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

    stop_server(&stop, handle);
}

/// A connection that finishes its handshake counts as a client the
/// instant `accept()` returns it, before any byte is read
/// (`connection::handle`'s own doc). Proved here with a real channel
/// rather than asserted: `ConnectionEvent::Connected` arrives before
/// the client has sent anything at all.
#[test]
fn a_connection_is_reported_connected_before_it_sends_its_hello() {
    let (_socket_dir, socket_path) = scoped_socket_path("s.sock");
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

    stop_server(&stop, handle);
}

/// decision-2 section 7's own contract: a malformed-hello refusal names
/// the server's version and says the client's is unknown, regardless of
/// which of the four ways a first frame can fail to be a usable hello.
/// `parse_server_hello` (the sdk's own parser) reading the reply is
/// exactly what a real client does - this does not re-prove the
/// refusal's wording (the sdk's own `handshake.rs` tests already do),
/// only that the *server* delivers it for each of these cases over a
/// real connection.
fn assert_malformed_hello_refusal(frame_payload: &[u8]) {
    let err = parse_server_hello(frame_payload).expect_err("expected a refusal, not a ServerHello");
    assert!(
        err.contains(&PROTOCOL_VERSION.to_string()),
        "message should name the server's version {PROTOCOL_VERSION}: {err}"
    );
    assert!(
        err.to_lowercase().contains("unknown"),
        "message should say the client's version is unknown: {err}"
    );
}

/// Asserts `stream` closes shortly after a reply was already read off
/// it - every malformed-hello case closes the connection, it does not
/// hold it open the way an accepted one would.
fn assert_closes_after_the_reply(stream: &mut UnixStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("setting a read timeout should not fail");
    let mut buf = [0_u8; 16];
    match stream.read(&mut buf) {
        Ok(0) => {}
        Ok(n) => panic!("expected the connection to close, got {n} more bytes"),
        Err(err) => assert_ne!(
            err.kind(),
            std::io::ErrorKind::WouldBlock,
            "the connection should close well within the 5s bound"
        ),
    }
}

/// decision-2 section 7's first case: the wrong magic. Already decided
/// by the sdk's own `check_hello` (this server adds nothing for it) -
/// what this proves is that the server actually delivers that decision
/// over a real connection, which nothing in this task's own suite did
/// before this review.
#[test]
fn a_hello_with_the_wrong_magic_gets_a_malformed_hello_refusal() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_secs(5));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let payload = serde_json::to_vec(&serde_json::json!({
        "steg": "not-steg",
        "protocol": PROTOCOL_VERSION,
        "client": "test-client",
        "kind": "cli",
    }))
    .unwrap();
    let bytes = encode_frame(&Frame::hello(payload)).unwrap();
    stream.write_all(&bytes).unwrap();

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    assert_malformed_hello_refusal(&frame_payload);
    assert_closes_after_the_reply(&mut stream);

    stop_server(&stop, handle);
}

/// decision-2 section 7's second case: a hello missing `protocol`
/// entirely (as opposed to a mismatched one, already proved by
/// `a_version_mismatch_names_both_versions_over_a_real_connection`).
#[test]
fn a_hello_missing_the_protocol_field_gets_a_malformed_hello_refusal() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_secs(5));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let payload = serde_json::to_vec(&serde_json::json!({
        "steg": "steg",
        "client": "test-client",
        "kind": "cli",
    }))
    .unwrap();
    let bytes = encode_frame(&Frame::hello(payload)).unwrap();
    stream.write_all(&bytes).unwrap();

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    assert_malformed_hello_refusal(&frame_payload);
    assert_closes_after_the_reply(&mut stream);

    stop_server(&stop, handle);
}

/// decision-2 section 7's third case, the one this review found never
/// reached a reply at all: the first frame's own bytes do not decode as
/// a frame (here, an unknown `Kind` tag) - never even reaching the
/// sdk's `check_hello`, since there is no payload to hand it. Routed
/// through `connection::FirstFrame::NotAHello` to the same refusal
/// shape `check_hello` would have produced.
#[test]
fn undecodable_bytes_as_the_first_frame_get_a_malformed_hello_refusal() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_secs(5));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    // A well-formed length prefix and header, but an unknown kind tag
    // (99) - `Decoder::next_frame`'s own `an_unknown_kind_tag_is_malformed`
    // test pins this exact shape as a `FrameError::Malformed`.
    let mut bytes = 5_u32.to_be_bytes().to_vec(); // header-only body
    bytes.extend_from_slice(&CONTROL_STREAM.to_be_bytes());
    bytes.push(99); // not a known Kind tag
    stream.write_all(&bytes).unwrap();

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    assert_malformed_hello_refusal(&frame_payload);
    assert_closes_after_the_reply(&mut stream);

    stop_server(&stop, handle);
}

/// decision-2 section 7's fourth case: a well-formed frame that decodes
/// fine, but is a control frame (an event, here) rather than a hello -
/// the review's other previously-unreached branch. `Control::Event`'s
/// own payload decodes without error, so this is distinct from the
/// undecodable-bytes case above: it proves the "wrong kind" branch, not
/// a second way to hit "wrong framing".
#[test]
fn a_control_frame_arriving_first_gets_a_malformed_hello_refusal() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_secs(5));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let event = Control::Event {
        name: "not-a-hello".into(),
        body: serde_json::json!({}),
    };
    let bytes = encode_frame(&Frame::control(CONTROL_STREAM, event.to_payload().unwrap())).unwrap();
    stream.write_all(&bytes).unwrap();

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    assert_malformed_hello_refusal(&frame_payload);
    assert_closes_after_the_reply(&mut stream);

    stop_server(&stop, handle);
}

/// The review's own named case: a declared length over the sdk's own
/// `MAX_FRAME` is refused from only the 4-byte length prefix, with
/// nothing allocated for the declared body (`Decoder::next_frame`'s own
/// proof) - and, now, with a reply, delivered promptly rather than
/// waiting out the handshake timeout. The bound asserted here
/// (comfortably under the 5s handshake timeout the server was started
/// with) is what proves "promptly": a client sending only 4 bytes and
/// never completing a frame would otherwise have to wait for the
/// timeout, as the stalled-handshake test below does.
#[test]
fn an_oversized_declared_length_gets_a_malformed_hello_refusal_promptly() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_secs(5));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    stream.write_all(&0xFFFF_FFFF_u32.to_be_bytes()).unwrap();

    let started = Instant::now();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    assert_malformed_hello_refusal(&frame_payload);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "an oversized length should be refused promptly, not waiting on the 5s handshake timeout, took {:?}",
        started.elapsed()
    );
    assert_closes_after_the_reply(&mut stream);

    stop_server(&stop, handle);
}

/// Distinguishes a stalled handshake from the cases above: a client
/// that sends only part of a frame (a complete, valid length prefix
/// declaring more bytes than it ever sends) and then stops is
/// indistinguishable, at the framing level, from a client that sent
/// nothing at all - `Decoder::next_frame` returns "need more bytes" for
/// both, so this is closed at the handshake timeout with **no reply**,
/// the same as `a_connection_that_never_sends_a_hello_is_closed_after_the_timeout`,
/// not a malformed-hello refusal (there was nothing decoded yet to
/// refuse).
#[test]
fn a_client_that_sends_half_a_frame_and_then_stalls_is_closed_at_the_timeout_with_no_reply() {
    let (_socket_dir, socket_path, stop, handle) = start_server(Duration::from_millis(100));

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let hello = ClientHello::new("test-client", PROTOCOL_VERSION, ClientKind::Cli, None);
    let payload = serde_json::to_vec(&hello).unwrap();
    let full = encode_frame(&Frame::hello(payload)).unwrap();
    // Sends only the length prefix and part of the header/payload,
    // never the rest - a real stall, not merely "nothing sent".
    let half = &full[..full.len() / 2];
    assert!(half.len() >= 4, "sanity: at least the length prefix went");
    stream.write_all(half).unwrap();

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buf = [0_u8; 16];
    match stream.read(&mut buf) {
        Ok(0) => {} // a clean close
        Ok(n) => panic!("expected the connection to close with no reply, got {n} bytes"),
        Err(err) => assert_ne!(
            err.kind(),
            std::io::ErrorKind::WouldBlock,
            "the connection should have closed well within the 5s bound"
        ),
    }

    stop_server(&stop, handle);
}

/// A review of this task found the handshake timeout was, in the
/// earlier version, per-`read()` rather than for the whole handshake:
/// a client sending one byte just inside every timeout window, never
/// completing a frame, kept its connection (and thread) open for as
/// long as it liked, each successful partial read resetting the
/// per-call clock. Proves the fix: a trickler sends one byte every
/// 40ms for up to 1.6s - comfortably longer than the 300ms handshake
/// timeout below, and well inside what the *old* per-read timeout
/// would tolerate indefinitely - and the connection must still close
/// at roughly the deadline, not after the trickle eventually stops.
#[test]
fn a_client_trickling_one_byte_at_a_time_is_still_closed_at_the_deadline() {
    let handshake_timeout = Duration::from_millis(300);
    let (_socket_dir, socket_path, stop, handle) = start_server(handshake_timeout);

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let connected_at = Instant::now();

    let mut trickler_stream = stream.try_clone().unwrap();
    // Joined below on every path (including a `panic!` from this test
    // itself unwinding past it is not a concern here - there is no
    // early `panic!`/`?` between spawning this and joining it) - the
    // same rule this task applies to every thread it spawns.
    let trickler = std::thread::spawn(move || {
        // A declared length well inside `Decoder::next_frame`'s own
        // accepted range (`HEADER_LEN..=MAX_FRAME`) - large enough
        // that the handful of bytes trickled after it never completes
        // a frame, so `connection.poll()` keeps returning "need more"
        // for the whole trickle (found necessary while writing this
        // test: a declared length that is itself too small or too
        // large fails fast at the framing level - a real but
        // different, already-tested branch - rather than ever
        // reaching the deadline this test means to exercise).
        let declared_len: u32 = 1_000_000;
        let mut bytes = declared_len.to_be_bytes().to_vec();
        bytes.extend(std::iter::repeat_n(0_u8, 36));
        for byte in &bytes {
            if trickler_stream
                .write_all(std::slice::from_ref(byte))
                .is_err()
            {
                break; // the server already closed its side
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    });

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buf = [0_u8; 16];
    let result = stream.read(&mut buf);
    let elapsed = connected_at.elapsed();

    run_bounded("the trickler thread to end", JOIN_DEADLINE, move || {
        trickler.join().expect("the trickler thread should not panic");
    });

    match result {
        Ok(0) => {} // a clean close
        Ok(n) => panic!("expected the connection to close with no reply, got {n} bytes"),
        Err(err) => assert_ne!(
            err.kind(),
            std::io::ErrorKind::WouldBlock,
            "the connection should have closed near the handshake timeout, not the test's own 5s read timeout"
        ),
    }
    assert!(
        elapsed < handshake_timeout + Duration::from_secs(1),
        "expected the deadline, not the still-running 1.6s trickle, to close this: closed after {elapsed:?} against a {handshake_timeout:?} handshake timeout"
    );

    stop_server(&stop, handle);
}

/// A hello that legitimately arrives in many small pieces is not
/// penalised by the deadline - only a connection that never finishes
/// is. Each byte is sent 5ms apart, comfortably inside the 5s
/// handshake timeout given below (the hello is well under 1s of bytes
/// at that rate).
#[test]
fn a_hello_sent_one_byte_at_a_time_completing_inside_the_deadline_is_accepted() {
    let handshake_timeout = Duration::from_secs(5);
    let (_socket_dir, socket_path, stop, handle) = start_server(handshake_timeout);

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let hello = ClientHello::new("test-client", PROTOCOL_VERSION, ClientKind::Cli, None);
    let payload = serde_json::to_vec(&hello).unwrap();
    let bytes = encode_frame(&Frame::hello(payload)).unwrap();
    for byte in &bytes {
        stream.write_all(std::slice::from_ref(byte)).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let frame_payload = read_one_frame_payload(&mut stream);
    parse_server_hello(&frame_payload)
        .expect("a hello completed well inside the deadline should still be accepted");

    stop_server(&stop, handle);
}

/// The other side of the same deadline: a hello that would only
/// finish well *after* it (roughly twice the handshake timeout away,
/// not a knife's-edge margin) is never accepted - closed with no
/// reply, the same as a client that sent nothing at all, because
/// nothing was ever fully decoded before the deadline passed. Either
/// the client's own write fails partway through (the server has
/// already shut the connection down) or it finishes sending and then
/// observes the close on its next read - both are a fine, equally
/// correct outcome for this test, only a hang or an accepted hello
/// would not be.
#[test]
fn a_hello_completing_well_after_the_deadline_is_closed_with_no_reply() {
    let handshake_timeout = Duration::from_millis(300);
    let (_socket_dir, socket_path, stop, handle) = start_server(handshake_timeout);

    let mut stream = UnixStream::connect(&socket_path).unwrap();
    let hello = ClientHello::new("test-client", PROTOCOL_VERSION, ClientKind::Cli, None);
    let payload = serde_json::to_vec(&hello).unwrap();
    let bytes = encode_frame(&Frame::hello(payload)).unwrap();
    let per_byte_delay = (handshake_timeout * 2) / u32::try_from(bytes.len()).unwrap();
    for byte in &bytes {
        if stream.write_all(std::slice::from_ref(byte)).is_err() {
            break; // the server already closed its side - fine, see this test's own doc
        }
        std::thread::sleep(per_byte_delay);
    }

    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buf = [0_u8; 16];
    match stream.read(&mut buf) {
        Ok(0) => {} // a clean close
        Ok(n) => panic!("expected no reply and a close, got {n} bytes"),
        Err(err) => assert_ne!(
            err.kind(),
            std::io::ErrorKind::WouldBlock,
            "the connection should have closed once the deadline passed"
        ),
    }

    stop_server(&stop, handle);
}
