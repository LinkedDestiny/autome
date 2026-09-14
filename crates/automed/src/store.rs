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

use autome_domain::run::{self, RunEvent, RunState, TransitionError};
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
                UNIQUE(aggregate_id, revision)
            );
            CREATE TABLE IF NOT EXISTS run_projections (
                aggregate_id TEXT PRIMARY KEY,
                revision INTEGER NOT NULL,
                phase TEXT NOT NULL,
                hold TEXT NOT NULL,
                terminal TEXT NOT NULL,
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
}
