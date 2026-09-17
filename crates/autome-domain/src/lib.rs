//! autome-domain: pure business types and state transitions for Autome 2.0.
//!
//! Authority: `docs/plans/2026-09-15-autome-2.0-technical-design.md` in the
//! sibling 1.x repository (read-only reference, see ADR-0001), together with
//! its requirements companion. The 09-13 greenfield plan it supersedes is
//! historical; see §6 of the decision record for what was dropped.
//!
//! Boundary (plan D2): the Rust core is the sole authority over business state
//! and legal transitions. This crate has no I/O, no async runtime and no
//! dependency on SQLite, IPC or the harnesses — everything here is
//! deterministic and unit-testable in isolation. `automed` owns persistence,
//! scheduling, Git and the session launcher, and must route state changes
//! through these functions rather than mutating projections directly.
//!
//! The modules, in dependency order:
//!
//! - [`role`] — the five Loop roles and two CLI runtimes everything routes on.
//! - [`config`] — global defaults, sparse project overrides, resolution and
//!   validation (SAME-MODEL, skill visibility).
//! - [`status_block`] — the design-document parser that is the source of truth
//!   for task progress.
//! - [`task`] — the task state machine and its transition table.
//! - [`project`] — projects, slugs and repository-relative paths.
//! - [`session`] — session records and the exit-marker protocol.
//! - [`skill`] — the read-only skill inventory.
//! - [`environment`] — the four checked local components.
//! - [`protocol`] — the protocol as versioned data, and the kernel contract.
//! - [`metrics`] — what one session and one task cost and produced.
//! - [`lesson`] — the retro round's output schema and its cross-task key.
//! - [`changelog`] — one protocol version's changes, with predicted impact.
//! - [`yaml_lite`] — the small block reader `lesson` and `changelog` share.

pub mod changelog;
pub mod config;
pub mod environment;
pub mod lesson;
pub mod metrics;
pub mod project;
pub mod protocol;
pub mod role;
pub mod session;
pub mod skill;
pub mod status_block;
pub mod task;
pub mod yaml_lite;
