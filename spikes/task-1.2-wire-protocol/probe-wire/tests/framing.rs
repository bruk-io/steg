//! Gate 2: the framing is self-delimiting over a byte stream nobody delimits for us, which is
//! what R5 needs now and R55 (SSH) needs later. These tests are the mechanical part of the
//! spike: they have a right answer that does not depend on any measurement.

use bytes::BytesMut;
use probe_wire::{Control, Frame, Framing, Kind, MAX_PAYLOAD, Snapshot, WireError};

/// Frames are compared through their meaning, not their bytes, because of a finding this spike
/// records: the NDJSON framing cannot preserve a control payload's bytes. It has to parse the
/// payload into JSON to embed it in its envelope and serialise it again on the way out, and
/// serde_json orders object keys, so the bytes come back reordered. The JSON *object* survives,
/// which is what TASK-1.3's contract needs; byte-identical control payloads do not. The length
/// framing preserves the bytes exactly, since it never looks inside them.
fn assert_equivalent(framing: &str, want: &[Frame], got: &[Frame]) {
    assert_eq!(want.len(), got.len(), "{framing}: frame count");
    for (before, after) in want.iter().zip(got) {
        assert_eq!(before.kind, after.kind, "{framing}: kind");
        assert_eq!(before.stream, after.stream, "{framing}: stream");
        match before.kind {
            Kind::Hello | Kind::Control => {
                let want: serde_json::Value = serde_json::from_slice(&before.payload).unwrap();
                let got: serde_json::Value = serde_json::from_slice(&after.payload).unwrap();
                assert_eq!(want, got, "{framing}: the control payload's JSON object");
            }
            Kind::Data => assert_eq!(before.payload, after.payload, "{framing}: terminal bytes"),
        }
    }
}

fn sample_frames() -> Vec<Frame> {
    let request = Control::Request {
        id: 7,
        method: "pane.split".to_owned(),
        params: serde_json::json!({"pane": "a1b2", "direction": "right", "cwd": null}),
    };
    let event = Control::Event {
        event: "pane.exited".to_owned(),
        body: serde_json::json!({"pane": 1, "code": 0}),
    };
    vec![
        Frame::hello(
            serde_json::to_vec(
                &serde_json::json!({"steg": "steg", "protocol": 1, "client": "test"}),
            )
            .unwrap(),
        ),
        Frame::control(0, request.to_payload().unwrap()),
        Frame::data(1, b"\x1b[1;32mhello\x1b[0m\r\n".to_vec()),
        Frame::data(1, (0u8..=255).collect()),
        Frame::control(1, event.to_payload().unwrap()),
        Frame::data(2, Vec::new()),
    ]
}

fn encode_all(framing_name: &str, frames: &[Frame]) -> Vec<u8> {
    let mut framing = Framing::named(framing_name).unwrap();
    let mut out = BytesMut::new();
    for frame in frames {
        framing.encode(frame, &mut out).unwrap();
    }
    out.to_vec()
}

fn decode_in_chunks(framing_name: &str, stream: &[u8], chunk: usize) -> Vec<Frame> {
    let mut framing = Framing::named(framing_name).unwrap();
    let mut buf = BytesMut::new();
    let mut decoded = Vec::new();
    for piece in stream.chunks(chunk.max(1)) {
        buf.extend_from_slice(piece);
        while let Some(frame) = framing.decode(&mut buf).unwrap() {
            decoded.push(frame);
        }
    }
    assert!(
        buf.is_empty(),
        "{framing_name}: {} bytes left over",
        buf.len()
    );
    decoded
}

#[test]
fn a_stream_decodes_the_same_whatever_the_chunk_boundaries_are() {
    for name in ["ndjson", "length"] {
        let frames = sample_frames();
        let stream = encode_all(name, &frames);
        let whole = decode_in_chunks(name, &stream, stream.len());
        assert_equivalent(&format!("{name}: in one piece"), &frames, &whole);

        let byte_at_a_time = decode_in_chunks(name, &stream, 1);
        assert_equivalent(
            &format!("{name}: one byte at a time"),
            &frames,
            &byte_at_a_time,
        );

        for chunk in [2, 3, 7, 13, 64, 1021] {
            let pieces = decode_in_chunks(name, &stream, chunk);
            assert_equivalent(
                &format!("{name}: {chunk} bytes at a time"),
                &frames,
                &pieces,
            );
        }
    }
}

