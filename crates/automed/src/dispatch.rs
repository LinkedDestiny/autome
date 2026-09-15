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
//! `readiness.record` (§5.9) is the same shape again: writes a
//! `readiness_receipts` row but appends no `Event`. Parses `params.receipt`
//! (a full `autome_domain::readiness::ReadinessReceipt`); its read
//! counterpart `readiness.get` parses `params.receipt_digest`.
//! `readiness.check` re-exercises both `ReadinessReceipt::is_current_against`
//! (against a caller-supplied *current* `params.fingerprint`) and
//! `ReadinessReceipt::is_ready`, reporting each as its own boolean rather
//! than collapsing them into one -- a receipt can be current but not ready
//! (missing programs) or ready but stale (environment moved on since it was
//! observed), and a caller needs to tell those apart.
//! `delivery.*` (§5.12) is a different shape from every write method above:
//! a `DeliveryChain` is a five-rung append-only sequence, not a single fact
//! recorded once, so it gets six write methods instead of one, all handled
//! in `handle_command` ahead of both `try_dispatch_read` and `dispatch`
//! (same reason as `attempt.record` etc -- each writes a `delivery_chains`
//! row but appends no `Event`). `delivery.start` parses `params.run_id` and
//! `params.subject` (a full `DeliverySubject`) and fixes the chain's subject
//! once. `delivery.append_rehearsal`/`delivery.append_approval`/
//! `delivery.append_delivery`/`delivery.append_tree_check`/
//! `delivery.append_project_target_transition` each parse `params.run_id`
//! and `params.receipt` (the matching receipt type) and call the domain
//! `DeliveryChain`'s own `append_*` method -- re-using its order/outcome
//! checks rather than reimplementing them, surfaced as
//! `ReplyErrorCode::TransitionRejected` on rejection. `delivery.get` parses
//! `params.run_id`. `delivery.check_completion` re-exercises
//! `DeliveryChain::is_ready_for_completion` against the currently recorded
//! chain, reporting `{ ready, reason }` rather than trusting a caller's own
//! judgment of whether every rung is in place.
//! §6.6 `replan.*` is a different shape from every write method above once
//! more: `evaluate_replan_proposal` takes its whole `old_graph` explicitly
//! rather than reading it from the store, so it is completely stateless and
//! wired straight into `try_dispatch_read` (`replan.evaluate_proposal`) as
//! a `{ ok, violations }` check, matching
//! `historical_red_light.evaluate`/`step_role.validate_schema`'s
//! convention. `authorize_replan` and `authorize_contract_amendment` are
//! each a real validating constructor (`Result<_, Vec<_>>`), so per the
//! `QualificationReceiptInputParam`/`SkillInstallInputParam` rule each gets
//! its own `...InputParam` mirroring the constructor's own argument list
//! rather than trusting a client-supplied finished
//! `ReplanAuthorization`/`ContractAmendmentAuthorization` -- `replan.authorize`
//! and `replan.authorize_contract_amendment` write a
//! `replan_authorizations`/`contract_amendment_authorizations` row but
//! append no domain `Event`, same "fact issued once" shape as
//! `readiness.record`. `replan.get_authorization` parses `params.run_id`
//! and `params.new_graph_hash` (`ReplanAuthorization` carries no digest of
//! its own, so a Run replanned more than once needs both to disambiguate);
//! `replan.get_contract_amendment_authorization` parses `params.new_run_id`
//! alone (the one field `ContractAmendmentAuthorization` does carry that is
//! naturally unique).

