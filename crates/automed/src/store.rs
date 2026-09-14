//! SQLite event journal per plan §11.1: "SQLite 是 2.0 控制状态的唯一权威
//! ... append-only events，带单调序号、唯一事件 ID、aggregate revision 与
//! 事务完整性检查." This is the M0 thin slice: only the Run aggregate is
//! journaled so far, but the shape (append-only events + same-transaction
//! projection update) is the pattern every other aggregate in §11.1 will
//! reuse.
//!
//! §11.2 recovery rule ("Rust 重启后只恢复 durable event 之前的状态") is
//! exercised directly by `state_survives_reconnect` below: there is no
//! in-memory cache that could diverge from what was actually committed.

use autome_domain::project::{self, ProjectEvent, ProjectState};
use autome_domain::run::{self, RunEvent, RunState, TransitionError};
use autome_domain::task::{self, Task, TaskEvent, TaskEventError};
use rusqlite::{Connection, OptionalExtension, params};

pub struct EventStore {
    conn: Connection,
}

#[derive(Debug)]
pub enum AppendError {
    Sql(rusqlite::Error),
    Transition(TransitionError),
}

impl From<rusqlite::Error> for AppendError {
    fn from(value: rusqlite::Error) -> Self {
        AppendError::Sql(value)
    }
}

#[derive(Debug)]
pub enum ProjectAppendError {
    Sql(rusqlite::Error),
    Transition(project::TransitionError),
}

impl From<rusqlite::Error> for ProjectAppendError {
    fn from(value: rusqlite::Error) -> Self {
        ProjectAppendError::Sql(value)
    }
}

#[derive(Debug)]
pub enum TaskAppendError {
    Sql(rusqlite::Error),
    Transition(TaskEventError),
}

impl From<rusqlite::Error> for TaskAppendError {
    fn from(value: rusqlite::Error) -> Self {
        TaskAppendError::Sql(value)
    }
}

#[derive(Debug, Clone)]
pub struct AppendedProjectEvent {
    pub seq: i64,
    pub event_id: String,
    pub revision: u64,
    pub event_type: &'static str,
    pub occurred_at: String,
    pub state: ProjectState,
}

/// Mirrors `AppendedRunEvent`/`AppendedProjectEvent`; see the latter's doc
/// comment.
#[derive(Debug, Clone)]
pub struct AppendedTaskEvent {
    pub seq: i64,
    pub event_id: String,
    pub revision: u64,
    pub event_type: &'static str,
    pub occurred_at: String,
    pub state: Task,
}

/// Everything an IPC dispatcher needs to build an outgoing `Event` envelope
/// after a successful append: the globally monotonic `seq` (the events
/// table's own rowid — already unique and ordered across every aggregate),
/// the per-aggregate `revision`, the event's own id/type/timestamp, and the
/// resulting projected state.
#[derive(Debug, Clone)]
pub struct AppendedRunEvent {
    pub seq: i64,
    pub event_id: String,
    pub revision: u64,
    pub event_type: &'static str,
    pub occurred_at: String,
    pub state: RunState,
}

