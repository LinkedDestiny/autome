//! UserCorrectionReceipt per plan §6.5.
//!
//! Two mechanical rules from the plan text, both enforced here rather than
//! left as caller convention:
//!
//! 1. "尚未冻结 ExecutionRunSpec 时，任何纠偏都归入 planning_revision" /
//!    "已冻结 ExecutionRunSpec 后，contract_preserving_execution 才能..." —
//!    `execution_spec_frozen` and `classification` must agree: pre-freeze
//!    only `PlanningRevision` is legal, post-freeze `PlanningRevision` is
//!    no longer legal (planning is already done).
//! 2. "任何会删除 Requirement、放宽 Check、改 ProjectIntent 或新增外部副
//!    作用的纠偏都不能归为 contract-preserving" — `ContractPreservingExecution`
//!    requires every `CorrectionImpactFlags` field to be false.
//!
//! `classification` and `disposition` are a fixed 1:1 mapping — the plan
//! gives each classification exactly one routing outcome, so a mismatched
//! pair (e.g. `GraphStrategy` routed as `ContractAmendmentRequired`) is
//! rejected rather than left to caller discipline. `Ambiguous` always maps
//! to `AwaitingUserChoice`, which is how "Agent 不得替用户定级" is
//! represented: there is no disposition variant that lets an agent resolve
//! an ambiguous correction on the user's behalf.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrectionClassification {
    PlanningRevision,
    ContractPreservingExecution,
    GraphStrategy,
    ContractSemanticChange,
    NewExternalFact,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrectionDisposition {
    NewPlanningRunSpec,
    NewRepairOrImplementationAttempt,
    ReplanProposal,
    ContractAmendmentRequired,
    RoutedThroughFactChannel,
    AwaitingUserChoice,
}

