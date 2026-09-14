//! Maps an incoming IPC `Command` (plan §3.2) to a domain event applied
//! through the `EventStore`, and the resulting `Event` envelope sent back
//! to Electron Main. This is the thinnest possible real closed loop for
//! M0: stdin frame -> Command -> reducer -> SQLite -> Event -> stdout
//! frame. Every RunEvent/ProjectEvent variant that carries no payload of
//! its own is wired up via the lookup tables below (method name ==
//! snake_case of the variant name, under a `run.`/`project.` prefix).
//! RunEvent's three variants that carry an `origin` field
//! (GraphReviewRejected/GraphReviewPassed/ReadinessConfirmed) are wired
//! explicitly in `dispatch` below, ahead of the lookup tables: `params.origin`
//! is a string, "initial_plan" | "replan" for GraphReviewOrigin's two cases,
//! "node_candidate" | "post_integration" for ReadinessOrigin's two cases.

use crate::ipc::{Command, Event};
use crate::store::{
    AppendError, AppendedProjectEvent, AppendedRunEvent, EventStore, ProjectAppendError,
};
use autome_domain::project::ProjectEvent;
use autome_domain::run::{GraphReviewOrigin, ReadinessOrigin, RunEvent};

#[derive(Debug)]
pub enum DispatchError {
    UnknownMethod(String),
    InvalidParams(String),
    Store(AppendError),
    ProjectStore(ProjectAppendError),
}

impl From<AppendError> for DispatchError {
    fn from(value: AppendError) -> Self {
        DispatchError::Store(value)
    }
}

impl From<ProjectAppendError> for DispatchError {
    fn from(value: ProjectAppendError) -> Self {
        DispatchError::ProjectStore(value)
    }
}

pub fn dispatch(store: &mut EventStore, command: &Command) -> Result<Event, DispatchError> {
    match command.method.as_str() {
        "run.graph_review_rejected" => {
            let aggregate_id = require_aggregate_id(command)?;
            let origin = parse_graph_review_origin(command)?;
            let appended =
                store.append_run_event(&aggregate_id, RunEvent::GraphReviewRejected { origin })?;
            return Ok(run_event_envelope(aggregate_id, appended));
        }
        "run.graph_review_passed" => {
            let aggregate_id = require_aggregate_id(command)?;
            let origin = parse_graph_review_origin(command)?;
            let appended =
                store.append_run_event(&aggregate_id, RunEvent::GraphReviewPassed { origin })?;
            return Ok(run_event_envelope(aggregate_id, appended));
        }
        "run.readiness_confirmed" => {
            let aggregate_id = require_aggregate_id(command)?;
            let origin = parse_readiness_origin(command)?;
            let appended =
                store.append_run_event(&aggregate_id, RunEvent::ReadinessConfirmed { origin })?;
            return Ok(run_event_envelope(aggregate_id, appended));
        }
        _ => {}
    }
    if let Some(event) = parameterless_run_event(&command.method) {
        let aggregate_id = require_aggregate_id(command)?;
        let appended = store.append_run_event(&aggregate_id, event)?;
        return Ok(run_event_envelope(aggregate_id, appended));
    }
    if let Some(event) = parameterless_project_event(&command.method) {
        let aggregate_id = require_aggregate_id(command)?;
        let appended = store.append_project_event(&aggregate_id, event)?;
        return Ok(project_event_envelope(aggregate_id, appended));
    }
    Err(DispatchError::UnknownMethod(command.method.clone()))
}

fn run_event_envelope(aggregate_id: String, appended: AppendedRunEvent) -> Event {
    Event {
        event_seq: appended.seq as u64,
        event_id: appended.event_id,
        aggregate_id,
        aggregate_revision: appended.revision,
        event_type: appended.event_type.to_string(),
        occurred_at: appended.occurred_at,
        payload: serde_json::to_value(appended.state).expect("RunState always serializes"),
    }
}

fn project_event_envelope(aggregate_id: String, appended: AppendedProjectEvent) -> Event {
    Event {
        event_seq: appended.seq as u64,
        event_id: appended.event_id,
        aggregate_id,
        aggregate_revision: appended.revision,
        event_type: appended.event_type.to_string(),
        occurred_at: appended.occurred_at,
        payload: serde_json::to_value(appended.state).expect("ProjectState always serializes"),
    }
}

