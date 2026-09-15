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

use std::collections::HashSet;

use autome_domain::attempt::{
    self, Attempt, AttemptPermissionProfile, AttemptShapeError, PermissionProfileViolation,
    PlanningWriteViolation,
};
use autome_domain::certificate::{
    self, AuditVerdict, CandidateCertificate, CandidateCertificateError, CompletionCertificate,
    CompletionCertificateError,
};
use autome_domain::config::{
    self, GlobalConfigImpactPreview, GlobalConfigRevision, ProjectConfigPatch, SaveGlobalConfigError,
};
use autome_domain::contract::{self, ContractEvent, ContractEventError, TaskContract};
use autome_domain::credential::{
    self, CredentialEvent, CredentialReceipt, CredentialReceiptError, CredentialRecord,
    CredentialShapeError,
};
use autome_domain::delivery::{
    DeliveryApprovalReceipt, DeliveryChain, DeliveryChainError, DeliveredTreeCheckReceipt,
    DeliveryReceipt, DeliveryRehearsalReceipt, DeliverySubject, ProjectTargetTransitionReceipt,
};
use autome_domain::evidence::{EvidenceReceipt, ReceiptId};
use autome_domain::execution_queue::{
    self, ExecutionQueue, ExecutionQueueError, ExecutionQueueEvent,
};
use autome_domain::graph::{self, GraphEvent, GraphEventError, TaskGraph};
use autome_domain::model_selection::QualificationReceipt;
use autome_domain::node::{self, NodeEvent, NodeStatus, NodeTransitionError};
use autome_domain::playbook::FrozenPlaybook;
use autome_domain::policy_restart::{
    self, BudgetGrantError, BudgetGrantReceipt, BudgetLimitGrant, PlanningPolicyRestart,
    PlanningPolicyRestartError, RunPolicyAmendment, RunPolicyAmendmentError,
};
use autome_domain::project::{
    self, ProjectEvent, ProjectIdentity, ProjectKind, ProjectState, TargetInspection,
    TargetRejection,
};
use autome_domain::project_intent::{
    self, InitializationResult, IntentRevision, KeyDecision, ProjectInitializationError,
    ProjectInitializationReceipt, ProjectIntentAmendment, ProjectIntentAmendmentError,
    ProjectIntentAmendmentRequest, ProjectIntentError, ProjectIntentRevision,
};
use autome_domain::readiness::{ReadinessFingerprint, ReadinessReceipt};
use autome_domain::requirement::RequirementId;
use autome_domain::review::{self, HumanReviewFinding, HumanReviewReceipt, HumanReviewReceiptError, ReviewDecision, ReviewSpecSubject, ReviewSubject};
use autome_domain::run::{self, RunEvent, RunState, TransitionError};
use autome_domain::skill::{
    self, GlobalSkillBinding, ProjectSkillBinding, SkillAuditOutcome, SkillEvidenceError,
    SkillEvidenceLadder, SkillInstallError, SkillInstallReceipt,
};
use autome_domain::task::{self, Task, TaskEvent, TaskEventError};
use autome_domain::user_correction::{
    self, CorrectionClassification, CorrectionDisposition, CorrectionImpactFlags,
    UserCorrectionError, UserCorrectionReceipt,
};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::fs_guard::{self, FsGuardError};
use crate::target_probe::TargetIdentityProbe;
use crate::workspace::{self, WorkspaceError};

/// `ExecutionQueue` is a single fleet-wide singleton (plan §6.2), not one
/// instance per caller-chosen id like the five aggregates above it in this
/// file — so it is always journaled under this one fixed aggregate id.
/// `pub` so `dispatch.rs` can use the same constant for the `Event`
/// envelope's `aggregate_id` rather than duplicating the string literal.
pub const EXECUTION_QUEUE_AGGREGATE_ID: &str = "execution-queue";

pub struct EventStore {
    conn: Connection,
    /// The path `EventStore::open` was called with — kept only so
    /// `project_home_for` can derive a `projects/<project-id>/` sibling
    /// directory next to the database file. Not otherwise used; the
    /// connection itself is the source of truth for everything else.
    db_path: std::path::PathBuf,
    /// Set by `verify_disk_layout` at the end of `open()` when the data
    /// root or any already-registered ProjectHome fails its §8.3
    /// re-check (wrong owner, symlink, group/world-writable, escaped
    /// root). `Some(reason)` puts the store into the read-only
    /// diagnostic state plan §8.3:1362 requires: every write command is
    /// refused (see `diagnostic_reason` and `dispatch::dispatch`'s check
    /// at the top of its match) while reads continue unaffected.
    diagnostic: Option<String>,
}

/// A single row of `project_projections`, for `project.list` — see
/// `EventStore::list_project_summaries`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub id: String,
    pub revision: u64,
    pub lifecycle: String,
    pub phase: String,
    pub hold: String,
    /// `NULL` for rows written before migrate_v3, or (should never happen
    /// post this increment) any row not created through `create_project`.
    pub display_name: Option<String>,
    /// Derived from `identity_json`, not a dedicated column — `kind` is
    /// display-only metadata for `project.list`, not something any query
    /// filters or indexes by, so a second denormalized column would only
    /// be duplication. `None` under the same conditions as `display_name`.
    pub kind: Option<ProjectKind>,
}

/// One task's resolved project label, for `queue.get`'s `entry_labels`
/// (plan §9: every execution-bar entry must show a Project name, not a
/// bare id). `project_id`/`project_display_name` are both `None` when the
/// task has no known project or that project has no registered identity —
/// never a fabricated label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskProjectLabel {
    pub task_id: String,
    pub project_id: Option<String>,
    pub project_display_name: Option<String>,
}

/// A single row of `task_projections`, for `task.list` — see
/// `EventStore::list_task_summaries`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSummary {
    pub id: String,
    pub revision: u64,
    pub lifecycle: String,
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
    /// `create_project` was called with an `aggregate_id` that already has
    /// a `project_projections` row.
    AlreadyExists,
    /// A write method was called against an `aggregate_id` with no
    /// `project.create` ever journaled for it. §5.1: Projects only come
    /// into existence via explicit creation — this closes the implicit
    /// `ProjectState::new()` fabrication `append_project_event` used to
    /// do for any unknown id.
    NotFound,
}

impl From<rusqlite::Error> for ProjectAppendError {
    fn from(value: rusqlite::Error) -> Self {
        ProjectAppendError::Sql(value)
    }
}

/// A single row of `project_targets` — §8.2's two-phase target
/// registration. `probe`/`inspection` are the exact values `register_target`
/// persisted (the probe never changes; `inspection.destination_absent` is
/// stale by the time this is read back and must never be trusted directly —
/// `create_project_from_target` recomputes it fresh).
#[derive(Debug, Clone, PartialEq)]
pub struct TargetRecord {
    pub target_id: String,
    pub kind: ProjectKind,
    pub registered_at: String,
    pub canonical_path: String,
    pub probe: TargetIdentityProbe,
    pub inspection: TargetInspection,
    pub trust_confirmed: bool,
    pub consumed_by_project_id: Option<String>,
}

#[derive(Debug)]
pub enum TargetConsumeError {
    Sql(rusqlite::Error),
    /// No `project_targets` row exists for the given `target_id`.
    NotFound,
    /// The target was already consumed by an earlier call — the same
    /// target cannot create two projects.
    AlreadyConsumed,
}

impl From<rusqlite::Error> for TargetConsumeError {
    fn from(value: rusqlite::Error) -> Self {
        TargetConsumeError::Sql(value)
    }
}

#[derive(Debug)]
pub enum CreateFromTargetError {
    Sql(rusqlite::Error),
    /// No `project_targets` row exists for the given `target_id`.
    TargetNotFound,
    /// The target was already consumed by an earlier
    /// `create_project_from_target` call.
    TargetAlreadyConsumed,
    /// `kind == ExistingRepository` but the caller did not pass
    /// `trust_confirmed: true`. §8.3:1354's trust gate — checked before
    /// any disk or SQL write happens.
    TrustNotConfirmed,
    /// `autome_domain::project::locator_for` rejected the target (not a
    /// git repo, unborn/unresolvable HEAD, dirty worktree, destination
    /// already exists, or an invalid destination name).
    Rejected(TargetRejection),
    /// `ProjectIdentity::new`'s own validation rejected the derived
    /// triple. Should not happen given `locator_for` already agreed with
    /// `kind` — kept for exhaustiveness rather than an `expect()` panic
    /// on a path that ultimately runs against untrusted filesystem input.
    Identity(project::ProjectIdentityError),
    /// ProjectHome creation, re-verification, or manifest write failed
    /// per §8.3's owner-only discipline.
    FsGuard(FsGuardError),
}

impl From<rusqlite::Error> for CreateFromTargetError {
    fn from(value: rusqlite::Error) -> Self {
        CreateFromTargetError::Sql(value)
    }
}

impl From<FsGuardError> for CreateFromTargetError {
    fn from(value: FsGuardError) -> Self {
        CreateFromTargetError::FsGuard(value)
    }
}

/// `create_project_from_target`'s result: the generated `project_id` (Core
/// owns generation here — unlike the legacy `project.create`, the
/// two-phase flow gives the caller nothing to pass in) alongside the
/// terminal `AppendedProjectEvent` (`IntentUnresolved`, revision 6).
#[derive(Debug, Clone)]
pub struct CreatedProjectFromTarget {
    pub project_id: String,
    pub appended: AppendedProjectEvent,
}

/// One `runs/<task_id>/<run_id>/repo` disposable clone (plan §8.1), as
/// persisted by `create_disposable_clone_for_run`. `repo_path`/`head_commit`
/// mirror `workspace::DisposableClone` field-for-field; this is this row's
/// SQL-backed twin, used once the clone has actually landed on disk and
/// been recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisposableCloneRecord {
    pub task_id: String,
    pub run_id: String,
    pub repo_path: String,
    pub head_commit: String,
    pub created_at: String,
}

#[derive(Debug)]
pub enum CreateDisposableCloneError {
    Sql(rusqlite::Error),
    Workspace(WorkspaceError),
}

impl From<rusqlite::Error> for CreateDisposableCloneError {
    fn from(value: rusqlite::Error) -> Self {
        CreateDisposableCloneError::Sql(value)
    }
}

impl From<WorkspaceError> for CreateDisposableCloneError {
    fn from(value: WorkspaceError) -> Self {
        CreateDisposableCloneError::Workspace(value)
    }
}

/// One `attempts` row (plan §5.6), as persisted by `record_attempt`: the
/// `Attempt` this step was, the `AttemptPermissionProfile` it was bound to
/// before it ran, and which `run_id` it belongs to. `run_id` is kept as an
/// explicit column rather than a field on `Attempt` itself, mirroring how
/// `DisposableCloneRecord` above adds `task_id`/`run_id` at the store layer
/// instead of the pure `autome_domain::attempt::Attempt` type carrying
/// aggregate-linking keys it has no business knowing about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptRecord {
    pub run_id: String,
    pub attempt: Attempt,
    pub permission_profile: AttemptPermissionProfile,
    pub created_at: String,
}

/// `record_attempt`'s failure modes. The three domain variants
/// (`Shape`/`PermissionViolations`/`PlanningWrite`) all come from
/// `autome_domain::attempt` validation that already exists and is already
/// tested there -- this type exists only so `record_attempt` can refuse to
/// write anything until all three pass, not to reimplement the checks.
#[derive(Debug)]
pub enum RecordAttemptError {
    Sql(rusqlite::Error),
    Shape(AttemptShapeError),
    PermissionViolations(Vec<PermissionProfileViolation>),
    PlanningWrite(PlanningWriteViolation),
}

impl From<rusqlite::Error> for RecordAttemptError {
    fn from(value: rusqlite::Error) -> Self {
        RecordAttemptError::Sql(value)
    }
}

/// A persisted §5.7 `EvidenceReceipt` plus when it landed. Unlike
/// `AttemptRecord`, no aggregate-linking key is added at the store layer --
/// `EvidenceReceipt` already carries its own `run_id`/`check_id`, so there is
/// nothing this layer needs to know that the domain type doesn't already say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    pub receipt: EvidenceReceipt,
    pub created_at: String,
}

/// `record_evidence`'s only failure mode. Unlike `record_attempt`, there is
/// no domain-level shape check to re-run here: §5.7 staleness
/// (`EvidenceReceipt::is_valid_against`) is a property checked against the
/// *current* fingerprint at query time, not at record time, so a receipt is
/// always well-formed to store as-is -- a duplicate `receipt_id` is the only
/// way this can fail, and that already surfaces as a primary-key violation.
#[derive(Debug)]
pub enum RecordEvidenceError {
    Sql(rusqlite::Error),
}

impl From<rusqlite::Error> for RecordEvidenceError {
    fn from(value: rusqlite::Error) -> Self {
        RecordEvidenceError::Sql(value)
    }
}

/// A persisted §5.9 `ReadinessReceipt` plus when it landed. Same shape as
/// `EvidenceRecord`: the domain type already carries every field a caller
/// needs (`profile_hash`, `subject`, `receipt_digest`), so the store layer
/// adds nothing beyond `created_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessRecord {
    pub receipt: ReadinessReceipt,
    pub created_at: String,
}

/// `record_readiness`'s only failure mode. Same reasoning as
/// `RecordEvidenceError`: staleness (`ReadinessReceipt::is_current_against`)
/// and gate-passing (`ReadinessReceipt::is_ready`) are both query-time
/// properties, not something this method decides, so a duplicate
/// `receipt_digest` -- surfaced as a primary-key violation -- is the only
/// way recording can fail.
#[derive(Debug)]
pub enum RecordReadinessError {
    Sql(rusqlite::Error),
}

impl From<rusqlite::Error> for RecordReadinessError {
    fn from(value: rusqlite::Error) -> Self {
        RecordReadinessError::Sql(value)
    }
}

/// A persisted §5.10 `QualificationReceipt` plus when it landed. Same shape
/// as `ReadinessRecord` -- the domain type already carries its own natural
/// key (`receipt_digest`), so the store layer adds nothing beyond
/// `created_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationRecord {
    pub receipt: QualificationReceipt,
    pub created_at: String,
}

/// `record_qualification_receipt`'s only failure mode. Same reasoning as
/// `RecordReadinessError`: `issue_qualification_receipt` already validated
/// the receipt's validity window at construction time, so a duplicate
/// `receipt_digest` -- surfaced as a primary-key violation -- is the only
/// way recording can fail.
#[derive(Debug)]
pub enum RecordQualificationReceiptError {
    Sql(rusqlite::Error),
}

impl From<rusqlite::Error> for RecordQualificationReceiptError {
    fn from(value: rusqlite::Error) -> Self {
        RecordQualificationReceiptError::Sql(value)
    }
}

/// A persisted §5.11 `SkillInstallReceipt` plus when it landed. Same shape
/// as `ReadinessRecord` -- the domain type already carries its own natural
/// key (`receipt_digest`), so the store layer adds nothing beyond
/// `created_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstallReceiptRecord {
    pub receipt: SkillInstallReceipt,
    pub created_at: String,
}

/// `issue_skill_install_receipt` returns a `SkillInstallReceipt` *and* the
/// freshly-`Installed` `SkillEvidenceLadder` it seeds in the same call --
/// see `record_skill_install_receipt`'s doc comment for why those two rows
/// are always written together. This is what that write path hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInstallRecord {
    pub receipt: SkillInstallReceipt,
    pub ladder: SkillEvidenceLadder,
    pub created_at: String,
}

/// `record_skill_install_receipt`'s failure modes. Re-runs
/// `skill::issue_skill_install_receipt` server-side rather than trusting an
/// already-built `SkillInstallReceipt` from the caller, same discipline as
/// `record_credential_receipt`/`record_user_correction`: `Install` is
/// `issue_skill_install_receipt`'s own `MissingUserApprovalDecision` check,
/// `Sql` is a duplicate `receipt_digest`.
#[derive(Debug)]
pub enum RecordSkillInstallReceiptError {
    Sql(rusqlite::Error),
    Install(SkillInstallError),
}

impl From<rusqlite::Error> for RecordSkillInstallReceiptError {
    fn from(value: rusqlite::Error) -> Self {
        RecordSkillInstallReceiptError::Sql(value)
    }
}

/// A persisted §5.11 `SkillEvidenceLadder` plus when it was last touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEvidenceLadderRecord {
    pub ladder: SkillEvidenceLadder,
    pub created_at: String,
}

/// Failure modes shared by every `mark_skill_*`/`record_skill_effective`
/// transition: the SQL layer, no ladder ever recorded for this
/// `skill_digest` (`NotFound` -- the store-level equivalent of
/// `RecordProjectIntentAmendmentError::NoCurrentRevision`, since a ladder
/// only ever comes into existence via `record_skill_install_receipt`), or
/// the domain ladder's own preceding-level check rejecting the transition
/// (`Ladder`).
#[derive(Debug)]
pub enum SkillLadderTransitionError {
    Sql(rusqlite::Error),
    NotFound,
    Ladder(SkillEvidenceError),
}

impl From<rusqlite::Error> for SkillLadderTransitionError {
    fn from(value: rusqlite::Error) -> Self {
        SkillLadderTransitionError::Sql(value)
    }
}

/// A persisted §5.1 `HumanReviewReceipt` plus when it landed. Same shape as
/// `QualificationRecord` -- the domain type already carries its own natural
/// key (`receipt_digest`), so the store layer adds nothing beyond
/// `created_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanReviewReceiptRecord {
    pub receipt: HumanReviewReceipt,
    pub created_at: String,
}

/// `record_human_review_receipt`'s failure modes. Re-runs
/// `review::issue_human_review_receipt` server-side rather than trusting an
/// already-built `HumanReviewReceipt` from the caller, same discipline as
/// `record_skill_install_receipt`: `Receipt` is
/// `issue_human_review_receipt`'s own reject-without-anchored-findings
/// checks, `Sql` is a duplicate `receipt_digest`.
#[derive(Debug)]
pub enum RecordHumanReviewReceiptError {
    Sql(rusqlite::Error),
    Receipt(Vec<HumanReviewReceiptError>),
}

impl From<rusqlite::Error> for RecordHumanReviewReceiptError {
    fn from(value: rusqlite::Error) -> Self {
        RecordHumanReviewReceiptError::Sql(value)
    }
}

/// A persisted §5.12 `DeliveryChain`. Unlike `AttemptRecord`/`EvidenceRecord`/
/// `ReadinessRecord` above, `DeliveryChain` itself has no
/// `Serialize`/`Deserialize` -- its five rungs are only reachable through the
/// fixed-order `append_*` methods, by design (see `delivery.rs`'s module
/// doc: which actor may call which append method is an authorization
/// concern the data-only domain crate deliberately can't enforce, so nothing
/// here should make it easy to reconstruct a chain except by replaying those
/// same methods). So the store keeps each rung's receipt in its own
/// nullable column and this record is the plain data shape read back;
/// `rebuild()` is what turns it back into a live `DeliveryChain` whenever an
/// append or a read needs to ask the domain object a question (is the next
/// rung reachable, is the chain ready for completion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryChainRecord {
    pub run_id: String,
    pub subject: DeliverySubject,
    pub rehearsal: Option<DeliveryRehearsalReceipt>,
    pub approval: Option<DeliveryApprovalReceipt>,
    pub delivery: Option<DeliveryReceipt>,
    pub tree_check: Option<DeliveredTreeCheckReceipt>,
    pub project_target_transition: Option<ProjectTargetTransitionReceipt>,
    pub created_at: String,
    pub updated_at: String,
}

impl DeliveryChainRecord {
    /// Replays whichever rungs are already recorded, in the chain's own
    /// fixed order, through the real `append_*` methods -- never constructs
    /// a `DeliveryChain` by any other means. Each `.expect()` below can only
    /// fire if a row this method reads back was never actually valid
    /// through the append path in the first place, which would be a store
    /// bug (writing a rung that didn't pass its own domain check), not a
    /// reachable runtime condition.
    pub fn rebuild(&self) -> DeliveryChain {
        let mut chain = DeliveryChain::new(self.subject.clone());
        if let Some(r) = &self.rehearsal {
            chain.append_rehearsal(r.clone());
        }
        if let Some(a) = &self.approval {
            chain
                .append_approval(a.clone())
                .expect("a previously-recorded approval replays cleanly");
        }
        if let Some(d) = &self.delivery {
            chain
                .append_delivery(d.clone())
                .expect("a previously-recorded delivery replays cleanly");
        }
        if let Some(t) = &self.tree_check {
            chain
                .append_tree_check(t.clone())
                .expect("a previously-recorded tree check replays cleanly");
        }
        if let Some(p) = &self.project_target_transition {
            chain
                .append_project_target_transition(p.clone())
                .expect("a previously-recorded project target transition replays cleanly");
        }
        chain
    }
}

/// `start_delivery_chain`'s only failure mode: a duplicate `run_id`,
/// surfaced as a SQL primary-key violation -- one chain per run, matching
/// `DeliveryChain::new` fixing its subject exactly once.
#[derive(Debug)]
pub enum StartDeliveryChainError {
    Sql(rusqlite::Error),
}

impl From<rusqlite::Error> for StartDeliveryChainError {
    fn from(value: rusqlite::Error) -> Self {
        StartDeliveryChainError::Sql(value)
    }
}

/// Every `delivery.append_*` write's failure modes: either the `run_id`
/// never had `start_delivery_chain` called for it (`NotFound`), or the
/// domain's own `DeliveryChain::append_*` rejected the rung out of order
/// (`Chain`) -- re-using `autome_domain::delivery`'s own validation rather
/// than reimplementing "rehearsal required before approval" etc. here.
#[derive(Debug)]
pub enum AppendDeliveryReceiptError {
    Sql(rusqlite::Error),
    NotFound,
    Chain(DeliveryChainError),
}

impl From<rusqlite::Error> for AppendDeliveryReceiptError {
    fn from(value: rusqlite::Error) -> Self {
        AppendDeliveryReceiptError::Sql(value)
    }
}

/// A persisted §5.8 `CandidateCertificate`. Like `ReadinessRecord`, the
/// domain type already carries its own `run_id`, so this record adds only
/// `created_at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateCertificateRecord {
    pub certificate: CandidateCertificate,
    pub created_at: String,
}

/// `issue_candidate_certificate`'s failure modes: the referenced readiness
/// receipt was never recorded (`ReadinessNotFound`), or the domain's own
/// `issue_candidate_certificate` rejected the certificate -- re-using
/// `autome_domain::certificate`'s own validation (missing verdicts, stale
/// readiness, etc.) rather than reimplementing it here.
#[derive(Debug)]
pub enum IssueCandidateCertificateError {
    Sql(rusqlite::Error),
    ReadinessNotFound,
    Domain(Vec<CandidateCertificateError>),
}

impl From<rusqlite::Error> for IssueCandidateCertificateError {
    fn from(value: rusqlite::Error) -> Self {
        IssueCandidateCertificateError::Sql(value)
    }
}

/// A persisted §5.8 `CompletionCertificate`. Unlike `CandidateCertificate`,
/// `CompletionCertificate` carries no `run_id` field of its own -- it's
/// composed from a candidate certificate and a delivery chain, neither of
/// which it references by id once built -- so this record wraps `run_id`
/// alongside the certificate, the same convention `AttemptRecord` uses for
/// a domain type that doesn't carry its own key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCertificateRecord {
    pub run_id: String,
    pub certificate: CompletionCertificate,
    pub created_at: String,
}

/// `issue_completion_certificate`'s failure modes: no candidate certificate
/// was ever issued for this `run_id` (`CandidateNotFound`), no delivery
/// chain was ever started for it (`DeliveryChainNotFound`), or the domain's
/// own `issue_completion_certificate` rejected it (chain not ready, missing
/// approval decision).
#[derive(Debug)]
pub enum IssueCompletionCertificateError {
    Sql(rusqlite::Error),
    CandidateNotFound,
    DeliveryChainNotFound,
    Domain(CompletionCertificateError),
}

impl From<rusqlite::Error> for IssueCompletionCertificateError {
    fn from(value: rusqlite::Error) -> Self {
        IssueCompletionCertificateError::Sql(value)
    }
}

/// A persisted §10.3 `FrozenPlaybook` plus which run it's bound to and when.
/// `FrozenPlaybook` has no `run_id` field of its own, mirroring
/// `AttemptRecord`'s reasoning: the pure domain type has no business knowing
/// about aggregate-linking keys, so the store layer adds `run_id` explicitly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenPlaybookRecord {
    pub run_id: String,
    pub playbook: FrozenPlaybook,
    pub created_at: String,
}

/// `bind_playbook`'s only failure mode: a duplicate `run_id`, surfaced as a
/// SQL primary-key violation -- a Run binds exactly one playbook for its
/// whole lifetime, so there is no domain-level shape check to re-run here
/// (any `PlaybookId`+hash combination is already well-formed).
#[derive(Debug)]
pub enum BindPlaybookError {
    Sql(rusqlite::Error),
}

impl From<rusqlite::Error> for BindPlaybookError {
    fn from(value: rusqlite::Error) -> Self {
        BindPlaybookError::Sql(value)
    }
}

/// A persisted §8.3 `CredentialRecord` plus the caller-chosen `credential_ref`
/// it is stored under. `CredentialRecord` has no id field of its own
/// (mirrors `Attempt`/`FrozenPlaybook`'s reasoning), so `credential_ref` is
/// an explicit column here, not pulled out of `record_json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialRecordRow {
    pub credential_ref: String,
    pub record: CredentialRecord,
    pub created_at: String,
}

/// `record_credential`'s failure modes. Unlike `AttemptRecord`/
/// `FrozenPlaybookRecord`, `record_credential` deliberately *upserts*
/// rather than refusing a duplicate `credential_ref`: `CredentialRecord`
/// is §8.3's record of a credential's *current* state, and rotation/
/// revocation are supposed to update the same record in place, not start
/// a new one -- so there is no `AlreadyRecorded`-style variant here, only
/// `Shape` (re-running `CredentialRecord::validate_shape()` before writing
/// anything, same discipline as `RecordAttemptError::Shape`).
#[derive(Debug)]
pub enum RecordCredentialError {
    Sql(rusqlite::Error),
    Shape(Vec<CredentialShapeError>),
}

impl From<rusqlite::Error> for RecordCredentialError {
    fn from(value: rusqlite::Error) -> Self {
        RecordCredentialError::Sql(value)
    }
}

/// `record_credential_receipt`'s failure modes. Re-runs
/// `credential::issue_credential_receipt` server-side rather than
/// accepting an already-built `CredentialReceipt` from the caller, same
/// "don't trust the caller already validated" discipline as
/// `record_attempt`.
#[derive(Debug)]
pub enum RecordCredentialReceiptError {
    Sql(rusqlite::Error),
    Receipt(CredentialReceiptError),
}

impl From<rusqlite::Error> for RecordCredentialReceiptError {
    fn from(value: rusqlite::Error) -> Self {
        RecordCredentialReceiptError::Sql(value)
    }
}

/// A persisted §6.5 `UserCorrectionReceipt` plus when it landed. Unlike
/// `AttemptRecord`, no aggregate-linking key is added at the store layer --
/// `UserCorrectionReceipt` already carries its own `run_id`, so there is
/// nothing this layer needs to know that the domain type doesn't already say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCorrectionRecord {
    pub receipt: UserCorrectionReceipt,
    pub created_at: String,
}

/// `record_user_correction`'s failure modes. Re-runs
/// `user_correction::issue_user_correction_receipt` server-side rather than
/// trusting an already-built `UserCorrectionReceipt` from the caller, same
/// discipline as `record_attempt`/`record_credential_receipt`. Unlike
/// `RecordAttemptError`, there is no separate `Sql`-vs-domain-check
/// distinction to make beyond `Receipt` -- a duplicate `receipt_digest`
/// surfaces as a plain SQL primary-key violation, same as
/// `RecordReadinessError`.
#[derive(Debug)]
pub enum RecordUserCorrectionError {
    Sql(rusqlite::Error),
    Receipt(Vec<UserCorrectionError>),
}

impl From<rusqlite::Error> for RecordUserCorrectionError {
    fn from(value: rusqlite::Error) -> Self {
        RecordUserCorrectionError::Sql(value)
    }
}

/// A persisted §5.1 `PlanningPolicyRestart` plus when it landed. Same
/// "no extra aggregate-linking key" reasoning as `UserCorrectionRecord` --
/// `PlanningPolicyRestart` already carries its own `task_id`/`current_run_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningPolicyRestartRecord {
    pub restart: PlanningPolicyRestart,
    pub created_at: String,
}

/// `record_planning_policy_restart`'s failure modes. Re-runs
/// `policy_restart::issue_planning_policy_restart` server-side, same
/// discipline as `record_user_correction`.
#[derive(Debug)]
pub enum RecordPlanningPolicyRestartError {
    Sql(rusqlite::Error),
    Restart(Vec<PlanningPolicyRestartError>),
}

impl From<rusqlite::Error> for RecordPlanningPolicyRestartError {
    fn from(value: rusqlite::Error) -> Self {
        RecordPlanningPolicyRestartError::Sql(value)
    }
}

/// A persisted §5.1 `RunPolicyAmendment` plus when it landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunPolicyAmendmentRecord {
    pub amendment: RunPolicyAmendment,
    pub created_at: String,
}

/// `record_run_policy_amendment`'s failure modes. Re-runs
/// `policy_restart::issue_run_policy_amendment` server-side.
#[derive(Debug)]
pub enum RecordRunPolicyAmendmentError {
    Sql(rusqlite::Error),
    Amendment(Vec<RunPolicyAmendmentError>),
}

impl From<rusqlite::Error> for RecordRunPolicyAmendmentError {
    fn from(value: rusqlite::Error) -> Self {
        RecordRunPolicyAmendmentError::Sql(value)
    }
}

/// A persisted §5.1 `BudgetGrantReceipt` plus when it landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetGrantRecord {
    pub receipt: BudgetGrantReceipt,
    pub created_at: String,
}

/// `record_budget_grant`'s failure modes. Re-runs
/// `policy_restart::issue_budget_grant_receipt` server-side.
#[derive(Debug)]
pub enum RecordBudgetGrantError {
    Sql(rusqlite::Error),
    Grant(Vec<BudgetGrantError>),
}

impl From<rusqlite::Error> for RecordBudgetGrantError {
    fn from(value: rusqlite::Error) -> Self {
        RecordBudgetGrantError::Sql(value)
    }
}

/// A persisted §5.1 `ProjectIntentRevision` plus when it landed. Same
/// "no extra aggregate-linking key" reasoning as `UserCorrectionRecord` --
/// `ProjectIntentRevision` already carries its own `project_id`/`revision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIntentRevisionRecord {
    pub revision: ProjectIntentRevision,
    pub created_at: String,
}

/// `record_project_intent_revision`'s failure modes. Re-runs
/// `project_intent::issue_project_intent_revision` server-side, same
/// discipline as `record_user_correction`. A duplicate `(project_id,
/// revision)` pair surfaces as a plain SQL primary-key violation.
#[derive(Debug)]
pub enum RecordProjectIntentRevisionError {
    Sql(rusqlite::Error),
    Revision(Vec<ProjectIntentError>),
}

impl From<rusqlite::Error> for RecordProjectIntentRevisionError {
    fn from(value: rusqlite::Error) -> Self {
        RecordProjectIntentRevisionError::Sql(value)
    }
}

