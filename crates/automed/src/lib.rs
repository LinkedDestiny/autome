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
//! - [`dispatch_protocol`] — the protocol repository and version page half of
//!   that table.
//! - [`store`] — SQLite: project registry, task index, session ledger, the
//!   user's decisions, and the event stream the UI resyncs against.
//! - [`config_io`] — the two TOML files, with sparse project overrides
//!   preserved across a write.
//! - [`git`] — the operations table from design §6, and nothing else.
//! - [`guards`] — what the core checks after a session, instead of asking a
//!   round to remember.
//! - [`init`] — the `.autome/` scaffold, including the session wrapper script.
//! - [`env_probe`] — the four local components.
//! - [`launcher`] — prompt construction, the CLI adapter table, and starting
//!   a session in a visible terminal.
//! - [`protocol`] — the `~/.autome/protocol/` repository: versions, tags, the
//!   kernel contract, and a task's frozen copy.
//! - [`scheduler`] — the one place a transition is applied and acted on.
//! - [`skills`] — the read-only skill inventory scan.
//! - [`task_metrics`] — one task folded into the numbers a later decision can
//!   be made on.
//! - [`usage`] — what a session cost, read out of the CLI's own event stream.
//! - [`version_page`] — what each protocol version cost, and whether its
//!   changes did what they said they would.

pub mod config_io;
pub mod dispatch;
pub mod dispatch_protocol;
pub mod env_probe;
pub mod git;
pub mod guards;
pub mod init;
pub mod ipc;
pub mod launcher;
pub mod protocol;
pub mod scheduler;
pub mod skills;
pub mod store;
pub mod task_metrics;
pub mod usage;
pub mod version_page;
pub mod stream_render;
