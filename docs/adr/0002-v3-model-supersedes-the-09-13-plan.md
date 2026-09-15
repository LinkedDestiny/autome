# ADR-0002: The 2026-09-15 model supersedes the 09-13 greenfield plan

Status: accepted
Date: 2026-09-15
Supersedes: the design content of ADR-0001's referenced plan (not ADR-0001's
own decision about repository independence, which stands).

## Context

The 09-13 greenfield plan specified a system built around a task contract, a
task graph with per-node write domains, evidence receipts, candidate and
completion certificates, a delivery rehearsal and a delivered-tree check, an
eight-phase project state machine with a parallel hold enumeration, a product
intent revision per project, a skill marketplace with quarantine and audit,
and a global-config impact preview. Roughly 40 000 lines of Rust were written
against it.

On 2026-09-15 the user reviewed the interaction design produced from that plan
and judged both the element density and the workflow length to be wrong for
the product: a single developer, on their own machine, wanting a one-line
request turned into a merged branch. Seven rounds of interactive design
followed. The outcome is recorded in
`autome/docs/plans/2026-09-15-autome-2.0-project-task-ui-decisions.md`, and
restated as a requirements document and a technical design of the same date.

## Decision

The 09-15 requirements and technical design are the authority. The 09-13 plan
is historical.

What survives from it, unchanged:

- D1: 2.0 is an independent greenfield repository (ADR-0001).
- D2: the Rust core is the sole authority over business state.
- D3: Electron is UI only and owns no business state.
- D9: local, single user, macOS.

What is dropped, and why:

| Dropped | Because |
|---|---|
| TaskContract, TaskGraph, requirement coverage matrix | The design document's milestone table already carries the same information, is written by the agent anyway, and travels with the branch. |
| EvidenceReceipt, AuditVerdict, certificates, delivery rehearsal, delivered-tree check | The user's acceptance test is reading the diff and pressing merge. A receipt chain proving what a verifier observed is machinery for a trust problem this product does not have. |
| Eight-phase project machine, ProjectIntentRevision, trust confirmation | A project is a directory. Choosing it is the trust decision; product intent lives in AGENTS.md and docs/. |
| Application Support ProjectHome | Configuration belongs in the repository, so a second machine picks it up through Git. |
| ExecutionQueue, HarnessLease | Replaced by a per-project semaphore. Tasks run in parallel worktrees; there is nothing to lease. |
| Skill marketplace, quarantine, vault, projection | Skills are read from the two CLIs' own directories. Autome inventories and binds; it does not distribute. |
| GlobalConfigImpactPreview, policy restart/amendment, human-review receipts | Configuration takes effect on the next session. There is no frozen spec to amend. |
| Nine AI steps, seventeen-node lifecycle | Five roles, thirteen nodes, two human stopping points. |
| The disposable-clone sandbox model | Sessions run in a real worktree so the user can watch them. The confinement is the working directory and the protocol, not a sandbox. This is an accepted risk, recorded in technical design §17. |

Two structural changes are worth stating separately, because they are the
reason the rest simplifies:

1. **The design document is the source of truth for task progress**, not
   SQLite. Rounds, milestones, Backlog and disputes are parsed out of it after
   every session. It is what the agents write, it travels with the branch, and
   it survives a machine change.
2. **The core owns every node transition.** In 1.x an agent launched its own
   successor session. Here it writes its result and exits, and the scheduler
   decides what happens next. That is what makes pause, the parallel limit,
   role toggles, round budgets and the two stopping points enforceable by the
   core rather than by the cooperation of a prompt.

## Consequences

- 26 domain modules and the 24 000-line store/dispatch pair were deleted, not
  refactored. Their shapes were specific to the contract/graph/receipt model
  and had no counterpart in the new one.
- What was kept: the framed JSON-RPC stdio layer, the Electron security
  baseline and sidecar lifecycle, and the CLI adapter skeleton reshaped into
  the session launcher.
- ADR-0001's provenance rule still applies. The `.autome/` directory layout,
  the five role names and the task-file entry sentences are deliberately
  *compatible* with 1.x's conventions, because the user asked for that
  (decision record, round 2). That compatibility is re-derived from the
  decision record, not copied from 1.x source.
