//! The protocol's logic (doc-16 R101, R102): encoding, decoding, the
//! handshake and the state of each stream, as logic that takes bytes and
//! returns bytes and events, with no socket, clock, thread or runtime
//! anywhere in this module tree. Every test here needs none either.
//!
//! Decision record: `backlog/decisions/decision-2`. Spec: `backlog/docs/doc-14`.

mod connection;
mod control;
mod frame;
mod handshake;

pub use connection::{Connection, Incoming, Role};
pub use control::Control;
pub use frame::{
    CONTROL_STREAM, Decoder, Frame, FrameError, Kind, MAX_FRAME, MAX_PAYLOAD, encode_frame,
};
pub use handshake::{
    ClientHello, Handshake, PROTOCOL_VERSION, Refusal, Refused, STEG_MAGIC, ServerHello,
    check_hello, parse_server_hello,
};
