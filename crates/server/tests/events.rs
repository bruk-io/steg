//! TASK-1.46: the tree snapshot, subscribing, and this task's own events,
//! proved over a real socket with the sdk's own `Connection` - the same
//! shape `routing.rs` already uses for its own real-connection proofs.

#![expect(
    clippy::unwrap_used,
    reason = "this whole file is a test; see dispatch/main.rs's own comment"
)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use steg_sdk::{
    ClientKind, Connection, Incoming, Response, SessionTree, TreeEvent, TreeSnapshot,
    WireRole as Role, description, parse_server_hello,
};
use steg_server::{LoginShell, ServerConfig, Stopper, serve};

#[path = "support/mod.rs"]
mod support;
use support::{ScopedTempDir, scoped_socket_path};

/// Same reasoning as `routing.rs`'s own `test_login`: a working `PATH` is
/// required (TASK-1.87) for any test here that starts a real program.
fn test_login() -> LoginShell {
    LoginShell {
        argv0: "/bin/sh".to_string(),
        home: std::env::temp_dir().to_str().unwrap().to_string(),
        user: None,
        base_env: vec![(
            std::ffi::OsString::from("PATH"),
            std::env::var_os("PATH").unwrap_or_default(),
        )],
    }
}

fn start_server() -> (ScopedTempDir, PathBuf, Stopper, std::thread::JoinHandle<()>) {
    let (socket_dir, socket_path) = scoped_socket_path("s.sock");
    let config = ServerConfig {
        socket_path: Some(socket_path.clone()),
        handshake_timeout: Duration::from_secs(5),
        login: test_login(),
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
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if UnixStream::connect(&socket_path).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "server never started accepting");
        std::thread::sleep(Duration::from_millis(5));
    }
    (socket_dir, socket_path, stop, handle)
}

fn stop_server(stop: &Stopper, handle: std::thread::JoinHandle<()>) {
    stop.stop();
    handle.join().expect("the server thread should not panic");
}

