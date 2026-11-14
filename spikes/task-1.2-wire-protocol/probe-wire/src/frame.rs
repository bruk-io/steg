//! Axis 1, the byte framing: how a frame's end is found on a stream nobody delimits for us.
//!
//! Two candidates behind one enum, so every other module is written once:
//!
//! - `Framing::Ndjson` is TASK-1.3's stand-in: one JSON object per line, terminal bytes
//!   base64 inside it.
//! - `Framing::Length` is the candidate: a 4-byte big-endian length (tokio-util's
//!   `LengthDelimitedCodec`), then a 5-byte header, then the payload, which is UTF-8 JSON for
//!   a control or hello frame and opaque bytes for a terminal data frame.
//!
//! A frame carries a stream id so control calls, events and several attachments' terminal
//! output share one connection (topology T1).

use base64::Engine as _;
use bytes::{Buf as _, BufMut as _, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::codec::{Decoder as _, Encoder as _, LengthDelimitedCodec};

/// The largest frame body either framing will encode or decode, header included. A hello that
/// declares more than this is refused before anything is allocated for it (`tests/version.rs`).
pub const MAX_FRAME: usize = 8 * 1024 * 1024;

/// The length framing's header: a 4-byte stream id and a 1-byte kind, inside the length.
const HEADER_LEN: usize = 5;

/// The largest payload a frame can carry, which is `MAX_FRAME` less the header, so the cap means
/// the same thing under both framings.
pub const MAX_PAYLOAD: usize = MAX_FRAME - HEADER_LEN;

/// Stream 0 carries the handshake, control requests and responses, and server events.
pub const CONTROL_STREAM: u32 = 0;

/// What a frame's payload is. The kind, not the stream id, says how to read the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// UTF-8 JSON: the first frame of a connection, in either direction.
    Hello,
    /// UTF-8 JSON: a control request, response, event, or an attachment update.
    Control,
    /// Opaque bytes: terminal output, or input on its way to a program.
    Data,
}

impl Kind {
    const fn tag(self) -> u8 {
        match self {
            Self::Hello => 0,
            Self::Control => 1,
            Self::Data => 2,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Hello),
            1 => Some(Self::Control),
            2 => Some(Self::Data),
            _ => None,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Hello => "hello",
            Self::Control => "control",
            Self::Data => "data",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "hello" => Some(Self::Hello),
            "control" => Some(Self::Control),
            "data" => Some(Self::Data),
            _ => None,
        }
    }
}

/// One message on the wire. Plain data: no framing is implied by the type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub stream: u32,
    pub kind: Kind,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn control(stream: u32, payload: Vec<u8>) -> Self {
        Self {
            stream,
            kind: Kind::Control,
            payload,
        }
    }

    pub fn hello(payload: Vec<u8>) -> Self {
        Self {
            stream: CONTROL_STREAM,
            kind: Kind::Hello,
            payload,
        }
    }

    pub fn data(stream: u32, payload: Vec<u8>) -> Self {
        Self {
            stream,
            kind: Kind::Data,
            payload,
        }
    }
}

