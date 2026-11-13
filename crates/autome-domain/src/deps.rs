//! Dependency validation and queue ordering, independent of persistence.
use std::collections::BTreeSet;

use crate::task::{Node, TaskKind, TaskState};

pub const MAX_AFTER: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepError(pub String);
impl std::fmt::Display for DepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DepError {}

pub fn check_after(
    task_id: &str,
    project_id: &str,
    proposed: &[String],
    lookup: impl Fn(&str) -> Option<(String, Vec<String>)>,
) -> Result<Vec<String>, DepError> {
    if proposed.len() > MAX_AFTER {
        return Err(DepError(format!(
            "依赖最多 8 个，给了 {} 个",
            proposed.len()
        )));
    }
    let mut seen = BTreeSet::new();
    let after: Vec<_> = proposed
        .iter()
        .filter(|id| seen.insert(*id))
        .cloned()
        .collect();
    for id in &after {
        if id == task_id {
            return Err(DepError(format!("任务不能依赖自己（{task_id}）")));
        }
        let Some((project, _)) = lookup(id) else {
            return Err(DepError(format!("依赖的任务 {id} 不存在")));
        };
        if project != project_id {
            return Err(DepError(format!(
                "依赖的任务 {id} 属于另一个项目，依赖只能指向同一项目的任务"
            )));
        }
    }
    fn visit(
        id: &str,
        target: &str,
        lookup: &impl Fn(&str) -> Option<(String, Vec<String>)>,
        seen: &mut BTreeSet<String>,
        path: &mut Vec<String>,
    ) -> bool {
        path.push(id.into());
        if id == target {
            return true;
        }
        if seen.insert(id.into())
            && let Some((_, edges)) = lookup(id)
        {
            for edge in edges {
                if visit(&edge, target, lookup, seen, path) {
                    return true;
                }
            }
        }
        path.pop();
        false
    }
    for id in &after {
        let mut path = vec![task_id.into()];
        if visit(id, task_id, &lookup, &mut BTreeSet::new(), &mut path) {
            return Err(DepError(format!(
                "这样设置会形成依赖环：{}",
                path.join(" → ")
            )));
        }
    }
    Ok(after)
}

pub fn check_editable(
    id: &str,
    kind: TaskKind,
    state: &TaskState,
    impl_started: bool,
) -> Result<(), DepError> {
    let implemented = || {
        DepError(format!(
            "{id} 已经开始实现轮（实现），依赖只能在设计批准之前修改"
        ))
    };
    if kind == TaskKind::Feature && impl_started {
        return Err(implemented());
    }
    if matches!(state, TaskState::Done | TaskState::Cancelled) {
        let label = if *state == TaskState::Done {
            "已完成"
        } else {
            "已取消"
        };
        return Err(DepError(format!("{id} 已经结束（{label}），不能再改依赖")));
    }
    if matches!(state, TaskState::Draft | TaskState::Queued) {
        return Ok(());
    }
    if kind == TaskKind::Bug {
        return Err(DepError(format!(
            "Bug 任务 {id} 已经开始，依赖只能在开始之前修改"
        )));
    }
    let node = match state {
        TaskState::Requeued { resume } | TaskState::Paused { resume } => *resume,
        TaskState::Active { node } => *node,
        TaskState::Stopped { at } | TaskState::Failed { at, .. } => *at,
        _ => unreachable!(),
    };
    if matches!(
        node,
        Node::Intake | Node::Design | Node::Review | Node::Adjudicate | Node::AwaitDesignApproval
    ) {
        Ok(())
    } else {
        Err(implemented())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    Cancelled,
    Failed,
}
impl BlockReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}
pub fn block_reason(state: &TaskState) -> Option<BlockReason> {
    match state {
        TaskState::Cancelled => Some(BlockReason::Cancelled),
        TaskState::Failed { .. } => Some(BlockReason::Failed),
        _ => None,
    }
}
pub fn is_satisfied(state: &TaskState) -> bool {
    *state == TaskState::Done
}

