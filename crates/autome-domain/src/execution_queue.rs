//! `ExecutionQueue`/`HarnessLease` per plan §6.2 (line 112, 436).
//!
//! "全局一次只允许一个可运行 Task 持有 HarnessLease；其它可运行 Task 进入
//! 持久化 FIFO ExecutionQueue，按 enqueued_event_seq + task_id 确定顺序":
//! this module is that global, single-instance scheduler. It produces the
//! `task::DispatchState`/`task::QueueEntry` values that the caller then
//! projects onto each Task via `task::project_dispatch_state` — it does not
//! own or mutate `Task` itself, only the fleet-wide queue/lease state.
//!
//! This module only ever reports `DispatchState::Queued`, `Running` or
//! `None`. `Waiting` is deliberately out of scope here: it means a Task is
//! not yet eligible to even enter this queue (e.g. a TaskGraph dependency
//! hasn't cleared), which this module has no visibility into — assigning
//! that state is the caller's job, one layer up.
//!
//! Releasing the held lease while a Task is being Paused/Blocked requires a
//! `SafeParkReceipt` (`safe_park.rs`) whose guards are all true; the
//! receipt must also name the released lease by id, so a receipt minted for
//! one lease can't be replayed to release another. Cancellation never goes
//! through SafePark — the Task is leaving the fleet outright, not pausing
//! to resume later — and is idempotent: cancelling a Task already absent
//! from both the queue and the lease is a no-op, so it can never leave a
//! dangling lease behind. Resuming a parked Task re-enters via `enqueue`
//! again with a fresh `enqueued_event_seq` ("以新 enqueue event 回到队尾"),
//! which places it at the tail, never back at the head.

use serde::{Deserialize, Serialize};

