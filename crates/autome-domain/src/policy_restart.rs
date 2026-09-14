//! PlanningPolicyRestart / RunPolicyAmendment / BudgetGrantReceipt per plan
//! §5.1's table of what a config/skill/budget/environment change does to an
//! in-flight Task.
//!
//! Shared invariant across the first two: "任何 restart/amendment 都先展示
//! 新旧 spec、影响范围和失效对象" — enforced by requiring the old and
//! proposed spec hashes to actually differ (a restart/amendment that
//! changes nothing has no reason to exist) and by requiring a non-empty
//! `approval_receipt`, matching the explicit-user-decision pattern used
//! throughout this crate (`ContractAmendment::user_decision_ref`,
//! `HumanReviewFinding`'s resolution refs, etc.).
//!
//! `RunPolicyAmendment` additionally encodes "2.0.0 不跨 Run 复用 Attempt
//! ...；策略变化即使只影响最后一步，也从冻结 base 重新执行全部实现、验证和
//! 审计": at least one prior Attempt must actually be listed as invalidated,
//! since a policy amendment that invalidates nothing would contradict that
//! rule.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanningPolicyRestart {
    pub task_id: String,
    pub current_run_id: String,
    pub old_planning_spec_hash: String,
    pub proposed_planning_spec_hash: String,
    pub trigger_revision_ref: String,
    pub config_revision_ref: String,
    pub skill_revision_ref: String,
    pub capability_revision_ref: String,
    pub invalidated_document_attempt_ids: Vec<String>,
    pub approval_receipt: String,
    pub restart_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanningPolicyRestartError {
    PlanningSpecUnchanged,
    MissingApprovalReceipt,
}

