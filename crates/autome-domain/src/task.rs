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
    AwaitMerge,
    Merging,
    Cleanup,
}

impl Node {
    pub const ALL: [Node; 12] = [
        Node::Intake,
        Node::Design,
        Node::Review,
        Node::Adjudicate,
        Node::AwaitDesignApproval,
        Node::Implement,
        Node::Audit,
        Node::Retro,
        Node::Rebase,
        Node::AwaitMerge,
        Node::Merging,
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
            Node::Implement => Some(Role::Impl),
            Node::Audit => Some(Role::Audit),
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
        !matches!(self, Node::AwaitDesignApproval | Node::AwaitMerge)
    }

    /// Whether this node is waiting on the user rather than on a machine.
    pub const fn awaits_user(self) -> bool {
        matches!(self, Node::AwaitDesignApproval | Node::AwaitMerge)
    }

    /// Whether the core drives this node itself, with no session at all.
    pub const fn is_core_step(self) -> bool {
        matches!(self, Node::Rebase | Node::Merging | Node::Cleanup)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Node::Intake => "intake",
            Node::Design => "design",
            Node::Review => "review",
            Node::Adjudicate => "adjudicate",
            Node::AwaitDesignApproval => "await_design_approval",
            Node::Implement => "implement",
            Node::Audit => "audit",
            Node::Retro => "retro",
            Node::Rebase => "rebase",
            Node::AwaitMerge => "await_merge",
            Node::Merging => "merging",
            Node::Cleanup => "cleanup",
        }
    }

    pub fn parse(s: &str) -> Option<Node> {
        Node::ALL.into_iter().find(|n| n.as_str() == s)
    }
}

/// Why a task stopped without completing (requirement T-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FailureReason {
    /// The design loop used up `design_rounds`.
    DesignBudget { limit: u32 },
    /// The implementation loop used up N.
    ImplBudget { limit: u32 },
    /// The design document said the task cannot be done.
    Infeasible,
    /// The design document could not be read, or said so itself.
    Protocol { detail: String },
    /// The session process died, or left no usable output.
    SessionCrashed { detail: String },
    /// A rebase conflict survived the implement/audit repair path.
    RebaseConflict { detail: String },
    /// A configuration problem blocked the launch (SAME-MODEL, skills, …).
    Config { detail: String },
    /// A core step (merge, cleanup) failed.
    CoreStep { node: Node, detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TaskState {
    /// Created, but the project's parallel slots are all taken.
    Queued,
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

impl TaskState {
    pub fn node(&self) -> Option<Node> {
        match self {
            TaskState::Active { node } => Some(*node),
            TaskState::Paused { resume } => Some(*resume),
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
    /// Merge preconditions unmet (dirty main worktree, stale branch, …).
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
    /// Delete the worktree and branch, archive the docs.
    RunCancel,
}

/// Extra prompt material for a role session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "inject", rename_all = "snake_case")]
pub enum Inject {
    /// The user rejected the design; re-run with their words.
    DesignFeedback { feedback: String },
    /// The user's Backlog/dispute decisions are being consumed.
    Decisions,
    /// Rebase left conflicts for the implement role to resolve.
    RebaseConflict { files: Vec<String> },
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
}

impl Transition {
    fn to(next: TaskState, action: Action) -> Self {
        Self {
            next,
            action,
            budget_n: None,
            consumes_decisions: false,
        }
    }

    fn active(node: Node, action: Action) -> Self {
        Self::to(TaskState::Active { node }, action)
    }

    fn fail(at: Node, reason: FailureReason) -> Self {
        Self::to(TaskState::Failed { at, reason }, Action::None)
    }

    fn with_budget(mut self, n: u32) -> Self {
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
fn after_implement_without_audit(
    status: &StatusBlock,
    budget_n: u32,
    config: &ResolvedConfig,
) -> Transition {
    if status.no_open_milestones() {
        after_the_loop(config)
    } else if status.impl_round >= budget_n {
        Transition::fail(
            Node::Implement,
            FailureReason::ImplBudget { limit: budget_n },
        )
    } else {
        Transition::active(
            Node::Implement,
            Action::StartRole {
                role: Role::Impl,
                inject: None,
            },
        )
    }
}

/// Applies a trigger. Returns the next state plus the side effect to perform,
/// or `Rejected` when the trigger does not apply.
pub fn apply(
    state: &TaskState,
    trigger: &Trigger,
    ctx: &Context<'_>,
) -> Result<Transition, Rejected> {
    // Control triggers first: they apply from many states, and handling them
    // once here keeps the node table below about the Loop itself.
    match trigger {
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
                TaskState::Queued => Err(reject("任务尚未开始")),
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
                // Resuming re-enters the queue: the slot may have been taken
                // while the task was paused (design §8).
                TaskState::Paused { .. } | TaskState::Stopped { .. } => {
                    Ok(Transition::to(TaskState::Queued, Action::None))
                }
                _ => Err(reject("任务不处于暂停或已停止")),
            };
        }
        _ => {}
    }

    match (state, trigger) {
        // ---- Queued ------------------------------------------------------
        (TaskState::Queued, Trigger::SlotAvailable) => {
            Ok(Transition::active(Node::Intake, Action::StartIntake))
        }

        // ---- Failure panel ----------------------------------------------
        (TaskState::Failed { reason, .. }, Trigger::ExtendBudget { extra_rounds }) => {
            let extra = *extra_rounds;
            if extra == 0 {
                return Err(reject("追加轮次必须大于 0"));
            }
            match reason {
                FailureReason::ImplBudget { limit } => Ok(Transition::active(
                    Node::Implement,
                    Action::StartRole {
                        role: Role::Impl,
                        inject: None,
                    },
                )
                .with_budget(limit + extra)),
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
                        Ok(after_implement_without_audit(status, budget, ctx.config))
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
                        Ok(Transition::active(
                            Node::Implement,
                            Action::StartRole {
                                role: Role::Impl,
                                inject: None,
                            },
                        ))
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
        ) => {
            if ctx.decisions.included > 0 {
                // Included Backlog items become new milestones; the task goes
                // back to implement and returns here after audit (T-09). The
                // budget grows by the configured factor per new milestone so
                // the extra work is not paid for out of the original N.
                let extra = ctx.config.loop_defaults.budget_factor * ctx.decisions.included;
                let n = ctx.budget_n.unwrap_or(0) + extra;
                return Ok(Transition::active(
                    Node::Implement,
                    Action::StartRole {
                        role: Role::Impl,
                        inject: Some(Inject::Decisions),
                    },
                )
                .with_budget(n)
                .consuming());
            }
            Ok(Transition::active(
                Node::Merging,
                Action::RunCoreStep {
                    node: Node::Merging,
                },
            ))
        }

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
                    Ok(Transition::active(Node::AwaitMerge, Action::None))
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
                (Node::Merging, CoreStepResult::Ok) => Ok(Transition::active(
                    Node::Cleanup,
                    Action::RunCoreStep {
                        node: Node::Cleanup,
                    },
                )),
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

        // ---- Everything else ----------------------------------------------
        _ => Err(reject("当前状态不接受该操作")),
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
    use super::*;
    use crate::config::{GlobalConfig, ProjectConfig, RoleOverrides, resolve};
    use crate::status_block::{ConvergenceMode, Milestone, MilestoneState};

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

    // ---- merge stopping point -------------------------------------------

    #[test]
    fn merging_with_an_included_backlog_item_goes_back_to_implement() {
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
        assert_eq!(t.next, TaskState::Queued);
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
}
