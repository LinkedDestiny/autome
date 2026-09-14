//! autome-domain: pure business types and state reducers for Autome 2.0.
//!
//! Plan D2: Rust Core is the sole authority over business state and legal
//! transitions. This crate deliberately has no I/O, no async runtime, and no
//! dependency on SQLite/IPC/Harness — everything here is deterministic and
//! unit-testable in isolation. The application service (`automed`) owns
//! persistence, scheduling and adapters, and must route all state changes
//! through these reducers rather than mutating projections directly.

pub mod certificate;
pub mod completion;
pub mod contract;
pub mod evidence;
pub mod graph;
pub mod node;
pub mod project;
pub mod requirement;
pub mod run;
