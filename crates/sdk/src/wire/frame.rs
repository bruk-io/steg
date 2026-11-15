//! The byte framing: a length-prefixed frame on one multiplexed connection
//! (decision-2 section 3). Pure data in, pure data out - nothing here reads
//! or writes a transport (doc-16 R102).
//!
//! Ported from `spikes/task-1.2-wire-protocol/probe-wire/src/frame.rs`: the
//! length framing only (the NDJSON stand-in it compared against is
//! superseded - decision-2 chose length framing and the comparison is
//! over), with `tokio`/`bytes` stripped in favour of plain `Vec<u8>`/`&[u8]`
//! (doc-16 R102/R106 forbid them in the sdk's public interface anyway).

/// The largest frame body this crate will encode or decode, header included.
/// A declared length over this is refused before anything is allocated for
/// it (`Decoder::next_frame`'s test).
pub const MAX_FRAME: usize = 8 * 1024 * 1024;

/// The length framing's header: a 4-byte stream id and a 1-byte kind,
/// inside the length.
const HEADER_LEN: usize = 5;

/// The largest payload a frame can carry: `MAX_FRAME` less the header.
pub const MAX_PAYLOAD: usize = MAX_FRAME - HEADER_LEN;

/// Stream 0 carries the handshake, control requests and responses, and
/// server events (decision-2 section 3).
pub const CONTROL_STREAM: u32 = 0;

/// What a frame's payload is. The kind, not the stream id, says how to read
/// the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// UTF-8 JSON: the first frame of a connection, in either direction.
    Hello,
    /// UTF-8 JSON: a control request, response, or event - including a
    /// screen-mode attachment's updates, which travel as control frames on
    /// the attachment's own stream (decision-2 section 3), not as `Data`.
    Control,
    /// Opaque bytes: terminal input, or - in a mode M1 does not use - raw
    /// terminal output.
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
}

/// One message on the wire. Plain data: no framing is implied by the type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub stream: u32,
    pub kind: Kind,
    pub payload: Vec<u8>,
}

impl Frame {
    #[must_use]
    pub fn hello(payload: Vec<u8>) -> Self {
        Self {
            stream: CONTROL_STREAM,
            kind: Kind::Hello,
            payload,
        }
    }

    #[must_use]
    pub fn control(stream: u32, payload: Vec<u8>) -> Self {
        Self {
            stream,
            kind: Kind::Control,
            payload,
        }
    }

    #[must_use]
    pub fn data(stream: u32, payload: Vec<u8>) -> Self {
        Self {
            stream,
            kind: Kind::Data,
            payload,
        }
    }
}

/// Anything the framing layer can refuse.
#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    /// A frame larger than `MAX_FRAME` was declared or written.
    TooLarge { size: usize, limit: usize },
    /// The bytes are not a frame.
    Malformed(String),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { size, limit } => {
                write!(f, "frame of {size} bytes is over the {limit} byte limit")
            }
            Self::Malformed(why) => write!(f, "malformed frame: {why}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Encodes one frame: a 4-byte big-endian length, the 5-byte header, then
/// the payload. The exact bytes a consumer writes to its transport.
///
/// # Errors
/// [`FrameError::TooLarge`] when the payload is over [`MAX_PAYLOAD`].
pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, FrameError> {
    if frame.payload.len() > MAX_PAYLOAD {
        return Err(FrameError::TooLarge {
            size: frame.payload.len(),
            limit: MAX_PAYLOAD,
        });
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked against MAX_PAYLOAD above, which is far below u32::MAX"
    )]
    let body_len = (HEADER_LEN + frame.payload.len()) as u32;
    let mut out = Vec::with_capacity(4 + body_len as usize);
    out.extend_from_slice(&body_len.to_be_bytes());
    out.extend_from_slice(&frame.stream.to_be_bytes());
    out.push(frame.kind.tag());
    out.extend_from_slice(&frame.payload);
    Ok(out)
}

/// Decodes frames out of a byte stream the caller feeds it, one at a time.
///
/// Owns its own accumulation buffer; the caller owns whatever it read bytes
/// into and only ever hands over a borrowed slice through [`Decoder::push`]
/// (doc-16 R102: who owns which buffer, stated once).
#[derive(Debug, Default)]
pub struct Decoder {
    buf: Vec<u8>,
}