/// Sole constructor for `PlanningPolicyRestart`. `old_planning_spec_hash`
/// and `proposed_planning_spec_hash` must differ, and `approval_receipt`
/// must be non-empty.
#[allow(clippy::too_many_arguments)]
pub fn issue_planning_policy_restart(
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
) -> Result<PlanningPolicyRestart, Vec<PlanningPolicyRestartError>> {
    let mut errors = Vec::new();
    if old_planning_spec_hash == proposed_planning_spec_hash {
        errors.push(PlanningPolicyRestartError::PlanningSpecUnchanged);
    }
    if approval_receipt.trim().is_empty() {
        errors.push(PlanningPolicyRestartError::MissingApprovalReceipt);
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(PlanningPolicyRestart {
        task_id: task_id.to_string(),
        current_run_id: current_run_id.to_string(),
        old_planning_spec_hash: old_planning_spec_hash.to_string(),
        proposed_planning_spec_hash: proposed_planning_spec_hash.to_string(),
        trigger_revision_ref: trigger_revision_ref.to_string(),
        config_revision_ref: config_revision_ref.to_string(),
        skill_revision_ref: skill_revision_ref.to_string(),
        capability_revision_ref: capability_revision_ref.to_string(),
        invalidated_document_attempt_ids,
        approval_receipt: approval_receipt.to_string(),
        restart_digest: restart_digest.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPolicyAmendment {
    pub task_id: String,
    pub current_run_id: String,
    pub old_execution_spec_hash: String,
    pub proposed_execution_spec_hash: String,
    pub unchanged_contract_hash: String,
    pub unchanged_graph_hash: String,
    pub unchanged_base_hash: String,
    pub policy_diff: String,
    pub invalidated_attempt_ids: Vec<String>,
    pub invalidated_evidence_ids: Vec<String>,
    pub invalidated_audit_ids: Vec<String>,
    pub invalidated_candidate_ids: Vec<String>,
    pub approval_receipt: String,
    pub amendment_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunPolicyAmendmentError {
    ExecutionSpecUnchanged,
    MissingPolicyDiff,
    MissingApprovalReceipt,
    NoInvalidatedAttempts,
}

/// Sole constructor for `RunPolicyAmendment`. Refuses to construct one that
/// invalidates zero prior Attempts, since "重新执行全部实现、验证和审计"
/// means there is always at least the prior implementation Attempt to
/// invalidate — an amendment that invalidates nothing would just be
/// silently reusing old Attempts across Runs, which 2.0.0 does not permit.
#[allow(clippy::too_many_arguments)]
pub fn issue_run_policy_amendment(
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
) -> Result<RunPolicyAmendment, Vec<RunPolicyAmendmentError>> {
    let mut errors = Vec::new();
    if old_execution_spec_hash == proposed_execution_spec_hash {
        errors.push(RunPolicyAmendmentError::ExecutionSpecUnchanged);
    }
    if policy_diff.trim().is_empty() {
        errors.push(RunPolicyAmendmentError::MissingPolicyDiff);
    }
    if approval_receipt.trim().is_empty() {
        errors.push(RunPolicyAmendmentError::MissingApprovalReceipt);
    }
    if invalidated_attempt_ids.is_empty() {
        errors.push(RunPolicyAmendmentError::NoInvalidatedAttempts);
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(RunPolicyAmendment {
        task_id: task_id.to_string(),
        current_run_id: current_run_id.to_string(),
        old_execution_spec_hash: old_execution_spec_hash.to_string(),
        proposed_execution_spec_hash: proposed_execution_spec_hash.to_string(),
        unchanged_contract_hash: unchanged_contract_hash.to_string(),
        unchanged_graph_hash: unchanged_graph_hash.to_string(),
        unchanged_base_hash: unchanged_base_hash.to_string(),
        policy_diff: policy_diff.to_string(),
        invalidated_attempt_ids,
        invalidated_evidence_ids,
        invalidated_audit_ids,
        invalidated_candidate_ids,
        approval_receipt: approval_receipt.to_string(),
        amendment_digest: amendment_digest.to_string(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetLimitGrant {
    Hard(u32),
    Soft(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetGrantReceipt {
    pub run_id: String,
    pub current_budget_hash: String,
    pub added_limits: Vec<BudgetLimitGrant>,
    pub reason: String,
    pub operator: String,
    pub expiry: Option<String>,
    pub grant_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BudgetGrantError {
    NoLimitsAdded,
    MissingReason,
    MissingOperator,
}

/// Sole constructor for `BudgetGrantReceipt`. A grant that adds no limits,
/// gives no reason, or names no operator is refused — this is meant to be
/// an explicit, accountable exception to the Run's frozen budget, not a
/// silent bump.
pub fn issue_budget_grant_receipt(
    run_id: &str,
    current_budget_hash: &str,
    added_limits: Vec<BudgetLimitGrant>,
    reason: &str,
    operator: &str,
    expiry: Option<&str>,
    grant_digest: &str,
) -> Result<BudgetGrantReceipt, Vec<BudgetGrantError>> {
    let mut errors = Vec::new();
    if added_limits.is_empty() {
        errors.push(BudgetGrantError::NoLimitsAdded);
    }
    if reason.trim().is_empty() {
        errors.push(BudgetGrantError::MissingReason);
    }
    if operator.trim().is_empty() {
        errors.push(BudgetGrantError::MissingOperator);
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(BudgetGrantReceipt {
        run_id: run_id.to_string(),
        current_budget_hash: current_budget_hash.to_string(),
        added_limits,
        reason: reason.to_string(),
        operator: operator.to_string(),
        expiry: expiry.map(|s| s.to_string()),
        grant_digest: grant_digest.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_restart() -> Result<PlanningPolicyRestart, Vec<PlanningPolicyRestartError>> {
        issue_planning_policy_restart(
            "task-1",
            "run-1",
            "old-planning-spec-hash",
            "new-planning-spec-hash",
            "trigger-rev-1",
            "config-rev-1",
            "skill-rev-1",
            "capability-rev-1",
            vec!["doc-attempt-1".into()],
            "approval-1",
            "restart-digest-1",
        )
    }

    #[test]
    fn valid_planning_policy_restart_is_issued() {
        let restart = valid_restart().unwrap();
        assert_eq!(restart.old_planning_spec_hash, "old-planning-spec-hash");
    }

    #[test]
    fn restart_rejects_unchanged_planning_spec() {
        let errors = issue_planning_policy_restart(
            "task-1",
            "run-1",
            "same-hash",
            "same-hash",
            "trigger-rev-1",
            "config-rev-1",
            "skill-rev-1",
            "capability-rev-1",
            vec![],
            "approval-1",
            "restart-digest-1",
        )
        .unwrap_err();
        assert_eq!(
            errors,
            vec![PlanningPolicyRestartError::PlanningSpecUnchanged]
        );
    }

    #[test]
    fn restart_rejects_missing_approval_receipt() {
        let errors = issue_planning_policy_restart(
            "task-1",
            "run-1",
            "old-hash",
            "new-hash",
            "trigger-rev-1",
            "config-rev-1",
            "skill-rev-1",
            "capability-rev-1",
            vec![],
            "",
            "restart-digest-1",
        )
        .unwrap_err();
        assert_eq!(
            errors,
            vec![PlanningPolicyRestartError::MissingApprovalReceipt]
        );
    }

    fn valid_amendment() -> Result<RunPolicyAmendment, Vec<RunPolicyAmendmentError>> {
        issue_run_policy_amendment(
            "task-1",
            "run-1",
            "old-execution-spec-hash",
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
            "amendment-digest-1",
        )
    }

    #[test]
    fn valid_run_policy_amendment_is_issued() {
        let amendment = valid_amendment().unwrap();
        assert_eq!(amendment.invalidated_attempt_ids, vec!["attempt-1"]);
    }

    #[test]
    fn amendment_rejects_unchanged_execution_spec() {
        let errors = issue_run_policy_amendment(
            "task-1",
            "run-1",
            "same-hash",
            "same-hash",
            "contract-hash-1",
            "graph-hash-1",
            "base-hash-1",
            "diff",
            vec!["attempt-1".into()],
            vec![],
            vec![],
            vec![],
            "approval-1",
            "amendment-digest-1",
        )
        .unwrap_err();
        assert!(errors.contains(&RunPolicyAmendmentError::ExecutionSpecUnchanged));
    }

    #[test]
    fn amendment_rejects_no_invalidated_attempts() {
        let errors = issue_run_policy_amendment(
            "task-1",
            "run-1",
            "old-hash",
            "new-hash",
            "contract-hash-1",
            "graph-hash-1",
            "base-hash-1",
            "diff",
            vec![],
            vec![],
            vec![],
            vec![],
            "approval-1",
            "amendment-digest-1",
        )
        .unwrap_err();
        assert!(errors.contains(&RunPolicyAmendmentError::NoInvalidatedAttempts));
    }

    #[test]
    fn amendment_rejects_empty_policy_diff() {
        let errors = issue_run_policy_amendment(
            "task-1",
            "run-1",
            "old-hash",
            "new-hash",
            "contract-hash-1",
            "graph-hash-1",
            "base-hash-1",
            "",
            vec!["attempt-1".into()],
            vec![],
            vec![],
            vec![],
            "approval-1",
            "amendment-digest-1",
        )
        .unwrap_err();
        assert!(errors.contains(&RunPolicyAmendmentError::MissingPolicyDiff));
    }

    #[test]
    fn amendment_reports_every_violation_together() {
        let errors = issue_run_policy_amendment(
            "task-1",
            "run-1",
            "same-hash",
            "same-hash",
            "contract-hash-1",
            "graph-hash-1",
            "base-hash-1",
            "",
            vec![],
            vec![],
            vec![],
            vec![],
            "",
            "amendment-digest-1",
        )
        .unwrap_err();
        assert_eq!(errors.len(), 4);
    }

    #[test]
    fn valid_budget_grant_is_issued() {
        let grant = issue_budget_grant_receipt(
            "run-1",
            "budget-hash-1",
            vec![BudgetLimitGrant::Soft(10)],
            "extra retries needed after flaky environment",
            "dannie",
            None,
            "grant-digest-1",
        )
        .unwrap();
        assert_eq!(grant.added_limits, vec![BudgetLimitGrant::Soft(10)]);
    }

    #[test]
    fn budget_grant_rejects_no_limits_added() {
        let errors = issue_budget_grant_receipt(
            "run-1",
            "budget-hash-1",
            vec![],
            "reason",
            "dannie",
            None,
            "grant-digest-1",
        )
        .unwrap_err();
        assert_eq!(errors, vec![BudgetGrantError::NoLimitsAdded]);
    }

    #[test]
    fn budget_grant_reports_every_violation_together() {
        let errors = issue_budget_grant_receipt(
            "run-1",
            "budget-hash-1",
            vec![],
            "",
            "",
            None,
            "grant-digest-1",
        )
        .unwrap_err();
        assert_eq!(errors.len(), 3);
    }
}
