//! 4-byte big-endian length-prefixed frame codec. Plan §3.1: "Electron Main
//! 与 Rust Core 使用 4-byte 长度前缀的 JSON-RPC 2.0 stdio." Plan §3.2 also
//! requires the protocol to configure a max frame size so an untrusted or
//! misbehaving peer cannot force unbounded buffering — `FrameDecoder`
//! rejects an oversized length as soon as the 4-byte header is seen, before
//! ever buffering the claimed payload.

use std::io::{self, Read, Write};

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    Io(String),
    FrameTooLarge { len: u32, max: u32 },
    UnexpectedEof,
}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        FrameError::Io(e.to_string())
    }
}

pub fn encode_frame(payload: &[u8]) -> Vec<u8> {
    let mut framed = Vec::with_capacity(4 + payload.len());
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(payload);
    framed
}

pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), FrameError> {
    writer.write_all(&encode_frame(payload))?;
    writer.flush()?;
    Ok(())
}

/// Reads exactly one frame synchronously from a blocking `Read`, enforcing
/// `max_frame_len`. Prefer `FrameDecoder` when bytes arrive incrementally
/// (e.g. from an async stdio pipe) rather than one frame at a time.
pub fn read_frame<R: Read>(reader: &mut R, max_frame_len: u32) -> Result<Vec<u8>, FrameError> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(FrameError::UnexpectedEof);
        }
        Err(e) => return Err(FrameError::Io(e.to_string())),
    }
    let len = u32::from_be_bytes(len_buf);
    if len > max_frame_len {
        return Err(FrameError::FrameTooLarge {
            len,
            max: max_frame_len,
        });
    }
    let mut payload = vec![0u8; len as usize];
    match reader.read_exact(&mut payload) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            return Err(FrameError::UnexpectedEof);
        }
        Err(e) => return Err(FrameError::Io(e.to_string())),
    }
    Ok(payload)
}

/// Incremental decoder for bytes that arrive in arbitrary chunks. Push
/// whatever was just read off the pipe, then drain however many complete
/// frames are now available; a partial frame stays buffered until the rest
/// arrives.
pub struct FrameDecoder {
    max_frame_len: u32,
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn new(max_frame_len: u32) -> Self {
        Self {
            max_frame_len,
            buffer: Vec::new(),
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Drains every complete frame currently buffered, in arrival order.
    /// Returns an error — without consuming anything — as soon as a
    /// length prefix claiming more than `max_frame_len` is seen, even if
    /// the full payload has not arrived yet.
    pub fn drain_frames(&mut self) -> Result<Vec<Vec<u8>>, FrameError> {
        let mut frames = Vec::new();
        loop {
            if self.buffer.len() < 4 {
                break;
            }
            let len = u32::from_be_bytes(self.buffer[0..4].try_into().unwrap());
            if len > self.max_frame_len {
                return Err(FrameError::FrameTooLarge {
                    len,
                    max: self.max_frame_len,
                });
            }
            let total = 4 + len as usize;
            if self.buffer.len() < total {
                break;
            }
            frames.push(self.buffer[4..total].to_vec());
            self.buffer.drain(0..total);
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn write_then_read_round_trips_a_frame() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"hello").unwrap();
        let mut cursor = Cursor::new(buf);
        let payload = read_frame(&mut cursor, 1024).unwrap();
        assert_eq!(payload, b"hello");
    }

    #[test]
    fn read_frame_handles_two_frames_back_to_back() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"first").unwrap();
        write_frame(&mut buf, b"second").unwrap();
        let mut cursor = Cursor::new(buf);
        assert_eq!(read_frame(&mut cursor, 1024).unwrap(), b"first");
        assert_eq!(read_frame(&mut cursor, 1024).unwrap(), b"second");
    }

    #[test]
    fn read_frame_rejects_oversized_length() {
        let buf = encode_frame(b"0123456789");
        let mut cursor = Cursor::new(buf);
        let err = read_frame(&mut cursor, 4).unwrap_err();
        assert_eq!(err, FrameError::FrameTooLarge { len: 10, max: 4 });
    }

    #[test]
    fn read_frame_reports_truncated_stream_as_unexpected_eof() {
        let mut buf = encode_frame(b"hello");
        buf.truncate(buf.len() - 2);
        let mut cursor = Cursor::new(buf);
        assert_eq!(
            read_frame(&mut cursor, 1024).unwrap_err(),
            FrameError::UnexpectedEof
        );
    }

    #[test]
    fn decoder_yields_nothing_until_a_frame_is_complete() {
        let full = encode_frame(b"payload");
        let mut decoder = FrameDecoder::new(1024);
        decoder.push(&full[..3]);
        assert_eq!(decoder.drain_frames().unwrap(), Vec::<Vec<u8>>::new());
        decoder.push(&full[3..]);
        assert_eq!(decoder.drain_frames().unwrap(), vec![b"payload".to_vec()]);
    }

    #[test]
    fn decoder_drains_multiple_concatenated_frames_in_order() {
        let mut concatenated = encode_frame(b"one");
        concatenated.extend(encode_frame(b"two"));
        let mut decoder = FrameDecoder::new(1024);
        decoder.push(&concatenated);
        assert_eq!(
            decoder.drain_frames().unwrap(),
            vec![b"one".to_vec(), b"two".to_vec()]
        );
    }

    #[test]
    fn decoder_rejects_oversized_length_without_waiting_for_payload() {
        let mut decoder = FrameDecoder::new(4);
        // Only the 4-byte header, claiming a 10-byte payload that never
        // arrives — the decoder must not block waiting for it.
        decoder.push(&10u32.to_be_bytes());
        let err = decoder.drain_frames().unwrap_err();
        assert_eq!(err, FrameError::FrameTooLarge { len: 10, max: 4 });
    }

    #[test]
    fn decoder_leaves_a_trailing_partial_frame_buffered() {
        let mut concatenated = encode_frame(b"complete");
        concatenated.extend(12u32.to_be_bytes());
        concatenated.extend(b"only-part");
        let mut decoder = FrameDecoder::new(1024);
        decoder.push(&concatenated);
        assert_eq!(decoder.drain_frames().unwrap(), vec![b"complete".to_vec()]);
        // The trailing partial frame is still buffered, not lost.
        decoder.push(b"ial");
        assert_eq!(
            decoder.drain_frames().unwrap(),
            vec![b"only-partial".to_vec()]
        );
    }
}
