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
        log: String,
    },
    /// The design loop used up `design_rounds`.
    DesignBudget {
        limit: u32,
    },
    /// The implementation loop used up N.
    ImplBudget {
        limit: u32,
    },
    FixBudget {
        limit: u32,
    },
    /// The design document said the task cannot be done.
    Infeasible,
    /// The design document could not be read, or said so itself.
    Protocol {
        detail: String,
    },
    /// The session process died, or left no usable output.
    SessionCrashed {
        detail: String,
    },
    /// A rebase conflict survived the implement/audit repair path.
    RebaseConflict {
        detail: String,
    },
    /// A configuration problem blocked the launch (SAME-MODEL, skills, …).
    Config {
        detail: String,
    },
    /// A core step (merge, cleanup) failed.
    CoreStep {
        node: Node,
        detail: String,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    #[default]
    Feature,
    Bug,
}

impl TaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Bug => "bug",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "feature" => Some(Self::Feature),
            "bug" => Some(Self::Bug),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "TaskStateRepr", into = "TaskStateRepr")]
pub enum TaskState {
    Draft,
    /// Created, but the project's parallel slots are all taken.
    Queued,
    /// Resumed work waiting for a slot; externally still `state: queued`.
    Requeued {
        resume: Node,
    },
    Active {
        node: Node,
    },
    /// Pause takes effect *after* the running session finishes (requirement
    /// T-08), so a paused task always names the node it would resume into.
    Paused {
        resume: Node,
    },
    Stopped {
        at: Node,
    },
    Failed {
        at: Node,
        reason: FailureReason,
    },
    Done,
    Cancelled,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
enum TaskStateRepr {
    Draft,
    Queued {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume: Option<Node>,
    },
    Active {
        node: Node,
    },
    Paused {
        resume: Node,
    },
    Stopped {
        at: Node,
    },
    Failed {
        at: Node,
        reason: FailureReason,
    },
    Done,
    Cancelled,
}

impl From<TaskState> for TaskStateRepr {
    fn from(state: TaskState) -> Self {
        match state {
            TaskState::Draft => Self::Draft,
            TaskState::Queued => Self::Queued { resume: None },
            TaskState::Requeued { resume } => Self::Queued {
                resume: Some(resume),
            },
            TaskState::Active { node } => Self::Active { node },
            TaskState::Paused { resume } => Self::Paused { resume },
            TaskState::Stopped { at } => Self::Stopped { at },
            TaskState::Failed { at, reason } => Self::Failed { at, reason },
            TaskState::Done => Self::Done,
            TaskState::Cancelled => Self::Cancelled,
        }
    }
}

impl From<TaskStateRepr> for TaskState {
    fn from(state: TaskStateRepr) -> Self {
        match state {
            TaskStateRepr::Draft => Self::Draft,
            TaskStateRepr::Queued { resume: None } => Self::Queued,
            TaskStateRepr::Queued {
                resume: Some(resume),
            } => Self::Requeued { resume },
            TaskStateRepr::Active { node } => Self::Active { node },
            TaskStateRepr::Paused { resume } => Self::Paused { resume },
            TaskStateRepr::Stopped { at } => Self::Stopped { at },
            TaskStateRepr::Failed { at, reason } => Self::Failed { at, reason },
            TaskStateRepr::Done => Self::Done,
            TaskStateRepr::Cancelled => Self::Cancelled,
        }
    }
}

impl TaskState {
    pub fn node(&self) -> Option<Node> {
        match self {
            TaskState::Active { node } => Some(*node),
            TaskState::Paused { resume } | TaskState::Requeued { resume } => Some(*resume),
            TaskState::Stopped { at } | TaskState::Failed { at, .. } => Some(*at),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Done | TaskState::Cancelled)
    }

    /// Whether this state holds one of the project's parallel slots.
    pub fn occupies_slot(&self) -> bool {
        match self {
            TaskState::Active { node } => node.occupies_slot(),
            _ => false,
        }
    }

    /// Whether the task is showing up in the "等待我" list.
    pub fn awaits_user(&self) -> bool {
        match self {
            TaskState::Active { node } => node.awaits_user(),
            TaskState::Failed { .. } => true,
            _ => false,
        }
    }
}

/// The user's disposition of one Backlog item or dispute (requirement T-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// No decision yet — nothing is consumed at the next stopping point.
    None,
    /// Backlog: turn into a new milestone. Disputes never use this.
    Include,
    /// Backlog: record in retro and move on.
    Ignore,
    /// Dispute: the user ruled. The ruling text lives alongside, in the store.
    Ruled,
}

/// The user's pending decisions as the transition needs to see them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDecisions {
    /// Backlog items marked `Include` and not yet consumed.
    pub included: u32,
    /// Disputes marked `Ruled` and not yet consumed.
    pub ruled: u32,
}

impl PendingDecisions {
    pub fn any(&self) -> bool {
        self.included > 0 || self.ruled > 0
    }
}

/// What made the machine move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "trigger", rename_all = "snake_case")]
pub enum Trigger {
    SetupFailed {
        command: String,
        exit_code: i32,
        log: String,
    },
    BugSessionEnded {
        outcome: crate::bug_loop::BugSessionOutcome,
    },
    ReproDone {
        head: String,
        command: String,
        exit_code: i32,
        timed_out: bool,
        tail: String,
    },
    Reply {
        info: String,
    },
    Start,
    /// A parallel slot opened up and this task is at the head of the queue.
    SlotAvailable,
    /// A session ended and its design document parsed cleanly.
    SessionEnded {
        outcome: SessionOutcome,
    },
    /// The user approved at the design stopping point.
    Approve,
    /// The user rejected at the design stopping point, with feedback.
    Reject {
        feedback: String,
    },
    /// The user pressed "合并到 main".
    Merge,
    /// A core step the transition previously asked for has finished.
    CoreStepDone {
        node: Node,
        result: CoreStepResult,
    },
    Pause,
    Resume,
    Stop,
    Cancel,
    /// From the failure panel: keep going with more rounds.
    ExtendBudget {
        extra_rounds: u32,
    },
    /// From the failure panel: go back and re-run from a node.
    RerunFrom {
        node: Node,
    },
    /// A gate reached the consecutive same-failure limit.
    GateSameFailure {
        node: Node,
        count: u32,
        summary: String,
        log: String,
    },
    /// The project's gate finished at one of the three gate nodes.
    GateDone {
        node: Node,
        outcome: GateOutcome,
        /// The design document as it stands, when it parsed. The gate after
        /// an implementation round needs it for the same decisions the
        /// implementation round's own ending would have made (budget, open
        /// milestones); the other two ignore it.
        status: Option<Box<StatusBlock>>,
    },
}

/// What the gate said, on which commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gate", rename_all = "snake_case")]
pub enum GateOutcome {
    Passed,
    Failed {
        head: String,
        command: String,
        /// The end of the gate's output: what the next round needs to see.
        tail: String,
    },
}

/// How a session ended. `Ok` carries the parsed document; the other variants
/// are the ways reading it can fail (design §5.4, §7.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SessionOutcome {
    Ok {
        status: Box<StatusBlock>,
    },
    /// Exit code non-zero, or the log was empty.
    Crashed {
        detail: String,
    },
    /// The document did not parse.
    Unparseable {
        error: ParseError,
    },
    /// The task file or design document was not created at all.
    MissingArtifact {
        path: String,
    },
    /// The document parsed, and the core's own checks rejected what the round
    /// did (plan §4.D): an implementation round claiming a milestone closed,
    /// or a loop round that left no evidence file.
    ///
    /// Separate from `Unparseable` because the two are fixed differently — one
    /// is a format error on a specific line, the other is a round having done
    /// something the protocol reserves for a different round — and because a
    /// person reading the failure panel needs to be told which.
    GuardFailed {
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum CoreStepResult {
    Ok,
    /// Rebase only.
    Conflict {
        files: Vec<String>,
        detail: String,
    },
    /// Merging only: all refusals are due to stale task branches.
    Behind {
        repos: Vec<String>,
        detail: String,
    },
    /// Merge preconditions unmet other than a stale branch.
    Blocked {
        detail: String,
    },
    Failed {
        detail: String,
    },
}

/// Everything the transition needs beyond the state and the trigger.
pub struct Context<'a> {
    pub config: &'a ResolvedConfig,
    /// The task's implementation budget N, once computed. `None` before the
    /// design is approved.
    pub budget_n: Option<u32>,
    pub decisions: PendingDecisions,
}

/// A side effect `automed` must perform as part of applying a transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    StartBugSession {
        step: crate::session::BugStep,
        inject: Option<Inject>,
    },
    /// Nothing to do; the new state is just a record.
    None,
    /// Launch the intake session (fixed system prompt, Claude Code).
    StartIntake,
    /// Launch a role session. `inject` carries anything the transition wants
    /// appended to the prompt: rejection feedback, conflict files, or the
    /// user's consumed decisions.
    StartRole { role: Role, inject: Option<Inject> },
    /// Run a core step the core performs itself.
    RunCoreStep { node: Node },
    /// Continue or relaunch the original session without starting a new budget round.
    Reenter { node: Node },
    /// Kill and remove stale jobs before a stopped gate is resumed.
    DiscardGateRuns { node: Node },
    /// Delete the worktree and branch, archive the docs.
    RunCancel,
}

/// Extra prompt material for a role session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "inject", rename_all = "snake_case")]
pub enum Inject {
    ReproNotFailing {
        head: String,
        command: String,
        exit_code: i32,
        why: String,
        tail: String,
    },
    FixRejected,
    /// The user rejected the design; re-run with their words.
    DesignFeedback {
        feedback: String,
    },
    /// The user's Backlog/dispute decisions are being consumed.
    Decisions,
    /// Rebase left conflicts for the implement role to resolve.
    RebaseConflict {
        files: Vec<String>,
    },
    /// The project's gate failed on the commit this round has to fix.
    GateFailed {
        head: String,
        command: String,
        tail: String,
    },
}

/// The result of applying a trigger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub next: TaskState,
    pub action: Action,
    /// Set when the transition computes or changes the implementation budget.
    pub budget_n: Option<u32>,
    /// True when the pending decisions were consumed by this transition, so
    /// the store can mark them and not apply them twice.
    pub consumes_decisions: bool,
    /// Authorization carried only through the requested rebase and merge gate.
    pub merge_requested: bool,
}

impl Transition {
    pub(crate) fn to(next: TaskState, action: Action) -> Self {
        Self {
            next,
            action,
            budget_n: None,
            consumes_decisions: false,
            merge_requested: false,
        }
    }

    pub(crate) fn active(node: Node, action: Action) -> Self {
        Self::to(TaskState::Active { node }, action)
    }

    pub(crate) fn fail(at: Node, reason: FailureReason) -> Self {
        Self::to(TaskState::Failed { at, reason }, Action::None)
    }

    pub(crate) fn with_budget(mut self, n: u32) -> Self {
        self.budget_n = Some(n);
        self
    }

    fn consuming(mut self) -> Self {
        self.consumes_decisions = true;
        self
    }
}

/// A trigger that does not apply in the current state. Distinct from a task
/// *failing*: this is a caller bug or a race, not a task outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rejected {
    pub reason: String,
}

fn reject(reason: &str) -> Rejected {
    Rejected {
        reason: reason.to_string(),
    }
}

/// Starts the design loop at the first enabled node after `Design` itself.
/// With review disabled there is nothing for adjudicate to adjudicate, so the
/// pair is skipped together and the design goes straight to the user
/// (requirement C-05).
fn after_design(config: &ResolvedConfig) -> Transition {
    if config.is_enabled(Role::Review) {
        Transition::active(
            Node::Review,
            Action::StartRole {
                role: Role::Review,
                inject: None,
            },
        )
    } else {
        Transition::active(Node::AwaitDesignApproval, Action::None)
    }
}

/// After review, adjudicate — unless adjudicate itself is disabled, in which
/// case the review's own verdict is taken as final and the design goes to the
/// user.
fn after_review(config: &ResolvedConfig) -> Transition {
    if config.is_enabled(Role::Adjudicate) {
        Transition::active(
            Node::Adjudicate,
            Action::StartRole {
                role: Role::Adjudicate,
                inject: None,
            },
        )
    } else {
        Transition::active(Node::AwaitDesignApproval, Action::None)
    }
}

