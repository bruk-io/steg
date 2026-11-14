//! Axis 2: the control envelope, and whether TASK-1.3's JSON payload contract survives a change
//! of framing.
//!
//! A request carries an id, a method name and JSON parameters; a response or an error carries
//! the same id; an event carries neither, so a long-lived subscription (`event watch`, doc-2
//! section 8) does not block requests on the same connection. `serde_json::Value` appears here
//! and nowhere else: this module is the edge (`.claude/rules/rust.md`), and the whole point is
//! that the parameters are the API description's JSON, not a Rust type this crate owns.

use serde::{Deserialize, Serialize};

use crate::snapshot::{Row, Snapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Control {
    Request {
        id: u64,
        method: String,
        params: serde_json::Value,
    },
    Response {
        id: u64,
        result: serde_json::Value,
    },
    Error {
        id: u64,
        code: String,
        message: String,
    },
    Event {
        event: String,
        body: serde_json::Value,
    },
}

impl Control {
    /// # Errors
    /// When the value cannot be serialised, which for these types means a non-string map key.
    pub fn to_payload(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// # Errors
    /// When the bytes are not a control message.
    pub fn from_payload(payload: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(payload)
    }
}

/// Axis 4: what the server puts on the attachment's stream after the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataMode {
    /// D1: raw pty bytes, replayed by an emulator in the client.
    Bytes,
    /// D2a: screen updates the server computes, applied to plain rows in the client, with every
    /// row that scrolls off carried too, so the client keeps a scrollback of its own.
    Deltas,
    /// D2b: the same screen updates with no scrollback on the stream at all. A client that wants
    /// scrollback asks for a page of it (`pane.scrollback`). This is what makes a delta stream
    /// cost the screen and the tick rate instead of the output rate.
    Screen,
}

impl DataMode {
    /// # Errors
    /// Returns the name back when it is neither mode.
    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "bytes" => Ok(Self::Bytes),
            "deltas" => Ok(Self::Deltas),
            "screen" => Ok(Self::Screen),
            other => Err(format!(
                "unknown data mode {other}, expected bytes, deltas or screen"
            )),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Deltas => "deltas",
            Self::Screen => "screen",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenParams {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenResult {
    pub pane: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachParams {
    pub pane: u32,
    pub mode: DataMode,
    /// The size this client wants. Carried per attachment, not per pane, so two clients with
    /// different windows can both attach (R7); which size wins is design decision 6 and M3's.
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachResult {
    pub attachment: u32,
    /// The stream id this attachment's terminal output and input use.
    pub stream: u32,
    /// Taken in the same step as the subscription, so the stream resumes at `snapshot.seq + 1`.
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureParams {
    pub pane: u32,
    #[serde(default)]
    pub scrollback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureResult {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollbackParams {
    pub pane: u32,
    /// Absolute row ids, not window positions: row 0 is the first row that ever scrolled off.
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollbackResult {
    pub rows: Vec<Row>,
    /// The absolute id of `rows[0]`. Greater than the `from` that was asked for when the server
    /// has already dropped those rows, which is how a client tells a gap from a full answer.
    pub first_row: u64,
    /// How many rows have ever scrolled off, and how many of them the server still holds.
    pub rows_ever: u64,
    pub held: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeParams {
    pub pane: u32,
    pub cols: u16,
    pub rows: u16,
}

/// What the measurements read out of the server, so a number comes from the server's own
/// counters and not from the client's guess.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub pane: u32,
    pub pty_bytes: u64,
    pub ticks: u64,
    pub attachments: u32,
    pub overflows: u64,
    pub resyncs: u64,
    pub blocked_nanos: u64,
}