/// A persisted §5.1 `ProjectIntentAmendment` plus when it landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIntentAmendmentRecord {
    pub amendment: ProjectIntentAmendment,
    pub created_at: String,
}

/// `record_project_intent_amendment`'s failure modes. Composes an
/// already-recorded current revision (looked up via
/// `load_current_project_intent_revision`, same "组合已存条目" pattern as
/// `issue_candidate_certificate` loading a readiness receipt by digest)
/// rather than requiring the caller to resupply the whole revision --
/// `NoCurrentRevision` is the store-level equivalent of
/// `IssueCandidateCertificateError::ReadinessNotFound`: there is no
/// revision yet to amend, not a domain rejection of a well-formed request.
#[derive(Debug)]
pub enum RecordProjectIntentAmendmentError {
    Sql(rusqlite::Error),
    NoCurrentRevision,
    Amendment(ProjectIntentAmendmentError),
}

impl From<rusqlite::Error> for RecordProjectIntentAmendmentError {
    fn from(value: rusqlite::Error) -> Self {
        RecordProjectIntentAmendmentError::Sql(value)
    }
}

/// A persisted §5.1 `ProjectInitializationReceipt` plus when it landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectInitializationReceiptRecord {
    pub receipt: ProjectInitializationReceipt,
    pub created_at: String,
}

/// `record_project_initialization_receipt`'s failure modes. Re-runs
/// `project_intent::issue_project_initialization_receipt` server-side.
#[derive(Debug)]
pub enum RecordProjectInitializationReceiptError {
    Sql(rusqlite::Error),
    Receipt(ProjectInitializationError),
}

impl From<rusqlite::Error> for RecordProjectInitializationReceiptError {
    fn from(value: rusqlite::Error) -> Self {
        RecordProjectInitializationReceiptError::Sql(value)
    }
}

/// A persisted §5.1 `GlobalConfigRevision` plus when it landed. Unlike
/// `ProjectIntentRevisionRecord`, there is no separate "amendment" concept
/// here -- a new `GlobalConfigRevision` row *is* the amendment, gated
/// entirely by `save_global_config_revision`'s preview re-check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalConfigRevisionRecord {
    pub revision: GlobalConfigRevision,
    pub created_at: String,
}

/// `save_global_config_revision`'s failure modes: the SQL layer (a
/// duplicate `revision`, surfaced as a primary-key violation), or
/// `config::validate_save_global_config_revision`'s own preview-hash/
/// project-set/expiry/second-confirmation checks rejecting the save
/// (`Rejected`, carrying every violation found rather than just the
/// first -- same multi-error shape the domain function itself returns).
#[derive(Debug)]
pub enum SaveGlobalConfigRevisionError {
    Sql(rusqlite::Error),
    Rejected(Vec<SaveGlobalConfigError>),
}

impl From<rusqlite::Error> for SaveGlobalConfigRevisionError {
    fn from(value: rusqlite::Error) -> Self {
        SaveGlobalConfigRevisionError::Sql(value)
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

#[derive(Debug)]
pub enum ExecutionQueueAppendError {
    Sql(rusqlite::Error),
    Transition(ExecutionQueueError),
}

impl From<rusqlite::Error> for ExecutionQueueAppendError {
    fn from(value: rusqlite::Error) -> Self {
        ExecutionQueueAppendError::Sql(value)
    }
}

#[derive(Debug)]
pub enum NodeAppendError {
    Sql(rusqlite::Error),
    Transition(NodeTransitionError),
}

impl From<rusqlite::Error> for NodeAppendError {
    fn from(value: rusqlite::Error) -> Self {
        NodeAppendError::Sql(value)
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

/// Mirrors `AppendedGraphEvent`, except this aggregate has no caller-chosen
/// `aggregate_id`: `ExecutionQueue` is a fleet-wide singleton, so
/// `append_execution_queue_event`/`load_execution_queue_state` always key
/// the same fixed row (see `EXECUTION_QUEUE_AGGREGATE_ID`) rather than
/// taking one as a parameter.
#[derive(Debug, Clone)]
pub struct AppendedExecutionQueueEvent {
    pub seq: i64,
    pub event_id: String,
    pub revision: u64,
    pub event_type: &'static str,
    pub occurred_at: String,
    pub state: ExecutionQueue,
}

/// Mirrors `AppendedGraphEvent`, except `state` is a bare `NodeStatus`
/// rather than a struct -- see `migrate_v12`'s doc comment for why there is
/// no wrapper type to project into.
#[derive(Debug, Clone)]
pub struct AppendedNodeEvent {
    pub seq: i64,
    pub event_id: String,
    pub revision: u64,
    pub event_type: &'static str,
    pub occurred_at: String,
    pub state: NodeStatus,
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

/// §11.1: "数据库使用编号迁移". Each step runs once, in its own
/// transaction, gated by `PRAGMA user_version`; a pre-existing database
/// (tables already present via the old `CREATE TABLE IF NOT EXISTS`-only
/// scheme, `user_version` still 0) replays every step from the top —
/// `migrate_v1`'s `IF NOT EXISTS` statements are idempotent against that
/// case, and later steps only add what is genuinely missing.
type MigrationStep = fn(&Connection) -> rusqlite::Result<()>;

const MIGRATIONS: &[MigrationStep] = &[
    migrate_v1,
    migrate_v2,
    migrate_v3,
    migrate_v4,
    migrate_v5,
    migrate_v6,
    migrate_v7,
    migrate_v8,
    migrate_v9,
    migrate_v10,
    migrate_v11,
    migrate_v12,
    migrate_v13,
    migrate_v14,
    migrate_v15,
    migrate_v16,
    migrate_v17,
    migrate_v18,
    migrate_v19,
    migrate_v21,
];

fn migrate_v1(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
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
        CREATE TABLE IF NOT EXISTS execution_queue_projections (
            aggregate_id TEXT PRIMARY KEY,
            revision INTEGER NOT NULL,
            state_json TEXT NOT NULL
        );
        ",
    )
}

/// Adds `task_projections.project_id`, backfilled from the JSON already
/// stored in `state_json` (`Task.project_id` was always present there —
/// this migration only promotes it to a queryable column), so
/// `list_task_summaries` can filter by project without deserializing every
/// row. Plan §5.1: any cross-project id mixing is a `ProtocolViolation`;
/// a real column (plus the index below) is what makes that check cheap
/// enough to run on every `task.get`/`task.list`.
fn migrate_v2(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        ALTER TABLE task_projections ADD COLUMN project_id TEXT;
        UPDATE task_projections
            SET project_id = json_extract(state_json, '$.project_id')
            WHERE project_id IS NULL;
        CREATE INDEX IF NOT EXISTS idx_task_projections_project_id
            ON task_projections(project_id);
        ",
    )
}

/// §5.1 identity: `display_name` and `identity_json` (the full
/// `ProjectIdentity`, for `load_project_identity`). Both nullable with no
/// backfill — nothing in `state_json` ever carried this data, so a
/// pre-migrate_v3 row's identity is honestly unknown rather than
/// fabricated (plan discipline: never render unverified data as
/// verified). Only `create_project` populates these columns going
/// forward.
fn migrate_v3(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        ALTER TABLE project_projections ADD COLUMN display_name TEXT;
        ALTER TABLE project_projections ADD COLUMN identity_json TEXT;
        ",
    )
}

/// §8.2's two-phase target registration: `project_targets` holds one row
/// per `register_target` call. `kind`/`canonical_path` are denormalized
/// out of `identity_probe_json` purely so `load_target` and future
/// listing UIs don't need to deserialize JSON just to filter/display;
/// `identity_probe_json`/`inspection_json` are the authoritative values.
/// `trust_confirmed` defaults to 0 at registration time — the trust gate
/// (§8.3:1354) is decided at `create_project_from_target` time, not here.
/// `consumed_by_project_id` starts NULL and is set exactly once, by
/// whichever `create_project_from_target` call successfully claims this
/// target — the `WHERE consumed_by_project_id IS NULL` guard on that
/// UPDATE is what makes "the same target cannot create two projects"
/// race-safe under concurrent Main-process commands.
fn migrate_v4(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS project_targets (
            target_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            registered_at TEXT NOT NULL,
            canonical_path TEXT NOT NULL,
            identity_probe_json TEXT NOT NULL,
            inspection_json TEXT NOT NULL,
            trust_confirmed INTEGER NOT NULL DEFAULT 0,
            consumed_by_project_id TEXT
        );
        ",
    )
}

/// `run_workspaces`: one row per `workspace::create_disposable_clone` call
/// (plan §8.1), recorded by `create_disposable_clone_for_run` immediately
/// after the clone lands on disk. `(task_id, run_id)` is the primary key —
/// matching `workspace.rs`'s own "a second call for the same pair fails"
/// discipline (`create_owned_dir` refuses to reuse the `run_id` directory),
/// so a duplicate row could never legitimately occur here either.
fn migrate_v5(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS run_workspaces (
            task_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            repo_path TEXT NOT NULL,
            head_commit TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (task_id, run_id)
        );
        ",
    )
}

/// `attempts`: one row per `record_attempt` call (plan §5.6). `attempt_id`
/// is the primary key -- `Attempt::id` is caller-chosen and globally unique
/// by construction (see `record_attempt`'s doc comment), so a duplicate row
/// could only mean the caller tried to reuse an id, which must fail rather
/// than silently overwrite what a step was actually permitted to do.
/// `loop_step_id`/`purpose` are denormalized out of `attempt_json` for the
/// same reason `migrate_v4`'s `project_targets` denormalizes `kind`:
/// `attempt_json`/`profile_json` remain the authoritative values.
fn migrate_v6(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS attempts (
            attempt_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            loop_step_id TEXT NOT NULL,
            purpose TEXT NOT NULL,
            attempt_json TEXT NOT NULL,
            profile_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_attempts_run_id ON attempts(run_id);
        ",
    )
}

/// `evidence_receipts`: one row per `record_evidence` call (plan §5.7).
/// `receipt_id` is the primary key, matching `ReceiptId`'s caller-chosen,
/// globally-unique-by-construction discipline -- same reasoning as
/// `migrate_v6`'s `attempts` table. `run_id`/`check_id` are denormalized out
/// of `receipt_json` purely to make `run_id`-scoped listing cheap later;
/// `receipt_json` remains the authoritative value.
fn migrate_v7(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS evidence_receipts (
            receipt_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            check_id TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_evidence_receipts_run_id ON evidence_receipts(run_id);
        ",
    )
}

/// `readiness_receipts`: one row per `record_readiness` call (plan §5.9).
/// `receipt_digest` is the primary key -- §5.9 revisions each get a fresh
/// digest rather than mutating in place ("环境相关输入、profile 或模型资格
/// 变化会追加新的 ReadinessReceipt revision"), so the digest is already the
/// natural globally-unique identity, matching `migrate_v6`/`migrate_v7`'s
/// caller-chosen-id discipline even though nothing here calls it an "id".
/// `profile_hash` is denormalized out of `receipt_json` purely to make
/// profile-scoped listing cheap later; `receipt_json` remains authoritative.
fn migrate_v8(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS readiness_receipts (
            receipt_digest TEXT PRIMARY KEY,
            profile_hash TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_readiness_receipts_profile_hash ON readiness_receipts(profile_hash);
        ",
    )
}

/// `delivery_chains`: one row per `start_delivery_chain` call (plan §5.12).
/// `run_id` is the primary key -- one `DeliveryChain` per run, matching
/// `DeliveryChain::new` fixing `subject` exactly once for the run's whole
/// delivery lifecycle. `subject_json` is fixed at `start_delivery_chain`
/// time; the five `*_json` rung columns start NULL and are each set exactly
/// once by the matching `append_delivery_*` method -- `DeliveryChain` itself
/// has no `Serialize`/`Deserialize` (see `DeliveryChainRecord`'s doc
/// comment), so each rung's own receipt type is what gets persisted, not
/// the chain object.
fn migrate_v9(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS delivery_chains (
            run_id TEXT PRIMARY KEY,
            subject_json TEXT NOT NULL,
            rehearsal_json TEXT,
            approval_json TEXT,
            delivery_json TEXT,
            tree_check_json TEXT,
            project_target_transition_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        ",
    )
}

/// `candidate_certificates`/`completion_certificates`: one row per
/// `issue_candidate_certificate`/`issue_completion_certificate` call (plan
/// §5.8). Both are keyed by `run_id` -- at most one of each certificate
/// per run, matching the domain's own one-shot issuance functions (issuing
/// again for the same run would mean re-litigating a decision that was
/// already made, so a duplicate write is refused as a primary-key
/// violation, same convention as `delivery_chains`/`readiness_receipts`).
fn migrate_v10(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS candidate_certificates (
            run_id TEXT PRIMARY KEY,
            certificate_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS completion_certificates (
            run_id TEXT PRIMARY KEY,
            certificate_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        ",
    )
}

/// `frozen_playbooks`: one row per `bind_playbook` call (plan §10.3). `run_id`
/// is the primary key -- a Run binds exactly one playbook for its whole
/// lifetime ("运行时固定内容哈希"), so binding again for the same run would
/// mean silently swapping the playbook out from under an in-progress Run;
/// that is refused as a primary-key violation, same convention as
/// `delivery_chains`/`candidate_certificates`. `FrozenPlaybook` itself has no
/// `run_id` field (mirrors `Attempt` in `migrate_v6`), so `run_id` is an
/// explicit column here, not something pulled out of `playbook_json`.
fn migrate_v11(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS frozen_playbooks (
            run_id TEXT PRIMARY KEY,
            playbook_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        ",
    )
}

/// `node_projections`: current `NodeStatus` per node aggregate (plan §6.3).
/// `NodeStatus` has no substructure to index on -- unlike `run_projections`/
/// `project_projections`, which carry derived query columns alongside their
/// JSON blob, there is a single `status_json` column that *is* the whole
/// projection. Mirrors `graph_projections`'s reasoning for omitting a
/// `status` column (see `migrate_v3`/`graph::apply`'s doc comment), just one
/// step further since here there's no other field either. A node's
/// `aggregate_id` is caller-composed (e.g. `"{run_id}:{node_id}"`) since a
/// bare `NodeId` repeats across Runs whenever a graph re-executes from
/// Pending (§6.6) -- this module imposes no structure on the string, same as
/// every other per-`aggregate_id` aggregate here.
fn migrate_v12(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS node_projections (
            aggregate_id TEXT PRIMARY KEY,
            revision INTEGER NOT NULL,
            status_json TEXT NOT NULL
        );
        ",
    )
}

/// `credentials`/`credential_receipts` (plan §8.3). `credentials` is keyed
/// by the caller-chosen `credential_ref` and holds the *current*
/// `CredentialRecord` snapshot -- `record_credential` upserts this row
/// rather than refusing a duplicate key, since rotation/revocation are
/// supposed to update the one record in place (see
/// `RecordCredentialError`'s doc comment). `credential_receipts` is a
/// genuine append-only log (§8.3: "创建、替换、撤销...都生成
/// CredentialReceipt"), so it is *not* keyed by `credential_ref` -- an
/// autoincrement `id` orders the log, mirroring how `events` uses `seq`
/// for the same reason, and `credential_ref` is only an index here, not a
/// primary key.
fn migrate_v13(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS credentials (
            credential_ref TEXT PRIMARY KEY,
            record_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS credential_receipts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            credential_ref TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_credential_receipts_credential_ref
            ON credential_receipts(credential_ref);
        ",
    )
}

/// `user_correction_receipts`: one row per `record_user_correction` call
/// (plan §6.5). Same "write returns Value not Event, one-time fact record"
/// shape as `readiness_receipts` -- `receipt_digest` is the caller-chosen
/// primary key, `run_id` is indexed (same reasoning as `attempts`/
/// `evidence_receipts`) even though no `list`-by-`run_id` read exists yet.
fn migrate_v14(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS user_correction_receipts (
            receipt_digest TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_user_correction_receipts_run_id
            ON user_correction_receipts(run_id);
        ",
    )
}

