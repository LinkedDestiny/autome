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
//! `ExecutionQueue` is a fleet-wide singleton (see `store::EXECUTION_QUEUE_AGGREGATE_ID`),
//! so its four `queue.*` methods take no `aggregate_id` param at all — unlike
//! every method above, `require_aggregate_id` is never called for them.
//! `queue.enqueue` parses `params.task_id` and `params.enqueued_event_seq`
//! (a non-negative integer); `queue.try_acquire_lease` parses `params.task_id`
//! and `params.lease_id`; `queue.release_via_safe_park` parses `params.receipt`
//! (a full `SafeParkReceipt`); `queue.cancel` parses `params.task_id`.
//! `workspace.create_disposable_clone` (§8.1) follows `project.register_target`'s
//! shape exactly: handled in `handle_command` ahead of both `try_dispatch_read`
//! and `dispatch`, since it writes a `run_workspaces` row but appends no
//! `Event`. Parses `params.task_id`, `params.run_id`, `params.source_repo`
//! (an OS path to a real git repo); its read counterpart `workspace.get`
//! parses `params.task_id`/`params.run_id`.
//! `attempt.record` (§5.6) follows the same shape again: writes an
//! `attempts` row but appends no `Event`, since a recorded Attempt is a
//! fact fixed before a step runs, not an aggregate with a reducer. Parses
//! `params.run_id`, `params.attempt` (a full `autome_domain::attempt::Attempt`)
//! and `params.permission_profile` (a full `AttemptPermissionProfile`); its
//! read counterpart `attempt.get` parses `params.attempt_id`.
//! `evidence.record` (§5.7) is the same shape again: writes an
//! `evidence_receipts` row but appends no `Event`. Parses `params.receipt`
//! (a full `autome_domain::evidence::EvidenceReceipt`); its read counterpart
//! `evidence.get` parses `params.receipt_id`. `evidence.check` re-exercises
//! `EvidenceReceipt::is_valid_against` against a caller-supplied *current*
//! fingerprint (`params.receipt_id`, `params.fingerprint`) rather than
//! trusting the caller's own staleness judgment.

use crate::ipc::{Command, Event, Reply, ReplyErrorCode, ReplyOutcome};
use crate::store::{
    AppendError, AppendedContractEvent, AppendedExecutionQueueEvent, AppendedGraphEvent,
    AppendedProjectEvent, AppendedRunEvent, AppendedTaskEvent, AttemptRecord, ContractAppendError,
    CreateDisposableCloneError, CreateFromTargetError, DisposableCloneRecord,
    EXECUTION_QUEUE_AGGREGATE_ID, EventStore, EvidenceRecord, ExecutionQueueAppendError,
    GraphAppendError, ProjectAppendError, ProjectSummary, RecordAttemptError, RecordEvidenceError,
    TaskAppendError, TaskSummary,
};
use autome_domain::attempt::{Attempt, AttemptPermissionProfile};
use autome_domain::evidence::{EvidenceFingerprint, EvidenceReceipt};
use autome_domain::contract::{AcceptanceCheck, ContractAmendment, ContractEvent};
use autome_domain::execution_queue::{ExecutionQueue, ExecutionQueueEvent};
use autome_domain::graph::{GraphEvent, GraphNode};
use autome_domain::project::{
    ProjectEvent, ProjectIdentity, ProjectIdentityError, ProjectKind, ProjectLocator, ProjectState,
};
use autome_domain::requirement::Requirement;
use autome_domain::run::{GraphReviewOrigin, ReadinessOrigin, RunEvent, RunState, RunTerminal};
use autome_domain::safe_park::SafeParkReceipt;
use autome_domain::task::{DispatchState, QueueEntry, TaskEvent};
use serde_json::Value;
use std::path::Path;

use crate::target_probe::{self, TargetProbeError};

#[derive(Debug)]
pub enum DispatchError {
    UnknownMethod(String),
    InvalidParams(String),
    Store(AppendError),
    ProjectStore(ProjectAppendError),
    TaskStore(TaskAppendError),
    ContractStore(ContractAppendError),
    GraphStore(GraphAppendError),
    ExecutionQueueStore(ExecutionQueueAppendError),
    /// `ProjectIdentity::new`'s own validation (kind/locator mismatch,
    /// blank id/display_name/project_home/locator field) rejected the
    /// `project.create` params before any store call was even made.
    ProjectIdentity(ProjectIdentityError),
    /// §8.2 write path: `create_project_from_target`'s failure modes,
    /// matched exhaustively below rather than flattened into one
    /// `DispatchError` variant per case -- mirroring how `ProjectStore`/
    /// `TaskStore`/etc. above already nest their own multi-case store
    /// errors instead of each getting a flat variant.
    CreateFromTarget(CreateFromTargetError),
    /// A `project.register_target` `path` param that failed
    /// `target_probe::probe_target` (not found, a symlink, an io error
    /// resolving it, `git` missing from `PATH`, etc.) -- always the
    /// caller's fault (a bad or since-removed path), never an internal
    /// failure.
    TargetProbe(TargetProbeError),
    /// §8.1 write path: `create_disposable_clone_for_run`'s failure modes
    /// (a `git` failure, or `fs_guard`'s owner-only discipline rejecting
    /// something under `runs/`), matched exhaustively below rather than
    /// flattened -- same reasoning as `CreateFromTarget` above.
    CreateDisposableClone(CreateDisposableCloneError),
    /// §5.6 write path: `record_attempt`'s failure modes (a shape mismatch
    /// between `purpose` and `spec_binding`, a self-contradictory
    /// `AttemptPermissionProfile`, or a Planning attempt paired with a
    /// write-granting profile), matched exhaustively below -- same
    /// reasoning as `CreateDisposableClone` above.
    RecordAttempt(RecordAttemptError),
    /// §5.7 write path: `record_evidence`'s only failure mode (a duplicate
    /// `receipt_id`, surfaced as a SQL primary-key violation).
    RecordEvidence(RecordEvidenceError),
    /// §8.3:1362 diagnostic state: `EventStore::open`'s own disk-layout
    /// re-verification failed and every write is refused until the
    /// underlying filesystem problem is fixed. Read methods are
    /// unaffected -- they never reach `dispatch` at all, see
    /// `try_dispatch_read`.
    Diagnostic(String),
}

impl From<CreateFromTargetError> for DispatchError {
    fn from(value: CreateFromTargetError) -> Self {
        DispatchError::CreateFromTarget(value)
    }
}

impl From<TargetProbeError> for DispatchError {
    fn from(value: TargetProbeError) -> Self {
        DispatchError::TargetProbe(value)
    }
}

impl From<CreateDisposableCloneError> for DispatchError {
    fn from(value: CreateDisposableCloneError) -> Self {
        DispatchError::CreateDisposableClone(value)
    }
}

impl From<RecordAttemptError> for DispatchError {
    fn from(value: RecordAttemptError) -> Self {
        DispatchError::RecordAttempt(value)
    }
}

