//! The wire protocol under test in TASK-1.2. Throwaway: see the spike's README.

mod control;
mod delta;
mod frame;
mod hello;
mod snapshot;

pub use control::{
    AttachParams, AttachResult, CaptureParams, CaptureResult, Control, DataMode, OpenParams,
    OpenResult, ResizeParams, ScrollbackParams, ScrollbackResult, Stats,
};
pub use delta::{Delta, RowChange, Update, apply, diff_screen};
pub use frame::{
    CONTROL_STREAM, Frame, FrameReader, FrameWriter, Framing, Kind, MAX_FRAME, MAX_PAYLOAD,
    WireError,
};
pub use hello::{
    ClientHello, Handshake, PROTOCOL_VERSION, Refusal, Refused, STEG_MAGIC, ServerHello,
    check_hello, shake_hands,
};
pub use snapshot::{
    Cursor, FLAG_BOLD, FLAG_DIM, FLAG_INVERSE, FLAG_ITALIC, FLAG_UNDERLINE, Row, Run, Snapshot,
    render_rows,
};
