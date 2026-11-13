//! Pure project merge-section and authorization rules.
use crate::task::{Node, TaskState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSource {
    User,
    Behind,
    Project,
}
impl AuthSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Behind => "behind",
            Self::Project => "project",
        }
    }
}

pub fn authorization(
    record: Option<(bool, Option<&str>)>,
    project_auto: bool,
) -> Option<AuthSource> {
    match record {
        Some((true, Some("user"))) => Some(AuthSource::User),
        Some((true, _)) => Some(AuthSource::Behind),
        Some((false, Some("user"))) => None,
        _ => project_auto.then_some(AuthSource::Project),
    }
}
pub fn holds(lock_active: bool, state: &TaskState) -> bool {
    lock_active
        && matches!(
            state,
            TaskState::Active {
                node: Node::MergeGate | Node::Merging
            }
        )
}
pub fn releases(lock_active: bool, next: &TaskState) -> bool {
    lock_active && !holds(true, next)
}

/// Returns the task to wait for. A holder never waits for itself or the queue.
pub fn turn<'a>(
    holder: Option<&'a str>,
    earlier: Option<(&'a str, u64)>,
    authorized: bool,
    task: &str,
    seq: Option<u64>,
) -> Option<&'a str> {
    if let Some(holder) = holder {
        return (holder != task).then_some(holder);
    }
    earlier
        .filter(|(id, order)| authorized && *id != task && seq.is_none_or(|seq| *order < seq))
        .map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merge_queue_effective_authorization_follows_the_ledger_then_the_project() {
        for auto in [false, true] {
            assert_eq!(
                authorization(None, auto),
                auto.then_some(AuthSource::Project)
            );
            assert_eq!(
                authorization(Some((false, None)), auto),
                auto.then_some(AuthSource::Project)
            );
            assert_eq!(authorization(Some((false, Some("user"))), auto), None);
            assert_eq!(
                authorization(Some((true, Some("user"))), auto),
                Some(AuthSource::User)
            );
            for by in [None, Some("behind")] {
                assert_eq!(
                    authorization(Some((true, by)), auto),
                    Some(AuthSource::Behind)
                );
            }
        }
    }
    #[test]
    fn merge_queue_holder_needs_a_lock_and_a_section_node() {
        for node in Node::ALL.into_iter().chain(Node::BUG) {
            let state = TaskState::Active { node };
            assert!(!holds(false, &state));
            assert_eq!(
                holds(true, &state),
                matches!(node, Node::MergeGate | Node::Merging)
            );
        }
        for state in [
            TaskState::Draft,
            TaskState::Queued,
            TaskState::Requeued {
                resume: Node::MergeGate,
            },
            TaskState::Paused {
                resume: Node::MergeGate,
            },
            TaskState::Stopped {
                at: Node::MergeGate,
            },
            TaskState::Done,
            TaskState::Cancelled,
        ] {
            assert!(!holds(true, &state));
        }
    }
    #[test]
    fn merge_queue_leaving_the_section_releases_the_lock() {
        for node in Node::ALL.into_iter().chain(Node::BUG) {
            let state = TaskState::Active { node };
            assert_eq!(
                releases(true, &state),
                !matches!(node, Node::MergeGate | Node::Merging)
            );
            assert!(!releases(false, &state));
        }
    }
    #[test]
    fn merge_queue_turn_follows_the_holder_then_the_queue_order() {
        assert_eq!(turn(None, Some(("A", 0)), true, "B", Some(0)), None);
        assert_eq!(
            turn(None, Some(("A", 0)), true, "B", Some(u64::MAX)),
            Some("A")
        );
        assert_eq!(turn(None, Some(("B", 0)), true, "B", None), None);
        for authorized in [false, true] {
            assert_eq!(turn(Some("A"), None, authorized, "B", None), Some("A"));
            assert_eq!(
                turn(Some("B"), Some(("A", 1)), authorized, "B", Some(2)),
                None
            );
            assert_eq!(
                turn(None, Some(("A", 1)), authorized, "B", None),
                authorized.then_some("A")
            );
            assert_eq!(
                turn(None, Some(("A", 1)), authorized, "B", Some(2)),
                authorized.then_some("A")
            );
            assert_eq!(turn(None, Some(("A", 2)), authorized, "B", Some(1)), None);
            assert_eq!(turn(None, None, authorized, "B", None), None);
        }
    }
}