impl From<RecordEvidenceError> for DispatchError {
    fn from(value: RecordEvidenceError) -> Self {
        DispatchError::RecordEvidence(value)
    }
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

impl From<ProjectIdentityError> for DispatchError {
    fn from(value: ProjectIdentityError) -> Self {
        DispatchError::ProjectIdentity(value)
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

impl From<ExecutionQueueAppendError> for DispatchError {
    fn from(value: ExecutionQueueAppendError) -> Self {
        DispatchError::ExecutionQueueStore(value)
    }
}

pub fn dispatch(store: &mut EventStore, command: &Command) -> Result<Event, DispatchError> {
    // §8.3:1362: once the store has flagged itself diagnostic (its own
    // owner-only disk layout failed re-verification), every write method
    // below is refused outright, unconditionally, before any params are
    // even parsed. Read methods never reach this function at all -- see
    // `try_dispatch_read`, checked first by `handle_command`.
    if let Some(reason) = store.diagnostic_reason() {
        return Err(DispatchError::Diagnostic(reason.to_string()));
    }
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
        "queue.enqueue" => {
            let task_id = parse_string_param(command, "task_id")?;
            let enqueued_event_seq = parse_u64_param(command, "enqueued_event_seq")?;
            let appended = store.append_execution_queue_event(ExecutionQueueEvent::Enqueued {
                task_id,
                enqueued_event_seq,
            })?;
            return Ok(execution_queue_event_envelope(appended));
        }
        "queue.try_acquire_lease" => {
            let task_id = parse_string_param(command, "task_id")?;
            let lease_id = parse_string_param(command, "lease_id")?;
            let appended =
                store.append_execution_queue_event(ExecutionQueueEvent::LeaseAcquired {
                    task_id,
                    lease_id,
                })?;
            return Ok(execution_queue_event_envelope(appended));
        }
        "queue.release_via_safe_park" => {
            let receipt = parse_safe_park_receipt_param(command)?;
            let appended = store.append_execution_queue_event(
                ExecutionQueueEvent::LeaseReleasedViaSafePark { receipt },
            )?;
            return Ok(execution_queue_event_envelope(appended));
        }
        "queue.cancel" => {
            let task_id = parse_string_param(command, "task_id")?;
            let appended =
                store.append_execution_queue_event(ExecutionQueueEvent::Cancelled { task_id })?;
            return Ok(execution_queue_event_envelope(appended));
        }
        // The original, low-level primitive: takes a raw `locator` object
        // straight from `params`. Kept for tests and any future CLI, but
        // Main never sends this over the wire -- `ipc-gate.js`'s write
        // whitelist and `write-gate.js` (§8.2) never admit it either,
        // since a nested `locator` object can't pass the scalar-only
        // param rule and, more fundamentally, the path it's derived from
        // would have had to come from somewhere Main isn't allowed to
        // trust (see this module's own doc comment on that restriction).
        // Real project creation goes through `project.create_from_target`
        // below instead.
        "project.create" => {
            let aggregate_id = require_aggregate_id(command)?;
            let display_name = parse_string_param(command, "display_name")?;
            let kind = parse_project_kind(command)?;
            let locator = parse_project_locator(command)?;
            let project_home = store.project_home_for(&aggregate_id);
            let identity =
                ProjectIdentity::new(&aggregate_id, &display_name, kind, locator, &project_home)?;
            let appended = store.create_project(&identity)?;
            return Ok(project_event_envelope(aggregate_id, appended));
        }
        // §8.2 phase ⑤: the only creation path Main actually uses. Takes
        // nothing but a previously-registered `target_id` (see
        // `handle_command`'s `project.register_target` branch, the only
        // way to obtain one) plus scalars -- the `ProjectLocator` is
        // derived by `create_project_from_target` itself from the target
        // record it already persisted, never read back out of `params`.
        "project.create_from_target" => {
            let target_id = parse_string_param(command, "target_id")?;
            let display_name = parse_string_param(command, "display_name")?;
            let trust_confirmed = parse_bool_param(command, "trust_confirmed")?;
            let destination_name = parse_optional_string_param(command, "destination_name")?;
            let created = store.create_project_from_target(
                &target_id,
                &display_name,
                trust_confirmed,
                destination_name.as_deref(),
            )?;
            return Ok(project_event_envelope(created.project_id, created.appended));
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

/// Every `Command` produces exactly one `Reply`; state-changing commands
/// additionally produce an `Event`. This wraps `dispatch` (left entirely
/// unchanged above, including its `Result<Event, DispatchError>` signature
/// and its 64 existing test call sites) rather than replacing it, so the
/// write path's behavior and test coverage are untouched by this addition.
pub struct DispatchOutcome {
    pub reply: Reply,
    pub event: Option<Event>,
}

/// Builds the `ReplyOutcome` for a command that produces a `Value`
/// payload and never an `Event` -- shared by the read branch below and
/// by `project.register_target`, the one *write* that also takes this
/// shape (see `handle_register_target`'s own doc comment for why).
fn value_outcome(
    store: &EventStore,
    result: Result<Value, (ReplyErrorCode, String)>,
) -> ReplyOutcome {
    let snapshot_seq = store.latest_event_seq().unwrap_or(0);
    match result {
        Ok(payload) => ReplyOutcome::Ok {
            snapshot_seq,
            payload,
        },
        Err((code, message)) => ReplyOutcome::Error { code, message },
    }
}

pub fn handle_command(store: &mut EventStore, command: &Command) -> DispatchOutcome {
    // §8.2 phase ③: handled here, ahead of both `try_dispatch_read` and
    // `dispatch`, because it fits neither -- it writes a `project_targets`
    // row (so it isn't a read) but appends no `ProjectEvent` (there is no
    // project aggregate yet to append one to), so it can't fit `dispatch`'s
    // `Result<Event, DispatchError>` shape either. This is the only place
    // in the whole dispatch layer that calls `handle_register_target`.
    if command.method == "project.register_target" {
        let result = handle_register_target(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §8.1: same shape as `project.register_target` above -- writes a
    // `run_workspaces` row (so it isn't a read) but appends no domain
    // `Event` (a disposable clone is a recorded fact, not an aggregate
    // with a reducer), so it can't fit `dispatch`'s `Result<Event, _>`
    // shape either.
    if command.method == "workspace.create_disposable_clone" {
        let result = handle_create_disposable_clone(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.6: same shape as `workspace.create_disposable_clone` above --
    // writes an `attempts` row (so it isn't a read) but appends no domain
    // `Event` (an Attempt is a fact fixed once before a step runs, not an
    // aggregate with a reducer), so it can't fit `dispatch`'s
    // `Result<Event, _>` shape either.
    if command.method == "attempt.record" {
        let result = handle_record_attempt(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.7: same shape again -- writes an `evidence_receipts` row (so it
    // isn't a read) but appends no domain `Event` (a receipt is a fact
    // fixed once by a verifier run, not an aggregate with a reducer).
    if command.method == "evidence.record" {
        let result = handle_record_evidence(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    if let Some(result) = try_dispatch_read(store, command) {
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    match dispatch(store, command) {
        Ok(event) => {
            let snapshot_seq = store.latest_event_seq().unwrap_or(event.event_seq);
            DispatchOutcome {
                reply: reply_for(
                    command,
                    ReplyOutcome::Ok {
                        snapshot_seq,
                        payload: event.payload.clone(),
                    },
                ),
                event: Some(event),
            }
        }
        Err(err) => {
            let (code, message) = dispatch_error_to_reply_error(err);
            DispatchOutcome {
                reply: reply_for(command, ReplyOutcome::Error { code, message }),
                event: None,
            }
        }
    }
}

fn reply_for(command: &Command, outcome: ReplyOutcome) -> Reply {
    Reply {
        request_id: command.request_id.clone(),
        command_id: command.command_id.clone(),
        protocol_version: command.protocol_version,
        outcome,
    }
}

/// §8.2 phase ③: the sole caller of `EventStore::register_target`. Takes
/// `{ kind, path }` -- `path` is an OS path Main obtained itself via its
/// own `dialog.showOpenDialog` (see this module's doc comment on the
/// broader "no Renderer/Agent-supplied path" rule); it is probed, the
/// result persisted as a new `project_targets` row, and only
/// `{ target_id, summary }` is handed back -- the `canonical_path` inside
/// `summary` is the one place a resolved path is allowed to flow back
/// *out* to a Renderer, since the rule constrains what can flow in, not
/// what can be displayed (see the design note in the governing plan).
/// Refuses to run while the store is in its diagnostic state, same as
/// every write in `dispatch`.
fn handle_register_target(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let kind = parse_project_kind(command).map_err(dispatch_error_to_reply_error)?;
    let path = parse_string_param(command, "path").map_err(dispatch_error_to_reply_error)?;
    let probe = target_probe::probe_target(Path::new(&path))
        .map_err(|e| (ReplyErrorCode::InvalidParams, e.to_string()))?;
    // `destination_absent` isn't knowable yet at registration time: for
    // `NewProduct` the destination name hasn't been typed by the user yet,
    // and for `ExistingRepository` it is always recomputed as `false`
    // regardless (see `create_project_from_target`). `true` here is a
    // documented placeholder -- `TargetRecord`'s own doc comment already
    // says this field must never be trusted from what gets persisted at
    // registration time; it is always re-derived fresh at creation time.
    let inspection = probe.to_inspection(true);
    let target_id = store
        .register_target(kind, &probe, &inspection)
        .map_err(internal_error)?;
    Ok(serde_json::json!({
        "target_id": target_id,
        "summary": {
            "kind": kind,
            "canonical_path": probe.canonical_path.to_string_lossy(),
            "is_git_repo": inspection.is_git_repo,
            "head_resolvable": inspection.head_resolvable,
            "worktree_clean": inspection.worktree_clean,
        },
    }))
}

/// §8.1: the sole caller of `EventStore::create_disposable_clone_for_run`.
/// Takes `{ task_id, run_id, source_repo }` -- `source_repo` is an OS path
/// to an already-materialized repository (a ProjectHome's worktree, in the
/// real flow this will eventually be driven from; any absolute path
/// pointing at a real git repo works for now, since the caller identity
/// that path came from is out of scope for this increment). Clones it into
/// `runs/<task_id>/<run_id>/repo` via `workspace::create_disposable_clone`
/// and records the result. Refuses to run while the store is in its
/// diagnostic state, same as every write in `dispatch` and
/// `handle_register_target`.
fn handle_create_disposable_clone(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let task_id = parse_string_param(command, "task_id").map_err(dispatch_error_to_reply_error)?;
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let source_repo =
        parse_string_param(command, "source_repo").map_err(dispatch_error_to_reply_error)?;
    let record = store
        .create_disposable_clone_for_run(&task_id, &run_id, Path::new(&source_repo))
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(disposable_clone_record_json(&record))
}

fn disposable_clone_record_json(record: &DisposableCloneRecord) -> Value {
    serde_json::json!({
        "task_id": record.task_id,
        "run_id": record.run_id,
        "repo_path": record.repo_path,
        "head_commit": record.head_commit,
        "created_at": record.created_at,
    })
}

/// §5.6: the sole caller of `EventStore::record_attempt`. Takes
/// `{ run_id, attempt, permission_profile }` -- `attempt`/`permission_profile`
/// are full JSON objects matching `autome_domain::attempt::{Attempt,
/// AttemptPermissionProfile}`. Same "write returns Value not Event" shape
/// as `handle_register_target`/`handle_create_disposable_clone`: a recorded
/// Attempt is a fact fixed before the step runs, not a state-machine
/// transition. Refuses to run while the store is in its diagnostic state,
/// same as every other write.
fn handle_record_attempt(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let attempt = parse_attempt_param(command).map_err(dispatch_error_to_reply_error)?;
    let permission_profile =
        parse_attempt_permission_profile_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_attempt(&run_id, &attempt, &permission_profile)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(attempt_record_json(&record))
}

fn attempt_record_json(record: &AttemptRecord) -> Value {
    serde_json::json!({
        "run_id": record.run_id,
        "attempt": record.attempt,
        "permission_profile": record.permission_profile,
        "created_at": record.created_at,
    })
}

fn parse_attempt_param(command: &Command) -> Result<Attempt, DispatchError> {
    let value = command
        .params
        .get("attempt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.attempt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.attempt is not a valid Attempt: {e}"))
    })
}

fn parse_attempt_permission_profile_param(
    command: &Command,
) -> Result<AttemptPermissionProfile, DispatchError> {
    let value = command
        .params
        .get("permission_profile")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams("params.permission_profile is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.permission_profile is not a valid AttemptPermissionProfile: {e}"
        ))
    })
}

/// Handles `evidence.record`: `{ receipt: EvidenceReceipt }`. Same
/// "write returns Value not Event" shape as `handle_record_attempt` --
/// a recorded receipt is a fact fixed once by a verifier run, not a
/// state-machine transition. Refuses to run while the store is in its
/// diagnostic state, same as every other write.
fn handle_record_evidence(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let receipt = parse_evidence_receipt_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_evidence(&receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(evidence_record_json(&record))
}

fn evidence_record_json(record: &EvidenceRecord) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "created_at": record.created_at,
    })
}

fn parse_evidence_receipt_param(command: &Command) -> Result<EvidenceReceipt, DispatchError> {
    let value = command
        .params
        .get("receipt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.receipt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.receipt is not a valid EvidenceReceipt: {e}"))
    })
}

fn parse_evidence_fingerprint_param(
    command: &Command,
) -> Result<EvidenceFingerprint, DispatchError> {
    let value = command.params.get("fingerprint").cloned().ok_or_else(|| {
        DispatchError::InvalidParams("params.fingerprint is required".to_string())
    })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.fingerprint is not a valid EvidenceFingerprint: {e}"
        ))
    })
}

