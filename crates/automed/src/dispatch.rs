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
//! TaskEvent has no parameterless variants at all (even `Cancelled` targets
//! a different store method than run.*/project.*), so all four `task.*`
//! methods are wired the same explicit way: `task.created` parses
//! `params.project` (a `ProjectState`), `params.project_id`,
//! `params.project_revision` (u32) and `params.original_request_ref`;
//! `task.cancelled` takes no extra params; `task.run_terminal_applied`
//! parses `params.run_terminal` (one of RunTerminal's variant names);
//! `task.run_state_projected` parses `params.run_state` (a `RunState`);
//! `task.dispatch_state_projected` parses `params.dispatch_state` (one of
//! `DispatchState`'s variant names) and an optional `params.queue_entry`
//! (a `QueueEntry`, absent meaning `None`).
//! ContractEvent has no parameterless variants either, so the three
//! `contract.*` methods follow the same explicit pattern: `contract.created`
//! parses `params.requirements` (`Vec<Requirement>`) and
//! `params.acceptance_checks` (`Vec<AcceptanceCheck>`); `contract.frozen`
//! takes no extra params; `contract.amended` parses `params.amendment` (a
//! `ContractAmendment`).
//! GraphEvent has no parameterless variants either (both `Created` and
//! `Replaced` carry `nodes`), so the two `graph.*` methods follow the same
//! explicit pattern: `graph.created` parses `params.contract_ref`,
//! `params.graph_hash` and `params.nodes` (`Vec<GraphNode>`);
//! `graph.replaced` parses `params.graph_hash` and `params.nodes`.

use crate::ipc::{Command, Event};
use crate::store::{
    AppendError, AppendedContractEvent, AppendedGraphEvent, AppendedProjectEvent, AppendedRunEvent,
    AppendedTaskEvent, ContractAppendError, EventStore, GraphAppendError, ProjectAppendError,
    TaskAppendError,
};
use autome_domain::contract::{AcceptanceCheck, ContractAmendment, ContractEvent};
use autome_domain::graph::{GraphEvent, GraphNode};
use autome_domain::project::{ProjectEvent, ProjectState};
use autome_domain::requirement::Requirement;
use autome_domain::run::{GraphReviewOrigin, ReadinessOrigin, RunEvent, RunState, RunTerminal};
use autome_domain::task::{DispatchState, QueueEntry, TaskEvent};