/// Where a finished implementation loop goes: through the retro round when it
/// is enabled, straight to rebase when it is not (requirement C-05).
fn after_the_loop(config: &ResolvedConfig) -> Transition {
    if config.is_enabled(Role::Retro) {
        Transition::active(
            Node::Retro,
            Action::StartRole {
                role: Role::Retro,
                inject: None,
            },
        )
    } else {
        Transition::active(Node::Rebase, Action::RunCoreStep { node: Node::Rebase })
    }
}

/// The implementation-side successor after an implement session, when audit
/// is disabled (requirement C-05): no independent check, so "no milestone is
/// still open" is the exit condition.
fn after_implement_without_audit(status: &StatusBlock, budget_n: u32, ctx: &Context) -> Transition {
    if status.no_open_milestones() {
        after_the_loop(ctx.config)
    } else if status.impl_round >= budget_n {
        Transition::fail(
            Node::Implement,
            FailureReason::ImplBudget { limit: budget_n },
        )
    } else {
        start_impl(ctx, None)
    }
}

/// Starts an implementation round, absorbing whatever the user has marked
/// 纳入 since the last one (T-09).
///
/// The decision used to be held until the merge stopping point, which put it
/// at the worst possible moment: hours after it was made, at the one moment
/// the user was asking to *finish*, the 合并 button sent the task back to the
/// implementation loop instead. It happened to a real task twice, and both
/// times the round that followed ended in a protocol failure nobody had asked
/// for. Absorbed at the top of each implementation round, the decision takes
/// effect where the work is, and the round it changes is the next one.
///
/// The budget grows by the configured factor per included item, exactly as
/// the merge stopping point used to do it: the extra work is not paid for out
/// of the original N.
///
/// `base` is the budget the caller had already decided on, for the one caller
/// that has one (追加轮次). `None` means "whatever the task already had".
///
/// A round that is already carrying an injection — a rebase conflict — keeps
/// it. That round has one job, and the Backlog items wait for the next one,
/// which is at most one round away.
fn start_impl(ctx: &Context, base: Option<u32>) -> Transition {
    let start = |inject| {
        Transition::active(
            Node::Implement,
            Action::StartRole {
                role: Role::Impl,
                inject,
            },
        )
    };
    if ctx.decisions.included == 0 {
        return match base {
            Some(n) => start(None).with_budget(n),
            None => start(None),
        };
    }
    let extra = ctx.config.loop_defaults.budget_factor * ctx.decisions.included;
    start(Some(Inject::Decisions))
        .with_budget(base.or(ctx.budget_n).unwrap_or(0) + extra)
        .consuming()
}

/// Applies a trigger. Returns the next state plus the side effect to perform,
/// or `Rejected` when the trigger does not apply.
pub fn apply(
    state: &TaskState,
    trigger: &Trigger,
    ctx: &Context<'_>,
) -> Result<Transition, Rejected> {
    apply_with_merge_request(state, trigger, ctx, false)
}

/// Applies a trigger with the authorization from a user-requested merge.
pub fn apply_with_merge_request(
    state: &TaskState,
    trigger: &Trigger,
    ctx: &Context<'_>,
    merge_requested: bool,
) -> Result<Transition, Rejected> {
    let requested = merge_requested && in_merge_stretch(state);
    let mut transition = apply_inner(state, trigger, ctx, requested)?;
    transition.merge_requested = matches!(
        trigger,
        Trigger::CoreStepDone {
            node: Node::Merging,
            result: CoreStepResult::Behind { .. }
        }
    ) || (requested && in_merge_stretch(&transition.next));
    Ok(transition)
}

fn in_merge_stretch(state: &TaskState) -> bool {
    matches!(
        state,
        TaskState::Active {
            node: Node::Rebase | Node::MergeGate
        } | TaskState::Requeued {
            resume: Node::Rebase | Node::MergeGate
        } | TaskState::Paused {
            resume: Node::Rebase | Node::MergeGate
        }
    )
}

fn press_merge(ctx: &Context<'_>) -> Transition {
    if ctx.decisions.included > 0 {
        start_impl(ctx, None)
    } else {
        Transition::active(
            Node::Merging,
            Action::RunCoreStep {
                node: Node::Merging,
            },
        )
    }
}

fn apply_inner(
    state: &TaskState,
    trigger: &Trigger,
    ctx: &Context<'_>,
    merge_requested: bool,
) -> Result<Transition, Rejected> {
    // Control triggers first: they apply from many states, and handling them
    // once here keeps the node table below about the Loop itself.
    match trigger {
        Trigger::SetupFailed {
            command,
            exit_code,
            log,
        } => {
            return match state {
                TaskState::Active { node } if !node.awaits_user() => Ok(Transition::fail(
                    *node,
                    FailureReason::Setup {
                        command: command.clone(),
                        exit_code: *exit_code,
                        log: log.clone(),
                    },
                )),
                _ => Err(reject("任务不在运行中")),
            };
        }
        Trigger::GateSameFailure {
            node,
            count,
            summary,
            log,
        } => {
            if !matches!(state, TaskState::Active { node: current } if current == node && matches!(node, Node::Gate | Node::MergeGate))
            {
                return Err(reject("门禁结果与当前节点不符"));
            }
            return Ok(Transition::fail(
                *node,
                FailureReason::GateSameFailure {
                    count: *count,
                    summary: summary.clone(),
                    log: log.clone(),
                },
            ));
        }

        Trigger::Start => {
            return if matches!(state, TaskState::Draft) {
                Ok(Transition::to(TaskState::Queued, Action::None))
            } else {
                Err(reject("只有待开始的任务可以开始"))
            };
        }

        Trigger::Cancel => {
            return if state.is_terminal() {
                Err(reject("任务已终结"))
            } else {
                Ok(Transition::to(TaskState::Cancelled, Action::RunCancel))
            };
        }
        Trigger::Pause => {
            return match state {
                TaskState::Active { node } if !node.awaits_user() => Ok(Transition::to(
                    TaskState::Paused { resume: *node },
                    Action::None,
                )),
                TaskState::Active { .. } => Err(reject("任务正在等待你，没有会话可暂停")),
                TaskState::Draft | TaskState::Queued => Err(reject("任务尚未开始")),
                _ => Err(reject("任务不在运行中")),
            };
        }
        Trigger::Stop => {
            return match state {
                TaskState::Active { node } if !node.awaits_user() => Ok(Transition::to(
                    TaskState::Stopped { at: *node },
                    Action::None,
                )),
                TaskState::Paused { resume } => Ok(Transition::to(
                    TaskState::Stopped { at: *resume },
                    Action::None,
                )),
                _ => Err(reject("任务不在运行中")),
            };
        }
        Trigger::Resume => {
            return match state {
                TaskState::Paused { resume: node } | TaskState::Stopped { at: node } => {
                    if node.role().is_some_and(|role| !ctx.config.is_enabled(role)) {
                        return Err(reject("该角色当前已关闭，无法从这里恢复"));
                    }
                    let action = if matches!(state, TaskState::Stopped { .. })
                        && matches!(node, Node::Gate | Node::MergeGate | Node::MainCheck)
                    {
                        Action::DiscardGateRuns { node: *node }
                    } else {
                        Action::None
                    };
                    Ok(Transition::to(
                        TaskState::Requeued { resume: *node },
                        action,
                    ))
                }
                _ => Err(reject("任务不处于暂停或已停止")),
            };
        }
        _ => {}
    }

    if state.node().is_some_and(|node| Node::BUG.contains(&node))
        && !matches!(
            (state, trigger),
            (TaskState::Requeued { .. }, Trigger::SlotAvailable)
        )
    {
        return Err(reject("Bug 节点不使用需求转换表"));
    }

    match (state, trigger) {
        // ---- Queued ------------------------------------------------------
        (TaskState::Queued, Trigger::SlotAvailable) => {
            Ok(Transition::active(Node::Intake, Action::StartIntake))
        }

        (TaskState::Requeued { resume: node }, Trigger::SlotAvailable) => {
            if node.role().is_some_and(|role| !ctx.config.is_enabled(role)) {
                return Ok(Transition::fail(
                    *node,
                    FailureReason::Config {
                        detail: "该角色当前已关闭，无法从这里恢复".into(),
                    },
                ));
            }
            let action = if node.is_core_step() {
                Action::RunCoreStep { node: *node }
            } else {
                Action::Reenter { node: *node }
            };
            Ok(Transition::active(*node, action))
        }

        // ---- Failure panel ----------------------------------------------
        (TaskState::Failed { at, reason }, Trigger::ExtendBudget { extra_rounds }) => {
            let extra = *extra_rounds;
            if extra == 0 {
                return Err(reject("追加轮次必须大于 0"));
            }
            match reason {
                FailureReason::GateSameFailure { .. } => {
                    let budget = ctx.budget_n.ok_or_else(|| reject("读不到实现预算"))?;
                    let limit = budget
                        .checked_add(extra)
                        .ok_or_else(|| reject("追加轮次不能溢出"))?;
                    Ok(
                        Transition::active(*at, Action::DiscardGateRuns { node: *at })
                            .with_budget(limit),
                    )
                }
                FailureReason::ImplBudget { limit } => Ok(start_impl(ctx, Some(limit + extra))),
                FailureReason::DesignBudget { .. } => Ok(Transition::active(
                    Node::Design,
                    Action::StartRole {
                        role: Role::Plan,
                        inject: None,
                    },
                )),
                _ => Err(reject("只有轮次耗尽的失败可以追加轮次")),
            }
        }
        (
            TaskState::Failed {
                at,
                reason: FailureReason::GateSameFailure { .. },
            },
            Trigger::RerunFrom { node },
        ) if node == at && matches!(node, Node::Gate | Node::MergeGate) => Ok(Transition::active(
            *node,
            Action::DiscardGateRuns { node: *node },
        )),
        (TaskState::Failed { .. } | TaskState::Stopped { .. }, Trigger::RerunFrom { node }) => {
            let node = *node;
            match node {
                Node::Intake => Ok(Transition::active(Node::Intake, Action::StartIntake)),
                Node::Design
                | Node::Review
                | Node::Adjudicate
                | Node::Implement
                | Node::Audit
                | Node::Retro => {
                    let role = node.role().expect("role nodes carry a role");
                    if !ctx.config.is_enabled(role) {
                        return Err(reject("该角色当前已关闭，无法从这里重跑"));
                    }
                    Ok(Transition::active(
                        node,
                        Action::StartRole { role, inject: None },
                    ))
                }
                Node::Rebase => Ok(Transition::active(
                    Node::Rebase,
                    Action::RunCoreStep { node: Node::Rebase },
                )),
                _ => Err(reject("只能从任务整理、六个角色节点或 rebase 重跑")),
            }
        }

        // ---- Session ended ----------------------------------------------
        (TaskState::Active { node }, Trigger::SessionEnded { outcome }) => {
            let node = *node;
            if node.is_core_step() || node.awaits_user() {
                return Err(reject("该节点不运行会话"));
            }
            let status = match outcome {
                SessionOutcome::Ok { status } => status,
                SessionOutcome::Crashed { detail } => {
                    return Ok(Transition::fail(
                        node,
                        FailureReason::SessionCrashed {
                            detail: detail.clone(),
                        },
                    ));
                }
                SessionOutcome::Unparseable { error } => {
                    return Ok(Transition::fail(
                        node,
                        FailureReason::Protocol {
                            detail: error.to_string(),
                        },
                    ));
                }
                SessionOutcome::MissingArtifact { path } => {
                    return Ok(Transition::fail(
                        node,
                        FailureReason::Protocol {
                            detail: format!("缺少产物 {path}"),
                        },
                    ));
                }
                SessionOutcome::GuardFailed { detail } => {
                    return Ok(Transition::fail(
                        node,
                        FailureReason::Protocol {
                            detail: detail.clone(),
                        },
                    ));
                }
            };

            // A document that declares its own failure overrides whatever the
            // node would otherwise conclude — the agent knows something the
            // milestone table does not.
            match status.status {
                DocStatus::Infeasible => {
                    return Ok(Transition::fail(node, FailureReason::Infeasible));
                }
                DocStatus::ProtocolFailure => {
                    return Ok(Transition::fail(
                        node,
                        FailureReason::Protocol {
                            detail: "设计文档自报协议失败".into(),
                        },
                    ));
                }
                _ => {}
            }

            match node {
                Node::Intake => Ok(Transition::active(
                    Node::Design,
                    Action::StartRole {
                        role: Role::Plan,
                        inject: None,
                    },
                )),
                Node::Design => Ok(after_design(ctx.config)),
                Node::Review => Ok(after_review(ctx.config)),
                Node::Adjudicate => {
                    if status.status == DocStatus::Designing
                        && status.design_round >= status.design_round_limit
                    {
                        return Ok(Transition::fail(
                            node,
                            FailureReason::DesignBudget {
                                limit: status.design_round_limit,
                            },
                        ));
                    }
                    // The adjudication round is what decides whether the
                    // design is final: it flips `status` off 设计中 when it
                    // is, and leaves it there when another review round is
                    // needed.
                    if status.status == DocStatus::Designing {
                        Ok(Transition::active(
                            Node::Review,
                            Action::StartRole {
                                role: Role::Review,
                                inject: None,
                            },
                        ))
                    } else {
                        Ok(Transition::active(Node::AwaitDesignApproval, Action::None))
                    }
                }
                Node::Implement => {
                    if ctx.config.gates_each_round() {
                        return Ok(Transition::active(
                            Node::Gate,
                            Action::RunCoreStep { node: Node::Gate },
                        ));
                    }
                    let budget = ctx.budget_n.unwrap_or(status.impl_round_limit);
                    if ctx.config.is_enabled(Role::Audit) {
                        Ok(Transition::active(
                            Node::Audit,
                            Action::StartRole {
                                role: Role::Audit,
                                inject: None,
                            },
                        ))
                    } else {
                        Ok(after_implement_without_audit(status, budget, ctx))
                    }
                }
                Node::Audit => {
                    let budget = ctx.budget_n.unwrap_or(status.impl_round_limit);
                    if status.all_milestones_done() {
                        Ok(after_the_loop(ctx.config))
                    } else if status.impl_round >= budget {
                        Ok(Transition::fail(
                            node,
                            FailureReason::ImplBudget { limit: budget },
                        ))
                    } else {
                        Ok(start_impl(ctx, None))
                    }
                }
                // The retro round produces `lessons.md` and nothing the
                // transition table reads; whatever it concluded, the next
                // step is the rebase.
                Node::Retro => Ok(Transition::active(
                    Node::Rebase,
                    Action::RunCoreStep { node: Node::Rebase },
                )),
                _ => unreachable!("core and waiting nodes were rejected above"),
            }
        }

        // ---- Design stopping point ---------------------------------------
        (
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
            Trigger::Approve,
        ) => {
            if ctx.decisions.any() {
                // Approval with pending decisions means "re-run one design
                // round carrying them", and stop again afterwards (T-09).
                return Ok(Transition::active(
                    Node::Design,
                    Action::StartRole {
                        role: Role::Plan,
                        inject: Some(Inject::Decisions),
                    },
                )
                .consuming());
            }
            Ok(Transition::active(
                Node::Implement,
                Action::StartRole {
                    role: Role::Impl,
                    inject: None,
                },
            )
            .with_budget(ctx.budget_n.unwrap_or(0)))
        }
        (
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
            Trigger::Reject { feedback },
        ) => {
            if feedback.trim().is_empty() {
                return Err(reject("驳回必须附意见"));
            }
            Ok(Transition::active(
                Node::Design,
                Action::StartRole {
                    role: Role::Plan,
                    inject: Some(Inject::DesignFeedback {
                        feedback: feedback.clone(),
                    }),
                },
            ))
        }

        // ---- Merge stopping point ----------------------------------------
        (
            TaskState::Active {
                node: Node::AwaitMerge,
            },
            Trigger::Merge,
        ) => Ok(press_merge(ctx)),

        // ---- Core steps ---------------------------------------------------
        (TaskState::Active { node }, Trigger::CoreStepDone { node: done, result }) => {
            let node = *node;
            if node != *done {
                return Err(reject("核心步骤与当前节点不符"));
            }
            if !node.is_core_step() {
                return Err(reject("该节点不是内核步骤"));
            }
            match (node, result) {
                (Node::Rebase, CoreStepResult::Ok) => {
                    if ctx.config.gate.is_some() {
                        return Ok(Transition::active(
                            Node::MergeGate,
                            Action::RunCoreStep {
                                node: Node::MergeGate,
                            },
                        ));
                    }
                    Ok(if merge_requested {
                        press_merge(ctx)
                    } else {
                        Transition::active(Node::AwaitMerge, Action::None)
                    })
                }
                (Node::Rebase, CoreStepResult::Conflict { files, .. }) => Ok(Transition::active(
                    Node::Implement,
                    Action::StartRole {
                        role: Role::Impl,
                        inject: Some(Inject::RebaseConflict {
                            files: files.clone(),
                        }),
                    },
                )),
                (Node::Merging, CoreStepResult::Behind { .. }) => Ok(Transition::to(
                    TaskState::Requeued {
                        resume: Node::Rebase,
                    },
                    Action::None,
                )),
                (_, CoreStepResult::Behind { .. }) => Err(reject("只有合并步骤可以报告分支落后")),
                (Node::Merging, CoreStepResult::Ok) => {
                    if ctx.config.gate.is_some() {
                        return Ok(Transition::active(
                            Node::MainCheck,
                            Action::RunCoreStep {
                                node: Node::MainCheck,
                            },
                        ));
                    }
                    Ok(Transition::active(
                        Node::Cleanup,
                        Action::RunCoreStep {
                            node: Node::Cleanup,
                        },
                    ))
                }
                // A blocked merge is not a failure: the user fixes their
                // worktree and presses merge again (requirement T-07).
                (Node::Merging, CoreStepResult::Blocked { .. }) => {
                    Ok(Transition::active(Node::AwaitMerge, Action::None))
                }
                (Node::Cleanup, CoreStepResult::Ok) => {
                    Ok(Transition::to(TaskState::Done, Action::None))
                }
                // Cleanup failing after a successful merge must not lose the
                // merge: the task is done, the leftovers are surfaced
                // separately (design §6).
                (Node::Cleanup, CoreStepResult::Failed { .. }) => {
                    Ok(Transition::to(TaskState::Done, Action::None))
                }
                (n, CoreStepResult::Conflict { detail, .. }) => Ok(Transition::fail(
                    n,
                    FailureReason::RebaseConflict {
                        detail: detail.clone(),
                    },
                )),
                (n, CoreStepResult::Failed { detail } | CoreStepResult::Blocked { detail }) => {
                    Ok(Transition::fail(
                        n,
                        FailureReason::CoreStep {
                            node: n,
                            detail: detail.clone(),
                        },
                    ))
                }
                // `Ok` for a core node is handled above; `is_core_step`
                // rules out every other node before this match.
                (n, CoreStepResult::Ok) => Err(reject(&format!("{n:?} 没有成功后继"))),
            }
        }

        // ---- The gate -----------------------------------------------------
        (
            TaskState::Active { node },
            Trigger::GateDone {
                node: done,
                outcome,
                status,
            },
        ) => {
            let node = *node;
            if node != *done || !node.is_gate() {
                return Err(reject("门禁结果与当前节点不符"));
            }
            if node == Node::MergeGate && *outcome == GateOutcome::Passed && merge_requested {
                return Ok(press_merge(ctx));
            }
            after_gate(node, outcome, status.as_deref(), ctx)
        }

        // ---- Everything else ----------------------------------------------
        _ => Err(reject("当前状态不接受该操作")),
    }
}