use crate::ipc::{Command, Event, Reply, ReplyErrorCode, ReplyOutcome};
use crate::store::{
    AppendDeliveryReceiptError, AppendError, AppendedContractEvent, AppendedExecutionQueueEvent,
    AppendedGraphEvent, AppendedNodeEvent, AppendedProjectEvent, AppendedRunEvent,
    AppendedTaskEvent, AttemptRecord, AuthorizeContractAmendmentError, AuthorizeReplanError,
    BindPlaybookError, BudgetGrantRecord,
    CandidateCertificateRecord, CompletionCertificateRecord, ContractAmendmentAuthorizationRecord,
    ContractAppendError,
    CreateDisposableCloneError, CreateFromTargetError, DeliveryChainRecord,
    DisposableCloneRecord, EXECUTION_QUEUE_AGGREGATE_ID, EventStore, EvidenceRecord,
    ExecutionQueueAppendError, FrozenPlaybookRecord, GlobalConfigRevisionRecord, GraphAppendError,
    HumanReviewReceiptRecord,
    IssueCandidateCertificateError, IssueCompletionCertificateError, NodeAppendError,
    PlanningPolicyRestartRecord, ProjectAppendError, ProjectSummary, CredentialRecordRow,
    ProjectIntentAmendmentRecord, ProjectIntentRevisionRecord,
    ProjectInitializationReceiptRecord, QualificationRecord, ReadinessRecord, RecordAttemptError,
    RecordBudgetGrantError, RecordCredentialError, RecordCredentialReceiptError,
    RecordEvidenceError, RecordHumanReviewReceiptError, RecordPlanningPolicyRestartError,
    RecordProjectIntentAmendmentError, RecordProjectIntentRevisionError,
    RecordProjectInitializationReceiptError, RecordQualificationReceiptError, RecordReadinessError,
    RecordRunPolicyAmendmentError, RecordSkillInstallReceiptError, RecordUserCorrectionError,
    ReplanAuthorizationRecord,
    RunPolicyAmendmentRecord, SaveGlobalConfigRevisionError, SkillEvidenceLadderRecord,
    SkillInstallRecord, SkillLadderTransitionError, StartDeliveryChainError, TaskAppendError,
    TaskSummary, UserCorrectionRecord,
};
use autome_domain::attempt::{Attempt, AttemptPermissionProfile, LoopStepId};
use autome_domain::bounded_failure::{
    self, BoundedBackoffPolicy, BudgetUsage, DollarBudget, FailureCategory, FailureOccurrence,
    FrozenPolicySnapshot,
};
use autome_domain::capability_broker::{
    self, BrokerActionError, BrokerActionOrigin, UserInitiatedOnlyActionKind,
};
use autome_domain::certificate::AuditVerdict;
use autome_domain::config::{
    self, AgentExecutionProfile, GlobalConfigImpactPreview, GlobalConfigRevision,
    ProjectConfigPatch,
};
use autome_domain::clarification::{
    self, DefaultAssumptionCandidate, MandatoryClarificationTrigger, MaterialAssumption,
};
use autome_domain::delivery::{
    DeliveryApprovalReceipt, DeliveredTreeCheckReceipt, DeliveryReceipt, DeliveryRehearsalReceipt,
    DeliverySubject, ProjectTargetTransitionReceipt,
};
use autome_domain::credential::{CredentialEvent, CredentialReceipt, CredentialRecord};
use autome_domain::evidence::{EvidenceFingerprint, EvidenceReceipt, ReceiptId};
use autome_domain::historical_red_light::{self, HistoricalRedLightAssessment};
use autome_domain::contract::{AcceptanceCheck, ContractAmendment, ContractEvent};
use autome_domain::execution_queue::{ExecutionQueue, ExecutionQueueEvent};
use autome_domain::graph::{GraphEvent, GraphNode, TaskGraph};
use autome_domain::model_selection::{self, ModelSelectionIdentity, QualificationResult};
use autome_domain::node::NodeEvent;
use autome_domain::playbook::{FrozenPlaybook, RoleOutput};
use autome_domain::policy_restart::BudgetLimitGrant;
use autome_domain::project::{
    ProjectEvent, ProjectIdentity, ProjectIdentityError, ProjectKind, ProjectLocator, ProjectState,
};
use autome_domain::project_intent::{InitializationResult, KeyDecision};
use autome_domain::readiness::{ReadinessFingerprint, ReadinessReceipt};
use autome_domain::replan::{
    self, AmendmentReviewKind, ContractAmendmentProposal, ReplanProposal,
};
use autome_domain::requirement::{CheckId, Requirement, RequirementId};
use autome_domain::review::{self, HumanReviewFinding, ReviewDecision, ReviewSpecSubject, ReviewSubject};
use autome_domain::run::{GraphReviewOrigin, ReadinessOrigin, RunEvent, RunState, RunTerminal};
use autome_domain::safe_park::SafeParkReceipt;
use autome_domain::skill::{self, GlobalSkillBinding, ProjectSkillBinding, SkillAuditOutcome, SkillDigest};
use autome_domain::step_role::{self, LogicalRole};
use autome_domain::task::{DispatchState, QueueEntry, TaskEvent};
use autome_domain::user_correction::{
    CorrectionClassification, CorrectionDisposition, CorrectionImpactFlags,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use time::OffsetDateTime;

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
    /// §6.3 write path: `append_node_event`'s failure modes, same
    /// Sql/Transition split as every other event-sourced aggregate above.
    NodeStore(NodeAppendError),
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
    /// §5.9 write path: `record_readiness`'s only failure mode (a duplicate
    /// `receipt_digest`, surfaced as a SQL primary-key violation).
    RecordQualificationReceipt(RecordQualificationReceiptError),
    RecordReadiness(RecordReadinessError),
    /// §5.12 write path: `start_delivery_chain`'s only failure mode (a
    /// duplicate `run_id`, surfaced as a SQL primary-key violation).
    StartDeliveryChain(StartDeliveryChainError),
    /// §5.12 write path: any `append_delivery_*` method's failure modes --
    /// either `run_id` was never started (`NotFound`) or the domain
    /// `DeliveryChain`'s own order/outcome check rejected the rung
    /// (`Chain`), matched exhaustively below -- same reasoning as
    /// `CreateDisposableClone` above.
    AppendDeliveryReceipt(AppendDeliveryReceiptError),
    /// §5.8 write path: `issue_candidate_certificate`'s failure modes --
    /// the referenced readiness receipt was never recorded
    /// (`ReadinessNotFound`) or the domain's own `issue_candidate_certificate`
    /// rejected the certificate (`Domain`), matched exhaustively below --
    /// same reasoning as `CreateDisposableClone` above.
    IssueCandidateCertificate(IssueCandidateCertificateError),
    /// §5.8 write path: `issue_completion_certificate`'s failure modes --
    /// no candidate certificate or delivery chain was ever recorded for
    /// this `run_id` (`CandidateNotFound`/`DeliveryChainNotFound`), or the
    /// domain's own `issue_completion_certificate` rejected it (`Domain`).
    IssueCompletionCertificate(IssueCompletionCertificateError),
    /// §10.3 write path: `bind_playbook`'s only failure mode (a duplicate
    /// `run_id`, surfaced as a SQL primary-key violation -- a Run binds
    /// exactly one playbook for its whole lifetime).
    BindPlaybook(BindPlaybookError),
    /// §8.3 write path: `record_credential`'s only failure mode -- a shape
    /// mismatch caught by `CredentialRecord::validate_shape` before
    /// anything is written. Unlike `RecordAttempt`/`RecordEvidence` etc.,
    /// there is no duplicate-key failure mode here: `record_credential`
    /// deliberately upserts.
    RecordCredential(RecordCredentialError),
    /// §8.3 write path: `record_credential_receipt`'s only failure mode --
    /// `credential::issue_credential_receipt`'s own
    /// `UninstallRetentionDecisionRequiresOperator` check, re-run
    /// server-side rather than trusting the caller.
    RecordCredentialReceipt(RecordCredentialReceiptError),
    /// §6.5 write path: `record_user_correction`'s failure modes --
    /// `user_correction::issue_user_correction_receipt`'s own mechanical
    /// rules (pre/post-freeze classification, classification/disposition
    /// mismatch, contract-preserving impact), re-run server-side rather
    /// than trusting the caller (`Receipt`), or a duplicate `receipt_digest`
    /// surfaced as a SQL primary-key violation (`Sql`).
    RecordUserCorrection(RecordUserCorrectionError),
    /// §5.1 write path: `record_planning_policy_restart`'s failure modes --
    /// `policy_restart::issue_planning_policy_restart`'s own mechanical
    /// rules (planning spec unchanged, missing approval receipt), re-run
    /// server-side rather than trusting the caller (`Restart`), or a
    /// duplicate `restart_digest` surfaced as a SQL primary-key violation
    /// (`Sql`).
    RecordPlanningPolicyRestart(RecordPlanningPolicyRestartError),
    /// §5.1 write path: `record_run_policy_amendment`'s failure modes --
    /// `policy_restart::issue_run_policy_amendment`'s own mechanical rules
    /// (execution spec unchanged, missing policy diff/approval receipt, no
    /// invalidated Attempts), re-run server-side (`Amendment`), or a
    /// duplicate `amendment_digest` surfaced as a SQL primary-key violation
    /// (`Sql`).
    RecordRunPolicyAmendment(RecordRunPolicyAmendmentError),
    /// §5.1 write path: `record_budget_grant`'s failure modes --
    /// `policy_restart::issue_budget_grant_receipt`'s own mechanical rules
    /// (no limits added, missing reason/operator), re-run server-side
    /// (`Grant`), or a duplicate `grant_digest` surfaced as a SQL
    /// primary-key violation (`Sql`).
    RecordBudgetGrant(RecordBudgetGrantError),
    /// §5.1 write path: `record_project_intent_revision`'s failure modes --
    /// `project_intent::issue_project_intent_revision`'s own mechanical
    /// rules (missing approved_by/approved_at/source anchor/product goal, an
    /// incomplete key decision), re-run server-side (`Revision`), or a
    /// duplicate `(project_id, revision)` pair surfaced as a SQL
    /// primary-key violation (`Sql`).
    RecordProjectIntentRevision(RecordProjectIntentRevisionError),
    /// §5.1 write path: `record_project_intent_amendment`'s failure modes --
    /// no current revision was ever recorded for this project
    /// (`NoCurrentRevision`, the store-level equivalent of
    /// `IssueCandidateCertificateError::ReadinessNotFound`), the domain's
    /// own `apply_project_intent_amendment` rejected the request
    /// (`Amendment`), or a duplicate `amendment_hash` surfaced as a SQL
    /// primary-key violation (`Sql`).
    RecordProjectIntentAmendment(RecordProjectIntentAmendmentError),
    /// §5.1 write path: `record_project_initialization_receipt`'s failure
    /// modes -- `project_intent::issue_project_initialization_receipt`'s
    /// own mechanical rule (a `Blocked` result with no issues, or a `Ready`
    /// result that still lists issues), re-run server-side (`Receipt`), or a
    /// duplicate `receipt_digest` surfaced as a SQL primary-key violation
    /// (`Sql`).
    RecordProjectInitializationReceipt(RecordProjectInitializationReceiptError),
    /// §5.11 write path: `record_skill_install_receipt`'s failure modes --
    /// `issue_skill_install_receipt`'s own `MissingUserApprovalDecision`
    /// check, re-run server-side (`Install`), or a duplicate
    /// `receipt_digest` surfaced as a SQL primary-key violation (`Sql`).
    RecordSkillInstallReceipt(RecordSkillInstallReceiptError),
    /// §5.11 write path: every `mark_skill_*`/`record_skill_effective`
    /// method's failure modes -- no ladder was ever seeded by an install
    /// for this `skill_digest` (`NotFound`), the domain's own
    /// `SkillEvidenceLadder` rung-order check rejected the transition
    /// (`Ladder`), or a SQL failure (`Sql`).
    SkillLadderTransition(SkillLadderTransitionError),
    /// §5.1 write path: `record_human_review_receipt`'s failure modes --
    /// `issue_human_review_receipt`'s own reject-without-anchored-findings
    /// checks, re-run server-side (`Receipt`), or a duplicate
    /// `receipt_digest` surfaced as a SQL primary-key violation (`Sql`).
    RecordHumanReviewReceipt(RecordHumanReviewReceiptError),
    /// config.rs write path: `save_global_config_revision`'s failure modes --
    /// `config::validate_save_global_config_revision`'s own preview-hash/
    /// project-set/expiry/second-confirmation checks, re-run server-side
    /// against the caller-supplied preview (`Rejected`), or a duplicate
    /// `revision` surfaced as a SQL primary-key violation (`Sql`).
    SaveGlobalConfigRevision(SaveGlobalConfigRevisionError),
    /// §6.6 write path: `authorize_replan`'s failure modes -- the domain's
    /// own `replan::authorize_replan` rejected the proposal, a missing
    /// graph review receipt, and/or a missing user approval, collected
    /// together rather than flattened to one case (`Authorization`), or a
    /// duplicate `(run_id, new_graph_hash)` pair surfaced as a SQL
    /// primary-key violation (`Sql`).
    AuthorizeReplan(AuthorizeReplanError),
    /// §6.6 write path: `authorize_contract_amendment`'s failure modes --
    /// same shape as `AuthorizeReplan` above, collected together rather
    /// than flattened (`Authorization`), or a duplicate `new_run_id`
    /// surfaced as a SQL primary-key violation (`Sql`).
    AuthorizeContractAmendment(AuthorizeContractAmendmentError),
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

impl From<BindPlaybookError> for DispatchError {
    fn from(value: BindPlaybookError) -> Self {
        DispatchError::BindPlaybook(value)
    }
}

impl From<RecordCredentialError> for DispatchError {
    fn from(value: RecordCredentialError) -> Self {
        DispatchError::RecordCredential(value)
    }
}

impl From<RecordCredentialReceiptError> for DispatchError {
    fn from(value: RecordCredentialReceiptError) -> Self {
        DispatchError::RecordCredentialReceipt(value)
    }
}

impl From<RecordUserCorrectionError> for DispatchError {
    fn from(value: RecordUserCorrectionError) -> Self {
        DispatchError::RecordUserCorrection(value)
    }
}

impl From<RecordPlanningPolicyRestartError> for DispatchError {
    fn from(value: RecordPlanningPolicyRestartError) -> Self {
        DispatchError::RecordPlanningPolicyRestart(value)
    }
}

impl From<RecordRunPolicyAmendmentError> for DispatchError {
    fn from(value: RecordRunPolicyAmendmentError) -> Self {
        DispatchError::RecordRunPolicyAmendment(value)
    }
}

impl From<RecordBudgetGrantError> for DispatchError {
    fn from(value: RecordBudgetGrantError) -> Self {
        DispatchError::RecordBudgetGrant(value)
    }
}

impl From<RecordProjectIntentRevisionError> for DispatchError {
    fn from(value: RecordProjectIntentRevisionError) -> Self {
        DispatchError::RecordProjectIntentRevision(value)
    }
}

impl From<RecordProjectIntentAmendmentError> for DispatchError {
    fn from(value: RecordProjectIntentAmendmentError) -> Self {
        DispatchError::RecordProjectIntentAmendment(value)
    }
}

impl From<RecordProjectInitializationReceiptError> for DispatchError {
    fn from(value: RecordProjectInitializationReceiptError) -> Self {
        DispatchError::RecordProjectInitializationReceipt(value)
    }
}

impl From<RecordEvidenceError> for DispatchError {
    fn from(value: RecordEvidenceError) -> Self {
        DispatchError::RecordEvidence(value)
    }
}

impl From<RecordReadinessError> for DispatchError {
    fn from(value: RecordReadinessError) -> Self {
        DispatchError::RecordReadiness(value)
    }
}

impl From<RecordQualificationReceiptError> for DispatchError {
    fn from(value: RecordQualificationReceiptError) -> Self {
        DispatchError::RecordQualificationReceipt(value)
    }
}

impl From<StartDeliveryChainError> for DispatchError {
    fn from(value: StartDeliveryChainError) -> Self {
        DispatchError::StartDeliveryChain(value)
    }
}

impl From<RecordSkillInstallReceiptError> for DispatchError {
    fn from(value: RecordSkillInstallReceiptError) -> Self {
        DispatchError::RecordSkillInstallReceipt(value)
    }
}

impl From<SkillLadderTransitionError> for DispatchError {
    fn from(value: SkillLadderTransitionError) -> Self {
        DispatchError::SkillLadderTransition(value)
    }
}

impl From<RecordHumanReviewReceiptError> for DispatchError {
    fn from(value: RecordHumanReviewReceiptError) -> Self {
        DispatchError::RecordHumanReviewReceipt(value)
    }
}

impl From<SaveGlobalConfigRevisionError> for DispatchError {
    fn from(value: SaveGlobalConfigRevisionError) -> Self {
        DispatchError::SaveGlobalConfigRevision(value)
    }
}

impl From<AppendDeliveryReceiptError> for DispatchError {
    fn from(value: AppendDeliveryReceiptError) -> Self {
        DispatchError::AppendDeliveryReceipt(value)
    }
}

impl From<IssueCandidateCertificateError> for DispatchError {
    fn from(value: IssueCandidateCertificateError) -> Self {
        DispatchError::IssueCandidateCertificate(value)
    }
}

impl From<IssueCompletionCertificateError> for DispatchError {
    fn from(value: IssueCompletionCertificateError) -> Self {
        DispatchError::IssueCompletionCertificate(value)
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

impl From<NodeAppendError> for DispatchError {
    fn from(value: NodeAppendError) -> Self {
        DispatchError::NodeStore(value)
    }
}

impl From<AuthorizeReplanError> for DispatchError {
    fn from(value: AuthorizeReplanError) -> Self {
        DispatchError::AuthorizeReplan(value)
    }
}

impl From<AuthorizeContractAmendmentError> for DispatchError {
    fn from(value: AuthorizeContractAmendmentError) -> Self {
        DispatchError::AuthorizeContractAmendment(value)
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
    if let Some(event) = parameterless_node_event(&command.method) {
        let aggregate_id = require_aggregate_id(command)?;
        let appended = store.append_node_event(&aggregate_id, event)?;
        return Ok(node_event_envelope(aggregate_id, appended));
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

    // §5.9: same shape again -- writes a `readiness_receipts` row (so it
    // isn't a read) but appends no domain `Event` (a receipt is a fact
    // fixed once by an environment probe, not an aggregate with a reducer).
    if command.method == "readiness.record" {
        let result = handle_record_readiness(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.12: the six `delivery.*` write methods -- same shape again, but a
    // `DeliveryChain` is a five-rung append-only sequence rather than one
    // fact recorded once, so there are six of them instead of one.
    if command.method == "delivery.start" {
        let result = handle_start_delivery_chain(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "delivery.append_rehearsal" {
        let result = handle_append_delivery_rehearsal(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "delivery.append_approval" {
        let result = handle_append_delivery_approval(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "delivery.append_delivery" {
        let result = handle_append_delivery_delivery(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "delivery.append_tree_check" {
        let result = handle_append_delivery_tree_check(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "delivery.append_project_target_transition" {
        let result = handle_append_delivery_project_target_transition(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.8: the two `certificate.*` write methods -- same shape again.
    // Each composes already-recorded pieces (a readiness receipt, or a
    // candidate certificate plus a delivery chain) rather than accepting
    // them again, and appends no domain `Event` (a certificate is a fact
    // issued once, not an aggregate with a reducer).
    if command.method == "certificate.issue_candidate" {
        let result = handle_issue_candidate_certificate(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "certificate.issue_completion" {
        let result = handle_issue_completion_certificate(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §10.3: same shape again -- writes a `frozen_playbooks` row (so it
    // isn't a read) but appends no domain `Event` (a bound playbook is a
    // fact fixed once for a Run's whole lifetime, not an aggregate with a
    // reducer).
    if command.method == "playbook.bind" {
        let result = handle_bind_playbook(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §8.3: same shape again -- `credential.record` upserts a
    // `credentials` row and `credential.issue_receipt` appends a
    // `credential_receipts` row, neither appends a domain `Event`.
    if command.method == "credential.record" {
        let result = handle_record_credential(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "credential.issue_receipt" {
        let result = handle_record_credential_receipt(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §6.5: same shape again -- `user_correction.record` appends a
    // `user_correction_receipts` row, not a domain `Event` (a correction
    // receipt is a fact issued once, not an aggregate with a reducer).
    if command.method == "user_correction.record" {
        let result = handle_record_user_correction(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.1: same shape again -- each of these three issues a one-time fact
    // record (a restart/amendment/grant), not a domain `Event`.
    if command.method == "policy_restart.issue_planning_restart" {
        let result = handle_record_planning_policy_restart(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "policy_restart.issue_run_amendment" {
        let result = handle_record_run_policy_amendment(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "policy_restart.issue_budget_grant" {
        let result = handle_record_budget_grant(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.1: project_intent's three fact records -- same shape again.
    if command.method == "project_intent.record_revision" {
        let result = handle_record_project_intent_revision(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "project_intent.record_amendment" {
        let result = handle_record_project_intent_amendment(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "project_intent.record_initialization_receipt" {
        let result = handle_record_project_initialization_receipt(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.10: same shape again -- writes a `qualification_receipts` row (so
    // it isn't a read) but appends no domain `Event` (a receipt is a fact
    // issued once by a qualification batch, not an aggregate with a
    // reducer).
    if command.method == "model_selection.issue_qualification_receipt" {
        let result = handle_issue_qualification_receipt(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.11: same shape again -- each writes a `skill_install_receipts`/
    // `skill_evidence_ladders`/`global_skill_bindings`/
    // `project_skill_bindings` row but appends no domain `Event`. A
    // Skill's Vault entry, evidence ladder, and binding are all
    // current-state facts, not aggregates with a reducer.
    if command.method == "skill.issue_install_receipt" {
        let result = handle_issue_skill_install_receipt(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "skill.mark_bound" {
        let result = handle_mark_skill_bound(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "skill.mark_discoverable" {
        let result = handle_mark_skill_discoverable(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "skill.mark_available_to_attempt" {
        let result = handle_mark_skill_available_to_attempt(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "skill.mark_invoked" {
        let result = handle_mark_skill_invoked(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "skill.record_effective" {
        let result = handle_record_skill_effective(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "skill.record_global_binding" {
        let result = handle_record_global_skill_binding(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "skill.record_project_binding" {
        let result = handle_record_project_skill_binding(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §5.1: same shape again -- writes a `human_review_receipts` row (so
    // it isn't a read) but appends no domain `Event` (a human review
    // decision is a fact issued once by an operator, not an aggregate with
    // a reducer).
    if command.method == "review.issue_receipt" {
        let result = handle_issue_human_review_receipt(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // config.rs: writes a `global_config_revisions`/`project_config_patches`
    // row but appends no domain `Event` -- same shape as `skill.*` above. A
    // saved `GlobalConfigRevision` and a project's current
    // `ProjectConfigPatch` are both current-state facts (the revision log
    // and the patch upsert respectively), not aggregates with a reducer.
    if command.method == "config.save_global_revision" {
        let result = handle_save_global_config_revision(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "config.save_project_patch" {
        let result = handle_record_project_config_patch(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }

    // §6.6: same shape again -- each writes a `replan_authorizations`/
    // `contract_amendment_authorizations` row but appends no domain `Event`.
    // A replan/contract-amendment authorization is a fact issued once per
    // accepted proposal, not a state-machine transition.
    if command.method == "replan.authorize" {
        let result = handle_authorize_replan(store, command);
        return DispatchOutcome {
            reply: reply_for(command, value_outcome(store, result)),
            event: None,
        };
    }
    if command.method == "replan.authorize_contract_amendment" {
        let result = handle_authorize_contract_amendment(store, command);
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

/// §10.3: the sole caller of `EventStore::bind_playbook`. Takes
/// `{ run_id, playbook }` -- `playbook` is a full JSON object matching
/// `autome_domain::playbook::FrozenPlaybook`. Same "write returns Value not
/// Event" shape as `handle_record_attempt`/`handle_record_evidence`: a
/// bound playbook is a fact fixed once for a Run's whole lifetime, not a
/// state-machine transition. Refuses to run while the store is in its
/// diagnostic state, same as every other write.
fn handle_bind_playbook(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let playbook = parse_frozen_playbook_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .bind_playbook(&run_id, &playbook)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(frozen_playbook_record_json(&record))
}

fn frozen_playbook_record_json(record: &FrozenPlaybookRecord) -> Value {
    serde_json::json!({
        "run_id": record.run_id,
        "playbook": record.playbook,
        "created_at": record.created_at,
    })
}

fn parse_frozen_playbook_param(command: &Command) -> Result<FrozenPlaybook, DispatchError> {
    let value = command
        .params
        .get("playbook")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.playbook is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.playbook is not a valid FrozenPlaybook: {e}"))
    })
}

/// §8.3: the sole caller of `EventStore::record_credential`. Takes
/// `{ credential_ref, record }` -- `record` is a full JSON object matching
/// `autome_domain::credential::CredentialRecord`. Unlike every other write
/// handler above, a second call with the same `credential_ref` succeeds
/// and overwrites the snapshot -- see `RecordCredentialError`'s doc
/// comment. Refuses to run while the store is in its diagnostic state,
/// same as every other write.
fn handle_record_credential(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let credential_ref =
        parse_string_param(command, "credential_ref").map_err(dispatch_error_to_reply_error)?;
    let record = parse_credential_record_param(command).map_err(dispatch_error_to_reply_error)?;
    let row = store
        .record_credential(&credential_ref, &record)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(credential_record_row_json(&row))
}

fn credential_record_row_json(row: &CredentialRecordRow) -> Value {
    serde_json::json!({
        "credential_ref": row.credential_ref,
        "record": row.record,
        "created_at": row.created_at,
    })
}

fn parse_credential_record_param(command: &Command) -> Result<CredentialRecord, DispatchError> {
    let value = command
        .params
        .get("record")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.record is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.record is not a valid CredentialRecord: {e}"))
    })
}

/// §8.3: the sole caller of `EventStore::record_credential_receipt`. Takes
/// `{ credential_ref, event, occurred_at, operator? }` -- `event` is a full
/// JSON object matching `autome_domain::credential::CredentialEvent`.
/// Re-validates server-side via `credential::issue_credential_receipt`
/// rather than trusting an already-built receipt from the caller, same
/// discipline as `handle_record_attempt` re-validating shape/profile.
/// Refuses to run while the store is in its diagnostic state, same as
/// every other write.
fn handle_record_credential_receipt(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let credential_ref =
        parse_string_param(command, "credential_ref").map_err(dispatch_error_to_reply_error)?;
    let event = parse_credential_event_param(command).map_err(dispatch_error_to_reply_error)?;
    let occurred_at =
        parse_string_param(command, "occurred_at").map_err(dispatch_error_to_reply_error)?;
    let operator = match command.params.get("operator") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_str().map(|s| s.to_string()).ok_or_else(|| {
            dispatch_error_to_reply_error(DispatchError::InvalidParams(
                "params.operator must be a string".to_string(),
            ))
        })?),
    };
    let receipt = store
        .record_credential_receipt(&credential_ref, event, &occurred_at, operator.as_deref())
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(credential_receipt_json(&receipt))
}

fn credential_receipt_json(receipt: &CredentialReceipt) -> Value {
    serde_json::json!({
        "credential_ref": receipt.credential_ref,
        "event": receipt.event,
        "occurred_at": receipt.occurred_at,
        "operator": receipt.operator,
    })
}

fn parse_credential_event_param(command: &Command) -> Result<CredentialEvent, DispatchError> {
    let value = command
        .params
        .get("event")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.event is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.event is not a valid CredentialEvent: {e}"))
    })
}

/// The raw field bundle `user_correction.record` takes as `params.input` --
/// every argument `user_correction::issue_user_correction_receipt` needs,
/// not a pre-built `UserCorrectionReceipt`, so `handle_record_user_correction`
/// still re-runs the domain constructor server-side rather than trusting the
/// caller. Purely an IPC-parsing convenience, not a new domain concept: its
/// shape is just `issue_user_correction_receipt`'s own argument list.
#[derive(Debug, Deserialize)]
struct UserCorrectionInputParam {
    project_id: String,
    task_id: String,
    run_id: String,
    attempt_id: Option<String>,
    planning_spec_hash: String,
    execution_spec_hash: Option<String>,
    execution_spec_frozen: bool,
    raw_text_ref: String,
    #[serde(default)]
    attachment_hashes: Vec<String>,
    submitted_at: String,
    operator: String,
    subject_contract_hash: Option<String>,
    subject_graph_hash: Option<String>,
    subject_candidate_hash: Option<String>,
    classification: CorrectionClassification,
    #[serde(default)]
    impact: CorrectionImpactFlags,
    #[serde(default)]
    affected_requirement_ids: Vec<String>,
    #[serde(default)]
    affected_node_ids: Vec<String>,
    disposition: CorrectionDisposition,
    successor_ref: Option<String>,
    receipt_digest: String,
}

fn parse_user_correction_input_param(
    command: &Command,
) -> Result<UserCorrectionInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid user_correction input: {e}"
        ))
    })
}

/// §6.5: the sole caller of `EventStore::record_user_correction`. Takes
/// `{ input: <UserCorrectionInputParam> }` and re-validates server-side via
/// `user_correction::issue_user_correction_receipt` rather than trusting an
/// already-built receipt from the caller, same discipline as
/// `handle_record_credential_receipt`. Refuses to run while the store is in
/// its diagnostic state, same as every other write.
fn handle_record_user_correction(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_user_correction_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_user_correction(
            &input.project_id,
            &input.task_id,
            &input.run_id,
            input.attempt_id.as_deref(),
            &input.planning_spec_hash,
            input.execution_spec_hash.as_deref(),
            input.execution_spec_frozen,
            &input.raw_text_ref,
            input.attachment_hashes,
            &input.submitted_at,
            &input.operator,
            input.subject_contract_hash.as_deref(),
            input.subject_graph_hash.as_deref(),
            input.subject_candidate_hash.as_deref(),
            input.classification,
            input.impact,
            input.affected_requirement_ids,
            input.affected_node_ids,
            input.disposition,
            input.successor_ref.as_deref(),
            &input.receipt_digest,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(user_correction_record_json(&record))
}

fn user_correction_record_json(record: &UserCorrectionRecord) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_record_user_correction` -- looks up the
/// recorded `user_correction_receipts` row for `receipt_digest`, if any.
fn read_user_correction_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_digest =
        parse_string_param(command, "receipt_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_user_correction(&receipt_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(user_correction_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no user correction receipt recorded with digest {receipt_digest}"),
        )),
    }
}

/// The raw field bundle `policy_restart.issue_planning_restart` takes as
/// `params.input` -- every argument
/// `policy_restart::issue_planning_policy_restart` needs, same "IPC-parsing
/// convenience, not a new domain concept" reasoning as
/// `UserCorrectionInputParam`.
#[derive(Debug, Deserialize)]
struct PlanningPolicyRestartInputParam {
    task_id: String,
    current_run_id: String,
    old_planning_spec_hash: String,
    proposed_planning_spec_hash: String,
    trigger_revision_ref: String,
    config_revision_ref: String,
    skill_revision_ref: String,
    capability_revision_ref: String,
    #[serde(default)]
    invalidated_document_attempt_ids: Vec<String>,
    approval_receipt: String,
    restart_digest: String,
}

fn parse_planning_policy_restart_input_param(
    command: &Command,
) -> Result<PlanningPolicyRestartInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid policy_restart planning-restart input: {e}"
        ))
    })
}

/// §5.1: the sole caller of `EventStore::record_planning_policy_restart`.
/// Takes `{ input: <PlanningPolicyRestartInputParam> }` and re-validates
/// server-side via `policy_restart::issue_planning_policy_restart` rather
/// than trusting an already-built restart from the caller, same discipline
/// as `handle_record_user_correction`. Refuses to run while the store is in
/// its diagnostic state, same as every other write.
fn handle_record_planning_policy_restart(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input =
        parse_planning_policy_restart_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_planning_policy_restart(
            &input.task_id,
            &input.current_run_id,
            &input.old_planning_spec_hash,
            &input.proposed_planning_spec_hash,
            &input.trigger_revision_ref,
            &input.config_revision_ref,
            &input.skill_revision_ref,
            &input.capability_revision_ref,
            input.invalidated_document_attempt_ids,
            &input.approval_receipt,
            &input.restart_digest,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(planning_policy_restart_record_json(&record))
}

fn planning_policy_restart_record_json(record: &PlanningPolicyRestartRecord) -> Value {
    serde_json::json!({
        "restart": record.restart,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_record_planning_policy_restart` -- looks up
/// the recorded `planning_policy_restarts` row for `restart_digest`, if any.
fn read_planning_policy_restart_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let restart_digest =
        parse_string_param(command, "restart_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_planning_policy_restart(&restart_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(planning_policy_restart_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no planning policy restart recorded with digest {restart_digest}"),
        )),
    }
}

/// The raw field bundle `policy_restart.issue_run_amendment` takes as
/// `params.input` -- every argument `policy_restart::issue_run_policy_amendment`
/// needs, same reasoning as `PlanningPolicyRestartInputParam`.
#[derive(Debug, Deserialize)]
struct RunPolicyAmendmentInputParam {
    task_id: String,
    current_run_id: String,
    old_execution_spec_hash: String,
    proposed_execution_spec_hash: String,
    unchanged_contract_hash: String,
    unchanged_graph_hash: String,
    unchanged_base_hash: String,
    policy_diff: String,
    #[serde(default)]
    invalidated_attempt_ids: Vec<String>,
    #[serde(default)]
    invalidated_evidence_ids: Vec<String>,
    #[serde(default)]
    invalidated_audit_ids: Vec<String>,
    #[serde(default)]
    invalidated_candidate_ids: Vec<String>,
    approval_receipt: String,
    amendment_digest: String,
}

fn parse_run_policy_amendment_input_param(
    command: &Command,
) -> Result<RunPolicyAmendmentInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid policy_restart run-amendment input: {e}"
        ))
    })
}

/// §5.1: the sole caller of `EventStore::record_run_policy_amendment`.
/// Takes `{ input: <RunPolicyAmendmentInputParam> }` and re-validates
/// server-side via `policy_restart::issue_run_policy_amendment`.
fn handle_record_run_policy_amendment(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input =
        parse_run_policy_amendment_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_run_policy_amendment(
            &input.task_id,
            &input.current_run_id,
            &input.old_execution_spec_hash,
            &input.proposed_execution_spec_hash,
            &input.unchanged_contract_hash,
            &input.unchanged_graph_hash,
            &input.unchanged_base_hash,
            &input.policy_diff,
            input.invalidated_attempt_ids,
            input.invalidated_evidence_ids,
            input.invalidated_audit_ids,
            input.invalidated_candidate_ids,
            &input.approval_receipt,
            &input.amendment_digest,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(run_policy_amendment_record_json(&record))
}

fn run_policy_amendment_record_json(record: &RunPolicyAmendmentRecord) -> Value {
    serde_json::json!({
        "amendment": record.amendment,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_record_run_policy_amendment` -- looks up the
/// recorded `run_policy_amendments` row for `amendment_digest`, if any.
fn read_run_policy_amendment_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let amendment_digest =
        parse_string_param(command, "amendment_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_run_policy_amendment(&amendment_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(run_policy_amendment_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no run policy amendment recorded with digest {amendment_digest}"),
        )),
    }
}

/// The raw field bundle `policy_restart.issue_budget_grant` takes as
/// `params.input` -- every argument `policy_restart::issue_budget_grant_receipt`
/// needs, same reasoning as `PlanningPolicyRestartInputParam`.
#[derive(Debug, Deserialize)]
struct BudgetGrantInputParam {
    run_id: String,
    current_budget_hash: String,
    #[serde(default)]
    added_limits: Vec<BudgetLimitGrant>,
    reason: String,
    operator: String,
    expiry: Option<String>,
    grant_digest: String,
}

fn parse_budget_grant_input_param(
    command: &Command,
) -> Result<BudgetGrantInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid policy_restart budget-grant input: {e}"
        ))
    })
}

/// §5.1: the sole caller of `EventStore::record_budget_grant`. Takes
/// `{ input: <BudgetGrantInputParam> }` and re-validates server-side via
/// `policy_restart::issue_budget_grant_receipt`.
fn handle_record_budget_grant(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_budget_grant_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_budget_grant(
            &input.run_id,
            &input.current_budget_hash,
            input.added_limits,
            &input.reason,
            &input.operator,
            input.expiry.as_deref(),
            &input.grant_digest,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(budget_grant_record_json(&record))
}

fn budget_grant_record_json(record: &BudgetGrantRecord) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_record_budget_grant` -- looks up the
/// recorded `budget_grant_receipts` row for `grant_digest`, if any.
fn read_budget_grant_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let grant_digest =
        parse_string_param(command, "grant_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_budget_grant(&grant_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(budget_grant_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no budget grant receipt recorded with digest {grant_digest}"),
        )),
    }
}

/// The raw field bundle `project_intent.record_revision` takes as
/// `params.input` -- every argument
/// `project_intent::issue_project_intent_revision` needs, same reasoning as
/// `PlanningPolicyRestartInputParam`.
#[derive(Debug, Deserialize)]
struct ProjectIntentRevisionInputParam {
    project_id: String,
    revision: u32,
    #[serde(default)]
    source_anchors: Vec<String>,
    approved_by: String,
    approved_at: String,
    product_goal: String,
    #[serde(default)]
    target_users: Vec<String>,
    #[serde(default)]
    durable_cross_task_constraints: Vec<String>,
    #[serde(default)]
    explicit_non_goals: Vec<String>,
    #[serde(default)]
    key_decisions: Vec<KeyDecision>,
    supersedes: Option<u32>,
    intent_hash: String,
}

fn parse_project_intent_revision_input_param(
    command: &Command,
) -> Result<ProjectIntentRevisionInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid project_intent revision input: {e}"
        ))
    })
}

/// §5.1: the sole caller of `EventStore::record_project_intent_revision`.
/// Takes `{ input: <ProjectIntentRevisionInputParam> }` and re-validates
/// server-side via `project_intent::issue_project_intent_revision`.
fn handle_record_project_intent_revision(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input =
        parse_project_intent_revision_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_project_intent_revision(
            &input.project_id,
            input.revision,
            input.source_anchors,
            &input.approved_by,
            &input.approved_at,
            &input.product_goal,
            input.target_users,
            input.durable_cross_task_constraints,
            input.explicit_non_goals,
            input.key_decisions,
            input.supersedes,
            &input.intent_hash,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(project_intent_revision_record_json(&record))
}

fn project_intent_revision_record_json(record: &ProjectIntentRevisionRecord) -> Value {
    serde_json::json!({
        "revision": record.revision,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_record_project_intent_revision` -- looks up
/// the recorded `project_intent_revisions` row for one exact
/// `(project_id, revision)` pair.
fn read_project_intent_revision_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let project_id =
        parse_string_param(command, "project_id").map_err(dispatch_error_to_reply_error)?;
    let revision = parse_u32_param(command, "revision").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_project_intent_revision(&project_id, revision)
        .map_err(internal_error)?
    {
        Some(record) => Ok(project_intent_revision_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no project intent revision recorded for project {project_id} revision {revision}"),
        )),
    }
}

/// The derived "current revision" read -- looks up the highest recorded
/// `revision` for `project_id`, same query `record_project_intent_amendment`
/// itself uses to find the revision an amendment request is checked
/// against.
fn read_current_project_intent_revision_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let project_id =
        parse_string_param(command, "project_id").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_current_project_intent_revision(&project_id)
        .map_err(internal_error)?
    {
        Some(record) => Ok(project_intent_revision_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no project intent revision recorded for project {project_id}"),
        )),
    }
}

/// The raw field bundle `project_intent.record_amendment` takes as
/// `params.input` -- every argument `project_intent::apply_project_intent_amendment`
/// needs, same reasoning as `PlanningPolicyRestartInputParam`.
#[derive(Debug, Deserialize)]
struct ProjectIntentAmendmentInputParam {
    project_id: String,
    from_revision: u32,
    trigger_task: Option<String>,
    semantic_diff: String,
    #[serde(default)]
    affected_active_tasks: Vec<String>,
    user_decision_receipt: String,
    amendment_hash: String,
}

fn parse_project_intent_amendment_input_param(
    command: &Command,
) -> Result<ProjectIntentAmendmentInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid project_intent amendment input: {e}"
        ))
    })
}

/// §5.1: the sole caller of `EventStore::record_project_intent_amendment`.
/// Takes `{ input: <ProjectIntentAmendmentInputParam> }`; the store loads
/// the project's current revision itself and re-validates the request
/// against it server-side.
fn handle_record_project_intent_amendment(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_project_intent_amendment_input_param(command)
        .map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_project_intent_amendment(
            &input.project_id,
            input.from_revision,
            input.trigger_task.as_deref(),
            &input.semantic_diff,
            input.affected_active_tasks,
            &input.user_decision_receipt,
            &input.amendment_hash,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(project_intent_amendment_record_json(&record))
}

fn project_intent_amendment_record_json(record: &ProjectIntentAmendmentRecord) -> Value {
    serde_json::json!({
        "amendment": record.amendment,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_record_project_intent_amendment` -- looks up
/// the recorded `project_intent_amendments` row for `amendment_hash`, if
/// any.
fn read_project_intent_amendment_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let amendment_hash =
        parse_string_param(command, "amendment_hash").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_project_intent_amendment(&amendment_hash)
        .map_err(internal_error)?
    {
        Some(record) => Ok(project_intent_amendment_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no project intent amendment recorded with hash {amendment_hash}"),
        )),
    }
}

/// The raw field bundle `project_intent.record_initialization_receipt`
/// takes as `params.input` -- every argument
/// `project_intent::issue_project_initialization_receipt` needs, same
/// reasoning as `PlanningPolicyRestartInputParam`.
#[derive(Debug, Deserialize)]
struct ProjectInitializationReceiptInputParam {
    project_id: String,
    project_revision: u32,
    subject_identity_hash: String,
    trust_decision_ref: Option<String>,
    environment_snapshot_id: String,
    skill_inventory_id: String,
    project_home_manifest: String,
    result: InitializationResult,
    #[serde(default)]
    issues: Vec<String>,
    receipt_digest: String,
}

fn parse_project_initialization_receipt_input_param(
    command: &Command,
) -> Result<ProjectInitializationReceiptInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid project_intent initialization-receipt input: {e}"
        ))
    })
}

/// §5.1: the sole caller of `EventStore::record_project_initialization_receipt`.
/// Takes `{ input: <ProjectInitializationReceiptInputParam> }` and
/// re-validates server-side via
/// `project_intent::issue_project_initialization_receipt`.
fn handle_record_project_initialization_receipt(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_project_initialization_receipt_input_param(command)
        .map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_project_initialization_receipt(
            &input.project_id,
            input.project_revision,
            &input.subject_identity_hash,
            input.trust_decision_ref.as_deref(),
            &input.environment_snapshot_id,
            &input.skill_inventory_id,
            &input.project_home_manifest,
            input.result,
            input.issues,
            &input.receipt_digest,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(project_initialization_receipt_record_json(&record))
}

fn project_initialization_receipt_record_json(
    record: &ProjectInitializationReceiptRecord,
) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_record_project_initialization_receipt` --
/// looks up the recorded `project_initialization_receipts` row for
/// `receipt_digest`, if any.
fn read_project_initialization_receipt_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_digest =
        parse_string_param(command, "receipt_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_project_initialization_receipt(&receipt_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(project_initialization_receipt_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no project initialization receipt recorded with digest {receipt_digest}"),
        )),
    }
}

/// Dispatch-layer-only input for `model_selection.issue_qualification_receipt`
/// -- mirrors `issue_qualification_receipt`'s own argument list rather than
/// accepting a client-supplied `QualificationReceipt` directly. Every field
/// of `QualificationReceipt` is `pub` with a derived `Deserialize`, so a
/// caller who could hand in a whole receipt could set `valid_until` before
/// `issued_at` or blow past the seven-day cap without an immutable snapshot,
/// bypassing the one invariant `issue_qualification_receipt` exists to
/// enforce -- the same derive-bypasses-the-constructor concern documented on
/// `DollarBudgetInputParam`/`FrozenPolicySnapshotInputParam` in
/// `bounded_failure`'s dispatch wiring. `identity` and `result` have no
/// validating constructor of their own, so they're trusted as given.
#[derive(Debug, Deserialize)]
struct QualificationReceiptInputParam {
    identity: ModelSelectionIdentity,
    harness_capability_snapshot_digest: String,
    account_capability_snapshot_digest: String,
    canary_manifest_hash: String,
    run_ids: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    issued_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    valid_until: OffsetDateTime,
    provider_snapshot_is_immutable: bool,
    result: QualificationResult,
    receipt_digest: String,
}

fn parse_qualification_receipt_input_param(
    command: &Command,
) -> Result<QualificationReceiptInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid QualificationReceiptInputParam: {e}"
        ))
    })
}

/// Handles `model_selection.issue_qualification_receipt`: `{ input:
/// QualificationReceiptInputParam }`. Calls `issue_qualification_receipt`
/// server-side (so its validity-window invariant is actually enforced) and
/// then records the resulting receipt, same "write returns Value not Event"
/// shape as `handle_record_readiness` -- a receipt is a fact issued once by
/// a qualification batch, not a state-machine transition. Refuses to run
/// while the store is in its diagnostic state, same as every other write.
fn handle_issue_qualification_receipt(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_qualification_receipt_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let receipt = model_selection::issue_qualification_receipt(
        input.identity,
        input.harness_capability_snapshot_digest,
        input.account_capability_snapshot_digest,
        input.canary_manifest_hash,
        input.run_ids,
        input.issued_at,
        input.valid_until,
        input.provider_snapshot_is_immutable,
        input.result,
        input.receipt_digest,
    )
    .map_err(|e| {
        (
            ReplyErrorCode::TransitionRejected,
            format!("qualification receipt rejected: {e:?}"),
        )
    })?;
    let record = store
        .record_qualification_receipt(&receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(qualification_record_json(&record))
}

fn qualification_record_json(record: &QualificationRecord) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_issue_qualification_receipt` -- looks up the
/// recorded `qualification_receipts` row for `receipt_digest`, if any.
fn read_qualification_receipt_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_digest =
        parse_string_param(command, "receipt_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_qualification_receipt(&receipt_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(qualification_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no qualification receipt recorded with digest {receipt_digest}"),
        )),
    }
}

/// §5.10: `{ first_run_at, second_run_at }` (RFC3339 strings) -- re-exercises
/// `validate_sealed_pass_window` server-side. Returns `{ ok: bool, error:
/// Option<SealedPassWindowError> }` rather than erroring the whole call on a
/// window violation, matching the `bounded_failure.may_auto_retry`-style
/// "check" convention of reporting a negative answer as data.
fn read_model_selection_validate_sealed_pass_window(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let first_run_at =
        parse_offset_date_time_param(command, "first_run_at").map_err(dispatch_error_to_reply_error)?;
    let second_run_at =
        parse_offset_date_time_param(command, "second_run_at").map_err(dispatch_error_to_reply_error)?;
    match model_selection::validate_sealed_pass_window(first_run_at, second_run_at) {
        Ok(()) => Ok(serde_json::json!({ "ok": true, "error": Value::Null })),
        Err(err) => Ok(serde_json::json!({ "ok": false, "error": format!("{err:?}") })),
    }
}

fn parse_offset_date_time_param(
    command: &Command,
    key: &str,
) -> Result<OffsetDateTime, DispatchError> {
    let value = command
        .params
        .get(key)
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams(format!("params.{key} is required")))?;
    let raw: String = serde_json::from_value(value)
        .map_err(|e| DispatchError::InvalidParams(format!("params.{key} is not a string: {e}")))?;
    OffsetDateTime::parse(&raw, &time::format_description::well_known::Rfc3339).map_err(|e| {
        DispatchError::InvalidParams(format!("params.{key} is not a valid RFC3339 timestamp: {e}"))
    })
}

/// §5.10: `{ model_choice_key_hash_by_step: HashMap<LoopStepId,
/// Option<String>> }` -- re-exercises `validate_model_separation`
/// server-side. Collects every violation rather than just the first,
/// matching the `historical_red_light.evaluate`/`step_role.validate_schema`
/// "check" convention.
fn read_model_selection_validate_model_separation(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let by_step =
        parse_model_choice_key_hash_by_step_param(command).map_err(dispatch_error_to_reply_error)?;
    let violations = model_selection::validate_model_separation(&by_step);
    Ok(serde_json::json!({
        "ok": violations.is_empty(),
        "violations": violations,
    }))
}

fn parse_model_choice_key_hash_by_step_param(
    command: &Command,
) -> Result<std::collections::HashMap<LoopStepId, Option<String>>, DispatchError> {
    let value = command
        .params
        .get("model_choice_key_hash_by_step")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams(
                "params.model_choice_key_hash_by_step is required".to_string(),
            )
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.model_choice_key_hash_by_step is not a valid HashMap<LoopStepId, Option<String>>: {e}"
        ))
    })
}

/// Dispatch-layer-only input for `skill.issue_install_receipt` -- mirrors
/// `issue_skill_install_receipt`'s own argument list rather than accepting a
/// client-supplied `SkillInstallReceipt` directly, same
/// derive-bypasses-the-constructor reasoning as
/// `QualificationReceiptInputParam`: every field of `SkillInstallReceipt` is
/// `pub` with a derived `Deserialize`, so a caller who could hand in a whole
/// receipt could blank out `user_approval_decision_ref` and bypass the one
/// invariant `issue_skill_install_receipt` exists to enforce.
#[derive(Debug, Deserialize)]
struct SkillInstallInputParam {
    package_digest: String,
    audit_outcome: SkillAuditOutcome,
    plan_digest: String,
    user_approval_decision_ref: String,
    receipt_digest: String,
}

fn parse_skill_install_input_param(
    command: &Command,
) -> Result<SkillInstallInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid SkillInstallInputParam: {e}"
        ))
    })
}

/// Handles `skill.issue_install_receipt`: `{ input: SkillInstallInputParam
/// }`. Calls `record_skill_install_receipt`, which itself calls
/// `issue_skill_install_receipt` server-side (so the approval-decision
/// invariant is actually enforced) and seeds the fresh `Installed` ladder
/// in the same write -- see `record_skill_install_receipt`'s own doc
/// comment for why this is one call, not two.
fn handle_issue_skill_install_receipt(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_skill_install_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_skill_install_receipt(
            &input.package_digest,
            input.audit_outcome,
            &input.plan_digest,
            &input.user_approval_decision_ref,
            &input.receipt_digest,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(skill_install_record_json(&record))
}

fn skill_install_record_json(record: &SkillInstallRecord) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "ladder": record.ladder,
        "created_at": record.created_at,
    })
}

fn skill_evidence_ladder_record_json(record: &SkillEvidenceLadderRecord) -> Value {
    serde_json::json!({
        "ladder": record.ladder,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_issue_skill_install_receipt` -- looks up the
/// recorded `skill_install_receipts` row for `receipt_digest`, if any.
fn read_skill_install_receipt_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_digest =
        parse_string_param(command, "receipt_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_skill_install_receipt(&receipt_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(serde_json::json!({
            "receipt": record.receipt,
            "created_at": record.created_at,
        })),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no skill install receipt recorded with digest {receipt_digest}"),
        )),
    }
}

/// Read counterpart to the ladder half of `handle_issue_skill_install_receipt`
/// and every `mark_skill_*`/`skill.record_effective` write below -- the
/// current `SkillEvidenceLadder` for `skill_digest`, if one has ever been
/// seeded by an install.
fn read_skill_evidence_ladder_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let skill_digest =
        parse_string_param(command, "skill_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_skill_evidence_ladder(&skill_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(skill_evidence_ladder_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no skill evidence ladder recorded for skill_digest {skill_digest}"),
        )),
    }
}

/// Shared by every `skill.mark_*`/`skill.record_effective` write handler
/// below: refuses to run in the store's diagnostic state, parses
/// `skill_digest`, runs the given ladder transition, and formats the
/// resulting `SkillEvidenceLadderRecord`.
fn handle_skill_ladder_transition(
    store: &mut EventStore,
    command: &Command,
    transition: impl FnOnce(&mut EventStore, &str) -> Result<SkillEvidenceLadderRecord, SkillLadderTransitionError>,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let skill_digest =
        parse_string_param(command, "skill_digest").map_err(dispatch_error_to_reply_error)?;
    let record = transition(store, &skill_digest)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(skill_evidence_ladder_record_json(&record))
}

/// Handles `skill.mark_bound`: `{ skill_digest }`.
fn handle_mark_skill_bound(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    handle_skill_ladder_transition(store, command, |store, skill_digest| {
        store.mark_skill_bound(skill_digest)
    })
}

/// Handles `skill.mark_discoverable`: `{ skill_digest }`.
fn handle_mark_skill_discoverable(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    handle_skill_ladder_transition(store, command, |store, skill_digest| {
        store.mark_skill_discoverable(skill_digest)
    })
}

/// Handles `skill.mark_available_to_attempt`: `{ skill_digest }`.
fn handle_mark_skill_available_to_attempt(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    handle_skill_ladder_transition(store, command, |store, skill_digest| {
        store.mark_skill_available_to_attempt(skill_digest)
    })
}

/// Handles `skill.mark_invoked`: `{ skill_digest }`.
fn handle_mark_skill_invoked(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    handle_skill_ladder_transition(store, command, |store, skill_digest| {
        store.mark_skill_invoked(skill_digest)
    })
}

/// Handles `skill.record_effective`: `{ skill_digest, effective }`.
fn handle_record_skill_effective(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let skill_digest =
        parse_string_param(command, "skill_digest").map_err(dispatch_error_to_reply_error)?;
    let effective = parse_bool_param(command, "effective").map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_skill_effective(&skill_digest, effective)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(skill_evidence_ladder_record_json(&record))
}

fn parse_global_skill_binding_param(
    command: &Command,
) -> Result<GlobalSkillBinding, DispatchError> {
    let value = command
        .params
        .get("binding")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.binding is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.binding is not a valid GlobalSkillBinding: {e}"
        ))
    })
}

/// Handles `skill.record_global_binding`: `{ binding: GlobalSkillBinding }`.
/// `GlobalSkillBinding` has no validating constructor of its own, so it is
/// trusted directly -- same reasoning as `handle_record_readiness` trusting
/// a whole client-supplied `ReadinessReceipt`.
fn handle_record_global_skill_binding(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let binding = parse_global_skill_binding_param(command).map_err(dispatch_error_to_reply_error)?;
    let created_at = store
        .record_global_skill_binding(&binding)
        .map_err(internal_error)?;
    Ok(serde_json::json!({ "binding": binding, "created_at": created_at }))
}

/// Read counterpart to `handle_record_global_skill_binding` -- the current
/// snapshot for `skill_digest`, if one has ever been recorded.
fn read_global_skill_binding_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let skill_digest =
        parse_string_param(command, "skill_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_global_skill_binding(&skill_digest)
        .map_err(internal_error)?
    {
        Some(binding) => Ok(serde_json::json!({ "binding": binding })),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no global skill binding recorded for skill_digest {skill_digest}"),
        )),
    }
}

struct ProjectSkillBindingInputParam {
    project_id: String,
    skill_digest: String,
    binding: ProjectSkillBinding,
}

fn parse_project_skill_binding_input_param(
    command: &Command,
) -> Result<ProjectSkillBindingInputParam, DispatchError> {
    let project_id = parse_string_param(command, "project_id")?;
    let skill_digest = parse_string_param(command, "skill_digest")?;
    let value = command
        .params
        .get("binding")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.binding is required".to_string()))?;
    let binding = serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.binding is not a valid ProjectSkillBinding: {e}"
        ))
    })?;
    Ok(ProjectSkillBindingInputParam {
        project_id,
        skill_digest,
        binding,
    })
}

/// Handles `skill.record_project_binding`: `{ project_id, skill_digest,
/// binding: ProjectSkillBinding }`. `ProjectSkillBinding` has no validating
/// constructor of its own either, so it is trusted directly -- same
/// reasoning as `handle_record_global_skill_binding` above. The composite
/// key's two halves come from the caller's own `project_id`/`skill_digest`
/// params, not from the (key-less) `ProjectSkillBinding` struct itself --
/// same reasoning documented on `record_project_skill_binding`.
fn handle_record_project_skill_binding(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input =
        parse_project_skill_binding_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let created_at = store
        .record_project_skill_binding(&input.project_id, &input.skill_digest, &input.binding)
        .map_err(internal_error)?;
    Ok(serde_json::json!({ "binding": input.binding, "created_at": created_at }))
}

/// Read counterpart to `handle_record_project_skill_binding` -- the current
/// snapshot for `(project_id, skill_digest)`, if one has ever been
/// recorded.
fn read_project_skill_binding_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let project_id =
        parse_string_param(command, "project_id").map_err(dispatch_error_to_reply_error)?;
    let skill_digest =
        parse_string_param(command, "skill_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_project_skill_binding(&project_id, &skill_digest)
        .map_err(internal_error)?
    {
        Some(binding) => Ok(serde_json::json!({ "binding": binding })),
        None => Err((
            ReplyErrorCode::NotFound,
            format!(
                "no project skill binding recorded for project_id {project_id} and skill_digest {skill_digest}"
            ),
        )),
    }
}

/// §5.11: `{ skill_digest, project_id: Option<String> }` -- re-exercises
/// `resolve_project_binding` server-side against whatever global/project
/// bindings are actually recorded, rather than trusting a caller's own
/// merge of the two. A `skill_digest` with no recorded global binding at
/// all is `NotFound` (there is nothing to resolve); a `project_id` with no
/// recorded project binding is treated as pure inheritance, same as passing
/// `None` to `resolve_project_binding` directly.
fn read_skill_resolve_binding(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let skill_digest =
        parse_string_param(command, "skill_digest").map_err(dispatch_error_to_reply_error)?;
    let global = store
        .load_global_skill_binding(&skill_digest)
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                ReplyErrorCode::NotFound,
                format!("no global skill binding recorded for skill_digest {skill_digest}"),
            )
        })?;
    let project_id = parse_optional_string_param(command, "project_id")
        .map_err(dispatch_error_to_reply_error)?;
    let project = match project_id {
        Some(project_id) => store
            .load_project_skill_binding(&project_id, &skill_digest)
            .map_err(internal_error)?,
        None => None,
    };
    let resolved = skill::resolve_project_binding(&global, project.as_ref());
    Ok(serde_json::json!({ "resolved": resolved }))
}

fn parse_skill_digest_set_param(
    command: &Command,
    key: &str,
) -> Result<HashSet<SkillDigest>, DispatchError> {
    let value = command
        .params
        .get(key)
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams(format!("params.{key} is required")))?;
    let digests: Vec<String> = serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.{key} is not a valid array of strings: {e}"))
    })?;
    Ok(digests.into_iter().map(SkillDigest).collect())
}

/// §5.11: `{ digest, currently_bound_digests: [String], \
/// historically_referenced_digests: [String] }` -- re-exercises
/// `can_garbage_collect` server-side, matching the
/// `bounded_failure.may_auto_retry`-style "check" convention of reporting a
/// negative answer as data rather than an error. Pure -- no store lookup,
/// the caller supplies both sets, mirroring how
/// `read_model_selection_validate_model_separation` takes its whole map as
/// a param rather than deriving it from recorded state.
fn read_skill_can_garbage_collect(command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let digest =
        parse_string_param(command, "digest").map_err(dispatch_error_to_reply_error)?;
    let currently_bound = parse_skill_digest_set_param(command, "currently_bound_digests")
        .map_err(dispatch_error_to_reply_error)?;
    let historically_referenced =
        parse_skill_digest_set_param(command, "historically_referenced_digests")
            .map_err(dispatch_error_to_reply_error)?;
    let can_garbage_collect = skill::can_garbage_collect(
        &SkillDigest(digest),
        &currently_bound,
        &historically_referenced,
    );
    Ok(serde_json::json!({ "can_garbage_collect": can_garbage_collect }))
}

/// Dispatch-layer-only input for `review.issue_receipt` -- mirrors
/// `issue_human_review_receipt`'s own argument list rather than accepting a
/// client-supplied `HumanReviewReceipt` directly, same
/// derive-bypasses-the-constructor reasoning as
/// `QualificationReceiptInputParam`/`SkillInstallInputParam`: every field of
/// `HumanReviewReceipt` is `pub` with a derived `Deserialize`, so a caller
/// who could hand in a whole receipt could set `decision: Reject` with an
/// empty `finding_ids` and bypass the one invariant
/// `issue_human_review_receipt` exists to enforce. `findings` is a
/// `Vec<HumanReviewFinding>` trusted as given -- unlike `HumanReviewReceipt`,
/// `HumanReviewFinding` has no validating constructor of its own (only the
/// separate, unwired `mark_resolved`/`mark_superseded` transitions), so
/// there is nothing here for a client to bypass by constructing one
/// directly.
#[derive(Debug, Deserialize)]
struct HumanReviewReceiptInputParam {
    project_hash: String,
    task_hash: String,
    run_hash: String,
    spec_subject: ReviewSpecSubject,
    step_id: LoopStepId,
    operator: String,
    decided_at: String,
    decision: ReviewDecision,
    review_output_hash: String,
    reason: String,
    findings: Vec<HumanReviewFinding>,
    subject: ReviewSubject,
    receipt_digest: String,
}

fn parse_human_review_receipt_input_param(
    command: &Command,
) -> Result<HumanReviewReceiptInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid HumanReviewReceiptInputParam: {e}"
        ))
    })
}

/// Handles `review.issue_receipt`: `{ input: HumanReviewReceiptInputParam
/// }`. Calls `record_human_review_receipt`, which itself calls
/// `issue_human_review_receipt` server-side (so the reject-requires-findings
/// invariant is actually enforced), same "write returns Value not Event"
/// shape as `handle_issue_qualification_receipt` -- a human review decision
/// is a fact issued once by an operator, not an aggregate with a reducer.
fn handle_issue_human_review_receipt(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_human_review_receipt_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_human_review_receipt(
            &input.project_hash,
            &input.task_hash,
            &input.run_hash,
            input.spec_subject,
            input.step_id,
            &input.operator,
            &input.decided_at,
            input.decision,
            &input.review_output_hash,
            &input.reason,
            &input.findings,
            input.subject,
            &input.receipt_digest,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(human_review_receipt_record_json(&record))
}

fn human_review_receipt_record_json(record: &HumanReviewReceiptRecord) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_issue_human_review_receipt` -- looks up the
/// recorded `human_review_receipts` row for `receipt_digest`, if any.
fn read_human_review_receipt_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_digest =
        parse_string_param(command, "receipt_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_human_review_receipt(&receipt_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(human_review_receipt_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no human review receipt recorded with digest {receipt_digest}"),
        )),
    }
}

fn parse_human_review_findings_param(
    command: &Command,
    key: &str,
) -> Result<Vec<HumanReviewFinding>, DispatchError> {
    let value = command
        .params
        .get(key)
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams(format!("params.{key} is required")))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.{key} is not a valid array of HumanReviewFinding: {e}"
        ))
    })
}

/// §5.1: `{ subject_changed: bool, findings: [HumanReviewFinding] }` --
/// re-exercises `can_resubmit_for_review` server-side. Pure -- no store
/// lookup, the caller supplies the current finding statuses, mirroring how
/// `read_skill_can_garbage_collect` takes both its sets as params rather
/// than deriving them from recorded state.
fn read_review_can_resubmit(command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let subject_changed =
        parse_bool_param(command, "subject_changed").map_err(dispatch_error_to_reply_error)?;
    let findings = parse_human_review_findings_param(command, "findings")
        .map_err(dispatch_error_to_reply_error)?;
    let can_resubmit = review::can_resubmit_for_review(subject_changed, &findings);
    Ok(serde_json::json!({ "can_resubmit": can_resubmit }))
}

fn parse_loop_step_id_vec_param(
    command: &Command,
    key: &str,
) -> Result<Vec<LoopStepId>, DispatchError> {
    let value = command
        .params
        .get(key)
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams(format!("params.{key} is required")))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.{key} is not a valid array of LoopStepId: {e}"
        ))
    })
}

fn parse_fresh_receipt_ids_by_step_param(
    command: &Command,
) -> Result<std::collections::HashMap<LoopStepId, String>, DispatchError> {
    let value = command
        .params
        .get("fresh_receipt_ids_by_step")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams(
                "params.fresh_receipt_ids_by_step is required".to_string(),
            )
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.fresh_receipt_ids_by_step is not a valid HashMap<LoopStepId, String>: {e}"
        ))
    })
}

/// §5.1: `{ required_step_ids: [LoopStepId], fresh_receipt_ids_by_step:
/// HashMap<LoopStepId, String> }` -- re-exercises
/// `validate_carried_planning_review_bundle` server-side. Pure -- no store
/// lookup, mirroring `read_model_selection_validate_model_separation`
/// taking its whole map as a param rather than deriving it from recorded
/// state; collects every violation rather than just the first, matching the
/// same "check" convention.
fn read_review_validate_carried_planning_bundle(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let required_step_ids = parse_loop_step_id_vec_param(command, "required_step_ids")
        .map_err(dispatch_error_to_reply_error)?;
    let fresh_receipt_ids_by_step =
        parse_fresh_receipt_ids_by_step_param(command).map_err(dispatch_error_to_reply_error)?;
    let violations = review::validate_carried_planning_review_bundle(
        &required_step_ids,
        &fresh_receipt_ids_by_step,
    );
    Ok(serde_json::json!({
        "ok": violations.is_empty(),
        "violations": violations,
    }))
}

/// `config.save_global_revision`'s params. `GlobalConfigRevision` itself
/// has no validating constructor of its own in `config.rs` -- what needs
/// gating is the *save*, not the revision's shape -- but the save is
/// gated by `config::validate_save_global_config_revision`, which takes a
/// `preview` plus the freshness/confirmation facts it is checked against.
/// Bundling all of that into one `input` object (rather than a bare
/// `revision` param) mirrors `SkillInstallInputParam`: every field here is
/// required for `save_global_config_revision` to even attempt the write.
#[derive(Debug, Deserialize)]
struct SaveGlobalConfigRevisionInputParam {
    revision: GlobalConfigRevision,
    preview: GlobalConfigImpactPreview,
    submitted_preview_hash: String,
    current_project_set_hash: String,
    second_confirmation_acquired: bool,
}

fn parse_save_global_config_revision_input_param(
    command: &Command,
) -> Result<SaveGlobalConfigRevisionInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid SaveGlobalConfigRevisionInputParam: {e}"
        ))
    })
}

fn global_config_revision_record_json(record: &GlobalConfigRevisionRecord) -> Value {
    serde_json::json!({
        "revision": record.revision,
        "created_at": record.created_at,
    })
}

/// Handles `config.save_global_revision`: `{ input:
/// SaveGlobalConfigRevisionInputParam }`. Calls `save_global_config_revision`,
/// which re-runs `config::validate_save_global_config_revision` server-side
/// (so the preview-freshness/second-confirmation invariants are actually
/// enforced) before appending the new revision row.
fn handle_save_global_config_revision(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input =
        parse_save_global_config_revision_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .save_global_config_revision(
            input.revision,
            &input.preview,
            &input.submitted_preview_hash,
            &input.current_project_set_hash,
            input.second_confirmation_acquired,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(global_config_revision_record_json(&record))
}

/// Read counterpart to `handle_save_global_config_revision` for one exact
/// `revision` number.
fn read_global_config_revision_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let revision = parse_u32_param(command, "revision").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_global_config_revision(revision)
        .map_err(internal_error)?
    {
        Some(record) => Ok(global_config_revision_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no global config revision recorded for revision {revision}"),
        )),
    }
}

/// The derived "current global config" read: the highest `revision` ever
/// recorded, not a separately stored value -- takes no params, matching
/// `read_step_role_canonical_bindings`'s shape.
fn read_current_global_config_revision_get(
    store: &EventStore,
) -> Result<Value, (ReplyErrorCode, String)> {
    match store
        .load_current_global_config_revision()
        .map_err(internal_error)?
    {
        Some(record) => Ok(global_config_revision_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            "no global config revision recorded yet".to_string(),
        )),
    }
}

fn parse_project_config_patch_param(command: &Command) -> Result<ProjectConfigPatch, DispatchError> {
    let value = command
        .params
        .get("patch")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.patch is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.patch is not a valid ProjectConfigPatch: {e}"
        ))
    })
}

/// Handles `config.save_project_patch`: `{ patch: ProjectConfigPatch }`.
/// `ProjectConfigPatch` has no validating constructor of its own, so it is
/// trusted directly -- same reasoning as `handle_record_global_skill_binding`
/// trusting a whole client-supplied `GlobalSkillBinding`. Upserts the
/// project's current patch row; `patch.project_id` supplies the key, unlike
/// `skill.record_project_binding` where the key's second half has to come
/// from a separate param.
fn handle_record_project_config_patch(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let patch = parse_project_config_patch_param(command).map_err(dispatch_error_to_reply_error)?;
    let created_at = store
        .record_project_config_patch(&patch)
        .map_err(internal_error)?;
    Ok(serde_json::json!({ "patch": patch, "created_at": created_at }))
}

/// Read counterpart to `handle_record_project_config_patch` -- the current
/// snapshot for `project_id`, if one has ever been recorded.
fn read_project_config_patch_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let project_id =
        parse_string_param(command, "project_id").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_project_config_patch(&project_id)
        .map_err(internal_error)?
    {
        Some(patch) => Ok(serde_json::json!({ "patch": patch })),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no project config patch recorded for project_id {project_id}"),
        )),
    }
}

/// `config.resolve_project_config`: `{ project_id: Option<String>,
/// valid_model_ids: [String], skill_binding_revision_ref, snapshot_hash }`
/// -- re-exercises `config::resolve_project_config` server-side against
/// whatever global revision and project patch are actually recorded,
/// rather than trusting a caller's own merge of the two (mirrors
/// `read_skill_resolve_binding`). No `project_id` is pure inheritance from
/// the global revision, same as passing `None` to `resolve_project_config`
/// directly. `valid_model_ids` stands in for the "real
/// qualification/capability facts" `resolve_project_config`'s own doc
/// comment says its `is_profile_valid` predicate must come from -- there is
/// no qualification/capability store wired up yet for this read to consult
/// instead, so the caller supplies the already-qualified set directly, same
/// trust boundary as `read_skill_can_garbage_collect` taking its
/// bound/referenced sets as plain params rather than store lookups.
fn read_config_resolve_project_config(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let global = store
        .load_current_global_config_revision()
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                ReplyErrorCode::NotFound,
                "no global config revision recorded".to_string(),
            )
        })?
        .revision;
    let project_id = parse_optional_string_param(command, "project_id")
        .map_err(dispatch_error_to_reply_error)?;
    let patch = match &project_id {
        Some(project_id) => store
            .load_project_config_patch(project_id)
            .map_err(internal_error)?,
        None => None,
    };
    let valid_model_ids_value = command.params.get("valid_model_ids").cloned().ok_or_else(|| {
        (
            ReplyErrorCode::InvalidParams,
            "params.valid_model_ids is required".to_string(),
        )
    })?;
    let valid_model_ids: HashSet<String> = serde_json::from_value(valid_model_ids_value)
        .map_err(|e| {
            (
                ReplyErrorCode::InvalidParams,
                format!("params.valid_model_ids is not a valid array of strings: {e}"),
            )
        })?;
    let skill_binding_revision_ref = parse_string_param(command, "skill_binding_revision_ref")
        .map_err(dispatch_error_to_reply_error)?;
    let snapshot_hash =
        parse_string_param(command, "snapshot_hash").map_err(dispatch_error_to_reply_error)?;
    match config::resolve_project_config(
        &global,
        patch.as_ref(),
        |profile: &AgentExecutionProfile| valid_model_ids.contains(&profile.model_id),
        &skill_binding_revision_ref,
        &snapshot_hash,
    ) {
        Ok(resolved) => Ok(serde_json::json!({ "resolved": resolved })),
        Err(errors) => Err((
            ReplyErrorCode::TransitionRejected,
            format!("project config resolution rejected: {errors:?}"),
        )),
    }
}

/// Dispatch-layer-only input for `replan.authorize` -- mirrors
/// `authorize_replan`'s own argument list rather than accepting a
/// client-supplied `ReplanAuthorization` directly, same
/// derive-bypasses-the-constructor reasoning as
/// `QualificationReceiptInputParam`/`PlanningPolicyRestartInputParam`:
/// `ReplanAuthorization` is a plain `pub`-field struct with a derived
/// `Deserialize`, so a caller who could hand one in directly could fabricate
/// an authorization with no graph review or user approval behind it at all.
#[derive(Debug, Deserialize)]
struct ReplanAuthorizationInputParam {
    old_graph: TaskGraph,
    proposal: ReplanProposal,
    #[serde(default)]
    must_requirements: Vec<RequirementId>,
    #[serde(default)]
    mandatory_checks_by_requirement: HashMap<RequirementId, Vec<CheckId>>,
    graph_review_receipt_ref: Option<String>,
    user_approval_ref: Option<String>,
}

fn parse_replan_authorization_input_param(
    command: &Command,
) -> Result<ReplanAuthorizationInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid ReplanAuthorizationInputParam: {e}"
        ))
    })
}

/// §6.6: the sole caller of `EventStore::authorize_replan`. Takes `{ input:
/// <ReplanAuthorizationInputParam> }` and re-validates server-side via
/// `replan::authorize_replan` rather than trusting an already-built
/// authorization from the caller, same discipline as
/// `handle_record_planning_policy_restart`. Refuses to run while the store
/// is in its diagnostic state, same as every other write.
fn handle_authorize_replan(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_replan_authorization_input_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .authorize_replan(
            &input.old_graph,
            &input.proposal,
            &input.must_requirements,
            &input.mandatory_checks_by_requirement,
            input.graph_review_receipt_ref.as_deref(),
            input.user_approval_ref.as_deref(),
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(replan_authorization_record_json(&record))
}

fn replan_authorization_record_json(record: &ReplanAuthorizationRecord) -> Value {
    serde_json::json!({
        "authorization": record.authorization,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_authorize_replan` -- looks up the recorded
/// `replan_authorizations` row for `(run_id, new_graph_hash)`, if any.
fn read_replan_get_authorization(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let new_graph_hash =
        parse_string_param(command, "new_graph_hash").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_replan_authorization(&run_id, &new_graph_hash)
        .map_err(internal_error)?
    {
        Some(record) => Ok(replan_authorization_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!(
                "no replan authorization recorded for run {run_id} and graph hash {new_graph_hash}"
            ),
        )),
    }
}

fn parse_old_graph_param(command: &Command) -> Result<TaskGraph, DispatchError> {
    let value = command
        .params
        .get("old_graph")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.old_graph is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.old_graph is not a valid TaskGraph: {e}"))
    })
}

fn parse_replan_proposal_param(command: &Command) -> Result<ReplanProposal, DispatchError> {
    let value = command
        .params
        .get("proposal")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.proposal is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.proposal is not a valid ReplanProposal: {e}"))
    })
}

fn parse_mandatory_checks_by_requirement_param(
    command: &Command,
) -> Result<HashMap<RequirementId, Vec<CheckId>>, DispatchError> {
    let value = command
        .params
        .get("mandatory_checks_by_requirement")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams(
                "params.mandatory_checks_by_requirement is required".to_string(),
            )
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.mandatory_checks_by_requirement is not a valid HashMap<RequirementId, Vec<CheckId>>: {e}"
        ))
    })
}

/// §6.6: `{ old_graph, proposal, must_requirement_ids,
/// mandatory_checks_by_requirement }` -- re-exercises
/// `replan::evaluate_replan_proposal` server-side rather than trusting a
/// caller's own judgment of whether a proposal is acceptable. Collects every
/// rejection rather than just the first, matching the
/// `historical_red_light.evaluate`/`step_role.validate_schema` "check"
/// convention.
fn read_replan_evaluate_proposal(command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let old_graph = parse_old_graph_param(command).map_err(dispatch_error_to_reply_error)?;
    let proposal = parse_replan_proposal_param(command).map_err(dispatch_error_to_reply_error)?;
    let must_requirements =
        parse_must_requirement_ids_param(command).map_err(dispatch_error_to_reply_error)?;
    let mandatory_checks_by_requirement = parse_mandatory_checks_by_requirement_param(command)
        .map_err(dispatch_error_to_reply_error)?;
    let rejections = replan::evaluate_replan_proposal(
        &old_graph,
        &proposal,
        &must_requirements,
        &mandatory_checks_by_requirement,
    );
    Ok(serde_json::json!({
        "ok": rejections.is_empty(),
        "rejections": rejections,
    }))
}

/// Dispatch-layer-only input for `replan.authorize_contract_amendment` --
/// mirrors `authorize_contract_amendment`'s own argument list, same
/// reasoning as `ReplanAuthorizationInputParam`.
#[derive(Debug, Deserialize)]
struct ContractAmendmentAuthorizationInputParam {
    proposal: ContractAmendmentProposal,
    #[serde(default)]
    required_review_kinds: Vec<AmendmentReviewKind>,
    #[serde(default)]
    provided_review_receipts: HashMap<AmendmentReviewKind, String>,
    user_approval_ref: Option<String>,
}

fn parse_contract_amendment_authorization_input_param(
    command: &Command,
) -> Result<ContractAmendmentAuthorizationInputParam, DispatchError> {
    let value = command
        .params
        .get("input")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.input is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.input is not a valid ContractAmendmentAuthorizationInputParam: {e}"
        ))
    })
}

/// §6.6: the sole caller of `EventStore::authorize_contract_amendment`.
/// Takes `{ input: <ContractAmendmentAuthorizationInputParam> }` and
/// re-validates server-side via `replan::authorize_contract_amendment`, same
/// discipline as `handle_authorize_replan`.
fn handle_authorize_contract_amendment(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let input = parse_contract_amendment_authorization_input_param(command)
        .map_err(dispatch_error_to_reply_error)?;
    let record = store
        .authorize_contract_amendment(
            &input.proposal,
            &input.required_review_kinds,
            &input.provided_review_receipts,
            input.user_approval_ref.as_deref(),
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(contract_amendment_authorization_record_json(&record))
}

fn contract_amendment_authorization_record_json(
    record: &ContractAmendmentAuthorizationRecord,
) -> Value {
    serde_json::json!({
        "authorization": record.authorization,
        "created_at": record.created_at,
    })
}

/// Read counterpart to `handle_authorize_contract_amendment` -- looks up the
/// recorded `contract_amendment_authorizations` row for `new_run_id`, if any.
fn read_replan_get_contract_amendment_authorization(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let new_run_id =
        parse_string_param(command, "new_run_id").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_contract_amendment_authorization(&new_run_id)
        .map_err(internal_error)?
    {
        Some(record) => Ok(contract_amendment_authorization_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no contract amendment authorization recorded for new_run_id {new_run_id}"),
        )),
    }
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

/// Handles `readiness.record`: `{ receipt: ReadinessReceipt }`. Same
/// "write returns Value not Event" shape as `handle_record_evidence` -- a
/// recorded receipt is a fact fixed once by an environment probe, not a
/// state-machine transition. Refuses to run while the store is in its
/// diagnostic state, same as every other write.
fn handle_record_readiness(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let receipt = parse_readiness_receipt_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .record_readiness(&receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(readiness_record_json(&record))
}

fn readiness_record_json(record: &ReadinessRecord) -> Value {
    serde_json::json!({
        "receipt": record.receipt,
        "created_at": record.created_at,
    })
}

fn parse_readiness_receipt_param(command: &Command) -> Result<ReadinessReceipt, DispatchError> {
    let value = command
        .params
        .get("receipt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.receipt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.receipt is not a valid ReadinessReceipt: {e}"
        ))
    })
}

fn parse_readiness_fingerprint_param(
    command: &Command,
) -> Result<ReadinessFingerprint, DispatchError> {
    let value = command.params.get("fingerprint").cloned().ok_or_else(|| {
        DispatchError::InvalidParams("params.fingerprint is required".to_string())
    })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.fingerprint is not a valid ReadinessFingerprint: {e}"
        ))
    })
}

fn parse_broker_action_kind_param(
    command: &Command,
) -> Result<UserInitiatedOnlyActionKind, DispatchError> {
    let value = command
        .params
        .get("action")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.action is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.action is not a valid UserInitiatedOnlyActionKind: {e}"
        ))
    })
}