/// `planning_policy_restarts`/`run_policy_amendments`/`budget_grant_receipts`:
/// one row per issued §5.1 restart/amendment/grant. Same "write returns
/// Value not Event, one-time fact record" shape as
/// `user_correction_receipts` -- each digest is the domain-chosen primary
/// key, `task_id`/`run_id` are indexed (same reasoning as `attempts`/
/// `evidence_receipts`) even though no `list`-by-`task_id`/`run_id` read
/// exists yet.
fn migrate_v15(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS planning_policy_restarts (
            restart_digest TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            restart_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_planning_policy_restarts_task_id
            ON planning_policy_restarts(task_id);
        CREATE TABLE IF NOT EXISTS run_policy_amendments (
            amendment_digest TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            amendment_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_run_policy_amendments_run_id
            ON run_policy_amendments(run_id);
        CREATE TABLE IF NOT EXISTS budget_grant_receipts (
            grant_digest TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_budget_grant_receipts_run_id
            ON budget_grant_receipts(run_id);
        ",
    )
}

/// §5.1 project_intent's three tables. `project_intent_revisions` uses a
/// composite `(project_id, revision)` primary key — same reasoning as
/// `run_workspaces`' `(task_id, run_id)` key from §8.1: multiple revisions
/// legitimately coexist per project, so no single column identifies a row.
fn migrate_v16(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS project_intent_revisions (
            project_id TEXT NOT NULL,
            revision INTEGER NOT NULL,
            revision_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (project_id, revision)
        );
        CREATE TABLE IF NOT EXISTS project_intent_amendments (
            amendment_hash TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            amendment_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_project_intent_amendments_project_id
            ON project_intent_amendments(project_id);
        CREATE TABLE IF NOT EXISTS project_initialization_receipts (
            receipt_digest TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_project_initialization_receipts_project_id
            ON project_initialization_receipts(project_id);
        ",
    )
}

/// §5.10 `qualification_receipts`: one row per `issue_qualification_receipt`
/// call. Same "write returns Value not Event, one-time fact record" shape as
/// `readiness_receipts` -- `receipt_digest` is the domain-chosen primary
/// key. `qualification_batch_id` (from `identity.qualification_batch_id`,
/// not a top-level `QualificationReceipt` field) is indexed for the same
/// reason `user_correction_receipts.run_id` is -- no `list`-by-batch read
/// exists yet, but every other receipt/event table in this file indexes its
/// natural lookup column rather than waiting for the read to justify it.
fn migrate_v17(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS qualification_receipts (
            receipt_digest TEXT PRIMARY KEY,
            qualification_batch_id TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_qualification_receipts_batch_id
            ON qualification_receipts(qualification_batch_id);
        ",
    )
}

/// §5.11's four tables: `skill_install_receipts` (fact record, PK
/// `receipt_digest`, same shape as `qualification_receipts`) and three
/// mutable current-state snapshots that get `INSERT OR REPLACE`d rather than
/// appended -- `skill_evidence_ladders` (PK `skill_digest`, a ladder is
/// mutated in place by `mark_*`/`record_effective` and there is no domain
/// type modelling its history), `global_skill_bindings` (PK `skill_digest`;
/// `GlobalSkillBinding` has no validating constructor of its own, so unlike
/// every fact-record table above there is nothing to re-check before
/// overwriting) and `project_skill_bindings` (composite PK
/// `(project_id, skill_digest)`, third occurrence of a composite key after
/// `run_workspaces`/`project_intent_revisions` -- `ProjectSkillBinding` has
/// no `skill_digest` field of its own, so the key's second half always comes
/// from the caller, not the row).
fn migrate_v18(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS skill_install_receipts (
            receipt_digest TEXT PRIMARY KEY,
            package_digest TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_skill_install_receipts_package_digest
            ON skill_install_receipts(package_digest);

        CREATE TABLE IF NOT EXISTS skill_evidence_ladders (
            skill_digest TEXT PRIMARY KEY,
            ladder_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS global_skill_bindings (
            skill_digest TEXT PRIMARY KEY,
            binding_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS project_skill_bindings (
            project_id TEXT NOT NULL,
            skill_digest TEXT NOT NULL,
            binding_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (project_id, skill_digest)
        );
        ",
    )
}

/// §5.1 `human_review_receipts`: one row per `issue_human_review_receipt`
/// call. Same "write returns Value not Event, one-time fact record" shape as
/// `qualification_receipts` -- `receipt_digest` is the domain-chosen primary
/// key. `run_hash` is indexed for the same reason
/// `qualification_receipts.qualification_batch_id` is -- no `list`-by-run
/// read exists yet, but every other receipt table in this file indexes its
/// natural lookup column rather than waiting for the read to justify it.
/// `HumanReviewFinding` and `CarriedPlanningReviewBundle` get no table of
/// their own: findings are supplied by the caller to
/// `issue_human_review_receipt`/`can_resubmit_for_review` rather than
/// tracked as current state here (see `review.rs`'s own module doc for why
/// this increment stops at the receipt), and
/// `validate_carried_planning_review_bundle` is a stateless check like
/// `model_selection::validate_model_separation`.
fn migrate_v19(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS human_review_receipts (
            receipt_digest TEXT PRIMARY KEY,
            run_hash TEXT NOT NULL,
            receipt_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_human_review_receipts_run_hash
            ON human_review_receipts(run_hash);
        ",
    )
}

/// config.rs's two tables (this file's 19th wired module, and -- along with
/// its `review`/`replan` siblings landing in parallel worktrees -- the last
/// of the pure-logic modules with zero real callers). `global_config_revisions`
/// is append-only keyed by `revision` alone (not a composite key like
/// `project_intent_revisions`'s `(project_id, revision)`): §5.1's config
/// model has exactly one global lineage, not one per project, so `revision`
/// on its own is already a natural, globally-unique identity -- same
/// reasoning as `qualification_receipts`/`skill_install_receipts` using
/// their own single natural key. `project_config_patches` is a
/// *current-state* row per project (`PRIMARY KEY (project_id)`, `INSERT OR
/// REPLACE`d), not append-only -- unlike `GlobalConfigRevision`, nothing in
/// `config.rs` treats old `ProjectConfigPatch` revisions as still
/// addressable once superseded, matching `global_skill_bindings`'s
/// current-row reasoning rather than `project_intent_revisions`'s
/// append-only one.
fn migrate_v21(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS global_config_revisions (
            revision INTEGER PRIMARY KEY,
            revision_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS project_config_patches (
            project_id TEXT PRIMARY KEY,
            patch_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        ",
    )
}

impl EventStore {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        let mut store = Self {
            conn,
            db_path: std::path::PathBuf::from(path),
            diagnostic: None,
        };
        store.migrate()?;
        store.diagnostic = store.verify_disk_layout();
        Ok(store)
    }

    /// §5.1: `project_home` is decided by Core, never accepted from
    /// Renderer params. This increment's rule (D14 anticipates a real
    /// platform application-support directory; that wiring is Electron
    /// Main's job, not this crate's): a `projects/<project-id>/` sibling
    /// of wherever this store's database file lives.
    pub fn project_home_for(&self, project_id: &str) -> String {
        let base = self
            .db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        base.join("projects")
            .join(project_id)
            .to_string_lossy()
            .into_owned()
    }

    /// Shared RFC3339 "now" formatting — the same expression this file
    /// already used inline at every `recorded_at` call site; factored out
    /// here because the two-phase target flow needs it in several more
    /// places (`register_target` and twice inside
    /// `create_project_from_target_tx`'s transaction).
    fn now_rfc3339() -> String {
        time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails")
    }

    /// §8.3:1362's startup re-check: the data root (this database file's
    /// parent directory) and every ProjectHome this store already knows
    /// about (one per `project_projections.aggregate_id`, but only the
    /// ones that actually have a directory on disk — rows created by the
    /// older disk-free `create_project` never had one, and are silently
    /// skipped rather than flagged) must still be owner-only,
    /// non-symlinked, and inside the expected boundary. A brand-new store
    /// (nothing under `projects/` yet) is not a failure — that directory
    /// is only checked if it already exists, since
    /// `create_project_from_target` creates it on demand. Returns
    /// `Some(reason)` on the first failure found, `None` if everything
    /// still checks out. Called once, at the end of `open()`; the result
    /// is cached in `self.diagnostic` rather than re-checked on every
    /// call.
    fn verify_disk_layout(&self) -> Option<String> {
        let data_root = self
            .db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();

        if std::fs::symlink_metadata(&data_root).is_ok()
            && let Err(e) = fs_guard::verify_owned_dir(&data_root, &data_root)
        {
            return Some(format!(
                "data root {data_root:?} failed re-verification: {e:?}"
            ));
        }

        let projects_root = data_root.join("projects");
        if std::fs::symlink_metadata(&projects_root).is_ok()
            && let Err(e) = fs_guard::verify_owned_dir(&projects_root, &data_root)
        {
            return Some(format!(
                "projects root {projects_root:?} failed re-verification: {e:?}"
            ));
        }

        let mut stmt = match self
            .conn
            .prepare("SELECT aggregate_id FROM project_projections")
        {
            Ok(stmt) => stmt,
            Err(e) => return Some(format!("could not query project_projections: {e:?}")),
        };
        let rows = match stmt.query_map([], |row| row.get::<_, String>(0)) {
            Ok(rows) => rows,
            Err(e) => return Some(format!("could not read project_projections rows: {e:?}")),
        };
        let collected: rusqlite::Result<Vec<String>> = rows.collect();
        let project_ids = match collected {
            Ok(ids) => ids,
            Err(e) => return Some(format!("could not collect project_projections rows: {e:?}")),
        };

        for project_id in project_ids {
            let home = std::path::PathBuf::from(self.project_home_for(&project_id));
            if std::fs::symlink_metadata(&home).is_err() {
                // No ProjectHome on disk for this id — either a legacy row
                // from the pre-target `create_project`, or (should never
                // happen) a row whose directory was removed out of band.
                // Either way there is nothing here for this check to
                // re-verify.
                continue;
            }
            if let Err(e) = fs_guard::verify_owned_dir(&home, &data_root) {
                return Some(format!(
                    "ProjectHome {home:?} for project {project_id} failed re-verification: {e:?}"
                ));
            }
        }

        None
    }

    /// `Some(reason)` when `verify_disk_layout` found a problem at open
    /// time — §8.3:1362's read-only diagnostic state. Callers (see
    /// `dispatch::dispatch`) must check this before any write and refuse
    /// with a diagnostic error if set; reads are unaffected.
    pub fn diagnostic_reason(&self) -> Option<&str> {
        self.diagnostic.as_deref()
    }

    fn migrate(&mut self) -> rusqlite::Result<()> {
        let current_version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        for (index, step) in MIGRATIONS.iter().enumerate() {
            let target_version = (index + 1) as i64;
            if current_version >= target_version {
                continue;
            }
            let tx = self.conn.transaction()?;
            step(&tx)?;
            tx.execute_batch(&format!("PRAGMA user_version = {target_version}"))?;
            tx.commit()?;
        }
        Ok(())
    }

    /// The `events` table's current max `seq`, or 0 for an empty journal.
    /// This is the `snapshot_seq` every `Reply` carries (plan §3.2: "先取
    /// 带 snapshot_seq 的快照，再从 snapshot_seq + 1 订阅").
    pub fn latest_event_seq(&self) -> rusqlite::Result<u64> {
        self.conn
            .query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |row| {
                let seq: i64 = row.get(0)?;
                Ok(seq as u64)
            })
    }

    /// One row per known Project, from the projection table only — never
    /// deserializes `state_json`, since the projection columns already
    /// carry everything a list view needs (plan's read path is snapshot,
    /// not full-state, for every row but the one the caller drilled into).
    pub fn list_project_summaries(&self) -> rusqlite::Result<Vec<ProjectSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT aggregate_id, revision, lifecycle, phase, hold, display_name, identity_json
             FROM project_projections ORDER BY aggregate_id",
        )?;
        let rows = stmt.query_map([], |row| {
            let revision: i64 = row.get(1)?;
            let identity_json: Option<String> = row.get(6)?;
            Ok(ProjectSummary {
                id: row.get(0)?,
                revision: revision as u64,
                lifecycle: row.get(2)?,
                phase: row.get(3)?,
                hold: row.get(4)?,
                display_name: row.get(5)?,
                kind: identity_json
                    .and_then(|json| serde_json::from_str::<ProjectIdentity>(&json).ok())
                    .map(|identity| identity.kind),
            })
        })?;
        rows.collect()
    }

    /// The full `ProjectIdentity` for `project.get`, or `None` when the
    /// aggregate has no projection row or predates migrate_v3 (in which
    /// case `identity_json` is `NULL`).
    pub fn load_project_identity(
        &self,
        aggregate_id: &str,
    ) -> rusqlite::Result<Option<ProjectIdentity>> {
        self.conn
            .query_row(
                "SELECT identity_json FROM project_projections WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .map(|identity_json| {
                serde_json::from_str(&identity_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .transpose()
    }

    /// For `queue.get`'s `entry_labels`: resolves every task id's
    /// `project_id` and that project's `display_name` in a single query
    /// (plan: "一次往返，不做 N+1"). A task with no known project, or a
    /// project with no registered display name, resolves to `None` rather
    /// than a fabricated label.
    pub fn resolve_task_project_labels(
        &self,
        task_ids: &[String],
    ) -> rusqlite::Result<Vec<TaskProjectLabel>> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = task_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT t.aggregate_id, t.project_id, p.display_name
             FROM task_projections t
             LEFT JOIN project_projections p ON p.aggregate_id = t.project_id
             WHERE t.aggregate_id IN ({placeholders})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(task_ids.iter()), |row| {
            Ok(TaskProjectLabel {
                task_id: row.get(0)?,
                project_id: row.get(1)?,
                project_display_name: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// One row per Task belonging to `project_id`. Filtering happens in
    /// SQL against the indexed column from `migrate_v2`, not in Rust after
    /// loading every task — the whole point of the column existing.
    pub fn list_task_summaries(&self, project_id: &str) -> rusqlite::Result<Vec<TaskSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT aggregate_id, revision, lifecycle
             FROM task_projections WHERE project_id = ?1 ORDER BY aggregate_id",
        )?;
        let rows = stmt.query_map(params![project_id], |row| {
            let revision: i64 = row.get(1)?;
            Ok(TaskSummary {
                id: row.get(0)?,
                revision: revision as u64,
                lifecycle: row.get(2)?,
            })
        })?;
        rows.collect()
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

    /// §5.1: the only way a Project comes into existence. Single
    /// transaction: rejects with `AlreadyExists` if `aggregate_id` already
    /// has a projection row; otherwise writes the `project.created` event
    /// (payload = `identity`, revision 1) and the initial projection
    /// (`state_json` = `ProjectState::new()`, plus the `display_name`/
    /// `identity_json` columns migrate_v3 added). After this,
    /// `append_project_event` on this id no longer returns `NotFound`.
    pub fn create_project(
        &mut self,
        identity: &ProjectIdentity,
    ) -> Result<AppendedProjectEvent, ProjectAppendError> {
        let tx = self.conn.transaction()?;

        let already_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_projections WHERE aggregate_id = ?1)",
            params![identity.id],
            |row| row.get(0),
        )?;
        if already_exists {
            return Err(ProjectAppendError::AlreadyExists);
        }

        let state = ProjectState::new();
        let event_id = uuid::Uuid::new_v4().to_string();
        let event_type: &'static str = "project.created";
        let identity_json =
            serde_json::to_string(identity).expect("ProjectIdentity always serializes");
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails");
        let state_json = serde_json::to_string(&state).expect("ProjectState always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Project', 1, ?3, ?4, ?5)",
            params![event_id, identity.id, event_type, identity_json, recorded_at],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO project_projections (aggregate_id, revision, lifecycle, phase, hold, state_json, display_name, identity_json)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                identity.id,
                format!("{:?}", state.lifecycle),
                format!("{:?}", state.phase),
                format!("{:?}", state.hold),
                state_json,
                identity.display_name,
                identity_json,
            ],
        )?;

        tx.commit()?;
        Ok(AppendedProjectEvent {
            seq,
            event_id,
            revision: 1,
            event_type,
            occurred_at: recorded_at,
            state,
        })
    }

    /// Applies `event` to the current state of `aggregate_id`, and — in a
    /// single transaction — appends the event and updates the projection.
    /// On an illegal transition, nothing is written: the event never
    /// existed as far as the journal is concerned. Mirrors
    /// `append_run_event`; see its doc comment for the shape this pattern
    /// generalizes from. Unlike `append_run_event`, an unknown
    /// `aggregate_id` is `NotFound`, not an implicit `ProjectState::new()`
    /// — §5.1: Projects only come into existence via `create_project`.
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
                None => return Err(ProjectAppendError::NotFound),
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
            "INSERT INTO task_projections (aggregate_id, revision, lifecycle, state_json, project_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                lifecycle = excluded.lifecycle,
                state_json = excluded.state_json,
                project_id = excluded.project_id",
            params![
                aggregate_id,
                next_revision as i64,
                format!("{:?}", next_state.lifecycle),
                state_json,
                next_state.project_id
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

    /// Loads the current projected (revision, ExecutionQueue), or `None` if
    /// no event has ever been journaled for it — in which case callers
    /// should treat the queue as `ExecutionQueue::new()` at revision 0.
    /// Unlike the five per-`aggregate_id` aggregates above, there is no
    /// `aggregate_id` parameter: `ExecutionQueue` is a single fleet-wide
    /// singleton, always keyed by the fixed `EXECUTION_QUEUE_AGGREGATE_ID`.
    pub fn load_execution_queue_state(&self) -> rusqlite::Result<Option<(u64, ExecutionQueue)>> {
        self.conn
            .query_row(
                "SELECT revision, state_json FROM execution_queue_projections WHERE aggregate_id = ?1",
                params![EXECUTION_QUEUE_AGGREGATE_ID],
                |row| {
                    let revision: i64 = row.get(0)?;
                    let state_json: String = row.get(1)?;
                    Ok((revision, state_json))
                },
            )
            .optional()?
            .map(|(revision, state_json)| {
                let state: ExecutionQueue = serde_json::from_str(&state_json).map_err(|e| {
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

    /// Applies `event` to the current singleton `ExecutionQueue` state, and
    /// — in the same transaction — appends the event and updates the
    /// projection. This is where plan §6.2's "ExecutionQueue 和
    /// HarnessLease 在同一个 Rust 事务里更新" is satisfied: both live inside
    /// the one `ExecutionQueue` value, serialized into the one
    /// `state_json` column written by this transaction. Does not also emit
    /// a `task.dispatch_state_projected` event for affected Tasks — that
    /// remains a deliberately separate follow-up call by whichever caller
    /// drives the queue, not something this method does implicitly.
    pub fn append_execution_queue_event(
        &mut self,
        event: ExecutionQueueEvent,
    ) -> Result<AppendedExecutionQueueEvent, ExecutionQueueAppendError> {
        let tx = self.conn.transaction()?;

        let (revision, current_state) = {
            let loaded = tx
                .query_row(
                    "SELECT revision, state_json FROM execution_queue_projections WHERE aggregate_id = ?1",
                    params![EXECUTION_QUEUE_AGGREGATE_ID],
                    |row| {
                        let revision: i64 = row.get(0)?;
                        let state_json: String = row.get(1)?;
                        Ok((revision, state_json))
                    },
                )
                .optional()?;
            match loaded {
                Some((revision, state_json)) => {
                    let state: ExecutionQueue = serde_json::from_str(&state_json).expect(
                        "execution_queue_projections.state_json is only ever written by this module as valid ExecutionQueue JSON",
                    );
                    (revision as u64, state)
                }
                None => (0, ExecutionQueue::new()),
            }
        };

        let event_type = execution_queue_event_type_name(&event);
        let payload = serde_json::to_string(&event).expect("ExecutionQueueEvent always serializes");
        let next_state = execution_queue::apply(current_state, event)
            .map_err(ExecutionQueueAppendError::Transition)?;
        let next_revision = revision + 1;
        let event_id = uuid::Uuid::new_v4().to_string();
        let recorded_at = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .expect("RFC3339 formatting of now_utc never fails");
        let state_json =
            serde_json::to_string(&next_state).expect("ExecutionQueue always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'ExecutionQueue', ?3, ?4, ?5, ?6)",
            params![
                event_id,
                EXECUTION_QUEUE_AGGREGATE_ID,
                next_revision as i64,
                event_type,
                payload,
                recorded_at
            ],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO execution_queue_projections (aggregate_id, revision, state_json)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                state_json = excluded.state_json",
            params![
                EXECUTION_QUEUE_AGGREGATE_ID,
                next_revision as i64,
                state_json
            ],
        )?;

        tx.commit()?;
        Ok(AppendedExecutionQueueEvent {
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

    /// §8.2 phase ①→③: persists the result of probing a target (already
    /// done by the caller via `target_probe::probe_target` — this method
    /// takes the probe, not a path, so it never touches the filesystem
    /// itself) as a new `project_targets` row and hands back an opaque
    /// `target_id`. Nothing about the underlying path is ever handed back
    /// to a Renderer beyond what the probe/inspection summary already
    /// reveals — see `dispatch::handle_command`'s `project.register_target`
    /// branch, which is the only caller.
    pub fn register_target(
        &mut self,
        kind: ProjectKind,
        probe: &TargetIdentityProbe,
        inspection: &TargetInspection,
    ) -> rusqlite::Result<String> {
        let target_id = uuid::Uuid::new_v4().to_string();
        let kind_str = format!("{kind:?}");
        let canonical_path = probe.canonical_path.to_string_lossy().into_owned();
        let probe_json =
            serde_json::to_string(probe).expect("TargetIdentityProbe always serializes");
        let inspection_json =
            serde_json::to_string(inspection).expect("TargetInspection always serializes");
        let registered_at = Self::now_rfc3339();

        self.conn.execute(
            "INSERT INTO project_targets
                (target_id, kind, registered_at, canonical_path, identity_probe_json, inspection_json, trust_confirmed, consumed_by_project_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL)",
            params![
                target_id,
                kind_str,
                registered_at,
                canonical_path,
                probe_json,
                inspection_json
            ],
        )?;

        Ok(target_id)
    }

    /// Loads a previously registered target by id, or `None` if unknown.
    pub fn load_target(&self, target_id: &str) -> rusqlite::Result<Option<TargetRecord>> {
        self.conn
            .query_row(
                "SELECT target_id, kind, registered_at, canonical_path, identity_probe_json,
                        inspection_json, trust_confirmed, consumed_by_project_id
                 FROM project_targets WHERE target_id = ?1",
                params![target_id],
                Self::target_record_from_row,
            )
            .optional()
    }

    fn target_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TargetRecord> {
        let target_id: String = row.get(0)?;
        let kind_str: String = row.get(1)?;
        let kind = match kind_str.as_str() {
            "NewProduct" => ProjectKind::NewProduct,
            "ExistingRepository" => ProjectKind::ExistingRepository,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::<dyn std::error::Error + Send + Sync>::from(format!(
                        "unknown project_targets.kind {other:?}"
                    )),
                ));
            }
        };
        let registered_at: String = row.get(2)?;
        let canonical_path: String = row.get(3)?;
        let probe_json: String = row.get(4)?;
        let probe: TargetIdentityProbe = serde_json::from_str(&probe_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let inspection_json: String = row.get(5)?;
        let inspection: TargetInspection = serde_json::from_str(&inspection_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let trust_confirmed_int: i64 = row.get(6)?;
        let consumed_by_project_id: Option<String> = row.get(7)?;

        Ok(TargetRecord {
            target_id,
            kind,
            registered_at,
            canonical_path,
            probe,
            inspection,
            trust_confirmed: trust_confirmed_int != 0,
            consumed_by_project_id,
        })
    }

    /// Marks `target_id` as consumed by `project_id` in its own,
    /// standalone transaction. `create_project_from_target` does *not*
    /// call this — it performs the equivalent guarded `UPDATE` inline,
    /// inside its own single transaction alongside the event writes, so
    /// consumption and project creation are atomic together. This exists
    /// for direct/test use and any future caller that wants
    /// target-consumption as its own atomic step.
    pub fn consume_target(
        &mut self,
        target_id: &str,
        project_id: &str,
    ) -> Result<(), TargetConsumeError> {
        let tx = self.conn.transaction()?;
        let current: Option<Option<String>> = tx
            .query_row(
                "SELECT consumed_by_project_id FROM project_targets WHERE target_id = ?1",
                params![target_id],
                |row| row.get(0),
            )
            .optional()?;
        match current {
            None => return Err(TargetConsumeError::NotFound),
            Some(Some(_)) => return Err(TargetConsumeError::AlreadyConsumed),
            Some(None) => {}
        }
        let claimed = tx.execute(
            "UPDATE project_targets SET consumed_by_project_id = ?1
             WHERE target_id = ?2 AND consumed_by_project_id IS NULL",
            params![project_id, target_id],
        )?;
        if claimed == 0 {
            // Raced with another consumer between the SELECT above and
            // this UPDATE.
            return Err(TargetConsumeError::AlreadyConsumed);
        }
        tx.commit()?;
        Ok(())
    }

    /// §8.2's write path: turns an already-registered, already-probed
    /// target into a real Project. In order: loads the target (rejecting
    /// an unknown or already-consumed one), enforces the §8.3:1354 trust
    /// gate for `ExistingRepository`, re-probes `destination_absent` fresh
    /// (never trusts the value cached at registration time — see
    /// `TargetIdentityProbe::to_inspection`'s own doc comment), derives
    /// the `ProjectLocator` via `autome_domain::project::locator_for`,
    /// creates the owner-only ProjectHome directory and a manifest file on
    /// disk, then — in a single SQL transaction — writes `project.created`
    /// (revision 1) followed by 4x `AdvanceNominal` and one
    /// `IntentUnresolved` (revisions 2-6), claiming the target row in the
    /// same transaction. If anything after the ProjectHome directory is
    /// created goes wrong (including a race where the target got consumed
    /// between the pre-check above and the transaction), the just-created
    /// directory is removed — no orphaned directory, no half-created
    /// project. This method does not itself check `diagnostic_reason()`;
    /// the dispatch layer is responsible for refusing all write commands
    /// while the store is in its diagnostic state (see
    /// `dispatch::dispatch`'s check at the top of its match).
    pub fn create_project_from_target(
        &mut self,
        target_id: &str,
        display_name: &str,
        trust_confirmed: bool,
        destination_name: Option<&str>,
    ) -> Result<CreatedProjectFromTarget, CreateFromTargetError> {
        let target = self
            .load_target(target_id)?
            .ok_or(CreateFromTargetError::TargetNotFound)?;
        if target.consumed_by_project_id.is_some() {
            return Err(CreateFromTargetError::TargetAlreadyConsumed);
        }
        if target.kind == ProjectKind::ExistingRepository && !trust_confirmed {
            return Err(CreateFromTargetError::TrustNotConfirmed);
        }

        let identity_str = target.probe.canonical_path.to_string_lossy().into_owned();
        let destination_absent = match target.kind {
            ProjectKind::NewProduct => {
                let name = destination_name.unwrap_or("");
                std::fs::symlink_metadata(target.probe.canonical_path.join(name)).is_err()
            }
            ProjectKind::ExistingRepository => false,
        };
        let inspection = target.probe.to_inspection(destination_absent);
        let locator =
            project::locator_for(target.kind, &identity_str, destination_name, inspection)
                .map_err(CreateFromTargetError::Rejected)?;

        let project_id = uuid::Uuid::new_v4().to_string();
        let project_home = self.project_home_for(&project_id);
        let identity_struct = ProjectIdentity::new(
            &project_id,
            display_name,
            target.kind,
            locator,
            &project_home,
        )
        .map_err(CreateFromTargetError::Identity)?;

        let data_root = self
            .db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let projects_root_path = data_root.join("projects");
        let projects_root_guard = if std::fs::symlink_metadata(&projects_root_path).is_ok() {
            fs_guard::verify_owned_dir(&projects_root_path, &data_root)?
        } else {
            fs_guard::create_owned_dir(&data_root, "projects")?
        };
        let dir_guard =
            fs_guard::create_owned_dir(&projects_root_guard.canonical_path, &project_id)?;

        let manifest = serde_json::json!({
            "project_id": project_id,
            "target_id": target_id,
            "trust_confirmed": trust_confirmed,
            "identity": identity_struct,
            "created_at": Self::now_rfc3339(),
        });
        let manifest_bytes =
            serde_json::to_vec_pretty(&manifest).expect("manifest json always serializes");
        if let Err(e) = fs_guard::write_owned_file(&dir_guard, "manifest.json", &manifest_bytes) {
            let _ = std::fs::remove_dir_all(&dir_guard.canonical_path);
            return Err(e.into());
        }

        let identity_json =
            serde_json::to_string(&identity_struct).expect("ProjectIdentity always serializes");
        match create_project_from_target_tx(
            &mut self.conn,
            &project_id,
            target_id,
            display_name,
            &identity_json,
        ) {
            Ok(appended) => Ok(CreatedProjectFromTarget {
                project_id,
                appended,
            }),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir_guard.canonical_path);
                Err(e)
            }
        }
    }

    /// §8.1's write path: builds a fresh `runs/<task_id>/<run_id>/repo`
    /// disposable clone of `source_repo` via `workspace::create_disposable_clone`,
    /// then records the resulting `(repo_path, head_commit)` as one row in
    /// `run_workspaces`. Unlike the five event-sourced aggregates, a
    /// disposable clone has no state-machine transitions of its own — it is
    /// a fact recorded once — so this is a plain SQL insert, not a journaled
    /// event, mirroring how `register_target` records a `project_targets`
    /// row rather than an `Event`. A second call for the same
    /// `task_id`/`run_id` fails: `workspace::create_disposable_clone`
    /// refuses to reuse an existing `<run_id>` directory, and the
    /// `(task_id, run_id)` primary key would refuse the duplicate row even
    /// if it somehow got that far.
    pub fn create_disposable_clone_for_run(
        &mut self,
        task_id: &str,
        run_id: &str,
        source_repo: &std::path::Path,
    ) -> Result<DisposableCloneRecord, CreateDisposableCloneError> {
        let data_root = self
            .db_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let data_root_guard = fs_guard::verify_owned_dir(&data_root, &data_root)
            .map_err(WorkspaceError::from)?;
        let runs_root = workspace::ensure_runs_root(&data_root_guard)?;
        let clone = workspace::create_disposable_clone(&runs_root, task_id, run_id, source_repo)?;

        let repo_path = clone.repo_path.to_string_lossy().into_owned();
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO run_workspaces (task_id, run_id, repo_path, head_commit, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![task_id, run_id, repo_path, clone.head_commit, created_at],
        )?;

        Ok(DisposableCloneRecord {
            task_id: task_id.to_string(),
            run_id: run_id.to_string(),
            repo_path,
            head_commit: clone.head_commit,
            created_at,
        })
    }

    /// Read counterpart to `create_disposable_clone_for_run` — looks up the
    /// recorded `run_workspaces` row for `(task_id, run_id)`, if any.
    pub fn load_disposable_clone_for_run(
        &self,
        task_id: &str,
        run_id: &str,
    ) -> rusqlite::Result<Option<DisposableCloneRecord>> {
        self.conn
            .query_row(
                "SELECT task_id, run_id, repo_path, head_commit, created_at \
                 FROM run_workspaces WHERE task_id = ?1 AND run_id = ?2",
                rusqlite::params![task_id, run_id],
                |row| {
                    Ok(DisposableCloneRecord {
                        task_id: row.get(0)?,
                        run_id: row.get(1)?,
                        repo_path: row.get(2)?,
                        head_commit: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                },
            )
            .optional()
    }

    /// §5.6's write path: records an `Attempt` bound to the
    /// `AttemptPermissionProfile` it must run under, before the step is
    /// ever dispatched to a Harness. Like `create_disposable_clone_for_run`,
    /// this is a fact recorded once, not a journaled event: nothing here
    /// transitions any aggregate's state, it fixes what a step was
    /// permitted to do so any later drift (in tool surface, filesystem
    /// scope, etc.) can be checked against a durable record instead of a
    /// Prompt's suggestion. Every §5.6 invariant already proven in
    /// `autome_domain::attempt` is re-checked here rather than trusted from
    /// the caller -- `attempt.validate_shape()`, `profile.validate()` and
    /// `validate_planning_attempt_is_read_only` must all pass before
    /// anything is written; the first failure short-circuits with no write.
    pub fn record_attempt(
        &mut self,
        run_id: &str,
        attempt: &Attempt,
        profile: &AttemptPermissionProfile,
    ) -> Result<AttemptRecord, RecordAttemptError> {
        attempt.validate_shape().map_err(RecordAttemptError::Shape)?;
        let violations = profile.validate();
        if !violations.is_empty() {
            return Err(RecordAttemptError::PermissionViolations(violations));
        }
        attempt::validate_planning_attempt_is_read_only(attempt, profile)
            .map_err(RecordAttemptError::PlanningWrite)?;

        let attempt_json = serde_json::to_string(attempt).expect("Attempt is serializable");
        let profile_json =
            serde_json::to_string(profile).expect("AttemptPermissionProfile is serializable");
        let purpose = format!("{:?}", attempt.purpose);
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO attempts (attempt_id, run_id, loop_step_id, purpose, attempt_json, profile_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                attempt.id.0,
                run_id,
                attempt.loop_step_id.0,
                purpose,
                attempt_json,
                profile_json,
                created_at,
            ],
        )?;

        Ok(AttemptRecord {
            run_id: run_id.to_string(),
            attempt: attempt.clone(),
            permission_profile: profile.clone(),
            created_at,
        })
    }

    /// Read counterpart to `record_attempt` — looks up the recorded
    /// `attempts` row for `attempt_id`, if any.
    pub fn load_attempt(&self, attempt_id: &str) -> rusqlite::Result<Option<AttemptRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT run_id, attempt_json, profile_json, created_at FROM attempts WHERE attempt_id = ?1",
                rusqlite::params![attempt_id],
                |row| {
                    let run_id: String = row.get(0)?;
                    let attempt_json: String = row.get(1)?;
                    let profile_json: String = row.get(2)?;
                    let created_at: String = row.get(3)?;
                    Ok((run_id, attempt_json, profile_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(run_id, attempt_json, profile_json, created_at)| {
            let attempt: Attempt =
                serde_json::from_str(&attempt_json).expect("attempts.attempt_json round-trips");
            let permission_profile: AttemptPermissionProfile = serde_json::from_str(&profile_json)
                .expect("attempts.profile_json round-trips");
            AttemptRecord {
                run_id,
                attempt,
                permission_profile,
                created_at,
            }
        }))
    }

    /// §8.3's write path for the *current-state* `credentials` row.
    /// Deliberately upserts (`INSERT OR REPLACE`) rather than refusing a
    /// duplicate `credential_ref`, unlike every fact-record write above --
    /// see `RecordCredentialError`'s doc comment for why.
    pub fn record_credential(
        &mut self,
        credential_ref: &str,
        record: &CredentialRecord,
    ) -> Result<CredentialRecordRow, RecordCredentialError> {
        let shape_errors = record.validate_shape();
        if !shape_errors.is_empty() {
            return Err(RecordCredentialError::Shape(shape_errors));
        }

        let record_json = serde_json::to_string(record).expect("CredentialRecord is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO credentials (credential_ref, record_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![credential_ref, record_json, created_at],
        )?;

        Ok(CredentialRecordRow {
            credential_ref: credential_ref.to_string(),
            record: record.clone(),
            created_at,
        })
    }

    /// Read counterpart to `record_credential` -- the current snapshot for
    /// `credential_ref`, if one has ever been recorded.
    pub fn load_credential(
        &self,
        credential_ref: &str,
    ) -> rusqlite::Result<Option<CredentialRecordRow>> {
        let row = self
            .conn
            .query_row(
                "SELECT record_json, created_at FROM credentials WHERE credential_ref = ?1",
                rusqlite::params![credential_ref],
                |row| {
                    let record_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((record_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(record_json, created_at)| {
            let record: CredentialRecord =
                serde_json::from_str(&record_json).expect("credentials.record_json round-trips");
            CredentialRecordRow {
                credential_ref: credential_ref.to_string(),
                record,
                created_at,
            }
        }))
    }

    /// §8.3's write path for the append-only `credential_receipts` log.
    /// Re-runs `credential::issue_credential_receipt` server-side rather
    /// than trusting an already-built `CredentialReceipt` from the caller,
    /// same discipline as `record_attempt` re-validating shape/profile.
    pub fn record_credential_receipt(
        &mut self,
        credential_ref: &str,
        event: CredentialEvent,
        occurred_at: &str,
        operator: Option<&str>,
    ) -> Result<CredentialReceipt, RecordCredentialReceiptError> {
        let receipt = credential::issue_credential_receipt(credential_ref, event, occurred_at, operator)
            .map_err(RecordCredentialReceiptError::Receipt)?;

        let receipt_json = serde_json::to_string(&receipt).expect("CredentialReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO credential_receipts (credential_ref, receipt_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![credential_ref, receipt_json, created_at],
        )?;

        Ok(receipt)
    }

    /// Read counterpart to `record_credential_receipt` -- every receipt
    /// ever issued for `credential_ref`, in append order.
    pub fn list_credential_receipts(
        &self,
        credential_ref: &str,
    ) -> rusqlite::Result<Vec<CredentialReceipt>> {
        let mut stmt = self.conn.prepare(
            "SELECT receipt_json FROM credential_receipts WHERE credential_ref = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(rusqlite::params![credential_ref], |row| {
            let receipt_json: String = row.get(0)?;
            Ok(receipt_json)
        })?;
        rows.map(|r| {
            r.map(|receipt_json| {
                serde_json::from_str(&receipt_json)
                    .expect("credential_receipts.receipt_json round-trips")
            })
        })
        .collect()
    }

    /// §6.5's write path: re-runs
    /// `user_correction::issue_user_correction_receipt` server-side --
    /// every raw field the domain constructor needs, not a pre-built
    /// receipt -- same "don't trust the caller already validated"
    /// discipline as `record_credential_receipt`. `INSERT`s rather than
    /// upserts: a `UserCorrectionReceipt` is a one-time fact about a single
    /// user correction, not current state to overwrite.
    #[allow(clippy::too_many_arguments)]
    pub fn record_user_correction(
        &mut self,
        project_id: &str,
        task_id: &str,
        run_id: &str,
        attempt_id: Option<&str>,
        planning_spec_hash: &str,
        execution_spec_hash: Option<&str>,
        execution_spec_frozen: bool,
        raw_text_ref: &str,
        attachment_hashes: Vec<String>,
        submitted_at: &str,
        operator: &str,
        subject_contract_hash: Option<&str>,
        subject_graph_hash: Option<&str>,
        subject_candidate_hash: Option<&str>,
        classification: CorrectionClassification,
        impact: CorrectionImpactFlags,
        affected_requirement_ids: Vec<String>,
        affected_node_ids: Vec<String>,
        disposition: CorrectionDisposition,
        successor_ref: Option<&str>,
        receipt_digest: &str,
    ) -> Result<UserCorrectionRecord, RecordUserCorrectionError> {
        let receipt = user_correction::issue_user_correction_receipt(
            project_id,
            task_id,
            run_id,
            attempt_id,
            planning_spec_hash,
            execution_spec_hash,
            execution_spec_frozen,
            raw_text_ref,
            attachment_hashes,
            submitted_at,
            operator,
            subject_contract_hash,
            subject_graph_hash,
            subject_candidate_hash,
            classification,
            impact,
            affected_requirement_ids,
            affected_node_ids,
            disposition,
            successor_ref,
            receipt_digest,
        )
        .map_err(RecordUserCorrectionError::Receipt)?;

        let receipt_json =
            serde_json::to_string(&receipt).expect("UserCorrectionReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO user_correction_receipts (receipt_digest, run_id, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![receipt.receipt_digest, receipt.run_id, receipt_json, created_at],
        )?;

        Ok(UserCorrectionRecord { receipt, created_at })
    }

    /// Read counterpart to `record_user_correction` -- looks up the
    /// recorded `user_correction_receipts` row for `receipt_digest`, if any.
    pub fn load_user_correction(
        &self,
        receipt_digest: &str,
    ) -> rusqlite::Result<Option<UserCorrectionRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM user_correction_receipts WHERE receipt_digest = ?1",
                rusqlite::params![receipt_digest],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: UserCorrectionReceipt = serde_json::from_str(&receipt_json)
                .expect("user_correction_receipts.receipt_json round-trips");
            UserCorrectionRecord { receipt, created_at }
        }))
    }

    /// §5.1's write path for `PlanningPolicyRestart`: re-runs
    /// `policy_restart::issue_planning_policy_restart` server-side, same
    /// discipline as `record_user_correction`. `INSERT`s rather than
    /// upserts -- a restart is a one-time fact, not current state to
    /// overwrite.
    #[allow(clippy::too_many_arguments)]
    pub fn record_planning_policy_restart(
        &mut self,
        task_id: &str,
        current_run_id: &str,
        old_planning_spec_hash: &str,
        proposed_planning_spec_hash: &str,
        trigger_revision_ref: &str,
        config_revision_ref: &str,
        skill_revision_ref: &str,
        capability_revision_ref: &str,
        invalidated_document_attempt_ids: Vec<String>,
        approval_receipt: &str,
        restart_digest: &str,
    ) -> Result<PlanningPolicyRestartRecord, RecordPlanningPolicyRestartError> {
        let restart = policy_restart::issue_planning_policy_restart(
            task_id,
            current_run_id,
            old_planning_spec_hash,
            proposed_planning_spec_hash,
            trigger_revision_ref,
            config_revision_ref,
            skill_revision_ref,
            capability_revision_ref,
            invalidated_document_attempt_ids,
            approval_receipt,
            restart_digest,
        )
        .map_err(RecordPlanningPolicyRestartError::Restart)?;

        let restart_json =
            serde_json::to_string(&restart).expect("PlanningPolicyRestart is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO planning_policy_restarts (restart_digest, task_id, run_id, restart_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                restart.restart_digest,
                restart.task_id,
                restart.current_run_id,
                restart_json,
                created_at
            ],
        )?;

        Ok(PlanningPolicyRestartRecord { restart, created_at })
    }

    /// Read counterpart to `record_planning_policy_restart`.
    pub fn load_planning_policy_restart(
        &self,
        restart_digest: &str,
    ) -> rusqlite::Result<Option<PlanningPolicyRestartRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT restart_json, created_at FROM planning_policy_restarts WHERE restart_digest = ?1",
                rusqlite::params![restart_digest],
                |row| {
                    let restart_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((restart_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(restart_json, created_at)| {
            let restart: PlanningPolicyRestart = serde_json::from_str(&restart_json)
                .expect("planning_policy_restarts.restart_json round-trips");
            PlanningPolicyRestartRecord { restart, created_at }
        }))
    }

    /// §5.1's write path for `RunPolicyAmendment`: re-runs
    /// `policy_restart::issue_run_policy_amendment` server-side.
    #[allow(clippy::too_many_arguments)]
    pub fn record_run_policy_amendment(
        &mut self,
        task_id: &str,
        current_run_id: &str,
        old_execution_spec_hash: &str,
        proposed_execution_spec_hash: &str,
        unchanged_contract_hash: &str,
        unchanged_graph_hash: &str,
        unchanged_base_hash: &str,
        policy_diff: &str,
        invalidated_attempt_ids: Vec<String>,
        invalidated_evidence_ids: Vec<String>,
        invalidated_audit_ids: Vec<String>,
        invalidated_candidate_ids: Vec<String>,
        approval_receipt: &str,
        amendment_digest: &str,
    ) -> Result<RunPolicyAmendmentRecord, RecordRunPolicyAmendmentError> {
        let amendment = policy_restart::issue_run_policy_amendment(
            task_id,
            current_run_id,
            old_execution_spec_hash,
            proposed_execution_spec_hash,
            unchanged_contract_hash,
            unchanged_graph_hash,
            unchanged_base_hash,
            policy_diff,
            invalidated_attempt_ids,
            invalidated_evidence_ids,
            invalidated_audit_ids,
            invalidated_candidate_ids,
            approval_receipt,
            amendment_digest,
        )
        .map_err(RecordRunPolicyAmendmentError::Amendment)?;

        let amendment_json =
            serde_json::to_string(&amendment).expect("RunPolicyAmendment is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO run_policy_amendments (amendment_digest, task_id, run_id, amendment_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                amendment.amendment_digest,
                amendment.task_id,
                amendment.current_run_id,
                amendment_json,
                created_at
            ],
        )?;

        Ok(RunPolicyAmendmentRecord { amendment, created_at })
    }

    /// Read counterpart to `record_run_policy_amendment`.
    pub fn load_run_policy_amendment(
        &self,
        amendment_digest: &str,
    ) -> rusqlite::Result<Option<RunPolicyAmendmentRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT amendment_json, created_at FROM run_policy_amendments WHERE amendment_digest = ?1",
                rusqlite::params![amendment_digest],
                |row| {
                    let amendment_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((amendment_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(amendment_json, created_at)| {
            let amendment: RunPolicyAmendment = serde_json::from_str(&amendment_json)
                .expect("run_policy_amendments.amendment_json round-trips");
            RunPolicyAmendmentRecord { amendment, created_at }
        }))
    }

    /// §5.1's write path for `BudgetGrantReceipt`: re-runs
    /// `policy_restart::issue_budget_grant_receipt` server-side.
    #[allow(clippy::too_many_arguments)]
    pub fn record_budget_grant(
        &mut self,
        run_id: &str,
        current_budget_hash: &str,
        added_limits: Vec<BudgetLimitGrant>,
        reason: &str,
        operator: &str,
        expiry: Option<&str>,
        grant_digest: &str,
    ) -> Result<BudgetGrantRecord, RecordBudgetGrantError> {
        let receipt = policy_restart::issue_budget_grant_receipt(
            run_id,
            current_budget_hash,
            added_limits,
            reason,
            operator,
            expiry,
            grant_digest,
        )
        .map_err(RecordBudgetGrantError::Grant)?;

        let receipt_json =
            serde_json::to_string(&receipt).expect("BudgetGrantReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO budget_grant_receipts (grant_digest, run_id, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![receipt.grant_digest, receipt.run_id, receipt_json, created_at],
        )?;

        Ok(BudgetGrantRecord { receipt, created_at })
    }

    /// Read counterpart to `record_budget_grant`.
    pub fn load_budget_grant(
        &self,
        grant_digest: &str,
    ) -> rusqlite::Result<Option<BudgetGrantRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM budget_grant_receipts WHERE grant_digest = ?1",
                rusqlite::params![grant_digest],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: BudgetGrantReceipt = serde_json::from_str(&receipt_json)
                .expect("budget_grant_receipts.receipt_json round-trips");
            BudgetGrantRecord { receipt, created_at }
        }))
    }

    /// §5.1's write path for `ProjectIntentRevision`: re-runs
    /// `project_intent::issue_project_intent_revision` server-side.
    #[allow(clippy::too_many_arguments)]
    pub fn record_project_intent_revision(
        &mut self,
        project_id: &str,
        revision: u32,
        source_anchors: Vec<String>,
        approved_by: &str,
        approved_at: &str,
        product_goal: &str,
        target_users: Vec<String>,
        durable_cross_task_constraints: Vec<String>,
        explicit_non_goals: Vec<String>,
        key_decisions: Vec<KeyDecision>,
        supersedes: Option<u32>,
        intent_hash: &str,
    ) -> Result<ProjectIntentRevisionRecord, RecordProjectIntentRevisionError> {
        let revision = project_intent::issue_project_intent_revision(
            project_id,
            IntentRevision(revision),
            source_anchors,
            approved_by,
            approved_at,
            product_goal,
            target_users,
            durable_cross_task_constraints,
            explicit_non_goals,
            key_decisions,
            supersedes.map(IntentRevision),
            intent_hash,
        )
        .map_err(RecordProjectIntentRevisionError::Revision)?;

        let revision_json =
            serde_json::to_string(&revision).expect("ProjectIntentRevision is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO project_intent_revisions (project_id, revision, revision_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                revision.project_id,
                revision.revision.0,
                revision_json,
                created_at
            ],
        )?;

        Ok(ProjectIntentRevisionRecord {
            revision,
            created_at,
        })
    }

    /// Read counterpart to `record_project_intent_revision` for one exact
    /// `(project_id, revision)` pair.
    pub fn load_project_intent_revision(
        &self,
        project_id: &str,
        revision: u32,
    ) -> rusqlite::Result<Option<ProjectIntentRevisionRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT revision_json, created_at FROM project_intent_revisions \
                 WHERE project_id = ?1 AND revision = ?2",
                rusqlite::params![project_id, revision],
                |row| {
                    let revision_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((revision_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(revision_json, created_at)| {
            let revision: ProjectIntentRevision = serde_json::from_str(&revision_json)
                .expect("project_intent_revisions.revision_json round-trips");
            ProjectIntentRevisionRecord {
                revision,
                created_at,
            }
        }))
    }

    /// The derived "current revision" read a project actually has: the
    /// highest `revision` recorded for `project_id`, not a separately
    /// stored value. `record_project_intent_amendment` uses this to find
    /// the revision an amendment request is checked against.
    pub fn load_current_project_intent_revision(
        &self,
        project_id: &str,
    ) -> rusqlite::Result<Option<ProjectIntentRevisionRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT revision_json, created_at FROM project_intent_revisions \
                 WHERE project_id = ?1 ORDER BY revision DESC LIMIT 1",
                rusqlite::params![project_id],
                |row| {
                    let revision_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((revision_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(revision_json, created_at)| {
            let revision: ProjectIntentRevision = serde_json::from_str(&revision_json)
                .expect("project_intent_revisions.revision_json round-trips");
            ProjectIntentRevisionRecord {
                revision,
                created_at,
            }
        }))
    }

    /// §5.1's write path for `ProjectIntentAmendment`: loads the project's
    /// current revision (an already-recorded prerequisite, same "组合已存
    /// 条目" pattern as `issue_candidate_certificate` loading a readiness
    /// receipt by digest) and checks the request against it. Only records
    /// the amendment itself and hands back `to_revision` -- building the
    /// next authoritative `ProjectIntentRevision` is a separate call to
    /// `record_project_intent_revision`, matching
    /// `project_intent::apply_project_intent_amendment`'s own division of
    /// labor.
    #[allow(clippy::too_many_arguments)]
    pub fn record_project_intent_amendment(
        &mut self,
        project_id: &str,
        from_revision: u32,
        trigger_task: Option<&str>,
        semantic_diff: &str,
        affected_active_tasks: Vec<String>,
        user_decision_receipt: &str,
        amendment_hash: &str,
    ) -> Result<ProjectIntentAmendmentRecord, RecordProjectIntentAmendmentError> {
        let current = self
            .load_current_project_intent_revision(project_id)?
            .ok_or(RecordProjectIntentAmendmentError::NoCurrentRevision)?;

        let request = ProjectIntentAmendmentRequest {
            project_id: project_id.to_string(),
            from_revision: IntentRevision(from_revision),
            trigger_task: trigger_task.map(|s| s.to_string()),
            semantic_diff: semantic_diff.to_string(),
            affected_active_tasks,
            user_decision_receipt: user_decision_receipt.to_string(),
            amendment_hash: amendment_hash.to_string(),
        };
        let amendment = project_intent::apply_project_intent_amendment(&current.revision, request)
            .map_err(RecordProjectIntentAmendmentError::Amendment)?;

        let amendment_json =
            serde_json::to_string(&amendment).expect("ProjectIntentAmendment is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO project_intent_amendments (amendment_hash, project_id, amendment_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                amendment.amendment_hash,
                amendment.project_id,
                amendment_json,
                created_at
            ],
        )?;

        Ok(ProjectIntentAmendmentRecord {
            amendment,
            created_at,
        })
    }

    /// Read counterpart to `record_project_intent_amendment`.
    pub fn load_project_intent_amendment(
        &self,
        amendment_hash: &str,
    ) -> rusqlite::Result<Option<ProjectIntentAmendmentRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT amendment_json, created_at FROM project_intent_amendments WHERE amendment_hash = ?1",
                rusqlite::params![amendment_hash],
                |row| {
                    let amendment_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((amendment_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(amendment_json, created_at)| {
            let amendment: ProjectIntentAmendment = serde_json::from_str(&amendment_json)
                .expect("project_intent_amendments.amendment_json round-trips");
            ProjectIntentAmendmentRecord {
                amendment,
                created_at,
            }
        }))
    }

    /// §5.1's write path for `ProjectInitializationReceipt`: re-runs
    /// `project_intent::issue_project_initialization_receipt` server-side.
    #[allow(clippy::too_many_arguments)]
    pub fn record_project_initialization_receipt(
        &mut self,
        project_id: &str,
        project_revision: u32,
        subject_identity_hash: &str,
        trust_decision_ref: Option<&str>,
        environment_snapshot_id: &str,
        skill_inventory_id: &str,
        project_home_manifest: &str,
        result: InitializationResult,
        issues: Vec<String>,
        receipt_digest: &str,
    ) -> Result<ProjectInitializationReceiptRecord, RecordProjectInitializationReceiptError> {
        let receipt = project_intent::issue_project_initialization_receipt(
            project_id,
            project_revision,
            subject_identity_hash,
            trust_decision_ref,
            environment_snapshot_id,
            skill_inventory_id,
            project_home_manifest,
            result,
            issues,
            receipt_digest,
        )
        .map_err(RecordProjectInitializationReceiptError::Receipt)?;

        let receipt_json = serde_json::to_string(&receipt)
            .expect("ProjectInitializationReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO project_initialization_receipts (receipt_digest, project_id, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                receipt.receipt_digest,
                receipt.project_id,
                receipt_json,
                created_at
            ],
        )?;

        Ok(ProjectInitializationReceiptRecord {
            receipt,
            created_at,
        })
    }

    /// Read counterpart to `record_project_initialization_receipt`.
    pub fn load_project_initialization_receipt(
        &self,
        receipt_digest: &str,
    ) -> rusqlite::Result<Option<ProjectInitializationReceiptRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM project_initialization_receipts WHERE receipt_digest = ?1",
                rusqlite::params![receipt_digest],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: ProjectInitializationReceipt = serde_json::from_str(&receipt_json)
                .expect("project_initialization_receipts.receipt_json round-trips");
            ProjectInitializationReceiptRecord {
                receipt,
                created_at,
            }
        }))
    }

    /// §5.7's write path: records an `EvidenceReceipt` produced by a
    /// verifier run. Like `record_attempt`, this is a fact recorded once,
    /// not a journaled event -- staleness is a query-time property
    /// (`EvidenceReceipt::is_valid_against`), not something this method
    /// decides, so there is nothing to validate before writing beyond what
    /// the primary key already enforces.
    pub fn record_evidence(
        &mut self,
        receipt: &EvidenceReceipt,
    ) -> Result<EvidenceRecord, RecordEvidenceError> {
        let receipt_json = serde_json::to_string(receipt).expect("EvidenceReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO evidence_receipts (receipt_id, run_id, check_id, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                receipt.receipt_id.0,
                receipt.run_id,
                receipt.check_id.0,
                receipt_json,
                created_at,
            ],
        )?;

        Ok(EvidenceRecord {
            receipt: receipt.clone(),
            created_at,
        })
    }

    /// Read counterpart to `record_evidence` -- looks up the recorded
    /// `evidence_receipts` row for `receipt_id`, if any.
    pub fn load_evidence(&self, receipt_id: &str) -> rusqlite::Result<Option<EvidenceRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM evidence_receipts WHERE receipt_id = ?1",
                rusqlite::params![receipt_id],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: EvidenceReceipt = serde_json::from_str(&receipt_json)
                .expect("evidence_receipts.receipt_json round-trips");
            EvidenceRecord { receipt, created_at }
        }))
    }

    /// §5.9's write path: records a `ReadinessReceipt` produced by an
    /// environment probe. Same shape as `record_evidence` -- a fact recorded
    /// once, not a journaled event, since staleness/readiness are query-time
    /// properties this method doesn't decide.
    pub fn record_readiness(
        &mut self,
        receipt: &ReadinessReceipt,
    ) -> Result<ReadinessRecord, RecordReadinessError> {
        let receipt_json =
            serde_json::to_string(receipt).expect("ReadinessReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO readiness_receipts (receipt_digest, profile_hash, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                receipt.receipt_digest,
                receipt.profile_hash,
                receipt_json,
                created_at,
            ],
        )?;

        Ok(ReadinessRecord {
            receipt: receipt.clone(),
            created_at,
        })
    }

    /// Read counterpart to `record_readiness` -- looks up the recorded
    /// `readiness_receipts` row for `receipt_digest`, if any.
    pub fn load_readiness(
        &self,
        receipt_digest: &str,
    ) -> rusqlite::Result<Option<ReadinessRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM readiness_receipts WHERE receipt_digest = ?1",
                rusqlite::params![receipt_digest],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: ReadinessReceipt = serde_json::from_str(&receipt_json)
                .expect("readiness_receipts.receipt_json round-trips");
            ReadinessRecord { receipt, created_at }
        }))
    }

    /// §5.10's write path: records a `QualificationReceipt` issued by
    /// `issue_qualification_receipt`. Same shape as `record_readiness` -- a
    /// fact recorded once, not a journaled event, since the receipt's
    /// domain-level validity was already enforced at construction time.
    pub fn record_qualification_receipt(
        &mut self,
        receipt: &QualificationReceipt,
    ) -> Result<QualificationRecord, RecordQualificationReceiptError> {
        let receipt_json =
            serde_json::to_string(receipt).expect("QualificationReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO qualification_receipts (receipt_digest, qualification_batch_id, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                receipt.receipt_digest,
                receipt.identity.qualification_batch_id,
                receipt_json,
                created_at,
            ],
        )?;

        Ok(QualificationRecord {
            receipt: receipt.clone(),
            created_at,
        })
    }

    /// Read counterpart to `record_qualification_receipt` -- looks up the
    /// recorded `qualification_receipts` row for `receipt_digest`, if any.
    pub fn load_qualification_receipt(
        &self,
        receipt_digest: &str,
    ) -> rusqlite::Result<Option<QualificationRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM qualification_receipts WHERE receipt_digest = ?1",
                rusqlite::params![receipt_digest],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: QualificationReceipt = serde_json::from_str(&receipt_json)
                .expect("qualification_receipts.receipt_json round-trips");
            QualificationRecord { receipt, created_at }
        }))
    }

    /// §5.11's write path: records a `SkillInstallReceipt` issued by
    /// `issue_skill_install_receipt`, and in the same call seeds the
    /// freshly-`Installed` `SkillEvidenceLadder` it returns alongside it --
    /// installation and the ladder's first rung are the same fact per
    /// `skill.rs`'s own doc comment ("安装成功的默认终态只是 Vault 中
    /// `Installed (disabled)`"), never two separate calls a caller could
    /// skip between. Re-installing the same `skill_digest` (`INSERT OR
    /// REPLACE`, mirroring `record_credential`'s upsert reasoning) resets
    /// its ladder back down to `Installed`-only: a reinstalled package has
    /// to re-earn Bound/Discoverable/etc, exactly as a brand-new one does.
    pub fn record_skill_install_receipt(
        &mut self,
        package_digest: &str,
        audit_outcome: SkillAuditOutcome,
        plan_digest: &str,
        user_approval_decision_ref: &str,
        receipt_digest: &str,
    ) -> Result<SkillInstallRecord, RecordSkillInstallReceiptError> {
        let (receipt, ladder) = skill::issue_skill_install_receipt(
            package_digest,
            audit_outcome,
            plan_digest,
            user_approval_decision_ref,
            receipt_digest,
        )
        .map_err(RecordSkillInstallReceiptError::Install)?;

        let receipt_json =
            serde_json::to_string(&receipt).expect("SkillInstallReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO skill_install_receipts (receipt_digest, package_digest, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                receipt.receipt_digest,
                receipt.package_digest,
                receipt_json,
                created_at,
            ],
        )?;

        let ladder_json =
            serde_json::to_string(&ladder).expect("SkillEvidenceLadder is serializable");
        self.conn.execute(
            "INSERT OR REPLACE INTO skill_evidence_ladders (skill_digest, ladder_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![ladder.skill_digest.0, ladder_json, created_at],
        )?;

        Ok(SkillInstallRecord {
            receipt,
            ladder,
            created_at,
        })
    }

    /// Read counterpart to `record_skill_install_receipt` -- looks up the
    /// recorded `skill_install_receipts` row for `receipt_digest`, if any.
    /// Returns only the receipt, not the (separately mutable) current
    /// ladder -- use `load_skill_evidence_ladder` for that.
    pub fn load_skill_install_receipt(
        &self,
        receipt_digest: &str,
    ) -> rusqlite::Result<Option<SkillInstallReceiptRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM skill_install_receipts WHERE receipt_digest = ?1",
                rusqlite::params![receipt_digest],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: SkillInstallReceipt = serde_json::from_str(&receipt_json)
                .expect("skill_install_receipts.receipt_json round-trips");
            SkillInstallReceiptRecord { receipt, created_at }
        }))
    }

    /// Read counterpart to the ladder half of `record_skill_install_receipt`
    /// and every `mark_skill_*`/`record_skill_effective` transition below --
    /// the current `SkillEvidenceLadder` for `skill_digest`, if one has ever
    /// been seeded by an install.
    pub fn load_skill_evidence_ladder(
        &self,
        skill_digest: &str,
    ) -> rusqlite::Result<Option<SkillEvidenceLadderRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT ladder_json, created_at FROM skill_evidence_ladders WHERE skill_digest = ?1",
                rusqlite::params![skill_digest],
                |row| {
                    let ladder_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((ladder_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(ladder_json, created_at)| {
            let ladder: SkillEvidenceLadder = serde_json::from_str(&ladder_json)
                .expect("skill_evidence_ladders.ladder_json round-trips");
            SkillEvidenceLadderRecord { ladder, created_at }
        }))
    }

    /// Shared by every `mark_skill_*`/`record_skill_effective` method below:
    /// loads the current ladder (`NotFound` if none was ever seeded by an
    /// install), applies the domain-level rung transition, and persists the
    /// result back with the same `INSERT OR REPLACE` as the install path.
    fn transition_skill_ladder(
        &mut self,
        skill_digest: &str,
        apply: impl FnOnce(&mut SkillEvidenceLadder) -> Result<(), SkillEvidenceError>,
    ) -> Result<SkillEvidenceLadderRecord, SkillLadderTransitionError> {
        let mut current = self
            .load_skill_evidence_ladder(skill_digest)?
            .ok_or(SkillLadderTransitionError::NotFound)?;
        apply(&mut current.ladder).map_err(SkillLadderTransitionError::Ladder)?;
        let ladder_json =
            serde_json::to_string(&current.ladder).expect("SkillEvidenceLadder is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO skill_evidence_ladders (skill_digest, ladder_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![skill_digest, ladder_json, created_at],
        )?;
        Ok(SkillEvidenceLadderRecord {
            ladder: current.ladder,
            created_at,
        })
    }

    pub fn mark_skill_bound(
        &mut self,
        skill_digest: &str,
    ) -> Result<SkillEvidenceLadderRecord, SkillLadderTransitionError> {
        self.transition_skill_ladder(skill_digest, |ladder| ladder.mark_bound())
    }

    pub fn mark_skill_discoverable(
        &mut self,
        skill_digest: &str,
    ) -> Result<SkillEvidenceLadderRecord, SkillLadderTransitionError> {
        self.transition_skill_ladder(skill_digest, |ladder| ladder.mark_discoverable())
    }

    pub fn mark_skill_available_to_attempt(
        &mut self,
        skill_digest: &str,
    ) -> Result<SkillEvidenceLadderRecord, SkillLadderTransitionError> {
        self.transition_skill_ladder(skill_digest, |ladder| ladder.mark_available_to_attempt())
    }

    pub fn mark_skill_invoked(
        &mut self,
        skill_digest: &str,
    ) -> Result<SkillEvidenceLadderRecord, SkillLadderTransitionError> {
        self.transition_skill_ladder(skill_digest, |ladder| ladder.mark_invoked())
    }

    pub fn record_skill_effective(
        &mut self,
        skill_digest: &str,
        effective: bool,
    ) -> Result<SkillEvidenceLadderRecord, SkillLadderTransitionError> {
        self.transition_skill_ladder(skill_digest, |ladder| ladder.record_effective(effective))
    }

    /// §5.11's write path for the *current-state* `global_skill_bindings`
    /// row. `GlobalSkillBinding` has no validating constructor of its own
    /// (unlike `SkillInstallReceipt`/`QualificationReceipt` etc.), so unlike
    /// every fact-record write above there is nothing to re-check
    /// server-side before writing -- same reasoning as why `readiness.record`
    /// trusts a whole client-supplied `ReadinessReceipt`. Deliberately
    /// upserts (`INSERT OR REPLACE`), same reasoning as `record_credential`:
    /// this is a binding's *current* state, and a new `revision` amending it
    /// is expected to replace the row, not conflict with it.
    pub fn record_global_skill_binding(
        &mut self,
        binding: &GlobalSkillBinding,
    ) -> rusqlite::Result<String> {
        let binding_json =
            serde_json::to_string(binding).expect("GlobalSkillBinding is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO global_skill_bindings (skill_digest, binding_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![binding.skill_digest.0, binding_json, created_at],
        )?;
        Ok(created_at)
    }

    /// Read counterpart to `record_global_skill_binding` -- the current
    /// snapshot for `skill_digest`, if one has ever been recorded.
    pub fn load_global_skill_binding(
        &self,
        skill_digest: &str,
    ) -> rusqlite::Result<Option<GlobalSkillBinding>> {
        let binding_json: Option<String> = self
            .conn
            .query_row(
                "SELECT binding_json FROM global_skill_bindings WHERE skill_digest = ?1",
                rusqlite::params![skill_digest],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(binding_json.map(|binding_json| {
            serde_json::from_str(&binding_json)
                .expect("global_skill_bindings.binding_json round-trips")
        }))
    }

    /// §5.11's write path for the *current-state* `project_skill_bindings`
    /// row. `ProjectSkillBinding` carries no `skill_digest` field of its own
    /// (it is meaningless without a global binding to resolve against), so
    /// the composite key's second half always comes from the caller, not the
    /// struct -- same reasoning as `record_global_skill_binding` for why
    /// there is nothing to re-check server-side before upserting.
    pub fn record_project_skill_binding(
        &mut self,
        project_id: &str,
        skill_digest: &str,
        binding: &ProjectSkillBinding,
    ) -> rusqlite::Result<String> {
        let binding_json =
            serde_json::to_string(binding).expect("ProjectSkillBinding is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO project_skill_bindings (project_id, skill_digest, binding_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![project_id, skill_digest, binding_json, created_at],
        )?;
        Ok(created_at)
    }

    /// Read counterpart to `record_project_skill_binding` -- the current
    /// snapshot for `(project_id, skill_digest)`, if one has ever been
    /// recorded.
    pub fn load_project_skill_binding(
        &self,
        project_id: &str,
        skill_digest: &str,
    ) -> rusqlite::Result<Option<ProjectSkillBinding>> {
        let binding_json: Option<String> = self
            .conn
            .query_row(
                "SELECT binding_json FROM project_skill_bindings WHERE project_id = ?1 AND skill_digest = ?2",
                rusqlite::params![project_id, skill_digest],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(binding_json.map(|binding_json| {
            serde_json::from_str(&binding_json)
                .expect("project_skill_bindings.binding_json round-trips")
        }))
    }

    /// §5.1's write path: records a `HumanReviewReceipt` issued by
    /// `issue_human_review_receipt`. Calls the real constructor server-side
    /// (so a Reject decision without at least one anchored, actionable
    /// finding is actually refused rather than trusted from the caller) and
    /// then persists the result -- same "write returns Value not Event,
    /// one-time fact record" shape as `record_qualification_receipt`, since
    /// a human review decision is a fact issued once by an operator, not an
    /// aggregate with a reducer.
    #[allow(clippy::too_many_arguments)]
    pub fn record_human_review_receipt(
        &mut self,
        project_hash: &str,
        task_hash: &str,
        run_hash: &str,
        spec_subject: ReviewSpecSubject,
        step_id: attempt::LoopStepId,
        operator: &str,
        decided_at: &str,
        decision: ReviewDecision,
        review_output_hash: &str,
        reason: &str,
        findings: &[HumanReviewFinding],
        subject: ReviewSubject,
        receipt_digest: &str,
    ) -> Result<HumanReviewReceiptRecord, RecordHumanReviewReceiptError> {
        let receipt = review::issue_human_review_receipt(
            project_hash,
            task_hash,
            run_hash,
            spec_subject,
            step_id,
            operator,
            decided_at,
            decision,
            review_output_hash,
            reason,
            findings,
            subject,
            receipt_digest,
        )
        .map_err(RecordHumanReviewReceiptError::Receipt)?;

        let receipt_json =
            serde_json::to_string(&receipt).expect("HumanReviewReceipt is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO human_review_receipts (receipt_digest, run_hash, receipt_json, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                receipt.receipt_digest,
                receipt.run_hash,
                receipt_json,
                created_at,
            ],
        )?;

        Ok(HumanReviewReceiptRecord { receipt, created_at })
    }

    /// Read counterpart to `record_human_review_receipt` -- looks up the
    /// recorded `human_review_receipts` row for `receipt_digest`, if any.
    pub fn load_human_review_receipt(
        &self,
        receipt_digest: &str,
    ) -> rusqlite::Result<Option<HumanReviewReceiptRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT receipt_json, created_at FROM human_review_receipts WHERE receipt_digest = ?1",
                rusqlite::params![receipt_digest],
                |row| {
                    let receipt_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((receipt_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(receipt_json, created_at)| {
            let receipt: HumanReviewReceipt = serde_json::from_str(&receipt_json)
                .expect("human_review_receipts.receipt_json round-trips");
            HumanReviewReceiptRecord { receipt, created_at }
        }))
    }

    /// config.rs's write path for `GlobalConfigRevision`: re-runs
    /// `config::validate_save_global_config_revision` server-side against
    /// the caller-supplied `preview` (rather than trusting the caller's own
    /// judgment that the preview is still fresh and matches), and only then
    /// appends the new revision row. `GlobalConfigRevision` itself has no
    /// validating constructor of its own in `config.rs` -- the *save*
    /// operation is what's gated, not the shape of the revision being
    /// saved -- so it is otherwise trusted directly, same reasoning as
    /// `record_global_skill_binding` trusting a whole client-supplied
    /// `GlobalSkillBinding`.
    pub fn save_global_config_revision(
        &mut self,
        revision: GlobalConfigRevision,
        preview: &GlobalConfigImpactPreview,
        submitted_preview_hash: &str,
        current_project_set_hash: &str,
        second_confirmation_acquired: bool,
    ) -> Result<GlobalConfigRevisionRecord, SaveGlobalConfigRevisionError> {
        let now = time::OffsetDateTime::now_utc();
        let errors = config::validate_save_global_config_revision(
            preview,
            submitted_preview_hash,
            current_project_set_hash,
            now,
            second_confirmation_acquired,
        );
        if !errors.is_empty() {
            return Err(SaveGlobalConfigRevisionError::Rejected(errors));
        }

        let revision_json =
            serde_json::to_string(&revision).expect("GlobalConfigRevision is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO global_config_revisions (revision, revision_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![revision.revision, revision_json, created_at],
        )?;

        Ok(GlobalConfigRevisionRecord {
            revision,
            created_at,
        })
    }

    /// Read counterpart to `save_global_config_revision` for one exact
    /// `revision` number.
    pub fn load_global_config_revision(
        &self,
        revision: u32,
    ) -> rusqlite::Result<Option<GlobalConfigRevisionRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT revision_json, created_at FROM global_config_revisions WHERE revision = ?1",
                rusqlite::params![revision],
                |row| {
                    let revision_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((revision_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(revision_json, created_at)| {
            let revision: GlobalConfigRevision = serde_json::from_str(&revision_json)
                .expect("global_config_revisions.revision_json round-trips");
            GlobalConfigRevisionRecord {
                revision,
                created_at,
            }
        }))
    }

    /// The derived "current global config" read: the highest `revision`
    /// ever recorded, not a separately stored value -- same reasoning as
    /// `load_current_project_intent_revision`. `resolve_project_config`
    /// always resolves against this one.
    pub fn load_current_global_config_revision(
        &self,
    ) -> rusqlite::Result<Option<GlobalConfigRevisionRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT revision_json, created_at FROM global_config_revisions \
                 ORDER BY revision DESC LIMIT 1",
                [],
                |row| {
                    let revision_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((revision_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(revision_json, created_at)| {
            let revision: GlobalConfigRevision = serde_json::from_str(&revision_json)
                .expect("global_config_revisions.revision_json round-trips");
            GlobalConfigRevisionRecord {
                revision,
                created_at,
            }
        }))
    }

    /// config.rs's write path for the *current-state* `project_config_patches`
    /// row. `ProjectConfigPatch` has no validating constructor of its own
    /// either (unlike `GlobalConfigRevision`'s save, which is gated by
    /// `validate_save_global_config_revision`), so it is trusted directly
    /// and upserted (`INSERT OR REPLACE`) -- a project has exactly one
    /// *current* patch, and a new one amending it is expected to replace the
    /// row, not conflict with it, same reasoning as
    /// `record_global_skill_binding`. Unlike `project_skill_bindings`,
    /// `ProjectConfigPatch` already carries its own `project_id` field, so
    /// there is no separate caller-supplied key half to thread through.
    pub fn record_project_config_patch(
        &mut self,
        patch: &ProjectConfigPatch,
    ) -> rusqlite::Result<String> {
        let patch_json = serde_json::to_string(patch).expect("ProjectConfigPatch is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT OR REPLACE INTO project_config_patches (project_id, patch_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![patch.project_id, patch_json, created_at],
        )?;
        Ok(created_at)
    }

    /// Read counterpart to `record_project_config_patch` -- the current
    /// snapshot for `project_id`, if one has ever been recorded.
    pub fn load_project_config_patch(
        &self,
        project_id: &str,
    ) -> rusqlite::Result<Option<ProjectConfigPatch>> {
        let patch_json: Option<String> = self
            .conn
            .query_row(
                "SELECT patch_json FROM project_config_patches WHERE project_id = ?1",
                rusqlite::params![project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(patch_json.map(|patch_json| {
            serde_json::from_str(&patch_json)
                .expect("project_config_patches.patch_json round-trips")
        }))
    }

    /// §5.12's write path: fixes a `DeliveryChain`'s `subject` for the rest
    /// of a run's delivery lifecycle. Like `record_attempt`, this is a fact
    /// recorded once, not a journaled event -- every subsequent
    /// `append_delivery_*` call mutates this row's rung columns, not a new
    /// row.
    pub fn start_delivery_chain(
        &mut self,
        run_id: &str,
        subject: &DeliverySubject,
    ) -> Result<DeliveryChainRecord, StartDeliveryChainError> {
        let subject_json =
            serde_json::to_string(subject).expect("DeliverySubject is serializable");
        let now = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO delivery_chains (run_id, subject_json, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![run_id, subject_json, now],
        )?;
        Ok(DeliveryChainRecord {
            run_id: run_id.to_string(),
            subject: subject.clone(),
            rehearsal: None,
            approval: None,
            delivery: None,
            tree_check: None,
            project_target_transition: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Read counterpart to `start_delivery_chain` -- looks up the recorded
    /// `delivery_chains` row for `run_id`, if any, deserializing whichever
    /// rung columns are non-NULL.
    pub fn load_delivery_chain(
        &self,
        run_id: &str,
    ) -> rusqlite::Result<Option<DeliveryChainRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT subject_json, rehearsal_json, approval_json, delivery_json, \
                        tree_check_json, project_target_transition_json, created_at, updated_at \
                 FROM delivery_chains WHERE run_id = ?1",
                rusqlite::params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        Ok(row.map(
            |(
                subject_json,
                rehearsal_json,
                approval_json,
                delivery_json,
                tree_check_json,
                project_target_transition_json,
                created_at,
                updated_at,
            )| DeliveryChainRecord {
                run_id: run_id.to_string(),
                subject: serde_json::from_str(&subject_json)
                    .expect("delivery_chains.subject_json round-trips"),
                rehearsal: rehearsal_json.map(|j| {
                    serde_json::from_str(&j).expect("delivery_chains.rehearsal_json round-trips")
                }),
                approval: approval_json.map(|j| {
                    serde_json::from_str(&j).expect("delivery_chains.approval_json round-trips")
                }),
                delivery: delivery_json.map(|j| {
                    serde_json::from_str(&j).expect("delivery_chains.delivery_json round-trips")
                }),
                tree_check: tree_check_json.map(|j| {
                    serde_json::from_str(&j).expect("delivery_chains.tree_check_json round-trips")
                }),
                project_target_transition: project_target_transition_json.map(|j| {
                    serde_json::from_str(&j).expect(
                        "delivery_chains.project_target_transition_json round-trips",
                    )
                }),
                created_at,
                updated_at,
            },
        ))
    }

    /// §5.12 rung 1: appends a `DeliveryRehearsalReceipt`. Unlike the four
    /// rungs below, `DeliveryChain::append_rehearsal` is infallible -- it
    /// has no prerequisite rung -- so there is no domain error to propagate
    /// here, only `NotFound` if `run_id` was never started.
    pub fn append_delivery_rehearsal(
        &mut self,
        run_id: &str,
        receipt: &DeliveryRehearsalReceipt,
    ) -> Result<DeliveryChainRecord, AppendDeliveryReceiptError> {
        let mut record = self
            .load_delivery_chain(run_id)?
            .ok_or(AppendDeliveryReceiptError::NotFound)?;
        let receipt_json =
            serde_json::to_string(receipt).expect("DeliveryRehearsalReceipt is serializable");
        let updated_at = Self::now_rfc3339();
        self.conn.execute(
            "UPDATE delivery_chains SET rehearsal_json = ?1, updated_at = ?2 WHERE run_id = ?3",
            rusqlite::params![receipt_json, updated_at, run_id],
        )?;
        record.rehearsal = Some(receipt.clone());
        record.updated_at = updated_at;
        Ok(record)
    }

    /// §5.12 rung 2: appends a `DeliveryApprovalReceipt`, rejecting via
    /// `DeliveryChainError::RehearsalRequiredBeforeApproval` if rung 1 is
    /// missing -- re-using `DeliveryChain::append_approval`'s own check
    /// rather than reimplementing it here.
    pub fn append_delivery_approval(
        &mut self,
        run_id: &str,
        receipt: &DeliveryApprovalReceipt,
    ) -> Result<DeliveryChainRecord, AppendDeliveryReceiptError> {
        let mut record = self
            .load_delivery_chain(run_id)?
            .ok_or(AppendDeliveryReceiptError::NotFound)?;
        record
            .rebuild()
            .append_approval(receipt.clone())
            .map_err(AppendDeliveryReceiptError::Chain)?;
        let receipt_json =
            serde_json::to_string(receipt).expect("DeliveryApprovalReceipt is serializable");
        let updated_at = Self::now_rfc3339();
        self.conn.execute(
            "UPDATE delivery_chains SET approval_json = ?1, updated_at = ?2 WHERE run_id = ?3",
            rusqlite::params![receipt_json, updated_at, run_id],
        )?;
        record.approval = Some(receipt.clone());
        record.updated_at = updated_at;
        Ok(record)
    }

    /// §5.12 rung 3: appends the `DeliveryReceipt` itself (the outcome of
    /// actually performing the delivery), rejecting out-of-order or
    /// repeated calls via `DeliveryChain::append_delivery`'s own checks. A
    /// `Failed` or `UnknownOutcome` receipt is still recorded permanently --
    /// see `append_delivery`'s doc comment in `delivery.rs` -- it just
    /// blocks `append_delivery_tree_check` from proceeding.
    pub fn append_delivery_delivery(
        &mut self,
        run_id: &str,
        receipt: &DeliveryReceipt,
    ) -> Result<DeliveryChainRecord, AppendDeliveryReceiptError> {
        let mut record = self
            .load_delivery_chain(run_id)?
            .ok_or(AppendDeliveryReceiptError::NotFound)?;
        record
            .rebuild()
            .append_delivery(receipt.clone())
            .map_err(AppendDeliveryReceiptError::Chain)?;
        let receipt_json =
            serde_json::to_string(receipt).expect("DeliveryReceipt is serializable");
        let updated_at = Self::now_rfc3339();
        self.conn.execute(
            "UPDATE delivery_chains SET delivery_json = ?1, updated_at = ?2 WHERE run_id = ?3",
            rusqlite::params![receipt_json, updated_at, run_id],
        )?;
        record.delivery = Some(receipt.clone());
        record.updated_at = updated_at;
        Ok(record)
    }

    /// §5.12 rung 4: appends a `DeliveredTreeCheckReceipt`, rejecting via
    /// `DeliveryChain::append_tree_check`'s own checks (delivery missing,
    /// delivery didn't succeed, or a tree check already recorded).
    pub fn append_delivery_tree_check(
        &mut self,
        run_id: &str,
        receipt: &DeliveredTreeCheckReceipt,
    ) -> Result<DeliveryChainRecord, AppendDeliveryReceiptError> {
        let mut record = self
            .load_delivery_chain(run_id)?
            .ok_or(AppendDeliveryReceiptError::NotFound)?;
        record
            .rebuild()
            .append_tree_check(receipt.clone())
            .map_err(AppendDeliveryReceiptError::Chain)?;
        let receipt_json =
            serde_json::to_string(receipt).expect("DeliveredTreeCheckReceipt is serializable");
        let updated_at = Self::now_rfc3339();
        self.conn.execute(
            "UPDATE delivery_chains SET tree_check_json = ?1, updated_at = ?2 WHERE run_id = ?3",
            rusqlite::params![receipt_json, updated_at, run_id],
        )?;
        record.tree_check = Some(receipt.clone());
        record.updated_at = updated_at;
        Ok(record)
    }

    /// §5.12 rung 5 (Greenfield-only): appends a
    /// `ProjectTargetTransitionReceipt`, rejecting via
    /// `DeliveryChain::append_project_target_transition`'s own checks
    /// (subject isn't Greenfield, tree check missing, or tree check didn't
    /// match).
    pub fn append_delivery_project_target_transition(
        &mut self,
        run_id: &str,
        receipt: &ProjectTargetTransitionReceipt,
    ) -> Result<DeliveryChainRecord, AppendDeliveryReceiptError> {
        let mut record = self
            .load_delivery_chain(run_id)?
            .ok_or(AppendDeliveryReceiptError::NotFound)?;
        record
            .rebuild()
            .append_project_target_transition(receipt.clone())
            .map_err(AppendDeliveryReceiptError::Chain)?;
        let receipt_json = serde_json::to_string(receipt)
            .expect("ProjectTargetTransitionReceipt is serializable");
        let updated_at = Self::now_rfc3339();
        self.conn.execute(
            "UPDATE delivery_chains SET project_target_transition_json = ?1, updated_at = ?2 \
             WHERE run_id = ?3",
            rusqlite::params![receipt_json, updated_at, run_id],
        )?;
        record.project_target_transition = Some(receipt.clone());
        record.updated_at = updated_at;
        Ok(record)
    }

    /// §5.8's first write path: issues a `CandidateCertificate` by composing
    /// an already-recorded readiness receipt (looked up by digest via
    /// `load_readiness`, the same record `readiness.record` produced)
    /// rather than requiring the caller to resupply the whole receipt
    /// blob -- callers only need to still be holding the digest they got
    /// back from `readiness.record`.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_candidate_certificate(
        &mut self,
        run_id: &str,
        contract_version: u32,
        candidate_commit: &str,
        candidate_tree_hash: &str,
        must_requirement_ids: &[RequirementId],
        verdicts: &[AuditVerdict],
        valid_receipt_ids: &HashSet<ReceiptId>,
        readiness_receipt_digest: &str,
        current_environment_fingerprint: &ReadinessFingerprint,
    ) -> Result<CandidateCertificateRecord, IssueCandidateCertificateError> {
        let readiness = self
            .load_readiness(readiness_receipt_digest)?
            .ok_or(IssueCandidateCertificateError::ReadinessNotFound)?;
        let certificate = certificate::issue_candidate_certificate(
            run_id,
            contract_version,
            candidate_commit,
            candidate_tree_hash,
            must_requirement_ids,
            verdicts,
            valid_receipt_ids,
            &readiness.receipt,
            current_environment_fingerprint,
        )
        .map_err(IssueCandidateCertificateError::Domain)?;

        let certificate_json =
            serde_json::to_string(&certificate).expect("CandidateCertificate is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO candidate_certificates (run_id, certificate_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![run_id, certificate_json, created_at],
        )?;

        Ok(CandidateCertificateRecord {
            certificate,
            created_at,
        })
    }

    /// Read counterpart to `issue_candidate_certificate` -- looks up the
    /// recorded `candidate_certificates` row for `run_id`, if any.
    pub fn load_candidate_certificate(
        &self,
        run_id: &str,
    ) -> rusqlite::Result<Option<CandidateCertificateRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT certificate_json, created_at FROM candidate_certificates \
                 WHERE run_id = ?1",
                rusqlite::params![run_id],
                |row| {
                    let certificate_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((certificate_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(certificate_json, created_at)| {
            let certificate: CandidateCertificate = serde_json::from_str(&certificate_json)
                .expect("candidate_certificates.certificate_json round-trips");
            CandidateCertificateRecord {
                certificate,
                created_at,
            }
        }))
    }

    /// §5.8's second write path: issues a `CompletionCertificate` by
    /// composing the already-recorded `CandidateCertificate` for this run
    /// and the already-recorded `DeliveryChainRecord` (rebuilt into a live
    /// `DeliveryChain`) -- the caller supplies only what neither of those
    /// records already carries: the delivered tree's hash and a reference
    /// to the human approval decision.
    pub fn issue_completion_certificate(
        &mut self,
        run_id: &str,
        delivery_tree_hash: &str,
        user_approval_decision_ref: &str,
    ) -> Result<CompletionCertificateRecord, IssueCompletionCertificateError> {
        let candidate = self
            .load_candidate_certificate(run_id)?
            .ok_or(IssueCompletionCertificateError::CandidateNotFound)?;
        let delivery_chain = self
            .load_delivery_chain(run_id)?
            .ok_or(IssueCompletionCertificateError::DeliveryChainNotFound)?;
        let certificate = certificate::issue_completion_certificate(
            &candidate.certificate,
            &delivery_chain.rebuild(),
            delivery_tree_hash,
            user_approval_decision_ref,
        )
        .map_err(IssueCompletionCertificateError::Domain)?;

        let certificate_json =
            serde_json::to_string(&certificate).expect("CompletionCertificate is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO completion_certificates (run_id, certificate_json, created_at) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![run_id, certificate_json, created_at],
        )?;

        Ok(CompletionCertificateRecord {
            run_id: run_id.to_string(),
            certificate,
            created_at,
        })
    }

    /// Read counterpart to `issue_completion_certificate` -- looks up the
    /// recorded `completion_certificates` row for `run_id`, if any.
    pub fn load_completion_certificate(
        &self,
        run_id: &str,
    ) -> rusqlite::Result<Option<CompletionCertificateRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT certificate_json, created_at FROM completion_certificates \
                 WHERE run_id = ?1",
                rusqlite::params![run_id],
                |row| {
                    let certificate_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((certificate_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(certificate_json, created_at)| {
            let certificate: CompletionCertificate = serde_json::from_str(&certificate_json)
                .expect("completion_certificates.certificate_json round-trips");
            CompletionCertificateRecord {
                run_id: run_id.to_string(),
                certificate,
                created_at,
            }
        }))
    }

    /// §10.3's write path: binds a `FrozenPlaybook` to `run_id` for the
    /// whole Run lifetime. Like `record_evidence`/`record_readiness`, this
    /// is a fact recorded once, not a journaled event -- currency
    /// (`FrozenPlaybook::is_current_against`) is a query-time property this
    /// method doesn't decide, so there is nothing to validate beyond what
    /// the primary key already enforces.
    pub fn bind_playbook(
        &mut self,
        run_id: &str,
        playbook: &FrozenPlaybook,
    ) -> Result<FrozenPlaybookRecord, BindPlaybookError> {
        let playbook_json = serde_json::to_string(playbook).expect("FrozenPlaybook is serializable");
        let created_at = Self::now_rfc3339();
        self.conn.execute(
            "INSERT INTO frozen_playbooks (run_id, playbook_json, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![run_id, playbook_json, created_at],
        )?;

        Ok(FrozenPlaybookRecord {
            run_id: run_id.to_string(),
            playbook: playbook.clone(),
            created_at,
        })
    }

    /// Read counterpart to `bind_playbook` -- looks up the bound
    /// `frozen_playbooks` row for `run_id`, if any.
    pub fn load_playbook(&self, run_id: &str) -> rusqlite::Result<Option<FrozenPlaybookRecord>> {
        let row = self
            .conn
            .query_row(
                "SELECT playbook_json, created_at FROM frozen_playbooks WHERE run_id = ?1",
                rusqlite::params![run_id],
                |row| {
                    let playbook_json: String = row.get(0)?;
                    let created_at: String = row.get(1)?;
                    Ok((playbook_json, created_at))
                },
            )
            .optional()?;
        Ok(row.map(|(playbook_json, created_at)| {
            let playbook: FrozenPlaybook = serde_json::from_str(&playbook_json)
                .expect("frozen_playbooks.playbook_json round-trips");
            FrozenPlaybookRecord {
                run_id: run_id.to_string(),
                playbook,
                created_at,
            }
        }))
    }

    /// Loads the current projected (revision, NodeStatus) for a node
    /// aggregate, or `None` if no event has ever been journaled for it --
    /// in which case callers should treat the aggregate as
    /// `NodeStatus::Pending` at revision 0. Mirrors `load_run_state`.
    pub fn load_node_status(&self, aggregate_id: &str) -> rusqlite::Result<Option<(u64, NodeStatus)>> {
        self.conn
            .query_row(
                "SELECT revision, status_json FROM node_projections WHERE aggregate_id = ?1",
                params![aggregate_id],
                |row| {
                    let revision: i64 = row.get(0)?;
                    let status_json: String = row.get(1)?;
                    Ok((revision, status_json))
                },
            )
            .optional()?
            .map(|(revision, status_json)| {
                let status: NodeStatus = serde_json::from_str(&status_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                Ok((revision as u64, status))
            })
            .transpose()
    }

    /// Applies `event` to the current status of `aggregate_id`, and — in a
    /// single transaction — appends the event and updates the projection.
    /// Mirrors `append_run_event`: defaults to `NodeStatus::Pending` when
    /// nothing has been journaled yet, same "default state, not `Option`"
    /// shape as Run/Project (§6.3's nominal path starts a node at Pending
    /// with no distinct "not yet created" state -- see `node.rs`'s own
    /// tests). On an illegal transition, nothing is written.
    pub fn append_node_event(
        &mut self,
        aggregate_id: &str,
        event: NodeEvent,
    ) -> Result<AppendedNodeEvent, NodeAppendError> {
        let tx = self.conn.transaction()?;

        let (revision, current_status) = {
            let loaded = tx
                .query_row(
                    "SELECT revision, status_json FROM node_projections WHERE aggregate_id = ?1",
                    params![aggregate_id],
                    |row| {
                        let revision: i64 = row.get(0)?;
                        let status_json: String = row.get(1)?;
                        Ok((revision, status_json))
                    },
                )
                .optional()?;
            match loaded {
                Some((revision, status_json)) => {
                    let status: NodeStatus = serde_json::from_str(&status_json).expect(
                        "node_projections.status_json is only ever written by this module as valid NodeStatus JSON",
                    );
                    (revision as u64, status)
                }
                None => (0, NodeStatus::Pending),
            }
        };

        let next_status =
            node::apply(current_status, event).map_err(NodeAppendError::Transition)?;
        let next_revision = revision + 1;
        let event_id = uuid::Uuid::new_v4().to_string();
        let event_type = node_event_type_name(&event);
        let payload = serde_json::to_string(&event).expect("NodeEvent always serializes");
        let recorded_at = Self::now_rfc3339();
        let status_json =
            serde_json::to_string(&next_status).expect("NodeStatus always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Node', ?3, ?4, ?5, ?6)",
            params![event_id, aggregate_id, next_revision as i64, event_type, payload, recorded_at],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO node_projections (aggregate_id, revision, status_json)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(aggregate_id) DO UPDATE SET
                revision = excluded.revision,
                status_json = excluded.status_json",
            params![aggregate_id, next_revision as i64, status_json],
        )?;

        tx.commit()?;
        Ok(AppendedNodeEvent {
            seq,
            event_id,
            revision: next_revision,
            event_type,
            occurred_at: recorded_at,
            state: next_status,
        })
    }
}

fn node_event_type_name(event: &NodeEvent) -> &'static str {
    match event {
        NodeEvent::BecomeReady => "BecomeReady",
        NodeEvent::StartProducing => "StartProducing",
        NodeEvent::ClaimSubmitted => "ClaimSubmitted",
        NodeEvent::StartVerifying => "StartVerifying",
        NodeEvent::VerificationPassed => "VerificationPassed",
        NodeEvent::VerificationFailedRepairable => "VerificationFailedRepairable",
        NodeEvent::VerificationInconclusive => "VerificationInconclusive",
        NodeEvent::VerificationProtocolViolation => "VerificationProtocolViolation",
        NodeEvent::EvaluationAccepted => "EvaluationAccepted",
        NodeEvent::EvaluationNeedsRepair => "EvaluationNeedsRepair",
        NodeEvent::EvaluationNeedsReplan => "EvaluationNeedsReplan",
        NodeEvent::EvaluationNeedsHuman => "EvaluationNeedsHuman",
        NodeEvent::RepairReady => "RepairReady",
        NodeEvent::RetryAfterInconclusive => "RetryAfterInconclusive",
        NodeEvent::IsolateAndRetry => "IsolateAndRetry",
        NodeEvent::HumanDecisionRecorded => "HumanDecisionRecorded",
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

/// The single-transaction body of `EventStore::create_project_from_target`:
/// claims `target_id` (guarded so a second caller racing for the same
/// target gets `TargetAlreadyConsumed` instead of a double-spend) and then
/// writes `project.created` (revision 1) followed by 4x `AdvanceNominal`
/// and one `IntentUnresolved` (revisions 2-6), mirroring `create_project`'s
/// revision-1 shape and `append_project_event`'s upsert shape for the rest
/// — see both of their doc comments. A free function, not a method, so it
/// can take `&mut Connection` directly rather than fight the borrow
/// checker over `&mut self` while `create_project_from_target` still holds
/// other data borrowed from `self`.
fn create_project_from_target_tx(
    conn: &mut Connection,
    project_id: &str,
    target_id: &str,
    display_name: &str,
    identity_json: &str,
) -> Result<AppendedProjectEvent, CreateFromTargetError> {
    let tx = conn.transaction()?;

    let claimed = tx.execute(
        "UPDATE project_targets SET consumed_by_project_id = ?1
         WHERE target_id = ?2 AND consumed_by_project_id IS NULL",
        params![project_id, target_id],
    )?;
    if claimed == 0 {
        return Err(CreateFromTargetError::TargetAlreadyConsumed);
    }

    let mut state = ProjectState::new();
    let mut event_id = uuid::Uuid::new_v4().to_string();
    let mut event_type: &'static str = "project.created";
    let mut occurred_at = EventStore::now_rfc3339();
    let state_json = serde_json::to_string(&state).expect("ProjectState always serializes");

    tx.execute(
        "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
         VALUES (?1, ?2, 'Project', 1, ?3, ?4, ?5)",
        params![event_id, project_id, event_type, identity_json, occurred_at],
    )?;
    let mut seq = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO project_projections (aggregate_id, revision, lifecycle, phase, hold, state_json, display_name, identity_json)
         VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            project_id,
            format!("{:?}", state.lifecycle),
            format!("{:?}", state.phase),
            format!("{:?}", state.hold),
            state_json,
            display_name,
            identity_json,
        ],
    )?;
    let mut revision: u64 = 1;

    let steps: [ProjectEvent; 5] = [
        ProjectEvent::AdvanceNominal,
        ProjectEvent::AdvanceNominal,
        ProjectEvent::AdvanceNominal,
        ProjectEvent::AdvanceNominal,
        ProjectEvent::IntentUnresolved,
    ];
    for step in steps {
        state = project::apply(state, step).expect(
            "4x AdvanceNominal + IntentUnresolved is always legal immediately after project.created",
        );
        revision += 1;
        event_id = uuid::Uuid::new_v4().to_string();
        event_type = project_event_type_name(&step);
        occurred_at = EventStore::now_rfc3339();
        let payload = serde_json::to_string(&step).expect("ProjectEvent always serializes");
        let state_json = serde_json::to_string(&state).expect("ProjectState always serializes");

        tx.execute(
            "INSERT INTO events (event_id, aggregate_id, aggregate_type, revision, event_type, payload, recorded_at)
             VALUES (?1, ?2, 'Project', ?3, ?4, ?5, ?6)",
            params![event_id, project_id, revision as i64, event_type, payload, occurred_at],
        )?;
        seq = tx.last_insert_rowid();
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
                project_id,
                revision as i64,
                format!("{:?}", state.lifecycle),
                format!("{:?}", state.phase),
                format!("{:?}", state.hold),
                state_json
            ],
        )?;
    }

    tx.commit()?;
    Ok(AppendedProjectEvent {
        seq,
        event_id,
        revision,
        event_type,
        occurred_at,
        state,
    })
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

fn execution_queue_event_type_name(event: &ExecutionQueueEvent) -> &'static str {
    match event {
        ExecutionQueueEvent::Enqueued { .. } => "Enqueued",
        ExecutionQueueEvent::LeaseAcquired { .. } => "LeaseAcquired",
        ExecutionQueueEvent::LeaseReleasedViaSafePark { .. } => "LeaseReleasedViaSafePark",
        ExecutionQueueEvent::Cancelled { .. } => "Cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::run::RunPhase;
    use autome_domain::skill::{
        BindingState, InvocationPolicy, ProjectSkillBindingMode, SkillDigest,
    };
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    fn temp_db_path(label: &str) -> String {
        std::env::temp_dir()
            .join(format!(
                "automed-store-test-{label}-{}.sqlite3",
                uuid::Uuid::new_v4()
            ))
            .to_string_lossy()
            .into_owned()
    }

    fn sample_project_identity(id: &str) -> ProjectIdentity {
        ProjectIdentity::new(
            id,
            &format!("Display {id}"),
            autome_domain::project::ProjectKind::ExistingRepository,
            autome_domain::project::ProjectLocator::ExistingRepository {
                repository_identity: format!("repo-{id}"),
            },
            &format!("/tmp/project-home-{id}"),
        )
        .unwrap()
    }

    /// §8.2 target-registration tests below each need their own isolated
    /// data root -- never the bare shared `std::env::temp_dir()` that
    /// `temp_db_path` points at -- because `create_project_from_target`
    /// lazily creates a shared `projects/` directory directly under the
    /// db's parent the first time any test calls it. With `cargo test`'s
    /// default thread-per-test parallelism, two tests racing to create
    /// that *same* shared directory for the first time is a real,
    /// observable flake (one loses the `mkdir` race). A fresh, uniquely
    /// named directory per test removes the shared resource entirely.
    fn temp_data_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "automed-store-test-root-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// A `TargetIdentityProbe` fixture with the three git-derived
    /// judgment inputs set directly, sidestepping a real `git` subprocess
    /// (that round trip is `target_probe.rs`'s own test responsibility).
    /// `canonical_path` is a freshly created, real directory -- needed
    /// because `create_project_from_target`'s `NewProduct` branch calls
    /// `symlink_metadata` on a path joined under it.
    fn sample_probe(
        label: &str,
        is_git_repo: bool,
        head_resolvable: bool,
        worktree_clean: bool,
    ) -> crate::target_probe::TargetIdentityProbe {
        let dir = std::env::temp_dir().join(format!(
            "automed-store-test-probe-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        crate::target_probe::TargetIdentityProbe {
            canonical_path: dir,
            dev: 0,
            ino: 0,
            is_git_repo,
            git_common_dir: None,
            object_format: None,
            config_digest_sha256_hex: None,
            head_commit: None,
            head_resolvable,
            worktree_clean,
        }
    }

    /// Simulates a real v3 database (`migrate_v1`..`migrate_v3` already
    /// applied, `user_version = 3`, one project row already journaled with
    /// the v3-era `display_name`/`identity_json` columns populated) built
    /// by calling the migration steps directly rather than hand-writing
    /// v3's DDL a second time -- `EventStore::open` must then run only
    /// `migrate_v4` and leave the pre-existing row exactly as it was.
    #[test]
    fn opening_a_v3_database_adds_project_targets_without_disturbing_existing_rows() {
        let root = temp_data_root("v3-migration");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        {
            let conn = Connection::open(&db_path).unwrap();
            migrate_v1(&conn).unwrap();
            migrate_v2(&conn).unwrap();
            migrate_v3(&conn).unwrap();
            conn.execute_batch("PRAGMA user_version = 3").unwrap();
            conn.execute(
                "INSERT INTO project_projections (aggregate_id, revision, lifecycle, phase, hold, state_json, display_name, identity_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "project-v3",
                    1,
                    "Active",
                    "Registered",
                    "None",
                    r#"{"lifecycle":"Active","phase":"Registered","hold":"None","revision":1}"#,
                    "V3 Project",
                    r#"{"id":"project-v3","display_name":"V3 Project","kind":"ExistingRepository","locator":{"ExistingRepository":{"repository_identity":"repo-v3"}},"project_home":"/tmp/project-home-v3"}"#,
                ],
            )
            .unwrap();
        }

        let mut store = EventStore::open(&db_path).unwrap();
        let version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);

        let summaries = store.list_project_summaries().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "project-v3");
        assert_eq!(summaries[0].display_name.as_deref(), Some("V3 Project"));

        // `project_targets` exists and is usable, proving migrate_v4 ran
        // on top of the simulated v3 database rather than being skipped.
        let probe = sample_probe("v3-migration", true, true, true);
        let inspection = probe.to_inspection(false);
        let target_id = store
            .register_target(ProjectKind::ExistingRepository, &probe, &inspection)
            .unwrap();
        assert!(store.load_target(&target_id).unwrap().is_some());

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.2: `create_project_from_target` claims the target row
    /// (`consumed_by_project_id` set to the new project's id); a second
    /// call against the same already-consumed `target_id` must be
    /// rejected outright rather than silently minting a second project
    /// from the same registration.
    #[test]
    fn create_from_target_consumes_the_target_and_rejects_reuse() {
        let root = temp_data_root("consume-target");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let probe = sample_probe("consume-target", true, true, true);
        let inspection = probe.to_inspection(false);
        let target_id = store
            .register_target(ProjectKind::ExistingRepository, &probe, &inspection)
            .unwrap();

        let created = store
            .create_project_from_target(&target_id, "Consume Target Project", true, None)
            .unwrap();

        let record = store.load_target(&target_id).unwrap().unwrap();
        assert_eq!(
            record.consumed_by_project_id.as_deref(),
            Some(created.project_id.as_str())
        );

        let second =
            store.create_project_from_target(&target_id, "Second Attempt Project", true, None);
        assert!(
            matches!(second, Err(CreateFromTargetError::TargetAlreadyConsumed)),
            "{second:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// Builds a throwaway real Git repository (via the real `git` binary)
    /// with one commit, to act as `source_repo` for
    /// `create_disposable_clone_for_run` tests below. Mirrors
    /// `workspace.rs`'s own `fixture_source_repo` test helper exactly,
    /// since this is exercising the same `git` round trip one layer up.
    fn fixture_source_repo(root: &std::path::Path) -> (std::path::PathBuf, String) {
        let dir = root.join("source");
        std::fs::create_dir_all(&dir).unwrap();
        let path_str = dir.to_string_lossy().into_owned();
        std::process::Command::new("git")
            .args(["-C", &path_str, "init", "--quiet"])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", &path_str, "config", "user.email", "test@example.com"])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", &path_str, "config", "user.name", "Test"])
            .status()
            .unwrap();
        std::fs::write(dir.join("README.md"), b"hello\n").unwrap();
        std::process::Command::new("git")
            .args(["-C", &path_str, "add", "README.md"])
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", &path_str, "commit", "--quiet", "-m", "initial"])
            .status()
            .unwrap();
        let head = std::process::Command::new("git")
            .args(["-C", &path_str, "rev-parse", "HEAD"])
            .output()
            .unwrap();
        let head_commit = String::from_utf8_lossy(&head.stdout).trim().to_string();
        (dir, head_commit)
    }

    /// §8.1: `create_disposable_clone_for_run` builds a real clone on disk
    /// via `workspace::create_disposable_clone` and records exactly what
    /// landed there as one `run_workspaces` row, readable back via
    /// `load_disposable_clone_for_run`.
    #[test]
    fn create_disposable_clone_for_run_records_what_workspace_created_on_disk() {
        let root = temp_data_root("disposable-clone");
        let (source_repo, expected_head) = fixture_source_repo(&root);
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(
            store
                .load_disposable_clone_for_run("task-1", "run-1")
                .unwrap()
                .is_none()
        );

        let record = store
            .create_disposable_clone_for_run("task-1", "run-1", &source_repo)
            .unwrap();
        assert_eq!(record.task_id, "task-1");
        assert_eq!(record.run_id, "run-1");
        assert_eq!(record.head_commit, expected_head);
        assert!(std::path::Path::new(&record.repo_path).join("README.md").is_file());

        let loaded = store
            .load_disposable_clone_for_run("task-1", "run-1")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call for the same `(task_id, run_id)` must fail rather than
    /// silently reusing or overwriting the first clone -- mirrors
    /// `workspace.rs`'s own `refuses_to_reuse_an_existing_run_directory`,
    /// one layer up through the store.
    #[test]
    fn create_disposable_clone_for_run_refuses_to_reuse_an_existing_run_id() {
        let root = temp_data_root("disposable-clone-reuse");
        let (source_repo, _) = fixture_source_repo(&root);
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        store
            .create_disposable_clone_for_run("task-1", "run-1", &source_repo)
            .unwrap();
        let err = store
            .create_disposable_clone_for_run("task-1", "run-1", &source_repo)
            .unwrap_err();
        assert!(
            matches!(err, CreateDisposableCloneError::Workspace(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A minimal well-formed `Attempt`/`AttemptPermissionProfile` pair,
    /// mirroring `attempt.rs`'s own `base_attempt`/`base_profile` test
    /// helpers exactly (same field values), so a test here that expects
    /// `record_attempt` to accept it is exercising the same "valid input"
    /// shape already proven valid one layer down.
    fn fixture_attempt_and_profile(
        attempt_id: &str,
    ) -> (Attempt, AttemptPermissionProfile) {
        let attempt = Attempt {
            id: attempt::AttemptId(attempt_id.to_string()),
            loop_step_id: attempt::LoopStepId("step-1".into()),
            node_id: None,
            purpose: attempt::AttemptPurpose::Execution,
            spec_binding: attempt::SpecBinding::Execution(attempt::SpecHash("exec-hash".into())),
            agent_execution_profile_hash: "hash-agent".into(),
            permission_profile_id: attempt::PermissionProfileId("perm-1".into()),
            harness_id: "claude-code".into(),
            model_selection_identity_ref: attempt::Ref("model-1".into()),
            qualification_receipt_ref: attempt::Ref("qual-1".into()),
            input_commit: "deadbeef".into(),
            input_tree_hash: "treehash".into(),
            skill_projection_fingerprint: "skillfp".into(),
            provider_session_id: "session-1".into(),
        };
        let profile = AttemptPermissionProfile {
            id: attempt::PermissionProfileId("perm-1".into()),
            loop_step_id: attempt::LoopStepId("step-1".into()),
            node_id: None,
            adapter_id: "claude-code".into(),
            installation_id: "install-1".into(),
            subject_scope_hash: "scope".into(),
            skill_set_snapshot_hash: "skillset".into(),
            tool_surface: attempt::ToolSurface {
                provider_available_tools: vec!["Read".into(), "Bash".into()],
                provider_allowed_tools: vec!["Read".into()],
                provider_denied_tools: vec!["Bash".into()],
                autome_control_tools: vec![],
                dynamic_tool_or_mcp_allowlist: vec![],
            },
            filesystem_policy: attempt::FilesystemPolicy {
                read_roots: vec!["/project".into()],
                write_roots: vec!["/workdir".into()],
                deny_roots: vec![],
                nofollow: true,
            },
            command_policy: attempt::CommandPolicy::default(),
            network_policy: attempt::NetworkPolicy {
                mode: attempt::NetworkMode::Denied,
                allowed_brokers: vec![],
                allowed_destinations: vec![],
            },
            sandbox_policy: attempt::SandboxPolicy {
                mechanism: "seatbelt".into(),
                required_capabilities: vec![],
                fail_closed: true,
            },
            secret_policy_hash: "secret".into(),
            safety_policy_hash: "safety".into(),
            profile_hash: "profile".into(),
        };
        (attempt, profile)
    }

    /// A well-formed `record_attempt` call lands one `attempts` row,
    /// readable back via `load_attempt`.
    #[test]
    fn record_attempt_records_a_well_formed_attempt_and_reads_it_back() {
        let root = temp_data_root("record-attempt");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let (attempt, profile) = fixture_attempt_and_profile("attempt-1");

        assert!(store.load_attempt("attempt-1").unwrap().is_none());

        let record = store.record_attempt("run-1", &attempt, &profile).unwrap();
        assert_eq!(record.run_id, "run-1");
        assert_eq!(record.attempt, attempt);
        assert_eq!(record.permission_profile, profile);

        let loaded = store.load_attempt("attempt-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call reusing the same `attempt_id` must fail rather than
    /// silently overwriting what the step was actually permitted to do.
    #[test]
    fn record_attempt_refuses_to_reuse_an_existing_attempt_id() {
        let root = temp_data_root("record-attempt-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let (attempt, profile) = fixture_attempt_and_profile("attempt-1");

        store.record_attempt("run-1", &attempt, &profile).unwrap();
        let err = store
            .record_attempt("run-1", &attempt, &profile)
            .unwrap_err();
        assert!(matches!(err, RecordAttemptError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.6: a `Planning`-purpose Attempt paired with a profile that grants
    /// filesystem writes must be refused before anything is written -- the
    /// same invariant `attempt.rs::validate_planning_attempt_is_read_only`
    /// already proves, re-checked here at the store boundary.
    #[test]
    fn record_attempt_refuses_a_planning_attempt_with_write_roots() {
        let root = temp_data_root("record-attempt-planning-write");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let (mut attempt, profile) = fixture_attempt_and_profile("attempt-1");
        attempt.purpose = attempt::AttemptPurpose::Planning;
        attempt.spec_binding = attempt::SpecBinding::Planning(attempt::SpecHash("plan-hash".into()));

        let err = store
            .record_attempt("run-1", &attempt, &profile)
            .unwrap_err();
        assert!(matches!(err, RecordAttemptError::PlanningWrite(_)), "{err:?}");
        assert!(store.load_attempt("attempt-1").unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.6: a profile whose `provider_allowed_tools` exceeds
    /// `provider_available_tools`, or overlaps `provider_denied_tools`,
    /// must be refused -- the same invariant
    /// `AttemptPermissionProfile::validate` already proves.
    #[test]
    fn record_attempt_refuses_a_self_contradictory_permission_profile() {
        let root = temp_data_root("record-attempt-bad-profile");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let (attempt, mut profile) = fixture_attempt_and_profile("attempt-1");
        profile
            .tool_surface
            .provider_allowed_tools
            .push("Write".into());

        let err = store
            .record_attempt("run-1", &attempt, &profile)
            .unwrap_err();
        assert!(
            matches!(err, RecordAttemptError::PermissionViolations(_)),
            "{err:?}"
        );
        assert!(store.load_attempt("attempt-1").unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_active_keychain_credential() -> CredentialRecord {
        CredentialRecord {
            provider: "anthropic".into(),
            auth_mode: autome_domain::credential::AuthMode::ApiKey,
            storage_kind: autome_domain::credential::StorageKind::AutomeManagedKeychainItem,
            storage_location: autome_domain::credential::StorageLocation::Keychain(
                autome_domain::credential::KeychainItemIdentity {
                    service: "com.autome.credentials".into(),
                    account: "anthropic-default".into(),
                },
            ),
            created_at: "2026-09-14T00:00:00Z".into(),
            rotated_at: None,
            revoked_at: None,
            status: autome_domain::credential::CredentialStatus::Active,
        }
    }

    /// A well-formed `record_credential` call lands one `credentials` row,
    /// readable back via `load_credential`.
    #[test]
    fn record_credential_records_a_well_formed_record_and_reads_it_back() {
        let root = temp_data_root("record-credential");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let record = fixture_active_keychain_credential();

        assert!(store.load_credential("cred-1").unwrap().is_none());

        let row = store.record_credential("cred-1", &record).unwrap();
        assert_eq!(row.credential_ref, "cred-1");
        assert_eq!(row.record, record);

        let loaded = store.load_credential("cred-1").unwrap().unwrap();
        assert_eq!(loaded, row);

        std::fs::remove_dir_all(&root).ok();
    }

    /// Unlike `record_attempt`, a second `record_credential` call reusing
    /// the same `credential_ref` must *overwrite* the snapshot in place
    /// (rotation/revocation update the one record), not be refused.
    #[test]
    fn record_credential_upserts_rather_than_refusing_a_reused_credential_ref() {
        let root = temp_data_root("record-credential-upsert");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let record = fixture_active_keychain_credential();
        store.record_credential("cred-1", &record).unwrap();

        let mut rotated = record.clone();
        rotated.status = autome_domain::credential::CredentialStatus::Rotated;
        rotated.rotated_at = Some("2026-09-14T02:00:00Z".into());
        store.record_credential("cred-1", &rotated).unwrap();

        let loaded = store.load_credential("cred-1").unwrap().unwrap();
        assert_eq!(loaded.record, rotated);

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3: a `Revoked` status without `revoked_at` must be refused
    /// before anything is written, mirroring `CredentialRecord::validate_shape`.
    #[test]
    fn record_credential_refuses_a_shape_invalid_record() {
        let root = temp_data_root("record-credential-shape");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let mut record = fixture_active_keychain_credential();
        record.status = autome_domain::credential::CredentialStatus::Revoked;

        let err = store.record_credential("cred-1", &record).unwrap_err();
        assert!(
            matches!(
                err,
                RecordCredentialError::Shape(ref errors)
                    if errors == &vec![autome_domain::credential::CredentialShapeError::RevokedStatusRequiresRevokedAt]
            ),
            "{err:?}"
        );
        assert!(store.load_credential("cred-1").unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3: `record_credential_receipt` re-validates server-side --
    /// an `UninstallRetentionDecision` without an operator must be refused
    /// and nothing appended to the log.
    #[test]
    fn record_credential_receipt_refuses_an_uninstall_decision_without_an_operator() {
        let root = temp_data_root("record-credential-receipt-no-operator");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = store
            .record_credential_receipt(
                "cred-1",
                CredentialEvent::UninstallRetentionDecision { retained: true },
                "2026-09-14T00:00:00Z",
                None,
            )
            .unwrap_err();
        assert!(matches!(err, RecordCredentialReceiptError::Receipt(_)), "{err:?}");
        assert!(store.list_credential_receipts("cred-1").unwrap().is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3: the receipt log is genuinely append-only -- multiple receipts
    /// for the same `credential_ref` all persist, in insertion order,
    /// unlike every reject-duplicate fact-record store above.
    #[test]
    fn record_credential_receipt_appends_every_receipt_in_order() {
        let root = temp_data_root("record-credential-receipt-append");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        store
            .record_credential_receipt("cred-1", CredentialEvent::Created, "2026-09-14T00:00:00Z", None)
            .unwrap();
        store
            .record_credential_receipt("cred-1", CredentialEvent::Rotated, "2026-09-14T01:00:00Z", None)
            .unwrap();
        store
            .record_credential_receipt(
                "cred-1",
                CredentialEvent::UninstallRetentionDecision { retained: false },
                "2026-09-14T02:00:00Z",
                Some("user-1"),
            )
            .unwrap();

        let receipts = store.list_credential_receipts("cred-1").unwrap();
        assert_eq!(receipts.len(), 3);
        assert_eq!(receipts[0].event, CredentialEvent::Created);
        assert_eq!(receipts[1].event, CredentialEvent::Rotated);
        assert_eq!(
            receipts[2].event,
            CredentialEvent::UninstallRetentionDecision { retained: false }
        );
        assert_eq!(receipts[2].operator, Some("user-1".to_string()));

        assert!(store.list_credential_receipts("cred-unknown").unwrap().is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[allow(clippy::too_many_arguments)]
    fn record_fixture_user_correction(
        store: &mut EventStore,
        receipt_digest: &str,
        execution_spec_frozen: bool,
        classification: CorrectionClassification,
        impact: CorrectionImpactFlags,
        disposition: CorrectionDisposition,
    ) -> Result<UserCorrectionRecord, RecordUserCorrectionError> {
        store.record_user_correction(
            "project-1",
            "task-1",
            "run-1",
            None,
            "planning-spec-hash-1",
            if execution_spec_frozen {
                Some("execution-spec-hash-1")
            } else {
                None
            },
            execution_spec_frozen,
            "raw-text-ref-1",
            vec![],
            "2026-09-14T00:00:00Z",
            "dannie",
            None,
            None,
            None,
            classification,
            impact,
            vec![],
            vec![],
            disposition,
            None,
            receipt_digest,
        )
    }

    /// A well-formed `record_user_correction` call lands one
    /// `user_correction_receipts` row, readable back via
    /// `load_user_correction`.
    #[test]
    fn record_user_correction_records_a_well_formed_receipt_and_reads_it_back() {
        let root = temp_data_root("record-user-correction");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(store.load_user_correction("UC-1").unwrap().is_none());

        let record = record_fixture_user_correction(
            &mut store,
            "UC-1",
            false,
            CorrectionClassification::PlanningRevision,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::NewPlanningRunSpec,
        )
        .unwrap();
        assert_eq!(record.receipt.receipt_digest, "UC-1");
        assert_eq!(record.receipt.run_id, "run-1");

        let loaded = store.load_user_correction("UC-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call reusing the same `receipt_digest` must fail rather
    /// than silently overwriting a prior correction's receipt.
    #[test]
    fn record_user_correction_refuses_to_reuse_an_existing_receipt_digest() {
        let root = temp_data_root("record-user-correction-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_user_correction(
            &mut store,
            "UC-1",
            false,
            CorrectionClassification::PlanningRevision,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::NewPlanningRunSpec,
        )
        .unwrap();
        let err = record_fixture_user_correction(
            &mut store,
            "UC-1",
            false,
            CorrectionClassification::PlanningRevision,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::NewPlanningRunSpec,
        )
        .unwrap_err();
        assert!(matches!(err, RecordUserCorrectionError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// `record_user_correction` re-runs `issue_user_correction_receipt`
    /// server-side, so a pre-freeze non-`PlanningRevision` classification is
    /// refused before anything is written -- same discipline as
    /// `record_attempt` re-validating shape/profile.
    #[test]
    fn record_user_correction_rejects_a_domain_rule_violation_without_writing_anything() {
        let root = temp_data_root("record-user-correction-domain-rejected");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = record_fixture_user_correction(
            &mut store,
            "UC-1",
            false,
            CorrectionClassification::GraphStrategy,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::ReplanProposal,
        )
        .unwrap_err();
        assert!(
            matches!(
                &err,
                RecordUserCorrectionError::Receipt(errors)
                    if errors.contains(&UserCorrectionError::PreFreezeMustBePlanningRevision)
            ),
            "{err:?}"
        );
        assert!(store.load_user_correction("UC-1").unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    fn record_fixture_planning_policy_restart(
        store: &mut EventStore,
        restart_digest: &str,
        old_planning_spec_hash: &str,
    ) -> Result<PlanningPolicyRestartRecord, RecordPlanningPolicyRestartError> {
        store.record_planning_policy_restart(
            "task-1",
            "run-1",
            old_planning_spec_hash,
            "new-planning-spec-hash",
            "trigger-rev-1",
            "config-rev-1",
            "skill-rev-1",
            "capability-rev-1",
            vec!["doc-attempt-1".into()],
            "approval-1",
            restart_digest,
        )
    }

    #[test]
    fn record_planning_policy_restart_records_a_well_formed_restart_and_reads_it_back() {
        let root = temp_data_root("record-planning-policy-restart-well-formed");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(
            store
                .load_planning_policy_restart("RESTART-1")
                .unwrap()
                .is_none()
        );

        let record =
            record_fixture_planning_policy_restart(&mut store, "RESTART-1", "old-hash").unwrap();
        assert_eq!(record.restart.restart_digest, "RESTART-1");

        let loaded = store
            .load_planning_policy_restart("RESTART-1")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_planning_policy_restart_refuses_to_reuse_an_existing_restart_digest() {
        let root = temp_data_root("record-planning-policy-restart-reuse-refused");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_planning_policy_restart(&mut store, "RESTART-1", "old-hash").unwrap();
        let err =
            record_fixture_planning_policy_restart(&mut store, "RESTART-1", "another-old-hash")
                .unwrap_err();
        assert!(
            matches!(err, RecordPlanningPolicyRestartError::Sql(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_planning_policy_restart_rejects_an_unchanged_spec_without_writing_anything() {
        let root = temp_data_root("record-planning-policy-restart-domain-rejected");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = record_fixture_planning_policy_restart(
            &mut store,
            "RESTART-1",
            "new-planning-spec-hash",
        )
        .unwrap_err();
        assert!(
            matches!(
                &err,
                RecordPlanningPolicyRestartError::Restart(errors)
                    if errors.contains(&PlanningPolicyRestartError::PlanningSpecUnchanged)
            ),
            "{err:?}"
        );
        assert!(
            store
                .load_planning_policy_restart("RESTART-1")
                .unwrap()
                .is_none()
        );

        std::fs::remove_dir_all(&root).ok();
    }

    fn record_fixture_run_policy_amendment(
        store: &mut EventStore,
        amendment_digest: &str,
        old_execution_spec_hash: &str,
    ) -> Result<RunPolicyAmendmentRecord, RecordRunPolicyAmendmentError> {
        store.record_run_policy_amendment(
            "task-1",
            "run-1",
            old_execution_spec_hash,
            "new-execution-spec-hash",
            "contract-hash-1",
            "graph-hash-1",
            "base-hash-1",
            "final_audit switched from claude-a to claude-b",
            vec!["attempt-1".into()],
            vec!["evidence-1".into()],
            vec!["audit-1".into()],
            vec!["candidate-1".into()],
            "approval-1",
            amendment_digest,
        )
    }

    #[test]
    fn record_run_policy_amendment_records_a_well_formed_amendment_and_reads_it_back() {
        let root = temp_data_root("record-run-policy-amendment-well-formed");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(
            store
                .load_run_policy_amendment("AMEND-1")
                .unwrap()
                .is_none()
        );

        let record =
            record_fixture_run_policy_amendment(&mut store, "AMEND-1", "old-hash").unwrap();
        assert_eq!(record.amendment.amendment_digest, "AMEND-1");

        let loaded = store.load_run_policy_amendment("AMEND-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_run_policy_amendment_refuses_to_reuse_an_existing_amendment_digest() {
        let root = temp_data_root("record-run-policy-amendment-reuse-refused");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_run_policy_amendment(&mut store, "AMEND-1", "old-hash").unwrap();
        let err = record_fixture_run_policy_amendment(&mut store, "AMEND-1", "another-old-hash")
            .unwrap_err();
        assert!(
            matches!(err, RecordRunPolicyAmendmentError::Sql(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_run_policy_amendment_rejects_an_unchanged_spec_without_writing_anything() {
        let root = temp_data_root("record-run-policy-amendment-domain-rejected");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = record_fixture_run_policy_amendment(
            &mut store,
            "AMEND-1",
            "new-execution-spec-hash",
        )
        .unwrap_err();
        assert!(
            matches!(
                &err,
                RecordRunPolicyAmendmentError::Amendment(errors)
                    if errors.contains(&RunPolicyAmendmentError::ExecutionSpecUnchanged)
            ),
            "{err:?}"
        );
        assert!(
            store
                .load_run_policy_amendment("AMEND-1")
                .unwrap()
                .is_none()
        );

        std::fs::remove_dir_all(&root).ok();
    }

    fn record_fixture_budget_grant(
        store: &mut EventStore,
        grant_digest: &str,
        added_limits: Vec<BudgetLimitGrant>,
    ) -> Result<BudgetGrantRecord, RecordBudgetGrantError> {
        store.record_budget_grant(
            "run-1",
            "budget-hash-1",
            added_limits,
            "extra retries needed after flaky environment",
            "dannie",
            None,
            grant_digest,
        )
    }

    #[test]
    fn record_budget_grant_records_a_well_formed_grant_and_reads_it_back() {
        let root = temp_data_root("record-budget-grant-well-formed");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(store.load_budget_grant("GRANT-1").unwrap().is_none());

        let record = record_fixture_budget_grant(
            &mut store,
            "GRANT-1",
            vec![BudgetLimitGrant::Soft(10)],
        )
        .unwrap();
        assert_eq!(record.receipt.grant_digest, "GRANT-1");

        let loaded = store.load_budget_grant("GRANT-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_budget_grant_refuses_to_reuse_an_existing_grant_digest() {
        let root = temp_data_root("record-budget-grant-reuse-refused");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_budget_grant(&mut store, "GRANT-1", vec![BudgetLimitGrant::Soft(10)])
            .unwrap();
        let err =
            record_fixture_budget_grant(&mut store, "GRANT-1", vec![BudgetLimitGrant::Hard(5)])
                .unwrap_err();
        assert!(matches!(err, RecordBudgetGrantError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_budget_grant_rejects_no_limits_added_without_writing_anything() {
        let root = temp_data_root("record-budget-grant-domain-rejected");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = record_fixture_budget_grant(&mut store, "GRANT-1", vec![]).unwrap_err();
        assert!(
            matches!(
                &err,
                RecordBudgetGrantError::Grant(errors)
                    if errors.contains(&BudgetGrantError::NoLimitsAdded)
            ),
            "{err:?}"
        );
        assert!(store.load_budget_grant("GRANT-1").unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[allow(clippy::too_many_arguments)]
    fn record_fixture_project_intent_revision(
        store: &mut EventStore,
        project_id: &str,
        revision: u32,
        approved_by: &str,
        supersedes: Option<u32>,
    ) -> Result<ProjectIntentRevisionRecord, RecordProjectIntentRevisionError> {
        store.record_project_intent_revision(
            project_id,
            revision,
            vec!["README.md".to_string()],
            approved_by,
            "2026-09-14T00:00:00Z",
            "Ship a local-first digital employee",
            vec!["solo developers".to_string()],
            vec!["never phone home".to_string()],
            vec!["no multi-tenant support".to_string()],
            vec![KeyDecision {
                id: "kd-1".into(),
                statement: "Use SQLite for the event journal".into(),
                rationale: "Local-first, single-user, no server dependency".into(),
                source_ref: "docs/plan.md#L42".into(),
            }],
            supersedes,
            "intent-hash-1",
        )
    }

    #[test]
    fn record_project_intent_revision_records_a_well_formed_revision_and_reads_it_back() {
        let root = temp_data_root("record-project-intent-revision-well-formed");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(
            store
                .load_project_intent_revision("project-1", 1)
                .unwrap()
                .is_none()
        );

        let record =
            record_fixture_project_intent_revision(&mut store, "project-1", 1, "dannie", None)
                .unwrap();
        assert_eq!(record.revision.revision, IntentRevision(1));

        let loaded = store
            .load_project_intent_revision("project-1", 1)
            .unwrap()
            .unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_project_intent_revision_refuses_to_reuse_an_existing_project_id_revision_pair() {
        let root = temp_data_root("record-project-intent-revision-reuse-refused");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_project_intent_revision(&mut store, "project-1", 1, "dannie", None)
            .unwrap();
        let err =
            record_fixture_project_intent_revision(&mut store, "project-1", 1, "someone-else", None)
                .unwrap_err();
        assert!(
            matches!(err, RecordProjectIntentRevisionError::Sql(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_project_intent_revision_rejects_missing_approved_by_without_writing_anything() {
        let root = temp_data_root("record-project-intent-revision-domain-rejected");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err =
            record_fixture_project_intent_revision(&mut store, "project-1", 1, "", None)
                .unwrap_err();
        assert!(
            matches!(
                &err,
                RecordProjectIntentRevisionError::Revision(errors)
                    if errors.contains(&ProjectIntentError::MissingApprovedBy)
            ),
            "{err:?}"
        );
        assert!(
            store
                .load_project_intent_revision("project-1", 1)
                .unwrap()
                .is_none()
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn load_current_project_intent_revision_returns_the_highest_revision() {
        let root = temp_data_root("load-current-project-intent-revision");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(
            store
                .load_current_project_intent_revision("project-1")
                .unwrap()
                .is_none()
        );

        record_fixture_project_intent_revision(&mut store, "project-1", 1, "dannie", None)
            .unwrap();
        record_fixture_project_intent_revision(
            &mut store,
            "project-1",
            2,
            "dannie",
            Some(1),
        )
        .unwrap();

        let current = store
            .load_current_project_intent_revision("project-1")
            .unwrap()
            .unwrap();
        assert_eq!(current.revision.revision, IntentRevision(2));

        std::fs::remove_dir_all(&root).ok();
    }

    fn record_fixture_project_intent_amendment(
        store: &mut EventStore,
        project_id: &str,
        from_revision: u32,
        amendment_hash: &str,
    ) -> Result<ProjectIntentAmendmentRecord, RecordProjectIntentAmendmentError> {
        store.record_project_intent_amendment(
            project_id,
            from_revision,
            Some("task-9"),
            "Added a non-goal: no team accounts in 2.0.0",
            vec!["task-9".to_string()],
            "decision-ref-1",
            amendment_hash,
        )
    }

    #[test]
    fn record_project_intent_amendment_records_a_well_formed_amendment_and_reads_it_back() {
        let root = temp_data_root("record-project-intent-amendment-well-formed");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_project_intent_revision(&mut store, "project-1", 1, "dannie", None)
            .unwrap();
        assert!(
            store
                .load_project_intent_amendment("AMEND-1")
                .unwrap()
                .is_none()
        );

        let record =
            record_fixture_project_intent_amendment(&mut store, "project-1", 1, "AMEND-1")
                .unwrap();
        assert_eq!(record.amendment.to_revision, IntentRevision(2));

        let loaded = store
            .load_project_intent_amendment("AMEND-1")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_project_intent_amendment_refuses_to_reuse_an_existing_amendment_hash() {
        let root = temp_data_root("record-project-intent-amendment-reuse-refused");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_project_intent_revision(&mut store, "project-1", 1, "dannie", None)
            .unwrap();
        record_fixture_project_intent_amendment(&mut store, "project-1", 1, "AMEND-1").unwrap();
        let err =
            record_fixture_project_intent_amendment(&mut store, "project-1", 1, "AMEND-1")
                .unwrap_err();
        assert!(
            matches!(err, RecordProjectIntentAmendmentError::Sql(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_project_intent_amendment_is_no_current_revision_when_none_recorded() {
        let root = temp_data_root("record-project-intent-amendment-no-current-revision");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err =
            record_fixture_project_intent_amendment(&mut store, "project-1", 1, "AMEND-1")
                .unwrap_err();
        assert!(
            matches!(err, RecordProjectIntentAmendmentError::NoCurrentRevision),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_project_intent_amendment_rejects_stale_from_revision_without_writing_anything() {
        let root = temp_data_root("record-project-intent-amendment-domain-rejected");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_project_intent_revision(&mut store, "project-1", 3, "dannie", None)
            .unwrap();
        let err =
            record_fixture_project_intent_amendment(&mut store, "project-1", 1, "AMEND-1")
                .unwrap_err();
        assert!(
            matches!(
                &err,
                RecordProjectIntentAmendmentError::Amendment(
                    ProjectIntentAmendmentError::FromRevisionMismatch { expected, actual }
                ) if *expected == IntentRevision(3) && *actual == IntentRevision(1)
            ),
            "{err:?}"
        );
        assert!(
            store
                .load_project_intent_amendment("AMEND-1")
                .unwrap()
                .is_none()
        );

        std::fs::remove_dir_all(&root).ok();
    }

    fn record_fixture_project_initialization_receipt(
        store: &mut EventStore,
        receipt_digest: &str,
        result: InitializationResult,
        issues: Vec<String>,
    ) -> Result<ProjectInitializationReceiptRecord, RecordProjectInitializationReceiptError> {
        store.record_project_initialization_receipt(
            "project-1",
            1,
            "identity-hash-1",
            None,
            "env-snapshot-1",
            "skill-inventory-1",
            "manifest-1",
            result,
            issues,
            receipt_digest,
        )
    }

    #[test]
    fn record_project_initialization_receipt_records_a_well_formed_receipt_and_reads_it_back() {
        let root = temp_data_root("record-project-initialization-receipt-well-formed");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(
            store
                .load_project_initialization_receipt("RECEIPT-1")
                .unwrap()
                .is_none()
        );

        let record = record_fixture_project_initialization_receipt(
            &mut store,
            "RECEIPT-1",
            InitializationResult::Ready,
            vec![],
        )
        .unwrap();
        assert_eq!(record.receipt.result, InitializationResult::Ready);

        let loaded = store
            .load_project_initialization_receipt("RECEIPT-1")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_project_initialization_receipt_refuses_to_reuse_an_existing_receipt_digest() {
        let root = temp_data_root("record-project-initialization-receipt-reuse-refused");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        record_fixture_project_initialization_receipt(
            &mut store,
            "RECEIPT-1",
            InitializationResult::Ready,
            vec![],
        )
        .unwrap();
        let err = record_fixture_project_initialization_receipt(
            &mut store,
            "RECEIPT-1",
            InitializationResult::Blocked,
            vec!["environment not qualified".to_string()],
        )
        .unwrap_err();
        assert!(
            matches!(err, RecordProjectInitializationReceiptError::Sql(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn record_project_initialization_receipt_rejects_blocked_with_no_issues_without_writing_anything()
     {
        let root = temp_data_root("record-project-initialization-receipt-domain-rejected");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = record_fixture_project_initialization_receipt(
            &mut store,
            "RECEIPT-1",
            InitializationResult::Blocked,
            vec![],
        )
        .unwrap_err();
        assert!(
            matches!(
                &err,
                RecordProjectInitializationReceiptError::Receipt(
                    ProjectInitializationError::BlockedResultRequiresAtLeastOneIssue
                )
            ),
            "{err:?}"
        );
        assert!(
            store
                .load_project_initialization_receipt("RECEIPT-1")
                .unwrap()
                .is_none()
        );

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_evidence_receipt(receipt_id: &str) -> EvidenceReceipt {
        use autome_domain::evidence::{
            CheckOutcome, EvidenceFingerprint, EvidencePayload, ProcessResultPayload, ReceiptId,
        };
        use autome_domain::requirement::CheckId;

        EvidenceReceipt {
            receipt_id: ReceiptId(receipt_id.to_string()),
            nonce: "nonce-1".into(),
            run_id: "run-1".into(),
            check_id: CheckId("C-001".into()),
            fingerprint: EvidenceFingerprint {
                contract_hash: "contract-1".into(),
                check_hash: "check-1".into(),
                project_rule_snapshot_hash: "rules-1".into(),
                candidate_tree_hash: "tree-1".into(),
                environment_class: "macos-15-arm64".into(),
            },
            verifier_version: "0.1.0".into(),
            payload: EvidencePayload::Process(ProcessResultPayload {
                program: "cargo".into(),
                args: vec!["test".into()],
                exit_code: 0,
                assertions: vec![],
                inventory_changes: vec![],
            }),
            result: CheckOutcome::Pass,
        }
    }

    /// A well-formed `record_evidence` call lands one `evidence_receipts`
    /// row, readable back via `load_evidence`.
    #[test]
    fn record_evidence_records_a_well_formed_receipt_and_reads_it_back() {
        let root = temp_data_root("record-evidence");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let receipt = fixture_evidence_receipt("EV-1");

        assert!(store.load_evidence("EV-1").unwrap().is_none());

        let record = store.record_evidence(&receipt).unwrap();
        assert_eq!(record.receipt, receipt);

        let loaded = store.load_evidence("EV-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call reusing the same `receipt_id` must fail rather than
    /// silently overwriting a prior verifier result.
    #[test]
    fn record_evidence_refuses_to_reuse_an_existing_receipt_id() {
        let root = temp_data_root("record-evidence-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let receipt = fixture_evidence_receipt("EV-1");

        store.record_evidence(&receipt).unwrap();
        let err = store.record_evidence(&receipt).unwrap_err();
        assert!(matches!(err, RecordEvidenceError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_readiness_receipt(receipt_digest: &str) -> ReadinessReceipt {
        use autome_domain::readiness::{
            ExistingRepoSubject, ReadinessResult, ReadinessScope, ReadinessSubject,
        };

        ReadinessReceipt {
            revision: 1,
            scope: ReadinessScope::Execution,
            profile_hash: "profile-1".into(),
            environment_relevant_inputs_digest: "env-digest-1".into(),
            observed_at: "2026-09-15T00:00:00Z".into(),
            valid_until: "2026-09-16T00:00:00Z".into(),
            subject: ReadinessSubject::ExistingRepo(ExistingRepoSubject {
                repository_identity_hash: "repo-hash".into(),
                base_commit: "base".into(),
                target_head: "head".into(),
                worktree_fingerprint: "wt-1".into(),
            }),
            programs: vec![],
            lockfile_hashes: vec![],
            result: ReadinessResult::Ready,
            missing: vec![],
            receipt_digest: receipt_digest.to_string(),
        }
    }

    /// A well-formed `record_readiness` call lands one `readiness_receipts`
    /// row, readable back via `load_readiness`.
    #[test]
    fn record_readiness_records_a_well_formed_receipt_and_reads_it_back() {
        let root = temp_data_root("record-readiness");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let receipt = fixture_readiness_receipt("RD-1");

        assert!(store.load_readiness("RD-1").unwrap().is_none());

        let record = store.record_readiness(&receipt).unwrap();
        assert_eq!(record.receipt, receipt);

        let loaded = store.load_readiness("RD-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call reusing the same `receipt_digest` must fail rather than
    /// silently overwriting a prior probe result.
    #[test]
    fn record_readiness_refuses_to_reuse_an_existing_receipt_digest() {
        let root = temp_data_root("record-readiness-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let receipt = fixture_readiness_receipt("RD-1");

        store.record_readiness(&receipt).unwrap();
        let err = store.record_readiness(&receipt).unwrap_err();
        assert!(matches!(err, RecordReadinessError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_qualification_receipt(receipt_digest: &str) -> QualificationReceipt {
        use autome_domain::model_selection::{
            issue_qualification_receipt, ModelSelectionIdentity, QualificationResult,
        };
        use time::OffsetDateTime;

        let issued_at = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        issue_qualification_receipt(
            ModelSelectionIdentity {
                adapter_id: "claude-code".into(),
                installation_id: "install-1".into(),
                provider: "anthropic".into(),
                cli_hash: "cli-hash".into(),
                protocol_hash: "proto-hash".into(),
                schema_hash: "schema-hash".into(),
                model_id: "claude-sonnet-5".into(),
                resolved_wire_name: Some("claude-sonnet-5-20260101".into()),
                service_tier: None,
                provider_native_effort: "medium".into(),
                auth_mode: "oauth".into(),
                account_fingerprint: "acct-1".into(),
                exposed_snapshot_or_fingerprint: None,
                qualification_batch_id: "batch-1".into(),
                model_choice_key_hash: Some("hash-1".into()),
                runtime_selection_hash: "runtime-1".into(),
            },
            "harness-digest".into(),
            "account-digest".into(),
            "canary-1".into(),
            vec!["run-1".into()],
            issued_at,
            issued_at + time::Duration::days(1),
            false,
            QualificationResult::Qualified,
            receipt_digest.to_string(),
        )
        .unwrap()
    }

    /// A well-formed `record_qualification_receipt` call lands one
    /// `qualification_receipts` row, readable back via
    /// `load_qualification_receipt`.
    #[test]
    fn record_qualification_receipt_records_a_well_formed_receipt_and_reads_it_back() {
        let root = temp_data_root("record-qualification-receipt");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let receipt = fixture_qualification_receipt("QR-1");

        assert!(store.load_qualification_receipt("QR-1").unwrap().is_none());

        let record = store.record_qualification_receipt(&receipt).unwrap();
        assert_eq!(record.receipt, receipt);

        let loaded = store.load_qualification_receipt("QR-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call reusing the same `receipt_digest` must fail rather than
    /// silently overwriting a prior qualification result.
    #[test]
    fn record_qualification_receipt_refuses_to_reuse_an_existing_receipt_digest() {
        let root = temp_data_root("record-qualification-receipt-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let receipt = fixture_qualification_receipt("QR-1");

        store.record_qualification_receipt(&receipt).unwrap();
        let err = store.record_qualification_receipt(&receipt).unwrap_err();
        assert!(
            matches!(err, RecordQualificationReceiptError::Sql(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A well-formed `record_skill_install_receipt` call lands one
    /// `skill_install_receipts` row and seeds an `Installed`-only
    /// `skill_evidence_ladders` row for the same `skill_digest`, both
    /// readable back.
    #[test]
    fn record_skill_install_receipt_seeds_both_the_receipt_and_a_fresh_ladder() {
        let root = temp_data_root("record-skill-install-receipt");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(store.load_skill_install_receipt("SR-1").unwrap().is_none());
        assert!(store.load_skill_evidence_ladder("pkg-1").unwrap().is_none());

        let record = store
            .record_skill_install_receipt(
                "pkg-1",
                SkillAuditOutcome::NoKnownRisksFound,
                "plan-1",
                "decision:1",
                "SR-1",
            )
            .unwrap();
        assert_eq!(record.receipt.package_digest, "pkg-1");
        assert!(record.ladder.installed);
        assert!(!record.ladder.bound);

        let loaded_receipt = store.load_skill_install_receipt("SR-1").unwrap().unwrap();
        assert_eq!(loaded_receipt.receipt, record.receipt);

        let loaded_ladder = store.load_skill_evidence_ladder("pkg-1").unwrap().unwrap();
        assert_eq!(loaded_ladder.ladder, record.ladder);

        std::fs::remove_dir_all(&root).ok();
    }

    /// `issue_skill_install_receipt`'s own `MissingUserApprovalDecision`
    /// check runs server-side and refuses the write -- no receipt or ladder
    /// row is left behind by a rejected install.
    #[test]
    fn record_skill_install_receipt_rejects_a_blank_user_approval_decision() {
        let root = temp_data_root("record-skill-install-receipt-no-approval");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = store
            .record_skill_install_receipt(
                "pkg-1",
                SkillAuditOutcome::NoKnownRisksFound,
                "plan-1",
                "  ",
                "SR-1",
            )
            .unwrap_err();
        assert!(
            matches!(
                err,
                RecordSkillInstallReceiptError::Install(SkillInstallError::MissingUserApprovalDecision)
            ),
            "{err:?}"
        );
        assert!(store.load_skill_install_receipt("SR-1").unwrap().is_none());
        assert!(store.load_skill_evidence_ladder("pkg-1").unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    /// Re-installing the same `skill_digest` resets its ladder back down to
    /// `Installed`-only, even if it had already progressed further.
    #[test]
    fn record_skill_install_receipt_resets_an_already_progressed_ladder() {
        let root = temp_data_root("record-skill-install-receipt-reinstall");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        store
            .record_skill_install_receipt(
                "pkg-1",
                SkillAuditOutcome::NoKnownRisksFound,
                "plan-1",
                "decision:1",
                "SR-1",
            )
            .unwrap();
        store.mark_skill_bound("pkg-1").unwrap();
        assert!(store.load_skill_evidence_ladder("pkg-1").unwrap().unwrap().ladder.bound);

        store
            .record_skill_install_receipt(
                "pkg-1",
                SkillAuditOutcome::NoKnownRisksFound,
                "plan-1",
                "decision:2",
                "SR-2",
            )
            .unwrap();
        let ladder = store.load_skill_evidence_ladder("pkg-1").unwrap().unwrap().ladder;
        assert!(ladder.installed);
        assert!(!ladder.bound);

        std::fs::remove_dir_all(&root).ok();
    }

    /// `mark_skill_bound`/`mark_skill_discoverable`/
    /// `mark_skill_available_to_attempt`/`mark_skill_invoked`/
    /// `record_skill_effective` walk a ladder forward one rung at a time,
    /// each persisted and readable back.
    #[test]
    fn skill_ladder_transitions_walk_forward_and_persist_each_rung() {
        let root = temp_data_root("skill-ladder-transitions");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        store
            .record_skill_install_receipt(
                "pkg-1",
                SkillAuditOutcome::NoKnownRisksFound,
                "plan-1",
                "decision:1",
                "SR-1",
            )
            .unwrap();

        store.mark_skill_bound("pkg-1").unwrap();
        store.mark_skill_discoverable("pkg-1").unwrap();
        store.mark_skill_available_to_attempt("pkg-1").unwrap();
        store.mark_skill_invoked("pkg-1").unwrap();
        let record = store.record_skill_effective("pkg-1", true).unwrap();

        assert!(record.ladder.invoked);
        assert_eq!(record.ladder.effective, Some(true));
        let loaded = store.load_skill_evidence_ladder("pkg-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A ladder transition attempted before its preceding rung is refused by
    /// the domain's own `SkillEvidenceError`, not silently applied.
    #[test]
    fn skill_ladder_transition_rejects_skipping_a_level() {
        let root = temp_data_root("skill-ladder-transition-skip");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        store
            .record_skill_install_receipt(
                "pkg-1",
                SkillAuditOutcome::NoKnownRisksFound,
                "plan-1",
                "decision:1",
                "SR-1",
            )
            .unwrap();

        let err = store.mark_skill_discoverable("pkg-1").unwrap_err();
        assert!(matches!(err, SkillLadderTransitionError::Ladder(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// A transition against a `skill_digest` that was never installed is
    /// `NotFound`, not a silently-created ladder.
    #[test]
    fn skill_ladder_transition_is_not_found_without_a_prior_install() {
        let root = temp_data_root("skill-ladder-transition-not-found");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = store.mark_skill_bound("never-installed").unwrap_err();
        assert!(matches!(err, SkillLadderTransitionError::NotFound), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_global_skill_binding(skill_digest: &str) -> GlobalSkillBinding {
        GlobalSkillBinding {
            revision: 1,
            skill_digest: SkillDigest(skill_digest.to_string()),
            steps: vec![attempt::LoopStepId("implementation".into())],
            cli_targets: vec!["claude-code".into()],
            invocation: InvocationPolicy::ExplicitOnly,
            state: BindingState::Enabled,
        }
    }

    /// `record_global_skill_binding` upserts the current-state row --
    /// no server-side re-validation since `GlobalSkillBinding` has no
    /// validating constructor to re-run.
    #[test]
    fn record_global_skill_binding_upserts_the_current_snapshot() {
        let root = temp_data_root("record-global-skill-binding");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(store.load_global_skill_binding("pkg-1").unwrap().is_none());

        let binding = fixture_global_skill_binding("pkg-1");
        store.record_global_skill_binding(&binding).unwrap();
        assert_eq!(
            store.load_global_skill_binding("pkg-1").unwrap().unwrap(),
            binding
        );

        let mut disabled = binding.clone();
        disabled.revision = 2;
        disabled.state = BindingState::Disabled;
        store.record_global_skill_binding(&disabled).unwrap();
        assert_eq!(
            store.load_global_skill_binding("pkg-1").unwrap().unwrap(),
            disabled
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// `record_project_skill_binding` is keyed by `(project_id,
    /// skill_digest)` -- distinct projects binding the same skill don't
    /// collide.
    #[test]
    fn record_project_skill_binding_is_keyed_by_project_and_skill_digest() {
        let root = temp_data_root("record-project-skill-binding");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let binding = ProjectSkillBinding {
            project_id: "proj-1".into(),
            revision: 1,
            mode: ProjectSkillBindingMode::Disable,
            steps: None,
            cli_targets: None,
            invocation: None,
            state: None,
        };
        store
            .record_project_skill_binding("proj-1", "pkg-1", &binding)
            .unwrap();

        assert_eq!(
            store
                .load_project_skill_binding("proj-1", "pkg-1")
                .unwrap()
                .unwrap(),
            binding
        );
        assert!(store
            .load_project_skill_binding("proj-2", "pkg-1")
            .unwrap()
            .is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_document_review_subject() -> ReviewSubject {
        ReviewSubject::DocumentStep {
            input_snapshot_hash: "input-1".into(),
            output_hash: "output-1".into(),
        }
    }

    fn fixture_anchored_finding() -> HumanReviewFinding {
        use autome_domain::review::{FindingStatus, HumanReviewFindingAnchor};

        HumanReviewFinding {
            id: "finding-1".into(),
            review_receipt_id: "receipt-1".into(),
            subject_hash: "output-1".into(),
            step_id: attempt::LoopStepId("contract_review".into()),
            anchor: HumanReviewFindingAnchor {
                path: Some("contract.md".into()),
                ..Default::default()
            },
            expected_change: "narrow the write scope".into(),
            severity: "major".into(),
            status: FindingStatus::Open,
            successor_attempt_id: None,
            resolution_subject_hash: None,
            finding_digest: "finding-digest-1".into(),
        }
    }

    /// A well-formed `record_human_review_receipt` call lands one
    /// `human_review_receipts` row, readable back via
    /// `load_human_review_receipt`.
    #[test]
    fn record_human_review_receipt_records_a_well_formed_receipt_and_reads_it_back() {
        let root = temp_data_root("record-human-review-receipt");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(store
            .load_human_review_receipt("HR-1")
            .unwrap()
            .is_none());

        let record = store
            .record_human_review_receipt(
                "project-1",
                "task-1",
                "run-1",
                ReviewSpecSubject::Planning {
                    planning_spec_hash: "planning-1".into(),
                },
                attempt::LoopStepId("contract_review".into()),
                "operator-1",
                "2026-09-15T00:00:00Z",
                ReviewDecision::Pass,
                "review-output-1",
                "reason",
                &[],
                fixture_document_review_subject(),
                "HR-1",
            )
            .unwrap();
        assert_eq!(record.receipt.receipt_digest, "HR-1");

        let loaded = store.load_human_review_receipt("HR-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_config_profile(model_id: &str) -> autome_domain::config::AgentExecutionProfile {
        autome_domain::config::AgentExecutionProfile {
            adapter_id: "codex".into(),
            installation_id: "install-1".into(),
            model_id: model_id.into(),
            effort_id: "medium".into(),
            skill_policy_ref: "skill-policy-1".into(),
        }
    }

    /// A fully-populated `GlobalConfigRevision` -- every
    /// `CONFIGURABLE_AI_STEPS` entry gets a `step_defaults` and
    /// `human_review_default` row, matching `config.rs`'s own test fixture
    /// shape (`resolve_project_config` requires every configurable step to
    /// have a global default or it refuses to resolve at all).
    fn fixture_global_config_revision(revision: u32) -> GlobalConfigRevision {
        use autome_domain::config::{HumanReviewSetting, CONFIGURABLE_AI_STEPS};
        use std::collections::HashMap;

        let mut step_defaults = HashMap::new();
        let mut human_review_default = HashMap::new();
        for step in CONFIGURABLE_AI_STEPS {
            step_defaults.insert(
                attempt::LoopStepId((*step).to_string()),
                fixture_config_profile("global-model"),
            );
            human_review_default.insert(
                attempt::LoopStepId((*step).to_string()),
                HumanReviewSetting::Off,
            );
        }
        GlobalConfigRevision {
            revision,
            step_defaults,
            human_review_default,
            environment_defaults_ref: "env-default".into(),
            budget_defaults_ref: "budget-default".into(),
            skill_policy_default_ref: "skill-policy-default".into(),
            safety_policy_hash: "safety-floor-1".into(),
            content_hash: format!("global-content-{revision}"),
        }
    }

    fn fixture_config_preview(
        expires_at: time::OffsetDateTime,
        requires_second_confirmation: bool,
    ) -> GlobalConfigImpactPreview {
        GlobalConfigImpactPreview {
            base_global_revision: 1,
            proposed_config_hash: "proposed-1".into(),
            observed_project_set_hash: "project-set-1".into(),
            affected_projects: vec![],
            blocking_project_ids: vec![],
            requires_second_confirmation,
            expires_at,
            preview_hash: "preview-1".into(),
        }
    }

    fn fixture_project_config_patch(project_id: &str) -> ProjectConfigPatch {
        ProjectConfigPatch {
            project_id: project_id.into(),
            revision: 1,
            step_overrides: std::collections::HashMap::new(),
            human_review_overrides: std::collections::HashMap::new(),
            environment_override_ref: None,
            budget_override_ref: None,
            skill_policy_override_ref: None,
            content_hash: "patch-content-1".into(),
        }
    }

    /// A well-formed `save_global_config_revision` call, with a
    /// non-expired, hash-matching, no-second-confirmation-needed preview,
    /// lands one `global_config_revisions` row readable back both by exact
    /// `revision` and as the "current" one.
    #[test]
    fn save_global_config_revision_persists_a_matching_preview() {
        let root = temp_data_root("save-global-config-revision");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(store.load_global_config_revision(1).unwrap().is_none());
        assert!(store.load_current_global_config_revision().unwrap().is_none());

        let far_future = time::OffsetDateTime::now_utc() + time::Duration::days(1);
        let record = store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, false),
                "preview-1",
                "project-set-1",
                false,
            )
            .unwrap();
        assert_eq!(record.revision.revision, 1);

        assert_eq!(
            store.load_global_config_revision(1).unwrap().unwrap().revision,
            record.revision
        );
        assert_eq!(
            store
                .load_current_global_config_revision()
                .unwrap()
                .unwrap()
                .revision,
            record.revision
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// `load_current_global_config_revision` tracks the highest `revision`
    /// ever recorded, not insertion order or a separately stored pointer.
    #[test]
    fn load_current_global_config_revision_is_the_highest_revision_recorded() {
        let root = temp_data_root("current-global-config-revision");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let far_future = time::OffsetDateTime::now_utc() + time::Duration::days(1);
        store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, false),
                "preview-1",
                "project-set-1",
                false,
            )
            .unwrap();
        store
            .save_global_config_revision(
                fixture_global_config_revision(2),
                &fixture_config_preview(far_future, false),
                "preview-1",
                "project-set-1",
                false,
            )
            .unwrap();

        assert_eq!(
            store
                .load_current_global_config_revision()
                .unwrap()
                .unwrap()
                .revision
                .revision,
            2
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// `save_global_config_revision` re-runs
    /// `config::validate_save_global_config_revision` server-side -- a
    /// preview-hash mismatch is refused and leaves no row behind, same as
    /// the domain-level test in `config.rs`.
    #[test]
    fn save_global_config_revision_rejects_a_preview_hash_mismatch() {
        let root = temp_data_root("save-global-config-revision-hash-mismatch");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let far_future = time::OffsetDateTime::now_utc() + time::Duration::days(1);
        let err = store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, false),
                "wrong-hash",
                "project-set-1",
                false,
            )
            .unwrap_err();
        assert!(
            matches!(
                err,
                SaveGlobalConfigRevisionError::Rejected(ref errors)
                    if errors == &vec![SaveGlobalConfigError::PreviewHashMismatch]
            ),
            "{err:?}"
        );
        assert!(store.load_global_config_revision(1).unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    /// A project-set hash that no longer matches what the preview observed
    /// is refused, same reasoning as the hash-mismatch case.
    #[test]
    fn save_global_config_revision_rejects_a_changed_project_set() {
        let root = temp_data_root("save-global-config-revision-project-set-changed");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let far_future = time::OffsetDateTime::now_utc() + time::Duration::days(1);
        let err = store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, false),
                "preview-1",
                "different-project-set",
                false,
            )
            .unwrap_err();
        assert!(
            matches!(
                err,
                SaveGlobalConfigRevisionError::Rejected(ref errors)
                    if errors == &vec![SaveGlobalConfigError::ProjectSetChanged]
            ),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// An expired preview is refused even if the hash and project set both
    /// still match.
    #[test]
    fn save_global_config_revision_rejects_an_expired_preview() {
        let root = temp_data_root("save-global-config-revision-expired");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let already_expired = time::OffsetDateTime::now_utc() - time::Duration::days(1);
        let err = store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(already_expired, false),
                "preview-1",
                "project-set-1",
                false,
            )
            .unwrap_err();
        assert!(
            matches!(
                err,
                SaveGlobalConfigRevisionError::Rejected(ref errors)
                    if errors == &vec![SaveGlobalConfigError::PreviewExpired]
            ),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A preview that demands a second confirmation is refused without one,
    /// then accepted once `second_confirmation_acquired` is true.
    #[test]
    fn save_global_config_revision_requires_second_confirmation_then_succeeds() {
        let root = temp_data_root("save-global-config-revision-second-confirmation");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let far_future = time::OffsetDateTime::now_utc() + time::Duration::days(1);
        let err = store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, true),
                "preview-1",
                "project-set-1",
                false,
            )
            .unwrap_err();
        assert!(
            matches!(
                err,
                SaveGlobalConfigRevisionError::Rejected(ref errors)
                    if errors == &vec![SaveGlobalConfigError::SecondConfirmationRequired]
            ),
            "{err:?}"
        );
        assert!(store.load_global_config_revision(1).unwrap().is_none());

        let record = store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, true),
                "preview-1",
                "project-set-1",
                true,
            )
            .unwrap();
        assert_eq!(record.revision.revision, 1);

        std::fs::remove_dir_all(&root).ok();
    }

    /// `issue_human_review_receipt`'s own invariant (a Reject decision needs
    /// at least one anchored, actionable finding) is enforced server-side --
    /// a caller cannot bypass it by handing in an already-built receipt.
    #[test]
    fn record_human_review_receipt_refuses_a_reject_decision_without_findings() {
        let root = temp_data_root("record-human-review-receipt-reject-without-findings");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = store
            .record_human_review_receipt(
                "project-1",
                "task-1",
                "run-1",
                ReviewSpecSubject::Planning {
                    planning_spec_hash: "planning-1".into(),
                },
                attempt::LoopStepId("contract_review".into()),
                "operator-1",
                "2026-09-15T00:00:00Z",
                ReviewDecision::Reject,
                "review-output-1",
                "reason",
                &[],
                fixture_document_review_subject(),
                "HR-1",
            )
            .unwrap_err();
        assert!(matches!(err, RecordHumanReviewReceiptError::Receipt(_)), "{err:?}");
        assert!(store
            .load_human_review_receipt("HR-1")
            .unwrap()
            .is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call reusing the same `receipt_digest` must fail rather than
    /// silently overwriting a prior review decision.
    #[test]
    fn record_human_review_receipt_refuses_to_reuse_an_existing_receipt_digest() {
        let root = temp_data_root("record-human-review-receipt-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let finding = fixture_anchored_finding();

        store
            .record_human_review_receipt(
                "project-1",
                "task-1",
                "run-1",
                ReviewSpecSubject::Planning {
                    planning_spec_hash: "planning-1".into(),
                },
                attempt::LoopStepId("contract_review".into()),
                "operator-1",
                "2026-09-15T00:00:00Z",
                ReviewDecision::Reject,
                "review-output-1",
                "reason",
                std::slice::from_ref(&finding),
                fixture_document_review_subject(),
                "HR-1",
            )
            .unwrap();
        let err = store
            .record_human_review_receipt(
                "project-1",
                "task-1",
                "run-1",
                ReviewSpecSubject::Planning {
                    planning_spec_hash: "planning-1".into(),
                },
                attempt::LoopStepId("contract_review".into()),
                "operator-1",
                "2026-09-15T00:00:00Z",
                ReviewDecision::Reject,
                "review-output-1",
                "reason",
                std::slice::from_ref(&finding),
                fixture_document_review_subject(),
                "HR-1",
            )
            .unwrap_err();
        assert!(matches!(err, RecordHumanReviewReceiptError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// Saving the same `revision` number twice hits the `revision INTEGER
    /// PRIMARY KEY` constraint and surfaces as `Sql`, not a silent
    /// overwrite -- `global_config_revisions` is an append-only lineage,
    /// unlike `project_config_patches`'s upsert.
    #[test]
    fn save_global_config_revision_rejects_a_duplicate_revision_number() {
        let root = temp_data_root("save-global-config-revision-duplicate");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let far_future = time::OffsetDateTime::now_utc() + time::Duration::days(1);
        store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, false),
                "preview-1",
                "project-set-1",
                false,
            )
            .unwrap();

        let err = store
            .save_global_config_revision(
                fixture_global_config_revision(1),
                &fixture_config_preview(far_future, false),
                "preview-1",
                "project-set-1",
                false,
            )
            .unwrap_err();
        assert!(
            matches!(err, SaveGlobalConfigRevisionError::Sql(_)),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// `record_project_config_patch` upserts the current-state row for a
    /// `project_id` -- a second call for the same project replaces the
    /// row, and a different project's row is untouched.
    #[test]
    fn record_project_config_patch_upserts_by_project_id() {
        let root = temp_data_root("record-project-config-patch");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        assert!(store.load_project_config_patch("proj-1").unwrap().is_none());

        let patch = fixture_project_config_patch("proj-1");
        store.record_project_config_patch(&patch).unwrap();
        assert_eq!(
            store.load_project_config_patch("proj-1").unwrap().unwrap(),
            patch
        );

        let mut updated = patch.clone();
        updated.revision = 2;
        updated.environment_override_ref = Some("env-override-1".into());
        store.record_project_config_patch(&updated).unwrap();
        assert_eq!(
            store.load_project_config_patch("proj-1").unwrap().unwrap(),
            updated
        );

        assert!(store.load_project_config_patch("proj-2").unwrap().is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_existing_repo_delivery_subject() -> DeliverySubject {
        use autome_domain::delivery::ExistingRepoDelivery;

        DeliverySubject::ExistingRepo(ExistingRepoDelivery {
            repository_identity_hash: "repo-hash".into(),
            target_head: "head".into(),
            target_worktree_fingerprint: "wt-1".into(),
            new_ref: "refs/heads/delivered".into(),
        })
    }

    fn fixture_greenfield_delivery_subject() -> DeliverySubject {
        use autome_domain::delivery::GreenfieldDelivery;

        DeliverySubject::Greenfield(GreenfieldDelivery {
            parent_directory_identity_hash: "parent-hash".into(),
            destination: "dest".into(),
            destination_absent_proof: "absent-proof".into(),
            template_hash: "template-hash".into(),
        })
    }

    fn fixture_delivery_envelope() -> autome_domain::delivery::DeliveryEnvelope {
        autome_domain::delivery::DeliveryEnvelope {
            run_id: "run-1".into(),
            contract_hash: "contract-1".into(),
            candidate_certificate_hash: "candidate-1".into(),
            policy_hash: "policy-1".into(),
            nonce: "nonce-1".into(),
            issued_at: "2026-09-15T00:00:00Z".into(),
            receipt_digest: "digest-1".into(),
        }
    }

    fn fixture_rehearsal(subject: DeliverySubject) -> DeliveryRehearsalReceipt {
        DeliveryRehearsalReceipt {
            envelope: fixture_delivery_envelope(),
            subject,
            target_head_or_parent: "head".into(),
            delivery_tree_hash: "tree-1".into(),
            check_receipt_ids: vec!["check-1".into()],
        }
    }

    fn fixture_approval() -> DeliveryApprovalReceipt {
        DeliveryApprovalReceipt {
            envelope: fixture_delivery_envelope(),
            rehearsal_receipt_digest: "digest-1".into(),
            display_summary: "summary".into(),
            destination_or_new_ref: "refs/heads/delivered".into(),
            artifact_destinations: vec![],
            valid_until: "2026-09-16T00:00:00Z".into(),
            operator_decision_ref: "decision:1".into(),
        }
    }

    fn fixture_delivery_receipt(
        outcome: autome_domain::delivery::DeliveryOutcome,
    ) -> DeliveryReceipt {
        DeliveryReceipt {
            envelope: fixture_delivery_envelope(),
            approval_receipt_digest: "digest-1".into(),
            before_identity_hash: "before-1".into(),
            after_identity_hash: "after-1".into(),
            outcome,
        }
    }

    fn fixture_tree_check(matches_delivery: bool) -> DeliveredTreeCheckReceipt {
        DeliveredTreeCheckReceipt {
            envelope: fixture_delivery_envelope(),
            delivery_receipt_digest: "digest-1".into(),
            observed_ref_or_tree: "refs/heads/delivered".into(),
            artifact_hashes: vec![],
            worktree_fingerprint: "wt-1".into(),
            matches_delivery,
        }
    }

    fn fixture_project_target_transition() -> ProjectTargetTransitionReceipt {
        ProjectTargetTransitionReceipt {
            envelope: fixture_delivery_envelope(),
            greenfield_destination: "dest".into(),
            delivered_tree_hash: "tree-1".into(),
            new_repository_identity_hash: "new-repo-hash".into(),
            pre_transition_project_revision: 1,
            post_transition_project_revision: 2,
        }
    }

    /// A well-formed `start_delivery_chain` call lands one `delivery_chains`
    /// row with every rung still `None`, readable back via
    /// `load_delivery_chain`.
    #[test]
    fn start_delivery_chain_records_a_new_chain_and_reads_it_back() {
        let root = temp_data_root("start-delivery-chain");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let subject = fixture_existing_repo_delivery_subject();

        assert!(store.load_delivery_chain("run-1").unwrap().is_none());

        let record = store.start_delivery_chain("run-1", &subject).unwrap();
        assert_eq!(record.subject, subject);
        assert!(record.rehearsal.is_none());

        let loaded = store.load_delivery_chain("run-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second `start_delivery_chain` call for the same `run_id` must fail
    /// rather than silently replacing the chain's subject.
    #[test]
    fn start_delivery_chain_refuses_to_reuse_an_existing_run_id() {
        let root = temp_data_root("start-delivery-chain-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let subject = fixture_existing_repo_delivery_subject();

        store.start_delivery_chain("run-1", &subject).unwrap();
        let err = store
            .start_delivery_chain("run-1", &subject)
            .unwrap_err();
        assert!(matches!(err, StartDeliveryChainError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// Appending any rung to a `run_id` that was never started is
    /// `NotFound`, not a domain chain-order rejection -- there is no chain
    /// to check an order against yet.
    #[test]
    fn append_delivery_rehearsal_is_not_found_for_an_unstarted_chain() {
        let root = temp_data_root("append-rehearsal-not-found");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let receipt = fixture_rehearsal(fixture_existing_repo_delivery_subject());

        let err = store
            .append_delivery_rehearsal("no-such-run", &receipt)
            .unwrap_err();
        assert!(matches!(err, AppendDeliveryReceiptError::NotFound), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.12: approval before rehearsal is rejected by the domain chain's
    /// own order check, surfaced here as `AppendDeliveryReceiptError::Chain`
    /// rather than reimplemented at the store layer.
    #[test]
    fn append_delivery_approval_before_rehearsal_is_rejected_by_the_domain_chain() {
        let root = temp_data_root("append-approval-before-rehearsal");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let subject = fixture_existing_repo_delivery_subject();
        store.start_delivery_chain("run-1", &subject).unwrap();

        let err = store
            .append_delivery_approval("run-1", &fixture_approval())
            .unwrap_err();
        assert!(
            matches!(
                err,
                AppendDeliveryReceiptError::Chain(
                    DeliveryChainError::RehearsalRequiredBeforeApproval
                )
            ),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.12 end to end for an `ExistingRepo` subject: rehearsal -> approval
    /// -> delivery (Succeeded) -> tree check (matches) reaches
    /// `is_ready_for_completion() == Ok(())` without any project target
    /// transition, matching `delivery.rs`'s own
    /// `successful_chain_for_existing_repo_is_ready_for_completion_without_transition`.
    #[test]
    fn full_existing_repo_chain_is_ready_for_completion_without_transition() {
        let root = temp_data_root("full-chain-existing-repo");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let subject = fixture_existing_repo_delivery_subject();
        store.start_delivery_chain("run-1", &subject).unwrap();

        store
            .append_delivery_rehearsal("run-1", &fixture_rehearsal(subject.clone()))
            .unwrap();
        store
            .append_delivery_approval("run-1", &fixture_approval())
            .unwrap();
        store
            .append_delivery_delivery(
                "run-1",
                &fixture_delivery_receipt(autome_domain::delivery::DeliveryOutcome::Succeeded),
            )
            .unwrap();
        let record = store
            .append_delivery_tree_check("run-1", &fixture_tree_check(true))
            .unwrap();

        assert!(record.rebuild().is_ready_for_completion().is_ok());

        let loaded = store.load_delivery_chain("run-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// §5.12: a `Greenfield` subject's chain is only ready for completion
    /// once a `ProjectTargetTransitionReceipt` has also been appended --
    /// matching `delivery.rs`'s own
    /// `greenfield_completion_requires_project_target_transition`.
    #[test]
    fn greenfield_chain_requires_project_target_transition_before_completion() {
        let root = temp_data_root("greenfield-chain-transition");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let subject = fixture_greenfield_delivery_subject();
        store.start_delivery_chain("run-1", &subject).unwrap();

        store
            .append_delivery_rehearsal("run-1", &fixture_rehearsal(subject.clone()))
            .unwrap();
        store
            .append_delivery_approval("run-1", &fixture_approval())
            .unwrap();
        store
            .append_delivery_delivery(
                "run-1",
                &fixture_delivery_receipt(autome_domain::delivery::DeliveryOutcome::Succeeded),
            )
            .unwrap();
        let record = store
            .append_delivery_tree_check("run-1", &fixture_tree_check(true))
            .unwrap();
        assert_eq!(
            record.rebuild().is_ready_for_completion().unwrap_err(),
            DeliveryChainError::GreenfieldCompletionRequiresProjectTargetTransition
        );

        let record = store
            .append_delivery_project_target_transition(
                "run-1",
                &fixture_project_target_transition(),
            )
            .unwrap();
        assert!(record.rebuild().is_ready_for_completion().is_ok());

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_readiness_fingerprint() -> ReadinessFingerprint {
        use autome_domain::readiness::{ExistingRepoSubject, ReadinessSubject};

        ReadinessFingerprint {
            profile_hash: "profile-1".into(),
            environment_relevant_inputs_digest: "env-digest-1".into(),
            subject: ReadinessSubject::ExistingRepo(ExistingRepoSubject {
                repository_identity_hash: "repo-hash".into(),
                base_commit: "base".into(),
                target_head: "head".into(),
                worktree_fingerprint: "wt-1".into(),
            }),
        }
    }

    fn fixture_audit_verdict(
        requirement_id: &str,
        outcome: autome_domain::certificate::AuditOutcome,
        receipt_ids: Vec<&str>,
    ) -> AuditVerdict {
        AuditVerdict::new(
            RequirementId(requirement_id.into()),
            outcome,
            receipt_ids.into_iter().map(|id| ReceiptId(id.into())).collect(),
        )
        .unwrap()
    }

    fn record_ready_readiness(store: &mut EventStore, digest: &str) {
        let mut receipt = fixture_readiness_receipt(digest);
        receipt.profile_hash = "profile-1".into();
        receipt.environment_relevant_inputs_digest = "env-digest-1".into();
        store.record_readiness(&receipt).unwrap();
    }

    /// A well-formed `issue_candidate_certificate` call composes an
    /// already-recorded readiness receipt (looked up by digest) with a
    /// satisfied verdict for every `must` requirement, lands one
    /// `candidate_certificates` row, and is readable back via
    /// `load_candidate_certificate`.
    #[test]
    fn issue_candidate_certificate_issues_a_well_formed_certificate_and_reads_it_back() {
        let root = temp_data_root("issue-candidate-certificate");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        record_ready_readiness(&mut store, "RD-1");
        let must = vec![RequirementId("R-001".into())];
        let verdicts = vec![fixture_audit_verdict(
            "R-001",
            autome_domain::certificate::AuditOutcome::Satisfied,
            vec!["EV-1"],
        )];
        let mut valid_receipt_ids = HashSet::new();
        valid_receipt_ids.insert(ReceiptId("EV-1".into()));

        assert!(store.load_candidate_certificate("run-1").unwrap().is_none());

        let record = store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &must,
                &verdicts,
                &valid_receipt_ids,
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap();
        assert_eq!(record.certificate.run_id, "run-1");
        assert_eq!(record.certificate.covered_requirement_ids, must);

        let loaded = store.load_candidate_certificate("run-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// Referencing a `readiness_receipt_digest` that was never recorded is
    /// `ReadinessNotFound`, not a domain rejection -- there is no receipt to
    /// even check readiness/currency against yet.
    #[test]
    fn issue_candidate_certificate_is_readiness_not_found_for_an_unrecorded_digest() {
        let root = temp_data_root("issue-candidate-certificate-readiness-not-found");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();

        let err = store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &[],
                &[],
                &HashSet::new(),
                "no-such-digest",
                &fixture_readiness_fingerprint(),
            )
            .unwrap_err();
        assert!(
            matches!(err, IssueCandidateCertificateError::ReadinessNotFound),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A `must` requirement with no verdict at all is rejected by the
    /// domain's own `issue_candidate_certificate`, surfaced here as
    /// `IssueCandidateCertificateError::Domain` rather than reimplemented at
    /// the store layer.
    #[test]
    fn issue_candidate_certificate_passes_through_a_domain_rejection() {
        let root = temp_data_root("issue-candidate-certificate-domain-rejection");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        record_ready_readiness(&mut store, "RD-1");
        let must = vec![RequirementId("R-001".into())];

        let err = store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &must,
                &[],
                &HashSet::new(),
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap_err();
        assert!(
            matches!(
                &err,
                IssueCandidateCertificateError::Domain(errors)
                    if errors == &vec![CandidateCertificateError::MissingVerdict {
                        requirement: RequirementId("R-001".into())
                    }]
            ),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second `issue_candidate_certificate` call for the same `run_id`
    /// must fail rather than silently replacing a prior candidate
    /// certificate.
    #[test]
    fn issue_candidate_certificate_refuses_to_reuse_an_existing_run_id() {
        let root = temp_data_root("issue-candidate-certificate-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        record_ready_readiness(&mut store, "RD-1");

        store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &[],
                &[],
                &HashSet::new(),
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap();
        let err = store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &[],
                &[],
                &HashSet::new(),
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap_err();
        assert!(matches!(err, IssueCandidateCertificateError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    fn build_ready_delivery_chain(store: &mut EventStore, run_id: &str) {
        let subject = fixture_existing_repo_delivery_subject();
        store.start_delivery_chain(run_id, &subject).unwrap();
        store
            .append_delivery_rehearsal(run_id, &fixture_rehearsal(subject.clone()))
            .unwrap();
        store
            .append_delivery_approval(run_id, &fixture_approval())
            .unwrap();
        store
            .append_delivery_delivery(
                run_id,
                &fixture_delivery_receipt(autome_domain::delivery::DeliveryOutcome::Succeeded),
            )
            .unwrap();
        store
            .append_delivery_tree_check(run_id, &fixture_tree_check(true))
            .unwrap();
    }

    /// A well-formed `issue_completion_certificate` call composes an
    /// already-issued candidate certificate with an already-ready delivery
    /// chain, lands one `completion_certificates` row, and is readable back
    /// via `load_completion_certificate`.
    #[test]
    fn issue_completion_certificate_issues_a_well_formed_certificate_and_reads_it_back() {
        let root = temp_data_root("issue-completion-certificate");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        record_ready_readiness(&mut store, "RD-1");
        store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &[],
                &[],
                &HashSet::new(),
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap();
        build_ready_delivery_chain(&mut store, "run-1");

        assert!(store.load_completion_certificate("run-1").unwrap().is_none());

        let record = store
            .issue_completion_certificate("run-1", "tree-2", "decision:1")
            .unwrap();
        assert_eq!(record.run_id, "run-1");
        assert_eq!(record.certificate.delivery_tree_hash, "tree-2");

        let loaded = store.load_completion_certificate("run-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// Issuing a completion certificate for a `run_id` with no candidate
    /// certificate is `CandidateNotFound`, not a domain rejection.
    #[test]
    fn issue_completion_certificate_is_candidate_not_found_without_a_candidate_certificate() {
        let root = temp_data_root("issue-completion-certificate-candidate-not-found");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        build_ready_delivery_chain(&mut store, "run-1");

        let err = store
            .issue_completion_certificate("run-1", "tree-2", "decision:1")
            .unwrap_err();
        assert!(
            matches!(err, IssueCompletionCertificateError::CandidateNotFound),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// Issuing a completion certificate for a `run_id` with no delivery
    /// chain started is `DeliveryChainNotFound`, not a domain rejection.
    #[test]
    fn issue_completion_certificate_is_delivery_chain_not_found_without_a_started_chain() {
        let root = temp_data_root("issue-completion-certificate-chain-not-found");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        record_ready_readiness(&mut store, "RD-1");
        store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &[],
                &[],
                &HashSet::new(),
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap();

        let err = store
            .issue_completion_certificate("run-1", "tree-2", "decision:1")
            .unwrap_err();
        assert!(
            matches!(err, IssueCompletionCertificateError::DeliveryChainNotFound),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A blank `user_approval_decision_ref` is rejected by the domain's own
    /// `issue_completion_certificate`, surfaced here as
    /// `IssueCompletionCertificateError::Domain` rather than reimplemented
    /// at the store layer.
    #[test]
    fn issue_completion_certificate_passes_through_a_domain_rejection() {
        let root = temp_data_root("issue-completion-certificate-domain-rejection");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        record_ready_readiness(&mut store, "RD-1");
        store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &[],
                &[],
                &HashSet::new(),
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap();
        build_ready_delivery_chain(&mut store, "run-1");

        let err = store
            .issue_completion_certificate("run-1", "tree-2", "  ")
            .unwrap_err();
        assert!(
            matches!(
                err,
                IssueCompletionCertificateError::Domain(
                    CompletionCertificateError::MissingUserApprovalDecision
                )
            ),
            "{err:?}"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second `issue_completion_certificate` call for the same `run_id`
    /// must fail rather than silently replacing a prior completion
    /// certificate.
    #[test]
    fn issue_completion_certificate_refuses_to_reuse_an_existing_run_id() {
        let root = temp_data_root("issue-completion-certificate-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        record_ready_readiness(&mut store, "RD-1");
        store
            .issue_candidate_certificate(
                "run-1",
                1,
                "commit-1",
                "tree-1",
                &[],
                &[],
                &HashSet::new(),
                "RD-1",
                &fixture_readiness_fingerprint(),
            )
            .unwrap();
        build_ready_delivery_chain(&mut store, "run-1");

        store
            .issue_completion_certificate("run-1", "tree-2", "decision:1")
            .unwrap();
        let err = store
            .issue_completion_certificate("run-1", "tree-2", "decision:1")
            .unwrap_err();
        assert!(matches!(err, IssueCompletionCertificateError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    fn fixture_frozen_playbook(hash: &str) -> FrozenPlaybook {
        use autome_domain::playbook::PlaybookId;

        FrozenPlaybook {
            playbook_id: PlaybookId::ExistingRepoChange,
            manifest_content_hash: hash.to_string(),
        }
    }

    /// A well-formed `bind_playbook` call lands one `frozen_playbooks` row,
    /// readable back via `load_playbook`.
    #[test]
    fn bind_playbook_records_a_well_formed_playbook_and_reads_it_back() {
        let root = temp_data_root("bind-playbook");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let playbook = fixture_frozen_playbook("hash-1");

        assert!(store.load_playbook("run-1").unwrap().is_none());

        let record = store.bind_playbook("run-1", &playbook).unwrap();
        assert_eq!(record.run_id, "run-1");
        assert_eq!(record.playbook, playbook);

        let loaded = store.load_playbook("run-1").unwrap().unwrap();
        assert_eq!(loaded, record);

        std::fs::remove_dir_all(&root).ok();
    }

    /// A second call reusing the same `run_id` must fail rather than
    /// silently swapping the playbook a Run is bound to mid-flight.
    #[test]
    fn bind_playbook_refuses_to_reuse_an_existing_run_id() {
        let root = temp_data_root("bind-playbook-reuse");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let playbook = fixture_frozen_playbook("hash-1");

        store.bind_playbook("run-1", &playbook).unwrap();
        let err = store.bind_playbook("run-1", &playbook).unwrap_err();
        assert!(matches!(err, BindPlaybookError::Sql(_)), "{err:?}");

        std::fs::remove_dir_all(&root).ok();
    }

    /// §6.1: a project created from a target lands at exactly
    /// `phase=ResolvingIntent, hold=IntentUnresolved` -- 4x `AdvanceNominal`
    /// from `Registered` followed by `IntentUnresolved`, never advanced
    /// further (there is no persisted `ProjectIntent` yet to justify
    /// `IntentResolved`).
    #[test]
    fn create_from_target_lands_project_at_resolving_intent_with_intent_unresolved_hold() {
        let root = temp_data_root("phase-hold");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let probe = sample_probe("phase-hold", true, true, true);
        let inspection = probe.to_inspection(false);
        let target_id = store
            .register_target(ProjectKind::ExistingRepository, &probe, &inspection)
            .unwrap();

        let created = store
            .create_project_from_target(&target_id, "Phase Hold Project", true, None)
            .unwrap();
        assert_eq!(created.appended.revision, 6);
        assert_eq!(created.appended.event_type, "IntentUnresolved");

        let (revision, state) = store
            .load_project_state(&created.project_id)
            .unwrap()
            .unwrap();
        assert_eq!(revision, 6);
        assert_eq!(
            state.phase,
            autome_domain::project::ProjectPhase::ResolvingIntent
        );
        assert_eq!(
            state.hold,
            autome_domain::project::ProjectHold::IntentUnresolved
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3: the ProjectHome directory `create_project_from_target` writes
    /// must actually land on disk, owner-only -- the directory itself
    /// `0700` and its `manifest.json` `0600` -- not just a string returned
    /// by `project_home_for` with nothing behind it.
    #[test]
    fn create_from_target_writes_a_project_home_with_owner_only_permissions() {
        let root = temp_data_root("permissions");
        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let probe = sample_probe("permissions", true, true, true);
        let inspection = probe.to_inspection(false);
        let target_id = store
            .register_target(ProjectKind::ExistingRepository, &probe, &inspection)
            .unwrap();

        let created = store
            .create_project_from_target(&target_id, "Permissions Project", true, None)
            .unwrap();

        let home = std::path::PathBuf::from(store.project_home_for(&created.project_id));
        let dir_meta = std::fs::symlink_metadata(&home).unwrap();
        assert!(dir_meta.is_dir());
        assert_eq!(dir_meta.mode() & 0o777, 0o700);

        let manifest_meta = std::fs::symlink_metadata(home.join("manifest.json")).unwrap();
        assert!(manifest_meta.is_file());
        assert_eq!(manifest_meta.mode() & 0o777, 0o600);

        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.2 "落盘失败即整事务回滚": here the shared `projects/` root
    /// already exists but is `0500` (no owner-write bit), so `mkdir`ing
    /// the new project's own directory underneath it fails at the OS
    /// level *before* any SQL is ever executed. Asserts the full
    /// invariant: no new `project_projections`/`events` rows, no orphan
    /// directory left under `projects/`, and the target itself comes back
    /// still unconsumed so a retry remains possible once the permission
    /// problem is fixed.
    #[test]
    fn disk_write_failure_leaves_no_orphan_directory_and_no_journaled_events() {
        let root = temp_data_root("disk-failure");
        let projects_root = root.join("projects");
        {
            let mut builder = std::fs::DirBuilder::new();
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o500);
            builder.create(&projects_root).unwrap();
        }

        let db_path = root.join("db.sqlite3").to_string_lossy().into_owned();
        let mut store = EventStore::open(&db_path).unwrap();
        let probe = sample_probe("disk-failure", true, true, true);
        let inspection = probe.to_inspection(false);
        let target_id = store
            .register_target(ProjectKind::ExistingRepository, &probe, &inspection)
            .unwrap();

        let projections_before: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM project_projections", [], |row| {
                row.get(0)
            })
            .unwrap();
        let events_before: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();

        let result =
            store.create_project_from_target(&target_id, "Disk Failure Project", true, None);
        assert!(
            matches!(result, Err(CreateFromTargetError::FsGuard(_))),
            "{result:?}"
        );

        let projections_after: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM project_projections", [], |row| {
                row.get(0)
            })
            .unwrap();
        let events_after: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(projections_before, projections_after);
        assert_eq!(events_before, events_after);
        assert_eq!(std::fs::read_dir(&projects_root).unwrap().count(), 0);

        let record = store.load_target(&target_id).unwrap().unwrap();
        assert!(record.consumed_by_project_id.is_none());

        std::fs::set_permissions(&projects_root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    /// §8.3:1362 diagnostic state: `EventStore::open` re-verifies the data
    /// root at startup and records a reason when it fails. This is the
    /// store-level half of that story (`diagnostic_reason()` reports
    /// `None`/`Some` correctly); the behavioral consequence -- writes
    /// refused, reads unaffected -- is enforced by `dispatch::dispatch`'s
    /// gate check (its own test suite), not by any individual store
    /// method here, since `create_project_from_target` and friends
    /// deliberately do not each re-check `diagnostic_reason()` themselves
    /// (see that method's doc comment).
    #[test]
    fn diagnostic_reason_is_none_for_healthy_root_and_set_when_reverification_fails() {
        let healthy_root = temp_data_root("diagnostic-healthy");
        let healthy_db_path = healthy_root
            .join("db.sqlite3")
            .to_string_lossy()
            .into_owned();
        let healthy_store = EventStore::open(&healthy_db_path).unwrap();
        assert!(healthy_store.diagnostic_reason().is_none());
        std::fs::remove_dir_all(&healthy_root).ok();

        let unsafe_root = temp_data_root("diagnostic-unsafe");
        std::fs::set_permissions(&unsafe_root, std::fs::Permissions::from_mode(0o777)).unwrap();
        let unsafe_db_path = unsafe_root
            .join("db.sqlite3")
            .to_string_lossy()
            .into_owned();
        let unsafe_store = EventStore::open(&unsafe_db_path).unwrap();
        assert!(unsafe_store.diagnostic_reason().is_some());

        std::fs::set_permissions(&unsafe_root, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_dir_all(&unsafe_root).ok();
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
    fn create_project_writes_revision_one_with_registered_projection() {
        use autome_domain::project::{ProjectHold, ProjectLifecycle, ProjectPhase};

        let path = temp_db_path("create-project-revision-one");
        let mut store = EventStore::open(&path).unwrap();
        let identity = sample_project_identity("project-1");
        let appended = store.create_project(&identity).unwrap();
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "project.created");
        assert_eq!(appended.state.lifecycle, ProjectLifecycle::Active);
        assert_eq!(appended.state.phase, ProjectPhase::Registered);
        assert_eq!(appended.state.hold, ProjectHold::None);
        let (revision, loaded) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        let loaded_identity = store.load_project_identity("project-1").unwrap().unwrap();
        assert_eq!(loaded_identity, identity);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_project_event_after_creation_advances_to_revision_two() {
        use autome_domain::project::ProjectPhase;

        let path = temp_db_path("first-project-event");
        let mut store = EventStore::open(&path).unwrap();
        store
            .create_project(&sample_project_identity("project-1"))
            .unwrap();
        let appended = store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(appended.state.phase, ProjectPhase::Inspecting);
        assert_eq!(appended.revision, 2);
        assert_eq!(appended.event_type, "AdvanceNominal");
        let (revision, loaded) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn create_project_rejects_duplicate_aggregate_id() {
        let path = temp_db_path("create-project-duplicate");
        let mut store = EventStore::open(&path).unwrap();
        store
            .create_project(&sample_project_identity("project-1"))
            .unwrap();
        let err = store
            .create_project(&sample_project_identity("project-1"))
            .unwrap_err();
        assert!(matches!(err, ProjectAppendError::AlreadyExists));
        assert_eq!(store.event_count("project-1"), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn append_project_event_on_uncreated_project_is_not_found() {
        let path = temp_db_path("uncreated-project-append");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap_err();
        assert!(matches!(err, ProjectAppendError::NotFound));
        assert!(store.load_project_state("project-1").unwrap().is_none());
        assert_eq!(store.event_count("project-1"), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_project_events_advance_revision_and_state() {
        use autome_domain::project::ProjectPhase;

        let path = temp_db_path("sequential-project");
        let mut store = EventStore::open(&path).unwrap();
        store
            .create_project(&sample_project_identity("project-1"))
            .unwrap();
        store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap();
        let second = store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(second.state.phase, ProjectPhase::AwaitingTrust);
        let (revision, _) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 3);
        assert_eq!(store.event_count("project-1"), 3);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_project_transition_writes_nothing() {
        let path = temp_db_path("illegal-project");
        let mut store = EventStore::open(&path).unwrap();
        store
            .create_project(&sample_project_identity("project-1"))
            .unwrap();
        let err = store
            .append_project_event("project-1", ProjectEvent::IntentResolved)
            .unwrap_err();
        assert!(matches!(err, ProjectAppendError::Transition(_)));
        let (revision, _) = store.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(store.event_count("project-1"), 1);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn project_state_survives_reconnect() {
        use autome_domain::project::ProjectPhase;

        let path = temp_db_path("project-reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .create_project(&sample_project_identity("project-1"))
                .unwrap();
            store
                .append_project_event("project-1", ProjectEvent::AdvanceNominal)
                .unwrap();
            store
                .append_project_event("project-1", ProjectEvent::AdvanceNominal)
                .unwrap();
        }
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_project_state("project-1").unwrap().unwrap();
        assert_eq!(revision, 3);
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
            .create_project(&sample_project_identity("same-id"))
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
    fn unknown_node_aggregate_has_no_projection() {
        let path = temp_db_path("unknown-node-aggregate");
        let store = EventStore::open(&path).unwrap();
        assert!(store.load_node_status("run-1:node-1").unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_legal_node_event_creates_revision_one() {
        let path = temp_db_path("first-node-event");
        let mut store = EventStore::open(&path).unwrap();
        let appended = store
            .append_node_event("run-1:node-1", NodeEvent::BecomeReady)
            .unwrap();
        assert_eq!(appended.state, NodeStatus::Ready);
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "BecomeReady");
        assert_eq!(appended.seq, 1);
        let (revision, loaded) = store.load_node_status("run-1:node-1").unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_node_events_advance_revision_and_state() {
        let path = temp_db_path("sequential-node");
        let mut store = EventStore::open(&path).unwrap();
        let first = store
            .append_node_event("run-1:node-1", NodeEvent::BecomeReady)
            .unwrap();
        let second = store
            .append_node_event("run-1:node-1", NodeEvent::StartProducing)
            .unwrap();
        assert_eq!(second.state, NodeStatus::Producing);
        assert_eq!(second.seq, first.seq + 1);
        let (revision, _) = store.load_node_status("run-1:node-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(store.event_count("run-1:node-1"), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_node_transition_writes_nothing() {
        let path = temp_db_path("illegal-node");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_node_event("run-1:node-1", NodeEvent::VerificationPassed)
            .unwrap_err();
        assert!(matches!(err, NodeAppendError::Transition(_)));
        assert!(store.load_node_status("run-1:node-1").unwrap().is_none());
        assert_eq!(store.event_count("run-1:node-1"), 0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn node_state_survives_reconnect() {
        let path = temp_db_path("node-reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .append_node_event("run-1:node-1", NodeEvent::BecomeReady)
                .unwrap();
            store
                .append_node_event("run-1:node-1", NodeEvent::StartProducing)
                .unwrap();
        }
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_node_status("run-1:node-1").unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(state, NodeStatus::Producing);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn node_aggregate_id_is_caller_composed_and_scoped_per_run() {
        // Same bare NodeId string, different Run scope (§6.6 re-planning
        // restarts a graph's nodes from Pending on a new Run): the two
        // caller-composed aggregate_ids must not collide.
        let path = temp_db_path("node-per-run-scoping");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_node_event("run-1:node-a", NodeEvent::BecomeReady)
            .unwrap();
        assert!(store.load_node_status("run-2:node-a").unwrap().is_none());
        let (_, state) = store.load_node_status("run-1:node-a").unwrap().unwrap();
        assert_eq!(state, NodeStatus::Ready);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn run_project_task_contract_graph_execution_queue_and_node_share_the_events_table_without_colliding()
     {
        let path = temp_db_path("shared-events-table-seven-way");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_run_event("same-id", RunEvent::AdvanceNominal)
            .unwrap();
        store
            .create_project(&sample_project_identity("same-id"))
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
        store
            .append_execution_queue_event(ExecutionQueueEvent::Enqueued {
                task_id: "same-id".to_string(),
                enqueued_event_seq: 1,
            })
            .unwrap();
        store
            .append_node_event("same-id", NodeEvent::BecomeReady)
            .unwrap();
        assert!(store.load_run_state("same-id").unwrap().is_some());
        assert!(store.load_project_state("same-id").unwrap().is_some());
        assert!(store.load_task_state("same-id").unwrap().is_some());
        assert!(store.load_contract_state("same-id").unwrap().is_some());
        assert!(store.load_graph_state("same-id").unwrap().is_some());
        assert!(store.load_execution_queue_state().unwrap().is_some());
        assert!(store.load_node_status("same-id").unwrap().is_some());
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

    #[test]
    fn unknown_execution_queue_has_no_projection() {
        let path = temp_db_path("unknown-execution-queue");
        let store = EventStore::open(&path).unwrap();
        assert!(store.load_execution_queue_state().unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn first_legal_execution_queue_event_creates_revision_one() {
        let path = temp_db_path("first-execution-queue-event");
        let mut store = EventStore::open(&path).unwrap();
        let appended = store
            .append_execution_queue_event(ExecutionQueueEvent::Enqueued {
                task_id: "task-1".to_string(),
                enqueued_event_seq: 1,
            })
            .unwrap();
        assert_eq!(appended.revision, 1);
        assert_eq!(appended.event_type, "Enqueued");
        let (revision, loaded) = store.load_execution_queue_state().unwrap().unwrap();
        assert_eq!(revision, 1);
        assert_eq!(loaded, appended.state);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn sequential_execution_queue_events_advance_revision_and_state() {
        use autome_domain::task::DispatchState;

        let path = temp_db_path("sequential-execution-queue");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_execution_queue_event(ExecutionQueueEvent::Enqueued {
                task_id: "task-1".to_string(),
                enqueued_event_seq: 1,
            })
            .unwrap();
        let second = store
            .append_execution_queue_event(ExecutionQueueEvent::LeaseAcquired {
                task_id: "task-1".to_string(),
                lease_id: "lease-1".to_string(),
            })
            .unwrap();
        assert_eq!(second.event_type, "LeaseAcquired");
        assert_eq!(
            second.state.dispatch_state_of("task-1"),
            DispatchState::Running
        );
        let (revision, _) = store.load_execution_queue_state().unwrap().unwrap();
        assert_eq!(revision, 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn illegal_execution_queue_transition_writes_nothing() {
        let path = temp_db_path("illegal-execution-queue");
        let mut store = EventStore::open(&path).unwrap();
        let err = store
            .append_execution_queue_event(ExecutionQueueEvent::LeaseAcquired {
                task_id: "task-1".to_string(),
                lease_id: "lease-1".to_string(),
            })
            .unwrap_err();
        assert!(matches!(err, ExecutionQueueAppendError::Transition(_)));
        assert!(store.load_execution_queue_state().unwrap().is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn execution_queue_state_survives_reconnect() {
        let path = temp_db_path("execution-queue-reconnect");
        {
            let mut store = EventStore::open(&path).unwrap();
            store
                .append_execution_queue_event(ExecutionQueueEvent::Enqueued {
                    task_id: "task-1".to_string(),
                    enqueued_event_seq: 1,
                })
                .unwrap();
            store
                .append_execution_queue_event(ExecutionQueueEvent::LeaseAcquired {
                    task_id: "task-1".to_string(),
                    lease_id: "lease-1".to_string(),
                })
                .unwrap();
        }
        let reopened = EventStore::open(&path).unwrap();
        let (revision, state) = reopened.load_execution_queue_state().unwrap().unwrap();
        assert_eq!(revision, 2);
        assert_eq!(state.lease().unwrap().lease_id, "lease-1");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn fresh_database_lands_on_the_latest_schema_version() {
        let path = temp_db_path("fresh-schema-version");
        let store = EventStore::open(&path).unwrap();
        let version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        std::fs::remove_file(&path).ok();
    }

    /// Simulates a v2 database (migrate_v1 + migrate_v2's shape, but not
    /// yet migrate_v3): `project_projections` exists with only the six
    /// pre-v3 columns and one row already journaled, `user_version` left
    /// at its default 0 so `migrate()` replays every step from the top —
    /// `migrate_v1`'s `IF NOT EXISTS` skips the manually-created table,
    /// `migrate_v3` still runs against it. Opening it through today's
    /// `EventStore` must add `display_name`/`identity_json` as NULL
    /// without disturbing the existing row (plan: no backfill source, a
    /// pre-migrate_v3 row's identity stays honestly unknown).
    #[test]
    fn opening_a_v2_database_adds_null_identity_columns_without_disturbing_existing_rows() {
        let path = temp_db_path("v2-migration");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "
                CREATE TABLE project_projections (
                    aggregate_id TEXT PRIMARY KEY,
                    revision INTEGER NOT NULL,
                    lifecycle TEXT NOT NULL,
                    phase TEXT NOT NULL,
                    hold TEXT NOT NULL,
                    state_json TEXT NOT NULL
                );
                ",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO project_projections (aggregate_id, revision, lifecycle, phase, hold, state_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "project-old",
                    1,
                    "Active",
                    "Registered",
                    "None",
                    r#"{"lifecycle":"Active","phase":"Registered","hold":"None","revision":1}"#
                ],
            )
            .unwrap();
        }

        let store = EventStore::open(&path).unwrap();
        let summaries = store.list_project_summaries().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "project-old");
        assert_eq!(summaries[0].display_name, None);
        assert_eq!(summaries[0].kind, None);
        assert!(
            store
                .load_project_identity("project-old")
                .unwrap()
                .is_none()
        );
        let version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        std::fs::remove_file(&path).ok();
    }

    /// Simulates a database written by pre-migration-framework code: the
    /// old `CREATE TABLE IF NOT EXISTS`-only scheme already ran (tables
    /// exist, `task_projections` has no `project_id` column, and
    /// `user_version` was never touched so it defaults to 0) with one Task
    /// row already journaled. Opening it through today's `EventStore`
    /// must backfill `project_id` from the row's own `state_json`, not
    /// leave it null.
    #[test]
    fn opening_a_pre_migration_database_backfills_task_project_id() {
        let path = temp_db_path("pre-migration-backfill");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "
                CREATE TABLE task_projections (
                    aggregate_id TEXT PRIMARY KEY,
                    revision INTEGER NOT NULL,
                    lifecycle TEXT NOT NULL,
                    state_json TEXT NOT NULL
                );
                ",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO task_projections (aggregate_id, revision, lifecycle, state_json)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "task-old",
                    1,
                    "Draft",
                    r#"{"project_id":"project-old","other":"ignored"}"#
                ],
            )
            .unwrap();
        }

        let store = EventStore::open(&path).unwrap();
        let summaries = store.list_task_summaries("project-old").unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "task-old");
        assert_eq!(summaries[0].lifecycle, "Draft");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn reopening_a_migrated_database_does_not_rerun_migrations() {
        let path = temp_db_path("no-rerun");
        {
            EventStore::open(&path).unwrap();
        }
        // A second open must not attempt `ALTER TABLE ... ADD COLUMN
        // project_id` again (that would error with a duplicate-column
        // SqliteFailure) — proving each step only ever runs once.
        let store = EventStore::open(&path).unwrap();
        let version: i64 = store
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn latest_event_seq_is_zero_for_an_empty_journal_and_tracks_appends() {
        let path = temp_db_path("latest-event-seq");
        let mut store = EventStore::open(&path).unwrap();
        assert_eq!(store.latest_event_seq().unwrap(), 0);
        store
            .append_run_event("run-1", RunEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(store.latest_event_seq().unwrap(), 1);
        store
            .create_project(&sample_project_identity("project-1"))
            .unwrap();
        assert_eq!(store.latest_event_seq().unwrap(), 2);
        store
            .append_project_event("project-1", ProjectEvent::AdvanceNominal)
            .unwrap();
        assert_eq!(store.latest_event_seq().unwrap(), 3);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn list_project_summaries_reflects_every_known_project_in_order() {
        let path = temp_db_path("list-project-summaries");
        let mut store = EventStore::open(&path).unwrap();
        store
            .create_project(&sample_project_identity("project-b"))
            .unwrap();
        store
            .create_project(&sample_project_identity("project-a"))
            .unwrap();
        let summaries = store.list_project_summaries().unwrap();
        assert_eq!(
            summaries.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["project-a", "project-b"]
        );
        assert_eq!(summaries[0].revision, 1);
        assert_eq!(
            summaries[0].display_name.as_deref(),
            Some("Display project-a")
        );
        assert_eq!(
            summaries[0].kind,
            Some(autome_domain::project::ProjectKind::ExistingRepository)
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn list_task_summaries_is_isolated_per_project() {
        let path = temp_db_path("list-task-summaries-isolated");
        let mut store = EventStore::open(&path).unwrap();
        store
            .append_task_event(
                "task-1",
                TaskEvent::Created {
                    id: "task-1".to_string(),
                    project: ready_project_state(),
                    project_id: "project-a".to_string(),
                    project_revision: 1,
                    original_request_ref: "ref-1".to_string(),
                },
            )
            .unwrap();
        store
            .append_task_event(
                "task-2",
                TaskEvent::Created {
                    id: "task-2".to_string(),
                    project: ready_project_state(),
                    project_id: "project-b".to_string(),
                    project_revision: 1,
                    original_request_ref: "ref-2".to_string(),
                },
            )
            .unwrap();

        let project_a_tasks = store.list_task_summaries("project-a").unwrap();
        assert_eq!(
            project_a_tasks
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            vec!["task-1"]
        );
        let project_b_tasks = store.list_task_summaries("project-b").unwrap();
        assert_eq!(
            project_b_tasks
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            vec!["task-2"]
        );
        let unknown_project_tasks = store.list_task_summaries("project-nonexistent").unwrap();
        assert!(unknown_project_tasks.is_empty());
        std::fs::remove_file(&path).ok();
    }
}
