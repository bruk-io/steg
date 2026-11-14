//! The handshake, and the version rule (R39, AC #3).
//!
//! The one thing that is frozen for every future version: the first frame of a connection is a
//! length-prefixed frame (4-byte big-endian length, the header of `frame.rs`) whose payload is
//! UTF-8 JSON holding `steg` and `protocol`. Unknown keys are ignored, so a later version may
//! add to it. That is what lets a mismatch be answered with a message naming both versions
//! rather than a parse error (TASK-1.9's acceptance criterion 4).

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::frame::{Frame, FrameReader, FrameWriter, Kind};

/// The version of everything in this crate: the framing, the control envelope and the terminal
/// data representation. They ship in one binary (R61) and a mismatch is not partly recoverable,
/// so they carry one number. The API's own version lives in the self-description (TASK-1.3).
pub const PROTOCOL_VERSION: u32 = 1;

/// Present so that a connection to something that is not steg fails as a refusal with a reason,
/// not as a confusing JSON error.
pub const STEG_MAGIC: &str = "steg";

/// The client's first frame. `#[serde(default)]` on nothing: both fields are required, and any
/// other field is ignored, which is the forward-compatibility promise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHello {
    pub steg: String,
    pub protocol: u32,
    pub client: String,
}

impl ClientHello {
    pub fn new(client: &str, protocol: u32) -> Self {
        Self {
            steg: STEG_MAGIC.to_owned(),
            protocol,
            client: client.to_owned(),
        }
    }
}

/// The server's answer when the two can work together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    pub steg: String,
    pub protocol: u32,
    pub server: String,
    /// The API description's own version, which is TASK-1.3's to define. Carried here so a
    /// client can be newer in methods without a framing mismatch.
    pub api: String,
}

/// The server's answer when they cannot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refused {
    pub steg: String,
    pub error: String,
    pub client_protocol: Option<u32>,
    pub server_protocol: u32,
    pub message: String,
}

/// Why a handshake was refused. One variant per thing the client can tell apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Both versions are known and they differ (R39).
    Mismatch { client: u32, server: u32 },
    /// The first frame was not a hello this or any version could read.
    Malformed { reason: String, server: u32 },
}

impl Refusal {
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Mismatch { .. } => "protocol-mismatch",
            Self::Malformed { .. } => "malformed-hello",
        }
    }

    /// The message a person reads. R39 requires both versions to be named, so both are in the
    /// text of every variant, including the malformed one, where the client's is unknown.
    pub fn message(&self) -> String {
        match self {
            Self::Mismatch { client, server } => format!(
                "steg protocol mismatch: the client speaks version {client} and this server speaks version {server}"
            ),
            Self::Malformed { reason, server } => format!(
                "steg could not read the client's hello ({reason}); this server speaks protocol version {server} and the client's version is unknown"
            ),
        }
    }

    pub fn to_wire(&self) -> Refused {
        let (client_protocol, server_protocol) = match self {
            Self::Mismatch { client, server } => (Some(*client), *server),
            Self::Malformed { server, .. } => (None, *server),
        };
        Refused {
            steg: STEG_MAGIC.to_owned(),
            error: self.error_code().to_owned(),
            client_protocol,
            server_protocol,
            message: self.message(),
        }
    }
}

/// What the server does with a first frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handshake {
    Accept { client: String, protocol: u32 },
    Refuse(Refusal),
}

/// The whole version decision, as a function of the bytes: no I/O, so `tests/version.rs` proves
/// it directly (`.claude/rules/rust.md`, core and shell).
#[must_use]
pub fn check_hello(payload: &[u8], server_protocol: u32) -> Handshake {
    let hello: ClientHello = match serde_json::from_slice(payload) {
        Ok(hello) => hello,
        Err(err) => {
            return Handshake::Refuse(Refusal::Malformed {
                reason: format!("{err}"),
                server: server_protocol,
            });
        }
    };
    if hello.steg != STEG_MAGIC {
        return Handshake::Refuse(Refusal::Malformed {
            reason: format!("the magic is {:?}, not {STEG_MAGIC:?}", hello.steg),
            server: server_protocol,
        });
    }
    if hello.protocol == server_protocol {
        Handshake::Accept {
            client: hello.client,
            protocol: hello.protocol,
        }
    } else {
        Handshake::Refuse(Refusal::Mismatch {
            client: hello.protocol,
            server: server_protocol,
        })
    }
}

/// The client's side of the handshake. One round trip, before anything else is sent, whatever the
/// transport underneath.
///
/// # Errors
/// The server's own message when it refuses (which names both versions), or a description of why
/// its answer could not be read.
pub async fn shake_hands<R, W>(
    frames: &mut FrameReader<R>,
    out: &mut FrameWriter<W>,
    client: &str,
    protocol: u32,
) -> Result<ServerHello, String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let hello = ClientHello::new(client, protocol);
    let payload = serde_json::to_vec(&hello).map_err(|err| err.to_string())?;
    out.send(&Frame::hello(payload))
        .await
        .map_err(|err| err.to_string())?;
    let answer = frames
        .next()
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "the server closed the connection without answering the hello".to_owned())?;
    if answer.kind != Kind::Hello {
        return Err(format!(
            "the server's first frame is a {:?} frame",
            answer.kind
        ));
    }
    if let Ok(server) = serde_json::from_slice::<ServerHello>(&answer.payload) {
        return Ok(server);
    }
    match serde_json::from_slice::<Refused>(&answer.payload) {
        Ok(refused) => Err(refused.message),
        Err(err) => Err(format!("the server's hello could not be read: {err}")),
    }
}
