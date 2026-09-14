//! Electron Main <-> Rust Core protocol per plan §3.1/§3.2: framed JSON-RPC
//! 2.0 over stdio with a 4-byte big-endian length prefix. Plan §3.1: "stdout
//! 只传协议帧，stderr 只传 Core 诊断日志" — enforced by convention at the
//! call site (whoever wires this to real stdio must keep the two streams
//! separate); this module only knows about `Read`/`Write`, not which file
//! descriptor they are.

pub mod envelope;
pub mod framing;

pub use envelope::{Command, Event, PROTOCOL_VERSION};
pub use framing::{FrameDecoder, FrameError, encode_frame, read_frame, write_frame};