/// Same shape as `routing.rs`'s own `connect_and_handshake`.
fn connect_and_handshake(socket_path: &std::path::Path) -> (UnixStream, Connection) {
    let mut stream = UnixStream::connect(socket_path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let hello_bytes = Connection::send_hello("events-test-client", ClientKind::Cli, None);
    stream.write_all(&hello_bytes).unwrap();

    let mut connection = Connection::new(Role::Client, description());
    let hello_reply = loop {
        if let Some(incoming) = connection.poll() {
            match incoming {
                Incoming::Hello(payload) => break payload,
                other @ (Incoming::Request { .. }
                | Incoming::Reply { .. }
                | Incoming::Event { .. }
                | Incoming::UnknownEvent { .. }
                | Incoming::StreamControl { .. }
                | Incoming::Data { .. }
                | Incoming::UnknownStream { .. }
                | Incoming::Malformed(_)) => {
                    panic!("expected the server's hello reply, got {other:?}")
                }
            }
        }
        let mut buf = [0_u8; 4096];
        let n = stream
            .read(&mut buf)
            .expect("reading the server's hello reply");
        assert!(n > 0, "server closed before replying to the hello");
        connection.receive(&buf[..n]);
    };
    parse_server_hello(&hello_reply).expect("a matching protocol version should be accepted");
    (stream, connection)
}

/// One deadline for the whole wait - `routing.rs`'s own flood test found
/// a per-read bound alone lets the total time grow without one.
const READ_DEADLINE: Duration = Duration::from_secs(10);

fn read_reply(stream: &mut UnixStream, connection: &mut Connection, id: u64) -> Response {
    let deadline = Instant::now() + READ_DEADLINE;
    loop {
        if let Some(Incoming::Reply {
            id: reply_id,
            response,
        }) = connection.poll()
        {
            assert_eq!(reply_id, id, "the reply must carry the id this call minted");
            return response;
        }
        assert!(Instant::now() < deadline, "timed out waiting for a reply");
        let mut buf = [0_u8; 65536];
        let n = stream.read(&mut buf).expect("reading the reply");
        assert!(n > 0, "server closed before replying");
        connection.receive(&buf[..n]);
    }
}

fn read_event(stream: &mut UnixStream, connection: &mut Connection) -> TreeEvent {
    let deadline = Instant::now() + READ_DEADLINE;
    loop {
        match connection.poll() {
            Some(Incoming::Event { name, body }) => {
                return TreeEvent::parse(&name, body).expect("a described event parses");
            }
            Some(other) => panic!("expected an event, got {other:?}"),
            None => {}
        }
        assert!(Instant::now() < deadline, "timed out waiting for an event");
        let mut buf = [0_u8; 65536];
        let n = stream.read(&mut buf).expect("reading the event");
        assert!(n > 0, "server closed before sending an event");
        connection.receive(&buf[..n]);
    }
}

/// Subscribes (`event.watch`) and reads the reply as a `TreeSnapshot` -
/// the one call every test below that needs a subscription starts with.
fn subscribe(
    stream: &mut UnixStream,
    connection: &mut Connection,
    target: Option<&str>,
    for_: Option<&str>,
    all: bool,
) -> TreeSnapshot {
    let (id, bytes) = connection.watch(target, for_, all).expect("a valid watch");
    stream.write_all(&bytes).unwrap();
    match read_reply(stream, connection, id) {
        Response::Ok(value) => serde_json::from_value(value).expect("a TreeSnapshot"),
        Response::Error(error) => panic!("event.watch failed: {error:?}"),
    }
}

fn call(
    stream: &mut UnixStream,
    connection: &mut Connection,
    method: &str,
    params: Map<String, Value>,
) -> Response {
    let (id, bytes) = connection.call(method, params).expect("a valid call");
    stream.write_all(&bytes).unwrap();
    read_reply(stream, connection, id)
}

fn ok_id(response: Response) -> String {
    match response {
        Response::Ok(value) => value["id"].as_str().unwrap().to_string(),
        Response::Error(error) => panic!("expected success, got {error:?}"),
    }
}

// --- AC #1: the snapshot --------------------------------------------------

#[test]
fn subscribing_to_a_fresh_server_replies_with_an_empty_tree() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut stream, mut connection) = connect_and_handshake(&socket_path);

    let snapshot = subscribe(&mut stream, &mut connection, None, None, true);

    assert_eq!(snapshot.sessions, Vec::new());
    stop_server(&stop, handle);
}

// --- AC #2: creation, outermost first -------------------------------------

#[test]
fn session_new_emits_session_created_tab_created_and_pane_created_in_order() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));

    let first = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::SessionCreated { id, .. } = &first else {
        panic!("expected session.created first, got {first:?}");
    };
    assert_eq!(id.to_string(), session_id);

    let second = read_event(&mut sub_stream, &mut sub_connection);
    assert!(
        matches!(second, TreeEvent::TabCreated { .. }),
        "expected tab.created second, got {second:?}"
    );

    let third = read_event(&mut sub_stream, &mut sub_connection);
    assert!(
        matches!(third, TreeEvent::PaneCreated { .. }),
        "expected pane.created third, got {third:?}"
    );

    stop_server(&stop, handle);
}

/// `tab.new`'s own creation cascade (`tab.created` then `pane.created`) is
/// already exercised by the filter tests below; `pane.split`'s own single
/// `pane.created` had no dedicated test until the second review round's
/// own systematic pass over every kernel function that creates or removes
/// a session, tab or pane found the gap.
#[test]
fn pane_split_emits_pane_created_only() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    let mut split_params = Map::new();
    split_params.insert("target".to_string(), json!(format!("{session_id}/1/1")));
    split_params.insert("direction".to_string(), json!("right"));
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "pane.split",
        split_params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let first = read_event(&mut sub_stream, &mut sub_connection);
    assert!(
        matches!(first, TreeEvent::PaneCreated { .. }),
        "expected pane.created, got {first:?}"
    );
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

