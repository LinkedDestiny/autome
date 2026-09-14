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

use autome_domain::contract::{self, ContractEvent, ContractEventError, TaskContract};
use autome_domain::graph::{self, GraphEvent, GraphEventError, TaskGraph};
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

#[derive(Debug)]
pub enum ContractAppendError {
    Sql(rusqlite::Error),
    Transition(ContractEventError),
}

impl From<rusqlite::Error> for ContractAppendError {
    fn from(value: rusqlite::Error) -> Self {
        ContractAppendError::Sql(value)
    }
}

#[derive(Debug)]
pub enum GraphAppendError {
    Sql(rusqlite::Error),
    Transition(GraphEventError),
}

impl From<rusqlite::Error> for GraphAppendError {
    fn from(value: rusqlite::Error) -> Self {
        GraphAppendError::Sql(value)
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

/// Mirrors `AppendedTaskEvent`; see its doc comment.
#[derive(Debug, Clone)]
pub struct AppendedContractEvent {
    pub seq: i64,
    pub event_id: String,
    pub revision: u64,
    pub event_type: &'static str,
    pub occurred_at: String,
    pub state: TaskContract,
}

/// Mirrors `AppendedContractEvent`; see its doc comment.
#[derive(Debug, Clone)]
pub struct AppendedGraphEvent {
    pub seq: i64,
    pub event_id: String,
    pub revision: u64,
    pub event_type: &'static str,
    pub occurred_at: String,
    pub state: TaskGraph,
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
            CREATE TABLE IF NOT EXISTS contract_projections (
                aggregate_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL,
                status TEXT NOT NULL,
                version INTEGER NOT NULL,
                state_json TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS graph_projections (
                aggregate_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL,
                version INTEGER NOT NULL,
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

    /// Loads the current projected (revision, TaskContract) for an
    /// aggregate, or `None` if it has never been created — mirrors
    /// `load_task_state`.
    pub fn load_contract_state(
        &self,
        aggregate_id: &str,
    ) -> rusqlite::Result<Option<(u64, TaskContract)>> {
        self.conn
            .query_row(
                "SELECT revision, state_json FROM contract_projections WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| {
                    let revision: i64 = row.get(0)?;
                    let state_json: String = row.get(1)?;
                    Ok((revision, state_json))
                },
            )
            .optional()?
            .map(|(revision, state_json)| {
                let state: TaskContract = serde_json::from_str(&state_json).map_err(|e| {
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
    /// Mirrors `append_task_event`; the only structural difference is the
    /// projection's scalar columns (`status`/`version` instead of
    /// `lifecycle`).
    pub fn append_contract_event(
        &mut self,
        aggregate_id: &str,
        event: ContractEvent,
    ) -> Result<AppendedContractEvent, ContractAppendError> {
        let tx = self.conn.transaction()?;

        let (revision, current_state) = {
            let loaded = tx
                .query_row(
                    "SELECT revision, state_json FROM contract_projections WHERE aggregate_id = ?1",
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
                    let state: TaskContract = serde_json::from_str(&state_json).expect(
                        "contract_projections.state_json is only ever written by this module as valid TaskContract JSON",
                    );
                    (revision as u64, Some(state))
                }
                None => (0, None),
            }
        };

        let event_type = contract_event_type_name(&event);
        let payload = serde_json::to_string(&event).expect("ContractEvent always serializes");
        let next_state =
            contract::apply(current_state, event).map_err(ContractAppendError::Transition)?;
        let next_revision = revision + 1;
        let event_id = uuid::Uuid::new_v4().to_string();
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails");
        let state_json =
            serde_json::to_string(&next_state).expect("TaskContract always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Contract', ?3, ?4, ?5, ?6)",
            params![event_id, aggregate_id, next_revision as i64, event_type, payload, recorded_at],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO contract_projections (aggregate_id, revision, status, version, state_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                status = excluded.status,
                version = excluded.version,
                state_json = excluded.state_json",
            params![
                aggregate_id,
                next_revision as i64,
                format!("{:?}", next_state.status),
                next_state.version.0,
                state_json
            ],
        )?;

        tx.commit()?;
        Ok(AppendedContractEvent {
            seq,
            event_id,
            revision: next_revision,
            event_type,
            occurred_at: recorded_at,
            state: next_state,
        })
    }

    /// Loads the current projected (revision, TaskGraph) for an aggregate,
    /// or `None` if it has never been created — mirrors
    /// `load_contract_state`.
    pub fn load_graph_state(
        &self,
        aggregate_id: &str,
    ) -> rusqlite::Result<Option<(u64, TaskGraph)>> {
        self.conn
            .query_row(
                "SELECT revision, state_json FROM graph_projections WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| {
                    let revision: i64 = row.get(0)?;
                    let state_json: String = row.get(1)?;
                    Ok((revision, state_json))
                },
            )
            .optional()?
            .map(|(revision, state_json)| {
                let state: TaskGraph = serde_json::from_str(&state_json).map_err(|e| {
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
    /// Mirrors `append_contract_event`; the projection has no `status`
    /// column since `TaskGraph` has no status field (see `graph::apply`'s
    /// doc comment).
    pub fn append_graph_event(
        &mut self,
        aggregate_id: &str,
        event: GraphEvent,
    ) -> Result<AppendedGraphEvent, GraphAppendError> {
        let tx = self.conn.transaction()?;

        let (revision, current_state) = {
            let loaded = tx
                .query_row(
                    "SELECT revision, state_json FROM graph_projections WHERE aggregate_id = ?1",
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
                    let state: TaskGraph = serde_json::from_str(&state_json).expect(
                        "graph_projections.state_json is only ever written by this module as valid TaskGraph JSON",
                    );
                    (revision as u64, Some(state))
                }
                None => (0, None),
            }
        };

        let event_type = graph_event_type_name(&event);
        let payload = serde_json::to_string(&event).expect("GraphEvent always serializes");
        let next_state =
            graph::apply(current_state, event).map_err(GraphAppendError::Transition)?;
        let next_revision = revision + 1;
        let event_id = uuid::Uuid::new_v4().to_string();
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails");
        let state_json = serde_json::to_string(&next_state).expect("TaskGraph always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Graph', ?3, ?4, ?5, ?6)",
            params![event_id, aggregate_id, next_revision as i64, event_type, payload, recorded_at],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO graph_projections (aggregate_id, revision, version, state_json)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                version = excluded.version,
                state_json = excluded.state_json",
            params![
                aggregate_id,
                next_revision as i64,
                next_state.version,
                state_json
            ],
        )?;

        tx.commit()?;
        Ok(AppendedGraphEvent {
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
        TaskEvent::DispatchStateProjected { .. } => "DispatchStateProjected",
    }
}

fn contract_event_type_name(event: &ContractEvent) -> &'static str {
    match event {
        ContractEvent::Created { .. } => "Created",
        ContractEvent::Frozen => "Frozen",
        ContractEvent::Amended { .. } => "Amended",
    }
}

fn graph_event_type_name(event: &GraphEvent) -> &'static str {
    match event {
        GraphEvent::Created { .. } => "Created",
        GraphEvent::Replaced { .. } => "Replaced",
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
    fn dispatch_state_projected_updates_task_dispatch_state_and_queue_entry() {
        use autome_domain::task::{DispatchState, QueueEntry};

        let path = temp_db_path("dispatch-state-projected");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_task_event("task-1", task_created_event())
            .unwrap();
        let appended = store
            .append_task_event(
                "task-1",
                TaskEvent::DispatchStateProjected {
                    dispatch_state: DispatchState::Queued,
                    queue_entry: Some(QueueEntry {
                        enqueued_event_seq: 3,
                        projected_position: 1,
                        blocked_by_task_id: Some("task-0".to_string()),
                    }),
                },
            )
            .unwrap();
        assert_eq!(appended.event_type, "DispatchStateProjected");
        assert_eq!(appended.state.dispatch_state, DispatchState::Queued);
        assert_eq!(
            appended.state.queue_entry.unwrap().blocked_by_task_id,
            Some("task-0".to_string())
        );
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
    fn run_project_task_contract_and_graph_aggregates_share_the_events_table_without_colliding() {
        let path = temp_db_path("shared-events-table-five-way");
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
        store
            .append_contract_event("same-id", contract_created_event())
            .unwrap();
        store
            .append_graph_event("same-id", graph_created_event())
            .unwrap();
        assert!(store.load_run_state("same-id").unwrap().is_some());
        assert!(store.load_project_state("same-id").unwrap().is_some());
        assert!(store.load_task_state("same-id").unwrap().is_some());
        assert!(store.load_contract_state("same-id").unwrap().is_some());
        assert!(store.load_graph_state("same-id").unwrap().is_some());
        std::fs::remove_file(&path).ok();
    }

    fn contract_created_event() -> ContractEvent {
        ContractEvent::Created {
            id: "contract-1".to_string(),
            content_hash: "hash-1".to_string(),
            requirements: vec![autome_domain::requirement::Requirement {
                id: autome_domain::requirement::RequirementId("R-001".into()),
                statement: "does something".into(),
                kind: autome_domain::requirement::RequirementKind::Functional,
                necessity: autome_domain::requirement::Necessity::Must,
                source_anchors: vec![autome_domain::requirement::SourceAnchor {
                    anchor_ref: "raw_text:0-10".into(),
                }],
                acceptance_logic: autome_domain::requirement::AllOf,
                acceptance_check_ids: vec![autome_domain::requirement::CheckId("C-001".into())],
                delivery_spec: None,
                risk_level: autome_domain::requirement::RiskLevel::Low,
                superseded_by: None,
            }],
            acceptance_checks: vec![autome_domain::contract::AcceptanceCheck {
                id: autome_domain::requirement::CheckId("C-001".into()),
                kind: autome_domain::contract::CheckKind::Process,
                requirement_id: autome_domain::requirement::RequirementId("R-001".into()),
                mandatory: true,
                expected_observation: "exit code 0".into(),
                negative_scenario: "non-zero exit".into(),
                required_environment_level: "base".into(),
                isolation_policy: "worktree".into(),
                repeat_policy: "once".into(),
                inventory_policy: "track".into(),
                freshness_policy: "must-be-current".into(),
            }],
        }
    }

    #[test]
    fn unknown_contract_aggregate_has_no_projection() {
        let path = temp_db_path("unknown-contract-aggregate");
        let store = EventStore::open(&path).unwrap();
        assert!(store.load_contract_state("contract-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_legal_contract_event_creates_revision_one() {
        use autome_domain::contract::ContractStatus;

        let path = temp_db_path("first-contract-event");
        let mut store = EventStore::open(&path).unwrap();
        let appended = store
            .append_contract_event("contract-1", contract_created_event())
            .unwrap();
        assert_eq!(appended.state.status, ContractStatus::Draft);
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "Created");
        let (revision, loaded) = store.load_contract_state("contract-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_contract_events_advance_revision_and_state() {
        use autome_domain::contract::ContractStatus;

        let path = temp_db_path("sequential-contract");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_contract_event("contract-1", contract_created_event())
            .unwrap();
        let second = store
            .append_contract_event("contract-1", ContractEvent::Frozen)
            .unwrap();
        assert_eq!(second.state.status, ContractStatus::Frozen);
        assert_eq!(second.event_type, "Frozen");
        let (revision, _) = store.load_contract_state("contract-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(store.event_count("contract-1"), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_contract_transition_writes_nothing() {
        let path = temp_db_path("illegal-contract");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_contract_event("contract-1", ContractEvent::Frozen)
            .unwrap_err();
        assert!(matches!(err, ContractAppendError::Transition(_)));
        assert!(store.load_contract_state("contract-1").unwrap().is_none());
        assert_eq!(store.event_count("contract-1"), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_state_survives_reconnect() {
        use autome_domain::contract::ContractStatus;

        let path = temp_db_path("contract-reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .append_contract_event("contract-1", contract_created_event())
                .unwrap();
            store
                .append_contract_event("contract-1", ContractEvent::Frozen)
                .unwrap();
        }
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_contract_state("contract-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(state.status, ContractStatus::Frozen);
        std::fs::remove_file(&path).ok();
    }

    fn graph_created_event() -> GraphEvent {
        GraphEvent::Created {
            id: "graph-1".to_string(),
            contract_ref: "contract-1".to_string(),
            graph_hash: "hash-1".to_string(),
            nodes: vec![autome_domain::graph::GraphNode {
                id: autome_domain::graph::NodeId("N-1".into()),
                kind: "generic".into(),
                purpose: autome_domain::graph::NodePurpose::Business,
                title: "do the thing".into(),
                requirement_ids: vec![autome_domain::requirement::RequirementId("R-001".into())],
                acceptance_check_ids: vec![autome_domain::requirement::CheckId("C-001".into())],
                depends_on: vec![],
                expected_outputs: vec!["artifact-1".into()],
                write_scope: vec!["src/lib.rs".into()],
                risk_level: autome_domain::graph::RiskLevel::Low,
                estimated_budget: 1,
            }],
        }
    }

    #[test]
    fn unknown_graph_aggregate_has_no_projection() {
        let path = temp_db_path("unknown-graph-aggregate");
        let store = EventStore::open(&path).unwrap();
        assert!(store.load_graph_state("graph-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_legal_graph_event_creates_revision_one() {
        let path = temp_db_path("first-graph-event");
        let mut store = EventStore::open(&path).unwrap();
        let appended = store
            .append_graph_event("graph-1", graph_created_event())
            .unwrap();
        assert_eq!(appended.state.version, 1);
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "Created");
        let (revision, loaded) = store.load_graph_state("graph-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_graph_events_advance_revision_and_state() {
        let path = temp_db_path("sequential-graph");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_graph_event("graph-1", graph_created_event())
            .unwrap();
        let second = store
            .append_graph_event(
                "graph-1",
                GraphEvent::Replaced {
                    graph_hash: "hash-2".to_string(),
                    nodes: vec![],
                },
            )
            .unwrap();
        assert_eq!(second.state.version, 2);
        assert_eq!(second.state.graph_hash, "hash-2");
        assert!(second.state.nodes.is_empty());
        assert_eq!(second.event_type, "Replaced");
        let (revision, _) = store.load_graph_state("graph-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(store.event_count("graph-1"), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_graph_transition_writes_nothing() {
        let path = temp_db_path("illegal-graph");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_graph_event(
                "graph-1",
                GraphEvent::Replaced {
                    graph_hash: "hash-2".to_string(),
                    nodes: vec![],
                },
            )
            .unwrap_err();
        assert!(matches!(err, GraphAppendError::Transition(_)));
        assert!(store.load_graph_state("graph-1").unwrap().is_none());
        assert_eq!(store.event_count("graph-1"), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_state_survives_reconnect() {
        let path = temp_db_path("graph-reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .append_graph_event("graph-1", graph_created_event())
                .unwrap();
            store
                .append_graph_event(
                    "graph-1",
                    GraphEvent::Replaced {
                        graph_hash: "hash-2".to_string(),
                        nodes: vec![],
                    },
                )
                .unwrap();
        }
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_graph_state("graph-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(state.version, 2);
        assert_eq!(state.graph_hash, "hash-2");
        std::fs::remove_file(&path).ok();
    }
}
