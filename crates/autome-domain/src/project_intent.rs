//! ProjectIntentRevision / ProjectIntentAmendment / ProjectInitializationReceipt
//! per plan §5.1.
//!
//! "已有仓库初始化时 Core 可从 README/规则/当前行为提出候选，但用户必须确认，
//! 不能把模型推断直接升为产品原则" is enforced by requiring a non-empty
//! `approved_by`/`approved_at` on every `ProjectIntentRevision` — there is no
//! constructor here that turns a model-proposed candidate directly into an
//! authoritative revision without a human in that field.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct IntentRevision(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyDecision {
    pub id: String,
    pub statement: String,
    pub rationale: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIntentRevision {
    pub project_id: String,
    pub revision: IntentRevision,
    pub source_anchors: Vec<String>,
    pub approved_by: String,
    pub approved_at: String,
    pub product_goal: String,
    pub target_users: Vec<String>,
    pub durable_cross_task_constraints: Vec<String>,
    pub explicit_non_goals: Vec<String>,
    pub key_decisions: Vec<KeyDecision>,
    pub supersedes: Option<IntentRevision>,
    pub intent_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectIntentError {
    MissingApprovedBy,
    MissingApprovedAt,
    MissingSourceAnchor,
    MissingProductGoal,
    IncompleteKeyDecision { id: String },
}

/// Sole constructor for `ProjectIntentRevision`. Collects every violation
/// rather than stopping at the first, matching this crate's other
/// multi-error validation gates.
#[allow(clippy::too_many_arguments)]
pub fn issue_project_intent_revision(
    project_id: &str,
    revision: IntentRevision,
    source_anchors: Vec<String>,
    approved_by: &str,
    approved_at: &str,
    product_goal: &str,
    target_users: Vec<String>,
    durable_cross_task_constraints: Vec<String>,
    explicit_non_goals: Vec<String>,
    key_decisions: Vec<KeyDecision>,
    supersedes: Option<IntentRevision>,
    intent_hash: &str,
) -> Result<ProjectIntentRevision, Vec<ProjectIntentError>> {
    let mut errors = Vec::new();
    if approved_by.trim().is_empty() {
        errors.push(ProjectIntentError::MissingApprovedBy);
    }
    if approved_at.trim().is_empty() {
        errors.push(ProjectIntentError::MissingApprovedAt);
    }
    if source_anchors.is_empty() {
        errors.push(ProjectIntentError::MissingSourceAnchor);
    }
    if product_goal.trim().is_empty() {
        errors.push(ProjectIntentError::MissingProductGoal);
    }
    for decision in &key_decisions {
        if decision.id.trim().is_empty()
            || decision.statement.trim().is_empty()
            || decision.rationale.trim().is_empty()
            || decision.source_ref.trim().is_empty()
        {
            errors.push(ProjectIntentError::IncompleteKeyDecision {
                id: decision.id.clone(),
            });
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ProjectIntentRevision {
        project_id: project_id.to_string(),
        revision,
        source_anchors,
        approved_by: approved_by.to_string(),
        approved_at: approved_at.to_string(),
        product_goal: product_goal.to_string(),
        target_users,
        durable_cross_task_constraints,
        explicit_non_goals,
        key_decisions,
        supersedes,
        intent_hash: intent_hash.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIntentAmendmentRequest {
    pub project_id: String,
    pub from_revision: IntentRevision,
    pub trigger_task: Option<String>,
    pub semantic_diff: String,
    pub affected_active_tasks: Vec<String>,
    pub user_decision_receipt: String,
    pub amendment_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIntentAmendment {
    pub project_id: String,
    pub from_revision: IntentRevision,
    pub to_revision: IntentRevision,
    pub trigger_task: Option<String>,
    pub semantic_diff: String,
    pub affected_active_tasks: Vec<String>,
    pub user_decision_receipt: String,
    pub amendment_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectIntentAmendmentError {
    ProjectMismatch,
    FromRevisionMismatch {
        expected: IntentRevision,
        actual: IntentRevision,
    },
    MissingSemanticDiff,
    MissingUserDecisionReceipt,
}

/// Sole way to record an intent amendment against a *current* revision.
/// Mirrors `contract::apply_amendment`'s base-version check: the request
/// must name the revision it amends, and the produced `to_revision` is
/// always exactly one past it — never picked by the caller. Building the
/// next authoritative `ProjectIntentRevision` (with `supersedes` set to
/// `to_revision`'s predecessor) is a separate call to
/// `issue_project_intent_revision`; this function only records the change
/// itself and hands back the revision number to use for it.
pub fn apply_project_intent_amendment(
    current: &ProjectIntentRevision,
    request: ProjectIntentAmendmentRequest,
) -> Result<ProjectIntentAmendment, ProjectIntentAmendmentError> {
    if current.project_id != request.project_id {
        return Err(ProjectIntentAmendmentError::ProjectMismatch);
    }
    if current.revision != request.from_revision {
        return Err(ProjectIntentAmendmentError::FromRevisionMismatch {
            expected: current.revision,
            actual: request.from_revision,
        });
    }
    if request.semantic_diff.trim().is_empty() {
        return Err(ProjectIntentAmendmentError::MissingSemanticDiff);
    }
    if request.user_decision_receipt.trim().is_empty() {
        return Err(ProjectIntentAmendmentError::MissingUserDecisionReceipt);
    }

    Ok(ProjectIntentAmendment {
        project_id: request.project_id,
        from_revision: request.from_revision,
        to_revision: IntentRevision(current.revision.0 + 1),
        trigger_task: request.trigger_task,
        semantic_diff: request.semantic_diff,
        affected_active_tasks: request.affected_active_tasks,
        user_decision_receipt: request.user_decision_receipt,
        amendment_hash: request.amendment_hash,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitializationResult {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInitializationReceipt {
    pub project_id: String,
    pub project_revision: u32,
    pub subject_identity_hash: String,
    pub trust_decision_ref: Option<String>,
    pub environment_snapshot_id: String,
    pub skill_inventory_id: String,
    pub project_home_manifest: String,
    pub result: InitializationResult,
    pub issues: Vec<String>,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectInitializationError {
    BlockedResultRequiresAtLeastOneIssue,
    ReadyResultCannotHaveIssues,
}

/// Sole constructor for `ProjectInitializationReceipt`. A `Blocked` result
/// with no issues (nothing for the user to act on) and a `Ready` result
/// that still lists issues (contradicts its own verdict) are both refused
/// rather than accepted as-is — the plan's `result`/`issues` pair must
/// actually agree with each other.
#[allow(clippy::too_many_arguments)]
pub fn issue_project_initialization_receipt(
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
) -> Result<ProjectInitializationReceipt, ProjectInitializationError> {
    match result {
        InitializationResult::Blocked if issues.is_empty() => {
            return Err(ProjectInitializationError::BlockedResultRequiresAtLeastOneIssue);
        }
        InitializationResult::Ready if !issues.is_empty() => {
            return Err(ProjectInitializationError::ReadyResultCannotHaveIssues);
        }
        _ => {}
    }

    Ok(ProjectInitializationReceipt {
        project_id: project_id.to_string(),
        project_revision,
        subject_identity_hash: subject_identity_hash.to_string(),
        trust_decision_ref: trust_decision_ref.map(|s| s.to_string()),
        environment_snapshot_id: environment_snapshot_id.to_string(),
        skill_inventory_id: skill_inventory_id.to_string(),
        project_home_manifest: project_home_manifest.to_string(),
        result,
        issues,
        receipt_digest: receipt_digest.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_key_decision() -> KeyDecision {
        KeyDecision {
            id: "kd-1".into(),
            statement: "Use SQLite for the event journal".into(),
            rationale: "Local-first, single-user, no server dependency".into(),
            source_ref: "docs/plan.md#L42".into(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn issue_valid_revision(
        revision: u32,
        approved_by: &str,
        supersedes: Option<IntentRevision>,
    ) -> Result<ProjectIntentRevision, Vec<ProjectIntentError>> {
        issue_project_intent_revision(
            "project-1",
            IntentRevision(revision),
            vec!["README.md".into()],
            approved_by,
            "2026-09-14T00:00:00Z",
            "Ship a local-first digital employee",
            vec!["solo developers".into()],
            vec!["never phone home".into()],
            vec!["no multi-tenant support".into()],
            vec![valid_key_decision()],
            supersedes,
            "intent-hash-1",
        )
    }

    #[test]
    fn valid_intent_revision_is_issued() {
        let revision = issue_valid_revision(1, "dannie", None).unwrap();
        assert_eq!(revision.revision, IntentRevision(1));
        assert_eq!(revision.key_decisions.len(), 1);
    }

    #[test]
    fn empty_approved_by_is_rejected() {
        let errors = issue_valid_revision(1, "", None).unwrap_err();
        assert_eq!(errors, vec![ProjectIntentError::MissingApprovedBy]);
    }

    #[test]
    fn empty_source_anchors_is_rejected() {
        let errors = issue_project_intent_revision(
            "project-1",
            IntentRevision(1),
            vec![],
            "dannie",
            "2026-09-14T00:00:00Z",
            "Ship a local-first digital employee",
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            "intent-hash-1",
        )
        .unwrap_err();
        assert_eq!(errors, vec![ProjectIntentError::MissingSourceAnchor]);
    }

    #[test]
    fn incomplete_key_decision_is_rejected() {
        let mut incomplete = valid_key_decision();
        incomplete.rationale = "".into();
        let errors = issue_project_intent_revision(
            "project-1",
            IntentRevision(1),
            vec!["README.md".into()],
            "dannie",
            "2026-09-14T00:00:00Z",
            "Ship a local-first digital employee",
            vec![],
            vec![],
            vec![],
            vec![incomplete],
            None,
            "intent-hash-1",
        )
        .unwrap_err();
        assert_eq!(
            errors,
            vec![ProjectIntentError::IncompleteKeyDecision { id: "kd-1".into() }]
        );
    }

    #[test]
    fn every_violation_is_reported_together() {
        let errors = issue_project_intent_revision(
            "project-1",
            IntentRevision(1),
            vec![],
            "",
            "",
            "",
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            "intent-hash-1",
        )
        .unwrap_err();
        assert_eq!(errors.len(), 4);
    }

    fn amendment_request(from_revision: u32) -> ProjectIntentAmendmentRequest {
        ProjectIntentAmendmentRequest {
            project_id: "project-1".into(),
            from_revision: IntentRevision(from_revision),
            trigger_task: Some("task-9".into()),
            semantic_diff: "Added a non-goal: no team accounts in 2.0.0".into(),
            affected_active_tasks: vec!["task-9".into()],
            user_decision_receipt: "decision-ref-1".into(),
            amendment_hash: "amendment-hash-1".into(),
        }
    }

    #[test]
    fn amendment_produces_the_next_revision_number() {
        let current = issue_valid_revision(1, "dannie", None).unwrap();
        let amendment = apply_project_intent_amendment(&current, amendment_request(1)).unwrap();
        assert_eq!(amendment.from_revision, IntentRevision(1));
        assert_eq!(amendment.to_revision, IntentRevision(2));
    }

    #[test]
    fn amendment_rejects_mismatched_project_id() {
        let current = issue_valid_revision(1, "dannie", None).unwrap();
        let mut request = amendment_request(1);
        request.project_id = "project-2".into();
        let err = apply_project_intent_amendment(&current, request).unwrap_err();
        assert_eq!(err, ProjectIntentAmendmentError::ProjectMismatch);
    }

    #[test]
    fn amendment_rejects_stale_from_revision() {
        let current = issue_valid_revision(3, "dannie", None).unwrap();
        let err = apply_project_intent_amendment(&current, amendment_request(1)).unwrap_err();
        assert_eq!(
            err,
            ProjectIntentAmendmentError::FromRevisionMismatch {
                expected: IntentRevision(3),
                actual: IntentRevision(1),
            }
        );
    }

    #[test]
    fn amendment_rejects_empty_user_decision_receipt() {
        let current = issue_valid_revision(1, "dannie", None).unwrap();
        let mut request = amendment_request(1);
        request.user_decision_receipt = "".into();
        let err = apply_project_intent_amendment(&current, request).unwrap_err();
        assert_eq!(err, ProjectIntentAmendmentError::MissingUserDecisionReceipt);
    }

    #[test]
    fn amendment_rejects_empty_semantic_diff() {
        let current = issue_valid_revision(1, "dannie", None).unwrap();
        let mut request = amendment_request(1);
        request.semantic_diff = "".into();
        let err = apply_project_intent_amendment(&current, request).unwrap_err();
        assert_eq!(err, ProjectIntentAmendmentError::MissingSemanticDiff);
    }

    #[test]
    fn next_revision_can_supersede_the_current_one() {
        let current = issue_valid_revision(1, "dannie", None).unwrap();
        let amendment = apply_project_intent_amendment(&current, amendment_request(1)).unwrap();
        let next = issue_valid_revision(amendment.to_revision.0, "dannie", Some(current.revision))
            .unwrap();
        assert_eq!(next.supersedes, Some(IntentRevision(1)));
    }

    fn issue_receipt(
        result: InitializationResult,
        issues: Vec<String>,
    ) -> Result<ProjectInitializationReceipt, ProjectInitializationError> {
        issue_project_initialization_receipt(
            "project-1",
            1,
            "identity-hash-1",
            None,
            "env-snapshot-1",
            "skill-inventory-1",
            "manifest-1",
            result,
            issues,
            "receipt-digest-1",
        )
    }

    #[test]
    fn ready_with_no_issues_is_issued() {
        let receipt = issue_receipt(InitializationResult::Ready, vec![]).unwrap();
        assert_eq!(receipt.result, InitializationResult::Ready);
    }

    #[test]
    fn blocked_with_no_issues_is_rejected() {
        let err = issue_receipt(InitializationResult::Blocked, vec![]).unwrap_err();
        assert_eq!(
            err,
            ProjectInitializationError::BlockedResultRequiresAtLeastOneIssue
        );
    }

    #[test]
    fn ready_with_issues_is_rejected() {
        let err = issue_receipt(
            InitializationResult::Ready,
            vec!["unexpected dirty worktree".into()],
        )
        .unwrap_err();
        assert_eq!(err, ProjectInitializationError::ReadyResultCannotHaveIssues);
    }

    #[test]
    fn blocked_with_issues_is_issued() {
        let receipt = issue_receipt(
            InitializationResult::Blocked,
            vec!["environment not qualified".into()],
        )
        .unwrap();
        assert_eq!(receipt.issues.len(), 1);
    }
}