// --- AC #2: closure, innermost first ---------------------------------------

#[test]
/// Two tabs, two panes each (the plan's own proof, not the one-tab-one
/// -pane shape a first draft of this test used, which cannot show order
/// *across* tabs at all) - every pane closed, then its own tab, repeated
/// per tab in the session's own order, then the session itself.
fn session_close_emits_the_whole_cascade_innermost_first() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    // The session's own default tab (position 1), split into two panes.
    let mut split_params = Map::new();
    split_params.insert("target".to_string(), json!(format!("{session_id}/1/1")));
    split_params.insert("direction".to_string(), json!("right"));
    call(
        &mut client_stream,
        &mut client_connection,
        "pane.split",
        split_params,
    );
    // A second tab, also split into two panes.
    let mut tab_params = Map::new();
    tab_params.insert("target".to_string(), json!(session_id.clone()));
    let second_tab = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "tab.new",
        tab_params,
    ));
    let mut split_params = Map::new();
    split_params.insert(
        "target".to_string(),
        json!(format!("{session_id}/{second_tab}/1")),
    );
    split_params.insert("direction".to_string(), json!("right"));
    call(
        &mut client_stream,
        &mut client_connection,
        "pane.split",
        split_params,
    );

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    let mut params = Map::new();
    params.insert("target".to_string(), json!(session_id.clone()));
    // session.new's own default program is a shell, which is "running"
    // until it exits - force is needed, the same as any other close of a
    // session with a live program in it.
    params.insert("force".to_string(), json!(true));
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "session.close",
        params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    // First tab: both its panes, then itself.
    let first = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(first, TreeEvent::PaneClosed { .. }), "{first:?}");
    let second = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(second, TreeEvent::PaneClosed { .. }), "{second:?}");
    let third = read_event(&mut sub_stream, &mut sub_connection);
    assert!(
        matches!(third, TreeEvent::TabClosed { .. }),
        "expected the first tab's own tab.closed third, got {third:?}"
    );
    // Second tab: both its panes, then itself.
    let fourth = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(fourth, TreeEvent::PaneClosed { .. }), "{fourth:?}");
    let fifth = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(fifth, TreeEvent::PaneClosed { .. }), "{fifth:?}");
    let sixth = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::TabClosed { id } = sixth else {
        panic!("expected the second tab's own tab.closed sixth, got {sixth:?}");
    };
    assert_eq!(id.to_string(), second_tab);
    // Finally, the session itself.
    let seventh = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::SessionClosed { id } = seventh else {
        panic!("expected session.closed seventh, got {seventh:?}");
    };
    assert_eq!(id.to_string(), session_id);
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

/// Important review finding: `pane.close` on the last pane of a tab that
/// is *not* the last tab in its session must still emit `tab.closed` for
/// the tab the kernel's own cascade also removed (R94) - the session
/// must not close, since another tab remains.
#[test]
fn pane_close_on_the_last_pane_of_a_tab_that_is_not_the_last_tab_closes_the_tab_too() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    let mut tab_params = Map::new();
    tab_params.insert("target".to_string(), json!(session_id.clone()));
    let second_tab = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "tab.new",
        tab_params,
    ));

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    // The second tab's own one pane, by position - closing it is closing
    // that tab's last pane, so the tab closes too, but the session must
    // not (the first tab still holds it open).
    let mut close_params = Map::new();
    close_params.insert(
        "target".to_string(),
        json!(format!("{session_id}/{second_tab}/1")),
    );
    close_params.insert("force".to_string(), json!(true));
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "pane.close",
        close_params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let first = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(first, TreeEvent::PaneClosed { .. }), "{first:?}");
    let second = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::TabClosed { id } = second else {
        panic!("expected tab.closed second, got {second:?}");
    };
    assert_eq!(id.to_string(), second_tab);
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