fn parse_broker_action_origin_param(
    command: &Command,
) -> Result<BrokerActionOrigin, DispatchError> {
    let value = command
        .params
        .get("origin")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.origin is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.origin is not a valid BrokerActionOrigin: {e}"
        ))
    })
}

/// Handles `certificate.issue_candidate`: `{ run_id, contract_version,
/// candidate_commit, candidate_tree_hash, must_requirement_ids, verdicts,
/// valid_receipt_ids, readiness_receipt_digest, fingerprint }`. Composes
/// the already-recorded readiness receipt (looked up by
/// `readiness_receipt_digest`, the digest `readiness.record` returned)
/// rather than accepting the whole receipt again -- callers only need to
/// still be holding the digest. Refuses to run while the store is in its
/// diagnostic state, same as every other write.
fn handle_issue_candidate_certificate(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let contract_version =
        parse_u32_param(command, "contract_version").map_err(dispatch_error_to_reply_error)?;
    let candidate_commit =
        parse_string_param(command, "candidate_commit").map_err(dispatch_error_to_reply_error)?;
    let candidate_tree_hash = parse_string_param(command, "candidate_tree_hash")
        .map_err(dispatch_error_to_reply_error)?;
    let must_requirement_ids =
        parse_must_requirement_ids_param(command).map_err(dispatch_error_to_reply_error)?;
    let verdicts = parse_audit_verdicts_param(command).map_err(dispatch_error_to_reply_error)?;
    let valid_receipt_ids =
        parse_valid_receipt_ids_param(command).map_err(dispatch_error_to_reply_error)?;
    let readiness_receipt_digest = parse_string_param(command, "readiness_receipt_digest")
        .map_err(dispatch_error_to_reply_error)?;
    let fingerprint =
        parse_readiness_fingerprint_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .issue_candidate_certificate(
            &run_id,
            contract_version,
            &candidate_commit,
            &candidate_tree_hash,
            &must_requirement_ids,
            &verdicts,
            &valid_receipt_ids,
            &readiness_receipt_digest,
            &fingerprint,
        )
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(candidate_certificate_record_json(&record))
}

fn candidate_certificate_record_json(record: &CandidateCertificateRecord) -> Value {
    serde_json::json!({
        "certificate": record.certificate,
        "created_at": record.created_at,
    })
}

fn parse_must_requirement_ids_param(
    command: &Command,
) -> Result<Vec<RequirementId>, DispatchError> {
    let value = command
        .params
        .get("must_requirement_ids")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams("params.must_requirement_ids is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.must_requirement_ids is not a valid Vec<RequirementId>: {e}"
        ))
    })
}

fn parse_audit_verdicts_param(command: &Command) -> Result<Vec<AuditVerdict>, DispatchError> {
    let value = command
        .params
        .get("verdicts")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.verdicts is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.verdicts is not a valid Vec<AuditVerdict>: {e}"
        ))
    })
}

fn parse_valid_receipt_ids_param(command: &Command) -> Result<HashSet<ReceiptId>, DispatchError> {
    let value = command
        .params
        .get("valid_receipt_ids")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams("params.valid_receipt_ids is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.valid_receipt_ids is not a valid HashSet<ReceiptId>: {e}"
        ))
    })
}

/// Handles `certificate.issue_completion`: `{ run_id, delivery_tree_hash,
/// user_approval_decision_ref }`. Composes the already-issued candidate
/// certificate and the already-recorded delivery chain for `run_id` (via
/// `load_candidate_certificate`/`load_delivery_chain`) rather than
/// accepting either again -- the caller supplies only what neither record
/// already carries. Refuses to run while the store is in its diagnostic
/// state, same as every other write.
fn handle_issue_completion_certificate(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let delivery_tree_hash =
        parse_string_param(command, "delivery_tree_hash").map_err(dispatch_error_to_reply_error)?;
    let user_approval_decision_ref = parse_string_param(command, "user_approval_decision_ref")
        .map_err(dispatch_error_to_reply_error)?;
    let record = store
        .issue_completion_certificate(&run_id, &delivery_tree_hash, &user_approval_decision_ref)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(completion_certificate_record_json(&record))
}

fn completion_certificate_record_json(record: &CompletionCertificateRecord) -> Value {
    serde_json::json!({
        "run_id": record.run_id,
        "certificate": record.certificate,
        "created_at": record.created_at,
    })
}

/// Handles `delivery.start`: `{ run_id, subject: DeliverySubject }`. Same
/// "write returns Value not Event" shape as `handle_record_attempt` -- fixes
/// a `DeliveryChain`'s subject once, not a state-machine transition.
/// Refuses to run while the store is in its diagnostic state, same as every
/// other write.
fn handle_start_delivery_chain(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let subject = parse_delivery_subject_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .start_delivery_chain(&run_id, &subject)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(delivery_chain_record_json(&record))
}

fn delivery_chain_record_json(record: &DeliveryChainRecord) -> Value {
    serde_json::json!({
        "run_id": record.run_id,
        "subject": record.subject,
        "rehearsal": record.rehearsal,
        "approval": record.approval,
        "delivery": record.delivery,
        "tree_check": record.tree_check,
        "project_target_transition": record.project_target_transition,
        "created_at": record.created_at,
        "updated_at": record.updated_at,
    })
}

fn parse_delivery_subject_param(command: &Command) -> Result<DeliverySubject, DispatchError> {
    let value = command
        .params
        .get("subject")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.subject is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.subject is not a valid DeliverySubject: {e}"
        ))
    })
}

/// Handles `delivery.append_rehearsal`: `{ run_id, receipt: DeliveryRehearsalReceipt }`.
fn handle_append_delivery_rehearsal(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let receipt =
        parse_delivery_rehearsal_receipt_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .append_delivery_rehearsal(&run_id, &receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(delivery_chain_record_json(&record))
}

fn parse_delivery_rehearsal_receipt_param(
    command: &Command,
) -> Result<DeliveryRehearsalReceipt, DispatchError> {
    let value = command
        .params
        .get("receipt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.receipt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.receipt is not a valid DeliveryRehearsalReceipt: {e}"
        ))
    })
}

/// Handles `delivery.append_approval`: `{ run_id, receipt: DeliveryApprovalReceipt }`.
/// Rejects (via `AppendDeliveryReceiptError::Chain`) if `run_id`'s chain has
/// no rehearsal recorded yet.
fn handle_append_delivery_approval(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let receipt =
        parse_delivery_approval_receipt_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .append_delivery_approval(&run_id, &receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(delivery_chain_record_json(&record))
}

fn parse_delivery_approval_receipt_param(
    command: &Command,
) -> Result<DeliveryApprovalReceipt, DispatchError> {
    let value = command
        .params
        .get("receipt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.receipt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.receipt is not a valid DeliveryApprovalReceipt: {e}"
        ))
    })
}

/// Handles `delivery.append_delivery`: `{ run_id, receipt: DeliveryReceipt }`
/// -- the outcome of actually performing the delivery. Rejects if `run_id`'s
/// chain has no approval recorded yet, or a delivery was already recorded.
fn handle_append_delivery_delivery(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let receipt = parse_delivery_receipt_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .append_delivery_delivery(&run_id, &receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(delivery_chain_record_json(&record))
}

fn parse_delivery_receipt_param(command: &Command) -> Result<DeliveryReceipt, DispatchError> {
    let value = command
        .params
        .get("receipt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.receipt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.receipt is not a valid DeliveryReceipt: {e}"
        ))
    })
}

/// Handles `delivery.append_tree_check`: `{ run_id, receipt: DeliveredTreeCheckReceipt }`.
/// Rejects if `run_id`'s chain has no successful delivery recorded yet, or a
/// tree check was already recorded.
fn handle_append_delivery_tree_check(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let receipt =
        parse_delivered_tree_check_receipt_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .append_delivery_tree_check(&run_id, &receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(delivery_chain_record_json(&record))
}

fn parse_delivered_tree_check_receipt_param(
    command: &Command,
) -> Result<DeliveredTreeCheckReceipt, DispatchError> {
    let value = command
        .params
        .get("receipt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.receipt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.receipt is not a valid DeliveredTreeCheckReceipt: {e}"
        ))
    })
}

