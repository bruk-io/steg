//! The multiplexed connection's state: which streams are open, and turning
//! bytes into typed events and typed calls into bytes - with no socket,
//! clock, thread or runtime anywhere in this module (doc-16 R102).
//!
//! One type drives both ends (R102: "it serves both ends of the
//! connection, since the server is a consumer too"). No transport trait is
//! defined: a blocking CLI, GPUI's executor, and a future Python asyncio
//! transport each have a different I/O model, and Python cannot implement
//! a Rust trait at all, so a trait here would either leak one runtime's
//! shape into the public interface (doc-16 R106 forbids that) or constrain
//! nothing useful. The interface this crate defines is this method
//! contract itself: bytes handed to [`Connection::receive`], bytes taken
//! back from every `send_*`/[`Connection::call`]/[`Connection::reply`] -
//! who owns which buffer is `frame::Decoder`'s own doc.

use std::collections::HashSet;

use serde_json::{Map, Value};

use super::control::Control;
use super::frame::{CONTROL_STREAM, Decoder, Frame, FrameError, Kind, encode_frame};
use super::handshake::{
    ClientHello, Handshake, PROTOCOL_VERSION, STEG_MAGIC, ServerHello, check_hello,
};
use crate::client_kind::ClientKind;
use crate::description::{ApiError, Description, Request, Response, validate};
use crate::ids::ClientId;

/// Which end of a connection this value is driving. Nothing here changes
/// what can be *decoded* - decoding is symmetric - only which `send_*`
/// methods a caller is expected to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

/// One decoded item [`Connection::poll`] can hand back.
#[derive(Debug)]
pub enum Incoming {
    /// The first frame of a connection, in either direction - a
    /// `ClientHello`, `ServerHello` or `Refused` payload, undecoded. A
    /// server passes this to [`Connection::accept_or_refuse`]; a client
    /// passes it to [`super::handshake::parse_server_hello`]. Decoding it
    /// needs inputs (the server's own name, a freshly minted [`ClientId`])
    /// only the caller has, so `poll` does not decode it itself.
    Hello(Vec<u8>),
    /// A method call, by name, with its parameters (AC #5) - the id is
    /// matched against the [`Incoming::Reply`] this task answers it with.
    Request { id: u64, request: Request },
    /// A method's result or its error, matched by id (AC #5).
    Reply { id: u64, response: Response },
    /// An event this connection's `Description` declares (R38).
    Event { name: String, body: Value },
    /// An event by a name the `Description` does not declare. Surfaced
    /// distinctly rather than silently treated the same as a known one.
    UnknownEvent { name: String, body: Value },
    /// A control-kind frame on a registered, non-zero stream - a
    /// screen-mode attachment's update (decision-2 section 3). Its JSON
    /// shape belongs to whichever task owns that stream (TASK-1.47); this
    /// crate only demuxes it off the one connection, it does not decode it.
    StreamControl { stream: u32, payload: Vec<u8> },
    /// Opaque bytes on a registered, non-zero stream - terminal input in
    /// M1.
    Data { stream: u32, bytes: Vec<u8> },
    /// A frame for a stream this connection never opened. Surfaced, not
    /// silently dropped or silently accepted - decision-2 section 3's own
    /// invariant is that a correct implementation never produces one of
    /// these (`unknown_stream_frames=0`).
    UnknownStream { stream: u32, kind: Kind },
    /// The frame itself did not decode, or decoded but its payload was not
    /// valid JSON where JSON was expected.
    ///
    /// **Fatal, same as the [`FrameError`] it can come from
    /// (`frame::Decoder::next_frame`'s own doc): this connection does not
    /// resynchronise on its own.** [`Connection::poll`] called again after
    /// one gives the identical `Malformed` again, for ever - the caller is
    /// expected to drop the connection, not keep polling it.
    Malformed(String),
}

/// The multiplexed connection: open streams, and the one place bytes
/// become [`Incoming`] values and calls become bytes.
pub struct Connection {
    role: Role,
    description: Description,
    decoder: Decoder,
    open_streams: HashSet<u32>,
    next_id: u64,
}

impl Connection {
    #[must_use]
    pub fn new(role: Role, description: Description) -> Self {
        Self {
            role,
            description,
            decoder: Decoder::new(),
            open_streams: HashSet::new(),
            next_id: 1,
        }
    }

    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// Appends bytes this connection's owner read from its own transport.
    /// The caller owns the buffer it read into; this only ever borrows it.
    pub fn receive(&mut self, bytes: &[u8]) {
        self.decoder.push(bytes);
    }

