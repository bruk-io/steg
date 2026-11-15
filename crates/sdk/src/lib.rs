//! Steg's public interface (R101): the API description, the shapes that cross
//! the API, and the protocol's logic - encoding, decoding, the handshake and
//! the state of each stream.
//!
//! The sdk does no I/O of its own (R102): a consumer supplies the transport
//! through an interface this crate defines, so it works on any runtime and
//! over any channel a consumer can provide. The `local-socket` feature
//! (declared in `Cargo.toml`) is this crate's own provider for the local
//! socket - finding a server, starting one if there is none, and connecting
//! (R103) - but it is declared and empty today: TASK-1.44 and draft-1 fill it
//! in.
//!
//! Identifiers, the colour set, pane kinds and client kinds are shapes that
//! cross the API, so they are defined once, here (doc-16 R105), and the
//! kernel uses them rather than defining its own. Each of those shapes also
//! has its wire
//! form here - how it serialises and deserialises - and `description` holds
//! the API itself, as one value (decision-1): every method, event and
//! error, and `validate`, the protocol logic that checks a request against
//! it with no I/O.

mod client_kind;
mod colour;
mod description;
mod ids;
mod layout;
mod pane_kind;
mod record;
mod screen;

pub use client_kind::ClientKind;
pub use colour::Colour;
pub use description::{
    ApiError, Description, ErrorCode, ErrorRecord, Event, Field, Method, MethodName, Param,
    Request, Response, Returns, Role, ValueKind, Version, description, validate,
};
pub use ids::{ClientId, PaneId, SessionId, TabId};
pub use layout::{Direction, Layout, Orientation, Ratio, Size};
pub use pane_kind::PaneKind;
pub use record::SessionRecord;
pub use screen::{
    CellStyle, Cursor, CursorShape, Page, Rgb, Row, RowId, Run, Screen, Underline, Update, Wide,
};