/// Handles `delivery.append_project_target_transition`:
/// `{ run_id, receipt: ProjectTargetTransitionReceipt }`. Rejects unless
/// `run_id`'s chain subject is `Greenfield` and a matching tree check is
/// already recorded.
fn handle_append_delivery_project_target_transition(
    store: &mut EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    if let Some(reason) = store.diagnostic_reason() {
        return Err((ReplyErrorCode::ProtocolViolation, reason.to_string()));
    }
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let receipt = parse_project_target_transition_receipt_param(command)
        .map_err(dispatch_error_to_reply_error)?;
    let record = store
        .append_delivery_project_target_transition(&run_id, &receipt)
        .map_err(|e| dispatch_error_to_reply_error(DispatchError::from(e)))?;
    Ok(delivery_chain_record_json(&record))
}

fn parse_project_target_transition_receipt_param(
    command: &Command,
) -> Result<ProjectTargetTransitionReceipt, DispatchError> {
    let value = command
        .params
        .get("receipt")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.receipt is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.receipt is not a valid ProjectTargetTransitionReceipt: {e}"
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
        DispatchError::NodeStore(NodeAppendError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::NodeStore(NodeAppendError::Transition(e)) => {
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
        DispatchError::BindPlaybook(BindPlaybookError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordCredential(RecordCredentialError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordCredential(RecordCredentialError::Shape(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordCredentialReceipt(RecordCredentialReceiptError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordCredentialReceipt(RecordCredentialReceiptError::Receipt(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordUserCorrection(RecordUserCorrectionError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordUserCorrection(RecordUserCorrectionError::Receipt(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordPlanningPolicyRestart(RecordPlanningPolicyRestartError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordPlanningPolicyRestart(RecordPlanningPolicyRestartError::Restart(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordRunPolicyAmendment(RecordRunPolicyAmendmentError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordRunPolicyAmendment(RecordRunPolicyAmendmentError::Amendment(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordBudgetGrant(RecordBudgetGrantError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordBudgetGrant(RecordBudgetGrantError::Grant(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::RecordProjectIntentRevision(RecordProjectIntentRevisionError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordProjectIntentRevision(
            RecordProjectIntentRevisionError::Revision(e),
        ) => (ReplyErrorCode::TransitionRejected, format!("{e:?}")),
        DispatchError::RecordProjectIntentAmendment(RecordProjectIntentAmendmentError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordProjectIntentAmendment(
            RecordProjectIntentAmendmentError::NoCurrentRevision,
        ) => (
            ReplyErrorCode::NotFound,
            "no current project intent revision recorded for this project".to_string(),
        ),
        DispatchError::RecordProjectIntentAmendment(
            RecordProjectIntentAmendmentError::Amendment(e),
        ) => (ReplyErrorCode::TransitionRejected, format!("{e:?}")),
        DispatchError::RecordProjectInitializationReceipt(
            RecordProjectInitializationReceiptError::Sql(e),
        ) => (ReplyErrorCode::Internal, e.to_string()),
        DispatchError::RecordProjectInitializationReceipt(
            RecordProjectInitializationReceiptError::Receipt(e),
        ) => (ReplyErrorCode::TransitionRejected, format!("{e:?}")),
        DispatchError::RecordReadiness(RecordReadinessError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordQualificationReceipt(RecordQualificationReceiptError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordSkillInstallReceipt(RecordSkillInstallReceiptError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordSkillInstallReceipt(RecordSkillInstallReceiptError::Install(e)) => {
            (
                ReplyErrorCode::TransitionRejected,
                format!("skill install receipt rejected: {e:?}"),
            )
        }
        DispatchError::SkillLadderTransition(SkillLadderTransitionError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::SkillLadderTransition(SkillLadderTransitionError::NotFound) => (
            ReplyErrorCode::NotFound,
            "no skill evidence ladder recorded for this skill_digest".to_string(),
        ),
        DispatchError::SkillLadderTransition(SkillLadderTransitionError::Ladder(e)) => (
            ReplyErrorCode::TransitionRejected,
            format!("skill ladder transition rejected: {e:?}"),
        ),
        DispatchError::RecordHumanReviewReceipt(RecordHumanReviewReceiptError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::RecordHumanReviewReceipt(RecordHumanReviewReceiptError::Receipt(e)) => (
            ReplyErrorCode::TransitionRejected,
            format!("human review receipt rejected: {e:?}"),
        ),
        DispatchError::SaveGlobalConfigRevision(SaveGlobalConfigRevisionError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::SaveGlobalConfigRevision(SaveGlobalConfigRevisionError::Rejected(
            errors,
        )) => (
            ReplyErrorCode::TransitionRejected,
            format!("global config revision save rejected: {errors:?}"),
        ),
        DispatchError::StartDeliveryChain(StartDeliveryChainError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::AppendDeliveryReceipt(AppendDeliveryReceiptError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::AppendDeliveryReceipt(AppendDeliveryReceiptError::NotFound) => (
            ReplyErrorCode::NotFound,
            "no delivery chain started for this run_id".to_string(),
        ),
        DispatchError::AppendDeliveryReceipt(AppendDeliveryReceiptError::Chain(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::IssueCandidateCertificate(IssueCandidateCertificateError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::IssueCandidateCertificate(
            IssueCandidateCertificateError::ReadinessNotFound,
        ) => (
            ReplyErrorCode::NotFound,
            "no readiness receipt recorded with this digest".to_string(),
        ),
        DispatchError::IssueCandidateCertificate(IssueCandidateCertificateError::Domain(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::IssueCompletionCertificate(IssueCompletionCertificateError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::IssueCompletionCertificate(
            IssueCompletionCertificateError::CandidateNotFound,
        ) => (
            ReplyErrorCode::NotFound,
            "no candidate certificate issued for this run_id".to_string(),
        ),
        DispatchError::IssueCompletionCertificate(
            IssueCompletionCertificateError::DeliveryChainNotFound,
        ) => (
            ReplyErrorCode::NotFound,
            "no delivery chain started for this run_id".to_string(),
        ),
        DispatchError::IssueCompletionCertificate(IssueCompletionCertificateError::Domain(e)) => {
            (ReplyErrorCode::TransitionRejected, format!("{e:?}"))
        }
        DispatchError::AuthorizeReplan(AuthorizeReplanError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::AuthorizeReplan(AuthorizeReplanError::Authorization(errors)) => (
            ReplyErrorCode::TransitionRejected,
            format!("replan authorization rejected: {errors:?}"),
        ),
        DispatchError::AuthorizeContractAmendment(AuthorizeContractAmendmentError::Sql(e)) => {
            (ReplyErrorCode::Internal, e.to_string())
        }
        DispatchError::AuthorizeContractAmendment(
            AuthorizeContractAmendmentError::Authorization(errors),
        ) => (
            ReplyErrorCode::TransitionRejected,
            format!("contract amendment authorization rejected: {errors:?}"),
        ),
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
        "readiness.get" => Some(read_readiness_get(store, command)),
        "readiness.check" => Some(read_readiness_check(store, command)),
        "delivery.get" => Some(read_delivery_get(store, command)),
        "delivery.check_completion" => Some(read_delivery_check_completion(store, command)),
        "certificate.get_candidate" => Some(read_certificate_get_candidate(store, command)),
        "certificate.get_completion" => Some(read_certificate_get_completion(store, command)),
        "capability_broker.validate_action_origin" => {
            Some(read_capability_broker_validate_action_origin(command))
        }
        "playbook.get" => Some(read_playbook_get(store, command)),
        "playbook.check_current" => Some(read_playbook_check_current(store, command)),
        "playbook.role_output_may_decide_state" => {
            Some(read_playbook_role_output_may_decide_state(command))
        }
        "node.get" => Some(read_node_get(store, command)),
        "clarification.may_proceed_with_default_assumption" => {
            Some(read_clarification_may_proceed_with_default_assumption(
                command,
            ))
        }
        "clarification.no_unresolved_material_assumptions" => Some(
            read_clarification_no_unresolved_material_assumptions(command),
        ),
        "historical_red_light.classify_pre_existing_failure" => Some(
            read_historical_red_light_classify_pre_existing_failure(command),
        ),
        "historical_red_light.evaluate" => {
            Some(read_historical_red_light_evaluate(command))
        }
        "credential.get" => Some(read_credential_get(store, command)),
        "credential.list_receipts" => Some(read_credential_list_receipts(store, command)),
        "step_role.canonical_bindings" => Some(read_step_role_canonical_bindings()),
        "step_role.validate_schema" => Some(read_step_role_validate_schema(command)),
        "step_role.role_properties" => Some(read_step_role_role_properties(command)),
        "user_correction.get" => Some(read_user_correction_get(store, command)),
        "policy_restart.get_planning_restart" => {
            Some(read_planning_policy_restart_get(store, command))
        }
        "policy_restart.get_run_amendment" => Some(read_run_policy_amendment_get(store, command)),
        "policy_restart.get_budget_grant" => Some(read_budget_grant_get(store, command)),
        "project_intent.get_revision" => Some(read_project_intent_revision_get(store, command)),
        "project_intent.get_current_revision" => {
            Some(read_current_project_intent_revision_get(store, command))
        }
        "project_intent.get_amendment" => Some(read_project_intent_amendment_get(store, command)),
        "project_intent.get_initialization_receipt" => {
            Some(read_project_initialization_receipt_get(store, command))
        }
        "bounded_failure.evaluate_stall" => Some(read_bounded_failure_evaluate_stall(command)),
        "bounded_failure.may_auto_retry" => Some(read_bounded_failure_may_auto_retry(command)),
        "bounded_failure.backoff_delay_ms" => {
            Some(read_bounded_failure_backoff_delay_ms(command))
        }
        "bounded_failure.check_budget" => Some(read_bounded_failure_check_budget(command)),
        "bounded_failure.has_exceeded_node_attempt_limit" => Some(
            read_bounded_failure_has_exceeded_node_attempt_limit(command),
        ),
        "bounded_failure.has_exceeded_replan_limit" => {
            Some(read_bounded_failure_has_exceeded_replan_limit(command))
        }
        "model_selection.get_qualification_receipt" => {
            Some(read_qualification_receipt_get(store, command))
        }
        "model_selection.validate_sealed_pass_window" => {
            Some(read_model_selection_validate_sealed_pass_window(command))
        }
        "model_selection.validate_model_separation" => {
            Some(read_model_selection_validate_model_separation(command))
        }
        "skill.get_install_receipt" => Some(read_skill_install_receipt_get(store, command)),
        "skill.get_evidence_ladder" => Some(read_skill_evidence_ladder_get(store, command)),
        "skill.get_global_binding" => Some(read_global_skill_binding_get(store, command)),
        "skill.get_project_binding" => Some(read_project_skill_binding_get(store, command)),
        "skill.resolve_binding" => Some(read_skill_resolve_binding(store, command)),
        "skill.can_garbage_collect" => Some(read_skill_can_garbage_collect(command)),
        "review.get_receipt" => Some(read_human_review_receipt_get(store, command)),
        "review.can_resubmit" => Some(read_review_can_resubmit(command)),
        "review.validate_carried_planning_bundle" => {
            Some(read_review_validate_carried_planning_bundle(command))
        }
        "config.get_global_revision" => Some(read_global_config_revision_get(store, command)),
        "config.get_current_global_revision" => {
            Some(read_current_global_config_revision_get(store))
        }
        "config.get_project_patch" => Some(read_project_config_patch_get(store, command)),
        "config.resolve_project_config" => Some(read_config_resolve_project_config(store, command)),
        "replan.evaluate_proposal" => Some(read_replan_evaluate_proposal(command)),
        "replan.get_authorization" => Some(read_replan_get_authorization(store, command)),
        "replan.get_contract_amendment_authorization" => Some(
            read_replan_get_contract_amendment_authorization(store, command),
        ),
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

/// Read counterpart to `handle_record_readiness`. Takes `{ receipt_digest }`;
/// `NotFound` if no receipt has been recorded with that digest yet.
fn read_readiness_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_digest =
        parse_string_param(command, "receipt_digest").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_readiness(&receipt_digest)
        .map_err(internal_error)?
    {
        Some(record) => Ok(readiness_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no readiness receipt recorded with digest {receipt_digest}"),
        )),
    }
}

/// §5.9: `{ receipt_digest, fingerprint }` -- re-exercises both
/// `ReadinessReceipt::is_current_against` (against the caller-supplied
/// *current* fingerprint) and `ReadinessReceipt::is_ready`, reported as two
/// separate booleans since a receipt can be current-but-not-ready (missing
/// programs) or ready-but-stale (environment moved on since it was
/// observed). `NotFound` if `receipt_digest` was never recorded.
fn read_readiness_check(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let receipt_digest =
        parse_string_param(command, "receipt_digest").map_err(dispatch_error_to_reply_error)?;
    let fingerprint =
        parse_readiness_fingerprint_param(command).map_err(dispatch_error_to_reply_error)?;
    let record = store
        .load_readiness(&receipt_digest)
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                ReplyErrorCode::NotFound,
                format!("no readiness receipt recorded with digest {receipt_digest}"),
            )
        })?;
    let current = record.receipt.is_current_against(&fingerprint);
    let ready = record.receipt.is_ready();
    Ok(serde_json::json!({
        "receipt_digest": receipt_digest,
        "current": current,
        "ready": ready,
    }))
}

/// §5.12: `{ run_id }` -- returns the full stored chain (subject plus every
/// rung recorded so far). `NotFound` if `delivery.start` was never called
/// for `run_id`.
fn read_delivery_get(store: &EventStore, command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    match store.load_delivery_chain(&run_id).map_err(internal_error)? {
        Some(record) => Ok(delivery_chain_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no delivery chain started for run {run_id}"),
        )),
    }
}

/// §5.12: `{ run_id }` -- rebuilds the live `DeliveryChain` from its stored
/// rungs and re-exercises `DeliveryChain::is_ready_for_completion` against
/// it, rather than trusting a caller's own judgment of whether every rung is
/// in place. `NotFound` if `delivery.start` was never called for `run_id`.
fn read_delivery_check_completion(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let record = store
        .load_delivery_chain(&run_id)
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                ReplyErrorCode::NotFound,
                format!("no delivery chain started for run {run_id}"),
            )
        })?;
    match record.rebuild().is_ready_for_completion() {
        Ok(()) => Ok(serde_json::json!({
            "run_id": run_id,
            "ready": true,
            "reason": Value::Null,
        })),
        Err(e) => Ok(serde_json::json!({
            "run_id": run_id,
            "ready": false,
            "reason": format!("{e:?}"),
        })),
    }
}

/// §5.8: `{ run_id }` -- returns the recorded `CandidateCertificate` for
/// `run_id`. `NotFound` if `certificate.issue_candidate` was never called
/// for it.
fn read_certificate_get_candidate(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_candidate_certificate(&run_id)
        .map_err(internal_error)?
    {
        Some(record) => Ok(candidate_certificate_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no candidate certificate issued for run {run_id}"),
        )),
    }
}

/// §5.8: `{ run_id }` -- returns the recorded `CompletionCertificate` for
/// `run_id`. `NotFound` if `certificate.issue_completion` was never called
/// for it.
fn read_certificate_get_completion(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_completion_certificate(&run_id)
        .map_err(internal_error)?
    {
        Some(record) => Ok(completion_certificate_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no completion certificate issued for run {run_id}"),
        )),
    }
}

/// §8.2: `{ action, origin }` -- stateless gate. Unlike every other read in
/// this file, this one touches no store at all: `capability_broker.rs`
/// records no facts, it just answers "is this origin allowed to cause this
/// action kind." Returns `{ allowed: bool, action, reason }` rather than
/// erroring on a negative result, matching the `evidence.check` /
/// `readiness.check` / `delivery.check_completion` "check" convention.
fn read_capability_broker_validate_action_origin(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let action =
        parse_broker_action_kind_param(command).map_err(dispatch_error_to_reply_error)?;
    let origin =
        parse_broker_action_origin_param(command).map_err(dispatch_error_to_reply_error)?;
    match capability_broker::validate_broker_action_origin(action, origin) {
        Ok(()) => Ok(serde_json::json!({
            "allowed": true,
            "action": action,
            "reason": Value::Null,
        })),
        Err(BrokerActionError::RequiresUserInitiationNotAgentProposal(action)) => {
            Ok(serde_json::json!({
                "allowed": false,
                "action": action,
                "reason": "RequiresUserInitiationNotAgentProposal",
            }))
        }
    }
}

/// §6.4: `{ candidate, mandatory_triggers }` -- stateless gate, same shape
/// as `read_capability_broker_validate_action_origin`: `clarification.rs`
/// records no facts, it just answers whether Core may proceed on a
/// default assumption instead of pausing to ask. Returns
/// `{ may_proceed: bool }` rather than erroring when the answer is no.
fn read_clarification_may_proceed_with_default_assumption(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let candidate =
        parse_default_assumption_candidate_param(command).map_err(dispatch_error_to_reply_error)?;
    let mandatory_triggers =
        parse_mandatory_triggers_param(command).map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "may_proceed": clarification::may_proceed_with_default_assumption(
            &candidate,
            &mandatory_triggers,
        ),
    }))
}

fn parse_default_assumption_candidate_param(
    command: &Command,
) -> Result<DefaultAssumptionCandidate, DispatchError> {
    let value = command.params.get("candidate").cloned().ok_or_else(|| {
        DispatchError::InvalidParams("params.candidate is required".to_string())
    })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.candidate is not a valid DefaultAssumptionCandidate: {e}"
        ))
    })
}

fn parse_mandatory_triggers_param(
    command: &Command,
) -> Result<Vec<MandatoryClarificationTrigger>, DispatchError> {
    let value = command
        .params
        .get("mandatory_triggers")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams("params.mandatory_triggers is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.mandatory_triggers is not a valid Vec<MandatoryClarificationTrigger>: {e}"
        ))
    })
}

/// §7: `{ assumptions }` -- re-exercises
/// `no_unresolved_material_assumptions` (the completion gate's
/// `no_open_blocking_question_or_material_assumption` condition) against a
/// caller-supplied list rather than trusting the caller's own count of
/// what's still open. Returns `{ no_unresolved_material_assumptions: bool }`.
fn read_clarification_no_unresolved_material_assumptions(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let assumptions =
        parse_material_assumptions_param(command).map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "no_unresolved_material_assumptions":
            clarification::no_unresolved_material_assumptions(&assumptions),
    }))
}

fn parse_material_assumptions_param(
    command: &Command,
) -> Result<Vec<MaterialAssumption>, DispatchError> {
    let value = command
        .params
        .get("assumptions")
        .cloned()
        .ok_or_else(|| {
            DispatchError::InvalidParams("params.assumptions is required".to_string())
        })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.assumptions is not a valid Vec<MaterialAssumption>: {e}"
        ))
    })
}

/// §7.2: `{ reproduced_under_specified_conditions, final_fingerprint_matches_baseline }`
/// -- stateless gate, same shape as `read_capability_broker_validate_action_origin`:
/// `historical_red_light.rs` records no facts, it just classifies a
/// pre-existing failure relative to a baseline. Returns
/// `{ classification: FailureClassification }`.
fn read_historical_red_light_classify_pre_existing_failure(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let reproduced =
        parse_bool_param(command, "reproduced_under_specified_conditions")
            .map_err(dispatch_error_to_reply_error)?;
    let fingerprint_matches =
        parse_bool_param(command, "final_fingerprint_matches_baseline")
            .map_err(dispatch_error_to_reply_error)?;
    let classification =
        historical_red_light::classify_pre_existing_failure(reproduced, fingerprint_matches);
    Ok(serde_json::json!({ "classification": classification }))
}

/// §7.2: `{ assessment }` -- re-exercises `evaluate_historical_red_light`
/// (all seven conditions gating whether a task may complete while a
/// historical red light is present) against a caller-supplied assessment
/// rather than trusting the caller's own tally. Returns
/// `{ may_complete: bool, violations: [HistoricalRedLightViolation] }`
/// rather than erroring when the answer is no, collecting every unmet
/// condition rather than just the first, matching the
/// `delivery.check_completion` "check" convention.
fn read_historical_red_light_evaluate(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let assessment =
        parse_historical_red_light_assessment_param(command).map_err(dispatch_error_to_reply_error)?;
    let violations = historical_red_light::evaluate_historical_red_light(&assessment);
    Ok(serde_json::json!({
        "may_complete": violations.is_empty(),
        "violations": violations,
    }))
}

fn parse_historical_red_light_assessment_param(
    command: &Command,
) -> Result<HistoricalRedLightAssessment, DispatchError> {
    let value = command.params.get("assessment").cloned().ok_or_else(|| {
        DispatchError::InvalidParams("params.assessment is required".to_string())
    })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.assessment is not a valid HistoricalRedLightAssessment: {e}"
        ))
    })
}

/// §10.1: no params -- the fixed nine-entry `LoopStepId`→`LogicalRole`
/// table itself, so a caller assembling a candidate schema (or just
/// inspecting the canonical mapping) doesn't need to hardcode it a second
/// time. Stateless, same shape as `read_historical_red_light_classify_pre_existing_failure`.
fn read_step_role_canonical_bindings() -> Result<Value, (ReplyErrorCode, String)> {
    Ok(serde_json::json!({
        "bindings": step_role::canonical_step_role_bindings(),
    }))
}

/// §10.1: `{ candidate: [(LoopStepId, LogicalRole)] }` -- re-exercises
/// `validate_step_schema` server-side against a caller-assembled schema
/// rather than trusting the caller's own count of nine. Returns
/// `{ may_start_scheduler: bool, errors: [StepScheduleError] }`, collecting
/// every violation rather than just the first, matching the
/// `historical_red_light.evaluate` "check" convention.
fn read_step_role_validate_schema(command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let candidate = parse_step_role_candidate_param(command).map_err(dispatch_error_to_reply_error)?;
    let errors = step_role::validate_step_schema(&candidate);
    Ok(serde_json::json!({
        "may_start_scheduler": errors.is_empty(),
        "errors": errors,
    }))
}

fn parse_step_role_candidate_param(
    command: &Command,
) -> Result<Vec<(LoopStepId, LogicalRole)>, DispatchError> {
    let value = command
        .params
        .get("candidate")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.candidate is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.candidate is not a valid Vec<(LoopStepId, LogicalRole)>: {e}"
        ))
    })
}

/// §10.1: `{ role: LogicalRole }` -- the role table's two derived
/// properties (`write_access`/`requires_independent_model`) for a single
/// role, so a caller doesn't need to hardcode the table's own read-only
/// vs. candidate-write / independent-model columns a second time.
fn read_step_role_role_properties(command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let role = parse_step_role_role_param(command).map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "write_access": role.write_access(),
        "requires_independent_model": role.requires_independent_model(),
    }))
}

fn parse_step_role_role_param(command: &Command) -> Result<LogicalRole, DispatchError> {
    let value = command
        .params
        .get("role")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.role is required".to_string()))?;
    serde_json::from_value(value)
        .map_err(|e| DispatchError::InvalidParams(format!("params.role is not a valid LogicalRole: {e}")))
}

/// Read counterpart to `handle_record_credential`. Takes
/// `{ credential_ref }`; `NotFound` if no credential has ever been
/// recorded with that ref.
fn read_credential_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let credential_ref =
        parse_string_param(command, "credential_ref").map_err(dispatch_error_to_reply_error)?;
    match store
        .load_credential(&credential_ref)
        .map_err(internal_error)?
    {
        Some(row) => Ok(credential_record_row_json(&row)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no credential recorded with ref {credential_ref}"),
        )),
    }
}

/// Read counterpart to `handle_record_credential_receipt`. Takes
/// `{ credential_ref }`; an empty list (never `NotFound`) if no receipt
/// has ever been issued for that ref, same "list of records per key"
/// convention as `task.list`.
fn read_credential_list_receipts(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let credential_ref =
        parse_string_param(command, "credential_ref").map_err(dispatch_error_to_reply_error)?;
    let receipts = store
        .list_credential_receipts(&credential_ref)
        .map_err(internal_error)?;
    Ok(serde_json::json!({
        "credential_ref": credential_ref,
        "receipts": receipts
            .iter()
            .map(credential_receipt_json)
            .collect::<Vec<_>>(),
    }))
}

/// Read counterpart to `handle_bind_playbook`. Takes `{ run_id }`;
/// `NotFound` if no playbook has been bound for that Run yet.
fn read_playbook_get(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    match store.load_playbook(&run_id).map_err(internal_error)? {
        Some(record) => Ok(frozen_playbook_record_json(&record)),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no playbook bound for run {run_id}"),
        )),
    }
}

/// §10.3: `{ run_id, current }` -- re-exercises
/// `FrozenPlaybook::is_current_against` against the caller-supplied
/// *current* playbook, rather than trusting the caller's own staleness
/// judgment. `NotFound` if `run_id` never had a playbook bound (there is
/// nothing to check currency of).
fn read_playbook_check_current(
    store: &EventStore,
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let run_id = parse_string_param(command, "run_id").map_err(dispatch_error_to_reply_error)?;
    let current = parse_current_frozen_playbook_param(command)
        .map_err(dispatch_error_to_reply_error)?;
    let record = store
        .load_playbook(&run_id)
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                ReplyErrorCode::NotFound,
                format!("no playbook bound for run {run_id}"),
            )
        })?;
    let is_current = record.playbook.is_current_against(&current);
    Ok(serde_json::json!({
        "run_id": run_id,
        "current": is_current,
    }))
}

fn parse_current_frozen_playbook_param(command: &Command) -> Result<FrozenPlaybook, DispatchError> {
    let value = command
        .params
        .get("current")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.current is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.current is not a valid FrozenPlaybook: {e}"))
    })
}

/// §10.3: `{ role_output }` -- stateless gate, same shape as
/// `read_capability_broker_validate_action_origin`. `RoleOutput::Narrative`
/// has no path to a decision payload; only `Structured` does, so this
/// answers "may this output decide state" without the caller having to
/// duplicate `RoleOutput`'s own match logic.
fn read_playbook_role_output_may_decide_state(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let role_output =
        parse_role_output_param(command).map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "may_decide_state": role_output.may_decide_state(),
        "structured_payload": role_output.structured_payload(),
    }))
}

fn parse_role_output_param(command: &Command) -> Result<RoleOutput, DispatchError> {
    let value = command.params.get("role_output").cloned().ok_or_else(|| {
        DispatchError::InvalidParams("params.role_output is required".to_string())
    })?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.role_output is not a valid RoleOutput: {e}"))
    })
}

/// §6.3: `aggregate_id` is the caller-composed `"{run_id}:{node_id}"`
/// string (see `store::migrate_v12`'s doc comment) -- this module imposes
/// no structure on it, same as `require_aggregate_id` everywhere else.
/// `NotFound` if no event has ever been journaled for it, rather than
/// reporting the implicit `NodeStatus::Pending` default: unlike
/// `run.get`/`graph.get` (which don't exist as read methods at all, since
/// every write already returns the full state), a caller asking
/// `node.get` for an id nothing ever wrote to almost always has a typo'd
/// or stale id, not a legitimate "not yet started" query.
fn read_node_get(store: &EventStore, command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let aggregate_id = require_aggregate_id(command).map_err(dispatch_error_to_reply_error)?;
    match store
        .load_node_status(&aggregate_id)
        .map_err(internal_error)?
    {
        Some((revision, status)) => Ok(serde_json::json!({
            "aggregate_id": aggregate_id,
            "revision": revision,
            "status": status,
        })),
        None => Err((
            ReplyErrorCode::NotFound,
            format!("no node event ever journaled for {aggregate_id}"),
        )),
    }
}

/// §6.7 dispatch-layer-only mirror of `FrozenPolicySnapshot`'s
/// `dollar_budget` field. Unlike `BoundedBackoffPolicy` (whose fields are
/// private but carry no invariant beyond what `new` itself copies -- see
/// its own `Deserialize` derive above), `DollarBudget` deliberately has no
/// `Deserialize` impl: its only constructor is
/// `DollarBudget::classify(limit_cents, provider_usage_is_streamable_or_server_enforced)`,
/// and a `#[derive(Deserialize)]` placed directly on `DollarBudget` would
/// generate code in the same module that can set its private `kind` field
/// straight from JSON, bypassing `classify` entirely and letting a caller
/// self-label a locally-estimated cost `Hard`. This struct carries only the
/// fact `classify` actually accepts; `build_frozen_policy_snapshot` below
/// is the only place a `DollarBudget` gets constructed from IPC input.
#[derive(Debug, Clone, Deserialize)]
struct DollarBudgetInputParam {
    limit_cents: u64,
    provider_usage_is_streamable_or_server_enforced: bool,
}

/// §6.7 dispatch-layer-only mirror of `FrozenPolicySnapshot`, for the same
/// reason as `DollarBudgetInputParam`: `FrozenPolicySnapshot` transitively
/// contains `Option<DollarBudget>`, so it cannot get a direct `Deserialize`
/// derive either without the same bypass.
#[derive(Debug, Clone, Deserialize)]
struct FrozenPolicySnapshotInputParam {
    max_attempts_per_node: u32,
    max_replan_count: u32,
    max_wall_clock_seconds: u64,
    max_turns: u32,
    max_tokens: u64,
    #[serde(default)]
    dollar_budget: Option<DollarBudgetInputParam>,
}

fn build_frozen_policy_snapshot(input: FrozenPolicySnapshotInputParam) -> FrozenPolicySnapshot {
    FrozenPolicySnapshot {
        max_attempts_per_node: input.max_attempts_per_node,
        max_replan_count: input.max_replan_count,
        max_wall_clock_seconds: input.max_wall_clock_seconds,
        max_turns: input.max_turns,
        max_tokens: input.max_tokens,
        dollar_budget: input.dollar_budget.map(|d| {
            DollarBudget::classify(
                d.limit_cents,
                d.provider_usage_is_streamable_or_server_enforced,
            )
        }),
    }
}

/// §6.7: `{ history: [FailureOccurrence] }` -- stateless gate, same shape
/// as `read_historical_red_light_classify_pre_existing_failure`. Returns
/// `{ hold: Option<RunHold> }`.
fn read_bounded_failure_evaluate_stall(command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let history =
        parse_failure_occurrence_history_param(command).map_err(dispatch_error_to_reply_error)?;
    let hold = bounded_failure::evaluate_stall(&history);
    Ok(serde_json::json!({ "hold": hold }))
}

fn parse_failure_occurrence_history_param(
    command: &Command,
) -> Result<Vec<FailureOccurrence>, DispatchError> {
    let value = command
        .params
        .get("history")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.history is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.history is not a valid Vec<FailureOccurrence>: {e}"
        ))
    })
}