/// Where each gate leads.
///
/// After an implementation round, a pass goes where that round would have
/// gone without a gate, and a failure sends the work straight back to the
/// implementation role carrying the gate's output — the same round-trip a
/// rebase conflict makes, counted against the same budget so a gate that
/// never passes ends the loop instead of running it forever.
///
/// Before the merge, a failure goes back the same way: the commit the user
/// would be merging does not pass.
///
/// After the merge there is nothing to send back. The task is cleaned up
/// either way; the core raises the alert itself.
fn after_gate(
    node: Node,
    outcome: &GateOutcome,
    status: Option<&StatusBlock>,
    ctx: &Context,
) -> Result<Transition, Rejected> {
    let back_to_impl = |head: &str, command: &str, tail: &str| {
        Transition::active(
            Node::Implement,
            Action::StartRole {
                role: Role::Impl,
                inject: Some(Inject::GateFailed {
                    head: head.to_string(),
                    command: command.to_string(),
                    tail: tail.to_string(),
                }),
            },
        )
    };
    match (node, outcome) {
        (Node::Gate, GateOutcome::Passed) => {
            if ctx.config.is_enabled(Role::Audit) {
                return Ok(Transition::active(
                    Node::Audit,
                    Action::StartRole {
                        role: Role::Audit,
                        inject: None,
                    },
                ));
            }
            let status = status.ok_or_else(|| reject("门禁通过，但读不到设计文档的状态块"))?;
            let budget = ctx.budget_n.unwrap_or(status.impl_round_limit);
            Ok(after_implement_without_audit(status, budget, ctx))
        }
        (
            Node::Gate,
            GateOutcome::Failed {
                head,
                command,
                tail,
            },
        ) => {
            if let Some(status) = status {
                let budget = ctx.budget_n.unwrap_or(status.impl_round_limit);
                if status.impl_round >= budget {
                    return Ok(Transition::fail(
                        Node::Gate,
                        FailureReason::ImplBudget { limit: budget },
                    ));
                }
            }
            Ok(back_to_impl(head, command, tail))
        }
        (Node::MergeGate, GateOutcome::Passed) => {
            Ok(Transition::active(Node::AwaitMerge, Action::None))
        }
        (
            Node::MergeGate,
            GateOutcome::Failed {
                head,
                command,
                tail,
            },
        ) => Ok(back_to_impl(head, command, tail)),
        (Node::MainCheck, _) => Ok(Transition::active(
            Node::Cleanup,
            Action::RunCoreStep {
                node: Node::Cleanup,
            },
        )),
        _ => Err(reject("不是门禁节点")),
    }
}

/// Computes the implementation budget N = factor × initial milestone count
/// (design §5.5). A design with no milestones yields the factor itself, so a
/// task can still make progress rather than starting already out of budget.
pub fn compute_budget(config: &ResolvedConfig, status: &StatusBlock) -> u32 {
    let milestones = status.milestones.len().max(1) as u32;
    config.loop_defaults.budget_factor * milestones
}