/// Important review finding, the case the review itself reproduced
/// against a running server: `pane.close` on the last pane of the *last*
/// tab in a session must cascade all three events, in order - before this
/// fix, only `pane.closed` was ever emitted, though the kernel's own
/// `close_pane` already reported the tab and the session closing too.
#[test]
fn pane_close_on_the_last_pane_of_the_last_tab_closes_pane_tab_and_session_in_order() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    let mut close_params = Map::new();
    close_params.insert("target".to_string(), json!(format!("{session_id}/1/1")));
    close_params.insert("force".to_string(), json!(true));
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "pane.close",
        close_params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let first = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(first, TreeEvent::PaneClosed { .. }), "{first:?}");
    let second = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(second, TreeEvent::TabClosed { .. }), "{second:?}");
    let third = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::SessionClosed { id } = third else {
        panic!("expected session.closed third, got {third:?}");
    };
    assert_eq!(id.to_string(), session_id);
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

/// Important review finding, the mirror case on `tab.close`: closing the
/// last tab of a session must emit every one of its own panes closed,
/// then the tab, then the session, in that order.
#[test]
fn tab_close_on_the_last_tab_closes_its_panes_then_the_tab_then_the_session() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    let mut split_params = Map::new();
    split_params.insert("target".to_string(), json!(format!("{session_id}/1/1")));
    split_params.insert("direction".to_string(), json!("right"));
    call(
        &mut client_stream,
        &mut client_connection,
        "pane.split",
        split_params,
    );

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    let mut close_params = Map::new();
    close_params.insert("target".to_string(), json!(format!("{session_id}/1")));
    close_params.insert("force".to_string(), json!(true));
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "tab.close",
        close_params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let first = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(first, TreeEvent::PaneClosed { .. }), "{first:?}");
    let second = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(second, TreeEvent::PaneClosed { .. }), "{second:?}");
    let third = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(third, TreeEvent::TabClosed { .. }), "{third:?}");
    let fourth = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::SessionClosed { id } = fourth else {
        panic!("expected session.closed fourth, got {fourth:?}");
    };
    assert_eq!(id.to_string(), session_id);
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

/// Important review finding, the second round: `pane.move` can empty its
/// source tab exactly as `pane.close` can (R94's own cascade,
/// `Tree::move_pane_to_tab`'s `source_tab_closed`) - here the session the
/// tab belonged to still holds another tab, so only `tab.closed` fires,
/// never `session.closed`. No `pane.closed` either: the pane is not
/// closed, it has moved into another tab - a change this task's own plan
/// does not cover (TASK-1.109's, see the notes).
#[test]
fn pane_move_emptying_a_tab_that_is_not_the_sessions_last_closes_only_the_tab() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    let mut show_params = Map::new();
    show_params.insert("target".to_string(), json!(format!("{session_id}/1")));
    let first_tab = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "tab.show",
        show_params,
    ));
    let mut tab_params = Map::new();
    tab_params.insert("target".to_string(), json!(session_id.clone()));
    let second_tab = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "tab.new",
        tab_params,
    ));

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    // The first tab's own one pane (by position), moved into the second
    // tab - emptying the first tab, which is not this session's last.
    let mut move_params = Map::new();
    move_params.insert("target".to_string(), json!(format!("{session_id}/1/1")));
    move_params.insert(
        "to".to_string(),
        json!(format!("{session_id}/{second_tab}")),
    );
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "pane.move",
        move_params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let first = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::TabClosed { id } = first else {
        panic!("expected tab.closed, nothing else, got {first:?}");
    };
    assert_eq!(id.to_string(), first_tab);
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

