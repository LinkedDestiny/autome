//! Maps an incoming IPC `Command` (plan §3.2) to a domain event applied
//! through the `EventStore`, and the resulting `Event` envelope sent back
//! to Electron Main. This is the thinnest possible real closed loop for
//! M0: stdin frame -> Command -> reducer -> SQLite -> Event -> stdout
//! frame. Only the Run aggregate's `AdvanceNominal` transition is wired up
//! so far; every other RunEvent variant and every other aggregate will
//! extend this match, not replace its shape.

use crate::ipc::{Command, Event};
use crate::store::{AppendError, EventStore};
use autome_domain::run::RunEvent;

#[derive(Debug)]
pub enum DispatchError {
    UnknownMethod(String),
    InvalidParams(String),
    Store(AppendError),
}

impl From<AppendError> for DispatchError {
    fn from(value: AppendError) -> Self {
        DispatchError::Store(value)
    }
}

pub fn dispatch(store: &mut EventStore, command: &Command) -> Result<Event, DispatchError> {
    match command.method.as_str() {
        "run.advance_nominal" => {
            let aggregate_id = require_aggregate_id(command)?;
            let appended = store.append_run_event(&aggregate_id, RunEvent::AdvanceNominal)?;
            Ok(Event {
                event_seq: appended.seq as u64,
                event_id: appended.event_id,
                aggregate_id,
                aggregate_revision: appended.revision,
                event_type: appended.event_type.to_string(),
                occurred_at: appended.occurred_at,
                payload: serde_json::to_value(appended.state).expect("RunState always serializes"),
            })
        }
        other => Err(DispatchError::UnknownMethod(other.to_string())),
    }
}

fn require_aggregate_id(command: &Command) -> Result<String, DispatchError> {
    command
        .params
        .get("aggregate_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            DispatchError::InvalidParams("params.aggregate_id must be a string".to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_store() -> (EventStore, String) {
        let path = std::env::temp_dir()
            .join(format!(
                "automed-dispatch-test-{}.sqlite3",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned();
        (EventStore::open(&path).unwrap(), path)
    }

    fn command(method: &str, params: serde_json::Value) -> Command {
        Command {
            request_id: "req-1".to_string(),
            command_id: "cmd-1".to_string(),
            expected_revision: None,
            protocol_version: crate::ipc::PROTOCOL_VERSION,
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn advance_nominal_appends_and_returns_matching_event() {
        let (mut store, path) = temp_store();
        let cmd = command("run.advance_nominal", json!({ "aggregate_id": "run-1" }));
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "run-1");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "AdvanceNominal");
        assert_eq!(event.event_seq, 1);
        let (revision, state) = store.load_run_state("run-1").unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn unknown_method_is_rejected_without_touching_the_store() {
        let (mut store, path) = temp_store();
        let cmd = command("run.teleport", json!({ "aggregate_id": "run-1" }));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::UnknownMethod(m) if m == "run.teleport"));
        assert!(store.load_run_state("run-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_aggregate_id_is_rejected() {
        let (mut store, path) = temp_store();
        let cmd = command("run.advance_nominal", json!({}));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_transition_surfaces_as_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command("run.advance_nominal", json!({ "aggregate_id": "run-1" }));
        // The linear nominal path (§6.2) has 17 phases; Received is index 0,
        // so 16 calls walk it all the way to the final phase and the 17th
        // has nowhere left to advance to.
        for _ in 0..16 {
            dispatch(&mut store, &cmd).unwrap();
        }
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(
            err,
            DispatchError::Store(AppendError::Transition(_))
        ));
        std::fs::remove_file(&path).ok();
    }
}