#[cfg(test)]
mod tests {
    fn setup_trigger() -> Trigger {
        Trigger::SetupFailed {
            command: "install".into(),
            exit_code: 7,
            log: "output/setup.log".into(),
        }
    }
    fn setup_reason() -> FailureReason {
        FailureReason::Setup {
            command: "install".into(),
            exit_code: 7,
            log: "output/setup.log".into(),
        }
    }
    #[test]
    fn setup_failure_fails_an_active_task_at_its_node_with_command_exit_code_and_log() {
        let config = resolve(&GlobalConfig::default(), &ProjectConfig::default());
        for node in Node::ALL
            .iter()
            .chain(Node::BUG.iter())
            .copied()
            .filter(|node| !node.awaits_user())
        {
            let result = apply(&active(node), &setup_trigger(), &ctx(&config)).unwrap();
            assert_eq!(
                result.next,
                TaskState::Failed {
                    at: node,
                    reason: setup_reason()
                }
            );
            assert_eq!(result.action, Action::None);
        }
    }
    #[test]
    fn setup_failure_is_rejected_unless_the_task_is_running() {
        let config = resolve(&GlobalConfig::default(), &ProjectConfig::default());
        let mut states = vec![
            TaskState::Draft,
            TaskState::Queued,
            TaskState::Done,
            TaskState::Cancelled,
            TaskState::Failed {
                at: Node::Gate,
                reason: setup_reason(),
            },
        ];
        for node in Node::ALL.iter().chain(Node::BUG.iter()).copied() {
            states.extend([
                TaskState::Paused { resume: node },
                TaskState::Stopped { at: node },
                TaskState::Requeued { resume: node },
            ]);
            if node.awaits_user() {
                states.push(active(node));
            }
        }
        for state in states {
            assert!(
                apply(&state, &setup_trigger(), &ctx(&config)).is_err(),
                "{state:?}"
            );
        }
    }
    #[test]
    fn setup_failure_reason_serializes_as_kind_setup() {
        let value = serde_json::to_value(setup_reason()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"kind":"setup", "command":"install", "exit_code":7, "log":"output/setup.log"})
        );
        assert_eq!(
            serde_json::from_value::<FailureReason>(value).unwrap(),
            setup_reason()
        );
    }
    use super::*;
    use crate::config::{GlobalConfig, ProjectConfig, RoleOverrides, resolve};
    use crate::status_block::{ConvergenceMode, Milestone, MilestoneState};

    fn requested_step(node: Node, result: CoreStepResult, config: &ResolvedConfig) -> Transition {
        apply_with_merge_request(
            &active(node),
            &Trigger::CoreStepDone { node, result },
            &ctx(config),
            true,
        )
        .unwrap()
    }

    fn same_failure_trigger(node: Node) -> Trigger {
        Trigger::GateSameFailure {
            node,
            count: 3,
            summary: "missing ws".into(),
            log: "/gate.log".into(),
        }
    }

    fn same_failure_state(node: Node) -> TaskState {
        TaskState::Failed {
            at: node,
            reason: FailureReason::GateSameFailure {
                count: 3,
                summary: "missing ws".into(),
                log: "/gate.log".into(),
            },
        }
    }

    #[test]
    fn same_failure_gate_trips() {
        let t = apply(
            &active(Node::Gate),
            &same_failure_trigger(Node::Gate),
            &ctx(&cfg()),
        )
        .unwrap();
        assert_eq!(t.next, same_failure_state(Node::Gate));
        assert_eq!(t.action, Action::None);
    }

    #[test]
    fn same_failure_merge_gate_trips() {
        let t = apply_with_merge_request(
            &active(Node::MergeGate),
            &same_failure_trigger(Node::MergeGate),
            &ctx(&cfg()),
            true,
        )
        .unwrap();
        assert_eq!(t.next, same_failure_state(Node::MergeGate));
        assert_eq!(t.action, Action::None);
        assert!(!t.merge_requested);
    }

    #[test]
    fn same_failure_takes_priority_over_exhausted_budget() {
        let c = cfg();
        let mut context = ctx(&c);
        context.budget_n = Some(0);
        let t = apply(
            &active(Node::Gate),
            &same_failure_trigger(Node::Gate),
            &context,
        )
        .unwrap();
        assert_eq!(t.next, same_failure_state(Node::Gate));
        assert_eq!(t.action, Action::None);
    }

    #[test]
    fn same_failure_rejects_wrong_nodes() {
        for node in [
            Node::MainCheck,
            Node::ReproCheck,
            Node::Implement,
            Node::FixGate,
        ] {
            let e = apply(&active(node), &same_failure_trigger(node), &ctx(&cfg())).unwrap_err();
            assert!(e.reason.contains("门禁结果与当前节点不符"));
        }
        assert!(
            apply(
                &active(Node::Gate),
                &same_failure_trigger(Node::MergeGate),
                &ctx(&cfg())
            )
            .is_err()
        );
        assert!(
            apply(
                &TaskState::Stopped { at: Node::Gate },
                &same_failure_trigger(Node::Gate),
                &ctx(&cfg())
            )
            .is_err()
        );
    }

    #[test]
    fn same_failure_rerun_discards_gate_runs() {
        for node in [Node::Gate, Node::MergeGate] {
            let t = apply(
                &same_failure_state(node),
                &Trigger::RerunFrom { node },
                &ctx(&cfg()),
            )
            .unwrap();
            assert_eq!(t.next, active(node));
            assert_eq!(t.action, Action::DiscardGateRuns { node });
        }
    }

    #[test]
    fn same_failure_rerun_rejects_other_failure_or_gate() {
        for node in [Node::Gate, Node::MergeGate, Node::FixGate] {
            let state = TaskState::Failed {
                at: node,
                reason: FailureReason::Infeasible,
            };
            assert!(apply(&state, &Trigger::RerunFrom { node }, &ctx(&cfg())).is_err());
            assert!(
                apply(
                    &TaskState::Stopped { at: node },
                    &Trigger::RerunFrom { node },
                    &ctx(&cfg())
                )
                .is_err()
            );
        }
        for node in [Node::MergeGate, Node::FixGate] {
            assert!(
                apply(
                    &same_failure_state(Node::Gate),
                    &Trigger::RerunFrom { node },
                    &ctx(&cfg())
                )
                .is_err()
            );
        }
    }

    #[test]
    fn same_failure_rerun_implement_starts_role() {
        let t = apply(
            &same_failure_state(Node::Gate),
            &Trigger::RerunFrom {
                node: Node::Implement,
            },
            &ctx(&cfg()),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: None
            }
        );
    }

    #[test]
    fn same_failure_extend_adds_budget_and_discards_runs() {
        let c = cfg();
        let mut context = ctx(&c);
        for node in [Node::Gate, Node::MergeGate] {
            context.budget_n = Some(25);
            let t = apply(
                &same_failure_state(node),
                &Trigger::ExtendBudget { extra_rounds: 5 },
                &context,
            )
            .unwrap();
            assert_eq!(t.next, active(node));
            assert_eq!(t.action, Action::DiscardGateRuns { node });
            assert_eq!(t.budget_n, Some(30));
        }
        context.budget_n = None;
        let e = apply(
            &same_failure_state(Node::Gate),
            &Trigger::ExtendBudget { extra_rounds: 5 },
            &context,
        )
        .unwrap_err();
        assert!(e.reason.contains("读不到实现预算"));
        context.budget_n = Some(u32::MAX);
        assert!(
            apply(
                &same_failure_state(Node::Gate),
                &Trigger::ExtendBudget { extra_rounds: 1 },
                &context
            )
            .is_err()
        );
        context.budget_n = Some(0);
        let t = apply(
            &same_failure_state(Node::Gate),
            &Trigger::ExtendBudget {
                extra_rounds: u32::MAX,
            },
            &context,
        )
        .unwrap();
        assert_eq!(t.budget_n, Some(u32::MAX));
    }

    #[test]
    fn same_failure_extend_zero_is_rejected() {
        let e = apply(
            &same_failure_state(Node::Gate),
            &Trigger::ExtendBudget { extra_rounds: 0 },
            &ctx(&cfg()),
        )
        .unwrap_err();
        assert!(e.reason.contains("追加轮次必须大于 0"));
    }

    #[test]
    fn same_failure_reason_json_shape() {
        let reason = FailureReason::GateSameFailure {
            count: 3,
            summary: "S".into(),
            log: "/l".into(),
        };
        let value =
            serde_json::json!({"kind":"gate_same_failure","count":3,"summary":"S","log":"/l"});
        assert_eq!(serde_json::to_value(&reason).unwrap(), value);
        assert_eq!(
            serde_json::from_value::<FailureReason>(value).unwrap(),
            reason
        );
    }

    #[test]
    fn merging_behind_the_default_branch_queues_a_rebase_carrying_the_request() {
        for repos in [vec![], vec!["repo".into()], vec!["a".into(), "b".into()]] {
            for requested in [false, true] {
                let t = apply_with_merge_request(
                    &active(Node::Merging),
                    &Trigger::CoreStepDone {
                        node: Node::Merging,
                        result: CoreStepResult::Behind {
                            repos: repos.clone(),
                            detail: String::new(),
                        },
                    },
                    &ctx(&cfg()),
                    requested,
                )
                .unwrap();
                assert_eq!(
                    t.next,
                    TaskState::Requeued {
                        resume: Node::Rebase
                    }
                );
                assert_eq!(t.action, Action::None);
                assert!(t.merge_requested);
            }
        }
    }

    #[test]
    fn a_requested_rebase_without_a_gate_goes_straight_to_merging() {
        let t = requested_step(Node::Rebase, CoreStepResult::Ok, &cfg());
        assert_eq!(t.next, active(Node::Merging));
        assert_eq!(
            t.action,
            Action::RunCoreStep {
                node: Node::Merging
            }
        );
        assert!(!t.merge_requested);
    }

    #[test]
    fn a_requested_rebase_with_a_gate_still_runs_the_merge_gate() {
        let t = requested_step(Node::Rebase, CoreStepResult::Ok, &cfg_with_gate());
        assert_eq!(t.next, active(Node::MergeGate));
        assert_eq!(
            t.action,
            Action::RunCoreStep {
                node: Node::MergeGate
            }
        );
        assert!(t.merge_requested);
    }

    #[test]
    fn a_passed_merge_gate_on_a_requested_merge_goes_straight_to_merging() {
        let t = apply_with_merge_request(
            &active(Node::MergeGate),
            &gate_done(Node::MergeGate, GateOutcome::Passed, None),
            &ctx(&cfg_with_gate()),
            true,
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Merging));
        assert_eq!(
            t.action,
            Action::RunCoreStep {
                node: Node::Merging
            }
        );
        assert!(!t.merge_requested);
    }

    #[test]
    fn without_a_request_rebase_and_merge_gate_still_stop_at_await_merge() {
        for (node, trigger, c) in [
            (
                Node::Rebase,
                Trigger::CoreStepDone {
                    node: Node::Rebase,
                    result: CoreStepResult::Ok,
                },
                cfg(),
            ),
            (
                Node::MergeGate,
                gate_done(Node::MergeGate, GateOutcome::Passed, None),
                cfg_with_gate(),
            ),
        ] {
            let t = apply_with_merge_request(&active(node), &trigger, &ctx(&c), false).unwrap();
            assert_eq!(t.next, active(Node::AwaitMerge));
            assert_eq!(t.action, Action::None);
            assert!(!t.merge_requested);
        }
    }

    #[test]
    fn a_conflicting_requested_rebase_goes_back_to_implement_and_drops_the_request() {
        let files = vec!["conflict".into()];
        let t = requested_step(
            Node::Rebase,
            CoreStepResult::Conflict {
                files: files.clone(),
                detail: "conflict".into(),
            },
            &cfg(),
        );
        assert_eq!(t.next, active(Node::Implement));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: Some(Inject::RebaseConflict { files })
            }
        );
        assert!(!t.merge_requested);
        let failed = requested_step(
            Node::Rebase,
            CoreStepResult::Failed {
                detail: "read failure".into(),
            },
            &cfg(),
        );
        assert!(matches!(failed.next, TaskState::Failed { .. }));
        assert!(!failed.merge_requested);
    }

    #[test]
    fn a_failed_merge_gate_on_a_requested_merge_goes_back_to_implement_and_drops_the_request() {
        let t = apply_with_merge_request(
            &active(Node::MergeGate),
            &gate_done(Node::MergeGate, red(), None),
            &ctx(&cfg_with_gate()),
            true,
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
        assert!(matches!(
            t.action,
            Action::StartRole {
                inject: Some(Inject::GateFailed { .. }),
                ..
            }
        ));
        assert!(!t.merge_requested);
    }

    #[test]
    fn the_request_survives_the_queue_and_a_pause_but_not_a_stop() {
        let c = cfg_with_gate();
        for node in [Node::Rebase, Node::MergeGate] {
            let mut state = TaskState::Requeued { resume: node };
            for trigger in [
                Trigger::SlotAvailable,
                Trigger::Pause,
                Trigger::Resume,
                Trigger::SlotAvailable,
            ] {
                let t = apply_with_merge_request(&state, &trigger, &ctx(&c), true).unwrap();
                assert!(t.merge_requested, "{state:?} {trigger:?}");
                state = t.next;
            }
            assert_eq!(state, active(node));
            for state in [state, TaskState::Paused { resume: node }] {
                let stopped =
                    apply_with_merge_request(&state, &Trigger::Stop, &ctx(&c), true).unwrap();
                assert!(!stopped.merge_requested);
                let resumed =
                    apply_with_merge_request(&stopped.next, &Trigger::Resume, &ctx(&c), true)
                        .unwrap();
                assert!(!resumed.merge_requested);
            }
            for state in [
                active(node),
                TaskState::Requeued { resume: node },
                TaskState::Paused { resume: node },
            ] {
                let t = apply_with_merge_request(&state, &Trigger::Cancel, &ctx(&c), true).unwrap();
                assert_eq!(t.next, TaskState::Cancelled);
                assert!(!t.merge_requested);
            }
        }
    }

    #[test]
    fn a_rebase_entered_from_anywhere_but_the_merge_carries_no_request() {
        let c = cfg();
        for state in [
            TaskState::Stopped { at: Node::Rebase },
            TaskState::Stopped {
                at: Node::Implement,
            },
        ] {
            let t = apply_with_merge_request(
                &state,
                &Trigger::RerunFrom { node: Node::Rebase },
                &ctx(&c),
                true,
            )
            .unwrap();
            assert_eq!(t.next, active(Node::Rebase));
            assert!(!t.merge_requested);
        }
        let t = apply_with_merge_request(
            &active(Node::Retro),
            &ended(block(DocStatus::Implementing, &[MilestoneState::Done])),
            &ctx(&c),
            true,
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Rebase));
        assert!(!t.merge_requested);
        // Every accepted transition from outside the stretch must drop stale authorization.
        for node in Node::ALL {
            if matches!(node, Node::Rebase | Node::MergeGate) {
                continue;
            }
            for trigger in [
                Trigger::Merge,
                Trigger::Resume,
                Trigger::SlotAvailable,
                Trigger::Cancel,
                Trigger::CoreStepDone {
                    node,
                    result: CoreStepResult::Ok,
                },
            ] {
                if let Ok(t) = apply_with_merge_request(&active(node), &trigger, &ctx(&c), true) {
                    assert!(!t.merge_requested, "{node:?} {trigger:?}");
                }
            }
        }
    }

    #[test]
    fn backlog_items_included_during_the_automatic_path_are_absorbed_before_merging() {
        for (node, trigger, c) in [
            (
                Node::Rebase,
                Trigger::CoreStepDone {
                    node: Node::Rebase,
                    result: CoreStepResult::Ok,
                },
                cfg(),
            ),
            (
                Node::MergeGate,
                gate_done(Node::MergeGate, GateOutcome::Passed, None),
                cfg_with_gate(),
            ),
        ] {
            let mut context = ctx(&c);
            context.decisions.included = 1;
            let t = apply_with_merge_request(&active(node), &trigger, &context, true).unwrap();
            assert_eq!(t.next, active(Node::Implement));
            assert!(t.consumes_decisions);
            assert_eq!(t.budget_n, Some(25 + c.loop_defaults.budget_factor));
            assert!(!t.merge_requested);
        }
    }

    #[test]
    fn behind_is_only_accepted_from_merging() {
        for node in Node::ALL {
            let result = apply_with_merge_request(
                &active(node),
                &Trigger::CoreStepDone {
                    node,
                    result: CoreStepResult::Behind {
                        repos: vec![],
                        detail: String::new(),
                    },
                },
                &ctx(&cfg()),
                true,
            );
            assert_eq!(result.is_ok(), node == Node::Merging, "{node:?}");
        }
        assert!(
            apply_with_merge_request(
                &active(Node::Rebase),
                &Trigger::CoreStepDone {
                    node: Node::Merging,
                    result: CoreStepResult::Behind {
                        repos: vec![],
                        detail: String::new()
                    }
                },
                &ctx(&cfg()),
                true
            )
            .is_err()
        );
    }
    fn cfg() -> ResolvedConfig {
        resolve(&GlobalConfig::default(), &ProjectConfig::default())
    }

    fn cfg_without(role: Role) -> ResolvedConfig {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            role,
            RoleOverrides {
                enabled: Some(false),
                ..Default::default()
            },
        );
        resolve(&GlobalConfig::default(), &project)
    }

    fn ctx<'a>(config: &'a ResolvedConfig) -> Context<'a> {
        Context {
            config,
            budget_n: Some(25),
            decisions: PendingDecisions::default(),
        }
    }

    fn block(status: DocStatus, states: &[MilestoneState]) -> StatusBlock {
        StatusBlock {
            status,
            design_round: 1,
            design_round_limit: 15,
            impl_round: 1,
            impl_round_limit: 25,
            current_milestone: None,
            current_milestone_reopens: 0,
            convergence_mode: ConvergenceMode::Normal,
            next_action: "无".into(),
            repos: vec![],
            milestones: states
                .iter()
                .enumerate()
                .map(|(i, s)| Milestone {
                    id: format!("M-{:02}", i + 1),
                    state: *s,
                    title: format!("m{i}"),
                    reopen_count: 0,
                    reopen_domains: vec![],
                })
                .collect(),
            backlog: vec![],
            disputes: vec![],
            manual_items: vec![],
        }
    }

    fn ended(status: StatusBlock) -> Trigger {
        Trigger::SessionEnded {
            outcome: SessionOutcome::Ok {
                status: Box::new(status),
            },
        }
    }

    fn active(node: Node) -> TaskState {
        TaskState::Active { node }
    }

    fn cfg_with_gate() -> ResolvedConfig {
        let project = ProjectConfig {
            gate: Some(crate::config::GateConfig {
                command: "make check".into(),
                timeout_minutes: 5,
            }),
            ..Default::default()
        };
        resolve(&GlobalConfig::default(), &project)
    }

    fn gate_done(node: Node, outcome: GateOutcome, status: Option<StatusBlock>) -> Trigger {
        Trigger::GateDone {
            node,
            outcome,
            status: status.map(Box::new),
        }
    }

    fn red() -> GateOutcome {
        GateOutcome::Failed {
            head: "abc".into(),
            command: "make check".into(),
            tail: "1 failed".into(),
        }
    }

    fn before_merge_config() -> ResolvedConfig {
        let mut c = cfg_with_gate();
        c.gate_run = crate::config::GateRun::BeforeMerge;
        c
    }

    #[test]
    fn before_merge_implement_with_audit_equals_no_gate() {
        let c = before_merge_config();
        let mut none = c.clone();
        none.gate = None;
        let event = ended(block(DocStatus::Implementing, &[MilestoneState::Pending]));
        let actual = apply(&active(Node::Implement), &event, &ctx(&c)).unwrap();
        assert_eq!(
            actual,
            apply(&active(Node::Implement), &event, &ctx(&none)).unwrap()
        );
        assert_eq!(actual.next, active(Node::Audit));
    }

    #[test]
    fn before_merge_implement_without_audit_equals_no_gate() {
        // Open, finished with/without retro, budget exhausted, included decision.
        for (open, retro, round, included) in [
            (true, true, 1, 0),
            (false, true, 1, 0),
            (false, false, 1, 0),
            (true, true, 25, 0),
            (true, true, 1, 2),
        ] {
            let mut c = cfg_without(Role::Audit);
            c.gate = cfg_with_gate().gate;
            c.gate_run = crate::config::GateRun::BeforeMerge;
            c.roles
                .iter_mut()
                .find(|r| r.role == Role::Retro)
                .unwrap()
                .config
                .enabled = retro;
            let mut none = c.clone();
            none.gate = None;
            let mut status = block(
                DocStatus::Implementing,
                &[if open {
                    MilestoneState::Open
                } else {
                    MilestoneState::Done
                }],
            );
            status.impl_round = round;
            let event = ended(status);
            let actual = apply(
                &active(Node::Implement),
                &event,
                &with_included(&c, included, 25),
            )
            .unwrap();
            assert_eq!(
                actual,
                apply(
                    &active(Node::Implement),
                    &event,
                    &with_included(&none, included, 25)
                )
                .unwrap()
            );
            if included > 0 {
                assert!(actual.consumes_decisions);
                assert_eq!(actual.budget_n, Some(35));
            }
        }
    }

    #[test]
    fn before_merge_rebase_and_merge_keep_full_gate_nodes() {
        let c = before_merge_config();
        for (node, next) in [
            (Node::Rebase, Node::MergeGate),
            (Node::Merging, Node::MainCheck),
        ] {
            let event = Trigger::CoreStepDone {
                node,
                result: CoreStepResult::Ok,
            };
            let actual = apply(&active(node), &event, &ctx(&c)).unwrap();
            assert_eq!(actual.next, active(next));
            assert_eq!(actual.action, Action::RunCoreStep { node: next });
        }
    }

    #[test]
    fn before_merge_merge_gate_outcomes_equal_each_round() {
        let c = before_merge_config();
        for requested in [false, true] {
            for outcome in [GateOutcome::Passed, red()] {
                let event = gate_done(Node::MergeGate, outcome, None);
                assert_eq!(
                    apply_with_merge_request(&active(Node::MergeGate), &event, &ctx(&c), requested)
                        .unwrap(),
                    apply_with_merge_request(
                        &active(Node::MergeGate),
                        &event,
                        &ctx(&cfg_with_gate()),
                        requested
                    )
                    .unwrap()
                );
            }
        }
    }

    #[test]
    fn before_merge_existing_gate_pass_still_uses_after_gate() {
        let c = before_merge_config();
        let event = gate_done(Node::Gate, GateOutcome::Passed, None);
        let actual = apply(&active(Node::Gate), &event, &ctx(&c)).unwrap();
        assert_eq!(
            actual,
            apply(&active(Node::Gate), &event, &ctx(&cfg_with_gate())).unwrap()
        );
        assert_eq!(actual.next, active(Node::Audit));
    }

    // ---- the gate --------------------------------------------------------

    #[test]
    fn with_no_gate_configured_nothing_about_the_loop_changes() {
        let c = cfg();
        let t = apply(
            &active(Node::Implement),
            &ended(block(DocStatus::Implementing, &[MilestoneState::Pending])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Audit));
        let rebased = apply(
            &active(Node::Rebase),
            &Trigger::CoreStepDone {
                node: Node::Rebase,
                result: CoreStepResult::Ok,
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(rebased.next, active(Node::AwaitMerge));
        let merged = apply(
            &active(Node::Merging),
            &Trigger::CoreStepDone {
                node: Node::Merging,
                result: CoreStepResult::Ok,
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(merged.next, active(Node::Cleanup));
    }

    #[test]
    fn an_implementation_round_goes_through_the_gate_before_the_audit() {
        let c = cfg_with_gate();
        let t = apply(
            &active(Node::Implement),
            &ended(block(DocStatus::Implementing, &[MilestoneState::Pending])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Gate));
        assert_eq!(t.action, Action::RunCoreStep { node: Node::Gate });

        let passed = apply(
            &active(Node::Gate),
            &gate_done(Node::Gate, GateOutcome::Passed, None),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(passed.next, active(Node::Audit));
    }

    #[test]
    fn a_red_gate_sends_the_round_back_with_its_output() {
        let c = cfg_with_gate();
        let t = apply(
            &active(Node::Gate),
            &gate_done(
                Node::Gate,
                red(),
                Some(block(DocStatus::Implementing, &[MilestoneState::Pending])),
            ),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
        assert!(matches!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: Some(Inject::GateFailed { .. })
            }
        ));
    }

    #[test]
    fn a_gate_that_never_passes_ends_the_loop_at_the_budget() {
        let c = cfg_with_gate();
        let mut status = block(DocStatus::Implementing, &[MilestoneState::Pending]);
        status.impl_round = 25;
        let t = apply(
            &active(Node::Gate),
            &gate_done(Node::Gate, red(), Some(status)),
            &ctx(&c),
        )
        .unwrap();
        assert!(matches!(
            t.next,
            TaskState::Failed {
                reason: FailureReason::ImplBudget { limit: 25 },
                ..
            }
        ));
    }

    #[test]
    fn with_audit_off_a_passing_gate_decides_from_the_document() {
        let mut c = cfg_with_gate();
        c = ResolvedConfig {
            roles: cfg_without(Role::Audit).roles,
            ..c
        };
        let t = apply(
            &active(Node::Gate),
            &gate_done(
                Node::Gate,
                GateOutcome::Passed,
                Some(block(DocStatus::Implementing, &[MilestoneState::Open])),
            ),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement), "a milestone is still open");
        assert!(
            apply(
                &active(Node::Gate),
                &gate_done(Node::Gate, GateOutcome::Passed, None),
                &ctx(&c)
            )
            .is_err(),
            "and without the document there is nothing to decide from"
        );
    }

    #[test]
    fn the_rebased_commit_is_gated_before_the_merge_stop() {
        let c = cfg_with_gate();
        let t = apply(
            &active(Node::Rebase),
            &Trigger::CoreStepDone {
                node: Node::Rebase,
                result: CoreStepResult::Ok,
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::MergeGate));
        let green = apply(
            &active(Node::MergeGate),
            &gate_done(Node::MergeGate, GateOutcome::Passed, None),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(green.next, active(Node::AwaitMerge));
        let failed = apply(
            &active(Node::MergeGate),
            &gate_done(Node::MergeGate, red(), None),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(failed.next, active(Node::Implement));
    }

    #[test]
    fn main_failing_after_the_merge_does_not_undo_it() {
        let c = cfg_with_gate();
        let merged = apply(
            &active(Node::Merging),
            &Trigger::CoreStepDone {
                node: Node::Merging,
                result: CoreStepResult::Ok,
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(merged.next, active(Node::MainCheck));
        for outcome in [GateOutcome::Passed, red()] {
            let t = apply(
                &active(Node::MainCheck),
                &gate_done(Node::MainCheck, outcome, None),
                &ctx(&c),
            )
            .unwrap();
            assert_eq!(t.next, active(Node::Cleanup));
        }
    }

    #[test]
    fn a_gate_result_for_another_node_is_refused() {
        let c = cfg_with_gate();
        assert!(
            apply(
                &active(Node::Audit),
                &gate_done(Node::Gate, GateOutcome::Passed, None),
                &ctx(&c)
            )
            .is_err()
        );
    }

    // ---- happy path ------------------------------------------------------

    #[test]
    fn queued_starts_intake_when_a_slot_opens() {
        let c = cfg();
        let t = apply(&TaskState::Queued, &Trigger::SlotAvailable, &ctx(&c)).unwrap();
        assert_eq!(t.next, active(Node::Intake));
        assert_eq!(t.action, Action::StartIntake);
    }

    #[test]
    fn intake_hands_off_to_the_design_role() {
        let c = cfg();
        let t = apply(
            &active(Node::Intake),
            &ended(block(DocStatus::Designing, &[])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Design));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Plan,
                inject: None
            }
        );
    }

    #[test]
    fn design_review_adjudicate_cycle_runs_in_order() {
        let c = cfg();
        let after_design = apply(
            &active(Node::Design),
            &ended(block(DocStatus::Designing, &[])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(after_design.next, active(Node::Review));

        let after_review = apply(
            &active(Node::Review),
            &ended(block(DocStatus::Designing, &[])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(after_review.next, active(Node::Adjudicate));

        // Still 设计中 => another review round.
        let another = apply(
            &active(Node::Adjudicate),
            &ended(block(DocStatus::Designing, &[])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(another.next, active(Node::Review));
    }

    #[test]
    fn adjudicate_finalising_the_design_stops_for_the_user() {
        let c = cfg();
        let t = apply(
            &active(Node::Adjudicate),
            &ended(block(DocStatus::Implementing, &[MilestoneState::Open])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::AwaitDesignApproval));
        assert_eq!(t.action, Action::None);
    }

    #[test]
    fn approval_enters_the_implementation_loop() {
        let c = cfg();
        let t = apply(
            &active(Node::AwaitDesignApproval),
            &Trigger::Approve,
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: None
            }
        );
    }

    #[test]
    fn implement_hands_off_to_audit() {
        let c = cfg();
        let t = apply(
            &active(Node::Implement),
            &ended(block(DocStatus::Implementing, &[MilestoneState::Pending])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Audit));
    }

    #[test]
    fn audit_reopening_a_milestone_goes_back_to_implement() {
        let c = cfg();
        let t = apply(
            &active(Node::Audit),
            &ended(block(
                DocStatus::Implementing,
                &[MilestoneState::Done, MilestoneState::Open],
            )),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
    }

    #[test]
    fn audit_closing_everything_goes_to_the_retro_round() {
        let c = cfg();
        let t = apply(
            &active(Node::Audit),
            &ended(block(
                DocStatus::Implementing,
                &[MilestoneState::Done, MilestoneState::Done],
            )),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Retro));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Retro,
                inject: None
            }
        );
    }

    #[test]
    fn the_retro_round_hands_off_to_the_rebase() {
        let c = cfg();
        let t = apply(
            &active(Node::Retro),
            &ended(block(
                DocStatus::Implementing,
                &[MilestoneState::Done, MilestoneState::Done],
            )),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Rebase));
        assert_eq!(t.action, Action::RunCoreStep { node: Node::Rebase });
    }

    #[test]
    fn disabling_the_retro_round_goes_straight_from_audit_to_rebase() {
        // Requirement C-05: every role can be switched off, and switching one
        // off must not strand the task at the node it would have run.
        let c = cfg_without(Role::Retro);
        let t = apply(
            &active(Node::Audit),
            &ended(block(
                DocStatus::Implementing,
                &[MilestoneState::Done, MilestoneState::Done],
            )),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Rebase));
    }

    #[test]
    fn a_clean_rebase_reaches_the_merge_stopping_point() {
        let c = cfg();
        let t = apply(
            &active(Node::Rebase),
            &Trigger::CoreStepDone {
                node: Node::Rebase,
                result: CoreStepResult::Ok,
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::AwaitMerge));
    }

    #[test]
    fn merge_then_cleanup_reaches_done() {
        let c = cfg();
        let merged = apply(&active(Node::AwaitMerge), &Trigger::Merge, &ctx(&c)).unwrap();
        assert_eq!(merged.next, active(Node::Merging));

        let cleaned = apply(
            &active(Node::Merging),
            &Trigger::CoreStepDone {
                node: Node::Merging,
                result: CoreStepResult::Ok,
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(cleaned.next, active(Node::Cleanup));

        let done = apply(
            &active(Node::Cleanup),
            &Trigger::CoreStepDone {
                node: Node::Cleanup,
                result: CoreStepResult::Ok,
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(done.next, TaskState::Done);
    }

    // ---- role toggles ----------------------------------------------------

    #[test]
    fn disabling_review_skips_adjudicate_too() {
        let c = cfg_without(Role::Review);
        let t = apply(
            &active(Node::Design),
            &ended(block(DocStatus::Designing, &[])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::AwaitDesignApproval));
    }

    #[test]
    fn disabling_adjudicate_takes_the_review_verdict_as_final() {
        let c = cfg_without(Role::Adjudicate);
        let t = apply(
            &active(Node::Review),
            &ended(block(DocStatus::Designing, &[])),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::AwaitDesignApproval));
    }

    #[test]
    fn disabling_audit_lets_pending_milestones_finish_the_loop() {
        let c = cfg_without(Role::Audit);
        let t = apply(
            &active(Node::Implement),
            &ended(block(
                DocStatus::Implementing,
                &[MilestoneState::Pending, MilestoneState::Done],
            )),
            &ctx(&c),
        )
        .unwrap();
        // With no independent check, "no milestone is still open" ends the
        // loop — and the loop ends at the retro round like any other.
        assert_eq!(t.next, active(Node::Retro));
    }

    #[test]
    fn disabling_audit_still_loops_while_a_milestone_is_open() {
        let c = cfg_without(Role::Audit);
        let t = apply(
            &active(Node::Implement),
            &ended(block(
                DocStatus::Implementing,
                &[MilestoneState::Open, MilestoneState::Done],
            )),
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
    }

    // ---- budgets ---------------------------------------------------------

    #[test]
    fn the_design_budget_fails_the_task_at_its_limit() {
        let c = cfg();
        let mut status = block(DocStatus::Designing, &[]);
        status.design_round = 15;
        status.design_round_limit = 15;
        let t = apply(&active(Node::Adjudicate), &ended(status), &ctx(&c)).unwrap();
        assert_eq!(
            t.next,
            TaskState::Failed {
                at: Node::Adjudicate,
                reason: FailureReason::DesignBudget { limit: 15 }
            }
        );
    }

    #[test]
    fn the_implementation_budget_fails_the_task_at_its_limit() {
        let c = cfg();
        let mut status = block(DocStatus::Implementing, &[MilestoneState::Open]);
        status.impl_round = 25;
        let t = apply(&active(Node::Audit), &ended(status), &ctx(&c)).unwrap();
        assert_eq!(
            t.next,
            TaskState::Failed {
                at: Node::Audit,
                reason: FailureReason::ImplBudget { limit: 25 }
            }
        );
    }

    #[test]
    fn a_finished_design_at_the_round_limit_is_not_a_budget_failure() {
        // Hitting the last round *and* finalising is success, not failure.
        let c = cfg();
        let mut status = block(DocStatus::Implementing, &[MilestoneState::Open]);
        status.design_round = 15;
        status.design_round_limit = 15;
        let t = apply(&active(Node::Adjudicate), &ended(status), &ctx(&c)).unwrap();
        assert_eq!(t.next, active(Node::AwaitDesignApproval));
    }

    #[test]
    fn compute_budget_multiplies_the_factor_by_the_milestone_count() {
        let c = cfg();
        let status = block(DocStatus::Implementing, &[MilestoneState::Open; 5]);
        assert_eq!(compute_budget(&c, &status), 25);
    }

    #[test]
    fn compute_budget_never_returns_zero_for_a_design_without_milestones() {
        let c = cfg();
        let status = block(DocStatus::Implementing, &[]);
        assert_eq!(compute_budget(&c, &status), c.loop_defaults.budget_factor);
    }

    #[test]
    fn extending_the_budget_resumes_implementation_with_a_higher_limit() {
        let c = cfg();
        let failed = TaskState::Failed {
            at: Node::Audit,
            reason: FailureReason::ImplBudget { limit: 25 },
        };
        let t = apply(
            &failed,
            &Trigger::ExtendBudget { extra_rounds: 5 },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
        assert_eq!(t.budget_n, Some(30));
    }

    #[test]
    fn extending_a_design_budget_failure_restarts_the_design_round() {
        let c = cfg();
        let failed = TaskState::Failed {
            at: Node::Adjudicate,
            reason: FailureReason::DesignBudget { limit: 15 },
        };
        let t = apply(
            &failed,
            &Trigger::ExtendBudget { extra_rounds: 5 },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Design));
    }

    #[test]
    fn extending_a_non_budget_failure_is_rejected() {
        let c = cfg();
        let failed = TaskState::Failed {
            at: Node::Implement,
            reason: FailureReason::Infeasible,
        };
        assert!(
            apply(
                &failed,
                &Trigger::ExtendBudget { extra_rounds: 5 },
                &ctx(&c)
            )
            .is_err()
        );
    }

    #[test]
    fn extending_by_zero_rounds_is_rejected() {
        let c = cfg();
        let failed = TaskState::Failed {
            at: Node::Audit,
            reason: FailureReason::ImplBudget { limit: 25 },
        };
        assert!(
            apply(
                &failed,
                &Trigger::ExtendBudget { extra_rounds: 0 },
                &ctx(&c)
            )
            .is_err()
        );
    }

    // ---- design stopping point ------------------------------------------

    #[test]
    fn rejection_re_runs_design_with_the_users_words() {
        let c = cfg();
        let t = apply(
            &active(Node::AwaitDesignApproval),
            &Trigger::Reject {
                feedback: "确认邮件只发登录用户".into(),
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Design));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Plan,
                inject: Some(Inject::DesignFeedback {
                    feedback: "确认邮件只发登录用户".into()
                })
            }
        );
    }

    #[test]
    fn rejection_without_feedback_is_refused() {
        let c = cfg();
        assert!(
            apply(
                &active(Node::AwaitDesignApproval),
                &Trigger::Reject {
                    feedback: "   ".into()
                },
                &ctx(&c),
            )
            .is_err()
        );
    }

    #[test]
    fn approving_with_pending_decisions_runs_one_more_design_round() {
        let c = cfg();
        let context = Context {
            config: &c,
            budget_n: None,
            decisions: PendingDecisions {
                included: 0,
                ruled: 1,
            },
        };
        let t = apply(
            &active(Node::AwaitDesignApproval),
            &Trigger::Approve,
            &context,
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Design));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Plan,
                inject: Some(Inject::Decisions)
            }
        );
        assert!(t.consumes_decisions);
    }

    // ---- Backlog absorbed at the top of each implementation round --------

    /// A context with `included` Backlog items waiting to be absorbed.
    fn with_included(c: &ResolvedConfig, included: u32, budget: u32) -> Context<'_> {
        Context {
            config: c,
            budget_n: Some(budget),
            decisions: PendingDecisions { included, ruled: 0 },
        }
    }

    #[test]
    fn an_audit_that_reopens_absorbs_the_backlog_items_decided_since_the_last_round() {
        // The behaviour this replaces held every included item until the merge
        // stopping point, so a decision made during the loop did nothing for
        // hours and then turned the 合并 button into "go back to work".
        let c = cfg();
        let context = with_included(&c, 2, 25);
        let doc = block(DocStatus::Implementing, &[MilestoneState::Open]);
        let t = apply(
            &active(Node::Audit),
            &Trigger::SessionEnded {
                outcome: SessionOutcome::Ok {
                    status: Box::new(doc),
                },
            },
            &context,
        )
        .unwrap();

        assert_eq!(t.next, active(Node::Implement));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: Some(Inject::Decisions),
            },
            "the round is told what it is picking up"
        );
        assert!(t.consumes_decisions, "and they are not picked up twice");
        assert_eq!(t.budget_n, Some(35), "25 + factor(5) × 2 new milestones");
    }

    #[test]
    fn an_implementation_round_with_nothing_decided_is_unchanged() {
        let c = cfg();
        let context = ctx(&c);
        let doc = block(DocStatus::Implementing, &[MilestoneState::Open]);
        let t = apply(
            &active(Node::Audit),
            &Trigger::SessionEnded {
                outcome: SessionOutcome::Ok {
                    status: Box::new(doc),
                },
            },
            &context,
        )
        .unwrap();
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: None,
            }
        );
        assert!(!t.consumes_decisions);
        assert_eq!(t.budget_n, None, "no decision, no budget change");
    }

    #[test]
    fn a_rebase_conflict_round_keeps_its_own_injection() {
        // That round has one job. The Backlog items wait for the next one,
        // which is at most one round away.
        let c = cfg();
        let context = with_included(&c, 1, 25);
        let t = apply(
            &active(Node::Rebase),
            &Trigger::CoreStepDone {
                node: Node::Rebase,
                result: CoreStepResult::Conflict {
                    files: vec!["a.rs".into()],
                    detail: "冲突".into(),
                },
            },
            &context,
        )
        .unwrap();
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: Some(Inject::RebaseConflict {
                    files: vec!["a.rs".into()]
                }),
            }
        );
        assert!(
            !t.consumes_decisions,
            "nothing was absorbed, so nothing is spent"
        );
    }

    #[test]
    fn extending_the_budget_absorbs_what_is_waiting_too() {
        let c = cfg();
        let context = with_included(&c, 1, 25);
        let failed = TaskState::Failed {
            at: Node::Implement,
            reason: FailureReason::ImplBudget { limit: 25 },
        };
        let t = apply(
            &failed,
            &Trigger::ExtendBudget { extra_rounds: 10 },
            &context,
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
        // The user's 10 extra rounds, plus factor(5) for the item being taken on.
        assert_eq!(t.budget_n, Some(40));
        assert!(t.consumes_decisions);
    }

    // ---- merge stopping point -------------------------------------------

    #[test]
    fn merging_with_an_item_decided_while_waiting_here_still_goes_back_to_implement() {
        // The only absorption this stopping point still does: an item decided
        // while the task sits here has no later round to be picked up by.
        let c = cfg();
        let context = Context {
            config: &c,
            budget_n: Some(25),
            decisions: PendingDecisions {
                included: 2,
                ruled: 0,
            },
        };
        let t = apply(&active(Node::AwaitMerge), &Trigger::Merge, &context).unwrap();
        assert_eq!(t.next, active(Node::Implement));
        assert!(t.consumes_decisions);
        // 25 + factor(5) × 2 new milestones.
        assert_eq!(t.budget_n, Some(35));
    }

    #[test]
    fn merging_with_only_ignored_items_proceeds_to_merge() {
        let c = cfg();
        let context = Context {
            config: &c,
            budget_n: Some(25),
            decisions: PendingDecisions {
                included: 0,
                ruled: 0,
            },
        };
        let t = apply(&active(Node::AwaitMerge), &Trigger::Merge, &context).unwrap();
        assert_eq!(t.next, active(Node::Merging));
    }

    #[test]
    fn a_blocked_merge_returns_to_the_stopping_point_rather_than_failing() {
        let c = cfg();
        let t = apply(
            &active(Node::Merging),
            &Trigger::CoreStepDone {
                node: Node::Merging,
                result: CoreStepResult::Blocked {
                    detail: "主工作树有未提交改动".into(),
                },
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::AwaitMerge));
    }

    #[test]
    fn a_failed_cleanup_after_a_successful_merge_still_completes_the_task() {
        let c = cfg();
        let t = apply(
            &active(Node::Cleanup),
            &Trigger::CoreStepDone {
                node: Node::Cleanup,
                result: CoreStepResult::Failed {
                    detail: "worktree busy".into(),
                },
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, TaskState::Done);
    }

    // ---- rebase ----------------------------------------------------------

    #[test]
    fn a_rebase_conflict_sends_the_implement_role_to_fix_it() {
        let c = cfg();
        let t = apply(
            &active(Node::Rebase),
            &Trigger::CoreStepDone {
                node: Node::Rebase,
                result: CoreStepResult::Conflict {
                    files: vec!["src/a.ts".into()],
                    detail: "conflict".into(),
                },
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Implement));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Impl,
                inject: Some(Inject::RebaseConflict {
                    files: vec!["src/a.ts".into()]
                })
            }
        );
    }

    #[test]
    fn a_core_step_result_for_a_different_node_is_rejected() {
        let c = cfg();
        assert!(
            apply(
                &active(Node::Rebase),
                &Trigger::CoreStepDone {
                    node: Node::Merging,
                    result: CoreStepResult::Ok,
                },
                &ctx(&c),
            )
            .is_err()
        );
    }

    // ---- session failures ------------------------------------------------

    #[test]
    fn a_crashed_session_fails_the_task_at_its_node() {
        let c = cfg();
        let t = apply(
            &active(Node::Implement),
            &Trigger::SessionEnded {
                outcome: SessionOutcome::Crashed {
                    detail: "exit 137".into(),
                },
            },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(
            t.next,
            TaskState::Failed {
                at: Node::Implement,
                reason: FailureReason::SessionCrashed {
                    detail: "exit 137".into()
                }
            }
        );
    }

    #[test]
    fn an_unparseable_document_is_a_protocol_failure() {
        let c = cfg();
        let t = apply(
            &active(Node::Design),
            &Trigger::SessionEnded {
                outcome: SessionOutcome::Unparseable {
                    error: ParseError::MissingField {
                        field: "status".into(),
                    },
                },
            },
            &ctx(&c),
        )
        .unwrap();
        assert!(matches!(
            t.next,
            TaskState::Failed {
                reason: FailureReason::Protocol { .. },
                ..
            }
        ));
    }

    #[test]
    fn a_missing_artifact_after_intake_is_a_protocol_failure() {
        let c = cfg();
        let t = apply(
            &active(Node::Intake),
            &Trigger::SessionEnded {
                outcome: SessionOutcome::MissingArtifact {
                    path: "docs/x/x-task.md".into(),
                },
            },
            &ctx(&c),
        )
        .unwrap();
        assert!(matches!(
            t.next,
            TaskState::Failed {
                at: Node::Intake,
                reason: FailureReason::Protocol { .. }
            }
        ));
    }

    #[test]
    fn a_document_declaring_infeasible_fails_the_task_from_any_node() {
        let c = cfg();
        for node in [Node::Design, Node::Adjudicate, Node::Implement, Node::Audit] {
            let t = apply(
                &active(node),
                &ended(block(DocStatus::Infeasible, &[])),
                &ctx(&c),
            )
            .unwrap();
            assert_eq!(
                t.next,
                TaskState::Failed {
                    at: node,
                    reason: FailureReason::Infeasible
                },
                "from {node:?}"
            );
        }
    }

    #[test]
    fn a_document_declaring_protocol_failure_fails_the_task() {
        let c = cfg();
        let t = apply(
            &active(Node::Implement),
            &ended(block(DocStatus::ProtocolFailure, &[])),
            &ctx(&c),
        )
        .unwrap();
        assert!(matches!(
            t.next,
            TaskState::Failed {
                reason: FailureReason::Protocol { .. },
                ..
            }
        ));
    }

    // ---- control ---------------------------------------------------------

    #[test]
    fn pause_records_the_node_to_resume_into() {
        let c = cfg();
        let t = apply(&active(Node::Implement), &Trigger::Pause, &ctx(&c)).unwrap();
        assert_eq!(
            t.next,
            TaskState::Paused {
                resume: Node::Implement
            }
        );
    }

    #[test]
    fn pausing_at_a_stopping_point_is_refused() {
        let c = cfg();
        for node in [Node::AwaitDesignApproval, Node::AwaitMerge] {
            assert!(apply(&active(node), &Trigger::Pause, &ctx(&c)).is_err());
        }
    }

    #[test]
    fn resume_re_enters_the_queue_rather_than_jumping_the_parallel_limit() {
        let c = cfg();
        let t = apply(
            &TaskState::Paused {
                resume: Node::Implement,
            },
            &Trigger::Resume,
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(
            t.next,
            TaskState::Requeued {
                resume: Node::Implement
            }
        );
    }

    #[test]
    fn requeued_paused_preserves_the_original_node() {
        let c = cfg();
        for node in Node::ALL.into_iter().filter(|n| !n.awaits_user()) {
            let queued = apply(
                &TaskState::Paused { resume: node },
                &Trigger::Resume,
                &ctx(&c),
            )
            .unwrap();
            let resumed = apply(&queued.next, &Trigger::SlotAvailable, &ctx(&c)).unwrap();
            assert_eq!(resumed.next, active(node));
            assert_eq!(queued.action, Action::None);
        }
    }

    #[test]
    fn requeued_stopped_preserves_the_original_node() {
        let c = cfg();
        for node in Node::ALL.into_iter().filter(|n| {
            !n.awaits_user() && !matches!(n, Node::Gate | Node::MergeGate | Node::MainCheck)
        }) {
            let t = apply(&TaskState::Stopped { at: node }, &Trigger::Resume, &ctx(&c)).unwrap();
            assert_eq!(t.next, TaskState::Requeued { resume: node });
            assert_eq!(t.action, Action::None);
        }
    }

    #[test]
    fn requeued_gate_discard_is_only_for_stopped_tasks() {
        let c = cfg();
        for node in [Node::Gate, Node::MergeGate, Node::MainCheck] {
            for stopped in [false, true] {
                let state = if stopped {
                    TaskState::Stopped { at: node }
                } else {
                    TaskState::Paused { resume: node }
                };
                let t = apply(&state, &Trigger::Resume, &ctx(&c)).unwrap();
                assert_eq!(t.next, TaskState::Requeued { resume: node });
                assert_eq!(
                    t.action,
                    if stopped {
                        Action::DiscardGateRuns { node }
                    } else {
                        Action::None
                    }
                );
            }
        }
    }

    #[test]
    fn requeued_dispatches_all_thirteen_nodes() {
        let c = cfg();
        let mut count = 0;
        for node in Node::ALL.into_iter().filter(|n| !n.awaits_user()) {
            let t = apply(
                &TaskState::Requeued { resume: node },
                &Trigger::SlotAvailable,
                &ctx(&c),
            )
            .unwrap();
            assert_eq!(t.next, active(node));
            assert_eq!(
                t.action,
                match node {
                    Node::Rebase
                    | Node::Merging
                    | Node::Cleanup
                    | Node::Gate
                    | Node::MergeGate
                    | Node::MainCheck => Action::RunCoreStep { node },
                    _ => Action::Reenter { node },
                }
            );
            assert_eq!(t.budget_n, None);
            assert!(!t.consumes_decisions);
            count += 1;
        }
        assert_eq!(count, 13);
        let fresh = apply(&TaskState::Queued, &Trigger::SlotAvailable, &ctx(&c)).unwrap();
        assert_eq!(fresh.next, active(Node::Intake));
        assert_eq!(fresh.action, Action::StartIntake);
    }

    #[test]
    fn requeued_disabled_role_fails_at_dequeue() {
        for node in Node::ALL {
            if let Some(role) = node.role() {
                let c = cfg_without(role);
                let t = apply(
                    &TaskState::Requeued { resume: node },
                    &Trigger::SlotAvailable,
                    &ctx(&c),
                )
                .unwrap();
                assert!(
                    matches!(t.next, TaskState::Failed { at, reason: FailureReason::Config { .. } } if at == node)
                );
                assert_eq!(t.action, Action::None);
            }
        }
    }

    #[test]
    fn requeued_disabled_role_rejects_resume() {
        for node in Node::ALL {
            if let Some(role) = node.role() {
                let c = cfg_without(role);
                for state in [
                    TaskState::Paused { resume: node },
                    TaskState::Stopped { at: node },
                ] {
                    assert!(apply(&state, &Trigger::Resume, &ctx(&c)).is_err());
                }
            }
        }
    }

    #[test]
    fn requeued_controls_only_allow_cancel() {
        let c = cfg();
        for node in Node::ALL.into_iter().filter(|n| !n.awaits_user()) {
            let state = TaskState::Requeued { resume: node };
            for trigger in [
                Trigger::Pause,
                Trigger::Stop,
                Trigger::Resume,
                Trigger::RerunFrom { node },
                Trigger::ExtendBudget { extra_rounds: 1 },
            ] {
                assert!(apply(&state, &trigger, &ctx(&c)).is_err());
            }
            let t = apply(&state, &Trigger::Cancel, &ctx(&c)).unwrap();
            assert_eq!(t.next, TaskState::Cancelled);
            assert_eq!(t.action, Action::RunCancel);
        }
    }

    #[test]
    fn requeued_has_node_without_holding_a_slot() {
        for node in Node::ALL.into_iter().filter(|n| !n.awaits_user()) {
            let state = TaskState::Requeued { resume: node };
            assert_eq!(state.node(), Some(node));
            assert!(!state.occupies_slot());
            assert!(!state.awaits_user());
            assert!(!state.is_terminal());
        }
    }

    #[test]
    fn requeued_json_preserves_old_shapes() {
        use serde_json::{from_value, json, to_value};
        let pairs = [
            (TaskState::Queued, json!({"state":"queued"})),
            (
                TaskState::Requeued {
                    resume: Node::Audit,
                },
                json!({"state":"queued","resume":"audit"}),
            ),
            (
                active(Node::Implement),
                json!({"state":"active","node":"implement"}),
            ),
            (
                TaskState::Paused { resume: Node::Gate },
                json!({"state":"paused","resume":"gate"}),
            ),
            (
                TaskState::Stopped { at: Node::Review },
                json!({"state":"stopped","at":"review"}),
            ),
            (
                TaskState::Failed {
                    at: Node::Design,
                    reason: FailureReason::Config { detail: "x".into() },
                },
                json!({"state":"failed","at":"design","reason":{"kind":"config","detail":"x"}}),
            ),
            (TaskState::Done, json!({"state":"done"})),
            (TaskState::Cancelled, json!({"state":"cancelled"})),
        ];
        for (state, value) in pairs {
            assert_eq!(to_value(&state).unwrap(), value);
            assert_eq!(from_value::<TaskState>(value).unwrap(), state);
        }
        assert_eq!(
            from_value::<TaskState>(json!({"state":"queued","resume":null})).unwrap(),
            TaskState::Queued
        );
        for value in [
            json!({"state":"queued","resume":""}),
            json!({"state":"queued","resume":"unknown"}),
            json!({"state":"queued","resume":0}),
        ] {
            assert!(from_value::<TaskState>(value).is_err());
        }
    }

    #[test]
    fn stop_from_paused_is_allowed() {
        let c = cfg();
        let t = apply(
            &TaskState::Paused {
                resume: Node::Audit,
            },
            &Trigger::Stop,
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, TaskState::Stopped { at: Node::Audit });
    }

    #[test]
    fn cancel_works_from_every_non_terminal_state() {
        let c = cfg();
        let states = [
            TaskState::Queued,
            active(Node::Design),
            active(Node::AwaitMerge),
            TaskState::Paused {
                resume: Node::Implement,
            },
            TaskState::Stopped { at: Node::Audit },
            TaskState::Failed {
                at: Node::Audit,
                reason: FailureReason::Infeasible,
            },
        ];
        for s in states {
            let t = apply(&s, &Trigger::Cancel, &ctx(&c)).unwrap();
            assert_eq!(t.next, TaskState::Cancelled);
            assert_eq!(t.action, Action::RunCancel);
        }
    }

    #[test]
    fn cancel_is_refused_once_the_task_is_terminal() {
        let c = cfg();
        for s in [TaskState::Done, TaskState::Cancelled] {
            assert!(apply(&s, &Trigger::Cancel, &ctx(&c)).is_err());
        }
    }

    #[test]
    fn rerun_from_a_disabled_role_is_refused() {
        let c = cfg_without(Role::Audit);
        let failed = TaskState::Failed {
            at: Node::Audit,
            reason: FailureReason::ImplBudget { limit: 25 },
        };
        assert!(apply(&failed, &Trigger::RerunFrom { node: Node::Audit }, &ctx(&c)).is_err());
    }

    #[test]
    fn rerun_from_an_enabled_role_starts_that_role() {
        let c = cfg();
        let failed = TaskState::Failed {
            at: Node::Audit,
            reason: FailureReason::ImplBudget { limit: 25 },
        };
        let t = apply(
            &failed,
            &Trigger::RerunFrom { node: Node::Design },
            &ctx(&c),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Design));
        assert_eq!(
            t.action,
            Action::StartRole {
                role: Role::Plan,
                inject: None
            }
        );
    }

    #[test]
    fn rerun_from_a_waiting_or_core_node_is_refused() {
        let c = cfg();
        let failed = TaskState::Failed {
            at: Node::Audit,
            reason: FailureReason::Infeasible,
        };
        for node in [
            Node::AwaitDesignApproval,
            Node::AwaitMerge,
            Node::Merging,
            Node::Cleanup,
        ] {
            assert!(
                apply(&failed, &Trigger::RerunFrom { node }, &ctx(&c)).is_err(),
                "{node:?} should not be a rerun target"
            );
        }
    }

    // ---- structural invariants ------------------------------------------

    #[test]
    fn the_two_stopping_points_never_hold_a_parallel_slot() {
        for node in Node::ALL {
            assert_eq!(
                node.occupies_slot(),
                !matches!(node, Node::AwaitDesignApproval | Node::AwaitMerge),
                "{node:?}"
            );
        }
    }

    #[test]
    fn exactly_six_nodes_carry_a_configurable_role() {
        let with_role: Vec<Node> = Node::ALL
            .into_iter()
            .filter(|n| n.role().is_some())
            .collect();
        assert_eq!(
            with_role,
            vec![
                Node::Design,
                Node::Review,
                Node::Adjudicate,
                Node::Implement,
                Node::Audit,
                Node::Retro
            ]
        );
    }

    #[test]
    fn every_node_name_round_trips() {
        for node in Node::ALL {
            assert_eq!(Node::parse(node.as_str()), Some(node));
        }
    }

    #[test]
    fn a_session_end_at_a_core_or_waiting_node_is_rejected() {
        let c = cfg();
        for node in [Node::Rebase, Node::Merging, Node::Cleanup, Node::AwaitMerge] {
            assert!(
                apply(
                    &active(node),
                    &ended(block(DocStatus::Implementing, &[])),
                    &ctx(&c)
                )
                .is_err(),
                "{node:?}"
            );
        }
    }

    #[test]
    fn approve_and_merge_only_apply_at_their_own_stopping_points() {
        let c = cfg();
        assert!(apply(&active(Node::Implement), &Trigger::Approve, &ctx(&c)).is_err());
        assert!(
            apply(
                &active(Node::AwaitDesignApproval),
                &Trigger::Merge,
                &ctx(&c)
            )
            .is_err()
        );
        assert!(apply(&active(Node::AwaitMerge), &Trigger::Approve, &ctx(&c)).is_err());
    }

    #[test]
    fn a_queued_task_holds_no_slot_and_a_running_one_does() {
        assert!(!TaskState::Queued.occupies_slot());
        assert!(active(Node::Implement).occupies_slot());
        assert!(!active(Node::AwaitMerge).occupies_slot());
        assert!(
            !TaskState::Paused {
                resume: Node::Implement
            }
            .occupies_slot()
        );
    }

    #[test]
    fn waiting_and_failed_tasks_are_the_ones_that_await_the_user() {
        assert!(active(Node::AwaitDesignApproval).awaits_user());
        assert!(active(Node::AwaitMerge).awaits_user());
        assert!(
            TaskState::Failed {
                at: Node::Audit,
                reason: FailureReason::Infeasible
            }
            .awaits_user()
        );
        assert!(!active(Node::Implement).awaits_user());
        assert!(!TaskState::Done.awaits_user());
    }

    #[test]
    fn task_state_round_trips_through_json() {
        let states = [
            TaskState::Queued,
            active(Node::Implement),
            TaskState::Paused {
                resume: Node::Audit,
            },
            TaskState::Failed {
                at: Node::Audit,
                reason: FailureReason::ImplBudget { limit: 25 },
            },
            TaskState::Done,
            TaskState::Cancelled,
        ];
        for s in states {
            let json = serde_json::to_string(&s).unwrap();
            assert_eq!(serde_json::from_str::<TaskState>(&json).unwrap(), s);
        }
    }
    mod task_card {
        use super::*;

        #[test]
        fn kind_wire_names_are_feature_and_bug_and_the_default_is_feature() {
            assert_eq!(TaskKind::default(), TaskKind::Feature);
            for kind in [TaskKind::Feature, TaskKind::Bug] {
                assert_eq!(TaskKind::parse(kind.as_str()), Some(kind));
                let wire = serde_json::to_value(kind).unwrap();
                assert_eq!(wire, kind.as_str());
                assert_eq!(serde_json::from_value::<TaskKind>(wire).unwrap(), kind);
            }
            for value in ["", "Feature", "BUG", "unknown", " feature", "bug "] {
                assert_eq!(TaskKind::parse(value), None);
                assert!(serde_json::from_value::<TaskKind>(serde_json::json!(value)).is_err());
            }
        }

        #[test]
        fn draft_has_its_own_wire_shape_and_is_not_queued() {
            let wire = serde_json::json!({"state":"draft"});
            assert_eq!(serde_json::to_value(TaskState::Draft).unwrap(), wire);
            assert_eq!(
                serde_json::from_value::<TaskState>(wire).unwrap(),
                TaskState::Draft
            );
            assert_ne!(
                serde_json::to_value(TaskState::Queued).unwrap(),
                serde_json::json!({"state":"draft"})
            );
            assert_eq!(
                serde_json::to_value(Trigger::Start).unwrap(),
                serde_json::json!({"trigger":"start"})
            );
        }

        #[test]
        fn draft_holds_no_slot_does_not_await_the_user_and_is_not_terminal() {
            assert!(!TaskState::Draft.occupies_slot());
            assert!(!TaskState::Draft.awaits_user());
            assert!(!TaskState::Draft.is_terminal());
            assert_eq!(TaskState::Draft.node(), None);
        }

        #[test]
        fn start_moves_a_draft_to_queued_without_an_action() {
            let result = apply(&TaskState::Draft, &Trigger::Start, &ctx(&cfg())).unwrap();
            assert_eq!(result.next, TaskState::Queued);
            assert_eq!(result.action, Action::None);
        }

        #[test]
        fn start_is_refused_for_every_state_that_is_not_a_draft() {
            for state in [
                TaskState::Queued,
                TaskState::Requeued {
                    resume: Node::Design,
                },
                active(Node::Intake),
                TaskState::Paused {
                    resume: Node::Design,
                },
                TaskState::Stopped { at: Node::Design },
                TaskState::Failed {
                    at: Node::Design,
                    reason: FailureReason::Config { detail: "x".into() },
                },
                TaskState::Done,
                TaskState::Cancelled,
            ] {
                assert_eq!(
                    apply(&state, &Trigger::Start, &ctx(&cfg()))
                        .unwrap_err()
                        .reason,
                    "只有待开始的任务可以开始"
                );
            }
        }

        #[test]
        fn a_draft_can_be_cancelled_directly() {
            let result = apply(&TaskState::Draft, &Trigger::Cancel, &ctx(&cfg())).unwrap();
            assert_eq!(result.next, TaskState::Cancelled);
            assert_eq!(result.action, Action::RunCancel);
        }

        #[test]
        fn a_draft_refuses_every_other_trigger() {
            for trigger in [
                Trigger::SlotAvailable,
                Trigger::Pause,
                Trigger::Stop,
                Trigger::Resume,
                Trigger::Approve,
                Trigger::Reject {
                    feedback: "x".into(),
                },
                Trigger::Merge,
                Trigger::ExtendBudget { extra_rounds: 1 },
                Trigger::RerunFrom { node: Node::Intake },
                Trigger::SessionEnded {
                    outcome: SessionOutcome::Crashed { detail: "x".into() },
                },
                Trigger::CoreStepDone {
                    node: Node::Cleanup,
                    result: CoreStepResult::Ok,
                },
                Trigger::GateDone {
                    node: Node::Gate,
                    outcome: GateOutcome::Passed,
                    status: None,
                },
            ] {
                assert!(
                    apply(&TaskState::Draft, &trigger, &ctx(&cfg())).is_err(),
                    "{trigger:?}"
                );
            }
            assert_eq!(
                apply(&TaskState::Draft, &Trigger::Pause, &ctx(&cfg()))
                    .unwrap_err()
                    .reason,
                "任务尚未开始"
            );
        }
    }
}