#[derive(Debug)]
pub enum DispatchError {
    UnknownMethod(String),
    InvalidParams(String),
    Store(AppendError),
    ProjectStore(ProjectAppendError),
    TaskStore(TaskAppendError),
    ContractStore(ContractAppendError),
    GraphStore(GraphAppendError),
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

impl From<TaskAppendError> for DispatchError {
    fn from(value: TaskAppendError) -> Self {
        DispatchError::TaskStore(value)
    }
}

impl From<ContractAppendError> for DispatchError {
    fn from(value: ContractAppendError) -> Self {
        DispatchError::ContractStore(value)
    }
}

impl From<GraphAppendError> for DispatchError {
    fn from(value: GraphAppendError) -> Self {
        DispatchError::GraphStore(value)
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
        "task.created" => {
            let aggregate_id = require_aggregate_id(command)?;
            let project = parse_project_state(command)?;
            let project_id = parse_string_param(command, "project_id")?;
            let project_revision = parse_u32_param(command, "project_revision")?;
            let original_request_ref = parse_string_param(command, "original_request_ref")?;
            let event = TaskEvent::Created {
                id: aggregate_id.clone(),
                project,
                project_id,
                project_revision,
                original_request_ref,
            };
            let appended = store.append_task_event(&aggregate_id, event)?;
            return Ok(task_event_envelope(aggregate_id, appended));
        }
        "task.cancelled" => {
            let aggregate_id = require_aggregate_id(command)?;
            let appended = store.append_task_event(&aggregate_id, TaskEvent::Cancelled)?;
            return Ok(task_event_envelope(aggregate_id, appended));
        }
        "task.run_terminal_applied" => {
            let aggregate_id = require_aggregate_id(command)?;
            let run_terminal = parse_run_terminal(command)?;
            let appended = store.append_task_event(
                &aggregate_id,
                TaskEvent::RunTerminalApplied { run_terminal },
            )?;
            return Ok(task_event_envelope(aggregate_id, appended));
        }
        "task.run_state_projected" => {
            let aggregate_id = require_aggregate_id(command)?;
            let run_state = parse_run_state(command)?;
            let appended = store
                .append_task_event(&aggregate_id, TaskEvent::RunStateProjected { run_state })?;
            return Ok(task_event_envelope(aggregate_id, appended));
        }
        "task.dispatch_state_projected" => {
            let aggregate_id = require_aggregate_id(command)?;
            let dispatch_state = parse_dispatch_state(command)?;
            let queue_entry = parse_optional_queue_entry_param(command)?;
            let appended = store.append_task_event(
                &aggregate_id,
                TaskEvent::DispatchStateProjected {
                    dispatch_state,
                    queue_entry,
                },
            )?;
            return Ok(task_event_envelope(aggregate_id, appended));
        }
        "contract.created" => {
            let aggregate_id = require_aggregate_id(command)?;
            let content_hash = parse_string_param(command, "content_hash")?;
            let requirements = parse_requirements_param(command)?;
            let acceptance_checks = parse_acceptance_checks_param(command)?;
            let event = ContractEvent::Created {
                id: aggregate_id.clone(),
                content_hash,
                requirements,
                acceptance_checks,
            };
            let appended = store.append_contract_event(&aggregate_id, event)?;
            return Ok(contract_event_envelope(aggregate_id, appended));
        }
        "contract.frozen" => {
            let aggregate_id = require_aggregate_id(command)?;
            let appended = store.append_contract_event(&aggregate_id, ContractEvent::Frozen)?;
            return Ok(contract_event_envelope(aggregate_id, appended));
        }
        "contract.amended" => {
            let aggregate_id = require_aggregate_id(command)?;
            let amendment = parse_contract_amendment_param(command)?;
            let appended =
                store.append_contract_event(&aggregate_id, ContractEvent::Amended { amendment })?;
            return Ok(contract_event_envelope(aggregate_id, appended));
        }
        "graph.created" => {
            let aggregate_id = require_aggregate_id(command)?;
            let contract_ref = parse_string_param(command, "contract_ref")?;
            let graph_hash = parse_string_param(command, "graph_hash")?;
            let nodes = parse_graph_nodes_param(command)?;
            let event = GraphEvent::Created {
                id: aggregate_id.clone(),
                contract_ref,
                graph_hash,
                nodes,
            };
            let appended = store.append_graph_event(&aggregate_id, event)?;
            return Ok(graph_event_envelope(aggregate_id, appended));
        }
        "graph.replaced" => {
            let aggregate_id = require_aggregate_id(command)?;
            let graph_hash = parse_string_param(command, "graph_hash")?;
            let nodes = parse_graph_nodes_param(command)?;
            let appended = store
                .append_graph_event(&aggregate_id, GraphEvent::Replaced { graph_hash, nodes })?;
            return Ok(graph_event_envelope(aggregate_id, appended));
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

fn task_event_envelope(aggregate_id: String, appended: AppendedTaskEvent) -> Event {
    Event {
        event_seq: appended.seq as u64,
        event_id: appended.event_id,
        aggregate_id,
        aggregate_revision: appended.revision,
        event_type: appended.event_type.to_string(),
        occurred_at: appended.occurred_at,
        payload: serde_json::to_value(appended.state).expect("Task always serializes"),
    }
}

fn contract_event_envelope(aggregate_id: String, appended: AppendedContractEvent) -> Event {
    Event {
        event_seq: appended.seq as u64,
        event_id: appended.event_id,
        aggregate_id,
        aggregate_revision: appended.revision,
        event_type: appended.event_type.to_string(),
        occurred_at: appended.occurred_at,
        payload: serde_json::to_value(appended.state).expect("TaskContract always serializes"),
    }
}

fn graph_event_envelope(aggregate_id: String, appended: AppendedGraphEvent) -> Event {
    Event {
        event_seq: appended.seq as u64,
        event_id: appended.event_id,
        aggregate_id,
        aggregate_revision: appended.revision,
        event_type: appended.event_type.to_string(),
        occurred_at: appended.occurred_at,
        payload: serde_json::to_value(appended.state).expect("TaskGraph always serializes"),
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

fn parse_project_state(command: &Command) -> Result<ProjectState, DispatchError> {
    let value =
        command.params.get("project").cloned().ok_or_else(|| {
            DispatchError::InvalidParams("params.project is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.project is not a valid ProjectState: {e}"))
    })
}

fn parse_string_param(command: &Command, key: &str) -> Result<String, DispatchError> {
    command
        .params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| DispatchError::InvalidParams(format!("params.{key} must be a string")))
}

fn parse_u32_param(command: &Command, key: &str) -> Result<u32, DispatchError> {
    command
        .params
        .get(key)
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| {
            DispatchError::InvalidParams(format!(
                "params.{key} must be a non-negative integer that fits in u32"
            ))
        })
}

fn parse_run_terminal(command: &Command) -> Result<RunTerminal, DispatchError> {
    match command.params.get("run_terminal").and_then(|v| v.as_str()) {
        Some("None") => Ok(RunTerminal::None),
        Some("Completed") => Ok(RunTerminal::Completed),
        Some("Superseded") => Ok(RunTerminal::Superseded),
        Some("ProtocolFailed") => Ok(RunTerminal::ProtocolFailed),
        Some("Infeasible") => Ok(RunTerminal::Infeasible),
        Some("Cancelled") => Ok(RunTerminal::Cancelled),
        _ => Err(DispatchError::InvalidParams(
            "params.run_terminal must be one of None|Completed|Superseded|ProtocolFailed|Infeasible|Cancelled"
                .to_string(),
        )),
    }
}

fn parse_run_state(command: &Command) -> Result<RunState, DispatchError> {
    let value =
        command.params.get("run_state").cloned().ok_or_else(|| {
            DispatchError::InvalidParams("params.run_state is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.run_state is not a valid RunState: {e}"))
    })
}

fn parse_dispatch_state(command: &Command) -> Result<DispatchState, DispatchError> {
    match command
        .params
        .get("dispatch_state")
        .and_then(|v| v.as_str())
    {
        Some("Queued") => Ok(DispatchState::Queued),
        Some("Running") => Ok(DispatchState::Running),
        Some("Waiting") => Ok(DispatchState::Waiting),
        Some("None") => Ok(DispatchState::None),
        _ => Err(DispatchError::InvalidParams(
            "params.dispatch_state must be one of Queued|Running|Waiting|None".to_string(),
        )),
    }
}

fn parse_optional_queue_entry_param(
    command: &Command,
) -> Result<Option<QueueEntry>, DispatchError> {
    match command.params.get("queue_entry").cloned() {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value).map(Some).map_err(|e| {
            DispatchError::InvalidParams(format!(
                "params.queue_entry is not a valid QueueEntry: {e}"
            ))
        }),
    }
}

fn parse_requirements_param(command: &Command) -> Result<Vec<Requirement>, DispatchError> {
    let value = command.params.get("requirements").cloned().ok_or_else(|| {
        DispatchError::InvalidParams("params.requirements is required".to_string())
    })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.requirements is not a valid Vec<Requirement>: {e}"
        ))
    })
}

