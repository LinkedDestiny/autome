# ADR-0003: The protocol ships in the binary; no protocol repository

Status: accepted
Date: 2026-10-11
Supersedes: the protocol-repository, pinning, rollback, upstream-tag and meta
task parts of `docs/plans/2026-09-17-loop-self-improvement-proposal.md`.

## Context

From 2026-09-17 the Loop protocol (rules, prompt templates, brief map, eval
cases) lived in a git repository Autome created at `~/.autome/protocol/`,
seeded from the copy compiled into the binary. Versions were `protocol/vN`
tags; a project could pin one; rollback made a forward commit; a newer
binary offered its seed on a `protocol/upstream-<hash>` tag; and a *meta
task* — a Loop task on a built-in "Loop 协议" project — was how the protocol
was meant to change.

Its release path was never finished. Nothing turned an upstream tag or a
merged meta task into a `protocol/vN`. On the one real machine `main` sat on
v1 while `protocol/v2` had been tagged by hand on the upstream commit, so a
meta task (branched from `main`) would have edited v1 and dropped v2. The
built-in project also never got its `.autome/` scaffold, so its first task
failed at intake for want of `run_session.sh`.

The user's position: there should be one git repository for Autome — this
one.

## Decision

The protocol's only home is `crates/automed/src/protocol/seed/`. The version
in force is the seed compiled into the running binary; its tag is the newest
`## protocol/vN` heading in the seed's own `CHANGELOG.md`, and its identity is
the content hash (`protocol::current()`).

Kept: every task still freezes the whole protocol into
`docs/<slug>/protocol/` at its first session and records `protocol_ref`
(tag + hash), so installing a newer build never changes the rules of a task
already running. The version page still groups task metrics by
`protocol_ref`, with each version's changelog entries taken from the
built-in changelog. `automed protocol eval <dir>` remains the gate for a
protocol change — run it against the edited `seed/`.

Removed: `~/.autome/protocol/` and everything that managed it; the
`protocol.eval`, `protocol.triggers`, `protocol.improve`, `protocol.pin` and
`protocol.rollback` IPC methods; the meta task and its evidence assembly;
the changelog `realized_impact` backfill (it wrote into that repository);
the per-project `[loop] protocol` pin (ignored on read, dropped on the next
save). Recovery unregisters a project at `~/.autome/protocol` left by an
older build and cancels its unfinished tasks; the directory is not touched.

To change the protocol: edit `seed/`, add a `## protocol/vN` section to its
changelog, run `automed protocol eval crates/automed/src/protocol/seed`, run
the suite, and ship a build.

## Consequences

- The protocol changes only with a release of Autome, through the same
  review as any other code change.
- `realized_impact` in the changelog is no longer filled in automatically;
  the version page's measured columns are the record.
- The seed's own `README.md` described the repository it used to be copied
  into. Rewriting it changed the content hash, so it shipped as
  `protocol/v3` (C-17) rather than silently giving v2 a second hash; the rule
  text and templates are byte-identical to v2.