impl EventStore {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                aggregate_id TEXT NOT NULL,
                aggregate_type TEXT NOT NULL,
                revision INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                recorded_at TEXT NOT NULL,
                UNIQUE(aggregate_type, aggregate_id, revision)
            );
            CREATE TABLE IF NOT EXISTS run_projections (
                aggregate_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL,
                phase TEXT NOT NULL,
                hold TEXT NOT NULL,
                terminal TEXT NOT NULL,
                state_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS project_projections (
                aggregate_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL,
                lifecycle TEXT NOT NULL,
                phase TEXT NOT NULL,
                hold TEXT NOT NULL,
                state_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS task_projections (
                aggregate_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL,
                lifecycle TEXT NOT NULL,
                state_json TEXT NOT NULL
            );
            ",
        )
    }

    /// Loads the current projected (revision, RunState) for an aggregate, or
    /// `None` if no event has ever been journaled for it — in which case
    /// callers should treat the aggregate as `RunState::received()` at
    /// revision 0.
    pub fn load_run_state(&self, aggregate_id: &str) -> rusqlite::Result<Option<(u64, RunState)>> {
        self.conn
            .query_row(
                "SELECT revision, state_json FROM run_projections WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| {
                    let revision: i64 = row.get(0)?;
                    let state_json: String = row.get(1)?;
                    Ok((revision, state_json))
                },
            )
            .optional()?
            .map(|(revision, state_json)| {
                let state: RunState = serde_json::from_str(&state_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                Ok((revision as u64, state))
            })
            .transpose()
    }

    /// Applies `event` to the current state of `aggregate_id`, and — in a
    /// single transaction — appends the event and updates the projection.
    /// On an illegal transition, nothing is written: the event never
    /// existed as far as the journal is concerned.
    pub fn append_run_event(
        &mut self,
        aggregate_id: &str,
        event: RunEvent,
    ) -> Result<AppendedRunEvent, AppendError> {
        let tx = self.conn.transaction()?;

        let (revision, current_state) = {
            let loaded = tx
                .query_row(
                    "SELECT revision, state_json FROM run_projections WHERE aggregate_id = ?1",
                    params![aggregate_id],
                    |row| {
                        let revision: i64 = row.get(0)?;
                        let state_json: String = row.get(1)?;
                        Ok((revision, state_json))
                    },
                )
                .optional()?;
            match loaded {
                Some((revision, state_json)) => {
                    let state: RunState = serde_json::from_str(&state_json).expect(
                        "run_projections.state_json is only ever written by this module as valid RunState JSON",
                    );
                    (revision as u64, state)
                }
                None => (0, RunState::received()),
            }
        };

        let next_state = run::apply(current_state, event).map_err(AppendError::Transition)?;
        let next_revision = revision + 1;
        let event_id = uuid::Uuid::new_v4().to_string();
        let event_type = event_type_name(&event);
        let payload = serde_json::to_string(&event).expect("RunEvent always serializes");
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails");
        let state_json = serde_json::to_string(&next_state).expect("RunState always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Run', ?3, ?4, ?5, ?6)",
            params![event_id, aggregate_id, next_revision as i64, event_type, payload, recorded_at],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO run_projections (aggregate_id, revision, phase, hold, terminal, state_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                phase = excluded.phase,
                hold = excluded.hold,
                terminal = excluded.terminal,
                state_json = excluded.state_json",
            params![
                aggregate_id,
                next_revision as i64,
                format!("{:?}", next_state.phase),
                format!("{:?}", next_state.hold),
                format!("{:?}", next_state.terminal),
                state_json
            ],
        )?;

        tx.commit()?;
        Ok(AppendedRunEvent {
            seq,
            event_id,
            revision: next_revision,
            event_type,
            occurred_at: recorded_at,
            state: next_state,
        })
    }

    /// Loads the current projected (revision, ProjectState) for an
    /// aggregate, or `None` if no event has ever been journaled for it — in
    /// which case callers should treat the aggregate as
    /// `ProjectState::new()` at revision 0.
    pub fn load_project_state(
        &self,
        aggregate_id: &str,
    ) -> rusqlite::Result<Option<(u64, ProjectState)>> {
        self.conn
            .query_row(
                "SELECT revision, state_json FROM project_projections WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| {
                    let revision: i64 = row.get(0)?;
                    let state_json: String = row.get(1)?;
                    Ok((revision, state_json))
                },
            )
            .optional()?
            .map(|(revision, state_json)| {
                let state: ProjectState = serde_json::from_str(&state_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                Ok((revision as u64, state))
            })
            .transpose()
    }

    /// Applies `event` to the current state of `aggregate_id`, and — in a
    /// single transaction — appends the event and updates the projection.
    /// On an illegal transition, nothing is written: the event never
    /// existed as far as the journal is concerned. Mirrors
    /// `append_run_event`; see its doc comment for the shape this pattern
    /// generalizes from.
    pub fn append_project_event(
        &mut self,
        aggregate_id: &str,
        event: ProjectEvent,
    ) -> Result<AppendedProjectEvent, ProjectAppendError> {
        let tx = self.conn.transaction()?;

        let (revision, current_state) = {
            let loaded = tx
                .query_row(
                    "SELECT revision, state_json FROM project_projections WHERE aggregate_id = ?1",
                    params![aggregate_id],
                    |row| {
                        let revision: i64 = row.get(0)?;
                        let state_json: String = row.get(1)?;
                        Ok((revision, state_json))
                    },
                )
                .optional()?;
            match loaded {
                Some((revision, state_json)) => {
                    let state: ProjectState = serde_json::from_str(&state_json).expect(
                        "project_projections.state_json is only ever written by this module as valid ProjectState JSON",
                    );
                    (revision as u64, state)
                }
                None => (0, ProjectState::new()),
            }
        };

        let next_state =
            project::apply(current_state, event).map_err(ProjectAppendError::Transition)?;
        let next_revision = revision + 1;
        let event_id = uuid::Uuid::new_v4().to_string();
        let event_type = project_event_type_name(&event);
        let payload = serde_json::to_string(&event).expect("ProjectEvent always serializes");
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails");
        let state_json =
            serde_json::to_string(&next_state).expect("ProjectState always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Project', ?3, ?4, ?5, ?6)",
            params![event_id, aggregate_id, next_revision as i64, event_type, payload, recorded_at],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO project_projections (aggregate_id, revision, lifecycle, phase, hold, state_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                lifecycle = excluded.lifecycle,
                phase = excluded.phase,
                hold = excluded.hold,
                state_json = excluded.state_json",
            params![
                aggregate_id,
                next_revision as i64,
                format!("{:?}", next_state.lifecycle),
                format!("{:?}", next_state.phase),
                format!("{:?}", next_state.hold),
                state_json
            ],
        )?;

        tx.commit()?;
        Ok(AppendedProjectEvent {
            seq,
            event_id,
            revision: next_revision,
            event_type,
            occurred_at: recorded_at,
            state: next_state,
        })
    }

    /// Loads the current projected (revision, Task) for an aggregate, or
    /// `None` if it has never been created — matching `task::apply`'s
    /// `state: Option<Task>` convention (see task.rs).
    pub fn load_task_state(&self, aggregate_id: &str) -> rusqlite::Result<Option<(u64, Task)>> {
        self.conn
            .query_row(
                "SELECT revision, state_json FROM task_projections WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| {
                    let revision: i64 = row.get(0)?;
                    let state_json: String = row.get(1)?;
                    Ok((revision, state_json))
                },
            )
            .optional()?
            .map(|(revision, state_json)| {
                let state: Task = serde_json::from_str(&state_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                Ok((revision as u64, state))
            })
            .transpose()
    }

    /// Applies `event` to the current state of `aggregate_id`, and — in a
    /// single transaction — appends the event and updates the projection.
    /// Mirrors `append_run_event`/`append_project_event`; the only
    /// structural difference is that a never-created Task has no default
    /// state to fall back to (`task::apply` takes `Option<Task>`, not a
    /// bare `Task`), so a missing projection row maps to `None` here
    /// instead of a constructed default.
    pub fn append_task_event(
        &mut self,
        aggregate_id: &str,
        event: TaskEvent,
    ) -> Result<AppendedTaskEvent, TaskAppendError> {
        let tx = self.conn.transaction()?;

        let (revision, current_state) = {
            let loaded = tx
                .query_row(
                    "SELECT revision, state_json FROM task_projections WHERE aggregate_id = ?1",
                    params![aggregate_id],
                    |row| {
                        let revision: i64 = row.get(0)?;
                        let state_json: String = row.get(1)?;
                        Ok((revision, state_json))
                    },
                )
                .optional()?;
            match loaded {
                Some((revision, state_json)) => {
                    let state: Task = serde_json::from_str(&state_json).expect(
                        "task_projections.state_json is only ever written by this module as valid Task JSON",
                    );
                    (revision as u64, Some(state))
                }
                None => (0, None),
            }
        };

        let event_type = task_event_type_name(&event);
        let payload = serde_json::to_string(&event).expect("TaskEvent always serializes");
        let next_state = task::apply(current_state, event).map_err(TaskAppendError::Transition)?;
        let next_revision = revision + 1;
        let event_id = uuid::Uuid::new_v4().to_string();
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails");
        let state_json = serde_json::to_string(&next_state).expect("Task always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Task', ?3, ?4, ?5, ?6)",
            params![event_id, aggregate_id, next_revision as i64, event_type, payload, recorded_at],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO task_projections (aggregate_id, revision, lifecycle, state_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                lifecycle = excluded.lifecycle,
                state_json = excluded.state_json",
            params![
                aggregate_id,
                next_revision as i64,
                format!("{:?}", next_state.lifecycle),
                state_json
            ],
        )?;

        tx.commit()?;
        Ok(AppendedTaskEvent {
            seq,
            event_id,
            revision: next_revision,
            event_type,
            occurred_at: recorded_at,
            state: next_state,
        })
    }

    #[cfg(test)]
    fn event_count(&self, aggregate_id: &str) -> i64 {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| row.get(0),
            )
            .unwrap()
    }
}

fn event_type_name(event: &RunEvent) -> &'static str {
    match event {
        RunEvent::AdvanceNominal => "AdvanceNominal",
        RunEvent::ContractReviewRejected => "ContractReviewRejected",
        RunEvent::GraphReviewRejected { .. } => "GraphReviewRejected",
        RunEvent::ReadinessReady => "ReadinessReady",
        RunEvent::PlanApproved => "PlanApproved",
        RunEvent::ReadinessInvalidated => "ReadinessInvalidated",
        RunEvent::ReadinessConfirmed { .. } => "ReadinessConfirmed",
        RunEvent::ReadinessCapabilityChanged => "ReadinessCapabilityChanged",
        RunEvent::ConfiguredHumanReviewRequired => "ConfiguredHumanReviewRequired",
        RunEvent::ConfiguredHumanReviewPassed => "ConfiguredHumanReviewPassed",
        RunEvent::ConfigDriftDetected => "ConfigDriftDetected",
        RunEvent::RepairRequested => "RepairRequested",
        RunEvent::RepairCompleted => "RepairCompleted",
        RunEvent::ReplanRequested => "ReplanRequested",
        RunEvent::ReplanGraphDrafted => "ReplanGraphDrafted",
        RunEvent::GraphReviewPassed { .. } => "GraphReviewPassed",
        RunEvent::FinalAuditPassed => "FinalAuditPassed",
        RunEvent::DeliveryRehearsalPassed => "DeliveryRehearsalPassed",
        RunEvent::DeliveryApproved => "DeliveryApproved",
        RunEvent::DeliveryReceiptWritten => "DeliveryReceiptWritten",
        RunEvent::CandidateChangedBeforeDelivery => "CandidateChangedBeforeDelivery",
        RunEvent::TargetContextChanged => "TargetContextChanged",
        RunEvent::TargetWorktreeFingerprintChanged => "TargetWorktreeFingerprintChanged",
        RunEvent::DeliveredTreeMismatchObserved => "DeliveredTreeMismatchObserved",
        RunEvent::DeliveryOutcomeUnknown => "DeliveryOutcomeUnknown",
        RunEvent::CompletionRecorded => "CompletionRecorded",
    }
}