/// Anything the framing layer can refuse.
#[derive(Debug)]
pub enum WireError {
    /// A frame larger than `MAX_FRAME` was declared or written.
    TooLarge {
        size: usize,
        limit: usize,
    },
    /// The bytes are not a frame of this framing.
    Malformed(String),
    Io(std::io::Error),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { size, limit } => {
                write!(f, "frame of {size} bytes is over the {limit} byte limit")
            }
            Self::Malformed(why) => write!(f, "malformed frame: {why}"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for WireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TooLarge { .. } | Self::Malformed(_) => None,
            Self::Io(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for WireError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// The NDJSON envelope. Public only so the two framings can be compared field by field in the
/// framing test; nothing else should parse a frame by hand.
#[derive(Serialize, Deserialize)]
struct Line {
    stream: u32,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    b64: Option<String>,
}

/// Axis 1's two candidates. An enum, not a trait object: the set is closed
/// (`.claude/rules/rust.md`).
pub enum Framing {
    Ndjson,
    Length(Box<LengthDelimitedCodec>),
}

impl Framing {
    /// # Errors
    /// Returns the name back when it is neither framing.
    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "ndjson" => Ok(Self::Ndjson),
            "length" => Ok(Self::length()),
            other => Err(format!(
                "unknown framing {other}, expected ndjson or length"
            )),
        }
    }

    pub fn length() -> Self {
        let codec = LengthDelimitedCodec::builder()
            .big_endian()
            .length_field_length(4)
            .max_frame_length(MAX_FRAME)
            .new_codec();
        Self::Length(Box::new(codec))
    }

    pub const fn name(&self) -> &'static str {
        match self {
            Self::Ndjson => "ndjson",
            Self::Length(_) => "length",
        }
    }

    /// Append one frame's bytes to `out`.
    ///
    /// # Errors
    /// `TooLarge` when the payload is over `MAX_FRAME`, `Malformed` when a control payload is
    /// not valid JSON (the NDJSON framing has to embed it as JSON, not as a string, so that
    /// both framings put the same JSON object on the wire).
    pub fn encode(&mut self, frame: &Frame, out: &mut BytesMut) -> Result<(), WireError> {
        if frame.payload.len() > MAX_PAYLOAD {
            return Err(WireError::TooLarge {
                size: frame.payload.len(),
                limit: MAX_PAYLOAD,
            });
        }
        match self {
            Self::Ndjson => {
                let line = match frame.kind {
                    Kind::Hello | Kind::Control => {
                        let json: serde_json::Value = serde_json::from_slice(&frame.payload)
                            .map_err(|err| WireError::Malformed(format!("{err}")))?;
                        Line {
                            stream: frame.stream,
                            kind: frame.kind.name().to_owned(),
                            json: Some(json),
                            b64: None,
                        }
                    }
                    Kind::Data => Line {
                        stream: frame.stream,
                        kind: frame.kind.name().to_owned(),
                        json: None,
                        b64: Some(base64::engine::general_purpose::STANDARD.encode(&frame.payload)),
                    },
                };
                let encoded = serde_json::to_vec(&line)
                    .map_err(|err| WireError::Malformed(format!("{err}")))?;
                out.reserve(encoded.len() + 1);
                out.put_slice(&encoded);
                out.put_u8(b'\n');
                Ok(())
            }
            Self::Length(codec) => {
                let mut body = BytesMut::with_capacity(frame.payload.len() + HEADER_LEN);
                body.put_u32(frame.stream);
                body.put_u8(frame.kind.tag());
                body.put_slice(&frame.payload);
                codec.encode(body.freeze(), out).map_err(WireError::from)
            }
        }
    }

