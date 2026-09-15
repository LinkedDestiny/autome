# Autome 2.0

A macOS desktop app that turns a one-line request into a merged branch.

You point it at a Git repository and type what you want. It runs Claude Code
and Codex through a five-role loop — design, review, adjudicate, implement,
audit — in a dedicated worktree, and stops for you exactly twice: once to
approve the design, once to press merge.

## The shape of it

```
Electron (UI only)  ──framed JSON-RPC over stdio──  automed (Rust, all state)
                                                          │
                                              ┌───────────┼───────────┐
                                           git/worktree  iTerm2   .autome/ · docs/
                                                          │
                                                   claude / codex
```

Three properties the design turns on:

- **The core owns every node transition.** An agent writes its result into the
  design document and exits; the scheduler reads the document and decides what
  runs next. That is what makes pause, the parallel limit, role toggles, round
  budgets and the two stopping points enforceable by the core rather than by
  the cooperation of a prompt.
- **Task progress lives in the repository**, in the design document's status
  block. It travels with the branch and survives a machine change. SQLite holds
  the registry, the index, the session ledger and your decisions.
- **Generation and evaluation never share a model.** Review must differ from
  design, and audit from implementation. A configuration that violates this
  cannot be saved.

Sessions run in a visible terminal, so you can watch them. Autome never pushes,
never opens a pull request, and never merges without you pressing the button.

## Layout

- `crates/autome-domain` — pure types and transitions. No I/O, no async, no
  SQLite. The five roles, the configuration overlay and its validation, the
  design-document parser, the task state machine.
- `crates/automed` — everything that touches the outside world: SQLite, Git,
  the session launcher, the scheduler, the environment probe, the skill scan,
  and the JSON-RPC surface.
- `apps/desktop` — the Electron shell. Two IPC channels with two independent
  allowlists; the renderer can never name a filesystem path.
- `docs/development/requirements.md`, `docs/development/technical-design.md` —
  mirrored from the authoring repository; see below.
- `docs/adr/` — the two decisions that shaped the repository.

## Authority

The specification lives in the sibling 1.x repository as a read-only reference
(ADR-0001), at `autome/docs/plans/`:

- `2026-09-15-autome-2.0-requirements.md`
- `2026-09-15-autome-2.0-technical-design.md`
- `2026-09-15-autome-2.0-project-task-ui-decisions.md` — the decision record
- `autome-2.0-desktop-ui.html` — the interaction design

The 2026-09-13 greenfield plan is historical; ADR-0002 records what it
specified, what survives, and why the rest was dropped.

## Building

```sh
cargo test                      # 176 domain + 286 automed + 16 end-to-end
cargo clippy --all-targets      # clean
cd apps/desktop && npm test     # 87: the shell, the gates and the renderer
```

The end-to-end suite drives a whole task from a one-line request to a merge
commit against a real Git repository, with a stand-in for the model. Nothing
else is substituted: real worktrees, the real wrapper script, the real
exit-marker protocol, a real rebase and a real merge.

### Against the real CLIs

A stand-in cannot catch a flag that does not exist, or one that means
something other than what you assumed — and that is the class of bug that bit
this project hardest. `tests/real_cli.rs` runs the real binaries:

```sh
# Free, no account: asks each CLI for its own --help and checks every flag
# we pass appears in it.
cargo test --test real_cli every_adapter_flag

# Costs money, needs a logged-in account.
AUTOMED_REAL_CLI=1 cargo test --test real_cli -- --nocapture

# The whole loop, real models, to a real merge commit. ~40 minutes.
AUTOMED_REAL_CLI=1 AUTOMED_REAL_LOOP=1 \
  cargo test --test real_cli the_whole_loop -- --nocapture --test-threads=1
```

## Status

The loop runs end to end against the real CLIs. Verified 2026-09-16 with
Claude Code 2.1.261 and Codex 0.153.4: a one-line request went through intake,
design, review, adjudication, the approval stop, implementation and an
independent audit, to a merge commit on `main` — six real sessions, the user
pressing two buttons.

Remaining before this is something to install: packaging and signing, and the
Onboarding wizard's in-app editing step wired into the renderer.