    /// Opens a stream, so frames arriving on it are no longer
    /// [`Incoming::UnknownStream`]. Deliberately a separate, explicit call:
    /// decision-2 section 3's ordering rule ("queue the attach result
    /// first, register the attachment's queue second") is a
    /// calling-convention the owner of a stream (TASK-1.47) must follow by
    /// sequencing its own reply before its own `open_stream`; this type
    /// only makes that order possible, it does not enforce it.
    pub fn open_stream(&mut self, stream: u32) {
        self.open_streams.insert(stream);
    }

    pub fn close_stream(&mut self, stream: u32) {
        self.open_streams.remove(&stream);
    }

    /// Drains one decoded item, or `None` meaning "need more bytes": call
    /// [`Connection::receive`] again before calling this again.
    ///
    /// [`Incoming::Malformed`] is fatal (see its own doc): calling this
    /// again afterwards repeats it, rather than ever recovering.
    pub fn poll(&mut self) -> Option<Incoming> {
        let frame = match self.decoder.next_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) => return None,
            Err(err) => return Some(Incoming::Malformed(err.to_string())),
        };

        if frame.kind == Kind::Hello {
            return Some(if frame.stream == CONTROL_STREAM {
                Incoming::Hello(frame.payload)
            } else {
                Incoming::Malformed(format!(
                    "a hello frame arrived on stream {}, not {CONTROL_STREAM}",
                    frame.stream
                ))
            });
        }

        if frame.stream == CONTROL_STREAM {
            return Some(match Control::from_payload(&frame.payload) {
                Ok(Control::Request { id, request }) => Incoming::Request { id, request },
                Ok(Control::Reply { id, response }) => Incoming::Reply { id, response },
                Ok(Control::Event { name, body }) => {
                    if self.description.event(&name).is_some() {
                        Incoming::Event { name, body }
                    } else {
                        Incoming::UnknownEvent { name, body }
                    }
                }
                Err(err) => Incoming::Malformed(err.to_string()),
            });
        }

        if !self.open_streams.contains(&frame.stream) {
            return Some(Incoming::UnknownStream {
                stream: frame.stream,
                kind: frame.kind,
            });
        }

        Some(match frame.kind {
            Kind::Data => Incoming::Data {
                stream: frame.stream,
                bytes: frame.payload,
            },
            Kind::Control => Incoming::StreamControl {
                stream: frame.stream,
                payload: frame.payload,
            },
            Kind::Hello => unreachable!("Kind::Hello is handled above, before the stream check"),
        })
    }

    // --- client-side output --------------------------------------------

    /// Builds the client's first frame. The bytes to write, nothing more.
    ///
    /// # Panics
    /// Never in practice: a hello is a handful of fields, far under the 8
    /// MiB frame cap.
    #[must_use]
    pub fn send_hello(client: &str, kind: ClientKind, label: Option<&str>) -> Vec<u8> {
        let hello = ClientHello::new(client, PROTOCOL_VERSION, kind, label.map(str::to_owned));
        let payload = serde_json::to_vec(&hello).expect("ClientHello always serializes to JSON");
        encode_frame(&Frame::hello(payload)).expect("a hello fits under the frame cap")
    }

    /// Calls a method by name, validated against this connection's own
    /// `Description` before anything is encoded (AC #5: no method is
    /// hand-written - this is the one path every call takes, whether a
    /// generated CLI command or the app). Returns the id to match against
    /// the eventual [`Incoming::Reply`], and the bytes to write.
    ///
    /// # Errors
    /// The [`ApiError`] `description::validate` would give a server asked
    /// the same thing - caught here instead of making a round trip for it.
    ///
    /// # Panics
    /// Never in practice: encoding a validated method call stays far under
    /// the 8 MiB frame cap.
    pub fn call(
        &mut self,
        method: &str,
        params: Map<String, Value>,
    ) -> Result<(u64, Vec<u8>), ApiError> {
        let request = Request {
            method: method.to_owned(),
            params,
        };
        validate(&self.description, &request)?;
        let id = self.next_id;
        self.next_id += 1;
        let payload = Control::Request { id, request }
            .to_payload()
            .expect("Control always serializes to JSON");
        let bytes = encode_frame(&Frame::control(CONTROL_STREAM, payload))
            .expect("a validated method call fits under the frame cap");
        Ok((id, bytes))
    }

    /// Subscribes (`event.watch`, TASK-1.46): the same shape [`Connection::
    /// call`] already has - the id to match against the eventual
    /// [`Incoming::Reply`], and the bytes to write. That reply's own
    /// `Response::Ok` is the tree (`serde_json::from_value::<
    /// crate::TreeSnapshot>`); every [`Incoming::Event`] on this connection
    /// afterward, with no further request, is one of this subscription's
    /// own events (`crate::TreeEvent::parse`) - no id ever matches them,
    /// since an event carries none (decision-2 section 3).
    ///
    /// `target` names the session to watch (`None` for the current
    /// session's own default); `for_` narrows to these event names only
    /// (empty for every event); `all` watches every session on the host.
    ///
    /// # Errors
    /// The same as [`Connection::call`].
    ///
    /// # Panics
    /// Never in practice, the same as [`Connection::call`].
    pub fn watch(
        &mut self,
        target: Option<&str>,
        for_: &[&str],
        all: bool,
    ) -> Result<(u64, Vec<u8>), ApiError> {
        let mut params = Map::new();
        if let Some(target) = target {
            params.insert("target".to_owned(), Value::String(target.to_owned()));
        }
        if !for_.is_empty() {
            params.insert(
                "for".to_owned(),
                Value::Array(
                    for_.iter()
                        .map(|name| Value::String((*name).to_owned()))
                        .collect(),
                ),
            );
        }
        if all {
            params.insert("all".to_owned(), Value::Bool(true));
        }
        self.call("event.watch", params)
    }

    // --- server-side output ---------------------------------------------

    /// Reads the client's first frame and answers it: `Accept` with the
    /// bytes of a `ServerHello`, or `Refuse` with the bytes of a `Refused`
    /// naming both versions (R39). `client_id` is minted by the caller
    /// through the kernel (`Ids::next_client`, TASK-1.42) before this is
    /// called - this type never mints one itself, since deciding the next
    /// id is the kernel's behaviour, not a shape (`ids.rs`'s own doc).
    ///
    /// # Panics
    /// Never in practice: a `ServerHello`/`Refused` stays far under the 8
    /// MiB frame cap.
    #[must_use]
    pub fn accept_or_refuse(
        payload: &[u8],
        server_name: &str,
        api: &str,
        client_id: ClientId,
    ) -> (Handshake, Vec<u8>) {
        let handshake = check_hello(payload, PROTOCOL_VERSION);
        let hello_payload = match &handshake {
            Handshake::Accept { .. } => serde_json::to_vec(&ServerHello {
                steg: STEG_MAGIC.to_owned(),
                protocol: PROTOCOL_VERSION,
                server: server_name.to_owned(),
                api: api.to_owned(),
                client: client_id,
            })
            .expect("ServerHello always serializes to JSON"),
            Handshake::Refuse(refusal) => {
                serde_json::to_vec(&refusal.to_wire()).expect("Refused always serializes to JSON")
            }
        };
        let bytes = encode_frame(&Frame::hello(hello_payload))
            .expect("a hello or refusal fits under the frame cap");
        (handshake, bytes)
    }

    /// Answers a request, matched by id, with its result or its error
    /// (AC #5).
    ///
    /// # Panics
    /// Never in practice: a response stays far under the 8 MiB frame cap.
    #[must_use]
    pub fn reply(id: u64, response: Response) -> Vec<u8> {
        let payload = Control::Reply { id, response }
            .to_payload()
            .expect("Control always serializes to JSON");
        encode_frame(&Frame::control(CONTROL_STREAM, payload))
            .expect("a reply fits under the frame cap")
    }

    /// Emits an event on stream 0. Carries no id, so a long-lived
    /// subscription never blocks a request on the same connection
    /// (decision-2 section 3).
    ///
    /// # Panics
    /// Never in practice: an event stays far under the 8 MiB frame cap,
    /// except a pathologically large body, which is this call's caller's
    /// to avoid - the same cap `send_data` enforces explicitly, below.
    #[must_use]
    pub fn emit_event(name: &str, body: Value) -> Vec<u8> {
        let payload = Control::Event {
            name: name.to_owned(),
            body,
        }
        .to_payload()
        .expect("Control always serializes to JSON");
        encode_frame(&Frame::control(CONTROL_STREAM, payload))
            .expect("an event fits under the frame cap")
    }

    // --- both ends --------------------------------------------------------

    /// Sends opaque bytes on an attachment's stream - pane input in M1.
    ///
    /// # Errors
    /// A [`FrameError::Malformed`] when `stream` was never opened on this
    /// connection, or [`FrameError::TooLarge`] when `bytes` is over the
    /// frame cap - both refused before anything is encoded.
    pub fn send_data(&self, stream: u32, bytes: &[u8]) -> Result<Vec<u8>, FrameError> {
        if !self.open_streams.contains(&stream) {
            return Err(FrameError::Malformed(format!(
                "stream {stream} was never opened on this connection"
            )));
        }
        encode_frame(&Frame::data(stream, bytes.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Connection, Incoming, Kind, Role};
    use crate::description::{Description, Event, Version};

    /// This crate's `tests/connection.rs` integration test covers the
    /// full client/server handshake-and-call round trip (AC #2, #4, #5)
    /// end to end; these unit tests cover the smaller per-case decisions
    /// `poll` makes that do not need two `Connection`s shuttling bytes at
    /// once.
    fn description_with_one_method_and_one_event() -> Description {
        Description {
            api_version: Version { major: 1, minor: 0 },
            methods: Vec::new(),
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
    fn an_event_the_description_declares_arrives_as_a_known_event() {
        let mut connection =
            Connection::new(Role::Client, description_with_one_method_and_one_event());
        let bytes = Connection::emit_event("pane.closed", json!({"pane": "00000001"}));
        connection.receive(&bytes);
        assert!(matches!(connection.poll(), Some(Incoming::Event { .. })));
    }

    #[test]
    fn an_event_the_description_does_not_declare_arrives_as_unknown() {
        let mut connection =
            Connection::new(Role::Client, description_with_one_method_and_one_event());
        let bytes = Connection::emit_event("made.up", json!({}));
        connection.receive(&bytes);
        assert!(matches!(
            connection.poll(),
            Some(Incoming::UnknownEvent { .. })
        ));
    }

    /// decision-2 section 3's own invariant, from the other side: a frame
    /// for a stream this connection never opened is surfaced, not
    /// silently dropped or silently accepted.
    #[test]
    fn data_on_an_unregistered_stream_is_surfaced_as_unknown_stream() {
        use super::super::frame::{Frame, encode_frame};

        let mut connection =
            Connection::new(Role::Server, description_with_one_method_and_one_event());
        let bytes = encode_frame(&Frame::data(9, b"typed while attached".to_vec())).unwrap();
        connection.receive(&bytes);
        assert!(matches!(
            connection.poll(),
            Some(Incoming::UnknownStream {
                stream: 9,
                kind: Kind::Data
            })
        ));
    }

    #[test]
    fn data_on_a_registered_stream_arrives_as_data() {
        use super::super::frame::{Frame, encode_frame};

        let mut connection =
            Connection::new(Role::Server, description_with_one_method_and_one_event());
        connection.open_stream(9);
        let bytes = encode_frame(&Frame::data(9, b"hello".to_vec())).unwrap();
        connection.receive(&bytes);
        assert!(matches!(
            connection.poll(),
            Some(Incoming::Data { stream: 9, .. })
        ));
    }

    #[test]
    fn a_control_frame_on_a_registered_non_zero_stream_arrives_as_stream_control() {
        use super::super::frame::{Frame, encode_frame};

        let mut connection =
            Connection::new(Role::Client, description_with_one_method_and_one_event());
        connection.open_stream(4);
        let bytes = encode_frame(&Frame::control(4, b"{\"seq\":1}".to_vec())).unwrap();
        connection.receive(&bytes);
        assert!(matches!(
            connection.poll(),
            Some(Incoming::StreamControl { stream: 4, .. })
        ));
    }

    #[test]
    fn send_data_refuses_an_unregistered_stream() {
        let connection = Connection::new(Role::Client, description_with_one_method_and_one_event());
        assert!(connection.send_data(2, b"hi").is_err());
    }

    #[test]
    fn poll_returns_none_when_more_bytes_are_needed() {
        let mut connection =
            Connection::new(Role::Client, description_with_one_method_and_one_event());
        connection.receive(&[0, 0, 0]); // less than a 4-byte length prefix
        assert!(connection.poll().is_none());
    }
}