/// The mirror case: the tab `pane.move` empties is the session's *last*
/// tab, so the cascade goes all the way - `tab.closed` then
/// `session.closed`, innermost first, same rule as every close path.
#[test]
fn pane_move_emptying_the_sessions_last_tab_closes_the_tab_then_the_session() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let source_session = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    let target_session = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    // The source session's only pane, in its only tab, moved into the
    // target session's own default tab - emptying and closing the source
    // tab, which was the source session's last, so the source session
    // closes too.
    let mut move_params = Map::new();
    move_params.insert("target".to_string(), json!(format!("{source_session}/1/1")));
    move_params.insert("to".to_string(), json!(format!("{target_session}/1")));
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "pane.move",
        move_params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let first = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(first, TreeEvent::TabClosed { .. }), "{first:?}");
    let second = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::SessionClosed { id } = second else {
        panic!("expected session.closed second, got {second:?}");
    };
    assert_eq!(id.to_string(), source_session);
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

// --- AC #2: focus, whole path -----------------------------------------------

#[test]
fn tab_focus_emits_tab_focused_with_the_whole_path() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    let mut tab_params = Map::new();
    tab_params.insert("target".to_string(), json!(session_id));
    let tab_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "tab.new",
        tab_params,
    ));

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    // Every connection has no context today (`Context::none`, confirmed
    // by reading `dispatch/mod.rs`) - a bare, single-segment tab target
    // needs a session to scope it within (doc-2 section 4's own grammar),
    // so the full `session/tab` path is given explicitly rather than
    // relying on a context fallback this connection does not have.
    let mut focus_params = Map::new();
    focus_params.insert(
        "target".to_string(),
        json!(format!("{session_id}/{tab_id}")),
    );
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "tab.focus",
        focus_params,
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    let event = read_event(&mut sub_stream, &mut sub_connection);
    let TreeEvent::TabFocused { session, tab, .. } = event else {
        panic!("expected tab.focused, got {event:?}");
    };
    assert_eq!(session.to_string(), session_id);
    assert_eq!(tab.to_string(), tab_id);

    stop_server(&stop, handle);
}

// --- AC #3: narrowed by name and by session --------------------------------

#[test]
fn a_filter_by_name_keeps_only_that_event() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));

    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(
        &mut sub_stream,
        &mut sub_connection,
        Some(&session_id),
        Some("tab.created"),
        false,
    );

    let mut tab_params = Map::new();
    tab_params.insert("target".to_string(), json!(session_id));
    call(
        &mut client_stream,
        &mut client_connection,
        "tab.new",
        tab_params,
    );

    // Only tab.created should arrive - pane.created, also caused by this
    // same request, must never reach a subscriber filtered to one name.
    let event = read_event(&mut sub_stream, &mut sub_connection);
    assert!(
        matches!(event, TreeEvent::TabCreated { .. }),
        "expected only tab.created, got {event:?}"
    );
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}

#[test]
fn a_filter_by_session_excludes_another_sessions_events() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let watched_session = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    let other_session = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));

    let (mut narrow_stream, mut narrow_connection) = connect_and_handshake(&socket_path);
    subscribe(
        &mut narrow_stream,
        &mut narrow_connection,
        Some(&watched_session),
        None,
        false,
    );
    let (mut wide_stream, mut wide_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut wide_stream, &mut wide_connection, None, None, true);

    let mut tab_params = Map::new();
    tab_params.insert("target".to_string(), json!(other_session));
    call(
        &mut client_stream,
        &mut client_connection,
        "tab.new",
        tab_params,
    );

    // The narrowly-subscribed connection must see nothing from the other
    // session; the `--all` one must see both of its creation events.
    assert_nothing_arrives_within(
        &mut narrow_stream,
        &mut narrow_connection,
        Duration::from_millis(300),
    );
    let first = read_event(&mut wide_stream, &mut wide_connection);
    assert!(matches!(first, TreeEvent::TabCreated { .. }), "{first:?}");
    let second = read_event(&mut wide_stream, &mut wide_connection);
    assert!(
        matches!(second, TreeEvent::PaneCreated { .. }),
        "{second:?}"
    );

    stop_server(&stop, handle);
}

