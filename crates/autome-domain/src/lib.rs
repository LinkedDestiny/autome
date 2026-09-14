//! autome-domain: pure business types and state reducers for Autome 2.0.
//!
//! Plan D2: Rust Core is the sole authority over business state and legal
//! transitions. This crate deliberately has no I/O, no async runtime, and no
//! dependency on SQLite/IPC/Harness — everything here is deterministic and
//! unit-testable in isolation. The application service (`automed`) owns
//! persistence, scheduling and adapters, and must route all state changes
//! through these reducers rather than mutating projections directly.

pub mod attempt;
pub mod certificate;
pub mod completion;
pub mod config;
pub mod contract;
pub mod delivery;
pub mod evidence;
pub mod graph;
pub mod model_selection;
pub mod node;
pub mod policy_restart;
pub mod project;
pub mod project_intent;
pub mod readiness;
pub mod requirement;
pub mod review;
pub mod run;
pub mod skill;
pub mod task;