/// §6.7: `{ category, attempt_index, policy }` -- re-exercises
/// `may_auto_retry` (transient errors retry within a bounded exponential
/// backoff; business failures never auto-retry) server-side. Returns
/// `{ may_retry: bool }`.
fn read_bounded_failure_may_auto_retry(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let category =
        parse_failure_category_param(command).map_err(dispatch_error_to_reply_error)?;
    let attempt_index =
        parse_u32_param(command, "attempt_index").map_err(dispatch_error_to_reply_error)?;
    let policy =
        parse_bounded_backoff_policy_param(command).map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "may_retry": bounded_failure::may_auto_retry(category, attempt_index, &policy),
    }))
}

fn parse_failure_category_param(command: &Command) -> Result<FailureCategory, DispatchError> {
    let value = command
        .params
        .get("category")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.category is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.category is not a valid FailureCategory: {e}"
        ))
    })
}

fn parse_bounded_backoff_policy_param(
    command: &Command,
) -> Result<BoundedBackoffPolicy, DispatchError> {
    let value = command
        .params
        .get("policy")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.policy is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.policy is not a valid BoundedBackoffPolicy: {e}"
        ))
    })
}

/// §6.7: `{ policy, attempt_index }` -- the same bounded exponential
/// backoff `may_auto_retry` uses internally, exposed directly so a caller
/// can show/schedule the delay without reimplementing the capped-growth
/// formula. Returns `{ delay_ms: u64 }`.
fn read_bounded_failure_backoff_delay_ms(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let policy =
        parse_bounded_backoff_policy_param(command).map_err(dispatch_error_to_reply_error)?;
    let attempt_index =
        parse_u32_param(command, "attempt_index").map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "delay_ms": policy.delay_ms_for_attempt(attempt_index),
    }))
}

/// §6.7: `{ snapshot: FrozenPolicySnapshotInputParam, usage: BudgetUsage }`
/// -- re-exercises `check_budget` (wall-clock/turns/tokens always hard;
/// the dollar limit's hardness is whatever `DollarBudget::classify`
/// already determined) server-side. Returns
/// `{ hard_exhausted: [BudgetLimitKind], soft_alerts: [BudgetLimitKind],
/// hold: Option<RunHold> }`, collecting every violation rather than just
/// the first, matching the `historical_red_light.evaluate` "check"
/// convention.
fn read_bounded_failure_check_budget(command: &Command) -> Result<Value, (ReplyErrorCode, String)> {
    let snapshot =
        parse_frozen_policy_snapshot_param(command).map_err(dispatch_error_to_reply_error)?;
    let usage = parse_budget_usage_param(command).map_err(dispatch_error_to_reply_error)?;
    let outcome = bounded_failure::check_budget(&snapshot, &usage);
    Ok(serde_json::json!({
        "hard_exhausted": outcome.hard_exhausted,
        "soft_alerts": outcome.soft_alerts,
        "hold": outcome.into_run_hold(),
    }))
}

fn parse_frozen_policy_snapshot_param(
    command: &Command,
) -> Result<FrozenPolicySnapshot, DispatchError> {
    let value = command
        .params
        .get("snapshot")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.snapshot is required".to_string()))?;
    let input: FrozenPolicySnapshotInputParam = serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!(
            "params.snapshot is not a valid FrozenPolicySnapshotInputParam: {e}"
        ))
    })?;
    Ok(build_frozen_policy_snapshot(input))
}

fn parse_budget_usage_param(command: &Command) -> Result<BudgetUsage, DispatchError> {
    let value = command
        .params
        .get("usage")
        .cloned()
        .ok_or_else(|| DispatchError::InvalidParams("params.usage is required".to_string()))?;
    serde_json::from_value(value).map_err(|e| {
        DispatchError::InvalidParams(format!("params.usage is not a valid BudgetUsage: {e}"))
    })
}

/// §6.7: `{ snapshot: FrozenPolicySnapshotInputParam, attempts_for_node }`
/// -- re-exercises `has_exceeded_node_attempt_limit` server-side. Returns
/// `{ exceeded: bool }`.
fn read_bounded_failure_has_exceeded_node_attempt_limit(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let snapshot =
        parse_frozen_policy_snapshot_param(command).map_err(dispatch_error_to_reply_error)?;
    let attempts_for_node =
        parse_u32_param(command, "attempts_for_node").map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "exceeded": bounded_failure::has_exceeded_node_attempt_limit(attempts_for_node, &snapshot),
    }))
}