fn project_event_type_name(event: &ProjectEvent) -> &'static str {
    match event {
        ProjectEvent::AdvanceNominal => "AdvanceNominal",
        ProjectEvent::IdentityChanged => "IdentityChanged",
        ProjectEvent::ReinitializationConfirmed => "ReinitializationConfirmed",
        ProjectEvent::IntentUnresolved => "IntentUnresolved",
        ProjectEvent::IntentResolved => "IntentResolved",
        ProjectEvent::ConfigInvalidated => "ConfigInvalidated",
        ProjectEvent::ConfigRevalidated => "ConfigRevalidated",
        ProjectEvent::EnvironmentBlocked => "EnvironmentBlocked",
        ProjectEvent::EnvironmentUnblocked => "EnvironmentUnblocked",
        ProjectEvent::SkillsBlocked => "SkillsBlocked",
        ProjectEvent::SkillsUnblocked => "SkillsUnblocked",
        ProjectEvent::InitializationFailed => "InitializationFailed",
        ProjectEvent::InitializationRetried => "InitializationRetried",
        ProjectEvent::Archived => "Archived",
        ProjectEvent::Reactivated => "Reactivated",
    }
}

fn task_event_type_name(event: &TaskEvent) -> &'static str {
    match event {
        TaskEvent::Created { .. } => "Created",
        TaskEvent::Cancelled => "Cancelled",
        TaskEvent::RunTerminalApplied { .. } => "RunTerminalApplied",
        TaskEvent::RunStateProjected { .. } => "RunStateProjected",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::run::RunPhase;

    fn temp_db_path(label: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "automed-store-test-{label}-{}.sqlite3",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn unknown_aggregate_has_no_projection() {
        let path = temp_db_path("unknown-aggregate");
        let store = EventStore::open(&path).unwrap();
        assert!(store.load_run_state("run-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_legal_event_creates_revision_one() {
        let path = temp_db_path("first-event");
        let mut store = EventStore::open(&path).unwrap();
        let appended = store
            .append_run_event("run-1", RunEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(appended.state.phase, RunPhase::ResolvingProjectContext);
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "AdvanceNominal");
        assert_eq!(appended.seq, 1);
        let (revision, loaded) = store.load_run_state("run-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_events_advance_revision_and_state() {
        let path = temp_db_path("sequential");
        let mut store = EventStore::open(&path).unwrap();
        let first = store
            .append_run_event("run-1", RunEvent::AdvanceNominal)
            .unwrap();
        let second = store
            .append_run_event("run-1", RunEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(second.state.phase, RunPhase::DiscoveringFacts);
        assert_eq!(second.seq, first.seq + 1);
        let (revision, _) = store.load_run_state("run-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(store.event_count("run-1"), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_transition_writes_nothing() {
        let path = temp_db_path("illegal");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_run_event("run-1", RunEvent::PlanApproved)
            .unwrap_err();
        assert!(matches!(err, AppendError::Transition(_)));
        assert!(store.load_run_state("run-1").unwrap().is_none());
        assert_eq!(store.event_count("run-1"), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn state_survives_reconnect() {
        let path = temp_db_path("reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .append_run_event("run-1", RunEvent::AdvanceNominal)
                .unwrap();
            store
                .append_run_event("run-1", RunEvent::AdvanceNominal)
                .unwrap();
        }
        // Fresh connection to the same file: nothing but what was committed
        // to disk should be visible, per §11.2.
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_run_state("run-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(state.phase, RunPhase::DiscoveringFacts);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn each_event_gets_a_unique_event_id() {
        let path = temp_db_path("unique-ids");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_run_event("run-1", RunEvent::AdvanceNominal)
            .unwrap();
        store
            .append_run_event("run-1", RunEvent::AdvanceNominal)
            .unwrap();
        let ids: Vec<String> = {
            let mut stmt = store
                .conn
                .prepare("SELECT event_id FROM events WHERE aggregate_id = 'run-1' ORDER BY seq")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unknown_project_aggregate_has_no_projection() {
        let path = temp_db_path("unknown-project-aggregate");
        let store = EventStore::open(&path).unwrap();
        assert!(store.load_project_state("project-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_legal_project_event_creates_revision_one() {
        use autome_domain::project::ProjectPhase;

        let path = temp_db_path("first-project-event");
        let mut store = EventStore::open(&path).unwrap();
        let appended = store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(appended.state.phase, ProjectPhase::Inspecting);
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "AdvanceNominal");
        let (revision, loaded) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_project_events_advance_revision_and_state() {
        use autome_domain::project::ProjectPhase;

        let path = temp_db_path("sequential-project");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap();
        let second = store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(second.state.phase, ProjectPhase::AwaitingTrust);
        let (revision, _) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(store.event_count("project-1"), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_project_transition_writes_nothing() {
        let path = temp_db_path("illegal-project");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_project_event("project-1", ProjectEvent::IntentResolved)
            .unwrap_err();
        assert!(matches!(err, ProjectAppendError::Transition(_)));
        assert!(store.load_project_state("project-1").unwrap().is_none());
        assert_eq!(store.event_count("project-1"), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_state_survives_reconnect() {
        use autome_domain::project::ProjectPhase;

        let path = temp_db_path("project-reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .append_project_event("project-1", ProjectEvent::AdvanceNominal)
                .unwrap();
            store
                .append_project_event("project-1", ProjectEvent::AdvanceNominal)
                .unwrap();
        }
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(state.phase, ProjectPhase::AwaitingTrust);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn run_and_project_aggregates_share_the_events_table_without_colliding() {
        let path = temp_db_path("shared-events-table");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_run_event("same-id", RunEvent::AdvanceNominal)
            .unwrap();
        store
            .append_project_event("same-id", ProjectEvent::AdvanceNominal)
            .unwrap();
        assert!(store.load_run_state("same-id").unwrap().is_some());
        assert!(store.load_project_state("same-id").unwrap().is_some());
        std::fs::remove_file(&path).ok();
    }

    fn ready_project_state() -> autome_domain::project::ProjectState {
        autome_domain::project::ProjectState {
            lifecycle: autome_domain::project::ProjectLifecycle::Active,
            phase: autome_domain::project::ProjectPhase::Ready,
            hold: autome_domain::project::ProjectHold::None,
            revision: 1,
        }
    }

    fn task_created_event() -> TaskEvent {
        TaskEvent::Created {
            id: "task-1".to_string(),
            project: ready_project_state(),
            project_id: "project-1".to_string(),
            project_revision: 1,
            original_request_ref: "original-request-ref-1".to_string(),
        }
    }

    #[test]
    fn unknown_task_aggregate_has_no_projection() {
        let path = temp_db_path("unknown-task-aggregate");
        let store = EventStore::open(&path).unwrap();
        assert!(store.load_task_state("task-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_legal_task_event_creates_revision_one() {
        use autome_domain::task::TaskLifecycle;

        let path = temp_db_path("first-task-event");
        let mut store = EventStore::open(&path).unwrap();
        let appended = store
            .append_task_event("task-1", task_created_event())
            .unwrap();
        assert_eq!(appended.state.lifecycle, TaskLifecycle::Draft);
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "Created");
        let (revision, loaded) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_task_events_advance_revision_and_state() {
        use autome_domain::task::TaskLifecycle;

        let path = temp_db_path("sequential-task");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_task_event("task-1", task_created_event())
            .unwrap();
        let second = store
            .append_task_event("task-1", TaskEvent::Cancelled)
            .unwrap();
        assert_eq!(second.state.lifecycle, TaskLifecycle::Cancelled);
        assert_eq!(second.event_type, "Cancelled");
        let (revision, _) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(store.event_count("task-1"), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_task_transition_writes_nothing() {
        let path = temp_db_path("illegal-task");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_task_event("task-1", TaskEvent::Cancelled)
            .unwrap_err();
        assert!(matches!(err, TaskAppendError::Transition(_)));
        assert!(store.load_task_state("task-1").unwrap().is_none());
        assert_eq!(store.event_count("task-1"), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_state_survives_reconnect() {
        use autome_domain::task::TaskLifecycle;

        let path = temp_db_path("task-reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .append_task_event("task-1", task_created_event())
                .unwrap();
            store
                .append_task_event("task-1", TaskEvent::Cancelled)
                .unwrap();
        }
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(state.lifecycle, TaskLifecycle::Cancelled);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn run_project_and_task_aggregates_share_the_events_table_without_colliding() {
        let path = temp_db_path("shared-events-table-three-way");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_run_event("same-id", RunEvent::AdvanceNominal)
            .unwrap();
        store
            .append_project_event("same-id", ProjectEvent::AdvanceNominal)
            .unwrap();
        store
            .append_task_event("same-id", task_created_event())
            .unwrap();
        assert!(store.load_run_state("same-id").unwrap().is_some());
        assert!(store.load_project_state("same-id").unwrap().is_some());
        assert!(store.load_task_state("same-id").unwrap().is_some());
        std::fs::remove_file(&path).ok();
    }
}