/// The 21 of 24 RunEvent variants that carry no payload. See the module
/// doc comment for why the remaining 3 are not here.
fn parameterless_run_event(method: &str) -> Option<RunEvent> {
    use RunEvent as E;
    Some(match method {
        "run.advance_nominal" => E::AdvanceNominal,
        "run.contract_review_rejected" => E::ContractReviewRejected,
        "run.readiness_ready" => E::ReadinessReady,
        "run.plan_approved" => E::PlanApproved,
        "run.readiness_invalidated" => E::ReadinessInvalidated,
        "run.readiness_capability_changed" => E::ReadinessCapabilityChanged,
        "run.configured_human_review_required" => E::ConfiguredHumanReviewRequired,
        "run.configured_human_review_passed" => E::ConfiguredHumanReviewPassed,
        "run.config_drift_detected" => E::ConfigDriftDetected,
        "run.repair_requested" => E::RepairRequested,
        "run.repair_completed" => E::RepairCompleted,
        "run.replan_requested" => E::ReplanRequested,
        "run.replan_graph_drafted" => E::ReplanGraphDrafted,
        "run.final_audit_passed" => E::FinalAuditPassed,
        "run.delivery_rehearsal_passed" => E::DeliveryRehearsalPassed,
        "run.delivery_approved" => E::DeliveryApproved,
        "run.delivery_receipt_written" => E::DeliveryReceiptWritten,
        "run.candidate_changed_before_delivery" => E::CandidateChangedBeforeDelivery,
        "run.target_context_changed" => E::TargetContextChanged,
        "run.target_worktree_fingerprint_changed" => E::TargetWorktreeFingerprintChanged,
        "run.delivered_tree_mismatch_observed" => E::DeliveredTreeMismatchObserved,
        "run.delivery_outcome_unknown" => E::DeliveryOutcomeUnknown,
        "run.completion_recorded" => E::CompletionRecorded,
        _ => return None,
    })
}