/// §6.7: `{ snapshot: FrozenPolicySnapshotInputParam, replan_count }` --
/// re-exercises `has_exceeded_replan_limit` server-side. Returns
/// `{ exceeded: bool }`.
fn read_bounded_failure_has_exceeded_replan_limit(
    command: &Command,
) -> Result<Value, (ReplyErrorCode, String)> {
    let snapshot =
        parse_frozen_policy_snapshot_param(command).map_err(dispatch_error_to_reply_error)?;
    let replan_count =
        parse_u32_param(command, "replan_count").map_err(dispatch_error_to_reply_error)?;
    Ok(serde_json::json!({
        "exceeded": bounded_failure::has_exceeded_replan_limit(replan_count, &snapshot),
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

/// Mirrors `graph_event_envelope`, except the payload is a bare
/// `NodeStatus` rather than a struct -- see `store::migrate_v12`'s doc
/// comment for why there is no wrapper type to serialize instead.
fn node_event_envelope(aggregate_id: String, appended: AppendedNodeEvent) -> Event {
    Event {
        event_seq: appended.seq as u64,
        event_id: appended.event_id,
        aggregate_id,
        aggregate_revision: appended.revision,
        event_type: appended.event_type.to_string(),
        occurred_at: appended.occurred_at,
        payload: serde_json::to_value(appended.state).expect("NodeStatus always serializes"),
    }
}

/// All 16 NodeEvent variants carry no payload.
fn parameterless_node_event(method: &str) -> Option<NodeEvent> {
    use NodeEvent as E;
    Some(match method {
        "node.become_ready" => E::BecomeReady,
        "node.start_producing" => E::StartProducing,
        "node.claim_submitted" => E::ClaimSubmitted,
        "node.start_verifying" => E::StartVerifying,
        "node.verification_passed" => E::VerificationPassed,
        "node.verification_failed_repairable" => E::VerificationFailedRepairable,
        "node.verification_inconclusive" => E::VerificationInconclusive,
        "node.verification_protocol_violation" => E::VerificationProtocolViolation,
        "node.evaluation_accepted" => E::EvaluationAccepted,
        "node.evaluation_needs_repair" => E::EvaluationNeedsRepair,
        "node.evaluation_needs_replan" => E::EvaluationNeedsReplan,
        "node.evaluation_needs_human" => E::EvaluationNeedsHuman,
        "node.repair_ready" => E::RepairReady,
        "node.retry_after_inconclusive" => E::RetryAfterInconclusive,
        "node.isolate_and_retry" => E::IsolateAndRetry,
        "node.human_decision_recorded" => E::HumanDecisionRecorded,
        _ => return None,
    })
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
    fn every_parameterless_node_method_is_recognized() {
        let methods = [
            "node.become_ready",
            "node.start_producing",
            "node.claim_submitted",
            "node.start_verifying",
            "node.verification_passed",
            "node.verification_failed_repairable",
            "node.verification_inconclusive",
            "node.verification_protocol_violation",
            "node.evaluation_accepted",
            "node.evaluation_needs_repair",
            "node.evaluation_needs_replan",
            "node.evaluation_needs_human",
            "node.repair_ready",
            "node.retry_after_inconclusive",
            "node.isolate_and_retry",
            "node.human_decision_recorded",
        ];
        for method in methods {
            let (mut store, path) = temp_store();
            let cmd = command(method, json!({ "aggregate_id": "run-1:node-1" }));
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

    // --- node.* -----------------------------------------------------------

    #[test]
    fn node_become_ready_appends_and_returns_matching_event() {
        let (mut store, path) = temp_store();
        let cmd = command(
            "node.become_ready",
            json!({ "aggregate_id": "run-1:node-1" }),
        );
        let event = dispatch(&mut store, &cmd).unwrap();
        assert_eq!(event.aggregate_id, "run-1:node-1");
        assert_eq!(event.aggregate_revision, 1);
        assert_eq!(event.event_type, "BecomeReady");
        assert_eq!(event.event_seq, 1);
        let (revision, state) = store.load_node_status("run-1:node-1").unwrap().unwrap();
        assert_eq!(revision, event.aggregate_revision);
        assert_eq!(serde_json::to_value(state).unwrap(), event.payload);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn node_sequential_events_advance_revision() {
        let (mut store, path) = temp_store();
        dispatch(
            &mut store,
            &command(
                "node.become_ready",
                json!({ "aggregate_id": "run-1:node-1" }),
            ),
        )
        .unwrap();
        let event = dispatch(
            &mut store,
            &command(
                "node.start_producing",
                json!({ "aggregate_id": "run-1:node-1" }),
            ),
        )
        .unwrap();
        assert_eq!(event.aggregate_revision, 2);
        assert_eq!(event.event_type, "StartProducing");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn node_illegal_transition_surfaces_as_store_error() {
        let (mut store, path) = temp_store();
        // Pending cannot jump straight to VerificationPassed (§6.3):
        // BecomeReady/StartProducing/ClaimSubmitted/StartVerifying must
        // each happen first.
        let cmd = command(
            "node.verification_passed",
            json!({ "aggregate_id": "run-1:node-1" }),
        );
        let err = dispatch(&mut store, &cmd).unwrap_err();
        assert!(matches!(
            err,
            DispatchError::NodeStore(NodeAppendError::Transition(_))
        ));
        assert!(store.load_node_status("run-1:node-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn node_get_returns_current_status_for_a_known_node() {
        let (mut store, path) = temp_store();
        dispatch(
            &mut store,
            &command(
                "node.become_ready",
                json!({ "aggregate_id": "run-1:node-1" }),
            ),
        )
        .unwrap();
        let cmd = command("node.get", json!({ "aggregate_id": "run-1:node-1" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["aggregate_id"], "run-1:node-1");
                assert_eq!(payload["revision"], 1);
                assert_eq!(payload["status"], "Ready");
            }
            other => panic!("expected ReplyOutcome::Ok, got {other:?}"),
        }
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn node_get_is_not_found_before_any_event() {
        let (mut store, path) = temp_store();
        let cmd = command("node.get", json!({ "aggregate_id": "run-1:node-1" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected ReplyOutcome::Error, got {other:?}"),
        }
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

    fn well_formed_readiness_receipt_json(receipt_digest: &str, result: &str) -> Value {
        json!({
            "receipt": {
                "revision": 1,
                "scope": "Execution",
                "profile_hash": "profile-1",
                "environment_relevant_inputs_digest": "env-digest-1",
                "observed_at": "2026-09-15T00:00:00Z",
                "valid_until": "2026-09-16T00:00:00Z",
                "subject": {
                    "ExistingRepo": {
                        "repository_identity_hash": "repo-hash",
                        "base_commit": "base",
                        "target_head": "head",
                        "worktree_fingerprint": "wt-1",
                    },
                },
                "programs": [],
                "lockfile_hashes": [],
                "result": result,
                "missing": [],
                "receipt_digest": receipt_digest,
            },
        })
    }

    /// §5.9: `readiness.record` follows the same no-`Event`-produced shape
    /// as `evidence.record` above, and its payload round-trips through the
    /// `readiness.get` read command.
    #[test]
    fn handle_command_readiness_record_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("readiness.record failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["receipt"]["receipt_digest"], "RD-1");

        let get_cmd = command("readiness.get", json!({ "receipt_digest": "RD-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("readiness.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `readiness.get` for a `receipt_digest` with no recorded receipt is
    /// `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_readiness_get_is_not_found_when_no_receipt_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command("readiness.get", json!({ "receipt_digest": "no-such-receipt" }));
        let outcome = handle_command(&mut store, &get_cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.9: `readiness.check` reports `current: true, ready: true` when the
    /// caller-supplied current fingerprint matches the recorded receipt
    /// exactly and its result was `Ready` with nothing missing.
    #[test]
    fn handle_command_readiness_check_reports_current_and_ready_for_a_matching_fingerprint() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        handle_command(&mut store, &record_cmd);

        let check_cmd = command(
            "readiness.check",
            json!({
                "receipt_digest": "RD-1",
                "fingerprint": {
                    "profile_hash": "profile-1",
                    "environment_relevant_inputs_digest": "env-digest-1",
                    "subject": {
                        "ExistingRepo": {
                            "repository_identity_hash": "repo-hash",
                            "base_commit": "base",
                            "target_head": "head",
                            "worktree_fingerprint": "wt-1",
                        },
                    },
                },
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["current"], true);
                assert_eq!(payload["ready"], true);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("readiness.check failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.9: a changed `environment_relevant_inputs_digest` must report
    /// `current: false` even though the receipt's own `result` is still
    /// `Ready` -- current-ness and readiness are independent booleans.
    #[test]
    fn handle_command_readiness_check_reports_stale_for_a_mismatched_fingerprint() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        handle_command(&mut store, &record_cmd);

        let check_cmd = command(
            "readiness.check",
            json!({
                "receipt_digest": "RD-1",
                "fingerprint": {
                    "profile_hash": "profile-1",
                    "environment_relevant_inputs_digest": "env-digest-2",
                    "subject": {
                        "ExistingRepo": {
                            "repository_identity_hash": "repo-hash",
                            "base_commit": "base",
                            "target_head": "head",
                            "worktree_fingerprint": "wt-1",
                        },
                    },
                },
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["current"], false);
                assert_eq!(payload["ready"], true);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("readiness.check failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.9: a `NotReady` result must report `ready: false` regardless of
    /// whether the fingerprint is current.
    #[test]
    fn handle_command_readiness_check_reports_not_ready_for_a_not_ready_result() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "NotReady"),
        );
        handle_command(&mut store, &record_cmd);

        let check_cmd = command(
            "readiness.check",
            json!({
                "receipt_digest": "RD-1",
                "fingerprint": {
                    "profile_hash": "profile-1",
                    "environment_relevant_inputs_digest": "env-digest-1",
                    "subject": {
                        "ExistingRepo": {
                            "repository_identity_hash": "repo-hash",
                            "base_commit": "base",
                            "target_head": "head",
                            "worktree_fingerprint": "wt-1",
                        },
                    },
                },
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["current"], true);
                assert_eq!(payload["ready"], false);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("readiness.check failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `readiness.check` for a `receipt_digest` with no recorded receipt is
    /// `NotFound` -- there is nothing to check staleness or readiness of.
    #[test]
    fn handle_command_readiness_check_is_not_found_when_no_receipt_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let check_cmd = command(
            "readiness.check",
            json!({
                "receipt_digest": "no-such-receipt",
                "fingerprint": {
                    "profile_hash": "profile-1",
                    "environment_relevant_inputs_digest": "env-digest-1",
                    "subject": {
                        "ExistingRepo": {
                            "repository_identity_hash": "repo-hash",
                            "base_commit": "base",
                            "target_head": "head",
                            "worktree_fingerprint": "wt-1",
                        },
                    },
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

    fn well_formed_existing_repo_subject_json() -> Value {
        json!({
            "ExistingRepo": {
                "repository_identity_hash": "repo-hash",
                "target_head": "head",
                "target_worktree_fingerprint": "wt-1",
                "new_ref": "refs/heads/delivered",
            },
        })
    }

    fn well_formed_delivery_envelope_json() -> Value {
        json!({
            "run_id": "run-1",
            "contract_hash": "contract-1",
            "candidate_certificate_hash": "candidate-1",
            "policy_hash": "policy-1",
            "nonce": "nonce-1",
            "issued_at": "2026-09-15T00:00:00Z",
            "receipt_digest": "digest-1",
        })
    }

    fn well_formed_rehearsal_receipt_json() -> Value {
        json!({
            "envelope": well_formed_delivery_envelope_json(),
            "subject": well_formed_existing_repo_subject_json(),
            "target_head_or_parent": "head",
            "delivery_tree_hash": "tree-1",
            "check_receipt_ids": ["check-1"],
        })
    }

    fn well_formed_approval_receipt_json() -> Value {
        json!({
            "envelope": well_formed_delivery_envelope_json(),
            "rehearsal_receipt_digest": "digest-1",
            "display_summary": "summary",
            "destination_or_new_ref": "refs/heads/delivered",
            "artifact_destinations": [],
            "valid_until": "2026-09-16T00:00:00Z",
            "operator_decision_ref": "decision:1",
        })
    }

    fn well_formed_delivery_receipt_json(outcome: &str) -> Value {
        json!({
            "envelope": well_formed_delivery_envelope_json(),
            "approval_receipt_digest": "digest-1",
            "before_identity_hash": "before-1",
            "after_identity_hash": "after-1",
            "outcome": outcome,
        })
    }

    fn well_formed_tree_check_receipt_json(matches_delivery: bool) -> Value {
        json!({
            "envelope": well_formed_delivery_envelope_json(),
            "delivery_receipt_digest": "digest-1",
            "observed_ref_or_tree": "refs/heads/delivered",
            "artifact_hashes": [],
            "worktree_fingerprint": "wt-1",
            "matches_delivery": matches_delivery,
        })
    }

    /// §5.12: `delivery.start` followed by `delivery.append_rehearsal`,
    /// `delivery.append_approval`, `delivery.append_delivery` and
    /// `delivery.append_tree_check` in order, for an `ExistingRepo` subject
    /// (no `project_target_transition` rung required). `delivery.get` reads
    /// back the same payload every append returned, and
    /// `delivery.check_completion` reports ready.
    #[test]
    fn handle_command_delivery_chain_full_existing_repo_happy_path_reads_back_and_completes() {
        let (mut store, root) = temp_store_with_isolated_root();

        let start_cmd = command(
            "delivery.start",
            json!({ "run_id": "run-1", "subject": well_formed_existing_repo_subject_json() }),
        );
        let start_outcome = handle_command(&mut store, &start_cmd);
        assert!(start_outcome.event.is_none());
        match start_outcome.reply.outcome {
            ReplyOutcome::Ok { .. } => {}
            ReplyOutcome::Error { code, message } => {
                panic!("delivery.start failed: {code:?} {message}")
            }
        }

        let rehearsal_cmd = command(
            "delivery.append_rehearsal",
            json!({ "run_id": "run-1", "receipt": well_formed_rehearsal_receipt_json() }),
        );
        let rehearsal_outcome = handle_command(&mut store, &rehearsal_cmd);
        assert!(rehearsal_outcome.event.is_none());
        if let ReplyOutcome::Error { code, message } = rehearsal_outcome.reply.outcome {
            panic!("delivery.append_rehearsal failed: {code:?} {message}")
        }

        let approval_cmd = command(
            "delivery.append_approval",
            json!({ "run_id": "run-1", "receipt": well_formed_approval_receipt_json() }),
        );
        let approval_outcome = handle_command(&mut store, &approval_cmd);
        if let ReplyOutcome::Error { code, message } = approval_outcome.reply.outcome {
            panic!("delivery.append_approval failed: {code:?} {message}")
        }

        let delivery_cmd = command(
            "delivery.append_delivery",
            json!({ "run_id": "run-1", "receipt": well_formed_delivery_receipt_json("Succeeded") }),
        );
        let delivery_outcome = handle_command(&mut store, &delivery_cmd);
        if let ReplyOutcome::Error { code, message } = delivery_outcome.reply.outcome {
            panic!("delivery.append_delivery failed: {code:?} {message}")
        }

        let tree_check_cmd = command(
            "delivery.append_tree_check",
            json!({ "run_id": "run-1", "receipt": well_formed_tree_check_receipt_json(true) }),
        );
        let tree_check_outcome = handle_command(&mut store, &tree_check_cmd);
        let tree_check_payload = match tree_check_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("delivery.append_tree_check failed: {code:?} {message}")
            }
        };

        let get_cmd = command("delivery.get", json!({ "run_id": "run-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload, tree_check_payload),
            ReplyOutcome::Error { code, message } => {
                panic!("delivery.get failed: {code:?} {message}")
            }
        }

        let check_cmd = command("delivery.check_completion", json!({ "run_id": "run-1" }));
        let check_outcome = handle_command(&mut store, &check_cmd);
        match check_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ready"], true);
                assert_eq!(payload["reason"], Value::Null);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("delivery.check_completion failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `delivery.append_approval` before any `delivery.append_rehearsal`
    /// call is rejected by the domain chain's own ordering rule, surfaced
    /// as `TransitionRejected`, not a silent no-op.
    #[test]
    fn handle_command_delivery_append_approval_before_rehearsal_is_transition_rejected() {
        let (mut store, root) = temp_store_with_isolated_root();

        let start_cmd = command(
            "delivery.start",
            json!({ "run_id": "run-1", "subject": well_formed_existing_repo_subject_json() }),
        );
        handle_command(&mut store, &start_cmd);

        let approval_cmd = command(
            "delivery.append_approval",
            json!({ "run_id": "run-1", "receipt": well_formed_approval_receipt_json() }),
        );
        let outcome = handle_command(&mut store, &approval_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `delivery.append_rehearsal` for a `run_id` that was never passed to
    /// `delivery.start` is `NotFound`, and `delivery.get` /
    /// `delivery.check_completion` are likewise `NotFound` for an unstarted
    /// chain.
    #[test]
    fn handle_command_delivery_methods_are_not_found_for_an_unstarted_chain() {
        let (mut store, root) = temp_store_with_isolated_root();

        let rehearsal_cmd = command(
            "delivery.append_rehearsal",
            json!({ "run_id": "no-such-run", "receipt": well_formed_rehearsal_receipt_json() }),
        );
        match handle_command(&mut store, &rehearsal_cmd).reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        let get_cmd = command("delivery.get", json!({ "run_id": "no-such-run" }));
        match handle_command(&mut store, &get_cmd).reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        let check_cmd = command("delivery.check_completion", json!({ "run_id": "no-such-run" }));
        match handle_command(&mut store, &check_cmd).reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `delivery.check_completion` reports `ready: false` with a reason
    /// while a chain has a delivery+tree_check but (for a `Greenfield`
    /// subject) still needs `project_target_transition` -- not an error,
    /// since asking "are we done yet" on an in-progress chain is a normal
    /// read, not a protocol violation.
    #[test]
    fn handle_command_delivery_check_completion_reports_not_ready_before_all_rungs_are_appended() {
        let (mut store, root) = temp_store_with_isolated_root();

        let start_cmd = command(
            "delivery.start",
            json!({ "run_id": "run-1", "subject": well_formed_existing_repo_subject_json() }),
        );
        handle_command(&mut store, &start_cmd);

        let check_cmd = command("delivery.check_completion", json!({ "run_id": "run-1" }));
        let outcome = handle_command(&mut store, &check_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ready"], false);
                assert!(payload["reason"].is_string());
            }
            ReplyOutcome::Error { code, message } => {
                panic!("delivery.check_completion failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_readiness_fingerprint_json() -> Value {
        json!({
            "profile_hash": "profile-1",
            "environment_relevant_inputs_digest": "env-digest-1",
            "subject": {
                "ExistingRepo": {
                    "repository_identity_hash": "repo-hash",
                    "base_commit": "base",
                    "target_head": "head",
                    "worktree_fingerprint": "wt-1",
                },
            },
        })
    }

    fn well_formed_audit_verdict_json(
        requirement_id: &str,
        outcome: &str,
        evidence_receipt_ids: &[&str],
    ) -> Value {
        json!({
            "requirement_id": requirement_id,
            "outcome": outcome,
            "evidence_receipt_ids": evidence_receipt_ids,
        })
    }

    fn issue_candidate_certificate_cmd(run_id: &str) -> Command {
        command(
            "certificate.issue_candidate",
            json!({
                "run_id": run_id,
                "contract_version": 1,
                "candidate_commit": "commit-1",
                "candidate_tree_hash": "tree-1",
                "must_requirement_ids": ["R-001"],
                "verdicts": [well_formed_audit_verdict_json("R-001", "Satisfied", &["EV-1"])],
                "valid_receipt_ids": ["EV-1"],
                "readiness_receipt_digest": "RD-1",
                "fingerprint": well_formed_readiness_fingerprint_json(),
            }),
        )
    }

    fn build_ready_delivery_chain_via_commands(store: &mut EventStore, run_id: &str) {
        let start_cmd = command(
            "delivery.start",
            json!({ "run_id": run_id, "subject": well_formed_existing_repo_subject_json() }),
        );
        if let ReplyOutcome::Error { code, message } = handle_command(store, &start_cmd).reply.outcome
        {
            panic!("delivery.start failed: {code:?} {message}")
        }

        let rehearsal_cmd = command(
            "delivery.append_rehearsal",
            json!({ "run_id": run_id, "receipt": well_formed_rehearsal_receipt_json() }),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(store, &rehearsal_cmd).reply.outcome
        {
            panic!("delivery.append_rehearsal failed: {code:?} {message}")
        }

        let approval_cmd = command(
            "delivery.append_approval",
            json!({ "run_id": run_id, "receipt": well_formed_approval_receipt_json() }),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(store, &approval_cmd).reply.outcome
        {
            panic!("delivery.append_approval failed: {code:?} {message}")
        }

        let delivery_cmd = command(
            "delivery.append_delivery",
            json!({ "run_id": run_id, "receipt": well_formed_delivery_receipt_json("Succeeded") }),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(store, &delivery_cmd).reply.outcome
        {
            panic!("delivery.append_delivery failed: {code:?} {message}")
        }

        let tree_check_cmd = command(
            "delivery.append_tree_check",
            json!({ "run_id": run_id, "receipt": well_formed_tree_check_receipt_json(true) }),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(store, &tree_check_cmd).reply.outcome
        {
            panic!("delivery.append_tree_check failed: {code:?} {message}")
        }
    }

    /// §5.8: `certificate.issue_candidate` composes an already-recorded
    /// readiness receipt (looked up by digest) with a satisfied verdict for
    /// every `must` requirement, produces no `Event`, and its payload reads
    /// back byte-for-byte through `certificate.get_candidate`.
    #[test]
    fn handle_command_certificate_candidate_issues_and_reads_back_a_well_formed_certificate() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(&mut store, &record_cmd).reply.outcome
        {
            panic!("readiness.record failed: {code:?} {message}")
        }

        let issue_cmd = issue_candidate_certificate_cmd("run-1");
        let issue_outcome = handle_command(&mut store, &issue_cmd);
        assert!(issue_outcome.event.is_none());
        let payload = match issue_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("certificate.issue_candidate failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["certificate"]["run_id"], "run-1");

        let get_cmd = command("certificate.get_candidate", json!({ "run_id": "run-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("certificate.get_candidate failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// Referencing a `readiness_receipt_digest` that was never recorded is
    /// `NotFound` -- there is no receipt to even check readiness/currency
    /// against yet.
    #[test]
    fn handle_command_certificate_issue_candidate_is_not_found_for_an_unrecorded_readiness_digest()
    {
        let (mut store, root) = temp_store_with_isolated_root();

        let issue_cmd = issue_candidate_certificate_cmd("run-1");
        let outcome = handle_command(&mut store, &issue_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `must` requirement with no verdict at all is rejected by the
    /// domain's own `issue_candidate_certificate`, surfaced as
    /// `TransitionRejected` rather than silently dropped.
    #[test]
    fn handle_command_certificate_issue_candidate_is_transition_rejected_for_a_missing_verdict() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        handle_command(&mut store, &record_cmd);

        let issue_cmd = command(
            "certificate.issue_candidate",
            json!({
                "run_id": "run-1",
                "contract_version": 1,
                "candidate_commit": "commit-1",
                "candidate_tree_hash": "tree-1",
                "must_requirement_ids": ["R-001"],
                "verdicts": [],
                "valid_receipt_ids": [],
                "readiness_receipt_digest": "RD-1",
                "fingerprint": well_formed_readiness_fingerprint_json(),
            }),
        );
        let outcome = handle_command(&mut store, &issue_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `certificate.get_candidate` for a `run_id` with no issued candidate
    /// certificate is `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_certificate_get_candidate_is_not_found_when_none_was_issued() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command("certificate.get_candidate", json!({ "run_id": "no-such-run" }));
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.8: `certificate.issue_completion` composes an already-issued
    /// candidate certificate with an already-ready delivery chain, produces
    /// no `Event`, and its payload reads back through
    /// `certificate.get_completion`.
    #[test]
    fn handle_command_certificate_completion_issues_and_reads_back_a_well_formed_certificate() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        handle_command(&mut store, &record_cmd);
        let issue_candidate_outcome =
            handle_command(&mut store, &issue_candidate_certificate_cmd("run-1"));
        if let ReplyOutcome::Error { code, message } = issue_candidate_outcome.reply.outcome {
            panic!("certificate.issue_candidate failed: {code:?} {message}")
        }
        build_ready_delivery_chain_via_commands(&mut store, "run-1");

        let issue_completion_cmd = command(
            "certificate.issue_completion",
            json!({
                "run_id": "run-1",
                "delivery_tree_hash": "tree-1",
                "user_approval_decision_ref": "decision:1",
            }),
        );
        let issue_outcome = handle_command(&mut store, &issue_completion_cmd);
        assert!(issue_outcome.event.is_none());
        let payload = match issue_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("certificate.issue_completion failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["run_id"], "run-1");

        let get_cmd = command("certificate.get_completion", json!({ "run_id": "run-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("certificate.get_completion failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `certificate.issue_completion` for a `run_id` with no issued
    /// candidate certificate is `NotFound` -- there is nothing to compose
    /// the completion certificate from.
    #[test]
    fn handle_command_certificate_issue_completion_is_not_found_without_a_candidate_certificate() {
        let (mut store, root) = temp_store_with_isolated_root();

        let issue_completion_cmd = command(
            "certificate.issue_completion",
            json!({
                "run_id": "run-1",
                "delivery_tree_hash": "tree-1",
                "user_approval_decision_ref": "decision:1",
            }),
        );
        let outcome = handle_command(&mut store, &issue_completion_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `certificate.issue_completion` for a `run_id` with a candidate
    /// certificate but no started delivery chain is `NotFound`.
    #[test]
    fn handle_command_certificate_issue_completion_is_not_found_without_a_delivery_chain() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        handle_command(&mut store, &record_cmd);
        let issue_candidate_outcome =
            handle_command(&mut store, &issue_candidate_certificate_cmd("run-1"));
        if let ReplyOutcome::Error { code, message } = issue_candidate_outcome.reply.outcome {
            panic!("certificate.issue_candidate failed: {code:?} {message}")
        }

        let issue_completion_cmd = command(
            "certificate.issue_completion",
            json!({
                "run_id": "run-1",
                "delivery_tree_hash": "tree-1",
                "user_approval_decision_ref": "decision:1",
            }),
        );
        let outcome = handle_command(&mut store, &issue_completion_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A blank `user_approval_decision_ref` is rejected by the domain's own
    /// `issue_completion_certificate`, surfaced as `TransitionRejected`.
    #[test]
    fn handle_command_certificate_issue_completion_is_transition_rejected_for_a_blank_approval_decision_ref(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "readiness.record",
            well_formed_readiness_receipt_json("RD-1", "Ready"),
        );
        handle_command(&mut store, &record_cmd);
        let issue_candidate_outcome =
            handle_command(&mut store, &issue_candidate_certificate_cmd("run-1"));
        if let ReplyOutcome::Error { code, message } = issue_candidate_outcome.reply.outcome {
            panic!("certificate.issue_candidate failed: {code:?} {message}")
        }
        build_ready_delivery_chain_via_commands(&mut store, "run-1");

        let issue_completion_cmd = command(
            "certificate.issue_completion",
            json!({
                "run_id": "run-1",
                "delivery_tree_hash": "tree-1",
                "user_approval_decision_ref": "   ",
            }),
        );
        let outcome = handle_command(&mut store, &issue_completion_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `certificate.get_completion` for a `run_id` with no issued
    /// completion certificate is `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_certificate_get_completion_is_not_found_when_none_was_issued() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command("certificate.get_completion", json!({ "run_id": "no-such-run" }));
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    const ALL_USER_INITIATED_ONLY_ACTION_KINDS: [&str; 5] = [
        "ProjectInitialization",
        "ConfigApplication",
        "EnvironmentTransaction",
        "SkillTransaction",
        "Delivery",
    ];

    #[test]
    fn handle_command_capability_broker_rejects_every_user_initiated_only_action_from_an_agent_proposal_origin(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        for action in ALL_USER_INITIATED_ONLY_ACTION_KINDS {
            let cmd = command(
                "capability_broker.validate_action_origin",
                json!({ "action": action, "origin": "AgentProposal" }),
            );
            let outcome = handle_command(&mut store, &cmd);
            let payload = match outcome.reply.outcome {
                ReplyOutcome::Ok { payload, .. } => payload,
                ReplyOutcome::Error { code, message } => {
                    panic!("capability_broker.validate_action_origin failed for {action}: {code:?} {message}")
                }
            };
            assert_eq!(payload["allowed"], false, "action = {action}");
            assert_eq!(
                payload["reason"], "RequiresUserInitiationNotAgentProposal",
                "action = {action}"
            );
            assert_eq!(payload["action"], action);
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_capability_broker_accepts_every_user_initiated_only_action_from_a_user_initiated_origin(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        for action in ALL_USER_INITIATED_ONLY_ACTION_KINDS {
            let cmd = command(
                "capability_broker.validate_action_origin",
                json!({ "action": action, "origin": "UserInitiatedFromUi" }),
            );
            let outcome = handle_command(&mut store, &cmd);
            let payload = match outcome.reply.outcome {
                ReplyOutcome::Ok { payload, .. } => payload,
                ReplyOutcome::Error { code, message } => {
                    panic!("capability_broker.validate_action_origin failed for {action}: {code:?} {message}")
                }
            };
            assert_eq!(payload["allowed"], true, "action = {action}");
            assert_eq!(payload["reason"], Value::Null, "action = {action}");
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_capability_broker_validate_action_origin_is_invalid_params_without_action() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "capability_broker.validate_action_origin",
            json!({ "origin": "UserInitiatedFromUi" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_capability_broker_validate_action_origin_is_invalid_params_without_origin() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "capability_broker.validate_action_origin",
            json!({ "action": "Delivery" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn all_conditions_met_candidate_json() -> Value {
        json!({
            "reversible": true,
            "locally_scoped": true,
            "preserves_user_goal": true,
            "preserves_acceptance_criteria": true,
            "no_high_risk_external_action": true,
            "verifiable_within_this_run": true,
            "has_stable_convention_support": true,
        })
    }

    #[test]
    fn handle_command_clarification_may_proceed_when_every_condition_holds_and_no_trigger_fired() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "clarification.may_proceed_with_default_assumption",
            json!({
                "candidate": all_conditions_met_candidate_json(),
                "mandatory_triggers": [],
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["may_proceed"], true),
            ReplyOutcome::Error { code, message } => {
                panic!("clarification.may_proceed_with_default_assumption failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_clarification_may_not_proceed_when_a_mandatory_trigger_fired() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "clarification.may_proceed_with_default_assumption",
            json!({
                "candidate": all_conditions_met_candidate_json(),
                "mandatory_triggers": ["ContradictoryUserRequirements"],
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["may_proceed"], false),
            ReplyOutcome::Error { code, message } => {
                panic!("clarification.may_proceed_with_default_assumption failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_clarification_may_proceed_is_invalid_params_without_candidate() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "clarification.may_proceed_with_default_assumption",
            json!({ "mandatory_triggers": [] }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_clarification_no_unresolved_material_assumptions_is_true_for_an_empty_list()
    {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "clarification.no_unresolved_material_assumptions",
            json!({ "assumptions": [] }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["no_unresolved_material_assumptions"], true)
            }
            ReplyOutcome::Error { code, message } => {
                panic!("clarification.no_unresolved_material_assumptions failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_clarification_no_unresolved_material_assumptions_is_false_when_one_is_open()
    {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "clarification.no_unresolved_material_assumptions",
            json!({
                "assumptions": [
                    { "id": "assumption-1", "statement": "Assumed default port 8080", "resolved": true },
                    { "id": "assumption-2", "statement": "Assumed SQLite over Postgres", "resolved": false },
                ],
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["no_unresolved_material_assumptions"], false)
            }
            ReplyOutcome::Error { code, message } => {
                panic!("clarification.no_unresolved_material_assumptions failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_clarification_no_unresolved_material_assumptions_is_invalid_params_without_assumptions(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("clarification.no_unresolved_material_assumptions", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_historical_red_light_classify_not_reproduced_is_unreproduced() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "historical_red_light.classify_pre_existing_failure",
            json!({
                "reproduced_under_specified_conditions": false,
                "final_fingerprint_matches_baseline": true,
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["classification"], "Unreproduced")
            }
            ReplyOutcome::Error { code, message } => panic!(
                "historical_red_light.classify_pre_existing_failure failed: {code:?} {message}"
            ),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_historical_red_light_classify_reproduced_with_matching_fingerprint_is_known_baseline_failure(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "historical_red_light.classify_pre_existing_failure",
            json!({
                "reproduced_under_specified_conditions": true,
                "final_fingerprint_matches_baseline": true,
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["classification"], "KnownBaselineFailure")
            }
            ReplyOutcome::Error { code, message } => panic!(
                "historical_red_light.classify_pre_existing_failure failed: {code:?} {message}"
            ),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_historical_red_light_classify_reproduced_with_differing_fingerprint_is_new_regression(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "historical_red_light.classify_pre_existing_failure",
            json!({
                "reproduced_under_specified_conditions": true,
                "final_fingerprint_matches_baseline": false,
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["classification"], "NewRegression")
            }
            ReplyOutcome::Error { code, message } => panic!(
                "historical_red_light.classify_pre_existing_failure failed: {code:?} {message}"
            ),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_historical_red_light_classify_is_invalid_params_without_reproduced_flag() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "historical_red_light.classify_pre_existing_failure",
            json!({ "final_fingerprint_matches_baseline": true }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn all_satisfied_historical_red_light_assessment_json() -> Value {
        json!({
            "failure_captured_by_core_before_change": true,
            "final_fingerprint_matches_baseline": true,
            "no_new_failures_skips_or_filtered": true,
            "impact_scope_check_confirmed_by_independent_reviewer": true,
            "impact_scope_check_passes_on_final_tree": true,
            "final_auditor_explicitly_accepted": true,
            "completion_certificate_fully_discloses": true,
        })
    }

    #[test]
    fn handle_command_historical_red_light_evaluate_may_complete_when_all_conditions_satisfied() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "historical_red_light.evaluate",
            json!({ "assessment": all_satisfied_historical_red_light_assessment_json() }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["may_complete"], true);
                assert_eq!(payload["violations"], json!([]));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("historical_red_light.evaluate failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_historical_red_light_evaluate_collects_every_unmet_condition() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut assessment = all_satisfied_historical_red_light_assessment_json();
        assessment["final_auditor_explicitly_accepted"] = json!(false);
        assessment["completion_certificate_fully_discloses"] = json!(false);
        let cmd = command("historical_red_light.evaluate", json!({ "assessment": assessment }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["may_complete"], false);
                assert_eq!(
                    payload["violations"],
                    json!([
                        "FinalAuditorDidNotExplicitlyAccept",
                        "CompletionCertificateDoesNotFullyDisclose",
                    ])
                );
            }
            ReplyOutcome::Error { code, message } => {
                panic!("historical_red_light.evaluate failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_historical_red_light_evaluate_is_invalid_params_without_assessment() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("historical_red_light.evaluate", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_credential_record_json() -> Value {
        json!({
            "provider": "anthropic",
            "auth_mode": "ApiKey",
            "storage_kind": "AutomeManagedKeychainItem",
            "storage_location": {
                "Keychain": {
                    "service": "com.autome.credentials",
                    "account": "anthropic-default",
                },
            },
            "created_at": "2026-09-14T00:00:00Z",
            "rotated_at": null,
            "revoked_at": null,
            "status": "Active",
        })
    }

    /// §8.3: `credential.record` follows the same no-`Event`-produced shape
    /// as `attempt.record`, and its payload round-trips through the
    /// `credential.get` read command.
    #[test]
    fn handle_command_credential_record_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "credential.record",
            json!({
                "credential_ref": "cred-1",
                "record": well_formed_credential_record_json(),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("credential.record failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["credential_ref"], "cred-1");
        assert_eq!(payload["record"]["provider"], "anthropic");

        let get_cmd = command("credential.get", json!({ "credential_ref": "cred-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("credential.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// Unlike `attempt.record`, a second `credential.record` call reusing
    /// the same `credential_ref` must succeed and overwrite the snapshot
    /// (rotation updates the one record in place), and `credential.get`
    /// must reflect the update.
    #[test]
    fn handle_command_credential_record_upserts_on_a_reused_credential_ref() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "credential.record",
            json!({
                "credential_ref": "cred-1",
                "record": well_formed_credential_record_json(),
            }),
        );
        handle_command(&mut store, &record_cmd);

        let mut rotated_record = well_formed_credential_record_json();
        rotated_record["status"] = json!("Rotated");
        rotated_record["rotated_at"] = json!("2026-09-14T02:00:00Z");
        let rotate_cmd = command(
            "credential.record",
            json!({
                "credential_ref": "cred-1",
                "record": rotated_record,
            }),
        );
        let outcome = handle_command(&mut store, &rotate_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { .. } => {}
            ReplyOutcome::Error { code, message } => {
                panic!("credential.record (rotate) failed: {code:?} {message}")
            }
        }

        let get_cmd = command("credential.get", json!({ "credential_ref": "cred-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["record"]["status"], "Rotated"),
            ReplyOutcome::Error { code, message } => {
                panic!("credential.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3: a `Revoked` status without `revoked_at` must be rejected as
    /// `TransitionRejected`, not written.
    #[test]
    fn handle_command_credential_record_rejects_a_shape_invalid_record() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut record = well_formed_credential_record_json();
        record["status"] = json!("Revoked");
        let record_cmd = command(
            "credential.record",
            json!({ "credential_ref": "cred-1", "record": record }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command("credential.get", json!({ "credential_ref": "cred-1" }));
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

    /// `credential.get` for a `credential_ref` with no recorded credential
    /// is `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_credential_get_is_not_found_when_no_credential_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command("credential.get", json!({ "credential_ref": "no-such-cred" }));
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3: `credential.issue_receipt` appends to the log and
    /// `credential.list_receipts` reads every receipt back in insertion
    /// order -- the first module where a repeat write is expected to
    /// succeed and accumulate, rather than being refused.
    #[test]
    fn handle_command_credential_issue_receipt_appends_and_lists_in_order() {
        let (mut store, root) = temp_store_with_isolated_root();

        let created_cmd = command(
            "credential.issue_receipt",
            json!({
                "credential_ref": "cred-1",
                "event": "Created",
                "occurred_at": "2026-09-14T00:00:00Z",
            }),
        );
        let outcome = handle_command(&mut store, &created_cmd);
        assert!(outcome.event.is_none());
        match outcome.reply.outcome {
            ReplyOutcome::Ok { .. } => {}
            ReplyOutcome::Error { code, message } => {
                panic!("credential.issue_receipt failed: {code:?} {message}")
            }
        }

        let rotated_cmd = command(
            "credential.issue_receipt",
            json!({
                "credential_ref": "cred-1",
                "event": "Rotated",
                "occurred_at": "2026-09-14T01:00:00Z",
            }),
        );
        handle_command(&mut store, &rotated_cmd);

        let uninstall_cmd = command(
            "credential.issue_receipt",
            json!({
                "credential_ref": "cred-1",
                "event": { "UninstallRetentionDecision": { "retained": false } },
                "occurred_at": "2026-09-14T02:00:00Z",
                "operator": "user-1",
            }),
        );
        handle_command(&mut store, &uninstall_cmd);

        let list_cmd = command("credential.list_receipts", json!({ "credential_ref": "cred-1" }));
        let list_outcome = handle_command(&mut store, &list_cmd);
        match list_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                let receipts = payload["receipts"].as_array().unwrap();
                assert_eq!(receipts.len(), 3);
                assert_eq!(receipts[0]["event"], "Created");
                assert_eq!(receipts[1]["event"], "Rotated");
                assert_eq!(
                    receipts[2]["event"],
                    json!({ "UninstallRetentionDecision": { "retained": false } })
                );
                assert_eq!(receipts[2]["operator"], "user-1");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("credential.list_receipts failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3: `credential.issue_receipt` re-validates server-side -- an
    /// `UninstallRetentionDecision` without an operator must be rejected
    /// as `TransitionRejected` and nothing appended to the log.
    #[test]
    fn handle_command_credential_issue_receipt_rejects_an_uninstall_decision_without_an_operator() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "credential.issue_receipt",
            json!({
                "credential_ref": "cred-1",
                "event": { "UninstallRetentionDecision": { "retained": true } },
                "occurred_at": "2026-09-14T00:00:00Z",
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let list_cmd = command("credential.list_receipts", json!({ "credential_ref": "cred-1" }));
        let list_outcome = handle_command(&mut store, &list_cmd);
        match list_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert!(payload["receipts"].as_array().unwrap().is_empty())
            }
            ReplyOutcome::Error { code, message } => {
                panic!("credential.list_receipts failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `credential.list_receipts` for an unknown `credential_ref` is an
    /// empty list, never `NotFound` -- same "list of records" convention
    /// as `task.list`.
    #[test]
    fn handle_command_credential_list_receipts_is_empty_for_an_unknown_credential_ref() {
        let (mut store, root) = temp_store_with_isolated_root();

        let list_cmd = command(
            "credential.list_receipts",
            json!({ "credential_ref": "no-such-cred" }),
        );
        let outcome = handle_command(&mut store, &list_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert!(payload["receipts"].as_array().unwrap().is_empty())
            }
            ReplyOutcome::Error { code, message } => {
                panic!("credential.list_receipts failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn frozen_playbook_json(playbook_id: &str, hash: &str) -> Value {
        json!({ "playbook_id": playbook_id, "manifest_content_hash": hash })
    }

    #[test]
    fn handle_command_playbook_binds_and_reads_back_a_well_formed_playbook() {
        let (mut store, root) = temp_store_with_isolated_root();

        let bind_cmd = command(
            "playbook.bind",
            json!({
                "run_id": "run-1",
                "playbook": frozen_playbook_json("ExistingRepoChange", "hash-1"),
            }),
        );
        let bind_outcome = handle_command(&mut store, &bind_cmd);
        assert!(bind_outcome.event.is_none());
        let payload = match bind_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("playbook.bind failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["run_id"], "run-1");

        let get_cmd = command("playbook.get", json!({ "run_id": "run-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("playbook.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_playbook_bind_refuses_to_reuse_an_existing_run_id() {
        let (mut store, root) = temp_store_with_isolated_root();

        let bind_cmd = command(
            "playbook.bind",
            json!({
                "run_id": "run-1",
                "playbook": frozen_playbook_json("ExistingRepoChange", "hash-1"),
            }),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(&mut store, &bind_cmd).reply.outcome
        {
            panic!("first playbook.bind failed: {code:?} {message}")
        }

        let outcome = handle_command(&mut store, &bind_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_playbook_get_is_not_found_when_none_was_bound() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command("playbook.get", json!({ "run_id": "no-such-run" }));
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_playbook_check_current_is_true_for_an_identical_playbook() {
        let (mut store, root) = temp_store_with_isolated_root();

        let bind_cmd = command(
            "playbook.bind",
            json!({
                "run_id": "run-1",
                "playbook": frozen_playbook_json("ExistingRepoChange", "hash-1"),
            }),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(&mut store, &bind_cmd).reply.outcome
        {
            panic!("playbook.bind failed: {code:?} {message}")
        }

        let check_cmd = command(
            "playbook.check_current",
            json!({
                "run_id": "run-1",
                "current": frozen_playbook_json("ExistingRepoChange", "hash-1"),
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("playbook.check_current failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["current"], true);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_playbook_check_current_is_false_for_a_drifted_content_hash() {
        let (mut store, root) = temp_store_with_isolated_root();

        let bind_cmd = command(
            "playbook.bind",
            json!({
                "run_id": "run-1",
                "playbook": frozen_playbook_json("ExistingRepoChange", "hash-1"),
            }),
        );
        if let ReplyOutcome::Error { code, message } =
            handle_command(&mut store, &bind_cmd).reply.outcome
        {
            panic!("playbook.bind failed: {code:?} {message}")
        }

        let check_cmd = command(
            "playbook.check_current",
            json!({
                "run_id": "run-1",
                "current": frozen_playbook_json("ExistingRepoChange", "hash-2"),
            }),
        );
        let outcome = handle_command(&mut store, &check_cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("playbook.check_current failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["current"], false);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_playbook_check_current_is_not_found_when_none_was_bound() {
        let (mut store, root) = temp_store_with_isolated_root();

        let check_cmd = command(
            "playbook.check_current",
            json!({
                "run_id": "no-such-run",
                "current": frozen_playbook_json("ExistingRepoChange", "hash-1"),
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
    fn handle_command_playbook_role_output_structured_may_decide_state() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "playbook.role_output_may_decide_state",
            json!({
                "role_output": {
                    "Structured": {
                        "schema_id": "contract_drafting.v1",
                        "payload": { "requirements": [] },
                    },
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("playbook.role_output_may_decide_state failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["may_decide_state"], true);
        assert_eq!(payload["structured_payload"], json!({ "requirements": [] }));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_playbook_role_output_narrative_may_not_decide_state() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "playbook.role_output_may_decide_state",
            json!({ "role_output": { "Narrative": "I think this looks about right." } }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("playbook.role_output_may_decide_state failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["may_decide_state"], false);
        assert_eq!(payload["structured_payload"], Value::Null);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_playbook_role_output_may_decide_state_is_invalid_params_without_role_output()
    {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("playbook.role_output_may_decide_state", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
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

    #[test]
    fn handle_command_step_role_canonical_bindings_returns_all_nine_steps() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("step_role.canonical_bindings", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                let bindings = payload["bindings"].as_array().expect("bindings is an array");
                assert_eq!(bindings.len(), 9);
                assert_eq!(bindings[0], json!(["fact_analysis", "Analyst"]));
                assert_eq!(bindings[5], json!(["implementation", "Implementer"]));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("step_role.canonical_bindings failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn canonical_step_role_candidate_json() -> Value {
        json!([
            ["fact_analysis", "Analyst"],
            ["contract_drafting", "Planner"],
            ["contract_review", "ContractReviewer"],
            ["task_graph_planning", "Planner"],
            ["graph_review", "ContractReviewer"],
            ["implementation", "Implementer"],
            ["repair", "Implementer"],
            ["node_evaluation", "Auditor"],
            ["final_audit", "Auditor"],
        ])
    }

    #[test]
    fn handle_command_step_role_validate_schema_may_start_scheduler_on_the_canonical_schema() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "step_role.validate_schema",
            json!({ "candidate": canonical_step_role_candidate_json() }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["may_start_scheduler"], true);
                assert_eq!(payload["errors"], json!([]));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("step_role.validate_schema failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_step_role_validate_schema_reports_a_missing_step_and_blocks_scheduler_start()
    {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut candidate = canonical_step_role_candidate_json();
        let array = candidate.as_array_mut().unwrap();
        array.retain(|entry| entry[0] != json!("repair"));

        let cmd = command("step_role.validate_schema", json!({ "candidate": candidate }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["may_start_scheduler"], false);
                assert_eq!(
                    payload["errors"],
                    json!([{ "MissingStep": "repair" }])
                );
            }
            ReplyOutcome::Error { code, message } => {
                panic!("step_role.validate_schema failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_step_role_validate_schema_is_invalid_params_without_candidate() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("step_role.validate_schema", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_step_role_role_properties_marks_implementer_as_the_only_write_role() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "step_role.role_properties",
            json!({ "role": "Implementer" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["write_access"], "CandidateWrite");
                assert_eq!(payload["requires_independent_model"], false);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("step_role.role_properties failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_step_role_role_properties_marks_auditor_as_requiring_an_independent_model() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("step_role.role_properties", json!({ "role": "Auditor" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["write_access"], "ReadOnly");
                assert_eq!(payload["requires_independent_model"], true);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("step_role.role_properties failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_step_role_role_properties_is_invalid_params_without_role() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("step_role.role_properties", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_user_correction_input_json() -> Value {
        json!({
            "project_id": "project-1",
            "task_id": "task-1",
            "run_id": "run-1",
            "attempt_id": null,
            "planning_spec_hash": "planning-spec-hash-1",
            "execution_spec_hash": null,
            "execution_spec_frozen": false,
            "raw_text_ref": "raw-text-ref-1",
            "attachment_hashes": [],
            "submitted_at": "2026-09-15T00:00:00Z",
            "operator": "dannie",
            "subject_contract_hash": null,
            "subject_graph_hash": null,
            "subject_candidate_hash": null,
            "classification": "PlanningRevision",
            "impact": {
                "would_delete_requirement": false,
                "would_relax_check": false,
                "would_change_project_intent": false,
                "adds_external_side_effect": false,
            },
            "affected_requirement_ids": [],
            "affected_node_ids": [],
            "disposition": "NewPlanningRunSpec",
            "successor_ref": null,
            "receipt_digest": "UC-1",
        })
    }

    /// §6.5: `user_correction.record` follows the same no-`Event`-produced
    /// shape as `credential.issue_receipt`, and its payload round-trips
    /// through the `user_correction.get` read command.
    #[test]
    fn handle_command_user_correction_record_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "user_correction.record",
            json!({ "input": well_formed_user_correction_input_json() }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("user_correction.record failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["receipt"]["receipt_digest"], "UC-1");
        assert_eq!(payload["receipt"]["classification"], "PlanningRevision");

        let get_cmd = command("user_correction.get", json!({ "receipt_digest": "UC-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("user_correction.get failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// Unlike `credential.record`, a second `user_correction.record` call
    /// reusing the same `receipt_digest` must be refused -- a correction
    /// receipt is a one-time fact, not current state to overwrite.
    #[test]
    fn handle_command_user_correction_record_refuses_to_reuse_an_existing_receipt_digest() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "user_correction.record",
            json!({ "input": well_formed_user_correction_input_json() }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §6.5: pre-freeze corrections must be classified `PlanningRevision`
    /// -- re-validated server-side via `issue_user_correction_receipt`
    /// rather than trusting the caller's own classification/disposition
    /// pairing.
    #[test]
    fn handle_command_user_correction_record_rejects_a_pre_freeze_non_planning_classification() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_user_correction_input_json();
        input["classification"] = json!("GraphStrategy");
        input["disposition"] = json!("ReplanProposal");
        let record_cmd = command("user_correction.record", json!({ "input": input }));
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command("user_correction.get", json!({ "receipt_digest": "UC-1" }));
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

    /// `user_correction.get` for a `receipt_digest` with no recorded
    /// correction is `NotFound`, not a silently empty payload.
    #[test]
    fn handle_command_user_correction_get_is_not_found_when_no_receipt_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command(
            "user_correction.get",
            json!({ "receipt_digest": "no-such-receipt" }),
        );
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_user_correction_record_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("user_correction.record", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_planning_policy_restart_input_json() -> Value {
        json!({
            "task_id": "task-1",
            "current_run_id": "run-1",
            "old_planning_spec_hash": "old-planning-spec-hash",
            "proposed_planning_spec_hash": "new-planning-spec-hash",
            "trigger_revision_ref": "trigger-rev-1",
            "config_revision_ref": "config-rev-1",
            "skill_revision_ref": "skill-rev-1",
            "capability_revision_ref": "capability-rev-1",
            "invalidated_document_attempt_ids": ["doc-attempt-1"],
            "approval_receipt": "approval-1",
            "restart_digest": "RESTART-1",
        })
    }

    /// §5.1: `policy_restart.issue_planning_restart` follows the same
    /// no-`Event`-produced shape as `user_correction.record`, and its
    /// payload round-trips through `policy_restart.get_planning_restart`.
    #[test]
    fn handle_command_policy_restart_issue_planning_restart_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "policy_restart.issue_planning_restart",
            json!({ "input": well_formed_planning_policy_restart_input_json() }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("policy_restart.issue_planning_restart failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["restart"]["restart_digest"], "RESTART-1");

        let get_cmd = command(
            "policy_restart.get_planning_restart",
            json!({ "restart_digest": "RESTART-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("policy_restart.get_planning_restart failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_policy_restart_issue_planning_restart_refuses_to_reuse_an_existing_restart_digest()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "policy_restart.issue_planning_restart",
            json!({ "input": well_formed_planning_policy_restart_input_json() }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_policy_restart_issue_planning_restart_rejects_an_unchanged_spec() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_planning_policy_restart_input_json();
        input["proposed_planning_spec_hash"] = json!("old-planning-spec-hash");
        let record_cmd = command(
            "policy_restart.issue_planning_restart",
            json!({ "input": input }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "policy_restart.get_planning_restart",
            json!({ "restart_digest": "RESTART-1" }),
        );
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

    #[test]
    fn handle_command_policy_restart_issue_planning_restart_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("policy_restart.issue_planning_restart", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_run_policy_amendment_input_json() -> Value {
        json!({
            "task_id": "task-1",
            "current_run_id": "run-1",
            "old_execution_spec_hash": "old-execution-spec-hash",
            "proposed_execution_spec_hash": "new-execution-spec-hash",
            "unchanged_contract_hash": "contract-hash-1",
            "unchanged_graph_hash": "graph-hash-1",
            "unchanged_base_hash": "base-hash-1",
            "policy_diff": "final_audit switched from claude-a to claude-b",
            "invalidated_attempt_ids": ["attempt-1"],
            "invalidated_evidence_ids": ["evidence-1"],
            "invalidated_audit_ids": ["audit-1"],
            "invalidated_candidate_ids": ["candidate-1"],
            "approval_receipt": "approval-1",
            "amendment_digest": "AMEND-1",
        })
    }

    /// §5.1: `policy_restart.issue_run_amendment` follows the same
    /// no-`Event`-produced shape, and its payload round-trips through
    /// `policy_restart.get_run_amendment`.
    #[test]
    fn handle_command_policy_restart_issue_run_amendment_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "policy_restart.issue_run_amendment",
            json!({ "input": well_formed_run_policy_amendment_input_json() }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("policy_restart.issue_run_amendment failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["amendment"]["amendment_digest"], "AMEND-1");

        let get_cmd = command(
            "policy_restart.get_run_amendment",
            json!({ "amendment_digest": "AMEND-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("policy_restart.get_run_amendment failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_policy_restart_issue_run_amendment_refuses_to_reuse_an_existing_amendment_digest()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "policy_restart.issue_run_amendment",
            json!({ "input": well_formed_run_policy_amendment_input_json() }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_policy_restart_issue_run_amendment_rejects_no_invalidated_attempts() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_run_policy_amendment_input_json();
        input["invalidated_attempt_ids"] = json!([]);
        let record_cmd = command(
            "policy_restart.issue_run_amendment",
            json!({ "input": input }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "policy_restart.get_run_amendment",
            json!({ "amendment_digest": "AMEND-1" }),
        );
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

    #[test]
    fn handle_command_policy_restart_issue_run_amendment_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("policy_restart.issue_run_amendment", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_budget_grant_input_json() -> Value {
        json!({
            "run_id": "run-1",
            "current_budget_hash": "budget-hash-1",
            "added_limits": [{ "Soft": 10 }],
            "reason": "extra retries needed after flaky environment",
            "operator": "dannie",
            "expiry": null,
            "grant_digest": "GRANT-1",
        })
    }

    /// §5.1: `policy_restart.issue_budget_grant` follows the same
    /// no-`Event`-produced shape, and its payload round-trips through
    /// `policy_restart.get_budget_grant`.
    #[test]
    fn handle_command_policy_restart_issue_budget_grant_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "policy_restart.issue_budget_grant",
            json!({ "input": well_formed_budget_grant_input_json() }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("policy_restart.issue_budget_grant failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["receipt"]["grant_digest"], "GRANT-1");

        let get_cmd = command(
            "policy_restart.get_budget_grant",
            json!({ "grant_digest": "GRANT-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("policy_restart.get_budget_grant failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_policy_restart_issue_budget_grant_refuses_to_reuse_an_existing_grant_digest() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "policy_restart.issue_budget_grant",
            json!({ "input": well_formed_budget_grant_input_json() }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_policy_restart_issue_budget_grant_rejects_no_limits_added() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_budget_grant_input_json();
        input["added_limits"] = json!([]);
        let record_cmd = command(
            "policy_restart.issue_budget_grant",
            json!({ "input": input }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "policy_restart.get_budget_grant",
            json!({ "grant_digest": "GRANT-1" }),
        );
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

    #[test]
    fn handle_command_policy_restart_issue_budget_grant_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("policy_restart.issue_budget_grant", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_project_intent_revision_input_json(revision: u32, supersedes: Option<u32>) -> Value {
        json!({
            "project_id": "project-1",
            "revision": revision,
            "source_anchors": ["README.md"],
            "approved_by": "dannie",
            "approved_at": "2026-09-14T00:00:00Z",
            "product_goal": "Ship a local-first digital employee",
            "target_users": ["solo developers"],
            "durable_cross_task_constraints": ["never phone home"],
            "explicit_non_goals": ["no multi-tenant support"],
            "key_decisions": [{
                "id": "kd-1",
                "statement": "Use SQLite for the event journal",
                "rationale": "Local-first, single-user, no server dependency",
                "source_ref": "docs/plan.md#L42",
            }],
            "supersedes": supersedes,
            "intent_hash": "intent-hash-1",
        })
    }

    /// §5.1: `project_intent.record_revision` follows the same
    /// no-`Event`-produced shape as `user_correction.record`, and its
    /// payload round-trips through `project_intent.get_revision`.
    #[test]
    fn handle_command_project_intent_record_revision_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "project_intent.record_revision",
            json!({ "input": well_formed_project_intent_revision_input_json(1, None) }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("project_intent.record_revision failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["revision"]["revision"], 1);
        assert_eq!(payload["revision"]["project_id"], "project-1");

        let get_cmd = command(
            "project_intent.get_revision",
            json!({ "project_id": "project-1", "revision": 1 }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("project_intent.get_revision failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_record_revision_refuses_to_reuse_an_existing_project_id_revision_pair()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "project_intent.record_revision",
            json!({ "input": well_formed_project_intent_revision_input_json(1, None) }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_record_revision_rejects_missing_approved_by() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_project_intent_revision_input_json(1, None);
        input["approved_by"] = json!("");
        let record_cmd = command("project_intent.record_revision", json!({ "input": input }));
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "project_intent.get_revision",
            json!({ "project_id": "project-1", "revision": 1 }),
        );
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

    #[test]
    fn handle_command_project_intent_get_revision_is_not_found_when_no_revision_was_recorded() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command(
            "project_intent.get_revision",
            json!({ "project_id": "no-such-project", "revision": 1 }),
        );
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_get_current_revision_returns_the_highest_revision() {
        let (mut store, root) = temp_store_with_isolated_root();

        handle_command(
            &mut store,
            &command(
                "project_intent.record_revision",
                json!({ "input": well_formed_project_intent_revision_input_json(1, None) }),
            ),
        );
        handle_command(
            &mut store,
            &command(
                "project_intent.record_revision",
                json!({ "input": well_formed_project_intent_revision_input_json(2, Some(1)) }),
            ),
        );

        let get_cmd = command(
            "project_intent.get_current_revision",
            json!({ "project_id": "project-1" }),
        );
        let outcome = handle_command(&mut store, &get_cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("project_intent.get_current_revision failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["revision"]["revision"], 2);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_record_revision_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("project_intent.record_revision", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_project_intent_amendment_input_json(from_revision: u32) -> Value {
        json!({
            "project_id": "project-1",
            "from_revision": from_revision,
            "trigger_task": "task-9",
            "semantic_diff": "Added a non-goal: no team accounts in 2.0.0",
            "affected_active_tasks": ["task-9"],
            "user_decision_receipt": "decision-ref-1",
            "amendment_hash": "AMEND-1",
        })
    }

    /// §5.1: `project_intent.record_amendment` follows the same
    /// no-`Event`-produced shape, and its payload round-trips through
    /// `project_intent.get_amendment`.
    #[test]
    fn handle_command_project_intent_record_amendment_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        handle_command(
            &mut store,
            &command(
                "project_intent.record_revision",
                json!({ "input": well_formed_project_intent_revision_input_json(1, None) }),
            ),
        );

        let record_cmd = command(
            "project_intent.record_amendment",
            json!({ "input": well_formed_project_intent_amendment_input_json(1) }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("project_intent.record_amendment failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["amendment"]["to_revision"], 2);

        let get_cmd = command(
            "project_intent.get_amendment",
            json!({ "amendment_hash": "AMEND-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("project_intent.get_amendment failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_record_amendment_refuses_to_reuse_an_existing_amendment_hash() {
        let (mut store, root) = temp_store_with_isolated_root();

        handle_command(
            &mut store,
            &command(
                "project_intent.record_revision",
                json!({ "input": well_formed_project_intent_revision_input_json(1, None) }),
            ),
        );
        let record_cmd = command(
            "project_intent.record_amendment",
            json!({ "input": well_formed_project_intent_amendment_input_json(1) }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// No revision has ever been recorded for this project -- the store's
    /// `NoCurrentRevision` prerequisite-missing case, mapped to `NotFound`
    /// same as `IssueCandidateCertificateError::ReadinessNotFound`.
    #[test]
    fn handle_command_project_intent_record_amendment_is_not_found_when_no_current_revision_exists()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "project_intent.record_amendment",
            json!({ "input": well_formed_project_intent_amendment_input_json(1) }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_record_amendment_rejects_stale_from_revision() {
        let (mut store, root) = temp_store_with_isolated_root();

        handle_command(
            &mut store,
            &command(
                "project_intent.record_revision",
                json!({ "input": well_formed_project_intent_revision_input_json(3, None) }),
            ),
        );

        let record_cmd = command(
            "project_intent.record_amendment",
            json!({ "input": well_formed_project_intent_amendment_input_json(1) }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "project_intent.get_amendment",
            json!({ "amendment_hash": "AMEND-1" }),
        );
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

    #[test]
    fn handle_command_project_intent_record_amendment_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("project_intent.record_amendment", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_project_initialization_receipt_input_json() -> Value {
        json!({
            "project_id": "project-1",
            "project_revision": 1,
            "subject_identity_hash": "identity-hash-1",
            "trust_decision_ref": null,
            "environment_snapshot_id": "env-snapshot-1",
            "skill_inventory_id": "skill-inventory-1",
            "project_home_manifest": "manifest-1",
            "result": "Ready",
            "issues": [],
            "receipt_digest": "RECEIPT-1",
        })
    }

    /// §5.1: `project_intent.record_initialization_receipt` follows the
    /// same no-`Event`-produced shape, and its payload round-trips through
    /// `project_intent.get_initialization_receipt`.
    #[test]
    fn handle_command_project_intent_record_initialization_receipt_produces_no_event_and_reads_back()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "project_intent.record_initialization_receipt",
            json!({ "input": well_formed_project_initialization_receipt_input_json() }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("project_intent.record_initialization_receipt failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["receipt"]["receipt_digest"], "RECEIPT-1");
        assert_eq!(payload["receipt"]["result"], "Ready");

        let get_cmd = command(
            "project_intent.get_initialization_receipt",
            json!({ "receipt_digest": "RECEIPT-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("project_intent.get_initialization_receipt failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_record_initialization_receipt_refuses_to_reuse_an_existing_receipt_digest()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "project_intent.record_initialization_receipt",
            json!({ "input": well_formed_project_initialization_receipt_input_json() }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_project_intent_record_initialization_receipt_rejects_blocked_with_no_issues()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_project_initialization_receipt_input_json();
        input["result"] = json!("Blocked");
        input["issues"] = json!([]);
        let record_cmd = command(
            "project_intent.record_initialization_receipt",
            json!({ "input": input }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "project_intent.get_initialization_receipt",
            json!({ "receipt_digest": "RECEIPT-1" }),
        );
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

    #[test]
    fn handle_command_project_intent_record_initialization_receipt_is_invalid_params_without_input()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("project_intent.record_initialization_receipt", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn failure_occurrence_json(fingerprint: &str, introduces_new_fact: bool) -> Value {
        json!({
            "fingerprint": fingerprint,
            "introduces_new_fact": introduces_new_fact,
        })
    }

    #[test]
    fn handle_command_bounded_failure_evaluate_stall_detects_three_identical_fingerprints_with_no_new_facts()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.evaluate_stall",
            json!({
                "history": [
                    failure_occurrence_json("fp-1", false),
                    failure_occurrence_json("fp-1", false),
                    failure_occurrence_json("fp-1", false),
                ],
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["hold"], json!("Stalled")),
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.evaluate_stall failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_evaluate_stall_is_none_with_fewer_than_three_occurrences() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.evaluate_stall",
            json!({
                "history": [
                    failure_occurrence_json("fp-1", false),
                    failure_occurrence_json("fp-1", false),
                ],
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["hold"], json!(null)),
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.evaluate_stall failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_evaluate_stall_is_invalid_params_without_history() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("bounded_failure.evaluate_stall", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn bounded_backoff_policy_json(base_delay_ms: u64, max_delay_ms: u64, max_retries: u32) -> Value {
        json!({
            "base_delay_ms": base_delay_ms,
            "max_delay_ms": max_delay_ms,
            "max_retries": max_retries,
        })
    }

    #[test]
    fn handle_command_bounded_failure_may_auto_retry_allows_transient_errors_within_the_bound() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.may_auto_retry",
            json!({
                "category": "TransientHarnessError",
                "attempt_index": 0,
                "policy": bounded_backoff_policy_json(100, 1000, 3),
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["may_retry"], true),
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.may_auto_retry failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_may_auto_retry_never_retries_business_failures() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.may_auto_retry",
            json!({
                "category": "BusinessFailure",
                "attempt_index": 0,
                "policy": bounded_backoff_policy_json(100, 1000, 10),
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => assert_eq!(payload["may_retry"], false),
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.may_auto_retry failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_may_auto_retry_is_invalid_params_without_category() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.may_auto_retry",
            json!({
                "attempt_index": 0,
                "policy": bounded_backoff_policy_json(100, 1000, 3),
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_backoff_delay_ms_grows_exponentially_but_is_capped() {
        let (mut store, root) = temp_store_with_isolated_root();

        let policy = bounded_backoff_policy_json(100, 450, 10);
        let expected = [(0u32, 100u64), (1, 200), (2, 400), (3, 450)];
        for (attempt_index, delay_ms) in expected {
            let cmd = command(
                "bounded_failure.backoff_delay_ms",
                json!({ "policy": policy.clone(), "attempt_index": attempt_index }),
            );
            let outcome = handle_command(&mut store, &cmd);
            match outcome.reply.outcome {
                ReplyOutcome::Ok { payload, .. } => {
                    assert_eq!(payload["delay_ms"], json!(delay_ms))
                }
                ReplyOutcome::Error { code, message } => {
                    panic!("bounded_failure.backoff_delay_ms failed: {code:?} {message}")
                }
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_backoff_delay_ms_is_invalid_params_without_policy() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.backoff_delay_ms",
            json!({ "attempt_index": 0 }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn frozen_policy_snapshot_input_json(dollar_budget: Option<Value>) -> Value {
        json!({
            "max_attempts_per_node": 5,
            "max_replan_count": 2,
            "max_wall_clock_seconds": 3600,
            "max_turns": 100,
            "max_tokens": 1_000_000,
            "dollar_budget": dollar_budget,
        })
    }

    #[test]
    fn handle_command_bounded_failure_check_budget_is_not_exhausted_within_all_limits() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.check_budget",
            json!({
                "snapshot": frozen_policy_snapshot_input_json(Some(json!({
                    "limit_cents": 1000,
                    "provider_usage_is_streamable_or_server_enforced": true,
                }))),
                "usage": {
                    "wall_clock_seconds": 10,
                    "turns": 1,
                    "tokens": 10,
                    "dollars_spent_cents": 1,
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["hard_exhausted"], json!([]));
                assert_eq!(payload["soft_alerts"], json!([]));
                assert_eq!(payload["hold"], json!(null));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.check_budget failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_check_budget_wall_clock_turn_and_token_caps_are_always_hard() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.check_budget",
            json!({
                "snapshot": frozen_policy_snapshot_input_json(None),
                "usage": {
                    "wall_clock_seconds": 3600,
                    "turns": 100,
                    "tokens": 1_000_000,
                    "dollars_spent_cents": null,
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(
                    payload["hard_exhausted"],
                    json!(["WallClockSeconds", "Turns", "Tokens"])
                );
                assert_eq!(payload["hold"], json!("BudgetExhausted"));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.check_budget failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_check_budget_a_hard_dollar_budget_exhausts_but_a_soft_one_only_alerts()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let usage = json!({
            "wall_clock_seconds": 0,
            "turns": 0,
            "tokens": 0,
            "dollars_spent_cents": 500,
        });

        let hard_cmd = command(
            "bounded_failure.check_budget",
            json!({
                "snapshot": frozen_policy_snapshot_input_json(Some(json!({
                    "limit_cents": 500,
                    "provider_usage_is_streamable_or_server_enforced": true,
                }))),
                "usage": usage,
            }),
        );
        let hard_outcome = handle_command(&mut store, &hard_cmd);
        match hard_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["hard_exhausted"], json!(["Dollars"]));
                assert_eq!(payload["hold"], json!("BudgetExhausted"));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.check_budget failed: {code:?} {message}")
            }
        }

        let soft_cmd = command(
            "bounded_failure.check_budget",
            json!({
                "snapshot": frozen_policy_snapshot_input_json(Some(json!({
                    "limit_cents": 500,
                    "provider_usage_is_streamable_or_server_enforced": false,
                }))),
                "usage": usage,
            }),
        );
        let soft_outcome = handle_command(&mut store, &soft_cmd);
        match soft_outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["hard_exhausted"], json!([]));
                assert_eq!(payload["soft_alerts"], json!(["Dollars"]));
                assert_eq!(payload["hold"], json!(null));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("bounded_failure.check_budget failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_check_budget_is_invalid_params_without_snapshot() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.check_budget",
            json!({
                "usage": {
                    "wall_clock_seconds": 0,
                    "turns": 0,
                    "tokens": 0,
                    "dollars_spent_cents": null,
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_has_exceeded_node_attempt_limit_checks_the_boundary() {
        let (mut store, root) = temp_store_with_isolated_root();

        for (attempts_for_node, expected) in [(4u32, false), (5, true)] {
            let cmd = command(
                "bounded_failure.has_exceeded_node_attempt_limit",
                json!({
                    "snapshot": frozen_policy_snapshot_input_json(None),
                    "attempts_for_node": attempts_for_node,
                }),
            );
            let outcome = handle_command(&mut store, &cmd);
            match outcome.reply.outcome {
                ReplyOutcome::Ok { payload, .. } => {
                    assert_eq!(payload["exceeded"], json!(expected))
                }
                ReplyOutcome::Error { code, message } => panic!(
                    "bounded_failure.has_exceeded_node_attempt_limit failed: {code:?} {message}"
                ),
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_has_exceeded_node_attempt_limit_is_invalid_params_without_attempts_for_node()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.has_exceeded_node_attempt_limit",
            json!({ "snapshot": frozen_policy_snapshot_input_json(None) }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_has_exceeded_replan_limit_checks_the_boundary() {
        let (mut store, root) = temp_store_with_isolated_root();

        for (replan_count, expected) in [(1u32, false), (2, true)] {
            let cmd = command(
                "bounded_failure.has_exceeded_replan_limit",
                json!({
                    "snapshot": frozen_policy_snapshot_input_json(None),
                    "replan_count": replan_count,
                }),
            );
            let outcome = handle_command(&mut store, &cmd);
            match outcome.reply.outcome {
                ReplyOutcome::Ok { payload, .. } => {
                    assert_eq!(payload["exceeded"], json!(expected))
                }
                ReplyOutcome::Error { code, message } => {
                    panic!("bounded_failure.has_exceeded_replan_limit failed: {code:?} {message}")
                }
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_bounded_failure_has_exceeded_replan_limit_is_invalid_params_without_replan_count()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "bounded_failure.has_exceeded_replan_limit",
            json!({ "snapshot": frozen_policy_snapshot_input_json(None) }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn model_selection_identity_json(model_choice_key_hash: Option<&str>) -> Value {
        json!({
            "adapter_id": "claude-code",
            "installation_id": "install-1",
            "provider": "anthropic",
            "cli_hash": "cli-hash",
            "protocol_hash": "proto-hash",
            "schema_hash": "schema-hash",
            "model_id": "claude-sonnet-5",
            "resolved_wire_name": "claude-sonnet-5-20260101",
            "service_tier": null,
            "provider_native_effort": "medium",
            "auth_mode": "oauth",
            "account_fingerprint": "acct-1",
            "exposed_snapshot_or_fingerprint": null,
            "qualification_batch_id": "batch-1",
            "model_choice_key_hash": model_choice_key_hash,
            "runtime_selection_hash": "runtime-1",
        })
    }

    fn qualification_receipt_input_json(
        receipt_digest: &str,
        issued_at: &str,
        valid_until: &str,
        provider_snapshot_is_immutable: bool,
    ) -> Value {
        json!({
            "identity": model_selection_identity_json(Some("hash-1")),
            "harness_capability_snapshot_digest": "harness-digest",
            "account_capability_snapshot_digest": "account-digest",
            "canary_manifest_hash": "canary-1",
            "run_ids": ["run-1"],
            "issued_at": issued_at,
            "valid_until": valid_until,
            "provider_snapshot_is_immutable": provider_snapshot_is_immutable,
            "result": "Qualified",
            "receipt_digest": receipt_digest,
        })
    }

    /// §5.10: `model_selection.issue_qualification_receipt` follows the same
    /// no-`Event`-produced shape as `readiness.record`, and its payload
    /// round-trips through the `model_selection.get_qualification_receipt`
    /// read command.
    #[test]
    fn handle_command_model_selection_issue_qualification_receipt_produces_no_event_and_reads_back(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "model_selection.issue_qualification_receipt",
            json!({
                "input": qualification_receipt_input_json(
                    "QR-1",
                    "2026-09-15T00:00:00Z",
                    "2026-09-16T00:00:00Z",
                    false,
                ),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.issue_qualification_receipt failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["receipt"]["receipt_digest"], "QR-1");

        let get_cmd = command(
            "model_selection.get_qualification_receipt",
            json!({ "receipt_digest": "QR-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.get_qualification_receipt failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_get_qualification_receipt_is_not_found_before_issuance() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command(
            "model_selection.get_qualification_receipt",
            json!({ "receipt_digest": "no-such-receipt" }),
        );
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// The domain constructor rejects `valid_until` before `issued_at`; the
    /// rejected receipt must never land in the store, so a subsequent `get`
    /// still reports `NotFound`.
    #[test]
    fn handle_command_model_selection_issue_qualification_receipt_rejects_valid_until_before_issued_at(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "model_selection.issue_qualification_receipt",
            json!({
                "input": qualification_receipt_input_json(
                    "QR-1",
                    "2026-09-15T00:00:00Z",
                    "2026-09-14T00:00:00Z",
                    false,
                ),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "model_selection.get_qualification_receipt",
            json!({ "receipt_digest": "QR-1" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// Without an immutable provider snapshot, the domain constructor caps
    /// the validity window at seven days.
    #[test]
    fn handle_command_model_selection_issue_qualification_receipt_rejects_validity_window_over_seven_days_without_immutable_snapshot(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "model_selection.issue_qualification_receipt",
            json!({
                "input": qualification_receipt_input_json(
                    "QR-1",
                    "2026-09-15T00:00:00Z",
                    "2026-09-30T00:00:00Z",
                    false,
                ),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// The same window is accepted when the provider snapshot is immutable.
    #[test]
    fn handle_command_model_selection_issue_qualification_receipt_allows_long_window_with_immutable_snapshot(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "model_selection.issue_qualification_receipt",
            json!({
                "input": qualification_receipt_input_json(
                    "QR-1",
                    "2026-09-15T00:00:00Z",
                    "2026-09-30T00:00:00Z",
                    true,
                ),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { .. } => {}
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.issue_qualification_receipt failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_issue_qualification_receipt_is_invalid_params_without_input()
    {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("model_selection.issue_qualification_receipt", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_sealed_pass_window_accepts_runs_within_twenty_four_hours(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "model_selection.validate_sealed_pass_window",
            json!({
                "first_run_at": "2026-09-15T00:00:00Z",
                "second_run_at": "2026-09-15T23:00:00Z",
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.validate_sealed_pass_window failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["ok"], true);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_sealed_pass_window_rejects_runs_more_than_twenty_four_hours_apart_in_either_order(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        for (first, second) in [
            ("2026-09-15T00:00:00Z", "2026-09-16T01:00:00Z"),
            ("2026-09-16T01:00:00Z", "2026-09-15T00:00:00Z"),
        ] {
            let cmd = command(
                "model_selection.validate_sealed_pass_window",
                json!({ "first_run_at": first, "second_run_at": second }),
            );
            let outcome = handle_command(&mut store, &cmd);
            let payload = match outcome.reply.outcome {
                ReplyOutcome::Ok { payload, .. } => payload,
                ReplyOutcome::Error { code, message } => panic!(
                    "model_selection.validate_sealed_pass_window failed: {code:?} {message}"
                ),
            };
            assert_eq!(payload["ok"], false);
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_sealed_pass_window_is_invalid_params_without_first_run_at(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "model_selection.validate_sealed_pass_window",
            json!({ "second_run_at": "2026-09-15T00:00:00Z" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_model_separation_passes_when_required_pairs_have_distinct_hashes(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "model_selection.validate_model_separation",
            json!({
                "model_choice_key_hash_by_step": {
                    "contract_drafting": "a",
                    "contract_review": "b",
                    "task_graph_planning": "a",
                    "graph_review": "b",
                    "implementation": "a",
                    "node_evaluation": "b",
                    "final_audit": "c",
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.validate_model_separation failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["violations"], json!([]));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_model_separation_flags_identical_hashes_on_a_required_pair(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "model_selection.validate_model_separation",
            json!({
                "model_choice_key_hash_by_step": {
                    "contract_drafting": "same",
                    "contract_review": "same",
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.validate_model_separation failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["ok"], false);
        assert_eq!(
            payload["violations"],
            json!([{ "step_a": "contract_drafting", "step_b": "contract_review" }])
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_model_separation_treats_unresolved_alias_as_a_violation(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "model_selection.validate_model_separation",
            json!({
                "model_choice_key_hash_by_step": {
                    "contract_drafting": null,
                    "contract_review": "b",
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.validate_model_separation failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["ok"], false);
        assert_eq!(payload["violations"].as_array().unwrap().len(), 1);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_model_separation_ignores_pairs_where_a_step_is_not_yet_routed(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "model_selection.validate_model_separation",
            json!({
                "model_choice_key_hash_by_step": {
                    "contract_drafting": "a",
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("model_selection.validate_model_separation failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["ok"], true);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_model_selection_validate_model_separation_is_invalid_params_without_map() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("model_selection.validate_model_separation", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn skill_install_input_json(package_digest: &str, receipt_digest: &str) -> Value {
        json!({
            "package_digest": package_digest,
            "audit_outcome": { "NoKnownRisksFound": null },
            "plan_digest": "plan-1",
            "user_approval_decision_ref": "decision:1",
            "receipt_digest": receipt_digest,
        })
    }

    fn issue_skill_install(store: &mut EventStore, package_digest: &str, receipt_digest: &str) {
        let cmd = command(
            "skill.issue_install_receipt",
            json!({ "input": skill_install_input_json(package_digest, receipt_digest) }),
        );
        let outcome = handle_command(store, &cmd);
        if let ReplyOutcome::Error { code, message } = outcome.reply.outcome {
            panic!("skill.issue_install_receipt failed: {code:?} {message}")
        }
    }

    /// §5.11: `skill.issue_install_receipt` follows the same
    /// no-`Event`-produced shape as `readiness.record`/
    /// `model_selection.issue_qualification_receipt`, and seeds a fresh
    /// `Installed`-only ladder in the same write -- both round-trip through
    /// their `skill.get_*` counterparts.
    #[test]
    fn handle_command_skill_issue_install_receipt_seeds_receipt_and_ladder_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        issue_skill_install(&mut store, "pkg-1", "SR-1");

        let get_receipt = command(
            "skill.get_install_receipt",
            json!({ "receipt_digest": "SR-1" }),
        );
        let outcome = handle_command(&mut store, &get_receipt);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["receipt"]["package_digest"], "pkg-1");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.get_install_receipt failed: {code:?} {message}")
            }
        }

        let get_ladder = command(
            "skill.get_evidence_ladder",
            json!({ "skill_digest": "pkg-1" }),
        );
        let outcome = handle_command(&mut store, &get_ladder);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ladder"]["installed"], true);
                assert_eq!(payload["ladder"]["bound"], false);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.get_evidence_ladder failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_issue_install_receipt_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("skill.issue_install_receipt", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `issue_skill_install_receipt`'s `MissingUserApprovalDecision` check
    /// is re-run server-side, same as `bounded_failure`/`model_selection`'s
    /// own domain re-validation.
    #[test]
    fn handle_command_skill_issue_install_receipt_rejects_a_blank_user_approval_decision() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = skill_install_input_json("pkg-1", "SR-1");
        input["user_approval_decision_ref"] = json!("  ");
        let cmd = command("skill.issue_install_receipt", json!({ "input": input }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => {
                assert_eq!(code, ReplyErrorCode::TransitionRejected)
            }
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_get_install_receipt_and_get_evidence_ladder_are_not_found_before_issuance()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_receipt = command(
            "skill.get_install_receipt",
            json!({ "receipt_digest": "SR-1" }),
        );
        let outcome = handle_command(&mut store, &get_receipt);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        let get_ladder = command(
            "skill.get_evidence_ladder",
            json!({ "skill_digest": "pkg-1" }),
        );
        let outcome = handle_command(&mut store, &get_ladder);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `skill.mark_bound`/`.mark_discoverable`/`.mark_available_to_attempt`/
    /// `.mark_invoked`/`.record_effective` walk the ladder forward one rung
    /// at a time, each persisted and readable back.
    #[test]
    fn handle_command_skill_ladder_transitions_walk_forward_and_persist_each_rung() {
        let (mut store, root) = temp_store_with_isolated_root();
        issue_skill_install(&mut store, "pkg-1", "SR-1");

        for method in [
            "skill.mark_bound",
            "skill.mark_discoverable",
            "skill.mark_available_to_attempt",
            "skill.mark_invoked",
        ] {
            let cmd = command(method, json!({ "skill_digest": "pkg-1" }));
            let outcome = handle_command(&mut store, &cmd);
            if let ReplyOutcome::Error { code, message } = outcome.reply.outcome {
                panic!("{method} failed: {code:?} {message}")
            }
        }
        let cmd = command(
            "skill.record_effective",
            json!({ "skill_digest": "pkg-1", "effective": true }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ladder"]["invoked"], true);
                assert_eq!(payload["ladder"]["effective"], true);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.record_effective failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_mark_discoverable_rejects_skipping_mark_bound() {
        let (mut store, root) = temp_store_with_isolated_root();
        issue_skill_install(&mut store, "pkg-1", "SR-1");

        let cmd = command("skill.mark_discoverable", json!({ "skill_digest": "pkg-1" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => {
                assert_eq!(code, ReplyErrorCode::TransitionRejected)
            }
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_mark_bound_is_not_found_without_a_prior_install() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "skill.mark_bound",
            json!({ "skill_digest": "never-installed" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_mark_bound_is_invalid_params_without_skill_digest() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("skill.mark_bound", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn global_skill_binding_json(skill_digest: &str, state: &str) -> Value {
        json!({
            "revision": 1,
            "skill_digest": skill_digest,
            "steps": ["implementation"],
            "cli_targets": ["claude-code"],
            "invocation": "ExplicitOnly",
            "state": state,
        })
    }

    /// `skill.record_global_binding` upserts the current-state row -- no
    /// server-side re-validation since `GlobalSkillBinding` has no
    /// validating constructor to re-run, same reasoning as
    /// `readiness.record`.
    #[test]
    fn handle_command_skill_record_global_binding_upserts_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "skill.record_global_binding",
            json!({ "binding": global_skill_binding_json("pkg-1", "Enabled") }),
        );
        let outcome = handle_command(&mut store, &cmd);
        if let ReplyOutcome::Error { code, message } = outcome.reply.outcome {
            panic!("skill.record_global_binding failed: {code:?} {message}")
        }

        let get = command(
            "skill.get_global_binding",
            json!({ "skill_digest": "pkg-1" }),
        );
        let outcome = handle_command(&mut store, &get);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["binding"]["state"], "Enabled");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.get_global_binding failed: {code:?} {message}")
            }
        }

        let cmd = command(
            "skill.record_global_binding",
            json!({ "binding": global_skill_binding_json("pkg-1", "Disabled") }),
        );
        handle_command(&mut store, &cmd);
        let outcome = handle_command(&mut store, &get);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["binding"]["state"], "Disabled");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.get_global_binding failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_get_global_binding_is_not_found_before_recording() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "skill.get_global_binding",
            json!({ "skill_digest": "pkg-1" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_record_global_binding_is_invalid_params_without_binding() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("skill.record_global_binding", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `skill.record_project_binding` is keyed by `(project_id,
    /// skill_digest)` -- distinct projects binding the same skill don't
    /// collide.
    #[test]
    fn handle_command_skill_record_project_binding_is_keyed_by_project_and_skill_digest() {
        let (mut store, root) = temp_store_with_isolated_root();

        let binding = json!({
            "project_id": "proj-1",
            "revision": 1,
            "mode": "Disable",
            "steps": null,
            "cli_targets": null,
            "invocation": null,
            "state": null,
        });
        let cmd = command(
            "skill.record_project_binding",
            json!({ "project_id": "proj-1", "skill_digest": "pkg-1", "binding": binding }),
        );
        let outcome = handle_command(&mut store, &cmd);
        if let ReplyOutcome::Error { code, message } = outcome.reply.outcome {
            panic!("skill.record_project_binding failed: {code:?} {message}")
        }

        let get = command(
            "skill.get_project_binding",
            json!({ "project_id": "proj-1", "skill_digest": "pkg-1" }),
        );
        let outcome = handle_command(&mut store, &get);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["binding"]["mode"], "Disable");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.get_project_binding failed: {code:?} {message}")
            }
        }

        let get_other_project = command(
            "skill.get_project_binding",
            json!({ "project_id": "proj-2", "skill_digest": "pkg-1" }),
        );
        let outcome = handle_command(&mut store, &get_other_project);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_record_project_binding_is_invalid_params_without_project_id() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "skill.record_project_binding",
            json!({
                "skill_digest": "pkg-1",
                "binding": {
                    "project_id": "proj-1",
                    "revision": 1,
                    "mode": "Inherit",
                    "steps": null,
                    "cli_targets": null,
                    "invocation": null,
                    "state": null,
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.11: `skill.resolve_binding` re-exercises `resolve_project_binding`
    /// server-side. No `project_id` is pure inheritance; a `Disable`-mode
    /// project binding forces the resolved state regardless of the global
    /// binding's own state.
    #[test]
    fn handle_command_skill_resolve_binding_inherits_without_a_project_id_and_overrides_with_one() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "skill.record_global_binding",
            json!({ "binding": global_skill_binding_json("pkg-1", "Enabled") }),
        );
        handle_command(&mut store, &cmd);

        let resolve_without_project = command(
            "skill.resolve_binding",
            json!({ "skill_digest": "pkg-1" }),
        );
        let outcome = handle_command(&mut store, &resolve_without_project);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["resolved"]["state"], "Enabled");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.resolve_binding failed: {code:?} {message}")
            }
        }

        let binding = json!({
            "project_id": "proj-1",
            "revision": 1,
            "mode": "Disable",
            "steps": null,
            "cli_targets": null,
            "invocation": null,
            "state": null,
        });
        let cmd = command(
            "skill.record_project_binding",
            json!({ "project_id": "proj-1", "skill_digest": "pkg-1", "binding": binding }),
        );
        handle_command(&mut store, &cmd);

        let resolve_with_project = command(
            "skill.resolve_binding",
            json!({ "skill_digest": "pkg-1", "project_id": "proj-1" }),
        );
        let outcome = handle_command(&mut store, &resolve_with_project);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["resolved"]["state"], "Disabled");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.resolve_binding failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_resolve_binding_is_not_found_without_a_global_binding() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("skill.resolve_binding", json!({ "skill_digest": "pkg-1" }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_can_garbage_collect_refuses_a_currently_bound_or_referenced_digest_and_allows_otherwise()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let bound_cmd = command(
            "skill.can_garbage_collect",
            json!({
                "digest": "pkg-1",
                "currently_bound_digests": ["pkg-1"],
                "historically_referenced_digests": [],
            }),
        );
        let outcome = handle_command(&mut store, &bound_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["can_garbage_collect"], false);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.can_garbage_collect failed: {code:?} {message}")
            }
        }

        let referenced_cmd = command(
            "skill.can_garbage_collect",
            json!({
                "digest": "pkg-1",
                "currently_bound_digests": [],
                "historically_referenced_digests": ["pkg-1"],
            }),
        );
        let outcome = handle_command(&mut store, &referenced_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["can_garbage_collect"], false);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.can_garbage_collect failed: {code:?} {message}")
            }
        }

        let free_cmd = command(
            "skill.can_garbage_collect",
            json!({
                "digest": "pkg-1",
                "currently_bound_digests": [],
                "historically_referenced_digests": [],
            }),
        );
        let outcome = handle_command(&mut store, &free_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["can_garbage_collect"], true);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("skill.can_garbage_collect failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_skill_can_garbage_collect_is_invalid_params_without_digest() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "skill.can_garbage_collect",
            json!({
                "currently_bound_digests": [],
                "historically_referenced_digests": [],
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn human_review_finding_json(
        id: &str,
        status: &str,
        with_anchor: bool,
        expected_change: &str,
    ) -> Value {
        json!({
            "id": id,
            "review_receipt_id": "receipt-1",
            "subject_hash": "output-1",
            "step_id": "contract_review",
            "anchor": if with_anchor {
                json!({ "path": "contract.md" })
            } else {
                json!({})
            },
            "expected_change": expected_change,
            "severity": "major",
            "status": status,
            "successor_attempt_id": null,
            "resolution_subject_hash": null,
            "finding_digest": format!("{id}-digest"),
        })
    }

    fn human_review_receipt_input_json(
        receipt_digest: &str,
        decision: &str,
        findings: Value,
    ) -> Value {
        json!({
            "project_hash": "project-1",
            "task_hash": "task-1",
            "run_hash": "run-1",
            "spec_subject": { "Planning": { "planning_spec_hash": "plan-1" } },
            "step_id": "contract_review",
            "operator": "operator-1",
            "decided_at": "2026-09-15T00:00:00Z",
            "decision": decision,
            "review_output_hash": "review-output-1",
            "reason": "looks good",
            "findings": findings,
            "subject": { "DocumentStep": { "input_snapshot_hash": "input-1", "output_hash": "output-1" } },
            "receipt_digest": receipt_digest,
        })
    }

    /// §5.1: `review.issue_receipt` follows the same no-`Event`-produced
    /// shape as `model_selection.issue_qualification_receipt`, and its
    /// payload round-trips through `review.get_receipt`.
    #[test]
    fn handle_command_review_issue_receipt_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "review.issue_receipt",
            json!({
                "input": human_review_receipt_input_json("HRR-1", "Pass", json!([])),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("review.issue_receipt failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["receipt"]["receipt_digest"], "HRR-1");

        let get_cmd = command("review.get_receipt", json!({ "receipt_digest": "HRR-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("review.get_receipt failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_get_receipt_is_not_found_before_issuance() {
        let (mut store, root) = temp_store_with_isolated_root();

        let get_cmd = command(
            "review.get_receipt",
            json!({ "receipt_digest": "no-such-receipt" }),
        );
        let outcome = handle_command(&mut store, &get_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// The domain constructor refuses a `Reject` decision with no findings
    /// at all; the rejected receipt must never land in the store.
    #[test]
    fn handle_command_review_issue_receipt_rejects_reject_without_findings() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "review.issue_receipt",
            json!({
                "input": human_review_receipt_input_json("HRR-1", "Reject", json!([])),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command("review.get_receipt", json!({ "receipt_digest": "HRR-1" }));
        let get_outcome = handle_command(&mut store, &get_cmd);
        match get_outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `Reject` decision with a finding that has no anchor is also
    /// refused.
    #[test]
    fn handle_command_review_issue_receipt_rejects_reject_with_unanchored_finding() {
        let (mut store, root) = temp_store_with_isolated_root();

        let findings = json!([human_review_finding_json(
            "finding-1",
            "Open",
            false,
            "narrow the write scope",
        )]);
        let record_cmd = command(
            "review.issue_receipt",
            json!({
                "input": human_review_receipt_input_json("HRR-1", "Reject", findings),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `Reject` decision with a finding that has an anchor but an empty
    /// `expected_change` is also refused.
    #[test]
    fn handle_command_review_issue_receipt_rejects_reject_with_empty_expected_change() {
        let (mut store, root) = temp_store_with_isolated_root();

        let findings = json!([human_review_finding_json("finding-1", "Open", true, "")]);
        let record_cmd = command(
            "review.issue_receipt",
            json!({
                "input": human_review_receipt_input_json("HRR-1", "Reject", findings),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `Reject` decision with a well-formed, anchored finding is accepted.
    #[test]
    fn handle_command_review_issue_receipt_accepts_reject_with_well_formed_finding() {
        let (mut store, root) = temp_store_with_isolated_root();

        let findings = json!([human_review_finding_json(
            "finding-1",
            "Open",
            true,
            "narrow the write scope",
        )]);
        let record_cmd = command(
            "review.issue_receipt",
            json!({
                "input": human_review_receipt_input_json("HRR-1", "Reject", findings),
            }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { .. } => {}
            ReplyOutcome::Error { code, message } => {
                panic!("review.issue_receipt failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_issue_receipt_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("review.issue_receipt", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_can_resubmit_is_true_when_subject_changed() {
        let (mut store, root) = temp_store_with_isolated_root();

        let findings = json!([human_review_finding_json(
            "finding-1",
            "Open",
            true,
            "narrow the write scope",
        )]);
        let cmd = command(
            "review.can_resubmit",
            json!({ "subject_changed": true, "findings": findings }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("review.can_resubmit failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["can_resubmit"], true);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_can_resubmit_is_false_when_open_findings_remain_and_subject_unchanged(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let findings = json!([human_review_finding_json(
            "finding-1",
            "Open",
            true,
            "narrow the write scope",
        )]);
        let cmd = command(
            "review.can_resubmit",
            json!({ "subject_changed": false, "findings": findings }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("review.can_resubmit failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["can_resubmit"], false);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_can_resubmit_is_invalid_params_without_subject_changed() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("review.can_resubmit", json!({ "findings": [] }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_validate_carried_planning_bundle_ok_when_receipts_cover_all_steps() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "review.validate_carried_planning_bundle",
            json!({
                "required_step_ids": ["contract_drafting", "contract_review"],
                "fresh_receipt_ids_by_step": {
                    "contract_drafting": "HRR-1",
                    "contract_review": "HRR-2",
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => panic!(
                "review.validate_carried_planning_bundle failed: {code:?} {message}"
            ),
        };
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["violations"], json!([]));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_validate_carried_planning_bundle_reports_missing_steps() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "review.validate_carried_planning_bundle",
            json!({
                "required_step_ids": ["contract_drafting", "contract_review"],
                "fresh_receipt_ids_by_step": {
                    "contract_drafting": "HRR-1",
                },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => panic!(
                "review.validate_carried_planning_bundle failed: {code:?} {message}"
            ),
        };
        assert_eq!(payload["ok"], false);
        assert_eq!(
            payload["violations"],
            json!([{ "step": "contract_review" }])
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_review_validate_carried_planning_bundle_is_invalid_params_without_required_step_ids(
    ) {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "review.validate_carried_planning_bundle",
            json!({ "fresh_receipt_ids_by_step": {} }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn config_profile_json(model_id: &str) -> Value {
        json!({
            "adapter_id": "codex",
            "installation_id": "install-1",
            "model_id": model_id,
            "effort_id": "medium",
            "skill_policy_ref": "skill-policy-1",
        })
    }

    /// A fully-populated `GlobalConfigRevision` JSON -- every
    /// `CONFIGURABLE_AI_STEPS` entry gets a `step_defaults` and
    /// `human_review_default` row, since `resolve_project_config` refuses
    /// to resolve at all when any configurable step lacks a global default.
    fn global_config_revision_json(revision: u32, model_id: &str) -> Value {
        use autome_domain::config::CONFIGURABLE_AI_STEPS;

        let mut step_defaults = serde_json::Map::new();
        let mut human_review_default = serde_json::Map::new();
        for step in CONFIGURABLE_AI_STEPS {
            step_defaults.insert((*step).to_string(), config_profile_json(model_id));
            human_review_default.insert((*step).to_string(), json!("Off"));
        }
        json!({
            "revision": revision,
            "step_defaults": step_defaults,
            "human_review_default": human_review_default,
            "environment_defaults_ref": "env-default",
            "budget_defaults_ref": "budget-default",
            "skill_policy_default_ref": "skill-policy-default",
            "safety_policy_hash": "safety-floor-1",
            "content_hash": format!("global-content-{revision}"),
        })
    }

    fn global_config_impact_preview_json(
        expires_at: &str,
        requires_second_confirmation: bool,
    ) -> Value {
        json!({
            "base_global_revision": 1,
            "proposed_config_hash": "proposed-1",
            "observed_project_set_hash": "project-set-1",
            "affected_projects": [],
            "blocking_project_ids": [],
            "requires_second_confirmation": requires_second_confirmation,
            "expires_at": expires_at,
            "preview_hash": "preview-1",
        })
    }

    fn save_global_config_revision_input_json(
        revision: u32,
        model_id: &str,
        expires_at: &str,
        requires_second_confirmation: bool,
        second_confirmation_acquired: bool,
    ) -> Value {
        json!({
            "revision": global_config_revision_json(revision, model_id),
            "preview": global_config_impact_preview_json(expires_at, requires_second_confirmation),
            "submitted_preview_hash": "preview-1",
            "current_project_set_hash": "project-set-1",
            "second_confirmation_acquired": second_confirmation_acquired,
        })
    }

    fn issue_config_save_global_revision(
        store: &mut EventStore,
        revision: u32,
        model_id: &str,
    ) {
        let cmd = command(
            "config.save_global_revision",
            json!({
                "input": save_global_config_revision_input_json(
                    revision,
                    model_id,
                    "2099-01-01T00:00:00Z",
                    false,
                    false,
                ),
            }),
        );
        let outcome = handle_command(store, &cmd);
        if let ReplyOutcome::Error { code, message } = outcome.reply.outcome {
            panic!("config.save_global_revision failed: {code:?} {message}")
        }
    }

    /// A well-formed `config.save_global_revision` call lands a
    /// `global_config_revisions` row, readable back both by exact
    /// `revision` and as the "current" one.
    #[test]
    fn handle_command_config_save_global_revision_persists_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        issue_config_save_global_revision(&mut store, 1, "global-model");

        let get = command("config.get_global_revision", json!({ "revision": 1 }));
        let outcome = handle_command(&mut store, &get);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["revision"]["revision"], 1);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("config.get_global_revision failed: {code:?} {message}")
            }
        }

        let get_current = command("config.get_current_global_revision", json!({}));
        let outcome = handle_command(&mut store, &get_current);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["revision"]["revision"], 1);
            }
            ReplyOutcome::Error { code, message } => {
                panic!("config.get_current_global_revision failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_config_save_global_revision_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("config.save_global_revision", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// `config::validate_save_global_config_revision` is re-run
    /// server-side -- a preview-hash mismatch is `TransitionRejected`, not
    /// silently accepted.
    #[test]
    fn handle_command_config_save_global_revision_rejects_a_preview_hash_mismatch() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input =
            save_global_config_revision_input_json(1, "global-model", "2099-01-01T00:00:00Z", false, false);
        input["submitted_preview_hash"] = json!("wrong-hash");
        let cmd = command("config.save_global_revision", json!({ "input": input }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => {
                assert_eq!(code, ReplyErrorCode::TransitionRejected)
            }
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A preview that demands a second confirmation is refused without one,
    /// then accepted once `second_confirmation_acquired` is true.
    #[test]
    fn handle_command_config_save_global_revision_requires_second_confirmation_then_succeeds() {
        let (mut store, root) = temp_store_with_isolated_root();

        let input = save_global_config_revision_input_json(
            1,
            "global-model",
            "2099-01-01T00:00:00Z",
            true,
            false,
        );
        let cmd = command("config.save_global_revision", json!({ "input": input }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => {
                assert_eq!(code, ReplyErrorCode::TransitionRejected)
            }
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let input = save_global_config_revision_input_json(
            1,
            "global-model",
            "2099-01-01T00:00:00Z",
            true,
            true,
        );
        let cmd = command("config.save_global_revision", json!({ "input": input }));
        let outcome = handle_command(&mut store, &cmd);
        if let ReplyOutcome::Error { code, message } = outcome.reply.outcome {
            panic!("config.save_global_revision failed: {code:?} {message}")
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_config_get_global_revision_is_not_found_before_saving() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("config.get_global_revision", json!({ "revision": 1 }));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_config_get_current_global_revision_is_not_found_before_saving() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("config.get_current_global_revision", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    fn project_config_patch_json(project_id: &str, model_id_override: Option<&str>) -> Value {
        let step_overrides = match model_id_override {
            Some(model_id) => json!({
                "contract_review": { "Replace": config_profile_json(model_id) },
            }),
            None => json!({}),
        };
        json!({
            "project_id": project_id,
            "revision": 1,
            "step_overrides": step_overrides,
            "human_review_overrides": {},
            "environment_override_ref": null,
            "budget_override_ref": null,
            "skill_policy_override_ref": null,
            "content_hash": "patch-content-1",
        })
    }

    /// `config.save_project_patch` upserts the current-state row for a
    /// `project_id` -- no server-side re-validation since
    /// `ProjectConfigPatch` has no validating constructor to re-run, same
    /// reasoning as `skill.record_project_binding`.
    #[test]
    fn handle_command_config_save_project_patch_upserts_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "config.save_project_patch",
            json!({ "patch": project_config_patch_json("proj-1", None) }),
        );
        let outcome = handle_command(&mut store, &cmd);
        if let ReplyOutcome::Error { code, message } = outcome.reply.outcome {
            panic!("config.save_project_patch failed: {code:?} {message}")
        }

        let get = command(
            "config.get_project_patch",
            json!({ "project_id": "proj-1" }),
        );
        let outcome = handle_command(&mut store, &get);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["patch"]["project_id"], "proj-1");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("config.get_project_patch failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_config_save_project_patch_is_invalid_params_without_patch() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("config.save_project_patch", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_config_get_project_patch_is_not_found_before_saving() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "config.get_project_patch",
            json!({ "project_id": "proj-1" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.1: `config.resolve_project_config` re-exercises
    /// `resolve_project_config` server-side. No `project_id` is pure
    /// inheritance from the global revision; a project patch overriding one
    /// step's profile changes just that step's provenance, leaving the rest
    /// inherited.
    #[test]
    fn handle_command_config_resolve_project_config_inherits_without_a_project_id_and_overrides_with_one()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        issue_config_save_global_revision(&mut store, 1, "global-model");

        let resolve_without_project = command(
            "config.resolve_project_config",
            json!({
                "valid_model_ids": ["global-model", "project-model"],
                "skill_binding_revision_ref": "binding-1",
                "snapshot_hash": "snap-1",
            }),
        );
        let outcome = handle_command(&mut store, &resolve_without_project);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert!(payload["resolved"]["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|s| s["provenance"] == "Global"));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("config.resolve_project_config failed: {code:?} {message}")
            }
        }

        let cmd = command(
            "config.save_project_patch",
            json!({ "patch": project_config_patch_json("proj-1", Some("project-model")) }),
        );
        handle_command(&mut store, &cmd);

        let resolve_with_project = command(
            "config.resolve_project_config",
            json!({
                "project_id": "proj-1",
                "valid_model_ids": ["global-model", "project-model"],
                "skill_binding_revision_ref": "binding-1",
                "snapshot_hash": "snap-1",
            }),
        );
        let outcome = handle_command(&mut store, &resolve_with_project);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                let steps = payload["resolved"]["steps"].as_array().unwrap();
                let contract_review = steps
                    .iter()
                    .find(|s| s["step_id"] == "contract_review")
                    .unwrap();
                assert_eq!(contract_review["profile"]["model_id"], "project-model");
                assert_eq!(contract_review["provenance"], "Project");
                let other = steps
                    .iter()
                    .find(|s| s["step_id"] == "final_audit")
                    .unwrap();
                assert_eq!(other["profile"]["model_id"], "global-model");
                assert_eq!(other["provenance"], "Global");
            }
            ReplyOutcome::Error { code, message } => {
                panic!("config.resolve_project_config failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_config_resolve_project_config_is_not_found_without_a_global_revision() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "config.resolve_project_config",
            json!({
                "valid_model_ids": ["global-model"],
                "skill_binding_revision_ref": "binding-1",
                "snapshot_hash": "snap-1",
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::NotFound),
            other => panic!("expected NotFound, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_config_resolve_project_config_is_invalid_params_without_valid_model_ids() {
        let (mut store, root) = temp_store_with_isolated_root();

        issue_config_save_global_revision(&mut store, 1, "global-model");

        let cmd = command(
            "config.resolve_project_config",
            json!({
                "skill_binding_revision_ref": "binding-1",
                "snapshot_hash": "snap-1",
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// A project override that fails `is_profile_valid` is refused outright
    /// -- not silently clamped back to the global default.
    #[test]
    fn handle_command_config_resolve_project_config_rejects_an_invalid_override_without_falling_back()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        issue_config_save_global_revision(&mut store, 1, "global-model");
        let cmd = command(
            "config.save_project_patch",
            json!({ "patch": project_config_patch_json("proj-1", Some("unqualified-model")) }),
        );
        handle_command(&mut store, &cmd);

        let cmd = command(
            "config.resolve_project_config",
            json!({
                "project_id": "proj-1",
                "valid_model_ids": ["global-model"],
                "skill_binding_revision_ref": "binding-1",
                "snapshot_hash": "snap-1",
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => {
                assert_eq!(code, ReplyErrorCode::TransitionRejected)
            }
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }


    fn replan_task_graph_json(graph_hash: &str, nodes: Vec<Value>) -> Value {
        json!({
            "id": "G-1",
            "version": 1,
            "graph_hash": graph_hash,
            "contract_ref": "C-1",
            "nodes": nodes,
        })
    }

    fn replan_proposal_json(old_graph_hash: &str, new_graph: Value) -> Value {
        json!({
            "run_id": "run-1",
            "trigger_evidence_refs": ["evidence-1"],
            "affected_requirement_ids": [],
            "affected_node_ids": [],
            "semantically_unchanged_node_ids": [],
            "invalidated_attempt_ids": ["attempt-1"],
            "invalidated_candidate_ids": [],
            "invalidated_receipt_ids": [],
            "old_graph_hash": old_graph_hash,
            "new_graph": new_graph,
            "budget_delta_ref": "budget-delta-1",
        })
    }

    fn well_formed_replan_authorization_input_json() -> Value {
        let old_graph = replan_task_graph_json(
            "hash-old",
            vec![graph_node_json("A", "Business", &["R1"])],
        );
        let new_graph = replan_task_graph_json(
            "hash-new",
            vec![graph_node_json("A", "Business", &["R1"])],
        );
        json!({
            "old_graph": old_graph,
            "proposal": replan_proposal_json("hash-old", new_graph),
            "must_requirements": [],
            "mandatory_checks_by_requirement": {},
            "graph_review_receipt_ref": "graph-review-1",
            "user_approval_ref": "user-approval-1",
        })
    }

    /// §6.6: `replan.authorize` follows the same no-`Event`-produced shape as
    /// `policy_restart.issue_planning_restart`, and its payload round-trips
    /// through `replan.get_authorization`.
    #[test]
    fn handle_command_replan_authorize_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "replan.authorize",
            json!({ "input": well_formed_replan_authorization_input_json() }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("replan.authorize failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["authorization"]["new_graph_hash"], "hash-new");

        let get_cmd = command(
            "replan.get_authorization",
            json!({ "run_id": "run-1", "new_graph_hash": "hash-new" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("replan.get_authorization failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_authorize_refuses_to_reuse_an_existing_run_id_and_new_graph_hash_pair()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "replan.authorize",
            json!({ "input": well_formed_replan_authorization_input_json() }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_authorize_rejects_a_stale_old_graph_hash() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_replan_authorization_input_json();
        input["proposal"]["old_graph_hash"] = json!("hash-stale");
        let record_cmd = command("replan.authorize", json!({ "input": input }));
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "replan.get_authorization",
            json!({ "run_id": "run-1", "new_graph_hash": "hash-new" }),
        );
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

    #[test]
    fn handle_command_replan_authorize_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("replan.authorize", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_get_authorization_is_not_found_before_issuance() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "replan.get_authorization",
            json!({ "run_id": "run-1", "new_graph_hash": "hash-new" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        assert!(matches!(
            outcome.reply.outcome,
            ReplyOutcome::Error {
                code: ReplyErrorCode::NotFound,
                ..
            }
        ));

        std::fs::remove_dir_all(&root).ok();
    }

    fn well_formed_contract_amendment_authorization_input_json() -> Value {
        json!({
            "proposal": {
                "triggering_run_id": "run-1",
                "preallocated_new_run_id": "run-2",
                "base_contract_version": 1,
                "new_contract_ref": "contract-2",
                "new_graph_ref": "graph-2",
                "new_execution_run_spec_ref": "spec-2",
            },
            "required_review_kinds": ["Planning", "Contract", "Graph"],
            "provided_review_receipts": {
                "Planning": "planning-receipt",
                "Contract": "contract-receipt",
                "Graph": "graph-receipt",
            },
            "user_approval_ref": "user-approval-1",
        })
    }

    /// §6.6: `replan.authorize_contract_amendment` follows the same
    /// no-`Event`-produced shape as `replan.authorize`, and its payload
    /// round-trips through `replan.get_contract_amendment_authorization`.
    #[test]
    fn handle_command_replan_authorize_contract_amendment_produces_no_event_and_reads_back() {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "replan.authorize_contract_amendment",
            json!({ "input": well_formed_contract_amendment_authorization_input_json() }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        assert!(outcome.event.is_none());
        let payload = match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("replan.authorize_contract_amendment failed: {code:?} {message}")
            }
        };
        assert_eq!(payload["authorization"]["new_run_id"], "run-2");

        let get_cmd = command(
            "replan.get_contract_amendment_authorization",
            json!({ "new_run_id": "run-2" }),
        );
        let get_outcome = handle_command(&mut store, &get_cmd);
        assert!(get_outcome.event.is_none());
        match get_outcome.reply.outcome {
            ReplyOutcome::Ok {
                payload: read_payload,
                ..
            } => assert_eq!(read_payload, payload),
            ReplyOutcome::Error { code, message } => {
                panic!("replan.get_contract_amendment_authorization failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_authorize_contract_amendment_refuses_to_reuse_an_existing_new_run_id()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let record_cmd = command(
            "replan.authorize_contract_amendment",
            json!({ "input": well_formed_contract_amendment_authorization_input_json() }),
        );
        handle_command(&mut store, &record_cmd);

        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::Internal),
            other => panic!("expected Internal, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_authorize_contract_amendment_rejects_a_missing_required_review_receipt()
     {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut input = well_formed_contract_amendment_authorization_input_json();
        input["provided_review_receipts"] = json!({
            "Planning": "planning-receipt",
            "Contract": "contract-receipt",
        });
        let record_cmd = command(
            "replan.authorize_contract_amendment",
            json!({ "input": input }),
        );
        let outcome = handle_command(&mut store, &record_cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::TransitionRejected),
            other => panic!("expected TransitionRejected, got {other:?}"),
        }

        let get_cmd = command(
            "replan.get_contract_amendment_authorization",
            json!({ "new_run_id": "run-2" }),
        );
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

    #[test]
    fn handle_command_replan_authorize_contract_amendment_is_invalid_params_without_input() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command("replan.authorize_contract_amendment", json!({}));
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_get_contract_amendment_authorization_is_not_found_before_issuance() {
        let (mut store, root) = temp_store_with_isolated_root();

        let cmd = command(
            "replan.get_contract_amendment_authorization",
            json!({ "new_run_id": "run-2" }),
        );
        let outcome = handle_command(&mut store, &cmd);
        assert!(matches!(
            outcome.reply.outcome,
            ReplyOutcome::Error {
                code: ReplyErrorCode::NotFound,
                ..
            }
        ));

        std::fs::remove_dir_all(&root).ok();
    }

    /// §6.6: `replan.evaluate_proposal` is stateless -- no store row is
    /// involved at all, same shape as `historical_red_light.evaluate`/
    /// `step_role.validate_schema`.
    #[test]
    fn handle_command_replan_evaluate_proposal_accepts_a_well_formed_proposal() {
        let (mut store, root) = temp_store_with_isolated_root();

        let mut old_node = graph_node_json("A", "Business", &["R1"]);
        old_node["acceptance_check_ids"] = json!(["chk-1"]);
        let mut new_node = graph_node_json("A", "Business", &["R1"]);
        new_node["acceptance_check_ids"] = json!(["chk-1"]);
        let old_graph = replan_task_graph_json("hash-old", vec![old_node]);
        let new_graph = replan_task_graph_json("hash-new", vec![new_node]);
        let cmd = command(
            "replan.evaluate_proposal",
            json!({
                "old_graph": old_graph,
                "proposal": replan_proposal_json("hash-old", new_graph),
                "must_requirement_ids": ["R1"],
                "mandatory_checks_by_requirement": { "R1": ["chk-1"] },
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ok"], true);
                assert_eq!(payload["rejections"], json!([]));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("replan.evaluate_proposal failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_evaluate_proposal_rejects_a_stale_old_graph_hash() {
        let (mut store, root) = temp_store_with_isolated_root();

        let old_graph = replan_task_graph_json("hash-old", vec![]);
        let new_graph = replan_task_graph_json("hash-new", vec![]);
        let cmd = command(
            "replan.evaluate_proposal",
            json!({
                "old_graph": old_graph,
                "proposal": replan_proposal_json("hash-stale", new_graph),
                "must_requirement_ids": [],
                "mandatory_checks_by_requirement": {},
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ok"], false);
                assert_eq!(
                    payload["rejections"],
                    json!(["ProposalTargetsWrongOldGraph"])
                );
            }
            ReplyOutcome::Error { code, message } => {
                panic!("replan.evaluate_proposal failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_evaluate_proposal_rejects_dropped_requirement_coverage() {
        let (mut store, root) = temp_store_with_isolated_root();

        let old_graph = replan_task_graph_json(
            "hash-old",
            vec![graph_node_json("A", "Business", &["R1", "R2"])],
        );
        let new_graph = replan_task_graph_json(
            "hash-new",
            vec![graph_node_json("A", "Business", &["R1"])],
        );
        let cmd = command(
            "replan.evaluate_proposal",
            json!({
                "old_graph": old_graph,
                "proposal": replan_proposal_json("hash-old", new_graph),
                "must_requirement_ids": [],
                "mandatory_checks_by_requirement": {},
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ok"], false);
                assert_eq!(
                    payload["rejections"],
                    json!([{ "RequirementCoverageDecreased": { "requirement": "R2" } }])
                );
            }
            ReplyOutcome::Error { code, message } => {
                panic!("replan.evaluate_proposal failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_evaluate_proposal_rejects_a_cyclic_new_graph() {
        let (mut store, root) = temp_store_with_isolated_root();

        let old_graph = replan_task_graph_json("hash-old", vec![]);
        let mut a = graph_node_json("A", "Business", &[]);
        a["depends_on"] = json!(["B"]);
        let mut b = graph_node_json("B", "Business", &[]);
        b["depends_on"] = json!(["A"]);
        let new_graph = replan_task_graph_json("hash-new", vec![a, b]);
        let cmd = command(
            "replan.evaluate_proposal",
            json!({
                "old_graph": old_graph,
                "proposal": replan_proposal_json("hash-old", new_graph),
                "must_requirement_ids": [],
                "mandatory_checks_by_requirement": {},
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => {
                assert_eq!(payload["ok"], false);
                let rejections = payload["rejections"].as_array().unwrap();
                assert!(rejections.iter().any(|r| r["GraphFreezeViolation"]["Cycle"].is_object()));
            }
            ReplyOutcome::Error { code, message } => {
                panic!("replan.evaluate_proposal failed: {code:?} {message}")
            }
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_evaluate_proposal_is_invalid_params_without_old_graph() {
        let (mut store, root) = temp_store_with_isolated_root();

        let new_graph = replan_task_graph_json("hash-new", vec![]);
        let cmd = command(
            "replan.evaluate_proposal",
            json!({
                "proposal": replan_proposal_json("hash-old", new_graph),
                "must_requirement_ids": [],
                "mandatory_checks_by_requirement": {},
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn handle_command_replan_evaluate_proposal_is_invalid_params_without_proposal() {
        let (mut store, root) = temp_store_with_isolated_root();

        let old_graph = replan_task_graph_json("hash-old", vec![]);
        let cmd = command(
            "replan.evaluate_proposal",
            json!({
                "old_graph": old_graph,
                "must_requirement_ids": [],
                "mandatory_checks_by_requirement": {},
            }),
        );
        let outcome = handle_command(&mut store, &cmd);
        match outcome.reply.outcome {
            ReplyOutcome::Error { code, .. } => assert_eq!(code, ReplyErrorCode::InvalidParams),
            other => panic!("expected InvalidParams, got {other:?}"),
        }

        std::fs::remove_dir_all(&root).ok();
    }
}
