//! Pure FIFO admission for host jobs. Event sequence numbers define order.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    #[default]
    Heavy,
    Light,
}

impl Lane {
    pub fn for_kind(kind: &str) -> Self {
        match kind {
            "quick" | "repro" => Self::Light,
            _ => Self::Heavy,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiter {
    pub task_id: String,
    pub seq: i64,
    pub limit: u32,
    pub lane: Lane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    Go,
    Wait { position: usize },
}

/// Input contains at most one effective entry per task; ordering is immaterial.
pub fn position(waiters: &[Waiter], task_id: &str) -> Option<usize> {
    let own = waiters.iter().find(|w| w.task_id == task_id)?;
    Some(
        1 + waiters
            .iter()
            .filter(|w| w.lane == own.lane && w.seq < own.seq)
            .count(),
    )
}

pub fn admission(
    held: usize,
    waiters: &[Waiter],
    task_id: &str,
    limit: u32,
    lane: Lane,
) -> Admission {
    let position = position(waiters, task_id)
        .unwrap_or_else(|| waiters.iter().filter(|w| w.lane == lane).count() + 1);
    if position == 1 && held < limit as usize {
        Admission::Go
    } else {
        Admission::Wait { position }
    }
}

/// Fold events in sequence order. Irrelevant events preserve the current entry.
pub fn queued_after_event(queued: bool, kind: &str) -> bool {
    match kind {
        "gate.queued" => true,
        "gate.started" | "setup.started" | "session.started" | "session.deferred"
        | "task.updated" | "gate.queue_dropped" => false,
        _ => queued,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn waiter(id: &str, seq: i64, limit: u32) -> Waiter {
        Waiter {
            task_id: id.into(),
            seq,
            limit,
            lane: Lane::Heavy,
        }
    }
    #[test]
    fn gate_queue_only_head_with_capacity_goes() {
        assert_eq!(admission(0, &[], "a", 1, Lane::Heavy), Admission::Go);
        let queue = [waiter("a", 1, 2), waiter("b", 2, 2)];
        assert_eq!(admission(1, &queue, "a", 2, Lane::Heavy), Admission::Go);
        assert_eq!(
            admission(0, &queue, "b", 2, Lane::Heavy),
            Admission::Wait { position: 2 }
        );
    }
    #[test]
    fn gate_queue_newcomer_joins_tail() {
        let queue = [waiter("a", 1, 10)];
        assert_eq!(
            admission(0, &queue, "b", 10, Lane::Heavy),
            Admission::Wait { position: 2 }
        );
    }
    #[test]
    fn gate_queue_full_capacity_blocks_everyone() {
        let queue = [waiter("a", 1, 1)];
        for held in [1, 2, usize::MAX] {
            assert_eq!(
                admission(held, &queue, "a", 1, Lane::Heavy),
                Admission::Wait { position: 1 }
            );
            assert_eq!(
                admission(held, &queue, "b", 1, Lane::Heavy),
                Admission::Wait { position: 2 }
            );
            assert_eq!(
                admission(held, &[], "a", 1, Lane::Heavy),
                Admission::Wait { position: 1 }
            );
        }
    }
    #[test]
    fn gate_queue_smaller_head_limit_blocks_larger_followers() {
        let queue = [waiter("a", 1, 1), waiter("b", 2, 10)];
        for w in &queue {
            assert_eq!(
                admission(1, &queue, &w.task_id, w.limit, Lane::Heavy),
                Admission::Wait {
                    position: w.seq as usize
                }
            );
        }
        assert_eq!(admission(0, &queue, "a", 1, Lane::Heavy), Admission::Go);
        assert_eq!(
            admission(1, &queue[1..], "b", 10, Lane::Heavy),
            Admission::Go
        );
    }
    #[test]
    fn gate_queue_position_uses_sequence_and_starts_at_one() {
        let queue = [waiter("b", i64::MAX, 1), waiter("a", 1, 1)];
        assert_eq!(position(&queue, "a"), Some(1));
        assert_eq!(position(&queue, "b"), Some(2));
        assert_eq!(position(&queue, "c"), None);
        assert_eq!(position(&[], "a"), None);
    }
    #[test]
    fn gate_queue_event_transitions() {
        for before in [false, true] {
            assert!(queued_after_event(before, "gate.queued"));
            for kind in [
                "gate.started",
                "setup.started",
                "session.started",
                "session.deferred",
                "task.updated",
                "gate.queue_dropped",
            ] {
                assert!(!queued_after_event(before, kind), "{kind}");
            }
            for kind in ["gate.result", "setup.result", "unknown", ""] {
                assert_eq!(queued_after_event(before, kind), before);
            }
        }
    }
}

#[cfg(test)]
mod lane_tests {
    use super::*;

    #[test]
    fn gate_lane_kind_maps_full_to_heavy_and_quick_repro_to_light() {
        for kind in ["full", "setup", "", "unknown"] {
            assert_eq!(Lane::for_kind(kind), Lane::Heavy);
        }
        for kind in ["quick", "repro"] {
            assert_eq!(Lane::for_kind(kind), Lane::Light);
        }
    }

    fn mixed() -> Vec<Waiter> {
        [
            ("h1", Lane::Heavy),
            ("l1", Lane::Light),
            ("h2", Lane::Heavy),
            ("l2", Lane::Light),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (id, lane))| Waiter {
            task_id: id.into(),
            seq: i as i64,
            limit: 1,
            lane,
        })
        .collect()
    }

    #[test]
    fn gate_lane_position_counts_only_the_same_lane() {
        let queue = mixed();
        for (id, rank) in [("h1", 1), ("l1", 1), ("h2", 2), ("l2", 2)] {
            assert_eq!(position(&queue, id), Some(rank));
        }
        assert_eq!(position(&queue, "missing"), None);
        assert_eq!(position(&[], "missing"), None);
    }

    #[test]
    fn gate_lane_admission_counts_held_per_lane() {
        let queue = mixed();
        for (id, lane) in [("h1", Lane::Heavy), ("l1", Lane::Light)] {
            assert_eq!(admission(0, &queue, id, 1, lane), Admission::Go);
            assert_eq!(
                admission(1, &queue, id, 1, lane),
                Admission::Wait { position: 1 }
            );
            assert_eq!(admission(1, &queue, id, 2, lane), Admission::Go);
            assert_eq!(
                admission(usize::MAX, &queue, id, 10, lane),
                Admission::Wait { position: 1 }
            );
            assert_eq!(
                admission(0, &queue, "new", 10, lane),
                Admission::Wait { position: 3 }
            );
        }
        assert_eq!(
            admission(0, &queue[0..1], "new", 1, Lane::Light),
            Admission::Go
        );
    }
}