/// Asserts nothing further arrives on `stream` within `bound` - checking
/// `connection`'s own already-decoded buffer *first*, not just whether
/// new bytes land on the raw socket: a single `read()` can (and does)
/// pull the bytes of more than one queued frame in one call, so a frame
/// already fully received and sitting inside `connection`'s own decoder,
/// never drained by an earlier `poll()` call, would otherwise go
/// unnoticed here - found exactly that way while building this test.
fn assert_nothing_arrives_within(
    stream: &mut UnixStream,
    connection: &mut Connection,
    bound: Duration,
) {
    if let Some(incoming) = connection.poll() {
        panic!("expected nothing, but connection already had this buffered: {incoming:?}");
    }
    stream.set_read_timeout(Some(bound)).unwrap();
    let mut buf = [0_u8; 65536];
    match stream.read(&mut buf) {
        Ok(0) => {}
        Ok(n) => {
            connection.receive(&buf[..n]);
            let incoming = connection.poll();
            panic!("expected nothing, got {n} bytes decoding to {incoming:?}");
        }
        Err(err) => assert_eq!(
            err.kind(),
            std::io::ErrorKind::WouldBlock,
            "expected a plain timeout, got {err:?}"
        ),
    }
}

// --- AC #1/#3: ending a subscription is disconnecting ----------------------

#[test]
fn disconnecting_ends_a_subscription_with_no_panic_and_no_further_broadcast() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);

    {
        let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
        subscribe(&mut sub_stream, &mut sub_connection, None, None, true);
        // Dropped here: the connection closes, same as a real client
        // disconnecting.
    }

    // A real mutation right after - the server must not panic or block
    // broadcasting to a subscriber that is already gone.
    let response = call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    );
    assert!(matches!(response, Response::Ok(_)), "{response:?}");

    stop_server(&stop, handle);
}

// --- AC #3: a slow subscriber is closed, never blocked on ------------------
//
// Proved at the unit level, in `runtime/events.rs`'s own test module
// (`a_slow_subscribers_connection_is_shut_down_once_its_queue_is_full`),
// not here: driving `Outbound`'s own `CONTROL_CAP` (256) to genuinely
// overflow needs the writer thread to never drain it at all, which is
// deterministic and immediate with a direct `broadcast` call in a tight
// loop, and flaky here - tried first, real-socket, pipelining thousands of
// requests to out-race a real writer thread and a real OS socket buffer of
// an unknown, platform-dependent size, which is exactly the instability
// this comment is warning the next reader away from repeating.

// --- AC #1: atomicity - the reply always precedes any event, under a race -

