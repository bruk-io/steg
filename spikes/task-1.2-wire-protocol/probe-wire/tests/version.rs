//! AC #3: how the protocol carries a version, and R39's "work together or fail with a message
//! naming both versions". The failure that matters is the one that is easy to get wrong: a
//! mismatch must come back as a message, not as a parse error, a hang, or a huge allocation.

use bytes::BytesMut;
use probe_wire::{
    ClientHello, Frame, Framing, Handshake, PROTOCOL_VERSION, Refusal, Refused, check_hello,
};

fn hello_payload(hello: &ClientHello) -> Vec<u8> {
    serde_json::to_vec(hello).unwrap()
}

#[test]
fn the_same_version_is_accepted() {
    let hello = ClientHello::new("probe-client/0.0.0", PROTOCOL_VERSION);
    let decision = check_hello(&hello_payload(&hello), PROTOCOL_VERSION);
    assert_eq!(
        decision,
        Handshake::Accept {
            client: "probe-client/0.0.0".to_owned(),
            protocol: PROTOCOL_VERSION
        }
    );
}

#[test]
fn a_newer_client_is_refused_with_both_versions_named() {
    let hello = ClientHello::new("probe-client/0.0.0", PROTOCOL_VERSION + 1);
    let Handshake::Refuse(refusal) = check_hello(&hello_payload(&hello), PROTOCOL_VERSION) else {
        panic!("a client one version ahead was accepted");
    };
    assert_eq!(
        refusal,
        Refusal::Mismatch {
            client: PROTOCOL_VERSION + 1,
            server: PROTOCOL_VERSION
        }
    );
    let message = refusal.message();
    assert!(
        message.contains(&(PROTOCOL_VERSION + 1).to_string()),
        "{message}"
    );
    assert!(message.contains(&PROTOCOL_VERSION.to_string()), "{message}");
}

#[test]
fn an_older_client_is_refused_with_both_versions_named() {
    let hello = ClientHello::new("probe-client/0.0.0", PROTOCOL_VERSION - 1);
    let Handshake::Refuse(refusal) = check_hello(&hello_payload(&hello), PROTOCOL_VERSION) else {
        panic!("a client one version behind was accepted");
    };
    let wire = refusal.to_wire();
    assert_eq!(wire.client_protocol, Some(PROTOCOL_VERSION - 1));
    assert_eq!(wire.server_protocol, PROTOCOL_VERSION);
    assert_eq!(wire.error, "protocol-mismatch");
}

#[test]
fn a_hello_that_cannot_be_read_is_an_error_naming_the_servers_version() {
    for (name, payload) in [
        ("not json", &b"]["[..]),
        ("truncated", &b"{\"steg\":\"steg\",\"proto"[..]),
        ("empty", &b""[..]),
        ("a json array", &b"[1,2,3]"[..]),
        (
            "the wrong magic",
            &br#"{"steg":"ssh","protocol":1,"client":"x"}"#[..],
        ),
        ("no protocol field", &br#"{"steg":"steg","client":"x"}"#[..]),
    ] {
        let Handshake::Refuse(refusal) = check_hello(payload, PROTOCOL_VERSION) else {
            panic!("{name}: accepted");
        };
        assert!(
            matches!(refusal, Refusal::Malformed { .. }),
            "{name}: {refusal:?} is not a malformed-hello refusal"
        );
        let message = refusal.message();
        assert!(
            message.contains(&PROTOCOL_VERSION.to_string()),
            "{name}: {message}"
        );
        assert_eq!(refusal.to_wire().client_protocol, None, "{name}");
    }
}

#[test]
fn an_unknown_field_in_a_hello_is_ignored_so_a_later_version_can_add_one() {
    // This is the forward-compatibility promise AC #3 turns on: whatever a future version adds,
    // this version still reads the two frozen fields and can answer with a message.
    let payload = br#"{"steg":"steg","protocol":1,"client":"future/9","capabilities":["x"],"nested":{"a":1}}"#;
    let decision = check_hello(payload, 1);
    assert_eq!(
        decision,
        Handshake::Accept {
            client: "future/9".to_owned(),
            protocol: 1
        }
    );
}

#[test]
fn a_refusal_travels_as_a_frame_a_client_of_any_version_can_read() {
    // The refusal has to come back through the framing, or the client sees a closed socket with
    // no reason. It is a hello-kind frame, so a client that cannot speak the server's control
    // version can still read it.
    let refusal = Refusal::Mismatch {
        client: 9,
        server: PROTOCOL_VERSION,
    };
    let payload = serde_json::to_vec(&refusal.to_wire()).unwrap();
    let frame = Frame::hello(payload);

    let mut framing = Framing::length();
    let mut out = BytesMut::new();
    framing.encode(&frame, &mut out).unwrap();

    let mut reading = Framing::length();
    let decoded = reading.decode(&mut out).unwrap().expect("a whole frame");
    let wire: Refused = serde_json::from_slice(&decoded.payload).unwrap();
    assert_eq!(wire.client_protocol, Some(9));
    assert_eq!(wire.server_protocol, PROTOCOL_VERSION);
    assert!(wire.message.contains('9'), "{}", wire.message);
}
