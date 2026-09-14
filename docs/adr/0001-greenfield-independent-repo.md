# ADR-0001: Autome 2.0 is a physically independent greenfield repository

Status: accepted
Date: 2026-09-14

## Context

The 2.0 research/design plan (`autome/docs/plans/2026-09-13-autome-2.0-greenfield-development-plan.md`,
decisions D1–D15) mandates that Autome 2.0 is an independent product: no
migration from the 1.x Bash CLI/plugin repo, no shared runtime directories,
no reused source. 1.x may only inform reinterpreted failure cases and
acceptance material — never code, protocol, or config.

## Decision

- This repository (`autome-v2`) is a new, standalone Git repo with no
  history, branch, worktree, or submodule relationship to the 1.x `autome`
  repo.
- No build, test, or release step may reference a path inside the 1.x repo.
- Every crate/file's provenance is either "written for 2.0 against the plan"
  or "copied from an explicitly named public source with license noted" —
  never "adapted from 1.x source".
- Rust workspace has exactly two crates (`autome-domain`, `automed`) per plan
  §4, to avoid speculative store/harness/verifier/ipc crate splits before
  there is a second real consumer of any such boundary.

## Consequences

- Anything that looks like 1.x (directory layout, CLI verbs, file protocol)
  must be independently re-derived from the plan and public docs, not eyeballed
  from 1.x source, even where the shapes end up superficially similar.
- Provenance is auditable per-commit: commits introducing new files should
  state their basis (plan section, or external doc URL) in the message body.