use crate::safe_park::SafeParkReceipt;
use crate::task::{DispatchState, QueueEntry};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessLease {
    pub lease_id: String,
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExecutionQueue {
    /// Kept sorted by `(enqueued_event_seq, task_id)` at all times, so the
    /// front of the vec is always the FIFO head.
    entries: Vec<(String, u64)>,
    lease: Option<HarnessLease>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionQueueError {
    AlreadyQueuedOrLeased { task_id: String },
    LeaseAlreadyHeld { held_by: String },
    NotAtHeadOfQueue { task_id: String, head: String },
    TaskNotQueued { task_id: String },
    NoLeaseHeld,
    SafeParkGuardsNotSatisfied,
    ReceiptDoesNotReferenceThisLease,
}

impl ExecutionQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lease(&self) -> Option<&HarnessLease> {
        self.lease.as_ref()
    }

    /// Also used for "resume after park": a resumed Task is, from this
    /// queue's point of view, simply becoming runnable again with a new
    /// `enqueued_event_seq` — there is no separate resume mechanism.
    pub fn enqueue(
        &mut self,
        task_id: &str,
        enqueued_event_seq: u64,
    ) -> Result<(), ExecutionQueueError> {
        let already_queued = self.entries.iter().any(|(id, _)| id == task_id);
        let holds_lease = self
            .lease
            .as_ref()
            .is_some_and(|lease| lease.task_id == task_id);
        if already_queued || holds_lease {
            return Err(ExecutionQueueError::AlreadyQueuedOrLeased {
                task_id: task_id.to_string(),
            });
        }
        self.entries.push((task_id.to_string(), enqueued_event_seq));
        self.entries
            .sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        Ok(())
    }

    pub fn try_acquire_lease(
        &mut self,
        task_id: &str,
        lease_id: &str,
    ) -> Result<HarnessLease, ExecutionQueueError> {
        if let Some(existing) = &self.lease {
            return Err(ExecutionQueueError::LeaseAlreadyHeld {
                held_by: existing.task_id.clone(),
            });
        }
        match self.entries.first() {
            Some((head, _)) if head == task_id => {}
            Some((head, _)) => {
                return Err(ExecutionQueueError::NotAtHeadOfQueue {
                    task_id: task_id.to_string(),
                    head: head.clone(),
                });
            }
            None => {
                return Err(ExecutionQueueError::TaskNotQueued {
                    task_id: task_id.to_string(),
                });
            }
        }
        self.entries.remove(0);
        let lease = HarnessLease {
            lease_id: lease_id.to_string(),
            task_id: task_id.to_string(),
        };
        self.lease = Some(lease.clone());
        Ok(lease)
    }

    pub fn release_lease_via_safe_park(
        &mut self,
        receipt: &SafeParkReceipt,
    ) -> Result<HarnessLease, ExecutionQueueError> {
        let lease = self
            .lease
            .as_ref()
            .ok_or(ExecutionQueueError::NoLeaseHeld)?;
        if !receipt.all_guards_satisfied() {
            return Err(ExecutionQueueError::SafeParkGuardsNotSatisfied);
        }
        if !receipt
            .released_leases
            .iter()
            .any(|id| id == &lease.lease_id)
        {
            return Err(ExecutionQueueError::ReceiptDoesNotReferenceThisLease);
        }
        Ok(self.lease.take().expect("checked Some above"))
    }

    /// Idempotent: returns whether anything actually changed, never an
    /// error, since cancelling an already-absent Task must be a safe no-op.
    pub fn cancel(&mut self, task_id: &str) -> bool {
        let had_entry_before = self.entries.len();
        self.entries.retain(|(id, _)| id != task_id);
        let removed_from_queue = self.entries.len() != had_entry_before;
        let released_lease = if self
            .lease
            .as_ref()
            .is_some_and(|lease| lease.task_id == task_id)
        {
            self.lease = None;
            true
        } else {
            false
        };
        removed_from_queue || released_lease
    }

    pub fn dispatch_state_of(&self, task_id: &str) -> DispatchState {
        if self
            .lease
            .as_ref()
            .is_some_and(|lease| lease.task_id == task_id)
        {
            DispatchState::Running
        } else if self.entries.iter().any(|(id, _)| id == task_id) {
            DispatchState::Queued
        } else {
            DispatchState::None
        }
    }

    pub fn queue_entry_of(&self, task_id: &str) -> Option<QueueEntry> {
        let position = self.entries.iter().position(|(id, _)| id == task_id)?;
        let (_, enqueued_event_seq) = self.entries[position];
        let blocked_by_task_id = if position == 0 {
            self.lease.as_ref().map(|lease| lease.task_id.clone())
        } else {
            Some(self.entries[position - 1].0.clone())
        };
        Some(QueueEntry {
            enqueued_event_seq,
            projected_position: position as u32,
            blocked_by_task_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attempt::RunId;
    use crate::run::RunPhase;
    use crate::safe_park::build_safe_park_receipt;

    fn satisfied_receipt(released_leases: Vec<String>) -> SafeParkReceipt {
        build_safe_park_receipt(
            RunId("run-1".to_string()),
            "planning-hash-1",
            None,
            RunPhase::Executing,
            "checkpoint-1",
            None,
            true,
            true,
            true,
            released_leases,
            "2026-09-14T00:00:00Z",
            "digest-1",
        )
        .unwrap()
    }

    #[test]
    fn queue_orders_by_enqueued_event_seq_then_task_id() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-b", 5).unwrap();
        queue.enqueue("task-a", 3).unwrap();
        queue.enqueue("task-c", 3).unwrap();
        assert_eq!(
            queue.queue_entry_of("task-a").unwrap().projected_position,
            0
        );
        assert_eq!(
            queue.queue_entry_of("task-c").unwrap().projected_position,
            1
        );
        assert_eq!(
            queue.queue_entry_of("task-b").unwrap().projected_position,
            2
        );
    }

    #[test]
    fn only_the_head_of_the_queue_can_acquire_the_lease() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.enqueue("task-b", 2).unwrap();
        let err = queue.try_acquire_lease("task-b", "lease-1").unwrap_err();
        assert_eq!(
            err,
            ExecutionQueueError::NotAtHeadOfQueue {
                task_id: "task-b".to_string(),
                head: "task-a".to_string(),
            }
        );
        let lease = queue.try_acquire_lease("task-a", "lease-1").unwrap();
        assert_eq!(lease.task_id, "task-a");
        assert_eq!(queue.dispatch_state_of("task-a"), DispatchState::Running);
        assert_eq!(queue.dispatch_state_of("task-b"), DispatchState::Queued);
    }

    #[test]
    fn a_second_task_cannot_acquire_the_lease_while_it_is_held() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.enqueue("task-b", 2).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();
        let err = queue.try_acquire_lease("task-b", "lease-2").unwrap_err();
        assert_eq!(
            err,
            ExecutionQueueError::LeaseAlreadyHeld {
                held_by: "task-a".to_string(),
            }
        );
    }

    #[test]
    fn blocked_by_task_id_points_at_the_lease_holder_for_the_head_and_the_predecessor_otherwise() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.enqueue("task-b", 2).unwrap();
        queue.enqueue("task-c", 3).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();
        assert_eq!(
            queue.queue_entry_of("task-b").unwrap().blocked_by_task_id,
            Some("task-a".to_string())
        );
        assert_eq!(
            queue.queue_entry_of("task-c").unwrap().blocked_by_task_id,
            Some("task-b".to_string())
        );
    }

    /// `build_safe_park_receipt` never hands back an unsatisfied receipt, but
    /// a receipt round-tripped through SQLite/JSON is just data again by the
    /// time it reaches this queue, so a tampered/stale one with a false
    /// guard must still be rejected here rather than trusted on sight.
    #[test]
    fn releasing_the_lease_requires_a_receipt_with_all_guards_satisfied() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();

        let mut unsatisfied = satisfied_receipt(vec!["lease-1".to_string()]);
        unsatisfied.no_verifier_or_preview_process = false;
        let err = queue.release_lease_via_safe_park(&unsatisfied).unwrap_err();
        assert_eq!(err, ExecutionQueueError::SafeParkGuardsNotSatisfied);
    }

    #[test]
    fn releasing_the_lease_fails_if_the_receipt_does_not_name_this_lease() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();
        let receipt = satisfied_receipt(vec!["some-other-lease".to_string()]);
        let err = queue.release_lease_via_safe_park(&receipt).unwrap_err();
        assert_eq!(err, ExecutionQueueError::ReceiptDoesNotReferenceThisLease);
    }

    #[test]
    fn releasing_the_lease_succeeds_when_the_receipt_names_it_and_all_guards_hold() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();
        let receipt = satisfied_receipt(vec!["lease-1".to_string()]);
        let released = queue.release_lease_via_safe_park(&receipt).unwrap();
        assert_eq!(released.lease_id, "lease-1");
        assert!(queue.lease().is_none());
        assert_eq!(queue.dispatch_state_of("task-a"), DispatchState::None);
    }

    #[test]
    fn releasing_with_no_lease_held_is_an_error() {
        let mut queue = ExecutionQueue::new();
        let receipt = satisfied_receipt(vec!["lease-1".to_string()]);
        let err = queue.release_lease_via_safe_park(&receipt).unwrap_err();
        assert_eq!(err, ExecutionQueueError::NoLeaseHeld);
    }

    #[test]
    fn resuming_after_a_park_re_enters_the_queue_at_the_tail() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();
        let receipt = satisfied_receipt(vec!["lease-1".to_string()]);
        queue.release_lease_via_safe_park(&receipt).unwrap();

        queue.enqueue("task-b", 2).unwrap();
        // task-a resumes with a fresh, later enqueued_event_seq.
        queue.enqueue("task-a", 3).unwrap();

        assert_eq!(
            queue.queue_entry_of("task-b").unwrap().projected_position,
            0
        );
        assert_eq!(
            queue.queue_entry_of("task-a").unwrap().projected_position,
            1
        );
    }

    #[test]
    fn cancelling_a_queued_task_removes_it_and_is_idempotent() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        assert!(queue.cancel("task-a"));
        assert_eq!(queue.dispatch_state_of("task-a"), DispatchState::None);
        assert!(!queue.cancel("task-a"));
    }

    #[test]
    fn cancelling_the_lease_holder_releases_the_lease_without_a_safe_park_receipt() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.enqueue("task-b", 2).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();

        assert!(queue.cancel("task-a"));
        assert!(queue.lease().is_none());
        // task-b, previously blocked by task-a, can now acquire the lease.
        let lease = queue.try_acquire_lease("task-b", "lease-2").unwrap();
        assert_eq!(lease.task_id, "task-b");
    }

    #[test]
    fn enqueueing_a_task_that_already_holds_the_lease_is_rejected() {
        let mut queue = ExecutionQueue::new();
        queue.enqueue("task-a", 1).unwrap();
        queue.try_acquire_lease("task-a", "lease-1").unwrap();
        let err = queue.enqueue("task-a", 2).unwrap_err();
        assert_eq!(
            err,
            ExecutionQueueError::AlreadyQueuedOrLeased {
                task_id: "task-a".to_string(),
            }
        );
    }
}