fn parse_acceptance_checks_param(command: &Command) -> Result<Vec<AcceptanceCheck>, DispatchError> {
    let value = command
        .params
        .get("acceptance_checks")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams("params.acceptance_checks is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.acceptance_checks is not a valid Vec<AcceptanceCheck>: {e}"
        ))
    })
}

fn parse_contract_amendment_param(command: &Command) -> Result<ContractAmendment, DispatchError> {
    let value =
        command.params.get("amendment").cloned().ok_or_else(|| {
            DispatchError::InvalidParams("params.amendment is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.amendment is not a valid ContractAmendment: {e}"
        ))
    })
}

fn parse_graph_nodes_param(command: &Command) -> Result<Vec<GraphNode>, DispatchError> {
    let value = command
        .params
        .get("nodes")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.nodes is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.nodes is not a valid Vec<GraphNode>: {e}"))
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

    fn ready_project_state_json() -> serde_json::Value {
        json!({
            "lifecycle": "Active",
            "phase": "Ready",
            "hold": "None",
            "revision": 1,
        })
    }

    fn task_created_command(aggregate_id: &str) -> Command {
        command(
            "task.created",
            json!({
                "aggregate_id": aggregate_id,
                "project": ready_project_state_json(),
                "project_id": "project-1",
                "project_revision": 1,
                "original_request_ref": "original-request-ref-1",
            }),
        )
    }

    #[test]
    fn task_created_appends_a_draft_task() {
        let (mut store, path) = temp_store();
        let cmd = task_created_command("task-1");
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "task-1");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "Created");
        let (revision, state) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_created_forwards_invalid_project_state_as_invalid_params() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "task.created",
            json!({
                "aggregate_id": "task-1",
                "project": { "not": "a project state" },
                "project_id": "project-1",
                "project_revision": 1,
                "original_request_ref": "original-request-ref-1",
            }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_created_forwards_project_not_ready_as_task_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "task.created",
            json!({
                "aggregate_id": "task-1",
                "project": {
                    "lifecycle": "Active",
                    "phase": "Registered",
                    "hold": "None",
                    "revision": 1,
                },
                "project_id": "project-1",
                "project_revision": 1,
                "original_request_ref": "original-request-ref-1",
            }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::TaskStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_cancelled_cancels_a_draft_task() {
        use autome_domain::task::TaskLifecycle;

        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command("task.cancelled", json!({ "aggregate_id": "task-1" }));
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "Cancelled");
        let (_, state) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(state.lifecycle, TaskLifecycle::Cancelled);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_run_terminal_applied_completes_the_task_on_a_completed_terminal() {
        use autome_domain::task::TaskLifecycle;

        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.run_terminal_applied",
            json!({ "aggregate_id": "task-1", "run_terminal": "Completed" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "RunTerminalApplied");
        let (_, state) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(state.lifecycle, TaskLifecycle::Completed);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_run_terminal_applied_rejects_unknown_terminal_string() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.run_terminal_applied",
            json!({ "aggregate_id": "task-1", "run_terminal": "sideways" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_run_state_projected_refreshes_the_status_projection() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.run_state_projected",
            json!({
                "aggregate_id": "task-1",
                "run_state": {
                    "phase": "GraphReview",
                    "hold": "None",
                    "terminal": "None",
                },
            }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "RunStateProjected");
        let (_, state) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(
            state.status_projection.phase,
            autome_domain::run::RunPhase::GraphReview
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_run_state_projected_rejects_missing_run_state() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.run_state_projected",
            json!({ "aggregate_id": "task-1" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_dispatch_state_projected_refreshes_dispatch_state_and_queue_entry() {
        use autome_domain::task::DispatchState;

        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.dispatch_state_projected",
            json!({
                "aggregate_id": "task-1",
                "dispatch_state": "Queued",
                "queue_entry": {
                    "enqueued_event_seq": 3,
                    "projected_position": 1,
                    "blocked_by_task_id": "task-0",
                },
            }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "DispatchStateProjected");
        let (_, state) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(state.dispatch_state, DispatchState::Queued);
        assert_eq!(
            state.queue_entry.unwrap().blocked_by_task_id,
            Some("task-0".to_string())
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_dispatch_state_projected_treats_a_missing_queue_entry_as_none() {
        use autome_domain::task::DispatchState;

        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.dispatch_state_projected",
            json!({ "aggregate_id": "task-1", "dispatch_state": "Running" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "DispatchStateProjected");
        let (_, state) = store.load_task_state("task-1").unwrap().unwrap();
        assert_eq!(state.dispatch_state, DispatchState::Running);
        assert!(state.queue_entry.is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_dispatch_state_projected_rejects_an_unknown_dispatch_state_string() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.dispatch_state_projected",
            json!({ "aggregate_id": "task-1", "dispatch_state": "sideways" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_dispatch_state_projected_on_a_never_created_task_surfaces_as_task_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "task.dispatch_state_projected",
            json!({ "aggregate_id": "task-1", "dispatch_state": "None" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::TaskStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_cancelled_on_a_never_created_task_surfaces_as_task_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command("task.cancelled", json!({ "aggregate_id": "task-1" }));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::TaskStore(_)));
        std::fs::remove_file(&path).ok();
    }

    fn requirement_json(id: &str, check_id: &str) -> serde_json::Value {
        json!({
            "id": id,
            "statement": "does something",
            "kind": "Functional",
            "necessity": "Must",
            "source_anchors": [{ "anchor_ref": "raw_text:0-10" }],
            "acceptance_logic": null,
            "acceptance_check_ids": [check_id],
            "delivery_spec": null,
            "risk_level": "Low",
            "superseded_by": null,
        })
    }

    fn acceptance_check_json(id: &str, requirement_id: &str, mandatory: bool) -> serde_json::Value {
        json!({
            "id": id,
            "kind": "Process",
            "requirement_id": requirement_id,
            "mandatory": mandatory,
            "expected_observation": "exit code 0",
            "negative_scenario": "non-zero exit",
            "required_environment_level": "base",
            "isolation_policy": "worktree",
            "repeat_policy": "once",
            "inventory_policy": "track",
            "freshness_policy": "must-be-current",
        })
    }

    fn contract_created_command(aggregate_id: &str) -> Command {
        command(
            "contract.created",
            json!({
                "aggregate_id": aggregate_id,
                "content_hash": "hash-1",
                "requirements": [requirement_json("R-001", "C-001")],
                "acceptance_checks": [acceptance_check_json("C-001", "R-001", true)],
            }),
        )
    }

    #[test]
    fn contract_created_appends_a_draft_contract() {
        use autome_domain::contract::ContractStatus;

        let (mut store, path) = temp_store();
        let cmd = contract_created_command("contract-1");
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "contract-1");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "Created");
        let (revision, state) = store.load_contract_state("contract-1").unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(state.status, ContractStatus::Draft);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_created_forwards_invalid_requirements_as_invalid_params() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "contract.created",
            json!({
                "aggregate_id": "contract-1",
                "content_hash": "hash-1",
                "requirements": [{ "not": "a requirement" }],
                "acceptance_checks": [],
            }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_created_on_an_existing_contract_surfaces_as_contract_store_error() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &contract_created_command("contract-1")).unwrap();
        let err = dispatch(&mut store, &contract_created_command("contract-1")).unwrap_err();
        assert!(matches!(err, DispatchError::ContractStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_frozen_freezes_a_well_formed_draft() {
        use autome_domain::contract::ContractStatus;

        let (mut store, path) = temp_store();
        dispatch(&mut store, &contract_created_command("contract-1")).unwrap();
        let cmd = command("contract.frozen", json!({ "aggregate_id": "contract-1" }));
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "Frozen");
        let (_, state) = store.load_contract_state("contract-1").unwrap().unwrap();
        assert_eq!(state.status, ContractStatus::Frozen);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_frozen_forwards_freeze_violations_as_contract_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "contract.created",
            json!({
                "aggregate_id": "contract-1",
                "content_hash": "hash-1",
                "requirements": [requirement_json("R-001", "C-001")],
                "acceptance_checks": [acceptance_check_json("C-001", "R-001", false)],
            }),
        );
        dispatch(&mut store, &cmd).unwrap();
        let cmd = command("contract.frozen", json!({ "aggregate_id": "contract-1" }));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::ContractStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_frozen_on_a_never_created_contract_surfaces_as_contract_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command("contract.frozen", json!({ "aggregate_id": "contract-1" }));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::ContractStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_amended_produces_a_new_draft_version() {
        use autome_domain::contract::{ContractStatus, ContractVersion};

        let (mut store, path) = temp_store();
        dispatch(&mut store, &contract_created_command("contract-1")).unwrap();
        dispatch(
            &mut store,
            &command("contract.frozen", json!({ "aggregate_id": "contract-1" })),
        )
        .unwrap();
        let cmd = command(
            "contract.amended",
            json!({
                "aggregate_id": "contract-1",
                "amendment": {
                    "base_version": 1,
                    "reason": "scope grew",
                    "user_decision_ref": "decision-1",
                    "change": { "AddRequirement": requirement_json("R-002", "C-002") },
                },
            }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "Amended");
        let (_, state) = store.load_contract_state("contract-1").unwrap().unwrap();
        assert_eq!(state.status, ContractStatus::Draft);
        assert_eq!(state.version, ContractVersion(2));
        assert_eq!(state.requirements.len(), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn contract_amended_rejects_missing_amendment_param() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &contract_created_command("contract-1")).unwrap();
        dispatch(
            &mut store,
            &command("contract.frozen", json!({ "aggregate_id": "contract-1" })),
        )
        .unwrap();
        let cmd = command("contract.amended", json!({ "aggregate_id": "contract-1" }));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    fn graph_node_json(id: &str, purpose: &str, requirement_ids: &[&str]) -> serde_json::Value {
        json!({
            "id": id,
            "kind": "generic",
            "purpose": purpose,
            "title": id,
            "requirement_ids": requirement_ids,
            "acceptance_check_ids": [],
            "depends_on": [],
            "expected_outputs": [],
            "write_scope": [],
            "risk_level": "Low",
            "estimated_budget": 1,
        })
    }

    fn graph_created_command(aggregate_id: &str) -> Command {
        command(
            "graph.created",
            json!({
                "aggregate_id": aggregate_id,
                "contract_ref": "contract-1",
                "graph_hash": "hash-1",
                "nodes": [graph_node_json("N-1", "Business", &[])],
            }),
        )
    }

    #[test]
    fn graph_created_appends_a_version_one_graph() {
        let (mut store, path) = temp_store();
        let cmd = graph_created_command("graph-1");
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "graph-1");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "Created");
        let (revision, state) = store.load_graph_state("graph-1").unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(state.version, 1);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_created_forwards_invalid_nodes_as_invalid_params() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "graph.created",
            json!({
                "aggregate_id": "graph-1",
                "contract_ref": "contract-1",
                "graph_hash": "hash-1",
                "nodes": [{ "not": "a node" }],
            }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_created_on_an_existing_graph_surfaces_as_graph_store_error() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &graph_created_command("graph-1")).unwrap();
        let err = dispatch(&mut store, &graph_created_command("graph-1")).unwrap_err();
        assert!(matches!(err, DispatchError::GraphStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_replaced_bumps_version_and_swaps_nodes() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &graph_created_command("graph-1")).unwrap();
        let cmd = command(
            "graph.replaced",
            json!({
                "aggregate_id": "graph-1",
                "graph_hash": "hash-2",
                "nodes": [graph_node_json("N-2", "Business", &[])],
            }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "Replaced");
        let (_, state) = store.load_graph_state("graph-1").unwrap().unwrap();
        assert_eq!(state.version, 2);
        assert_eq!(state.graph_hash, "hash-2");
        assert_eq!(state.nodes.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_replaced_on_a_never_created_graph_surfaces_as_graph_store_error() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "graph.replaced",
            json!({
                "aggregate_id": "graph-1",
                "graph_hash": "hash-2",
                "nodes": [],
            }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::GraphStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn graph_replaced_rejects_missing_nodes_param() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &graph_created_command("graph-1")).unwrap();
        let cmd = command(
            "graph.replaced",
            json!({ "aggregate_id": "graph-1", "graph_hash": "hash-2" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }
}