impl CorrectionClassification {
    pub fn expected_disposition(self) -> CorrectionDisposition {
        match self {
            CorrectionClassification::PlanningRevision => CorrectionDisposition::NewPlanningRunSpec,
            CorrectionClassification::ContractPreservingExecution => {
                CorrectionDisposition::NewRepairOrImplementationAttempt
            }
            CorrectionClassification::GraphStrategy => CorrectionDisposition::ReplanProposal,
            CorrectionClassification::ContractSemanticChange => {
                CorrectionDisposition::ContractAmendmentRequired
            }
            CorrectionClassification::NewExternalFact => {
                CorrectionDisposition::RoutedThroughFactChannel
            }
            CorrectionClassification::Ambiguous => CorrectionDisposition::AwaitingUserChoice,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CorrectionImpactFlags {
    pub would_delete_requirement: bool,
    pub would_relax_check: bool,
    pub would_change_project_intent: bool,
    pub adds_external_side_effect: bool,
}

impl CorrectionImpactFlags {
    pub fn is_contract_preserving(&self) -> bool {
        !self.would_delete_requirement
            && !self.would_relax_check
            && !self.would_change_project_intent
            && !self.adds_external_side_effect
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCorrectionReceipt {
    pub project_id: String,
    pub task_id: String,
    pub run_id: String,
    pub attempt_id: Option<String>,
    pub planning_spec_hash: String,
    pub execution_spec_hash: Option<String>,
    pub raw_text_ref: String,
    pub attachment_hashes: Vec<String>,
    pub submitted_at: String,
    pub operator: String,
    pub subject_contract_hash: Option<String>,
    pub subject_graph_hash: Option<String>,
    pub subject_candidate_hash: Option<String>,
    pub classification: CorrectionClassification,
    pub affected_requirement_ids: Vec<String>,
    pub affected_node_ids: Vec<String>,
    pub disposition: CorrectionDisposition,
    pub successor_ref: Option<String>,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCorrectionError {
    PreFreezeMustBePlanningRevision,
    PostFreezeCannotUsePlanningRevision,
    ClassificationDispositionMismatch {
        expected: CorrectionDisposition,
        actual: CorrectionDisposition,
    },
    ContractPreservingExecutionRequiresNoContractImpact,
}

/// Sole constructor for `UserCorrectionReceipt`. Collects every violation
/// rather than stopping at the first.
#[allow(clippy::too_many_arguments)]
pub fn issue_user_correction_receipt(
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
) -> Result<UserCorrectionReceipt, Vec<UserCorrectionError>> {
    let mut errors = Vec::new();

    if !execution_spec_frozen && classification != CorrectionClassification::PlanningRevision {
        errors.push(UserCorrectionError::PreFreezeMustBePlanningRevision);
    }
    if execution_spec_frozen && classification == CorrectionClassification::PlanningRevision {
        errors.push(UserCorrectionError::PostFreezeCannotUsePlanningRevision);
    }

    let expected = classification.expected_disposition();
    if disposition != expected {
        errors.push(UserCorrectionError::ClassificationDispositionMismatch {
            expected,
            actual: disposition,
        });
    }

    if classification == CorrectionClassification::ContractPreservingExecution
        && !impact.is_contract_preserving()
    {
        errors.push(UserCorrectionError::ContractPreservingExecutionRequiresNoContractImpact);
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(UserCorrectionReceipt {
        project_id: project_id.to_string(),
        task_id: task_id.to_string(),
        run_id: run_id.to_string(),
        attempt_id: attempt_id.map(|s| s.to_string()),
        planning_spec_hash: planning_spec_hash.to_string(),
        execution_spec_hash: execution_spec_hash.map(|s| s.to_string()),
        raw_text_ref: raw_text_ref.to_string(),
        attachment_hashes,
        submitted_at: submitted_at.to_string(),
        operator: operator.to_string(),
        subject_contract_hash: subject_contract_hash.map(|s| s.to_string()),
        subject_graph_hash: subject_graph_hash.map(|s| s.to_string()),
        subject_candidate_hash: subject_candidate_hash.map(|s| s.to_string()),
        classification,
        affected_requirement_ids,
        affected_node_ids,
        disposition,
        successor_ref: successor_ref.map(|s| s.to_string()),
        receipt_digest: receipt_digest.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn issue(
        execution_spec_frozen: bool,
        classification: CorrectionClassification,
        impact: CorrectionImpactFlags,
        disposition: CorrectionDisposition,
    ) -> Result<UserCorrectionReceipt, Vec<UserCorrectionError>> {
        issue_user_correction_receipt(
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
            "receipt-digest-1",
        )
    }

    #[test]
    fn pre_freeze_planning_revision_is_issued() {
        let receipt = issue(
            false,
            CorrectionClassification::PlanningRevision,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::NewPlanningRunSpec,
        )
        .unwrap();
        assert_eq!(
            receipt.classification,
            CorrectionClassification::PlanningRevision
        );
    }

    #[test]
    fn pre_freeze_non_planning_classification_is_rejected() {
        let errors = issue(
            false,
            CorrectionClassification::GraphStrategy,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::ReplanProposal,
        )
        .unwrap_err();
        assert!(errors.contains(&UserCorrectionError::PreFreezeMustBePlanningRevision));
    }

    #[test]
    fn post_freeze_planning_revision_is_rejected() {
        let errors = issue(
            true,
            CorrectionClassification::PlanningRevision,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::NewPlanningRunSpec,
        )
        .unwrap_err();
        assert!(errors.contains(&UserCorrectionError::PostFreezeCannotUsePlanningRevision));
    }

    #[test]
    fn post_freeze_contract_preserving_execution_with_no_impact_is_issued() {
        let receipt = issue(
            true,
            CorrectionClassification::ContractPreservingExecution,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::NewRepairOrImplementationAttempt,
        )
        .unwrap();
        assert_eq!(
            receipt.disposition,
            CorrectionDisposition::NewRepairOrImplementationAttempt
        );
    }

    #[test]
    fn contract_preserving_execution_with_requirement_deletion_impact_is_rejected() {
        let errors = issue(
            true,
            CorrectionClassification::ContractPreservingExecution,
            CorrectionImpactFlags {
                would_delete_requirement: true,
                ..Default::default()
            },
            CorrectionDisposition::NewRepairOrImplementationAttempt,
        )
        .unwrap_err();
        assert!(
            errors.contains(
                &UserCorrectionError::ContractPreservingExecutionRequiresNoContractImpact
            )
        );
    }

    #[test]
    fn mismatched_classification_and_disposition_is_rejected() {
        let errors = issue(
            true,
            CorrectionClassification::GraphStrategy,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::ContractAmendmentRequired,
        )
        .unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            UserCorrectionError::ClassificationDispositionMismatch { .. }
        )));
    }

    #[test]
    fn ambiguous_always_routes_to_awaiting_user_choice() {
        let receipt = issue(
            true,
            CorrectionClassification::Ambiguous,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::AwaitingUserChoice,
        )
        .unwrap();
        assert_eq!(
            receipt.disposition,
            CorrectionDisposition::AwaitingUserChoice
        );
    }

    #[test]
    fn ambiguous_cannot_be_routed_directly_to_a_concrete_disposition() {
        let errors = issue(
            true,
            CorrectionClassification::Ambiguous,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::NewRepairOrImplementationAttempt,
        )
        .unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            UserCorrectionError::ClassificationDispositionMismatch { .. }
        )));
    }

    #[test]
    fn new_external_fact_routes_through_fact_channel() {
        let receipt = issue(
            true,
            CorrectionClassification::NewExternalFact,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::RoutedThroughFactChannel,
        )
        .unwrap();
        assert_eq!(
            receipt.disposition,
            CorrectionDisposition::RoutedThroughFactChannel
        );
    }

    #[test]
    fn contract_semantic_change_forces_contract_amendment() {
        let receipt = issue(
            true,
            CorrectionClassification::ContractSemanticChange,
            CorrectionImpactFlags::default(),
            CorrectionDisposition::ContractAmendmentRequired,
        )
        .unwrap();
        assert_eq!(
            receipt.disposition,
            CorrectionDisposition::ContractAmendmentRequired
        );
    }
}