#[test]
fn the_snapshots_reply_always_precedes_any_event_under_a_real_race() {
    let (_socket_dir, socket_path, stop, handle) = start_server();

    for _ in 0..20 {
        let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
        let (mut mutator_stream, mut mutator_connection) = connect_and_handshake(&socket_path);

        // Both threads start as close together as `Barrier` can manage,
        // so the mutation genuinely can land before, during or after
        // this connection's own subscribe - never only "after", which
        // would prove nothing about the race.
        let barrier = Arc::new(Barrier::new(2));
        let mutate_barrier = Arc::clone(&barrier);
        let mutator = std::thread::spawn(move || {
            mutate_barrier.wait();
            call(
                &mut mutator_stream,
                &mut mutator_connection,
                "session.new",
                Map::new(),
            )
        });

        barrier.wait();
        let (id, bytes) = sub_connection.watch(None, None, true).unwrap();
        sub_stream.write_all(&bytes).unwrap();

        // The very first thing decoded off this connection must be the
        // reply (the tree), never an event - proving this by reading the
        // first `Incoming` directly, not by calling the `subscribe`
        // helper (which would itself only ever look for a reply and so
        // could not catch an event arriving first).
        let first = loop {
            if let Some(incoming) = sub_connection.poll() {
                break incoming;
            }
            let mut buf = [0_u8; 65536];
            let n = sub_stream.read(&mut buf).unwrap();
            assert!(n > 0);
            sub_connection.receive(&buf[..n]);
        };
        let snapshot = match first {
            Incoming::Reply {
                id: reply_id,
                response: Response::Ok(value),
            } => {
                assert_eq!(reply_id, id);
                serde_json::from_value::<TreeSnapshot>(value).unwrap()
            }
            Incoming::Reply {
                response: Response::Error(error),
                ..
            } => panic!("event.watch failed: {error:?}"),
            other @ (Incoming::Hello(_)
            | Incoming::Request { .. }
            | Incoming::Event { .. }
            | Incoming::UnknownEvent { .. }
            | Incoming::StreamControl { .. }
            | Incoming::Data { .. }
            | Incoming::UnknownStream { .. }
            | Incoming::Malformed(_)) => {
                panic!("expected the reply first, got {other:?} instead")
            }
        };

        let new_session_id = ok_id(mutator.join().unwrap());

        // Exactly once: either the race's own session (with its own tab
        // and pane - `session.new` mints all three atomically, so either
        // the whole triple is in the snapshot or none of it is) is
        // already in the snapshot (it won the race to the lock), or its
        // own three creation events arrive right after (never zero,
        // never duplicated).
        let in_snapshot = snapshot
            .sessions
            .iter()
            .any(|session: &SessionTree| session.session.id.to_string() == new_session_id);
        if !in_snapshot {
            let first = read_event(&mut sub_stream, &mut sub_connection);
            let TreeEvent::SessionCreated { id, .. } = first else {
                panic!("expected session.created, got {first:?}");
            };
            assert_eq!(id.to_string(), new_session_id);
            let second = read_event(&mut sub_stream, &mut sub_connection);
            assert!(matches!(second, TreeEvent::TabCreated { .. }), "{second:?}");
            let third = read_event(&mut sub_stream, &mut sub_connection);
            assert!(matches!(third, TreeEvent::PaneCreated { .. }), "{third:?}");
        }
        assert_nothing_arrives_within(
            &mut sub_stream,
            &mut sub_connection,
            Duration::from_millis(100),
        );
    }

    stop_server(&stop, handle);
}

// --- No periodic wake-up: a quiet window produces nothing ------------------

#[test]
fn a_quiet_window_produces_nothing_and_a_change_right_after_produces_exactly_one_event() {
    let (_socket_dir, socket_path, stop, handle) = start_server();
    let (mut client_stream, mut client_connection) = connect_and_handshake(&socket_path);
    let session_id = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "session.new",
        Map::new(),
    ));
    let mut tab_params = Map::new();
    tab_params.insert("target".to_string(), json!(session_id.clone()));
    let other_tab = ok_id(call(
        &mut client_stream,
        &mut client_connection,
        "tab.new",
        tab_params,
    ));

    // Subscribed only after every creation event above has already
    // happened - `tab.focus` below is the one, single event this test
    // is actually about; `session.new`/`tab.new` legitimately produce
    // three and two events respectively (proven by their own tests), so
    // neither belongs in a test asserting "exactly one".
    let (mut sub_stream, mut sub_connection) = connect_and_handshake(&socket_path);
    subscribe(&mut sub_stream, &mut sub_connection, None, None, true);

    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(500),
    );

    let mut focus_params = Map::new();
    focus_params.insert(
        "target".to_string(),
        json!(format!("{session_id}/{other_tab}")),
    );
    call(
        &mut client_stream,
        &mut client_connection,
        "tab.focus",
        focus_params,
    );
    let event = read_event(&mut sub_stream, &mut sub_connection);
    assert!(matches!(event, TreeEvent::TabFocused { .. }), "{event:?}");
    assert_nothing_arrives_within(
        &mut sub_stream,
        &mut sub_connection,
        Duration::from_millis(300),
    );

    stop_server(&stop, handle);
}
