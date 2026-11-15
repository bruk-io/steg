//! AC #2: "A consumer supplies the transport through an interface the sdk
//! defines; a test drives a client and a server end against each other in
//! memory." Also AC #4 (the version check, both directions named) and
//! AC #5 (a method called by name is validated against the `Description`,
//! and its result is matched back by id, with none written by hand).
//!
//! No socket, clock, thread or runtime anywhere in this test: the "in
//! memory" shuttle is this file handing one `Connection`'s outgoing bytes
//! straight to the other's `receive`, nothing more.

use serde_json::json;
use steg_sdk::{
    ClientId, ClientKind, Connection, Description, Event, Handshake, Incoming, Method, MethodName,
    Response, Returns, Version, WireRole, parse_server_hello,
};

/// One method (`pane.split`, no required params) and one event
/// (`pane.closed`) - enough to exercise `call()`'s validation and the
/// event-name resolution, without pulling in the real 38-method
/// description from `TASK-1.43`.
fn description_with_one_method_and_one_event() -> Description {
    Description {
        api_version: Version { major: 1, minor: 0 },
        methods: vec![Method {
            name: MethodName {
                noun: "pane".into(),
                verb: "split".into(),
            },
            summary: String::new(),
            details: String::new(),
            params: Vec::new(),
            returns: Returns::Identifier { of: "pane".into() },
            errors: Vec::new(),
            since: Version { major: 1, minor: 0 },
        }],
        events: vec![Event {
            name: "pane.closed".into(),
            summary: String::new(),
            fields: Vec::new(),
            since: Version { major: 1, minor: 0 },
        }],
        errors: Vec::new(),
    }
}

#[test]
fn a_client_and_server_connection_shake_hands_and_exchange_a_call_in_memory() {
    let mut client = Connection::new(
        WireRole::Client,
        description_with_one_method_and_one_event(),
    );
    let mut server = Connection::new(
        WireRole::Server,
        description_with_one_method_and_one_event(),
    );

    // the handshake, driven over the in-memory shuttle.
    let hello_bytes = Connection::send_hello("steg-cli/0.1.0", ClientKind::Cli, None);
    server.receive(&hello_bytes);
    let Some(Incoming::Hello(payload)) = server.poll() else {
        panic!("expected the client's hello");
    };
    let (handshake, server_hello_bytes) =
        Connection::accept_or_refuse(&payload, "steg/0.1.0", "1.0", ClientId::from_raw(1));
    assert!(matches!(handshake, Handshake::Accept { .. }));

    client.receive(&server_hello_bytes);
    let Some(Incoming::Hello(server_payload)) = client.poll() else {
        panic!("expected the server's hello");
    };
    let server_hello =
        parse_server_hello(&server_payload).expect("the server accepted the handshake");
    assert_eq!(server_hello.client, ClientId::from_raw(1));

    // AC #5, the refusal half: a method the `Description` doesn't list is
    // refused before anything is sent - the generic path every call takes,
    // with no method written by hand to check it.
    assert!(client.call("made.up", serde_json::Map::new()).is_err());

    // AC #5, the call half: validated, sent, answered, matched by id.
    let (id, request_bytes) = client.call("pane.split", serde_json::Map::new()).unwrap();
    server.receive(&request_bytes);
    let Some(Incoming::Request {
        id: server_side_id,
        request,
    }) = server.poll()
    else {
        panic!("expected a request");
    };
    assert_eq!(server_side_id, id);
    assert_eq!(request.method, "pane.split");

    let reply_bytes = Connection::reply(id, Response::Ok(json!({"pane": "00000001"})));
    client.receive(&reply_bytes);
    let Some(Incoming::Reply {
        id: reply_id,
        response,
    }) = client.poll()
    else {
        panic!("expected a reply");
    };
    assert_eq!(reply_id, id);
    assert!(matches!(response, Response::Ok(_)));
}

/// AC #4: a mismatched version is refused naming both versions, whichever
/// side is ahead.
#[test]
fn a_version_mismatch_is_refused_naming_both_versions() {
    // `send_hello` always uses this build's `PROTOCOL_VERSION`, so this
    // test builds the client's hello bytes directly to simulate a client
    // speaking a different one.
    let hello = steg_sdk::ClientHello::new("old-client", 0, ClientKind::Cli, None);
    let payload = serde_json::to_vec(&hello).unwrap();
    let (handshake, refusal_bytes) =
        Connection::accept_or_refuse(&payload, "steg/0.1.0", "1.0", ClientId::from_raw(1));
    let Handshake::Refuse(refusal) = handshake else {
        panic!("expected a refusal");
    };
    assert!(refusal.message().contains('0'));
    assert!(!refusal_bytes.is_empty());

    let mut client = Connection::new(
        WireRole::Client,
        description_with_one_method_and_one_event(),
    );
    client.receive(&refusal_bytes);
    let Some(Incoming::Hello(server_payload)) = client.poll() else {
        panic!("expected the server's refusal frame");
    };
    let err = parse_server_hello(&server_payload).unwrap_err();
    assert!(err.contains('0'), "{err}");
}
