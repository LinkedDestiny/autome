//! Task per plan §5.1's Task shape and §6.1's Project/Task relationship.
//!
//! "没有 active + initialized + identity current + intent approved 的
//! Project，Core 拒绝创建或启动 Task" is enforced by requiring
//! `ProjectState::can_start_task()` before `create_task` will construct
//! anything. Reaching `ProjectPhase::Ready` already implies the project
//! passed through `ResolvingIntent` and `CheckingEnvironmentAndSkills`
//! (plan §6.1's nominal path), so this module does not track a second,
//! separately-invented "intent approved" flag that would just duplicate
//! what the phase already proves.
//!
//! `status_projection` is literally the active Run's `RunState`
//! (phase/hold/terminal) — not a separately invented Task-facing enum —
//! per "status_projection 由 active Run 的 phase/hold/terminal 派生".
//! Task lifecycle only changes on a Completed Run terminal or an explicit
//! user cancellation: a Superseded Run leaves the Task Active (it only
//! needs `active_run_ref` repointed at the new Run, which is the caller's
//! job once it has the new Run's id — this module only encodes the
//! lifecycle rule itself, not the repoint).

use serde::{Deserialize, Serialize};

use crate::project::ProjectState;
use crate::run::{RunState, RunTerminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskLifecycle {
    Draft,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DispatchState {
    Queued,
    Running,
    Waiting,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub enqueued_event_seq: u64,
    pub projected_position: u32,
    pub blocked_by_task_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub project_revision: u32,
    pub original_request_ref: String,
    pub latest_contract_ref: Option<String>,
    pub active_run_ref: Option<String>,
    pub lifecycle: TaskLifecycle,
    pub status_projection: RunState,
    pub dispatch_state: DispatchState,
    pub queue_entry: Option<QueueEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateTaskError {
    ProjectNotReadyToStartTask,
    MissingOriginalRequestRef,
}

/// Sole constructor for `Task`. Always starts `Draft`, with no active run
/// yet (`status_projection` is a freshly `Received` `RunState`, matching
/// the fact that no Run has been created for this Task at this point).
pub fn create_task(
    id: &str,
    project: &ProjectState,
    project_id: &str,
    project_revision: u32,
    original_request_ref: &str,
) -> Result<Task, CreateTaskError> {
    if !project.can_start_task() {
        return Err(CreateTaskError::ProjectNotReadyToStartTask);
    }
    if original_request_ref.trim().is_empty() {
        return Err(CreateTaskError::MissingOriginalRequestRef);
    }

    Ok(Task {
        id: id.to_string(),
        project_id: project_id.to_string(),
        project_revision,
        original_request_ref: original_request_ref.to_string(),
        latest_contract_ref: None,
        active_run_ref: None,
        lifecycle: TaskLifecycle::Draft,
        status_projection: RunState::received(),
        dispatch_state: DispatchState::None,
        queue_entry: None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskLifecycleError {
    TaskAlreadyTerminal,
}

/// The only way to move a Task to `Cancelled`. Refuses to "cancel" a Task
/// that is already `Completed` or `Cancelled` rather than silently
/// re-asserting a terminal state.
pub fn cancel_task(mut task: Task) -> Result<Task, TaskLifecycleError> {
    if matches!(
        task.lifecycle,
        TaskLifecycle::Completed | TaskLifecycle::Cancelled
    ) {
        return Err(TaskLifecycleError::TaskAlreadyTerminal);
    }
    task.lifecycle = TaskLifecycle::Cancelled;
    Ok(task)
}

/// Applies a Run's terminal outcome to its Task. Only `Completed` changes
/// Task lifecycle (to `Completed`); every other terminal — including
/// `Superseded` — leaves it as-is, per the plan's explicit carve-out for
/// Superseded runs. A no-op once the Task is already terminal, so a stray
/// late-arriving `Completed` cannot resurrect an already-Cancelled Task.
pub fn apply_run_terminal_to_task(mut task: Task, run_terminal: RunTerminal) -> Task {
    let already_terminal = matches!(
        task.lifecycle,
        TaskLifecycle::Completed | TaskLifecycle::Cancelled
    );
    if run_terminal == RunTerminal::Completed && !already_terminal {
        task.lifecycle = TaskLifecycle::Completed;
    }
    task
}

/// Refreshes `status_projection` from the active Run's current state.
/// Pure projection — does not touch `lifecycle` (`apply_run_terminal_to_task`
/// is the only thing that does).
pub fn project_run_state(mut task: Task, run_state: RunState) -> Task {
    task.status_projection = run_state;
    task
}

/// Every event this module's event-sourced Task aggregate can replay.
/// `apply` below is a thin dispatcher onto the pure functions already
/// defined above (`create_task`/`cancel_task`/`apply_run_terminal_to_task`/
/// `project_run_state`) — it adds no new business rules, only the same
/// Event+apply envelope `run.rs`/`project.rs` already have, so `automed`'s
/// SQLite journal (§11.1) can extend to a third aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskEvent {
    Created {
        id: String,
        project: ProjectState,
        project_id: String,
        project_revision: u32,
        original_request_ref: String,
    },
    Cancelled,
    RunTerminalApplied {
        run_terminal: RunTerminal,
    },
    RunStateProjected {
        run_state: RunState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEventError {
    AlreadyCreated,
    NotYetCreated,
    Create(CreateTaskError),
    Cancel(TaskLifecycleError),
}

/// `state = None` means the aggregate has never been created — mirrors
/// `automed::store` treating "no projection row" as "not created yet"
/// rather than inventing a separate empty-Task representation. Matches on
/// `event` first (not on `(state, event)` jointly) so adding a TaskEvent
/// variant without handling it here is still a compile error, the same
/// guarantee `run::apply`/`project::apply` rely on.
pub fn apply(state: Option<Task>, event: TaskEvent) -> Result<Task, TaskEventError> {
    match event {
        TaskEvent::Created {
            id,
            project,
            project_id,
            project_revision,
            original_request_ref,
        } => {
            if state.is_some() {
                return Err(TaskEventError::AlreadyCreated);
            }
            create_task(
                &id,
                &project,
                &project_id,
                project_revision,
                &original_request_ref,
            )
            .map_err(TaskEventError::Create)
        }
        TaskEvent::Cancelled => {
            let task = state.ok_or(TaskEventError::NotYetCreated)?;
            cancel_task(task).map_err(TaskEventError::Cancel)
        }
        TaskEvent::RunTerminalApplied { run_terminal } => {
            let task = state.ok_or(TaskEventError::NotYetCreated)?;
            Ok(apply_run_terminal_to_task(task, run_terminal))
        }
        TaskEvent::RunStateProjected { run_state } => {
            let task = state.ok_or(TaskEventError::NotYetCreated)?;
            Ok(project_run_state(task, run_state))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{ProjectHold, ProjectLifecycle, ProjectPhase};
    use crate::run::RunPhase;

    fn ready_project() -> ProjectState {
        ProjectState {
            lifecycle: ProjectLifecycle::Active,
            phase: ProjectPhase::Ready,
            hold: ProjectHold::None,
            revision: 1,
        }
    }

    fn not_ready_project() -> ProjectState {
        ProjectState {
            lifecycle: ProjectLifecycle::Active,
            phase: ProjectPhase::ResolvingIntent,
            hold: ProjectHold::None,
            revision: 1,
        }
    }

    #[test]
    fn task_is_created_from_a_ready_project() {
        let task = create_task(
            "task-1",
            &ready_project(),
            "project-1",
            1,
            "original-request-ref-1",
        )
        .unwrap();
        assert_eq!(task.lifecycle, TaskLifecycle::Draft);
        assert_eq!(task.status_projection.phase, RunPhase::Received);
        assert!(task.active_run_ref.is_none());
    }

    #[test]
    fn task_creation_is_rejected_for_a_project_not_ready() {
        let err = create_task(
            "task-1",
            &not_ready_project(),
            "project-1",
            1,
            "original-request-ref-1",
        )
        .unwrap_err();
        assert_eq!(err, CreateTaskError::ProjectNotReadyToStartTask);
    }

    #[test]
    fn task_creation_is_rejected_for_a_project_on_hold() {
        let mut project = ready_project();
        project.hold = ProjectHold::EnvironmentBlocked;
        let err =
            create_task("task-1", &project, "project-1", 1, "original-request-ref-1").unwrap_err();
        assert_eq!(err, CreateTaskError::ProjectNotReadyToStartTask);
    }

    #[test]
    fn task_creation_rejects_empty_original_request_ref() {
        let err = create_task("task-1", &ready_project(), "project-1", 1, "").unwrap_err();
        assert_eq!(err, CreateTaskError::MissingOriginalRequestRef);
    }

    fn active_task() -> Task {
        let mut task = create_task(
            "task-1",
            &ready_project(),
            "project-1",
            1,
            "original-request-ref-1",
        )
        .unwrap();
        task.lifecycle = TaskLifecycle::Active;
        task
    }

    #[test]
    fn active_task_can_be_cancelled() {
        let cancelled = cancel_task(active_task()).unwrap();
        assert_eq!(cancelled.lifecycle, TaskLifecycle::Cancelled);
    }

    #[test]
    fn completed_task_cannot_be_cancelled() {
        let mut task = active_task();
        task.lifecycle = TaskLifecycle::Completed;
        let err = cancel_task(task).unwrap_err();
        assert_eq!(err, TaskLifecycleError::TaskAlreadyTerminal);
    }

    #[test]
    fn completed_run_terminal_completes_the_task() {
        let task = apply_run_terminal_to_task(active_task(), RunTerminal::Completed);
        assert_eq!(task.lifecycle, TaskLifecycle::Completed);
    }

    #[test]
    fn superseded_run_terminal_leaves_task_active() {
        let task = apply_run_terminal_to_task(active_task(), RunTerminal::Superseded);
        assert_eq!(task.lifecycle, TaskLifecycle::Active);
    }

    #[test]
    fn other_run_terminals_also_leave_task_active() {
        let task = apply_run_terminal_to_task(active_task(), RunTerminal::ProtocolFailed);
        assert_eq!(task.lifecycle, TaskLifecycle::Active);
        let task = apply_run_terminal_to_task(active_task(), RunTerminal::Infeasible);
        assert_eq!(task.lifecycle, TaskLifecycle::Active);
    }

    #[test]
    fn a_stray_completed_terminal_cannot_resurrect_a_cancelled_task() {
        let cancelled = cancel_task(active_task()).unwrap();
        let task = apply_run_terminal_to_task(cancelled, RunTerminal::Completed);
        assert_eq!(task.lifecycle, TaskLifecycle::Cancelled);
    }

    #[test]
    fn status_projection_is_refreshed_from_run_state() {
        let task = project_run_state(
            active_task(),
            RunState {
                phase: RunPhase::Executing,
                hold: crate::run::RunHold::None,
                terminal: crate::run::RunTerminal::None,
            },
        );
        assert_eq!(task.status_projection.phase, RunPhase::Executing);
    }

    fn created_event() -> TaskEvent {
        TaskEvent::Created {
            id: "task-1".to_string(),
            project: ready_project(),
            project_id: "project-1".to_string(),
            project_revision: 1,
            original_request_ref: "original-request-ref-1".to_string(),
        }
    }

    #[test]
    fn apply_created_on_none_state_constructs_a_draft_task() {
        let task = apply(None, created_event()).unwrap();
        assert_eq!(task.id, "task-1");
        assert_eq!(task.lifecycle, TaskLifecycle::Draft);
    }

    #[test]
    fn apply_created_on_an_existing_task_is_rejected() {
        let task = apply(None, created_event()).unwrap();
        let err = apply(Some(task), created_event()).unwrap_err();
        assert_eq!(err, TaskEventError::AlreadyCreated);
    }

    #[test]
    fn apply_created_forwards_create_task_validation_errors() {
        let mut event = created_event();
        if let TaskEvent::Created {
            original_request_ref,
            ..
        } = &mut event
        {
            *original_request_ref = String::new();
        }
        let err = apply(None, event).unwrap_err();
        assert_eq!(
            err,
            TaskEventError::Create(CreateTaskError::MissingOriginalRequestRef)
        );
    }

    #[test]
    fn apply_any_event_other_than_created_on_none_state_is_rejected() {
        for event in [
            TaskEvent::Cancelled,
            TaskEvent::RunTerminalApplied {
                run_terminal: RunTerminal::Completed,
            },
            TaskEvent::RunStateProjected {
                run_state: RunState::received(),
            },
        ] {
            let err = apply(None, event).unwrap_err();
            assert_eq!(err, TaskEventError::NotYetCreated);
        }
    }

    #[test]
    fn apply_cancelled_cancels_a_draft_task() {
        let task = apply(None, created_event()).unwrap();
        let task = apply(Some(task), TaskEvent::Cancelled).unwrap();
        assert_eq!(task.lifecycle, TaskLifecycle::Cancelled);
    }

    #[test]
    fn apply_cancelled_on_a_completed_task_forwards_the_lifecycle_error() {
        let task = apply(None, created_event()).unwrap();
        let task = apply_run_terminal_to_task(
            {
                let mut t = task;
                t.lifecycle = TaskLifecycle::Active;
                t
            },
            RunTerminal::Completed,
        );
        let err = apply(Some(task), TaskEvent::Cancelled).unwrap_err();
        assert_eq!(
            err,
            TaskEventError::Cancel(TaskLifecycleError::TaskAlreadyTerminal)
        );
    }

    #[test]
    fn apply_run_terminal_applied_completes_an_active_task() {
        let task = apply(None, created_event()).unwrap();
        let task = apply(
            Some(Task {
                lifecycle: TaskLifecycle::Active,
                ..task
            }),
            TaskEvent::RunTerminalApplied {
                run_terminal: RunTerminal::Completed,
            },
        )
        .unwrap();
        assert_eq!(task.lifecycle, TaskLifecycle::Completed);
    }

    #[test]
    fn apply_run_state_projected_refreshes_status_projection() {
        let task = apply(None, created_event()).unwrap();
        let task = apply(
            Some(task),
            TaskEvent::RunStateProjected {
                run_state: RunState {
                    phase: RunPhase::Executing,
                    hold: crate::run::RunHold::None,
                    terminal: RunTerminal::None,
                },
            },
        )
        .unwrap();
        assert_eq!(task.status_projection.phase, RunPhase::Executing);
    }
}
