//! Steg's public interface (R101): the API description, the shapes that cross
//! the API, and the protocol's logic - encoding, decoding, the handshake and
//! the state of each stream.
//!
//! The sdk does no I/O of its own (R102): a consumer supplies the transport
//! through an interface this crate defines, so it works on any runtime and
//! over any channel a consumer can provide. The `local-socket` feature is
//! this crate's own provider for the local socket - finding a server,
//! starting one if there is none, and connecting (R103) - off by default,
//! and the one place in this crate allowed real I/O.
//!
//! Identifiers, the colour set, pane kinds, client kinds, a pane's
//! program, its status and the settings registry are shapes that cross
//! the API, so they are defined once, here (doc-16 R105), and the kernel
//! uses them rather than defining its own.
//! Each of those shapes also has its wire form here - how it serialises
//! and deserialises - and `description` holds the API itself, as one
//! value (decision-1): every method, event and error, and `validate`, the
//! protocol logic that checks a request against it with no I/O.

mod client_kind;
mod colour;
mod description;
mod ids;
mod keys;
mod layout;
mod pane_kind;
mod program;
mod record;
mod screen;
mod setting;
mod status;
mod tree_event;
mod wire;

pub use client_kind::ClientKind;
pub use colour::Colour;
pub use description::{
    ApiError, Description, ErrorCode, ErrorRecord, Event, Field, Follows, Method, MethodName,
    Param, Request, Response, Returns, Role, ValueKind, Version, description, validate,
};
pub use ids::{ClientId, PaneId, SessionId, TabId};
pub use keys::{key_names, named_key_bytes};
pub use layout::{Direction, Layout, Orientation, Ratio, Size};
pub use pane_kind::PaneKind;
pub use program::{Program, ProgramStatus};
pub use record::{
    ClientRecord, PaneRecord, ServerRecord, SessionRecord, SessionTree, SettingRecord, TabRecord,
    TabTree, TreeSnapshot,
};
pub use screen::{
    CellStyle, Cursor, CursorShape, EndReason, Page, PaneAttached, Rgb, Row, RowId, Run, Screen,
    ScreenUpdate, Underline, Wide,
};
pub use setting::{Setting, settings};
pub use status::Status;
pub use tree_event::TreeEvent;
pub use wire::{
    CONTROL_STREAM, ClientHello, Connection, Control, Decoder, Frame, FrameError, Handshake,
    Incoming, Kind, MAX_FRAME, MAX_PAYLOAD, PROTOCOL_VERSION, Refusal, Refused, Role as WireRole,
    STEG_MAGIC, ServerHello, check_hello, encode_frame, parse_server_hello,
};

#[cfg(feature = "local-socket")]
mod local_socket;
#[cfg(feature = "local-socket")]
pub use local_socket::{
    ConnectError, ConnectOptions, connect, connect_handshaken, run_fake_server_for_tests,
};

mod paths;
pub use paths::socket_path;
