# Autome 2.0

Rust-core, Electron-desktop, evidence-gated task execution system. See
`docs/development/plan.md` (mirrored from the authoring plan) for the full
specification. Authoritative source of truth for scope/decisions is
`autome/docs/plans/2026-09-13-autome-2.0-greenfield-development-plan.md` in
the sibling 1.x repo — read-only reference, not a dependency (see
[ADR-0001](docs/adr/0001-greenfield-independent-repo.md)).

## Workspace layout

- `crates/autome-domain` — pure domain types and state reducers (no I/O).
- `crates/automed` — application service: SQLite, event journal, scheduler,
  Harness adapters, brokers, JSON-RPC stdio IPC.
- `apps/desktop` — Electron shell (Main/Preload/Renderer); UI only, no
  business state (D3).
- `contracts/` — generated JSON Schema / TS bindings (not yet generated).
- `profiles/`, `install-recipes/`, `skill-policies/`, `playbooks/` — see
  plan §4 for each directory's authority.

## Status

M0 in progress. Current state: domain-level state machines (Project, Run,
TaskGraph node, completion gate) implemented and unit-tested in
`autome-domain`; `automed` is a compiling stub with no IPC/SQLite/Harness yet.

## Building

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```