/// All 15 ProjectEvent variants carry no payload.
fn parameterless_project_event(method: &str) -> Option<ProjectEvent> {
    use ProjectEvent as E;
    Some(match method {
        "project.advance_nominal" => E::AdvanceNominal,
        "project.identity_changed" => E::IdentityChanged,
        "project.reinitialization_confirmed" => E::ReinitializationConfirmed,
        "project.intent_unresolved" => E::IntentUnresolved,
        "project.intent_resolved" => E::IntentResolved,
        "project.config_invalidated" => E::ConfigInvalidated,
        "project.config_revalidated" => E::ConfigRevalidated,
        "project.environment_blocked" => E::EnvironmentBlocked,
        "project.environment_unblocked" => E::EnvironmentUnblocked,
        "project.skills_blocked" => E::SkillsBlocked,
        "project.skills_unblocked" => E::SkillsUnblocked,
        "project.initialization_failed" => E::InitializationFailed,
        "project.initialization_retried" => E::InitializationRetried,
        "project.archived" => E::Archived,
        "project.reactivated" => E::Reactivated,
        _ => return None,
    })
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

fn parse_graph_review_origin(command: &Command) -> Result<GraphReviewOrigin, DispatchError> {
    match command.params.get("origin").and_then(|v| v.as_str()) {
        Some("initial_plan") => Ok(GraphReviewOrigin::InitialPlan),
        Some("replan") => Ok(GraphReviewOrigin::Replan),
        _ => Err(DispatchError::InvalidParams(
            "params.origin must be \"initial_plan\" or \"replan\"".to_string(),
        )),
    }
}

fn parse_readiness_origin(command: &Command) -> Result<ReadinessOrigin, DispatchError> {
    match command.params.get("origin").and_then(|v| v.as_str()) {
        Some("node_candidate") => Ok(ReadinessOrigin::NodeCandidate),
        Some("post_integration") => Ok(ReadinessOrigin::PostIntegration),
        _ => Err(DispatchError::InvalidParams(
            "params.origin must be \"node_candidate\" or \"post_integration\"".to_string(),
        )),
    }
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
    fn project_advance_nominal_appends_and_returns_matching_event() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "project.advance_nominal",
            json!({ "aggregate_id": "project-1" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "project-1");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "AdvanceNominal");
        let (revision, state) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_illegal_transition_surfaces_as_project_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "project.advance_nominal",
            json!({ "aggregate_id": "project-1" }),
        );
        // §6.1's nominal path has 8 phases; Registered is index 0, so 7
        // calls walk it all the way to Ready and the 8th has nowhere left
        // to advance to.
        for _ in 0..7 {
            dispatch(&mut store, &cmd).unwrap();
        }
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(
            err,
            DispatchError::ProjectStore(ProjectAppendError::Transition(_))
        ));
        std::fs::remove_file(&path).ok();
    }

    /// Drives a fresh run through N `run.advance_nominal` calls so a test
    /// can exercise an event legal only at a later phase (§6.2's nominal
    /// path: GraphReview is index 6, CheckingReadiness is index 7).
    fn advance_to(store: &mut EventStore, aggregate_id: &str, steps: usize) {
        let cmd = command(
            "run.advance_nominal",
            json!({ "aggregate_id": aggregate_id }),
        );
        for _ in 0..steps {
            dispatch(store, &cmd).unwrap();
        }
    }

    #[test]
    fn graph_review_rejected_with_initial_plan_origin_appends_event() {
        let (mut store, path) = temp_store();
        advance_to(&mut store, "run-1", 6); // Received -> ... -> GraphReview
        let cmd = command(
            "run.graph_review_rejected",
            json!({ "aggregate_id": "run-1", "origin": "initial_plan" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "GraphReviewRejected");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_review_passed_with_replan_origin_supersedes_the_run() {
        let (mut store, path) = temp_store();
        advance_to(&mut store, "run-1", 6); // Received -> ... -> GraphReview
        let cmd = command(
            "run.graph_review_passed",
            json!({ "aggregate_id": "run-1", "origin": "replan" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "GraphReviewPassed");
        let (_, state) = store.load_run_state("run-1").unwrap().unwrap();
        assert_eq!(state.terminal, autome_domain::run::RunTerminal::Superseded);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn readiness_confirmed_with_node_candidate_origin_appends_event() {
        let (mut store, path) = temp_store();
        advance_to(&mut store, "run-1", 7); // Received -> ... -> CheckingReadiness
        let cmd = command(
            "run.readiness_confirmed",
            json!({ "aggregate_id": "run-1", "origin": "node_candidate" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "ReadinessConfirmed");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_review_rejected_rejects_unknown_origin_string() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "run.graph_review_rejected",
            json!({ "aggregate_id": "run-1", "origin": "sideways" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        assert!(store.load_run_state("run-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn readiness_confirmed_rejects_missing_origin() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "run.readiness_confirmed",
            json!({ "aggregate_id": "run-1" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn every_parameterless_run_method_is_recognized() {
        let methods = [
            "run.advance_nominal",
            "run.contract_review_rejected",
            "run.readiness_ready",
            "run.plan_approved",
            "run.readiness_invalidated",
            "run.readiness_capability_changed",
            "run.configured_human_review_required",
            "run.configured_human_review_passed",
            "run.config_drift_detected",
            "run.repair_requested",
            "run.repair_completed",
            "run.replan_requested",
            "run.replan_graph_drafted",
            "run.final_audit_passed",
            "run.delivery_rehearsal_passed",
            "run.delivery_approved",
            "run.delivery_receipt_written",
            "run.candidate_changed_before_delivery",
            "run.target_context_changed",
            "run.target_worktree_fingerprint_changed",
            "run.delivered_tree_mismatch_observed",
            "run.delivery_outcome_unknown",
            "run.completion_recorded",
        ];
        for method in methods {
            let (mut store, path) = temp_store();
            let cmd = command(method, json!({ "aggregate_id": "run-1" }));
            let outcome = dispatch(&mut store, &cmd);
            assert!(
                !matches!(outcome, Err(DispatchError::UnknownMethod(_))),
                "{method} should be recognized by the lookup table"
            );
            std::fs::remove_file(&path).ok();
        }
    }

    #[test]
    fn every_project_method_is_recognized() {
        let methods = [
            "project.advance_nominal",
            "project.identity_changed",
            "project.reinitialization_confirmed",
            "project.intent_unresolved",
            "project.intent_resolved",
            "project.config_invalidated",
            "project.config_revalidated",
            "project.environment_blocked",
            "project.environment_unblocked",
            "project.skills_blocked",
            "project.skills_unblocked",
            "project.initialization_failed",
            "project.initialization_retried",
            "project.archived",
            "project.reactivated",
        ];
        for method in methods {
            let (mut store, path) = temp_store();
            let cmd = command(method, json!({ "aggregate_id": "project-1" }));
            let outcome = dispatch(&mut store, &cmd);
            assert!(
                !matches!(outcome, Err(DispatchError::UnknownMethod(_))),
                "{method} should be recognized by the lookup table"
            );
            std::fs::remove_file(&path).ok();
        }
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