impl Decoder {
    #[must_use]
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Appends newly read bytes.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Takes the next whole frame out of the buffer, leaving a partial frame
    /// (if any) behind.
    ///
    /// # Errors
    /// [`FrameError::TooLarge`] when the declared length is over
    /// [`MAX_FRAME`] - checked as soon as the 4-byte length prefix is
    /// available and before anything past it is read, so nothing is ever
    /// allocated for the declared body. [`FrameError::Malformed`] for a
    /// header that does not name a known [`Kind`].
    ///
    /// `Ok(None)` means the buffer holds the start of a frame and no more:
    /// read more bytes, `push` them, and call again.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, FrameError> {
        if self.buf.len() < 4 {
            return Ok(None);
        }
        let declared_len =
            u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        if declared_len > MAX_FRAME {
            return Err(FrameError::TooLarge {
                size: declared_len,
                limit: MAX_FRAME,
            });
        }
        if declared_len < HEADER_LEN {
            return Err(FrameError::Malformed(format!(
                "{declared_len} byte body is smaller than the {HEADER_LEN} byte header"
            )));
        }
        let total = 4 + declared_len;
        if self.buf.len() < total {
            return Ok(None);
        }

        let stream = u32::from_be_bytes([self.buf[4], self.buf[5], self.buf[6], self.buf[7]]);
        let tag = self.buf[8];
        let kind = Kind::from_tag(tag)
            .ok_or_else(|| FrameError::Malformed(format!("unknown frame kind tag {tag}")))?;
        let payload = self.buf[9..total].to_vec();

        self.buf.drain(0..total);
        Ok(Some(Frame {
            stream,
            kind,
            payload,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{CONTROL_STREAM, Decoder, Frame, FrameError, Kind, MAX_FRAME, encode_frame};

    #[test]
    fn a_frame_round_trips_through_encode_and_decode() {
        let frame = Frame::control(3, b"hello".to_vec());
        let bytes = encode_frame(&frame).unwrap();
        let mut decoder = Decoder::new();
        decoder.push(&bytes);
        assert_eq!(decoder.next_frame().unwrap(), Some(frame));
        assert_eq!(decoder.next_frame().unwrap(), None);
    }

    #[test]
    fn decode_waits_for_more_bytes_a_byte_at_a_time() {
        let frame = Frame::data(7, b"abcdef".to_vec());
        let bytes = encode_frame(&frame).unwrap();
        let mut decoder = Decoder::new();
        for (i, byte) in bytes.iter().enumerate() {
            decoder.push(std::slice::from_ref(byte));
            let decoded = decoder.next_frame().unwrap();
            if i + 1 < bytes.len() {
                assert_eq!(decoded, None, "frame should not be complete yet");
            } else {
                assert_eq!(decoded, Some(frame.clone()));
            }
        }
    }

    #[test]
    fn two_frames_back_to_back_decode_in_order() {
        let first = Frame::hello(b"{}".to_vec());
        let second = Frame::data(1, b"xy".to_vec());
        let mut bytes = encode_frame(&first).unwrap();
        bytes.extend(encode_frame(&second).unwrap());
        let mut decoder = Decoder::new();
        decoder.push(&bytes);
        assert_eq!(decoder.next_frame().unwrap(), Some(first));
        assert_eq!(decoder.next_frame().unwrap(), Some(second));
        assert_eq!(decoder.next_frame().unwrap(), None);
    }

    #[test]
    fn encode_refuses_a_payload_over_the_cap() {
        let frame = Frame::data(0, vec![0_u8; super::MAX_PAYLOAD + 1]);
        assert_eq!(
            encode_frame(&frame),
            Err(FrameError::TooLarge {
                size: super::MAX_PAYLOAD + 1,
                limit: super::MAX_PAYLOAD,
            })
        );
    }

    /// decision-2's cap, and the clause this task adds: refused with nothing
    /// allocated for the declared body. Only the 4-byte length prefix is
    /// ever pushed - if `next_frame` tried to allocate or wait for
    /// `0xFFFF_FFFF` bytes it would hang or abort this test, not return.
    #[test]
    fn an_oversized_declared_length_is_refused_before_anything_past_it_is_read() {
        let mut decoder = Decoder::new();
        decoder.push(&0xFFFF_FFFF_u32.to_be_bytes());
        assert_eq!(
            decoder.next_frame(),
            Err(FrameError::TooLarge {
                size: 0xFFFF_FFFF,
                limit: MAX_FRAME,
            })
        );
    }

    #[test]
    fn an_unknown_kind_tag_is_malformed() {
        let mut bytes = 5_u32.to_be_bytes().to_vec(); // header-only body
        bytes.extend_from_slice(&CONTROL_STREAM.to_be_bytes());
        bytes.push(99); // not a known Kind tag
        let mut decoder = Decoder::new();
        decoder.push(&bytes);
        assert!(matches!(
            decoder.next_frame(),
            Err(FrameError::Malformed(_))
        ));
    }

    #[test]
    fn frame_constructors_set_the_expected_kind_and_stream() {
        assert_eq!(Frame::hello(vec![]).stream, CONTROL_STREAM);
        assert_eq!(Frame::hello(vec![]).kind, Kind::Hello);
        assert_eq!(Frame::control(2, vec![]).kind, Kind::Control);
        assert_eq!(Frame::data(2, vec![]).kind, Kind::Data);
    }
}
