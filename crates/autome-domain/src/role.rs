//! The five Loop roles and the two CLI runtimes, per technical design §5.2.
//!
//! These are the only two closed enumerations the whole system routes on, so
//! they live in their own module rather than inside `config`: the scheduler,
//! the session launcher, the skill inventory and the transition table all
//! need them without needing anything else config-shaped.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A Loop role. Ordering here is the Loop's own order (design → review →
/// adjudicate → impl → audit), which `Role::ALL` preserves so that anything
/// iterating roles for display gets the graph order for free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Plan,
    Review,
    Adjudicate,
    Impl,
    Audit,
}

impl Role {
    pub const ALL: [Role; 5] = [
        Role::Plan,
        Role::Review,
        Role::Adjudicate,
        Role::Impl,
        Role::Audit,
    ];

    /// The TOML key / IPC wire name. Kept as an explicit match rather than
    /// derived from `Debug` so renaming the Rust variant can never silently
    /// change an on-disk config file's schema.
    pub const fn as_str(self) -> &'static str {
        match self {
            Role::Plan => "plan",
            Role::Review => "review",
            Role::Adjudicate => "adjudicate",
            Role::Impl => "impl",
            Role::Audit => "audit",
        }
    }

    pub fn parse(s: &str) -> Option<Role> {
        Role::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// The entry sentence appended to `docs/<slug>/<slug>-task.md` when this
    /// role's session starts (design §5.2). The task-file protocol inherited
    /// from 1.x numbers the four follow-up roles as "additional task 1..4";
    /// `plan` is the unnumbered Task 1.
    pub const fn task_file_entry(self) -> &'static str {
        match self {
            Role::Plan => "Task 1",
            Role::Review => "Task 1 additional task 1",
            Role::Adjudicate => "Task 1 additional task 2",
            Role::Impl => "Task 1 additional task 3",
            Role::Audit => "Task 1 additional task 4",
        }
    }

    /// The generating side of each SAME-MODEL pair, for the evaluating roles.
    /// `None` for roles that are not evaluators — the discipline only
    /// constrains review-vs-plan and audit-vs-impl (design §5, requirement
    /// C-06); adjudicate is deliberately unconstrained.
    pub const fn same_model_counterpart(self) -> Option<Role> {
        match self {
            Role::Review => Some(Role::Plan),
            Role::Audit => Some(Role::Impl),
            _ => None,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which CLI a role's session runs in. 2.0 supports exactly these two
/// (requirement C-04); a third harness is explicitly out of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    Claude,
    Codex,
}

impl Runtime {
    pub const ALL: [Runtime; 2] = [Runtime::Claude, Runtime::Codex];

    pub const fn as_str(self) -> &'static str {
        match self {
            Runtime::Claude => "claude",
            Runtime::Codex => "codex",
        }
    }

    pub fn parse(s: &str) -> Option<Runtime> {
        Runtime::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// Human-facing product name, for messages the user reads.
    pub const fn display_name(self) -> &'static str {
        match self {
            Runtime::Claude => "Claude Code",
            Runtime::Codex => "Codex",
        }
    }

    /// The per-runtime skill directory basename, relative to `$HOME` for the
    /// global scope and to the repository root for the project scope
    /// (design §12). Claude reads `.claude/skills`, Codex reads
    /// `.agents/skills`.
    pub const fn skill_dir(self) -> &'static str {
        match self {
            Runtime::Claude => ".claude/skills",
            Runtime::Codex => ".agents/skills",
        }
    }
}

impl fmt::Display for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_str_round_trips_for_every_variant() {
        for role in Role::ALL {
            assert_eq!(Role::parse(role.as_str()), Some(role));
        }
        assert_eq!(Role::parse("nope"), None);
    }

    #[test]
    fn runtime_str_round_trips_for_every_variant() {
        for runtime in Runtime::ALL {
            assert_eq!(Runtime::parse(runtime.as_str()), Some(runtime));
        }
        assert_eq!(Runtime::parse("gemini"), None);
    }

    #[test]
    fn role_serde_uses_the_same_wire_names_as_as_str() {
        for role in Role::ALL {
            let json = serde_json::to_string(&role).unwrap();
            assert_eq!(json, format!("\"{}\"", role.as_str()));
        }
    }

    #[test]
    fn task_file_entries_are_distinct_and_cover_all_five_roles() {
        let mut entries: Vec<&str> = Role::ALL.iter().map(|r| r.task_file_entry()).collect();
        entries.sort_unstable();
        entries.dedup();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn same_model_pairs_are_exactly_review_plan_and_audit_impl() {
        let pairs: Vec<(Role, Option<Role>)> = Role::ALL
            .into_iter()
            .map(|r| (r, r.same_model_counterpart()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                (Role::Plan, None),
                (Role::Review, Some(Role::Plan)),
                (Role::Adjudicate, None),
                (Role::Impl, None),
                (Role::Audit, Some(Role::Impl)),
            ]
        );
    }

    #[test]
    fn skill_dirs_differ_per_runtime() {
        assert_eq!(Runtime::Claude.skill_dir(), ".claude/skills");
        assert_eq!(Runtime::Codex.skill_dir(), ".agents/skills");
    }
}
