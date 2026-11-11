//! The task state machine. Technical design §5.1 and §5.3; requirement T-03.
//!
//! `automed` owns every transition (design §1: "automed 独占节点转换"). Agents
//! no longer launch their own successor session — they write their result into
//! the design document and exit, and this table decides what happens next.
//! That is what makes pause, the parallel limit, role toggles, the round
//! budgets and the two human stopping points enforceable by the core rather
//! than by the cooperation of a prompt.
//!
//! Everything here is a pure function of (state, trigger, context). The
//! context carries the resolved configuration and the freshly-parsed status
//! block; no clock, no filesystem, no database.

use serde::{Deserialize, Serialize};

use crate::config::ResolvedConfig;
use crate::role::Role;
use crate::status_block::{DocStatus, ParseError, StatusBlock};

/// A node where work happens or waits. The nodes the task panel draws, minus
/// the two terminal ones (`Done` is a `TaskState`, not a node).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Node {
    Intake,
    Design,
    Review,
    Adjudicate,
    AwaitDesignApproval,
    Implement,
    /// The project's gate, run by the core on the commit the implementation
    /// round just left, before the audit round is asked to believe it. Only
    /// reached when the project configures `[gate]`.
    Gate,
    Audit,
    /// One pass over the whole run, once the implementation loop is done,
    /// writing `docs/<slug>/lessons.md`. Before this existed, what a task
    /// learned stayed in the task directory: the retro file was free prose,
    /// nobody aggregated it, and the path from "we got this wrong three times"
    /// to "the protocol says not to" ran through a human remembering.
    ///
    /// It sits before `Rebase` rather than after the merge so that the lessons
    /// are written while the worktree still holds the evidence, and so that
    /// they travel with the branch.
    Retro,
    Rebase,
    /// The gate again, on the rebased commit — the one the merge will bring
    /// across. The implementation-loop gate ran on a branch that has since
    /// moved onto a newer base.
    MergeGate,
    AwaitMerge,
    Merging,
    /// The gate once more, on the merge commit, after the merge. The task is
    /// merged whatever this finds: a failure here raises an alert on the
    /// project rather than undoing anything.
    MainCheck,
    Cleanup,
    Locate,
    ReproCheck,
    Fix,
    FixGate,
    FixReview,
    AwaitInfo,
    NotABug,
}

impl Node {
    pub const BUG: [Node; 7] = [
        Node::Locate,
        Node::ReproCheck,
        Node::Fix,
        Node::FixGate,
        Node::FixReview,
        Node::AwaitInfo,
        Node::NotABug,
    ];

    /// All feature Loop nodes, in their original order.
    pub const ALL: [Node; 15] = [
        Node::Intake,
        Node::Design,
        Node::Review,
        Node::Adjudicate,
        Node::AwaitDesignApproval,
        Node::Implement,
        Node::Gate,
        Node::Audit,
        Node::Retro,
        Node::Rebase,
        Node::MergeGate,
        Node::AwaitMerge,
        Node::Merging,
        Node::MainCheck,
        Node::Cleanup,
    ];

    /// The role whose session this node runs, if any. `None` for the system
    /// steps and the two human stopping points, which is exactly the set the
    /// routing graph greys out as non-configurable (requirement C-04).
    pub const fn role(self) -> Option<Role> {
        match self {
            Node::Design => Some(Role::Plan),
            Node::Review => Some(Role::Review),
            Node::Adjudicate => Some(Role::Adjudicate),
            Node::Implement | Node::Locate | Node::Fix => Some(Role::Impl),
            Node::Audit | Node::FixReview => Some(Role::Audit),
            Node::Retro => Some(Role::Retro),
            // Intake runs a session too, but as a fixed system step on a
            // dedicated prompt, not as one of the five configurable roles.
            _ => None,
        }
    }

    /// Whether this node holds one of the project's parallel slots
    /// (design §8). The two human stopping points deliberately do not: a task
    /// waiting on the user must never block another task from running.
    pub const fn occupies_slot(self) -> bool {
        !matches!(
            self,
            Node::AwaitDesignApproval | Node::AwaitMerge | Node::AwaitInfo | Node::NotABug
        )
    }

    /// Whether this node is waiting on the user rather than on a machine.
    pub const fn awaits_user(self) -> bool {
        matches!(
            self,
            Node::AwaitDesignApproval | Node::AwaitMerge | Node::AwaitInfo | Node::NotABug
        )
    }

    /// Whether the core drives this node itself, with no session at all.
    pub const fn is_core_step(self) -> bool {
        matches!(
            self,
            Node::Rebase
                | Node::Merging
                | Node::Cleanup
                | Node::Gate
                | Node::MergeGate
                | Node::MainCheck
                | Node::ReproCheck
                | Node::FixGate
        )
    }

    /// The core steps that run the project's gate. Unlike the other core
    /// steps they take minutes, so the core starts them in the background and
    /// collects the result on a later tick.
    pub const fn is_gate(self) -> bool {
        matches!(
            self,
            Node::Gate | Node::MergeGate | Node::MainCheck | Node::ReproCheck | Node::FixGate
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Node::Intake => "intake",
            Node::Design => "design",
            Node::Review => "review",
            Node::Adjudicate => "adjudicate",
            Node::AwaitDesignApproval => "await_design_approval",
            Node::Implement => "implement",
            Node::Gate => "gate",
            Node::Audit => "audit",
            Node::Retro => "retro",
            Node::Rebase => "rebase",
            Node::MergeGate => "merge_gate",
            Node::AwaitMerge => "await_merge",
            Node::Merging => "merging",
            Node::MainCheck => "main_check",
            Node::Cleanup => "cleanup",
            Node::Locate => "locate",
            Node::ReproCheck => "repro_check",
            Node::Fix => "fix",
            Node::FixGate => "fix_gate",
            Node::FixReview => "fix_review",
            Node::AwaitInfo => "await_info",
            Node::NotABug => "not_a_bug",
        }
    }

    pub fn parse(s: &str) -> Option<Node> {
        Node::ALL
            .into_iter()
            .chain(Node::BUG)
            .find(|n| n.as_str() == s)
    }
}

/// Why a task stopped without completing (requirement T-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FailureReason {
    Setup {
        command: String,
        exit_code: i32,
        log: String,
    },
    /// Consecutive failures of the same gate for the same reason.
    GateSameFailure {
        count: u32,
        summary: String,
