//! automed: the application service. Owns SQLite persistence, the event
//! journal, Harness adapters, brokers and the versioned JSON-RPC stdio
//! protocol spoken to the Electron Main process (plan §3, §4, §11).
//!
//! This crate is intentionally a stub at M0 start: it exists so the
//! workspace builds end-to-end from day one and so every subsequent piece
//! (SQLite schema, IPC framing, Codex adapter) has a real place to land
//! instead of accreting in `autome-domain`.

pub mod ipc;
pub mod store;