    /// Take the next whole frame out of `buf`, leaving a partial frame behind.
    ///
    /// # Errors
    /// `TooLarge` or `Malformed` for bytes that are not a frame of this framing. `Ok(None)`
    /// means the buffer holds the start of a frame and no more: read again and call again.
    pub fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Frame>, WireError> {
        match self {
            Self::Ndjson => {
                let Some(end) = buf.iter().position(|byte| *byte == b'\n') else {
                    if buf.len() > MAX_FRAME {
                        return Err(WireError::TooLarge {
                            size: buf.len(),
                            limit: MAX_FRAME,
                        });
                    }
                    return Ok(None);
                };
                let line = buf.split_to(end + 1);
                let decoded: Line = serde_json::from_slice(&line[..end])
                    .map_err(|err| WireError::Malformed(format!("{err}")))?;
                let kind = Kind::from_name(&decoded.kind)
                    .ok_or_else(|| WireError::Malformed(format!("kind {}", decoded.kind)))?;
                let payload = match kind {
                    Kind::Hello | Kind::Control => {
                        let json = decoded
                            .json
                            .ok_or_else(|| WireError::Malformed("no json field".to_owned()))?;
                        serde_json::to_vec(&json)
                            .map_err(|err| WireError::Malformed(format!("{err}")))?
                    }
                    Kind::Data => {
                        let b64 = decoded
                            .b64
                            .ok_or_else(|| WireError::Malformed("no b64 field".to_owned()))?;
                        base64::engine::general_purpose::STANDARD
                            .decode(b64)
                            .map_err(|err| WireError::Malformed(format!("{err}")))?
                    }
                };
                Ok(Some(Frame {
                    stream: decoded.stream,
                    kind,
                    payload,
                }))
            }
            Self::Length(codec) => {
                let Some(mut body) = codec.decode(buf).map_err(WireError::from)? else {
                    return Ok(None);
                };
                if body.len() < HEADER_LEN {
                    return Err(WireError::Malformed(format!("{} byte header", body.len())));
                }
                let stream = body.get_u32();
                let tag = body.get_u8();
                let kind = Kind::from_tag(tag)
                    .ok_or_else(|| WireError::Malformed(format!("kind tag {tag}")))?;
                Ok(Some(Frame {
                    stream,
                    kind,
                    payload: body.to_vec(),
                }))
            }
        }
    }
}

/// The read half of a connection. Generic over the stream, because gate 2 runs the same client
/// over a Unix socket and over a pipe pair it does not control.
pub struct FrameReader<R> {
    reader: R,
    framing: Framing,
    buf: BytesMut,
    eof: bool,
    bytes: u64,
    frames: u64,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    pub fn new(reader: R, framing: Framing) -> Self {
        Self {
            reader,
            framing,
            buf: BytesMut::with_capacity(64 * 1024),
            eof: false,
            bytes: 0,
            frames: 0,
        }
    }

    pub const fn bytes_read(&self) -> u64 {
        self.bytes
    }

    pub const fn frames_read(&self) -> u64 {
        self.frames
    }

    /// The next frame, or `None` at end of stream.
    ///
    /// # Errors
    /// Anything `Framing::decode` refuses, or an I/O error on the stream.
    pub async fn next(&mut self) -> Result<Option<Frame>, WireError> {
        loop {
            if let Some(frame) = self.framing.decode(&mut self.buf)? {
                self.frames += 1;
                return Ok(Some(frame));
            }
            if self.eof {
                return if self.buf.is_empty() {
                    Ok(None)
                } else {
                    Err(WireError::Malformed(format!(
                        "{} bytes left at end of stream",
                        self.buf.len()
                    )))
                };
            }
            let read = self.reader.read_buf(&mut self.buf).await?;
            self.bytes += read as u64;
            if read == 0 {
                self.eof = true;
            }
        }
    }
}

/// The write half of a connection.
pub struct FrameWriter<W> {
    writer: W,
    framing: Framing,
    buf: BytesMut,
    bytes: u64,
    frames: u64,
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
    pub fn new(writer: W, framing: Framing) -> Self {
        Self {
            writer,
            framing,
            buf: BytesMut::with_capacity(64 * 1024),
            bytes: 0,
            frames: 0,
        }
    }

    pub const fn bytes_written(&self) -> u64 {
        self.bytes
    }

    pub const fn frames_written(&self) -> u64 {
        self.frames
    }

    /// Encode one frame and flush it.
    ///
    /// # Errors
    /// Anything `Framing::encode` refuses, or an I/O error, which is what a killed client looks
    /// like from here.
    pub async fn send(&mut self, frame: &Frame) -> Result<(), WireError> {
        self.buf.clear();
        self.framing.encode(frame, &mut self.buf)?;
        self.bytes += self.buf.len() as u64;
        self.frames += 1;
        let bytes = Bytes::copy_from_slice(&self.buf);
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;
        Ok(())
    }
}
