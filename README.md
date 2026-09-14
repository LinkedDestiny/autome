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

M0 in progress. Current state: domain-level state machines and reducers for
all six aggregates/singletons (Project, Run, Task, Contract, Graph,
ExecutionQueue) implemented and unit-tested in `autome-domain`; `automed`
has a real event-sourced SQLite store, the framed stdio JSON-RPC loop, and
IPC dispatch for all of them. `apps/desktop` has an Electron Main that
enforces the §9.4 security baseline (privileged `autome://` scheme, no
Node/remote content in the renderer, denied navigation/permissions) and
spawns/talks to the `automed` sidecar over the same protocol — no real
navigation UI or business-state IPC surface yet, and none of §9.5's
packaging-dependent lifecycle guarantees (manifest/signature verification,
single-instance lock, PrepareShutdown/SafePark-gated quit).

## Building

Rust:

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Desktop shell (`apps/desktop`):

```
npm install
npm test    # framing + sidecar e2e tests against the built automed binary
npm start   # launch the Electron shell
```
