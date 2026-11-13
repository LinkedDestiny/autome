//! Bug Loop transitions. Feature tasks delegate to the original table.
use crate::{
    bug_doc::{BugDoc, BugDocError, BugReview, BugStatus},
    role::Role,
    session::BugStep,
    task::*,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BugProgress {
    pub fix_round: u32,
    pub repro_misses: u32,
    pub test_fixes: u32,
    pub test_fix_open: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum BugSessionOutcome {
    Ok { doc: Box<BugDoc> },
    Crashed { detail: String },
    Unparseable { error: BugDocError },
    MissingArtifact { path: String },
    GuardFailed { detail: String },
}
pub const TEST_FIX_LIMIT: u32 = 2;

pub fn test_fix_limit_message(limit: u32) -> String {
    format!(
        "回归测试已退回定位轮修正 {limit} 次（上限 {limit}），仍被判有缺陷。请人工检查回归测试后 autome rerun <任务> locate，或取消后重提"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproPair {
    pub base_exit: i32,
    pub base_timed_out: bool,
    pub head_exit: i32,
    pub head_timed_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestFixJudgement {
    Accept,
    NeedsFix,
    NotReproduced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TestFixEvent {
    Requested {
        by: BugStep,
        attempt: u32,
        limit: u32,
    },
    Settled {
        result: TestFixJudgement,
        pair: ReproPair,
    },
}

fn reproduced(exit: i32, timed_out: bool) -> bool {
    !timed_out && exit != 0 && ![126, 127].contains(&exit)
}

pub fn judge_test_fix(pair: &ReproPair) -> TestFixJudgement {
    if !reproduced(pair.base_exit, pair.base_timed_out) {
        TestFixJudgement::NotReproduced
    } else if pair.head_exit == 0 && !pair.head_timed_out {
        TestFixJudgement::Accept
    } else {
        TestFixJudgement::NeedsFix
    }
}

pub fn uses_bug_loop(kind: TaskKind, state: &TaskState, started: bool) -> bool {
    kind == TaskKind::Bug && (started || matches!(state, TaskState::Draft | TaskState::Queued))
}
fn rejected(reason: &str) -> Rejected {
    Rejected {
        reason: reason.into(),
    }
}
fn session(step: BugStep, inject: Option<Inject>, ctx: &Context<'_>) -> Transition {
    if !ctx.config.is_enabled(step.role()) {
        Transition::fail(
            step.node(),
            FailureReason::Config {
                detail: "该角色当前已关闭".into(),
            },
        )
    } else {
        Transition::active(step.node(), Action::StartBugSession { step, inject })
    }
}
fn core(node: Node) -> Transition {
    Transition::active(node, Action::RunCoreStep { node })
}
fn start_fix(
    at: Node,
    inject: Option<Inject>,
    ctx: &Context<'_>,
    progress: BugProgress,
) -> Transition {
    let limit = ctx.budget_n.unwrap_or(ctx.config.bug.fix_rounds);
    if progress.fix_round >= limit {
        Transition::fail(at, FailureReason::FixBudget { limit })
    } else {
        session(BugStep::Fix, inject, ctx).with_budget(limit)
    }
}

pub fn apply_for(
    kind: TaskKind,
    state: &TaskState,
    trigger: &Trigger,
    ctx: &Context<'_>,
    progress: BugProgress,
    merge_requested: bool,
) -> Result<Transition, Rejected> {
    if kind == TaskKind::Feature {
        return apply_with_merge_request(state, trigger, ctx, merge_requested);
    }
    if matches!(trigger, Trigger::Start | Trigger::SlotAvailable)
        && !ctx.config.is_enabled(Role::Audit)
    {
        return Err(rejected("Bug 任务需要独立复核，请启用审计角色后启动"));
    }
    match (state, trigger) {
        (
            _,
            Trigger::GateSameFailure {
                node,
                count,
                summary,
                log,
            },
        ) => {
            if !matches!(state, TaskState::Active { node: current } if current == node && matches!(node, Node::FixGate | Node::MergeGate))
            {
                return Err(rejected("门禁结果与当前节点不符"));
            }
            Ok(Transition::fail(
                *node,
                FailureReason::GateSameFailure {
                    count: *count,
                    summary: summary.clone(),
                    log: log.clone(),
                },
            ))
        }
        (
            TaskState::Failed {
                at,
                reason: FailureReason::GateSameFailure { .. },
            },
            Trigger::RerunFrom { node },
        ) if node == at && matches!(node, Node::FixGate | Node::MergeGate) => Ok(
            Transition::active(*node, Action::DiscardGateRuns { node: *node }),
        ),
        (
            TaskState::Failed {
                at,
                reason: FailureReason::GateSameFailure { .. },
            },
            Trigger::ExtendBudget { extra_rounds },
        ) => {
            let limit = ctx
                .budget_n
                .unwrap_or(ctx.config.bug.fix_rounds)
                .checked_add(*extra_rounds)
                .filter(|_| *extra_rounds > 0)
                .ok_or_else(|| rejected("追加轮次必须大于 0 且不溢出"))?;
            Ok(Transition::active(*at, Action::DiscardGateRuns { node: *at }).with_budget(limit))
        }

        (TaskState::Queued, Trigger::SlotAvailable) => Ok(session(BugStep::Locate, None, ctx)),
        (
            TaskState::Stopped {
                at: node @ (Node::ReproCheck | Node::FixGate),
            },
            Trigger::Resume,
        ) => Ok(Transition::to(
            TaskState::Requeued { resume: *node },
            Action::DiscardGateRuns { node: *node },
        )),
        (
            TaskState::Failed {
                reason: FailureReason::FixBudget { limit },
                ..
            },
            Trigger::ExtendBudget { extra_rounds },
        ) => {
            let limit = limit
                .checked_add(*extra_rounds)
                .filter(|_| *extra_rounds > 0)
                .ok_or_else(|| rejected("追加轮次必须大于 0 且不溢出"))?;
            Ok(session(BugStep::Fix, None, ctx).with_budget(limit))
        }
        (TaskState::Failed { at, .. } | TaskState::Stopped { at }, Trigger::RerunFrom { node }) => {
            let step = match node {
                Node::Locate => BugStep::Locate,
                Node::Fix => BugStep::Fix,
                Node::FixReview => BugStep::FixReview,
                Node::Rebase => return Ok(core(Node::Rebase)),
                _ => return Err(rejected("Bug 任务只能从定位、修复、复核或 rebase 重跑")),
            };
            if !ctx.config.is_enabled(step.role()) {
                return Err(rejected("该角色当前已关闭，无法从这里重跑"));
            }
            if step == BugStep::Fix {
                return Ok(start_fix(*at, None, ctx, progress));
            }
            Ok(session(step, None, ctx))
        }
        (TaskState::Failed { .. } | TaskState::Stopped { .. }, Trigger::Reply { info }) => {
            if info.trim().is_empty() {
                return Err(rejected("补充信息不能为空"));
            }
            let limit = progress
                .fix_round
                .checked_add(ctx.config.bug.fix_rounds)
                .ok_or_else(|| rejected("修复轮次预算溢出，请用 autome task new 新建 Bug 任务"))?
                .max(ctx.budget_n.unwrap_or(ctx.config.bug.fix_rounds));
            Ok(session(BugStep::Locate, None, ctx).with_budget(limit))
        }
        (
            TaskState::Active {
                node: Node::AwaitInfo | Node::NotABug,
            },
            Trigger::Reply { info },
        ) => {
            if info.trim().is_empty() {
                return Err(rejected("补充信息不能为空"));
            }
            Ok(session(BugStep::Locate, None, ctx))
        }
        (_, Trigger::Reply { .. }) => Err(rejected(match state {
            TaskState::Draft => "Bug 任务尚未开始，请先用 autome task start <任务>",
            TaskState::Queued | TaskState::Requeued { .. } => {
                "Bug 任务正在排队，请用 autome status <任务> 查看进度，等待定位结果后再补充信息"
            }
            TaskState::Paused { .. } => {
                "Bug 任务已暂停，请用 autome resume <任务> 恢复，或用 autome stop <任务> 停止后再 reply"
            }
            TaskState::Done => "Bug 任务已完成，请用 autome task new --kind bug 新建任务",
            TaskState::Cancelled => "Bug 任务已取消，请用 autome task new --kind bug 新建任务",
            _ => {
                "Bug 任务正在处理，请用 autome status <任务> 查看进度，或用 autome stop <任务> 停止后再 reply"
            }
        })),
        (
            TaskState::Active {
                node: node @ (Node::Locate | Node::Fix | Node::FixReview),
            },
            Trigger::BugSessionEnded { outcome },
        ) => {
            let doc = match outcome {
                BugSessionOutcome::Ok { doc } => doc,
                BugSessionOutcome::Crashed { detail } => {
                    return Ok(Transition::fail(
                        *node,
                        FailureReason::SessionCrashed {
                            detail: detail.clone(),
                        },
                    ));
                }
                BugSessionOutcome::Unparseable { error } => {
                    return Ok(Transition::fail(
                        *node,
                        FailureReason::Protocol {
                            detail: error.to_string(),
                        },
                    ));
                }
                BugSessionOutcome::MissingArtifact { path } => {
                    return Ok(Transition::fail(
                        *node,
                        FailureReason::Protocol {
                            detail: format!("缺少文件 {path}"),
                        },
                    ));
                }
                BugSessionOutcome::GuardFailed { detail } => {
                    return Ok(Transition::fail(
                        *node,
                        FailureReason::Protocol {
                            detail: detail.clone(),
                        },
                    ));
                }
            };
            if doc.regression_defect && !ctx.bug_test_defect {
                return Ok(Transition::fail(
                    *node,
                    FailureReason::Protocol {
                        detail: "Bug 文档字段 回归测试 缺失、重复或值无效".into(),
                    },
                ));
            }
            let invalid = if doc.regression_defect
                && (*node == Node::Locate
                    || (*node == Node::FixReview && doc.review != BugReview::Rejected))
            {
                Some("回归测试有缺陷只能由修复轮或打回复核轮声明")
            } else if *node != Node::Locate && doc.status != BugStatus::Located {
                Some("status 必须是已定位")
            } else if (*node == Node::FixReview) == (doc.review == BugReview::Pending) {
                Some("复核 字段不符合当前轮次")
            } else {
                None
            };
            if let Some(detail) = invalid {
                return Ok(Transition::fail(
                    *node,
                    FailureReason::Protocol {
                        detail: detail.into(),
                    },
                ));
            }
            if doc.regression_defect {
                if progress.test_fixes >= TEST_FIX_LIMIT {
                    return Ok(Transition::fail(
                        *node,
                        FailureReason::TestFixLimit {
                            limit: TEST_FIX_LIMIT,
                        },
                    ));
                }
                let mut transition = session(BugStep::Locate, None, ctx);
                if matches!(transition.next, TaskState::Active { .. }) {
                    transition.test_fix_event = Some(TestFixEvent::Requested {
                        by: if *node == Node::Fix {
                            BugStep::Fix
                        } else {
                            BugStep::FixReview
                        },
                        attempt: progress.test_fixes + 1,
                        limit: TEST_FIX_LIMIT,
                    });
                }
                return Ok(transition);
            }
            Ok(match node {
                Node::Locate => match doc.status {
                    BugStatus::Located => core(Node::ReproCheck),
                    BugStatus::CannotReproduce => Transition::active(Node::AwaitInfo, Action::None),
                    BugStatus::NotABug => Transition::active(Node::NotABug, Action::None),
                },
                Node::Fix => core(Node::FixGate),
                _ if doc.review == BugReview::Passed && !ctx.handoff_pending => core(Node::Rebase),
                _ => start_fix(*node, Some(Inject::FixRejected), ctx, progress),
            })
        }
        (
            TaskState::Active {
                node: Node::ReproCheck,
            },
            Trigger::ReproDone {
                paired,
                head,
                command,
                exit_code,
                timed_out,
                tail,
            },
        ) => {
            let (exit_code, timed_out) = if ctx.bug_test_defect && progress.test_fix_open {
                let Some(pair) = paired else {
                    return Ok(Transition::fail(
                        Node::ReproCheck,
                        FailureReason::Protocol {
                            detail: "修正回归测试缺少成对复现结果".into(),
                        },
                    ));
                };
                let result = judge_test_fix(pair);
                let transition = match result {
                    TestFixJudgement::Accept => Some(core(Node::FixGate)),
                    TestFixJudgement::NeedsFix => Some(start_fix(
                        Node::ReproCheck,
                        Some(Inject::GateFailed {
                            head: head.clone(),
                            command: command.clone(),
                            tail: tail.clone(),
                        }),
                        ctx,
                        progress,
                    )),
                    TestFixJudgement::NotReproduced => None,
                };
                if let Some(mut transition) = transition {
                    transition.test_fix_event = Some(TestFixEvent::Settled {
                        result,
                        pair: *pair,
                    });
                    return Ok(transition);
                }
                (&pair.base_exit, &pair.base_timed_out)
            } else {
                if reproduced(*exit_code, *timed_out) {
                    return Ok(start_fix(Node::ReproCheck, None, ctx, progress));
                }
                (exit_code, timed_out)
            };
            if progress.repro_misses >= 1 {
                return Ok(Transition::active(Node::AwaitInfo, Action::None));
            }
            let why = if *timed_out {
                "timed_out"
            } else if *exit_code == 0 {
                "exited_zero"
            } else {
                "not_runnable"
            };
            Ok(session(
                BugStep::Locate,
                Some(Inject::ReproNotFailing {
                    head: head.clone(),
                    command: command.clone(),
                    exit_code: *exit_code,
                    why: why.into(),
                    tail: tail.clone(),
                }),
                ctx,
            ))
        }
        (
            TaskState::Active {
                node: node @ (Node::FixGate | Node::MergeGate),
            },
            Trigger::GateDone {
                node: done,
                outcome,
                ..
            },
        ) if node == done => match outcome {
            GateOutcome::Failed {
                head,
                command,
                tail,
            } => Ok(start_fix(
                *node,
                Some(Inject::GateFailed {
                    head: head.clone(),
                    command: command.clone(),
                    tail: tail.clone(),
                }),
                ctx,
                progress,
            )),
            GateOutcome::Passed if *node == Node::FixGate => {
                Ok(session(BugStep::FixReview, None, ctx))
            }
            _ => apply_with_merge_request(state, trigger, ctx, merge_requested),
        },
        (
            TaskState::Active {
                node: Node::MergeGate,
            },
            Trigger::CoreStepDone {
                node: Node::MergeGate,
                result: CoreStepResult::Conflict { files, .. },
            },
        ) => Ok(start_fix(
            Node::MergeGate,
            Some(Inject::RebaseConflict {
                files: files.clone(),
            }),
            ctx,
            progress,
        )),
        (
            TaskState::Active { node: Node::Rebase },
            Trigger::CoreStepDone {
                node: Node::Rebase,
                result,
            },
        ) => match result {
            CoreStepResult::Ok => Ok(core(Node::MergeGate)),
            CoreStepResult::Conflict { files, .. } => Ok(start_fix(
                Node::Rebase,
                Some(Inject::RebaseConflict {
                    files: files.clone(),
                }),
                ctx,
                progress,
            )),
            _ => apply_with_merge_request(state, trigger, ctx, merge_requested),
        },
        _ => apply_with_merge_request(state, trigger, ctx, merge_requested),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn setup_failure_fails_a_bug_task_at_its_bug_node() {
        for node in [Node::Locate, Node::ReproCheck, Node::FixGate] {
            let result = at(
                node,
                Trigger::SetupFailed {
                    command: "install".into(),
                    exit_code: 124,
                    log: "setup.log".into(),
                },
            );
            assert_eq!(
                result.next,
                TaskState::Failed {
                    at: node,
                    reason: FailureReason::Setup {
                        command: "install".into(),
                        exit_code: 124,
                        log: "setup.log".into()
                    }
                }
            );
            assert_eq!(result.action, Action::None);
        }
    }
    use super::*;
    use crate::{
        config::{GateConfig, GlobalConfig, ProjectConfig, ResolvedConfig, resolve},
        session::SessionKind,
        status_block,
    };
    fn config() -> ResolvedConfig {
        resolve(&GlobalConfig::default(), &ProjectConfig::default())
    }
    fn active(node: Node) -> TaskState {
        TaskState::Active { node }
    }
    fn ctx(config: &ResolvedConfig) -> Context<'_> {
        Context {
            bug_test_defect: false,
            handoff_pending: false,
            config,
            budget_n: None,
            decisions: PendingDecisions::default(),
        }
    }
    fn run(
        state: &TaskState,
        trigger: &Trigger,
        context: &Context<'_>,
        progress: BugProgress,
    ) -> Result<Transition, Rejected> {
        apply_for(TaskKind::Bug, state, trigger, context, progress, false)
    }
    fn at(node: Node, trigger: Trigger) -> Transition {
        run(
            &active(node),
            &trigger,
            &ctx(&config()),
            BugProgress::default(),
        )
        .unwrap()
    }
    fn ended(status: BugStatus, review: BugReview) -> Trigger {
        Trigger::BugSessionEnded {
            outcome: BugSessionOutcome::Ok {
                doc: Box::new(BugDoc {
                    regression_defect: false,
                    regression_defect_section: None,
                    status,
                    review,
                    fix_round: 0,
                    fix_limit: 3,
                    repro_command: "test bug".into(),
                    root_cause: "cause".into(),
                    repos: vec![],
                }),
            },
        }
    }
    fn repro(code: i32, timed_out: bool) -> Trigger {
        Trigger::ReproDone {
            paired: None,
            head: "sha".into(),
            command: "test bug".into(),
            exit_code: code,
            timed_out,
            tail: "tail".into(),
        }
    }
    fn gate(node: Node, passed: bool) -> Trigger {
        Trigger::GateDone {
            node,
            outcome: if passed {
                GateOutcome::Passed
            } else {
                GateOutcome::Failed {
                    head: "sha".into(),
                    command: "test bug".into(),
                    tail: "tail".into(),
                }
            },
            status: None,
        }
    }
    fn core_done(node: Node, result: CoreStepResult) -> Trigger {
        Trigger::CoreStepDone { node, result }
    }
    fn off(config: &mut ResolvedConfig, role: Role) {
        config
            .roles
            .iter_mut()
            .find(|r| r.role == role)
            .unwrap()
            .config
            .enabled = false;
    }
    fn reasons() -> Vec<FailureReason> {
        vec![
            FailureReason::DesignBudget { limit: 3 },
            FailureReason::ImplBudget { limit: 3 },
            FailureReason::FixBudget { limit: 3 },
            FailureReason::Infeasible,
            FailureReason::Protocol {
                detail: "bad".into(),
            },
            FailureReason::SessionCrashed {
                detail: "crash".into(),
            },
            FailureReason::RebaseConflict {
                detail: "conflict".into(),
            },
            FailureReason::Config {
                detail: "config".into(),
            },
            FailureReason::CoreStep {
                node: Node::Rebase,
                detail: "core".into(),
            },
        ]
    }
    fn triggers() -> Vec<Trigger> {
        let mut list = vec![
            Trigger::Start,
            Trigger::SlotAvailable,
            Trigger::Approve,
            Trigger::Reject {
                feedback: "feedback".into(),
            },
            Trigger::Merge,
            Trigger::Pause,
            Trigger::Resume,
            Trigger::Stop,
            Trigger::Cancel,
            Trigger::ExtendBudget { extra_rounds: 1 },
            Trigger::ExtendBudget { extra_rounds: 0 },
            Trigger::Reply {
                info: "info".into(),
            },
            ended(BugStatus::Located, BugReview::Pending),
            repro(1, false),
        ];
        for status in [
            status_block::DocStatus::Designing,
            status_block::DocStatus::Implementing,
            status_block::DocStatus::Done,
            status_block::DocStatus::Infeasible,
            status_block::DocStatus::ProtocolFailure,
        ] {
            let block = status_block::StatusBlock {
                status,
                design_round: 1,
                design_round_limit: 15,
                impl_round: 1,
                impl_round_limit: 25,
                current_milestone: None,
                current_milestone_reopens: 0,
                convergence_mode: status_block::ConvergenceMode::Normal,
                next_action: "无".into(),
                repos: vec![],
                milestones: vec![],
                backlog: vec![],
                disputes: vec![],
                manual_items: vec![],
            };
            list.push(Trigger::SessionEnded {
                outcome: SessionOutcome::Ok {
                    status: Box::new(block),
                },
            });
        }
        for outcome in [
            SessionOutcome::Crashed {
                detail: "crash".into(),
            },
            SessionOutcome::MissingArtifact {
                path: "missing".into(),
            },
            SessionOutcome::GuardFailed {
                detail: "guard".into(),
            },
            SessionOutcome::Unparseable {
                error: status_block::parse("").unwrap_err(),
            },
        ] {
            list.push(Trigger::SessionEnded { outcome });
        }
        for node in Node::ALL.into_iter().chain(Node::BUG) {
            list.push(Trigger::RerunFrom { node });
            list.push(gate(node, true));
            list.push(gate(node, false));
            for result in [
                CoreStepResult::Ok,
                CoreStepResult::Conflict {
                    files: vec!["file".into()],
                    detail: "conflict".into(),
                },
                CoreStepResult::Behind {
                    repos: vec![],
                    detail: "behind".into(),
                },
                CoreStepResult::Blocked {
                    detail: "blocked".into(),
                },
                CoreStepResult::Failed {
                    detail: "failed".into(),
                },
            ] {
                list.push(core_done(node, result));
            }
        }
        list
    }
    fn same_failure_state(node: Node) -> TaskState {
        TaskState::Failed {
            at: node,
            reason: FailureReason::GateSameFailure {
                count: 3,
                summary: "S".into(),
                log: "/l".into(),
            },
        }
    }

    #[test]
    fn same_failure_bug_gate_trips_and_rejects_other_nodes() {
        for node in [Node::FixGate, Node::MergeGate] {
            let t = run(
                &active(node),
                &Trigger::GateSameFailure {
                    node,
                    count: 3,
                    summary: "S".into(),
                    log: "/l".into(),
                },
                &ctx(&config()),
                BugProgress {
                    fix_round: u32::MAX,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(t.next, same_failure_state(node));
            assert_eq!(t.action, Action::None);
        }
        for (current, node) in [
            (Node::Gate, Node::Gate),
            (Node::MainCheck, Node::MainCheck),
            (Node::ReproCheck, Node::ReproCheck),
            (Node::Fix, Node::Fix),
            (Node::FixGate, Node::MergeGate),
        ] {
            let e = run(
                &active(current),
                &Trigger::GateSameFailure {
                    node,
                    count: 3,
                    summary: "S".into(),
                    log: "/l".into(),
                },
                &ctx(&config()),
                BugProgress::default(),
            )
            .unwrap_err();
            assert!(e.reason.contains("门禁结果与当前节点不符"));
        }
    }

    #[test]
    fn same_failure_bug_rerun_and_extend() {
        let c = config();
        let mut context = ctx(&c);
        for node in [Node::FixGate, Node::MergeGate] {
            let state = same_failure_state(node);
            let t = run(
                &state,
                &Trigger::RerunFrom { node },
                &context,
                BugProgress::default(),
            )
            .unwrap();
            assert_eq!(t.next, active(node));
            assert_eq!(t.action, Action::DiscardGateRuns { node });
            for budget in [None, Some(25)] {
                context.budget_n = budget;
                let t = run(
                    &state,
                    &Trigger::ExtendBudget { extra_rounds: 5 },
                    &context,
                    BugProgress::default(),
                )
                .unwrap();
                assert_eq!(t.next, active(node));
                assert_eq!(t.action, Action::DiscardGateRuns { node });
                assert_eq!(t.budget_n, Some(budget.unwrap_or(c.bug.fix_rounds) + 5));
            }
            assert!(
                run(
                    &state,
                    &Trigger::ExtendBudget { extra_rounds: 0 },
                    &context,
                    BugProgress::default()
                )
                .is_err()
            );
            context.budget_n = Some(u32::MAX);
            assert!(
                run(
                    &state,
                    &Trigger::ExtendBudget { extra_rounds: 1 },
                    &context,
                    BugProgress::default()
                )
                .is_err()
            );
            for other in [
                Node::Gate,
                Node::ReproCheck,
                if node == Node::FixGate {
                    Node::MergeGate
                } else {
                    Node::FixGate
                },
            ] {
                assert!(
                    run(
                        &state,
                        &Trigger::RerunFrom { node: other },
                        &context,
                        BugProgress::default()
                    )
                    .is_err()
                );
            }
            let other_failure = TaskState::Failed {
                at: node,
                reason: FailureReason::Infeasible,
            };
            assert!(
                run(
                    &other_failure,
                    &Trigger::RerunFrom { node },
                    &context,
                    BugProgress::default()
                )
                .is_err()
            );
            let t = run(
                &state,
                &Trigger::RerunFrom { node: Node::Fix },
                &context,
                BugProgress::default(),
            )
            .unwrap();
            assert_eq!(t.next, active(Node::Fix));
            assert!(matches!(t.action, Action::StartBugSession { .. }));
        }
    }

    #[test]
    fn bug_loop_nodes_have_wire_names_roles_and_flags() {
        for (node, wire, role, core_step, wait) in [
            (Node::Locate, "locate", Some(Role::Impl), false, false),
            (Node::ReproCheck, "repro_check", None, true, false),
            (Node::Fix, "fix", Some(Role::Impl), false, false),
            (Node::FixGate, "fix_gate", None, true, false),
            (
                Node::FixReview,
                "fix_review",
                Some(Role::Audit),
                false,
                false,
            ),
            (Node::AwaitInfo, "await_info", None, false, true),
            (Node::NotABug, "not_a_bug", None, false, true),
        ] {
            assert_eq!(node.as_str(), wire);
            assert_eq!(Node::parse(wire), Some(node));
            assert_eq!(serde_json::to_string(&node).unwrap(), format!("\"{wire}\""));
            assert_eq!(node.role(), role);
            assert_eq!(node.is_core_step(), core_step);
            assert_eq!(node.is_gate(), core_step);
            assert_eq!(node.awaits_user(), wait);
            assert_eq!(node.occupies_slot(), !wait);
            if let Some(role) = role {
                let kind = SessionKind::for_node(node).unwrap();
                assert_eq!(kind.role(), Some(role));
                assert!(matches!(kind, SessionKind::Bug { .. }));
            } else {
                assert!(SessionKind::for_node(node).is_none());
            }
        }
    }
    #[test]
    fn bug_loop_feature_node_list_is_unchanged() {
        assert_eq!(
            Node::ALL.map(Node::as_str).to_vec(),
            [
                "intake",
                "design",
                "review",
                "adjudicate",
                "await_design_approval",
                "implement",
                "gate",
                "audit",
                "retro",
                "rebase",
                "merge_gate",
                "await_merge",
                "merging",
                "main_check",
                "cleanup"
            ]
        );
        assert!(Node::BUG.iter().all(|n| !Node::ALL.contains(n)));
    }
    #[test]
    fn bug_loop_feature_kind_matches_the_feature_table_exactly() {
        let cfg = config();
        let context = ctx(&cfg);
        let triggers = triggers();
        let mut states = vec![
            TaskState::Draft,
            TaskState::Queued,
            TaskState::Done,
            TaskState::Cancelled,
        ];
        for node in Node::ALL {
            states.extend([
                active(node),
                TaskState::Paused { resume: node },
                TaskState::Stopped { at: node },
                TaskState::Requeued { resume: node },
            ]);
            for reason in reasons() {
                states.push(TaskState::Failed { at: node, reason });
            }
        }
        for state in states {
            for trigger in &triggers {
                for requested in [false, true] {
                    assert_eq!(
                        apply_for(
                            TaskKind::Feature,
                            &state,
                            trigger,
                            &context,
                            BugProgress::default(),
                            requested
                        ),
                        apply_with_merge_request(&state, trigger, &context, requested),
                        "{state:?} {trigger:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn bug_loop_feature_table_rejects_bug_triggers_and_never_enters_bug_nodes() {
        let cfg = config();
        let context = ctx(&cfg);
        for node in Node::ALL {
            for trigger in [
                ended(BugStatus::Located, BugReview::Pending),
                repro(1, false),
                Trigger::Reply {
                    info: "info".into(),
                },
            ] {
                assert!(
                    apply_with_merge_request(&active(node), &trigger, &context, false).is_err()
                );
            }
        }
        for node in Node::BUG {
            for trigger in triggers() {
                if matches!(
                    trigger,
                    Trigger::Cancel | Trigger::Pause | Trigger::Stop | Trigger::Resume
                ) {
                    continue;
                }
                assert!(
                    apply_with_merge_request(&active(node), &trigger, &context, false).is_err(),
                    "{node:?} {trigger:?}"
                );
            }
            assert_eq!(
                apply_with_merge_request(&active(node), &Trigger::Cancel, &context, false)
                    .unwrap()
                    .next,
                TaskState::Cancelled
            );
            for trigger in [Trigger::Pause, Trigger::Stop] {
                assert_eq!(
                    apply_with_merge_request(&active(node), &trigger, &context, false).is_ok(),
                    !node.awaits_user()
                );
            }
            assert!(
                apply_with_merge_request(
                    &TaskState::Paused { resume: node },
                    &Trigger::Resume,
                    &context,
                    false
                )
                .is_ok()
            );
            assert_eq!(
                apply_with_merge_request(
                    &TaskState::Requeued { resume: node },
                    &Trigger::SlotAvailable,
                    &context,
                    false
                )
                .unwrap()
                .next,
                active(node)
            );
        }
    }
    #[test]
    fn bug_loop_legacy_bug_tasks_already_in_feature_nodes_keep_the_feature_table() {
        for node in Node::ALL {
            assert!(!uses_bug_loop(TaskKind::Bug, &active(node), false));
            assert!(uses_bug_loop(TaskKind::Bug, &active(node), true));
            assert!(!uses_bug_loop(TaskKind::Feature, &active(node), true));
        }
        for state in [TaskState::Draft, TaskState::Queued] {
            assert!(uses_bug_loop(TaskKind::Bug, &state, false));
        }
    }
    #[test]
    fn bug_loop_a_queued_bug_starts_at_locate() {
        let t = run(
            &TaskState::Queued,
            &Trigger::SlotAvailable,
            &ctx(&config()),
            BugProgress::default(),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Locate));
        assert_eq!(
            t.action,
            Action::StartBugSession {
                step: BugStep::Locate,
                inject: None
            }
        );
    }
    #[test]
    fn bug_loop_impl_disabled_fails_with_config_at_start() {
        let mut cfg = config();
        off(&mut cfg, Role::Impl);
        assert!(matches!(
            run(
                &TaskState::Queued,
                &Trigger::SlotAvailable,
                &ctx(&cfg),
                BugProgress::default()
            )
            .unwrap()
            .next,
            TaskState::Failed {
                reason: FailureReason::Config { .. },
                ..
            }
        ));
    }
    #[test]
    fn bug_loop_located_runs_the_repro_check() {
        assert_eq!(
            at(Node::Locate, ended(BugStatus::Located, BugReview::Pending)),
            core(Node::ReproCheck)
        );
    }
    #[test]
    fn bug_loop_cannot_reproduce_and_not_a_bug_wait_without_a_slot() {
        for (status, node) in [
            (BugStatus::CannotReproduce, Node::AwaitInfo),
            (BugStatus::NotABug, Node::NotABug),
        ] {
            let t = at(Node::Locate, ended(status, BugReview::Pending));
            assert_eq!(t.next, active(node));
            assert!(!t.next.occupies_slot());
            assert!(t.next.awaits_user());
            assert_eq!(t.action, Action::None);
        }
    }
    #[test]
    fn bug_loop_reply_restores_a_fix_window_without_losing_history() {
        let cfg = config();
        for state in [
            TaskState::Stopped { at: Node::FixGate },
            TaskState::Failed {
                at: Node::Fix,
                reason: FailureReason::FixBudget { limit: 3 },
            },
        ] {
            for (used, budget) in [
                (0, None),
                (3, Some(3)),
                (3, Some(20)),
                (u32::MAX - cfg.bug.fix_rounds, None),
            ] {
                let mut context = ctx(&cfg);
                context.budget_n = budget;
                let progress = BugProgress {
                    fix_round: used,
                    repro_misses: 0,
                    ..BugProgress::default()
                };
                let t = run(
                    &state,
                    &Trigger::Reply {
                        info: "补充".into(),
                    },
                    &context,
                    progress,
                )
                .unwrap();
                assert_eq!(t.next, active(Node::Locate));
                assert_eq!(
                    t.action,
                    Action::StartBugSession {
                        step: BugStep::Locate,
                        inject: None
                    }
                );
                assert_eq!(
                    t.budget_n,
                    Some(
                        budget
                            .unwrap_or(cfg.bug.fix_rounds)
                            .max(used + cfg.bug.fix_rounds)
                    )
                );
                context.budget_n = t.budget_n;
                let fix = run(
                    &active(Node::ReproCheck),
                    &repro(1, false),
                    &context,
                    progress,
                )
                .unwrap();
                assert_eq!(fix.next, active(Node::Fix));
                assert_eq!(fix.budget_n, t.budget_n);
            }
            for info in ["", " \n\t"] {
                assert!(
                    run(
                        &state,
                        &Trigger::Reply { info: info.into() },
                        &ctx(&cfg),
                        BugProgress::default()
                    )
                    .is_err()
                );
            }
            assert!(
                run(
                    &state,
                    &Trigger::Reply {
                        info: "补充".into()
                    },
                    &ctx(&cfg),
                    BugProgress {
                        fix_round: u32::MAX,
                        repro_misses: 0,
                        ..BugProgress::default()
                    }
                )
                .unwrap_err()
                .reason
                .contains("溢出")
            );
        }
        for node in [Node::AwaitInfo, Node::NotABug] {
            let t = run(
                &active(node),
                &Trigger::Reply {
                    info: "补充".into(),
                },
                &ctx(&cfg),
                BugProgress {
                    fix_round: 3,
                    repro_misses: 0,
                    ..BugProgress::default()
                },
            )
            .unwrap();
            assert_eq!(t.budget_n, None);
        }
    }

    #[test]
    fn bug_loop_reply_invalid_states_explain_the_next_command() {
        for (state, command) in [
            (TaskState::Draft, "autome task start"),
            (TaskState::Queued, "autome status"),
            (
                TaskState::Requeued {
                    resume: Node::Locate,
                },
                "autome status",
            ),
            (TaskState::Paused { resume: Node::Fix }, "autome resume"),
            (TaskState::Done, "autome task new"),
            (TaskState::Cancelled, "autome task new"),
        ]
        .into_iter()
        .chain(
            Node::ALL
                .into_iter()
                .chain(Node::BUG)
                .filter(|node| !matches!(node, Node::AwaitInfo | Node::NotABug))
                .map(|node| (active(node), "autome stop")),
        ) {
            let reason = run(
                &state,
                &Trigger::Reply {
                    info: "补充".into(),
                },
                &ctx(&config()),
                BugProgress::default(),
            )
            .unwrap_err()
            .reason;
            assert!(reason.contains(command), "{reason}");
            assert!(!reason.contains("转换表"), "{reason}");
        }
    }

    #[test]
    fn bug_loop_reply_returns_to_locate_from_both_waiting_nodes() {
        for node in [Node::AwaitInfo, Node::NotABug] {
            assert_eq!(
                at(
                    node,
                    Trigger::Reply {
                        info: "补充  信息".into()
                    }
                )
                .action,
                Action::StartBugSession {
                    step: BugStep::Locate,
                    inject: None
                }
            );
        }
    }
    #[test]
    fn bug_loop_reply_is_refused_when_empty_or_not_waiting() {
        for node in Node::BUG.into_iter().chain(Node::ALL) {
            for info in ["", " \n\t", "info"] {
                assert_eq!(
                    run(
                        &active(node),
                        &Trigger::Reply { info: info.into() },
                        &ctx(&config()),
                        BugProgress::default()
                    )
                    .is_ok(),
                    matches!(node, Node::AwaitInfo | Node::NotABug) && info == "info"
                );
            }
        }
    }
    #[test]
    fn bug_loop_waiting_nodes_allow_cancel_but_not_pause_stop_or_extend() {
        for node in [Node::AwaitInfo, Node::NotABug] {
            for trigger in [
                Trigger::Cancel,
                Trigger::Pause,
                Trigger::Stop,
                Trigger::ExtendBudget { extra_rounds: 1 },
            ] {
                let cfg = config();
                let context = ctx(&cfg);
                let result = run(&active(node), &trigger, &context, BugProgress::default());
                assert_eq!(
                    result,
                    apply_with_merge_request(&active(node), &trigger, &context, false)
                );
                if trigger == Trigger::Cancel {
                    let t = result.unwrap();
                    assert_eq!(t.next, TaskState::Cancelled);
                    assert_eq!(t.action, Action::RunCancel);
                } else {
                    assert!(result.is_err());
                }
            }
        }
    }
    #[test]
    fn bug_loop_reproduced_starts_fix_round_one_with_the_configured_budget() {
        for limit in [1, 3, 20] {
            let mut cfg = config();
            cfg.bug.fix_rounds = limit;
            let t = run(
                &active(Node::ReproCheck),
                &repro(1, false),
                &ctx(&cfg),
                BugProgress::default(),
            )
            .unwrap();
            assert_eq!(t.next, active(Node::Fix));
            assert_eq!(t.budget_n, Some(limit));
            assert_eq!(
                t.action,
                Action::StartBugSession {
                    step: BugStep::Fix,
                    inject: None
                }
            );
        }
    }
    #[test]
    fn bug_loop_exit_zero_returns_to_locate_with_the_tail() {
        let t = at(Node::ReproCheck, repro(0, false));
        assert_eq!(t.next, active(Node::Locate));
        assert_eq!(
            t.action,
            Action::StartBugSession {
                step: BugStep::Locate,
                inject: Some(Inject::ReproNotFailing {
                    head: "sha".into(),
                    command: "test bug".into(),
                    exit_code: 0,
                    why: "exited_zero".into(),
                    tail: "tail".into()
                })
            }
        );
    }
    #[test]
    fn bug_loop_timeout_and_unrunnable_commands_do_not_count_as_reproduced() {
        for (code, timeout, why) in [
            (1, true, "timed_out"),
            (0, true, "timed_out"),
            (126, false, "not_runnable"),
            (127, false, "not_runnable"),
        ] {
            let t = at(Node::ReproCheck, repro(code, timeout));
            assert_eq!(t.next, active(Node::Locate));
            assert!(
                matches!(t.action,Action::StartBugSession { inject:Some(Inject::ReproNotFailing { why: actual,.. }),.. } if actual==why)
            );
        }
        for code in [1, 125, 128, 255, -1] {
            assert_eq!(
                at(Node::ReproCheck, repro(code, false)).next,
                active(Node::Fix)
            );
        }
    }
    #[test]
    fn bug_loop_second_consecutive_miss_waits_for_info() {
        for misses in [1, u32::MAX] {
            for (code, timeout) in [(0, false), (1, true), (126, false), (127, false)] {
                let t = run(
                    &active(Node::ReproCheck),
                    &repro(code, timeout),
                    &ctx(&config()),
                    BugProgress {
                        fix_round: 0,
                        test_fixes: 0,
                        test_fix_open: false,
                        repro_misses: misses,
                    },
                )
                .unwrap();
                assert_eq!(t.next, active(Node::AwaitInfo));
                assert_eq!(t.action, Action::None);
            }
        }
    }
    #[test]
    fn bug_loop_fix_always_runs_the_fix_gate() {
        for gate in [
            None,
            Some(GateConfig {
                command: "true".into(),
                timeout_minutes: 1,
            }),
        ] {
            let mut cfg = config();
            cfg.gate = gate;
            assert_eq!(
                run(
                    &active(Node::Fix),
                    &ended(BugStatus::Located, BugReview::Pending),
                    &ctx(&cfg),
                    BugProgress::default()
                )
                .unwrap(),
                core(Node::FixGate)
            );
        }
    }
    #[test]
    fn bug_loop_failed_fix_gate_returns_to_fix_with_the_tail() {
        let t = at(Node::FixGate, gate(Node::FixGate, false));
        assert_eq!(t.next, active(Node::Fix));
        assert_eq!(
            t.action,
            Action::StartBugSession {
                step: BugStep::Fix,
                inject: Some(Inject::GateFailed {
                    head: "sha".into(),
                    command: "test bug".into(),
                    tail: "tail".into()
                })
            }
        );
    }
    #[test]
    fn bug_loop_passed_fix_gate_requires_review_even_when_audit_is_off() {
        let mut cfg = config();
        assert_eq!(
            at(Node::FixGate, gate(Node::FixGate, true)).next,
            active(Node::FixReview)
        );
        off(&mut cfg, Role::Audit);
        assert_eq!(
            run(
                &active(Node::FixGate),
                &gate(Node::FixGate, true),
                &ctx(&cfg),
                BugProgress::default()
            )
            .unwrap(),
            Transition::fail(
                Node::FixReview,
                FailureReason::Config {
                    detail: "该角色当前已关闭".into(),
                },
            )
        );
    }
    #[test]
    fn bug_loop_review_pass_goes_to_rebase_without_retro() {
        assert_eq!(
            at(
                Node::FixReview,
                ended(BugStatus::Located, BugReview::Passed)
            ),
            core(Node::Rebase)
        );
    }
    #[test]
    fn bug_loop_review_reject_returns_to_fix() {
        assert_eq!(
            at(
                Node::FixReview,
                ended(BugStatus::Located, BugReview::Rejected)
            )
            .action,
            Action::StartBugSession {
                step: BugStep::Fix,
                inject: Some(Inject::FixRejected)
            }
        );
    }
    #[test]
    fn bug_loop_fix_budget_runs_out_after_the_configured_rounds() {
        let cfg = config();
        let context = Context {
            budget_n: Some(2),
            ..ctx(&cfg)
        };
        for (node, trigger) in [
            (Node::FixGate, gate(Node::FixGate, false)),
            (Node::MergeGate, gate(Node::MergeGate, false)),
            (
                Node::FixReview,
                ended(BugStatus::Located, BugReview::Rejected),
            ),
            (Node::ReproCheck, repro(1, false)),
            (
                Node::Rebase,
                core_done(
                    Node::Rebase,
                    CoreStepResult::Conflict {
                        files: vec![],
                        detail: "conflict".into(),
                    },
                ),
            ),
        ] {
            for round in [1, 2, 3] {
                let t = run(
                    &active(node),
                    &trigger,
                    &context,
                    BugProgress {
                        fix_round: round,
                        test_fixes: 0,
                        test_fix_open: false,
                        repro_misses: 0,
                    },
                )
                .unwrap();
                if round < 2 {
                    assert_eq!(t.next, active(Node::Fix));
                } else {
                    assert_eq!(
                        t.next,
                        TaskState::Failed {
                            at: node,
                            reason: FailureReason::FixBudget { limit: 2 }
                        }
                    );
                }
            }
        }
    }
    #[test]
    fn bug_loop_extend_resumes_fix_with_a_larger_limit() {
        let state = TaskState::Failed {
            at: Node::FixGate,
            reason: FailureReason::FixBudget { limit: 3 },
        };
        let cfg = config();
        let t = run(
            &state,
            &Trigger::ExtendBudget { extra_rounds: 2 },
            &ctx(&cfg),
            BugProgress {
                fix_round: 3,
                test_fixes: 0,
                test_fix_open: false,
                repro_misses: 0,
            },
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Fix));
        assert_eq!(t.budget_n, Some(5));
        for extra in [0, u32::MAX] {
            assert!(
                run(
                    &state,
                    &Trigger::ExtendBudget {
                        extra_rounds: extra
                    },
                    &ctx(&cfg),
                    BugProgress::default()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn bug_loop_rebase_always_runs_the_merge_gate() {
        for requested in [false, true] {
            let t = apply_for(
                TaskKind::Bug,
                &active(Node::Rebase),
                &core_done(Node::Rebase, CoreStepResult::Ok),
                &ctx(&config()),
                BugProgress::default(),
                requested,
            )
            .unwrap();
            assert_eq!(t.next, active(Node::MergeGate));
            assert_eq!(
                t.action,
                Action::RunCoreStep {
                    node: Node::MergeGate
                }
            );
            assert!(!t.grants_merge_request);
        }
    }
    #[test]
    fn merge_conflict_a_bug_dequeue_conflict_goes_back_to_fix() {
        let c = config();
        for requested in [false, true] {
            for files in [vec![], vec!["file".into()]] {
                let trigger = core_done(
                    Node::MergeGate,
                    CoreStepResult::Conflict {
                        files: files.clone(),
                        detail: "conflict".into(),
                    },
                );
                let t = apply_for(
                    TaskKind::Bug,
                    &active(Node::MergeGate),
                    &trigger,
                    &ctx(&c),
                    BugProgress::default(),
                    requested,
                )
                .unwrap();
                assert_eq!(t.next, active(Node::Fix));
                assert_eq!(
                    t.action,
                    Action::StartBugSession {
                        step: BugStep::Fix,
                        inject: Some(Inject::RebaseConflict { files })
                    }
                );
                let t = apply_for(
                    TaskKind::Bug,
                    &active(Node::MergeGate),
                    &trigger,
                    &ctx(&c),
                    BugProgress {
                        fix_round: c.bug.fix_rounds,
                        test_fixes: 0,
                        test_fix_open: false,
                        repro_misses: 0,
                    },
                    requested,
                )
                .unwrap();
                assert!(matches!(
                    t.next,
                    TaskState::Failed {
                        at: Node::MergeGate,
                        reason: FailureReason::FixBudget { .. }
                    }
                ));
            }
        }
        assert!(
            apply_for(
                TaskKind::Bug,
                &active(Node::MergeGate),
                &core_done(
                    Node::Rebase,
                    CoreStepResult::Conflict {
                        files: vec![],
                        detail: "mismatch".into()
                    }
                ),
                &ctx(&c),
                BugProgress::default(),
                false
            )
            .is_err()
        );
    }

    #[test]
    fn bug_loop_rebase_conflict_and_merge_gate_failure_return_to_fix_not_implement() {
        let t = at(
            Node::Rebase,
            core_done(
                Node::Rebase,
                CoreStepResult::Conflict {
                    files: vec!["file".into()],
                    detail: "conflict".into(),
                },
            ),
        );
        assert_eq!(t.next, active(Node::Fix));
        assert_eq!(
            t.action,
            Action::StartBugSession {
                step: BugStep::Fix,
                inject: Some(Inject::RebaseConflict {
                    files: vec!["file".into()]
                })
            }
        );
        assert_eq!(
            at(Node::MergeGate, gate(Node::MergeGate, false)).next,
            active(Node::Fix)
        );
    }
    #[test]
    fn bug_loop_merge_tail_is_shared_with_the_feature_loop() {
        for node in [
            Node::MergeGate,
            Node::AwaitMerge,
            Node::Merging,
            Node::MainCheck,
            Node::Cleanup,
        ] {
            for trigger in triggers() {
                if matches!(
                    trigger,
                    Trigger::GateDone {
                        node: Node::MergeGate,
                        outcome: GateOutcome::Failed { .. },
                        ..
                    }
                ) {
                    continue;
                }
                for requested in [false, true] {
                    let cfg = config();
                    let context = ctx(&cfg);
                    if node == Node::MergeGate
                        && let Trigger::CoreStepDone {
                            node: Node::MergeGate,
                            result: CoreStepResult::Conflict { files, .. },
                        } = &trigger
                    {
                        assert_eq!(
                            apply_for(
                                TaskKind::Bug,
                                &active(node),
                                &trigger,
                                &context,
                                BugProgress::default(),
                                requested,
                            )
                            .unwrap(),
                            start_fix(
                                node,
                                Some(Inject::RebaseConflict {
                                    files: files.clone()
                                }),
                                &context,
                                BugProgress::default(),
                            )
                        );
                        continue;
                    }
                    assert_eq!(
                        apply_for(
                            TaskKind::Bug,
                            &active(node),
                            &trigger,
                            &context,
                            BugProgress::default(),
                            requested
                        ),
                        if matches!(trigger, Trigger::Reply { .. }) {
                            // Reply is still rejected, now with Bug-specific recovery guidance.
                            Err(rejected(
                                "Bug 任务正在处理，请用 autome status <任务> 查看进度，或用 autome stop <任务> 停止后再 reply",
                            ))
                        } else {
                            apply_with_merge_request(&active(node), &trigger, &context, requested)
                        }
                    );
                }
            }
        }
    }
    #[test]
    fn bug_loop_session_errors_fail_like_feature_sessions() {
        for node in [Node::Locate, Node::Fix, Node::FixReview] {
            for outcome in [
                BugSessionOutcome::Crashed {
                    detail: "crash".into(),
                },
                BugSessionOutcome::Unparseable {
                    error: BugDocError {
                        detail: "status".into(),
                    },
                },
                BugSessionOutcome::MissingArtifact {
                    path: "bug.md".into(),
                },
                BugSessionOutcome::GuardFailed {
                    detail: "复核".into(),
                },
            ] {
                let crashed = matches!(outcome, BugSessionOutcome::Crashed { .. });
                let t = at(node, Trigger::BugSessionEnded { outcome });
                assert!(
                    matches!(t.next,TaskState::Failed { at,reason:FailureReason::SessionCrashed { .. } } if at==node)
                        && crashed
                        || matches!(t.next,TaskState::Failed { at,reason:FailureReason::Protocol { .. } } if at==node)
                            && !crashed
                );
            }
            for status in [
                BugStatus::Located,
                BugStatus::CannotReproduce,
                BugStatus::NotABug,
            ] {
                for review in [BugReview::Pending, BugReview::Passed, BugReview::Rejected] {
                    let valid = (node == Node::Locate || status == BugStatus::Located)
                        && ((node == Node::FixReview) != (review == BugReview::Pending));
                    assert_eq!(
                        matches!(
                            at(node, ended(status, review)).next,
                            TaskState::Failed {
                                reason: FailureReason::Protocol { .. },
                                ..
                            }
                        ),
                        !valid
                    );
                }
            }
        }
    }
    #[test]
    fn bug_loop_rerun_from_accepts_locate_fix_review_and_rebase_only() {
        for state in [
            TaskState::Stopped { at: Node::Fix },
            TaskState::Failed {
                at: Node::Fix,
                reason: FailureReason::FixBudget { limit: 1 },
            },
        ] {
            for node in Node::ALL.into_iter().chain(Node::BUG) {
                let t = run(
                    &state,
                    &Trigger::RerunFrom { node },
                    &ctx(&config()),
                    BugProgress {
                        fix_round: 20,
                        test_fixes: 0,
                        test_fix_open: false,
                        repro_misses: 0,
                    },
                );
                assert_eq!(
                    t.is_ok(),
                    matches!(
                        node,
                        Node::Locate | Node::Fix | Node::FixReview | Node::Rebase
                    )
                );
                if let Ok(t) = t {
                    if node == Node::Fix {
                        assert_eq!(
                            t.next,
                            TaskState::Failed {
                                at: Node::Fix,
                                reason: FailureReason::FixBudget {
                                    limit: config().bug.fix_rounds,
                                },
                            }
                        );
                        assert_eq!(t.action, Action::None);
                    } else {
                        assert_eq!(t.next, active(node));
                    }
                }
            }
        }
    }
    #[test]
    fn bug_loop_rerun_fix_checks_configured_and_extended_budgets() {
        let mut cfg = config();
        cfg.bug.fix_rounds = 1;
        for budget in [None, Some(3)] {
            let limit = budget.unwrap_or(cfg.bug.fix_rounds);
            let mut context = ctx(&cfg);
            context.budget_n = budget;
            for state in [
                TaskState::Stopped { at: Node::Fix },
                TaskState::Failed {
                    at: Node::Fix,
                    reason: FailureReason::FixBudget { limit },
                },
            ] {
                for fix_round in [limit - 1, limit, limit + 1] {
                    let t = run(
                        &state,
                        &Trigger::RerunFrom { node: Node::Fix },
                        &context,
                        BugProgress {
                            fix_round,
                            test_fixes: 0,
                            test_fix_open: false,
                            repro_misses: 0,
                        },
                    )
                    .unwrap();
                    if fix_round < limit {
                        assert_eq!(t.next, active(Node::Fix));
                        assert_eq!(t.budget_n, Some(limit));
                        assert_eq!(
                            t.action,
                            Action::StartBugSession {
                                step: BugStep::Fix,
                                inject: None,
                            }
                        );
                    } else {
                        assert_eq!(
                            t.next,
                            TaskState::Failed {
                                at: Node::Fix,
                                reason: FailureReason::FixBudget { limit },
                            }
                        );
                        assert_eq!(t.action, Action::None);
                    }
                }
            }
        }
    }
    #[test]
    fn bug_loop_stopped_bug_gates_discard_runs_and_requeued_bug_nodes_reenter() {
        for node in [Node::ReproCheck, Node::FixGate] {
            let t = run(
                &TaskState::Stopped { at: node },
                &Trigger::Resume,
                &ctx(&config()),
                BugProgress::default(),
            )
            .unwrap();
            assert_eq!(t.next, TaskState::Requeued { resume: node });
            assert_eq!(t.action, Action::DiscardGateRuns { node });
        }
        assert_eq!(
            at(Node::Fix, Trigger::Pause).next,
            TaskState::Paused { resume: Node::Fix }
        );
        let resumed = run(
            &TaskState::Paused { resume: Node::Fix },
            &Trigger::Resume,
            &ctx(&config()),
            BugProgress::default(),
        )
        .unwrap();
        assert_eq!(resumed.next, TaskState::Requeued { resume: Node::Fix });
        assert_eq!(resumed.action, Action::None);
        for node in Node::BUG {
            let t = run(
                &TaskState::Requeued { resume: node },
                &Trigger::SlotAvailable,
                &ctx(&config()),
                BugProgress::default(),
            )
            .unwrap();
            assert_eq!(t.next, active(node));
            assert_eq!(
                t.action,
                if node.is_core_step() {
                    Action::RunCoreStep { node }
                } else {
                    Action::Reenter { node }
                }
            );
        }
    }

    #[test]
    fn scope_patch_passed_review_returns_to_fix() {
        let c = config();
        let context = Context {
            bug_test_defect: false,
            handoff_pending: true,
            budget_n: Some(2),
            ..ctx(&c)
        };
        for review in [BugReview::Passed, BugReview::Rejected] {
            for round in [0, 1] {
                let t = run(
                    &active(Node::FixReview),
                    &ended(BugStatus::Located, review),
                    &context,
                    BugProgress {
                        fix_round: round,
                        test_fixes: 0,
                        test_fix_open: false,
                        repro_misses: 0,
                    },
                )
                .unwrap();
                assert_eq!(t.next, active(Node::Fix));
                assert_eq!(
                    t.action,
                    Action::StartBugSession {
                        step: BugStep::Fix,
                        inject: Some(Inject::FixRejected)
                    }
                );
            }
        }
    }

    #[test]
    fn scope_patch_passed_review_obeys_fix_budget() {
        let c = config();
        for limit in [0, 2, u32::MAX] {
            let context = Context {
                bug_test_defect: false,
                handoff_pending: true,
                budget_n: Some(limit),
                ..ctx(&c)
            };
            for round in [limit, u32::MAX] {
                let t = run(
                    &active(Node::FixReview),
                    &ended(BugStatus::Located, BugReview::Passed),
                    &context,
                    BugProgress {
                        fix_round: round,
                        test_fixes: 0,
                        test_fix_open: false,
                        repro_misses: 0,
                    },
                )
                .unwrap();
                assert_eq!(
                    t.next,
                    TaskState::Failed {
                        at: Node::FixReview,
                        reason: FailureReason::FixBudget { limit }
                    }
                );
            }
        }
    }

    #[test]
    fn scope_patch_clean_passed_review_rebases() {
        assert_eq!(
            at(
                Node::FixReview,
                ended(BugStatus::Located, BugReview::Passed)
            ),
            core(Node::Rebase)
        );
    }
    fn defect(review: BugReview) -> Trigger {
        let mut trigger = ended(BugStatus::Located, review);
        if let Trigger::BugSessionEnded {
            outcome: BugSessionOutcome::Ok { doc },
        } = &mut trigger
        {
            doc.regression_defect = true;
        }
        trigger
    }
    #[test]
    fn test_fix_fix_declaring_a_defect_starts_locate_without_a_fix_round() {
        let config = config();
        let mut context = ctx(&config);
        context.bug_test_defect = true;
        for used in [0, 1] {
            let progress = BugProgress {
                fix_round: u32::MAX,
                test_fixes: used,
                ..BugProgress::default()
            };
            let t = run(
                &active(Node::Fix),
                &defect(BugReview::Pending),
                &context,
                progress,
            )
            .unwrap();
            assert_eq!(t.next, active(Node::Locate));
            assert_eq!(
                t.action,
                Action::StartBugSession {
                    step: BugStep::Locate,
                    inject: None
                }
            );
            assert_eq!(t.budget_n, None);
            assert_eq!(progress.fix_round, u32::MAX);
            assert_eq!(
                t.test_fix_event,
                Some(TestFixEvent::Requested {
                    by: BugStep::Fix,
                    attempt: used + 1,
                    limit: 2
                })
            );
        }
        let mut disabled = config.clone();
        off(&mut disabled, Role::Impl);
        let mut disabled_ctx = ctx(&disabled);
        disabled_ctx.bug_test_defect = true;
        let t = run(
            &active(Node::Fix),
            &defect(BugReview::Pending),
            &disabled_ctx,
            BugProgress::default(),
        )
        .unwrap();
        assert!(matches!(
            t.next,
            TaskState::Failed {
                at: Node::Locate,
                reason: FailureReason::Config { .. }
            }
        ));
        assert_eq!(t.test_fix_event, None);
    }
    #[test]
    fn test_fix_review_declaring_a_defect_starts_locate() {
        let config = config();
        let mut context = ctx(&config);
        context.bug_test_defect = true;
        let t = run(
            &active(Node::FixReview),
            &defect(BugReview::Rejected),
            &context,
            BugProgress::default(),
        )
        .unwrap();
        assert_eq!(t.next, active(Node::Locate));
        assert_eq!(
            t.action,
            Action::StartBugSession {
                step: BugStep::Locate,
                inject: None
            }
        );
        assert_eq!(
            t.test_fix_event,
            Some(TestFixEvent::Requested {
                by: BugStep::FixReview,
                attempt: 1,
                limit: 2
            })
        );
        for (node, review) in [
            (Node::Locate, BugReview::Pending),
            (Node::FixReview, BugReview::Passed),
            (Node::FixReview, BugReview::Pending),
            (Node::Fix, BugReview::Rejected),
        ] {
            let t = run(
                &active(node),
                &defect(review),
                &context,
                BugProgress::default(),
            )
            .unwrap();
            assert!(matches!(
                t.next,
                TaskState::Failed {
                    reason: FailureReason::Protocol { .. },
                    ..
                }
            ));
            assert_eq!(t.test_fix_event, None);
        }
    }
    #[test]
    fn test_fix_limit_reached_fails_with_a_readable_reason() {
        let config = config();
        let mut context = ctx(&config);
        context.bug_test_defect = true;
        for used in [2, 3, u32::MAX] {
            for (node, review) in [
                (Node::Fix, BugReview::Pending),
                (Node::FixReview, BugReview::Rejected),
            ] {
                let t = run(
                    &active(node),
                    &defect(review),
                    &context,
                    BugProgress {
                        test_fixes: used,
                        ..BugProgress::default()
                    },
                )
                .unwrap();
                assert_eq!(
                    t.next,
                    TaskState::Failed {
                        at: node,
                        reason: FailureReason::TestFixLimit { limit: 2 }
                    }
                );
                assert_eq!(t.action, Action::None);
                assert_eq!(t.test_fix_event, None);
            }
        }
        assert_eq!(
            test_fix_limit_message(2),
            "回归测试已退回定位轮修正 2 次（上限 2），仍被判有缺陷。请人工检查回归测试后 autome rerun <任务> locate，或取消后重提"
        );
    }
    #[test]
    fn test_fix_old_protocol_treats_the_key_as_unparseable() {
        for (node, review) in [
            (Node::Locate, BugReview::Pending),
            (Node::Fix, BugReview::Pending),
            (Node::FixReview, BugReview::Rejected),
        ] {
            let bad = at(node, defect(review));
            let unparseable = at(
                node,
                Trigger::BugSessionEnded {
                    outcome: BugSessionOutcome::Unparseable {
                        error: BugDocError {
                            detail: "Bug 文档字段 回归测试 缺失、重复或值无效".into(),
                        },
                    },
                },
            );
            assert_eq!(bad, unparseable);
        }
    }
    #[test]
    fn test_fix_pair_judgement_accept_needs_fix_not_reproduced() {
        for base_exit in [i32::MIN, -1, 0, 1, 126, 127, i32::MAX] {
            for head_exit in [i32::MIN, 0, 1, 126, 127, i32::MAX] {
                for base_timed_out in [false, true] {
                    for head_timed_out in [false, true] {
                        let expected = if base_timed_out || [0, 126, 127].contains(&base_exit) {
                            TestFixJudgement::NotReproduced
                        } else if head_exit == 0 && !head_timed_out {
                            TestFixJudgement::Accept
                        } else {
                            TestFixJudgement::NeedsFix
                        };
                        assert_eq!(
                            judge_test_fix(&ReproPair {
                                base_exit,
                                base_timed_out,
                                head_exit,
                                head_timed_out
                            }),
                            expected
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn test_fix_settled_results_route_to_fix_gate_fix_or_the_miss_rule() {
        let config = config();
        let mut context = ctx(&config);
        context.bug_test_defect = true;
        let progress = BugProgress {
            fix_round: 1,
            test_fix_open: true,
            test_fixes: 1,
            ..BugProgress::default()
        };
        let locate = run(
            &active(Node::Locate),
            &ended(BugStatus::Located, BugReview::Pending),
            &context,
            progress,
        )
        .unwrap();
        assert_eq!(locate.next, active(Node::ReproCheck));
        for (base_exit, base_timed_out, head_exit, head_timed_out, expected) in [
            (1, false, 0, false, Node::FixGate),
            (1, false, 1, false, Node::Fix),
            (1, false, 0, true, Node::Fix),
            (0, false, 0, false, Node::Locate),
            (126, false, 0, false, Node::Locate),
            (127, false, 0, false, Node::Locate),
            (1, true, 0, false, Node::Locate),
        ] {
            let pair = ReproPair {
                base_exit,
                base_timed_out,
                head_exit,
                head_timed_out,
            };
            let mut trigger = repro(1, false);
            if let Trigger::ReproDone { paired, .. } = &mut trigger {
                *paired = Some(pair);
            }
            let t = run(&active(Node::ReproCheck), &trigger, &context, progress).unwrap();
            assert_eq!(t.next, active(expected));
            if expected == Node::Locate {
                assert_eq!(t.test_fix_event, None);
                let why = if base_timed_out {
                    "timed_out"
                } else if base_exit == 0 {
                    "exited_zero"
                } else {
                    "not_runnable"
                };
                assert!(
                    matches!(&t.action, Action::StartBugSession { step: BugStep::Locate, inject: Some(Inject::ReproNotFailing { exit_code, why: actual, .. }) } if *exit_code == base_exit && actual == why)
                );
                let second = run(
                    &active(Node::ReproCheck),
                    &trigger,
                    &context,
                    BugProgress {
                        repro_misses: 1,
                        ..progress
                    },
                )
                .unwrap();
                assert_eq!(second.next, active(Node::AwaitInfo));
                assert_eq!(second.test_fix_event, None);
            } else {
                assert_eq!(
                    t.test_fix_event,
                    Some(TestFixEvent::Settled {
                        result: judge_test_fix(&pair),
                        pair
                    })
                );
                if expected == Node::Fix {
                    assert_eq!(t.budget_n, Some(3));
                    assert!(matches!(
                        t.action,
                        Action::StartBugSession {
                            step: BugStep::Fix,
                            inject: Some(Inject::GateFailed { .. })
                        }
                    ));
                    let spent = run(
                        &active(Node::ReproCheck),
                        &trigger,
                        &context,
                        BugProgress {
                            fix_round: 3,
                            ..progress
                        },
                    )
                    .unwrap();
                    assert!(matches!(
                        spent.next,
                        TaskState::Failed {
                            reason: FailureReason::FixBudget { limit: 3 },
                            ..
                        }
                    ));
                } else {
                    assert_eq!(
                        t.action,
                        Action::RunCoreStep {
                            node: Node::FixGate
                        }
                    );
                    let gate_t =
                        run(&t.next, &gate(Node::FixGate, true), &context, progress).unwrap();
                    assert_eq!(gate_t.next, active(Node::FixReview));
                }
            }
            // A pair does not activate the path for legacy tasks or closed repair mode.
            assert_eq!(
                run(&active(Node::ReproCheck), &trigger, &ctx(&config), progress)
                    .unwrap()
                    .next,
                active(Node::Fix)
            );
            assert_eq!(
                run(
                    &active(Node::ReproCheck),
                    &trigger,
                    &context,
                    BugProgress::default()
                )
                .unwrap()
                .next,
                active(Node::Fix)
            );
        }
        let missing = run(
            &active(Node::ReproCheck),
            &repro(1, false),
            &context,
            progress,
        )
        .unwrap();
        assert!(matches!(
            missing.next,
            TaskState::Failed {
                reason: FailureReason::Protocol { .. },
                ..
            }
        ));
        let mut old = serde_json::to_value(repro(1, false)).unwrap();
        old.as_object_mut().unwrap().remove("paired");
        assert_eq!(
            serde_json::from_value::<Trigger>(old).unwrap(),
            repro(1, false)
        );
    }
}