#[test]
fn a_partial_frame_asks_for_more_rather_than_failing() {
    for name in ["ndjson", "length"] {
        let frames = sample_frames();
        let stream = encode_all(name, &frames);
        let mut framing = Framing::named(name).unwrap();
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&stream[..stream.len() / 2]);
        let mut decoded = 0;
        while let Some(_frame) = framing.decode(&mut buf).unwrap() {
            decoded += 1;
        }
        assert!(
            decoded < frames.len(),
            "{name}: half a stream decoded every frame"
        );
    }
}

#[test]
fn the_json_object_a_client_parses_is_the_same_under_both_framings() {
    // TASK-1.3 fixed that the JSON payload is the contract. The two framings put different
    // bytes on the wire (NDJSON adds a newline and base64s terminal data), so byte equality
    // would prove nothing: what has to hold is that the JSON object a client parses out of a
    // control frame is identical, and that terminal bytes survive unchanged.
    let frames = sample_frames();
    for name in ["ndjson", "length"] {
        let stream = encode_all(name, &frames);
        let decoded = decode_in_chunks(name, &stream, 1);
        for (before, after) in frames.iter().zip(&decoded) {
            assert_eq!(before.kind, after.kind, "{name}");
            assert_eq!(before.stream, after.stream, "{name}");
            match before.kind {
                Kind::Hello | Kind::Control => {
                    let want: serde_json::Value = serde_json::from_slice(&before.payload).unwrap();
                    let got: serde_json::Value = serde_json::from_slice(&after.payload).unwrap();
                    assert_eq!(want, got, "{name}: the control payload's JSON object");
                }
                Kind::Data => assert_eq!(before.payload, after.payload, "{name}: terminal bytes"),
            }
        }
    }
}

#[test]
fn a_frame_at_the_cap_is_accepted_and_one_over_it_is_refused() {
    for name in ["ndjson", "length"] {
        let mut framing = Framing::named(name).unwrap();
        let mut out = BytesMut::new();
        let at_cap = Frame::data(1, vec![b'x'; MAX_PAYLOAD]);
        framing
            .encode(&at_cap, &mut out)
            .expect("a payload at the cap encodes");

        let over_cap = Frame::data(1, vec![b'x'; MAX_PAYLOAD + 1]);
        let refused = framing.encode(&over_cap, &mut out);
        assert!(
            matches!(refused, Err(WireError::TooLarge { .. })),
            "{name}: a payload over the cap was not refused: {refused:?}"
        );
    }
}

#[test]
fn a_declared_length_over_the_cap_is_refused_without_allocating_it() {
    // The length framing's first four bytes could ask for 4 GiB. tokio-util refuses it against
    // max_frame_length instead of reserving the buffer, which is what stops a local process from
    // exhausting memory before the hello is even read.
    let mut framing = Framing::length();
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&u32::MAX.to_be_bytes());
    buf.extend_from_slice(b"not this much");
    let refused = framing.decode(&mut buf);
    assert!(
        refused.is_err(),
        "a length of u32::MAX was accepted: {refused:?}"
    );
}

#[test]
fn bytes_that_are_not_a_frame_are_refused_rather_than_guessed_at() {
    let mut ndjson = Framing::named("ndjson").unwrap();
    let mut buf = BytesMut::from(&b"this is not json\n"[..]);
    assert!(matches!(
        ndjson.decode(&mut buf),
        Err(WireError::Malformed(_))
    ));

    let mut length = Framing::length();
    let mut buf = BytesMut::new();
    buf.extend_from_slice(&3u32.to_be_bytes());
    buf.extend_from_slice(b"abc");
    assert!(
        matches!(length.decode(&mut buf), Err(WireError::Malformed(_))),
        "a frame too short to hold the header was accepted"
    );
}

#[test]
fn a_snapshot_round_trips_through_a_control_frame() {
    // The reattach path puts a whole snapshot through the control envelope, so the framing has
    // to carry something large and deeply nested, not just small messages.
    let snapshot = Snapshot {
        seq: 42,
        cols: 80,
        rows: 24,
        screen: vec![probe_wire::Row::default(); 24],
        scrollback: vec![probe_wire::Row::default(); 10_000],
        rows_ever: 10_000,
        cursor: probe_wire::Cursor {
            col: 0,
            row: 23,
            visible: true,
        },
        alt_screen: false,
    };
    let payload = serde_json::to_vec(&snapshot).unwrap();
    for name in ["ndjson", "length"] {
        let frame = Frame::control(0, payload.clone());
        let stream = encode_all(name, std::slice::from_ref(&frame));
        let decoded = decode_in_chunks(name, &stream, 4096);
        let back: Snapshot = serde_json::from_slice(&decoded[0].payload).unwrap();
        assert_eq!(back, snapshot, "{name}");
    }
}
