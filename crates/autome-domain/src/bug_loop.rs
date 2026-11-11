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
        (TaskState::Failed { .. } | TaskState::Stopped { .. }, Trigger::RerunFrom { node }) => {
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
            Ok(session(step, None, ctx))
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
            let invalid = if *node != Node::Locate && doc.status != BugStatus::Located {
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
            Ok(match node {
                Node::Locate => match doc.status {
                    BugStatus::Located => core(Node::ReproCheck),
                    BugStatus::CannotReproduce => Transition::active(Node::AwaitInfo, Action::None),
                    BugStatus::NotABug => Transition::active(Node::NotABug, Action::None),
                },
                Node::Fix => core(Node::FixGate),
                _ if doc.review == BugReview::Passed => core(Node::Rebase),
                _ => start_fix(*node, Some(Inject::FixRejected), ctx, progress),
            })
        }
        (
            TaskState::Active {
                node: Node::ReproCheck,
            },
            Trigger::ReproDone {
                head,
                command,
                exit_code,
                timed_out,
                tail,
            },
        ) => {
            if !timed_out && *exit_code != 0 && ![126, 127].contains(exit_code) {
                return Ok(start_fix(Node::ReproCheck, None, ctx, progress));
            }
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
                Ok(if ctx.config.is_enabled(Role::Audit) {
                    session(BugStep::FixReview, None, ctx)
                } else {
                    core(Node::Rebase)
                })
            }
            _ => apply_with_merge_request(state, trigger, ctx, merge_requested),
        },
        (
            TaskState::Active { node: Node::Rebase },
            Trigger::CoreStepDone {
                node: Node::Rebase,
                result,
            },
        ) => match result {
            CoreStepResult::Ok => {
                let mut t = core(Node::MergeGate);
                t.merge_requested = merge_requested;
                Ok(t)
            }
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
    fn bug_loop_passed_fix_gate_goes_to_review_or_to_rebase_when_audit_is_off() {
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
            core(Node::Rebase)
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
            assert_eq!(t.merge_requested, requested);
        }
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
                    assert_eq!(
                        apply_for(
                            TaskKind::Bug,
                            &active(node),
                            &trigger,
                            &context,
                            BugProgress::default(),
                            requested
                        ),
                        apply_with_merge_request(&active(node), &trigger, &context, requested)
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
                    assert_eq!(t.next, active(node));
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
}
