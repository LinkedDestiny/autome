//! automed: the application service behind the Autome 2.0 desktop app.
//!
//! Authority: `docs/plans/2026-09-15-autome-2.0-technical-design.md` in the
//! sibling 1.x repository (read-only reference, ADR-0001).
//!
//! Boundary (plan D2/D3): `autome-domain` decides *what* is legal; this crate
//! does everything that touches the outside world and owns no business rules
//! of its own. Electron renders what this crate projects and holds no state.
//!
//! - [`ipc`] — framed JSON-RPC over stdio, the only channel to Electron Main.
//! - [`dispatch`] — the command table: every method the Renderer can call.
//! - [`store`] — SQLite: project registry, task index, session ledger, the
//!   user's decisions, and the event stream the UI resyncs against.
//! - [`config_io`] — the two TOML files, with sparse project overrides
//!   preserved across a write.
//! - [`git`] — the operations table from design §6, and nothing else.
//! - [`init`] — the `.autome/` scaffold, including the session wrapper script.
//! - [`env_probe`] — the four local components.
//! - [`launcher`] — prompt construction, the CLI adapter table, and starting
//!   a session in a visible terminal.
//! - [`scheduler`] — the one place a transition is applied and acted on.
//! - [`skills`] — the read-only skill inventory scan.

pub mod config_io;
pub mod dispatch;
pub mod env_probe;
pub mod git;
pub mod init;
pub mod ipc;
pub mod launcher;
pub mod scheduler;
pub mod skills;
pub mod store;
pub mod stream_render;