pub enum Placement<'a> {
    Top,
    Before(&'a str),
    After(&'a str),
}
pub fn reposition(
    order: &[String],
    task: &str,
    placement: Placement<'_>,
) -> Result<Vec<String>, DepError> {
    let missing = |id| {
        DepError(format!(
            "{id} 不在队列里：只能参照同项目排队中或待开始的任务"
        ))
    };
    if !order.iter().any(|id| id == task) {
        return Err(missing(task));
    }
    let mut result: Vec<_> = order.iter().filter(|id| *id != task).cloned().collect();
    let index = match placement {
        Placement::Top => 0,
        Placement::Before(id) | Placement::After(id) => {
            if id == task {
                return Err(DepError("不能以任务自己为参照移动".into()));
            }
            let index = result
                .iter()
                .position(|item| item == id)
                .ok_or_else(|| missing(id))?;
            index + usize::from(matches!(placement, Placement::After(_)))
        }
    };
    result.insert(index, task.into());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::FailureReason;
    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).into()).collect()
    }
    fn check(values: &[&str], edges: &[(&str, &str, &[&str])]) -> Result<Vec<String>, DepError> {
        check_after("T-1", "p", &ids(values), |id| {
            edges
                .iter()
                .find(|(key, _, _)| *key == id)
                .map(|(_, p, a)| ((*p).into(), ids(a)))
        })
    }
    fn wrappers(node: Node) -> Vec<TaskState> {
        vec![
            TaskState::Active { node },
            TaskState::Requeued { resume: node },
            TaskState::Paused { resume: node },
            TaskState::Stopped { at: node },
            TaskState::Failed {
                at: node,
                reason: FailureReason::Protocol {
                    detail: "failure".into(),
                },
            },
        ]
    }
    #[test]
    fn self_reference() {
        assert_eq!(
            check(&["T-1"], &[]).unwrap_err().0,
            "任务不能依赖自己（T-1）"
        );
    }
    #[test]
    fn missing() {
        assert_eq!(
            check(&["T-99"], &[]).unwrap_err().0,
            "依赖的任务 T-99 不存在"
        );
        assert!(check(&[""], &[]).is_err());
    }
    #[test]
    fn cross_project() {
        assert_eq!(
            check(&["T-2"], &[("T-2", "q", &[])]).unwrap_err().0,
            "依赖的任务 T-2 属于另一个项目，依赖只能指向同一项目的任务"
        );
    }
    #[test]
    fn direct_cycle() {
        assert_eq!(
            check(&["T-2"], &[("T-2", "p", &["T-1"])]).unwrap_err().0,
            "这样设置会形成依赖环：T-1 → T-2 → T-1"
        );
    }
    #[test]
    fn three_node_cycle() {
        assert_eq!(
            check(&["T-2"], &[("T-2", "p", &["T-3"]), ("T-3", "p", &["T-1"])])
                .unwrap_err()
                .0,
            "这样设置会形成依赖环：T-1 → T-2 → T-3 → T-1"
        );
    }
    #[test]
    fn input_limit() {
        assert_eq!(
            check(&["T-2"; 9], &[]).unwrap_err().0,
            "依赖最多 8 个，给了 9 个"
        );
        let values: Vec<String> = (2..=10).map(|n| format!("T-{n}")).collect();
        assert!(check_after("T-1", "p", &values, |_| Some(("p".into(), vec![]))).is_err());
        assert_eq!(
            check_after("T-1", "p", &values[..8], |_| Some(("p".into(), vec![])))
                .unwrap()
                .len(),
            8
        );
    }
    #[test]
    fn deduplicates_in_order_without_state_restrictions() {
        // Lookup deliberately has no state: Done, Cancelled and Draft are equally valid targets.
        assert_eq!(
            check(
                &["T-3", "T-2", "T-3"],
                &[("T-2", "p", &[]), ("T-3", "p", &[])]
            )
            .unwrap(),
            ids(&["T-3", "T-2"])
        );
        assert!(check(&[], &[]).unwrap().is_empty());
        assert_eq!(
            check(&["T-02"], &[("T-02", "p", &[])]).unwrap(),
            ids(&["T-02"])
        );
        assert!(check(&["T-2"], &[("T-2", "p", &["T-3"]), ("T-3", "p", &["T-2"])]).is_ok());
    }
    #[test]
    fn rejection_messages_are_single_line_chinese() {
        let order = ids(&["T-1", "T-2"]);
        let errors = vec![
            check(&["T-1"], &[]).unwrap_err(),
            check(&["T-9"], &[]).unwrap_err(),
            check(&["T-2"], &[("T-2", "q", &[])]).unwrap_err(),
            check(&["T-2"], &[("T-2", "p", &["T-1"])]).unwrap_err(),
            check(&["x"; 9], &[]).unwrap_err(),
            check_editable("T-1", TaskKind::Feature, &TaskState::Queued, true).unwrap_err(),
            check_editable(
                "T-1",
                TaskKind::Bug,
                &TaskState::Active { node: Node::Locate },
                false,
            )
            .unwrap_err(),
            check_editable("T-1", TaskKind::Feature, &TaskState::Cancelled, false).unwrap_err(),
            reposition(&order, "T-1", Placement::Before("T-1")).unwrap_err(),
            reposition(&order, "T-1", Placement::Before("T-9")).unwrap_err(),
        ];
        for error in errors {
            assert!(!error.0.contains(['\r', '\n']));
            assert!(
                error
                    .0
                    .chars()
                    .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
            );
        }
    }
    #[test]
    fn feature_editable_state_table() {
        for state in [
            TaskState::Draft,
            TaskState::Queued,
            TaskState::Done,
            TaskState::Cancelled,
        ] {
            assert_eq!(
                check_editable("T-1", TaskKind::Feature, &state, false).is_ok(),
                matches!(state, TaskState::Draft | TaskState::Queued)
            );
            assert!(check_editable("T-1", TaskKind::Feature, &state, true).is_err());
        }
        for (index, node) in Node::ALL.into_iter().enumerate() {
            for state in wrappers(node) {
                assert_eq!(
                    check_editable("T-1", TaskKind::Feature, &state, false).is_ok(),
                    index < 5,
                    "{state:?}"
                );
                assert!(
                    check_editable("T-1", TaskKind::Feature, &state, true).is_err(),
                    "{state:?}"
                );
            }
        }
    }
    #[test]
    fn bug_editable_state_table() {
        for state in [TaskState::Draft, TaskState::Queued] {
            assert!(check_editable("T-1", TaskKind::Bug, &state, false).is_ok());
        }
        for node in Node::ALL.into_iter().chain(Node::BUG) {
            for state in wrappers(node) {
                assert!(check_editable("T-1", TaskKind::Bug, &state, false).is_err());
            }
        }
        for state in [TaskState::Done, TaskState::Cancelled] {
            assert!(check_editable("T-1", TaskKind::Bug, &state, false).is_err());
        }
    }
    #[test]
    fn blocking_and_satisfaction() {
        let mut states = vec![
            TaskState::Draft,
            TaskState::Queued,
            TaskState::Done,
            TaskState::Cancelled,
        ];
        for node in Node::ALL.into_iter().chain(Node::BUG) {
            states.extend(wrappers(node));
        }
        for state in states {
            assert_eq!(is_satisfied(&state), state == TaskState::Done);
            let expected = match state {
                TaskState::Cancelled => Some("cancelled"),
                TaskState::Failed { .. } => Some("failed"),
                _ => None,
            };
            assert_eq!(block_reason(&state).map(BlockReason::as_str), expected);
        }
    }
    #[test]
    fn reposition_variants_and_rejections() {
        let order = ids(&["a", "b", "c"]);
        for placement in [Placement::Top, Placement::Before("a")] {
            assert_eq!(
                reposition(&order, "c", placement).unwrap(),
                ids(&["c", "a", "b"])
            );
        }
        assert_eq!(
            reposition(&order, "a", Placement::After("c")).unwrap(),
            ids(&["b", "c", "a"])
        );
        assert_eq!(
            reposition(&order, "c", Placement::After("a")).unwrap(),
            ids(&["a", "c", "b"])
        );
        for placement in [
            Placement::Before("a"),
            Placement::After("a"),
            Placement::Before("missing"),
            Placement::After("missing"),
        ] {
            assert!(reposition(&order, "a", placement).is_err());
        }
        assert_eq!(
            reposition(&ids(&["a"]), "a", Placement::Top).unwrap(),
            ids(&["a"])
        );
        assert!(reposition(&[], "a", Placement::Top).is_err());
        assert_eq!(order, ids(&["a", "b", "c"]));
    }
}
