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
    /// (design §8). User waits and the short Git core steps do not hold
    /// session capacity. Gates retain their existing slot accounting.
    pub const fn occupies_slot(self) -> bool {
        !matches!(
            self,
            Node::AwaitDesignApproval
                | Node::AwaitMerge
                | Node::AwaitInfo
                | Node::NotABug
                | Node::Rebase
                | Node::Merging
                | Node::Cleanup
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
    /// True only when this transition grants authorization after a behind merge.
    pub grants_merge_request: bool,
}

impl Transition {
    pub(crate) fn to(next: TaskState, action: Action) -> Self {
        Self {
            next,
            action,
            budget_n: None,
            consumes_decisions: false,
            grants_merge_request: false,
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
    let requested = merge_requested;
    let mut transition = apply_inner(state, trigger, ctx, requested)?;
    transition.grants_merge_request = matches!(
        trigger,
        Trigger::CoreStepDone {
            node: Node::Merging,
            result: CoreStepResult::Behind { .. }
        }
    );
    Ok(transition)
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
                    if ctx.config.gates_after_implement() {
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
                (Node::Merging, CoreStepResult::Behind { .. }) => Ok(Transition::active(
                    Node::Rebase,
                    Action::RunCoreStep { node: Node::Rebase },
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
        assert!(!t.grants_merge_request);
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
    fn merge_auth_only_a_behind_merge_grants_a_request() {
        for node in Node::ALL.into_iter().chain(Node::BUG) {
            for trigger in [
                Trigger::Pause,
                Trigger::Stop,
                Trigger::Cancel,
                Trigger::CoreStepDone {
                    node,
                    result: CoreStepResult::Ok,
                },
                Trigger::CoreStepDone {
                    node,
                    result: CoreStepResult::Behind {
                        repos: vec![],
                        detail: String::new(),
                    },
                },
            ] {
                if let Ok(t) = apply_with_merge_request(&active(node), &trigger, &ctx(&cfg()), true)
                {
                    assert_eq!(
                        t.grants_merge_request,
                        matches!(
                            trigger,
                            Trigger::CoreStepDone {
                                node: Node::Merging,
                                result: CoreStepResult::Behind { .. }
                            }
                        )
                    );
                }
            }
        }