/// Maps every `DispatchError` variant to a `ReplyErrorCode`. `Sql(_)`
/// variants (genuine I/O/internal failures) become `Internal`;
/// `Transition(_)` variants (a reducer rejecting the event given the
/// aggregate's current state — a well-formed, expected rejection, not a
/// bug) become `TransitionRejected`, kept distinct from `Internal` so real
/// failures don't get lost among ordinary domain-rule rejections.
fn dispatch_error_to_reply_error(err: DispatchError) -> (ReplyErrorCode, String) {
    match err {
        DispatchError::UnknownMethod(method) => (
            ReplyErrorCode::UnknownMethod,
            format!("unknown method: {method}"),
        ),
        DispatchError::InvalidParams(message) => (ReplyErrorCode::InvalidParams, message),
        DispatchError::Store(AppendError::Sql(e)) => (ReplyErrorCode::Internal, e.to_string()),
        DispatchError::Store(AppendError::Transition(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::ProjectStore(ProjectAppendError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::ProjectStore(ProjectAppendError::Transition(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::ProjectStore(ProjectAppendError::AlreadyExists) => (
            ReplyErrorCode::ProtocolViolation,
            "a project with this aggregate_id already exists".to_string(),
        ),
        DispatchError::ProjectStore(ProjectAppendError::NotFound) => (
            ReplyErrorCode::NotFound,
            "no project has been created with this aggregate_id".to_string(),
        ),
        DispatchError::ProjectIdentity(e) => (ReplyErrorCode::InvalidParams, format!("{e:?}")),
        DispatchError::TaskStore(TaskAppendError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::TaskStore(TaskAppendError::Transition(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::ContractStore(ContractAppendError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::ContractStore(ContractAppendError::Transition(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::GraphStore(GraphAppendError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::GraphStore(GraphAppendError::Transition(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::ExecutionQueueStore(ExecutionQueueAppendError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::ExecutionQueueStore(ExecutionQueueAppendError::Transition(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::CreateFromTarget(CreateFromTargetError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::CreateFromTarget(CreateFromTargetError::TargetNotFound) => (
            ReplyErrorCode::NotFound,
            "no registered target with this target_id".to_string(),
        ),
        DispatchError::CreateFromTarget(CreateFromTargetError::TargetAlreadyConsumed) => (
            ReplyErrorCode::InvalidParams,
            "this target_id has already been used to create a project".to_string(),
        ),
        DispatchError::CreateFromTarget(CreateFromTargetError::TrustNotConfirmed) => (
            ReplyErrorCode::InvalidParams,
            "existing_repository targets require trust_confirmed: true".to_string(),
        ),
        DispatchError::CreateFromTarget(CreateFromTargetError::Rejected(rejection)) => (
            ReplyErrorCode::InvalidParams,
            format!("target rejected: {rejection:?}"),
        ),
        DispatchError::CreateFromTarget(CreateFromTargetError::Identity(e)) => {
            (ReplyErrorCode::InvalidParams, format!("{e:?}"))
        }
        DispatchError::CreateFromTarget(CreateFromTargetError::FsGuard(e)) => {
            (ReplyErrorCode::InvalidParams, e.to_string())
        }
        DispatchError::TargetProbe(e) => (ReplyErrorCode::InvalidParams, e.to_string()),
        DispatchError::CreateDisposableClone(CreateDisposableCloneError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::CreateDisposableClone(CreateDisposableCloneError::Workspace(e)) => {
            (ReplyErrorCode::InvalidParams, e.to_string())
        }
        DispatchError::RecordAttempt(RecordAttemptError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordAttempt(RecordAttemptError::Shape(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordAttempt(RecordAttemptError::PermissionViolations(v)) => {
            (ReplyErrorCode::TransitionRejected, format!("{v:?}"))
        }
        DispatchError::RecordAttempt(RecordAttemptError::PlanningWrite(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordEvidence(RecordEvidenceError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::Diagnostic(reason) => (ReplyErrorCode::ProtocolViolation, reason),
    }
}

/// The five read-only methods this increment adds. `None` means "not a
/// read method" — the caller falls through to the unchanged write-path
/// `dispatch`. List queries only ever touch projection-table columns
/// (never `state_json`), matching plan §2's point of having projection
/// tables at all.
fn try_dispatch_read(
    store: &EventStore,
    command: &Command,
) -> Option<Result<Value, (ReplyErrorCode, String)>> {
    match command.method.as_str() {
        "project.list" => Some(read_project_list(store)),
        "project.get" => Some(read_project_get(store, command)),
        "task.list" => Some(read_task_list(store, command)),
        "task.get" => Some(read_task_get(store, command)),
        "queue.get" => Some(read_queue_get(store)),
        "workspace.get" => Some(read_workspace_get(store, command)),
        "attempt.get" => Some(read_attempt_get(store, command)),
        "evidence.get" => Some(read_evidence_get(store, command)),
        "evidence.check" => Some(read_evidence_check(store, command)),
        _ => None,
    }
}

fn read_project_list(store: &EventStore) -> Result<Value, (ReplyErrorCode, String)> {
    let summaries = store.list_project_summaries().map_err(internal_error)?;
    Ok(serde_json::json!({
        "projects": summaries
            .into_iter()
            .map(|s: ProjectSummary| {
                serde_json::json!({
                    "id": s.id,
                    "revision": s.revision,
                    "lifecycle": s.lifecycle,
                    "phase": s.phase,
                    "hold": s.hold,
                    "display_name": s.display_name,
                    "kind": s.kind,
                })
            })
            .collect::<Vec<_>>(),
    }))
}

fn read_project_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let aggregate_id = require_aggregate_id(command).map_err(dispatch_error_to_reply_error)?;
    match store
        .load_project_state(&aggregate_id)
        .map_err(internal_error)?
    {
        Some((revision, state)) => {
            let identity = store
                .load_project_identity(&aggregate_id)
                .map_err(internal_error)?;
            Ok(serde_json::json!({
                "id": aggregate_id,
                "revision": revision,
                "state": state,
                "identity": identity,
            }))
        }
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no project with id {aggregate_id}"),
        )),
    }
}

fn read_task_list(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let project_id =
        parse_string_param(command, "project_id").map_err(dispatch_error_to_reply_error)?;
    let summaries = store
        .list_task_summaries(&project_id)
        .map_err(internal_error)?;
    Ok(serde_json::json!({
        "project_id": project_id,
        "tasks": summaries
            .into_iter()
            .map(|t: TaskSummary| {
                serde_json::json!({
                    "id": t.id,
                    "revision": t.revision,
                    "lifecycle": t.lifecycle,
                })
            })
            .collect::<Vec<_>>(),
    }))
}

/// §5.1's cross-project id-leak rule: a `task_id` that exists but belongs
/// to a different `project_id` than the caller asserted is
/// `ProtocolViolation`, never `NotFound` — the two are not
/// interchangeable. `NotFound` means the id doesn't exist at all;
/// `ProtocolViolation` means the caller is doing something the protocol
/// forbids (asserting a project/task pairing that isn't true), which must
/// not be silently downgraded to an ordinary "missing" response.
fn read_task_get(store: &EventStore, command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let task_id = parse_string_param(command, "task_id").map_err(dispatch_error_to_reply_error)?;
    let project_id =
        parse_string_param(command, "project_id").map_err(dispatch_error_to_reply_error)?;
    match store.load_task_state(&task_id).map_err(internal_error)? {
        Some((revision, task)) if task.project_id == project_id => Ok(serde_json::json!({
            "id": task_id,
            "revision": revision,
            "state": task,
        })),
        Some((_, task)) => Err((
            ReplyErrorCode::ProtocolViolation,
            format!(
                "task {task_id} belongs to project {}, not {project_id}",
                task.project_id
            ),
        )),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no task with id {task_id}"),
        )),
    }
}

/// `ExecutionQueue` is a fleet-wide singleton (see
/// `store::EXECUTION_QUEUE_AGGREGATE_ID`) that conceptually always exists —
/// an empty, unleased queue before its first event is a real state, not a
/// missing one, so a store with no queue events yet reads back as revision
/// 0 / `ExecutionQueue::default()` rather than `NotFound`.
fn read_queue_get(store: &EventStore) -> Result<Value, (ReplyErrorCode, String)> {
    let (revision, state) = store
        .load_execution_queue_state()
        .map_err(internal_error)?
        .unwrap_or((0, ExecutionQueue::default()));
    let task_ids = state.known_task_ids();
    let labels = store
        .resolve_task_project_labels(&task_ids)
        .map_err(internal_error)?;
    Ok(serde_json::json!({
        "id": EXECUTION_QUEUE_AGGREGATE_ID,
        "revision": revision,
        "state": state,
        "entry_labels": labels
            .into_iter()
            .map(|l| serde_json::json!({
                "task_id": l.task_id,
                "project_id": l.project_id,
                "project_display_name": l.project_display_name,
            }))
            .collect::<Vec<_>>(),
    }))
}

/// Read counterpart to `handle_create_disposable_clone`. Takes
/// `{ task_id, run_id }`; `NotFound` if no clone has been recorded for that
/// pair yet.
fn read_workspace_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let task_id = parse_string_param(command, "task_id").map_err(dispatch_error_to_reply_error)?;
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_disposable_clone_for_run(&task_id, &run_id)
        .map_err(internal_error)?
    {
        Some(record) => Ok(disposable_clone_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no disposable clone recorded for task {task_id} run {run_id}"),
        )),
    }
}

/// Read counterpart to `handle_record_attempt`. Takes `{ attempt_id }`;
/// `NotFound` if no Attempt has been recorded with that id yet.
fn read_attempt_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let attempt_id =
        parse_string_param(command, "attempt_id").map_err(dispatch_error_to_reply_error)?;
    match store.load_attempt(&attempt_id).map_err(internal_error)? {
        Some(record) => Ok(attempt_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no attempt recorded with id {attempt_id}"),
        )),
    }
}

/// Read counterpart to `handle_record_evidence`. Takes `{ receipt_id }`;
/// `NotFound` if no receipt has been recorded with that id yet.
fn read_evidence_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_id =
        parse_string_param(command, "receipt_id").map_err(dispatch_error_to_reply_error)?;
    match store.load_evidence(&receipt_id).map_err(internal_error)? {
        Some(record) => Ok(evidence_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no evidence receipt recorded with id {receipt_id}"),
        )),
    }
}

/// §5.7: `{ receipt_id, fingerprint }` -- re-exercises
/// `EvidenceReceipt::is_valid_against` against the caller-supplied *current*
/// fingerprint, rather than trusting the caller's own staleness judgment.
/// `NotFound` if `receipt_id` was never recorded (there is nothing to check
/// staleness of).
fn read_evidence_check(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_id =
        parse_string_param(command, "receipt_id").map_err(dispatch_error_to_reply_error)?;
    let fingerprint =
        parse_evidence_fingerprint_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .load_evidence(&receipt_id)
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                ReplyErrorCode::NotFound,
                format!("no evidence receipt recorded with id {receipt_id}"),
            )
        })?;
    let valid = record.receipt.is_valid_against(&fingerprint);
    Ok(serde_json::json!({
        "receipt_id": receipt_id,
        "valid": valid,
    }))
}

fn internal_error(err: rusqlite::Error) -> (ReplyErrorCode, String) {
    (ReplyErrorCode::Internal, err.to_string())
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

/// Mirrors `graph_event_envelope`, except `ExecutionQueue` is a singleton
/// with no caller-chosen id, so this always fills `aggregate_id` with the
/// same fixed `EXECUTION_QUEUE_AGGREGATE_ID` constant the store uses.
fn execution_queue_event_envelope(appended: AppendedExecutionQueueEvent) -> Event {
    Event {
        event_seq: appended.seq as u64,
        event_id: appended.event_id,
        aggregate_id: EXECUTION_QUEUE_AGGREGATE_ID.to_string(),
        aggregate_revision: appended.revision,
        event_type: appended.event_type.to_string(),
        occurred_at: appended.occurred_at,
        payload: serde_json::to_value(appended.state).expect("ExecutionQueue always serializes"),
    }
}

/// The 23 of 26 RunEvent variants that carry no payload. See the module
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

fn parse_bool_param(command: &Command, key: &str) -> Result<bool, DispatchError> {
    command
        .params
        .get(key)
        .and_then(|v| v.as_bool())
        .ok_or_else(|| DispatchError::InvalidParams(format!("params.{key} must be a boolean")))
}

/// Like `parse_string_param`, but the key is allowed to be absent or
/// explicit `null` -- `project.create_from_target`'s `destination_name`
/// is the one caller: required for `NewProduct`, meaningless for
/// `ExistingRepository` (rejected downstream by `locator_for`, not here).
fn parse_optional_string_param(
    command: &Command,
    key: &str,
) -> Result<Option<String>, DispatchError> {
    match command.params.get(key).cloned() {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s)),
        Some(_) => Err(DispatchError::InvalidParams(format!(
            "params.{key} must be a string when present"
        ))),
    }
}

/// String-enum style, matching `parse_run_terminal` below: the variant
/// name verbatim, not a snake_case or kebab-case transform of it.
fn parse_project_kind(command: &Command) -> Result<ProjectKind, DispatchError> {
    match command.params.get("kind").and_then(|v| v.as_str()) {
        Some("NewProduct") => Ok(ProjectKind::NewProduct),
        Some("ExistingRepository") => Ok(ProjectKind::ExistingRepository),
        _ => Err(DispatchError::InvalidParams(
            "params.kind must be one of NewProduct|ExistingRepository".to_string(),
        )),
    }
}

/// Serde-blob style, matching `parse_project_state` above: `ProjectLocator`
/// derives `Deserialize` with serde's default externally-tagged
/// representation, e.g. `{"ExistingRepository":{"repository_identity":"..."}}`.
fn parse_project_locator(command: &Command) -> Result<ProjectLocator, DispatchError> {
    let value =
        command.params.get("locator").cloned().ok_or_else(|| {
            DispatchError::InvalidParams("params.locator is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.locator is not a valid ProjectLocator: {e}"))
    })
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

fn parse_u64_param(command: &Command, key: &str) -> Result<u64, DispatchError> {
    command
        .params
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            DispatchError::InvalidParams(format!("params.{key} must be a non-negative integer"))
        })
}

fn parse_safe_park_receipt_param(command: &Command) -> Result<SafeParkReceipt, DispatchError> {
    let value =
        command.params.get("receipt").cloned().ok_or_else(|| {
            DispatchError::InvalidParams("params.receipt is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.receipt is not a valid SafeParkReceipt: {e}"
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

    fn create_project_command(aggregate_id: &str) -> Command {
        command(
            "project.create",
            json!({
                "aggregate_id": aggregate_id,
                "display_name": format!("Display {aggregate_id}"),
                "kind": "ExistingRepository",
                "locator": {
                    "ExistingRepository": { "repository_identity": format!("repo-{aggregate_id}") }
                },
            }),
        )
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
        dispatch(&mut store, &create_project_command("project-1")).unwrap();
        let cmd = command(
            "project.advance_nominal",
            json!({ "aggregate_id": "project-1" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "project-1");
        assert_eq!(event.aggregate_revision, 2);
        assert_eq!(event.event_type, "AdvanceNominal");
        let (revision, state) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_illegal_transition_surfaces_as_project_store_error() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &create_project_command("project-1")).unwrap();
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

    fn safe_park_receipt_json(lease_id: &str) -> serde_json::Value {
        json!({
            "run": "run-1",
            "planning_spec_hash": "hash-1",
            "execution_spec_hash": null,
            "prior_phase": "Executing",
            "durable_checkpoint": "checkpoint-1",
            "provider_session": null,
            "no_active_tool_call": true,
            "no_verifier_or_preview_process": true,
            "no_environment_skill_git_transaction": true,
            "released_leases": [lease_id],
            "parked_at": "2026-09-14T00:00:00Z",
            "receipt_digest": "digest-1",
        })
    }

    #[test]
    fn queue_enqueue_appends_and_returns_matching_event() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "queue.enqueue",
            json!({ "task_id": "task-1", "enqueued_event_seq": 1 }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "execution-queue");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "Enqueued");
        let (revision, state) = store.load_execution_queue_state().unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn queue_try_acquire_lease_moves_the_head_of_queue_into_the_lease() {
        use autome_domain::task::DispatchState;

        let (mut store, path) = temp_store();
        dispatch(
            &mut store,
            &command(
                "queue.enqueue",
                json!({ "task_id": "task-1", "enqueued_event_seq": 1 }),
            ),
        )
        .unwrap();
        let cmd = command(
            "queue.try_acquire_lease",
            json!({ "task_id": "task-1", "lease_id": "lease-1" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "LeaseAcquired");
        let (_, state) = store.load_execution_queue_state().unwrap().unwrap();
        assert_eq!(state.dispatch_state_of("task-1"), DispatchState::Running);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn queue_try_acquire_lease_rejects_a_task_not_at_the_head_as_execution_queue_store_error() {
        let (mut store, path) = temp_store();
        dispatch(
            &mut store,
            &command(
                "queue.enqueue",
                json!({ "task_id": "task-1", "enqueued_event_seq": 1 }),
            ),
        )
        .unwrap();
        dispatch(
            &mut store,
            &command(
                "queue.enqueue",
                json!({ "task_id": "task-2", "enqueued_event_seq": 2 }),
            ),
        )
        .unwrap();
        let cmd = command(
            "queue.try_acquire_lease",
            json!({ "task_id": "task-2", "lease_id": "lease-1" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::ExecutionQueueStore(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn queue_release_via_safe_park_clears_the_lease() {
        use autome_domain::task::DispatchState;

        let (mut store, path) = temp_store();
        dispatch(
            &mut store,
            &command(
                "queue.enqueue",
                json!({ "task_id": "task-1", "enqueued_event_seq": 1 }),
            ),
        )
        .unwrap();
        dispatch(
            &mut store,
            &command(
                "queue.try_acquire_lease",
                json!({ "task_id": "task-1", "lease_id": "lease-1" }),
            ),
        )
        .unwrap();
        let cmd = command(
            "queue.release_via_safe_park",
            json!({ "receipt": safe_park_receipt_json("lease-1") }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "LeaseReleasedViaSafePark");
        let (_, state) = store.load_execution_queue_state().unwrap().unwrap();
        assert_eq!(state.dispatch_state_of("task-1"), DispatchState::None);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn queue_release_via_safe_park_rejects_a_malformed_receipt_as_invalid_params() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "queue.release_via_safe_park",
            json!({ "receipt": { "not": "a receipt" } }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn queue_cancel_removes_a_queued_task() {
        use autome_domain::task::DispatchState;

        let (mut store, path) = temp_store();
        dispatch(
            &mut store,
            &command(
                "queue.enqueue",
                json!({ "task_id": "task-1", "enqueued_event_seq": 1 }),
            ),
        )
        .unwrap();
        let cmd = command("queue.cancel", json!({ "task_id": "task-1" }));
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "Cancelled");
        let (_, state) = store.load_execution_queue_state().unwrap().unwrap();
        assert_eq!(state.dispatch_state_of("task-1"), DispatchState::None);
        std::fs::remove_file(&path).ok();
    }

    // --- handle_command: Reply-wrapping layer -----------------------------

    #[test]
    fn handle_command_wraps_a_successful_write_with_an_ok_reply_and_an_event() {
        let (mut store, path) = temp_store();
        let cmd = command("run.advance_nominal", json!({ "aggregate_id": "run-1" }));
        let outcome = handle_command(&mut store, &cmd);
        assert_eq!(outcome.reply.request_id, "req-1");
        assert_eq!(outcome.reply.command_id, "cmd-1");
        let event = outcome
            .event
            .expect("a write command must produce an event");
        assert_eq!(event.aggregate_id, "run-1");
        match outcome.reply.outcome {
            ReplyOutcome::Ok {
                snapshot_seq,
                payload,
            } => {
                assert_eq!(snapshot_seq, store.latest_event_seq().unwrap());
                assert_eq!(payload, event.payload);
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_command_wraps_an_unknown_method_as_an_error_reply_with_no_event() {
        let (mut store, path) = temp_store();
        let cmd = command("run.frobnicate", json!({ "aggregate_id": "run-1" }));
        let outcome = handle_command(&mut store, &cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::UnknownMethod),
            other => panic!("expected ReplyOutcome::Error, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_command_maps_a_domain_transition_rejection_to_transition_rejected_not_internal() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &create_project_command("project-1")).unwrap();
        // §6.1's nominal path has 8 phases; Registered is index 0, so 7
        // calls walk it all the way to Ready and the 8th has nowhere left
        // to advance to — a legitimate domain rejection, not a bug.
        for _ in 0..7 {
            dispatch(
                &mut store,
                &command(
                    "project.advance_nominal",
                    json!({ "aggregate_id": "project-1" }),
                ),
            )
            .unwrap();
        }
        let cmd = command(
            "project.advance_nominal",
            json!({ "aggregate_id": "project-1" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => {
                assert_eq!(code, ReplyErrorCode::TransitionRejected)
            }
            other => panic!("expected ReplyOutcome::Error, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn handle_command_wraps_a_read_method_without_producing_an_event() {
        let (mut store, path) = temp_store();
        let cmd = command("project.list", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Ok {
                snapshot_seq,
                payload,
            } => {
                assert_eq!(snapshot_seq, 0);
                assert_eq!(payload, json!({ "projects": [] }));
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    // --- project.list / project.get ----------------------------------------

    #[test]
    fn project_list_is_empty_on_a_fresh_store() {
        let (store, path) = temp_store();
        let summaries = store.list_project_summaries().unwrap();
        assert!(summaries.is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_list_returns_every_project_ordered_by_id() {
        let (mut store, path) = temp_store();
        for id in ["project-b", "project-a"] {
            dispatch(&mut store, &create_project_command(id)).unwrap();
        }
        let cmd = command("project.list", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        };
        let projects = payload["projects"].as_array().unwrap();
        let ids: Vec<&str> = projects.iter().map(|p| p["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["project-a", "project-b"]);
        let display_names: Vec<&str> = projects
            .iter()
            .map(|p| p["display_name"].as_str().unwrap())
            .collect();
        assert_eq!(
            display_names,
            vec!["Display project-a", "Display project-b"]
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_get_returns_full_state_for_a_known_project() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &create_project_command("project-1")).unwrap();
        let cmd = command("project.get", json!({ "aggregate_id": "project-1" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["id"], "project-1");
                assert_eq!(payload["revision"], 1);
                assert!(payload["state"].is_object());
                assert_eq!(payload["identity"]["display_name"], "Display project-1");
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_get_is_not_found_for_an_unknown_project() {
        let (mut store, path) = temp_store();
        let cmd = command("project.get", json!({ "aggregate_id": "no-such-project" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected ReplyOutcome::Error, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    // --- project.create -------------------------------------------------------

    #[test]
    fn project_create_appends_a_project_created_event() {
        let (mut store, path) = temp_store();
        let event = dispatch(&mut store, &create_project_command("project-1")).unwrap();
        assert_eq!(event.aggregate_id, "project-1");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "project.created");
        let identity = store.load_project_identity("project-1").unwrap().unwrap();
        assert_eq!(identity.display_name, "Display project-1");
        assert_eq!(identity.kind, ProjectKind::ExistingRepository);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_create_rejects_missing_display_name() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "project.create",
            json!({
                "aggregate_id": "project-1",
                "kind": "ExistingRepository",
                "locator": { "ExistingRepository": { "repository_identity": "repo-1" } },
            }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::InvalidParams(_)));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_create_rejects_kind_locator_mismatch() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "project.create",
            json!({
                "aggregate_id": "project-1",
                "display_name": "Display project-1",
                "kind": "NewProduct",
                "locator": { "ExistingRepository": { "repository_identity": "repo-1" } },
            }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(err, DispatchError::ProjectIdentity(_)));
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::InvalidParams);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_create_on_an_existing_project_surfaces_as_protocol_violation() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &create_project_command("project-1")).unwrap();
        let err = dispatch(&mut store, &create_project_command("project-1")).unwrap_err();
        assert!(matches!(
            err,
            DispatchError::ProjectStore(ProjectAppendError::AlreadyExists)
        ));
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::ProtocolViolation);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_advance_nominal_on_an_uncreated_project_is_not_found() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "project.advance_nominal",
            json!({ "aggregate_id": "project-1" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(
            err,
            DispatchError::ProjectStore(ProjectAppendError::NotFound)
        ));
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::NotFound);
        assert_eq!(store.latest_event_seq().unwrap(), 0);
        std::fs::remove_file(&path).ok();
    }

    // --- task.list / task.get -----------------------------------------------

    #[test]
    fn task_list_requires_project_id() {
        let (mut store, path) = temp_store();
        let cmd = command("task.list", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected ReplyOutcome::Error, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_list_only_returns_tasks_for_the_requested_project() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let other_project_task = command(
            "task.created",
            json!({
                "aggregate_id": "task-2",
                "project": ready_project_state_json(),
                "project_id": "project-2",
                "project_revision": 1,
                "original_request_ref": "original-request-ref-2",
            }),
        );
        dispatch(&mut store, &other_project_task).unwrap();

        let cmd = command("task.list", json!({ "project_id": "project-1" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                let ids: Vec<&str> = payload["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| t["id"].as_str().unwrap())
                    .collect();
                assert_eq!(ids, vec!["task-1"]);
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_get_returns_the_task_when_project_id_matches() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.get",
            json!({ "task_id": "task-1", "project_id": "project-1" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["id"], "task-1");
                assert_eq!(payload["state"]["project_id"], "project-1");
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_get_rejects_a_cross_project_id_as_protocol_violation_not_not_found() {
        let (mut store, path) = temp_store();
        dispatch(&mut store, &task_created_command("task-1")).unwrap();
        let cmd = command(
            "task.get",
            json!({ "task_id": "task-1", "project_id": "some-other-project" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => {
                assert_eq!(code, ReplyErrorCode::ProtocolViolation)
            }
            other => panic!("expected ReplyOutcome::Error, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn task_get_is_not_found_for_an_unknown_task_id() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "task.get",
            json!({ "task_id": "no-such-task", "project_id": "project-1" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected ReplyOutcome::Error, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    // --- queue.get ------------------------------------------------------------

    #[test]
    fn queue_get_returns_a_default_state_before_any_queue_event() {
        let (mut store, path) = temp_store();
        let cmd = command("queue.get", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["revision"], 0);
                assert!(payload["state"].is_object());
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn queue_get_reflects_state_after_an_enqueue() {
        use autome_domain::task::DispatchState;

        let (mut store, path) = temp_store();
        dispatch(
            &mut store,
            &command(
                "queue.enqueue",
                json!({ "task_id": "task-1", "enqueued_event_seq": 1 }),
            ),
        )
        .unwrap();
        let cmd = command("queue.get", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["revision"], 1);
                let state: ExecutionQueue = serde_json::from_value(payload["state"].clone())
                    .expect("queue.get payload must deserialize back into ExecutionQueue");
                assert_ne!(state.dispatch_state_of("task-1"), DispatchState::None);
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    // ---- §8.2 target-registration write path ----

    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    /// Like `temp_store`, but places the sqlite file inside its own
    /// freshly created, uniquely named directory rather than directly
    /// under the shared `std::env::temp_dir()`. Every test below that
    /// calls `create_project_from_target` needs this: unlike the legacy
    /// `create_project` the pre-existing tests above exercise (which never
    /// touches the filesystem), it lazily creates a `projects/` directory
    /// as a sibling of the db file the first time it runs anywhere, and
    /// `cargo test`'s default thread-per-test parallelism makes two tests
    /// racing to create that *same* shared directory a real, observable
    /// flake if they all shared the bare temp root (the identical fix is
    /// in `store.rs`'s own test module, `temp_data_root`).
    fn temp_store_with_isolated_root() -> (EventStore, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "automed-dispatch-test-root-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        (EventStore::open(&db_path).unwrap(), root)
    }

    /// Runs a `git` command against `dir` for building real fixtures (not
    /// the code under test itself) -- mirrors `target_probe.rs`'s own
    /// test-module helper of the same shape.
    fn fixture_git(dir: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .expect("fixture git command should spawn");
        assert!(status.success(), "fixture git {args:?} failed in {dir:?}");
    }

    fn init_repo_with_one_commit(dir: &std::path::Path) {
        fixture_git(dir, &["init", "--quiet"]);
        fixture_git(
            dir,
            &[
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=test",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "initial",
            ],
        );
    }

    /// Drives `handle_command`'s `project.register_target` branch end to
    /// end and hands back the resulting `target_id`, panicking on any
    /// registration failure -- registration succeeding is a precondition
    /// every `create_from_target` test below needs, not what any of them
    /// are individually testing.
    fn register(store: &mut EventStore, kind: &str, path: &std::path::Path) -> String {
        let cmd = command(
            "project.register_target",
            json!({ "kind": kind, "path": path.to_string_lossy() }),
        );
        let outcome = handle_command(store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload["target_id"]
                .as_str()
                .expect("register_target payload must contain target_id")
                .to_string(),
            ReplyOutcome::Error { code, message } => {
                panic!("register_target failed: {code:?} {message}")
            }
        }
    }

    fn create_from_target_command(
        target_id: &str,
        display_name: &str,
        trust_confirmed: bool,
        destination_name: Option<&str>,
    ) -> Command {
        let mut params = json!({
            "target_id": target_id,
            "display_name": display_name,
            "trust_confirmed": trust_confirmed,
        });
        if let Some(name) = destination_name {
            params["destination_name"] = json!(name);
        }
        command("project.create_from_target", params)
    }

    #[test]
    fn register_target_returns_a_target_id_and_a_summary_reflecting_the_probe() {
        let (mut store, root) = temp_store_with_isolated_root();
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        init_repo_with_one_commit(&repo);

        let cmd = command(
            "project.register_target",
            json!({ "kind": "ExistingRepository", "path": repo.to_string_lossy() }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert!(!payload["target_id"].as_str().unwrap().is_empty());
                assert_eq!(payload["summary"]["kind"], "ExistingRepository");
                assert_eq!(payload["summary"]["is_git_repo"], true);
                assert_eq!(payload["summary"]["head_resolvable"], true);
                assert_eq!(payload["summary"]["worktree_clean"], true);
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_register_target_produces_no_event() {
        let (mut store, root) = temp_store_with_isolated_root();
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        init_repo_with_one_commit(&repo);

        let cmd = command(
            "project.register_target",
            json!({ "kind": "ExistingRepository", "path": repo.to_string_lossy() }),
        );
        let outcome = handle_command(&mut store, &cmd);
        assert!(outcome.event.is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.1: `workspace.create_disposable_clone` follows the same
    /// no-`Event`-produced shape as `project.register_target` above, and
    /// its payload round-trips through the `workspace.get` read command.
    #[test]
    fn handle_command_workspace_create_disposable_clone_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        init_repo_with_one_commit(&repo);

        let create_cmd = command(
            "workspace.create_disposable_clone",
            json!({
                "task_id": "task-1",
                "run_id": "run-1",
                "source_repo": repo.to_string_lossy(),
            }),
        );
        let outcome = handle_command(&mut store, &create_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("workspace.create_disposable_clone failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["task_id"], "task-1");
        assert_eq!(payload["run_id"], "run-1");
        let repo_path = payload["repo_path"]
            .as_str()
            .expect("repo_path must be a string");
        assert!(std::path::Path::new(repo_path).join(".git").is_dir());

        let get_cmd = command(
            "workspace.get",
            json!({ "task_id": "task-1", "run_id": "run-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("workspace.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `workspace.get` for a `(task_id, run_id)` pair with no recorded
    /// clone is `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_workspace_get_is_not_found_when_no_clone_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command(
            "workspace.get",
            json!({ "task_id": "no-such-task", "run_id": "no-such-run" }),
        );
        let outcome = handle_command(&mut store, &get_cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A minimal well-formed `Attempt`/`AttemptPermissionProfile` JSON pair
    /// -- exercises the real IPC-boundary `serde_json` round trip (a raw
    /// JSON literal, not a typed Rust value handed straight in), same
    /// field-for-field values as `store.rs`'s own `fixture_attempt_and_profile`
    /// test helper.
    fn well_formed_attempt_and_profile_json(attempt_id: &str, write_roots: Vec<&str>) -> Value {
        json!({
            "run_id": "run-1",
            "attempt": {
                "id": attempt_id,
                "loop_step_id": "step-1",
                "node_id": null,
                "purpose": "Execution",
                "spec_binding": { "Execution": "exec-hash" },
                "agent_execution_profile_hash": "hash-agent",
                "permission_profile_id": "perm-1",
                "harness_id": "claude-code",
                "model_selection_identity_ref": "model-1",
                "qualification_receipt_ref": "qual-1",
                "input_commit": "deadbeef",
                "input_tree_hash": "treehash",
                "skill_projection_fingerprint": "skillfp",
                "provider_session_id": "session-1",
            },
            "permission_profile": {
                "id": "perm-1",
                "loop_step_id": "step-1",
                "node_id": null,
                "adapter_id": "claude-code",
                "installation_id": "install-1",
                "subject_scope_hash": "scope",
                "skill_set_snapshot_hash": "skillset",
                "tool_surface": {
                    "provider_available_tools": ["Read", "Bash"],
                    "provider_allowed_tools": ["Read"],
                    "provider_denied_tools": ["Bash"],
                    "autome_control_tools": [],
                    "dynamic_tool_or_mcp_allowlist": [],
                },
                "filesystem_policy": {
                    "read_roots": ["/project"],
                    "write_roots": write_roots,
                    "deny_roots": [],
                    "nofollow": true,
                },
                "command_policy": {
                    "qualified_runner_ids": [],
                    "argv_policy_hash": "",
                    "shell_allowed": false,
                },
                "network_policy": {
                    "mode": "Denied",
                    "allowed_brokers": [],
                    "allowed_destinations": [],
                },
                "sandbox_policy": {
                    "mechanism": "seatbelt",
                    "required_capabilities": [],
                    "fail_closed": true,
                },
                "secret_policy_hash": "secret",
                "safety_policy_hash": "safety",
                "profile_hash": "profile",
            },
        })
    }

    /// §5.6: `attempt.record` follows the same no-`Event`-produced shape as
    /// `workspace.create_disposable_clone` above, and its payload round-trips
    /// through the `attempt.get` read command.
    #[test]
    fn handle_command_attempt_record_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "attempt.record",
            well_formed_attempt_and_profile_json("attempt-1", vec!["/workdir"]),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("attempt.record failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["run_id"], "run-1");
        assert_eq!(payload["attempt"]["id"], "attempt-1");

        let get_cmd = command("attempt.get", json!({ "attempt_id": "attempt-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("attempt.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `attempt.get` for an `attempt_id` with no recorded Attempt is
    /// `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_attempt_get_is_not_found_when_no_attempt_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command("attempt.get", json!({ "attempt_id": "no-such-attempt" }));
        let outcome = handle_command(&mut store, &get_cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.6: a Planning-purpose Attempt paired with a write-granting profile
    /// must be rejected as `TransitionRejected`, not written.
    #[test]
    fn handle_command_attempt_record_rejects_a_planning_attempt_with_write_roots() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut params = well_formed_attempt_and_profile_json("attempt-1", vec!["/workdir"]);
        params["attempt"]["purpose"] = json!("Planning");
        params["attempt"]["spec_binding"] = json!({ "Planning": "plan-hash" });
        let record_cmd = command("attempt.record", params);
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command("attempt.get", json!({ "attempt_id": "attempt-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(matches!(
            get_outcome.reply.outcome,
            ReplyOutcome::Error {
                code: ReplyErrorCode::NotFound,
                ..
            }
        ));

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.6: a self-contradictory `AttemptPermissionProfile` (an allowed
    /// tool the provider never exposed) must be rejected as
    /// `TransitionRejected`, not written.
    #[test]
    fn handle_command_attempt_record_rejects_a_self_contradictory_permission_profile() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut params = well_formed_attempt_and_profile_json("attempt-1", vec![]);
        params["permission_profile"]["tool_surface"]["provider_allowed_tools"] =
            json!(["Read", "Write"]);
        let record_cmd = command("attempt.record", params);
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_evidence_receipt_json(receipt_id: &str) -> Value {
        json!({
            "receipt": {
                "receipt_id": receipt_id,
                "nonce": "nonce-1",
                "run_id": "run-1",
                "check_id": "C-001",
                "fingerprint": {
                    "contract_hash": "contract-1",
                    "check_hash": "check-1",
                    "project_rule_snapshot_hash": "rules-1",
                    "candidate_tree_hash": "tree-1",
                    "environment_class": "macos-15-arm64",
                },
                "verifier_version": "0.1.0",
                "payload": {
                    "Process": {
                        "program": "cargo",
                        "args": ["test"],
                        "exit_code": 0,
                        "assertions": [],
                        "inventory_changes": [],
                    },
                },
                "result": "Pass",
            },
        })
    }

    /// §5.7: `evidence.record` follows the same no-`Event`-produced shape as
    /// `attempt.record` above, and its payload round-trips through the
    /// `evidence.get` read command.
    #[test]
    fn handle_command_evidence_record_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command("evidence.record", well_formed_evidence_receipt_json("EV-1"));
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("evidence.record failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["receipt"]["receipt_id"], "EV-1");

        let get_cmd = command("evidence.get", json!({ "receipt_id": "EV-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("evidence.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `evidence.get` for a `receipt_id` with no recorded receipt is
    /// `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_evidence_get_is_not_found_when_no_receipt_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command("evidence.get", json!({ "receipt_id": "no-such-receipt" }));
        let outcome = handle_command(&mut store, &get_cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.7: `evidence.check` reports `valid: true` when the caller-supplied
    /// current fingerprint matches the recorded receipt's fingerprint
    /// exactly.
    #[test]
    fn handle_command_evidence_check_reports_valid_for_a_matching_fingerprint() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command("evidence.record", well_formed_evidence_receipt_json("EV-1"));
        handle_command(&mut store, &record_cmd);

        let check_cmd = command(
            "evidence.check",
            json!({
                "receipt_id": "EV-1",
                "fingerprint": {
                    "contract_hash": "contract-1",
                    "check_hash": "check-1",
                    "project_rule_snapshot_hash": "rules-1",
                    "candidate_tree_hash": "tree-1",
                    "environment_class": "macos-15-arm64",
                },
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["valid"], true),
            ReplyOutcome::Error { code, message } => {
                panic!("evidence.check failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.7: any single fingerprint field mismatch (here, a stale
    /// `candidate_tree_hash`) must report `valid: false` -- matching four
    /// out of five fields is not "close enough".
    #[test]
    fn handle_command_evidence_check_reports_invalid_for_a_mismatched_fingerprint() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command("evidence.record", well_formed_evidence_receipt_json("EV-1"));
        handle_command(&mut store, &record_cmd);

        let check_cmd = command(
            "evidence.check",
            json!({
                "receipt_id": "EV-1",
                "fingerprint": {
                    "contract_hash": "contract-1",
                    "check_hash": "check-1",
                    "project_rule_snapshot_hash": "rules-1",
                    "candidate_tree_hash": "tree-2",
                    "environment_class": "macos-15-arm64",
                },
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["valid"], false),
            ReplyOutcome::Error { code, message } => {
                panic!("evidence.check failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `evidence.check` for a `receipt_id` with no recorded receipt is
    /// `NotFound` -- there is nothing to check staleness of.
    #[test]
    fn handle_command_evidence_check_is_not_found_when_no_receipt_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let check_cmd = command(
            "evidence.check",
            json!({
                "receipt_id": "no-such-receipt",
                "fingerprint": {
                    "contract_hash": "contract-1",
                    "check_hash": "check-1",
                    "project_rule_snapshot_hash": "rules-1",
                    "candidate_tree_hash": "tree-1",
                    "environment_class": "macos-15-arm64",
                },
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_from_target_succeeds_for_a_real_clean_repository_and_lands_at_intent_unresolved() {
        let (mut store, root) = temp_store_with_isolated_root();
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        init_repo_with_one_commit(&repo);

        let target_id = register(&mut store, "ExistingRepository", &repo);
        let cmd = create_from_target_command(&target_id, "My Repo", true, None);
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.event_type, "IntentUnresolved");
        assert_eq!(event.aggregate_revision, 6);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_from_target_rejects_existing_repository_without_trust_confirmed() {
        let (mut store, root) = temp_store_with_isolated_root();
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        init_repo_with_one_commit(&repo);

        let target_id = register(&mut store, "ExistingRepository", &repo);
        let cmd = create_from_target_command(&target_id, "My Repo", false, None);
        let err = dispatch(&mut store, &cmd).unwrap_err();
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::InvalidParams);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_from_target_rejects_a_non_git_directory() {
        let (mut store, root) = temp_store_with_isolated_root();
        let plain = root.join("plain");
        std::fs::create_dir(&plain).unwrap();

        let target_id = register(&mut store, "ExistingRepository", &plain);
        let cmd = create_from_target_command(&target_id, "Not Git", true, None);
        let err = dispatch(&mut store, &cmd).unwrap_err();
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::InvalidParams);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_from_target_rejects_a_dirty_worktree() {
        let (mut store, root) = temp_store_with_isolated_root();
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        init_repo_with_one_commit(&repo);
        std::fs::write(repo.join("untracked.txt"), b"dirty").unwrap();

        let target_id = register(&mut store, "ExistingRepository", &repo);
        let cmd = create_from_target_command(&target_id, "Dirty", true, None);
        let err = dispatch(&mut store, &cmd).unwrap_err();
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::InvalidParams);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_from_target_rejects_a_destination_name_containing_a_path_separator() {
        let (mut store, root) = temp_store_with_isolated_root();
        let parent = root.join("parent");
        std::fs::create_dir(&parent).unwrap();

        let target_id = register(&mut store, "NewProduct", &parent);
        let cmd = create_from_target_command(&target_id, "New Thing", false, Some("sub/dir"));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::InvalidParams);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_from_target_rejects_a_destination_that_already_exists() {
        let (mut store, root) = temp_store_with_isolated_root();
        let parent = root.join("parent");
        std::fs::create_dir(&parent).unwrap();
        std::fs::create_dir(parent.join("existing")).unwrap();

        let target_id = register(&mut store, "NewProduct", &parent);
        let cmd = create_from_target_command(&target_id, "New Thing", false, Some("existing"));
        let err = dispatch(&mut store, &cmd).unwrap_err();
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::InvalidParams);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_from_target_rejects_an_unknown_target_id() {
        let (mut store, root) = temp_store_with_isolated_root();
        let cmd = create_from_target_command("does-not-exist", "Ghost", true, None);
        let err = dispatch(&mut store, &cmd).unwrap_err();
        let (code, _) = dispatch_error_to_reply_error(err);
        assert_eq!(code, ReplyErrorCode::NotFound);
        std::fs::remove_dir_all(&root).ok();
    }

    /// The behavioral half of the diagnostic-state story that `store.rs`'s
    /// own test suite deliberately deferred here (see its
    /// `diagnostic_reason_is_none_for_healthy_root_and_set_when_reverification_fails`
    /// doc comment): once `EventStore::open` has flagged a store
    /// diagnostic, every write must be refused and every read must keep
    /// working.
    #[test]
    fn dispatch_refuses_all_writes_while_diagnostic_but_reads_still_work() {
        let root = std::env::temp_dir().join(format!(
            "automed-dispatch-test-diagnostic-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777)).unwrap();
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        assert!(store.diagnostic_reason().is_some());

        let write_cmd = command("run.advance_nominal", json!({ "aggregate_id": "run-1" }));
        let err = dispatch(&mut store, &write_cmd).unwrap_err();
        assert!(matches!(err, DispatchError::Diagnostic(_)));

        let read_cmd = command("project.list", json!({}));
        let read_result =
            try_dispatch_read(&store, &read_cmd).expect("project.list is a read method");
        assert!(read_result.is_ok());

        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_dir_all(&root).ok();
    }
}
