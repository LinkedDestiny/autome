//! HumanReviewReceipt, HumanReviewFinding and CarriedPlanningReviewBundle
//! per plan §5.1.
//!
//! "人工审计" is an optional extra quality gate layered on top of the
//! mandatory D11 contract confirmation, independent AI reviewer/auditor and
//! Rust verifier — turning it off never removes those. This module encodes
//! three mechanical rules from the plan text rather than leaving them to
//! caller convention:
//!
//! 1. A Reject decision is a contradiction without at least one anchored,
//!    actionable finding — `issue_human_review_receipt` refuses to
//!    construct a Reject receipt otherwise.
//! 2. A finding can only become Resolved by pointing at the successor
//!    Attempt and content hash that actually addressed it — there is no
//!    method that flips a finding to Resolved without those, so a
//!    "resolved but unbound" finding cannot exist as data.
//! 3. Resubmitting unchanged output while findings are still Open is
//!    refused by `can_resubmit_for_review` — real progress (either the
//!    subject changed, or every finding was closed) is required before a
//!    subject can be sent back for review.
//!
//! Project/Task/config-inheritance types from the rest of §5.1
//! (ProjectIntentRevision, GlobalConfigRevision/ProjectConfigPatch/
//! ResolvedProjectConfig, GlobalConfigImpactPreview, PlanningPolicyRestart,
//! RunPolicyAmendment, BudgetGrantReceipt) are a separate, still
//! unimplemented slice — deliberately left for a future increment rather
//! than folded in here.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::attempt::LoopStepId;
use crate::requirement::RequirementId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewSpecSubject {
    Planning {
        planning_spec_hash: String,
    },
    Execution {
        planning_spec_hash: String,
        execution_spec_hash: String,
    },
}

/// §5.1: "`fact_analysis`...`graph_review` 使用 DocumentStep...
/// `implementation`...`final_audit` 使用 CandidateStep." Also the
/// staleness anchor: "任一 subject 输入变化即过期."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewSubject {
    DocumentStep {
        input_snapshot_hash: String,
        output_hash: String,
    },
    CandidateStep {
        candidate_tree_hash: String,
        evidence_set_hash: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewDecision {
    Pass,
    Reject,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanReviewFindingAnchor {
    pub requirement_id: Option<RequirementId>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub ui_region: Option<String>,
    pub evidence_ref: Option<String>,
}

impl HumanReviewFindingAnchor {
    pub fn is_empty(&self) -> bool {
        self == &HumanReviewFindingAnchor::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingStatus {
    Open,
    Resolved,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanReviewFinding {
    pub id: String,
    pub review_receipt_id: String,
    pub subject_hash: String,
    pub step_id: LoopStepId,
    pub anchor: HumanReviewFindingAnchor,
    pub expected_change: String,
    pub severity: String,
    pub status: FindingStatus,
    pub successor_attempt_id: Option<String>,
    pub resolution_subject_hash: Option<String>,
    pub finding_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingResolutionError {
    FindingNotOpen,
    MissingSuccessorAttempt,
    MissingResolutionSubjectHash,
}

impl HumanReviewFinding {
    /// The only way to move a finding out of Open into Resolved — always
    /// requires the successor Attempt and the content hash it produced, so
    /// a Resolved finding can never be "unbound".
    pub fn mark_resolved(
        &mut self,
        successor_attempt_id: &str,
        resolution_subject_hash: &str,
    ) -> Result<(), FindingResolutionError> {
        if self.status != FindingStatus::Open {
            return Err(FindingResolutionError::FindingNotOpen);
        }
        if successor_attempt_id.trim().is_empty() {
            return Err(FindingResolutionError::MissingSuccessorAttempt);
        }
        if resolution_subject_hash.trim().is_empty() {
            return Err(FindingResolutionError::MissingResolutionSubjectHash);
        }
        self.status = FindingStatus::Resolved;
        self.successor_attempt_id = Some(successor_attempt_id.to_string());
        self.resolution_subject_hash = Some(resolution_subject_hash.to_string());
        Ok(())
    }

    pub fn mark_superseded(
        &mut self,
        successor_attempt_id: &str,
    ) -> Result<(), FindingResolutionError> {
        if self.status != FindingStatus::Open {
            return Err(FindingResolutionError::FindingNotOpen);
        }
        if successor_attempt_id.trim().is_empty() {
            return Err(FindingResolutionError::MissingSuccessorAttempt);
        }
        self.status = FindingStatus::Superseded;
        self.successor_attempt_id = Some(successor_attempt_id.to_string());
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanReviewReceipt {
    pub project_hash: String,
    pub task_hash: String,
    pub run_hash: String,
    pub spec_subject: ReviewSpecSubject,
    pub step_id: LoopStepId,
    pub operator: String,
    pub decided_at: String,
    pub decision: ReviewDecision,
    pub review_output_hash: String,
    pub reason: String,
    pub finding_ids: Vec<String>,
    pub subject: ReviewSubject,
    pub receipt_digest: String,
}

impl HumanReviewReceipt {
    /// §5.1: "任一 subject 输入变化即过期."
    pub fn is_current_against(&self, current_subject: &ReviewSubject) -> bool {
        &self.subject == current_subject
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HumanReviewReceiptError {
    RejectRequiresAtLeastOneFinding,
    FindingMissingAnchor { index: usize },
    FindingMissingExpectedChange { index: usize },
}

/// §5.1: "reject 必须至少包含一条有锚点和期望变化的 HumanReviewFinding." The
/// sole constructor for `HumanReviewReceipt` — a Reject decision cannot be
/// built without findings that actually anchor and describe the problem.
#[allow(clippy::too_many_arguments)]
pub fn issue_human_review_receipt(
    project_hash: &str,
    task_hash: &str,
    run_hash: &str,
    spec_subject: ReviewSpecSubject,
    step_id: LoopStepId,
    operator: &str,
    decided_at: &str,
    decision: ReviewDecision,
    review_output_hash: &str,
    reason: &str,
    findings: &[HumanReviewFinding],
    subject: ReviewSubject,
    receipt_digest: &str,
) -> Result<HumanReviewReceipt, Vec<HumanReviewReceiptError>> {
    let mut errors = Vec::new();
    if decision == ReviewDecision::Reject {
        if findings.is_empty() {
            errors.push(HumanReviewReceiptError::RejectRequiresAtLeastOneFinding);
        }
        for (index, finding) in findings.iter().enumerate() {
            if finding.anchor.is_empty() {
                errors.push(HumanReviewReceiptError::FindingMissingAnchor { index });
            }
            if finding.expected_change.trim().is_empty() {
                errors.push(HumanReviewReceiptError::FindingMissingExpectedChange { index });
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(HumanReviewReceipt {
        project_hash: project_hash.to_string(),
        task_hash: task_hash.to_string(),
        run_hash: run_hash.to_string(),
        spec_subject,
        step_id,
        operator: operator.to_string(),
        decided_at: decided_at.to_string(),
        decision,
        review_output_hash: review_output_hash.to_string(),
        reason: reason.to_string(),
        finding_ids: findings.iter().map(|f| f.id.clone()).collect(),
        subject,
        receipt_digest: receipt_digest.to_string(),
    })
}

/// §5.1: "输出 subject 未变化、finding 未逐项 resolved/superseded...时，
/// Core 拒绝再次送审." Resolved/Superseded are structurally already bound
/// to a successor (see `mark_resolved`/`mark_superseded`), so checking
/// status here is sufficient to also guarantee the successor binding.
pub fn can_resubmit_for_review(subject_changed: bool, findings: &[HumanReviewFinding]) -> bool {
    subject_changed || findings.iter().all(|f| f.status != FindingStatus::Open)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarriedPlanningReviewBundle {
    pub new_run_hash: String,
    pub execution_spec_hash: String,
    pub origin_chain_hash: String,
    pub contract_hash: String,
    pub graph_hash: String,
    pub document_output_hashes: Vec<String>,
    pub required_step_ids: Vec<LoopStepId>,
    pub presentation_hash: String,
    pub newly_issued_human_review_receipt_ids: Vec<String>,
    pub bundle_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarriedPlanningReviewError {
    pub step: LoopStepId,
}

/// §5.1: steps the *current* config requires human review for must get a
/// freshly issued `HumanReviewReceipt` when carrying forward prior
/// planning output; steps that aren't required may rely on origin chain +
/// content hash alone, so only `required_step_ids` are checked here.
pub fn validate_carried_planning_review_bundle(
    required_step_ids: &[LoopStepId],
    fresh_receipt_ids_by_step: &HashMap<LoopStepId, String>,
) -> Vec<CarriedPlanningReviewError> {
    required_step_ids
        .iter()
        .filter(|step| !fresh_receipt_ids_by_step.contains_key(step))
        .map(|step| CarriedPlanningReviewError { step: step.clone() })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str) -> LoopStepId {
        LoopStepId(id.to_string())
    }

    fn document_subject() -> ReviewSubject {
        ReviewSubject::DocumentStep {
            input_snapshot_hash: "input-1".into(),
            output_hash: "output-1".into(),
        }
    }

    fn anchored_finding(status: FindingStatus) -> HumanReviewFinding {
        HumanReviewFinding {
            id: "finding-1".into(),
            review_receipt_id: "receipt-1".into(),
            subject_hash: "output-1".into(),
            step_id: step("contract_review"),
            anchor: HumanReviewFindingAnchor {
                path: Some("contract.md".into()),
                ..Default::default()
            },
            expected_change: "narrow the write scope".into(),
            severity: "major".into(),
            status,
            successor_attempt_id: None,
            resolution_subject_hash: None,
            finding_digest: "finding-digest-1".into(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn issue(
        decision: ReviewDecision,
        findings: &[HumanReviewFinding],
    ) -> Result<HumanReviewReceipt, Vec<HumanReviewReceiptError>> {
        issue_human_review_receipt(
            "project-1",
            "task-1",
            "run-1",
            ReviewSpecSubject::Planning {
                planning_spec_hash: "planning-1".into(),
            },
            step("contract_review"),
            "operator-1",
            "2026-09-14T00:00:00Z",
            decision,
            "review-output-1",
            "reason",
            findings,
            document_subject(),
            "receipt-digest-1",
        )
    }

    #[test]
    fn pass_decision_requires_no_findings() {
        assert!(issue(ReviewDecision::Pass, &[]).is_ok());
    }

    #[test]
    fn reject_decision_without_findings_is_rejected() {
        assert_eq!(
            issue(ReviewDecision::Reject, &[]).unwrap_err(),
            vec![HumanReviewReceiptError::RejectRequiresAtLeastOneFinding]
        );
    }

    #[test]
    fn reject_decision_with_unanchored_finding_is_rejected() {
        let finding = HumanReviewFinding {
            anchor: HumanReviewFindingAnchor::default(),
            ..anchored_finding(FindingStatus::Open)
        };
        assert_eq!(
            issue(ReviewDecision::Reject, &[finding]).unwrap_err(),
            vec![HumanReviewReceiptError::FindingMissingAnchor { index: 0 }]
        );
    }

    #[test]
    fn reject_decision_with_empty_expected_change_is_rejected() {
        let finding = HumanReviewFinding {
            expected_change: "   ".into(),
            ..anchored_finding(FindingStatus::Open)
        };
        assert_eq!(
            issue(ReviewDecision::Reject, &[finding]).unwrap_err(),
            vec![HumanReviewReceiptError::FindingMissingExpectedChange { index: 0 }]
        );
    }

    #[test]
    fn reject_decision_with_a_proper_finding_is_issued() {
        let finding = anchored_finding(FindingStatus::Open);
        let receipt = issue(ReviewDecision::Reject, &[finding]).unwrap();
        assert_eq!(receipt.finding_ids, vec!["finding-1".to_string()]);
    }

    #[test]
    fn receipt_is_current_only_against_matching_subject() {
        let receipt = issue(ReviewDecision::Pass, &[]).unwrap();
        assert!(receipt.is_current_against(&document_subject()));
        let changed = ReviewSubject::DocumentStep {
            input_snapshot_hash: "input-1".into(),
            output_hash: "output-2".into(),
        };
        assert!(!receipt.is_current_against(&changed));
    }

    #[test]
    fn mark_resolved_requires_successor_attempt() {
        let mut finding = anchored_finding(FindingStatus::Open);
        assert_eq!(
            finding.mark_resolved("", "resolution-hash").unwrap_err(),
            FindingResolutionError::MissingSuccessorAttempt
        );
    }

    #[test]
    fn mark_resolved_requires_resolution_subject_hash() {
        let mut finding = anchored_finding(FindingStatus::Open);
        assert_eq!(
            finding.mark_resolved("attempt-2", "").unwrap_err(),
            FindingResolutionError::MissingResolutionSubjectHash
        );
    }

    #[test]
    fn mark_resolved_refuses_a_non_open_finding() {
        let mut finding = anchored_finding(FindingStatus::Superseded);
        assert_eq!(
            finding
                .mark_resolved("attempt-2", "resolution-hash")
                .unwrap_err(),
            FindingResolutionError::FindingNotOpen
        );
    }

    #[test]
    fn mark_resolved_succeeds_and_binds_successor() {
        let mut finding = anchored_finding(FindingStatus::Open);
        finding
            .mark_resolved("attempt-2", "resolution-hash")
            .unwrap();
        assert_eq!(finding.status, FindingStatus::Resolved);
        assert_eq!(finding.successor_attempt_id.as_deref(), Some("attempt-2"));
        assert_eq!(
            finding.resolution_subject_hash.as_deref(),
            Some("resolution-hash")
        );
    }

    #[test]
    fn mark_superseded_requires_successor_attempt() {
        let mut finding = anchored_finding(FindingStatus::Open);
        assert_eq!(
            finding.mark_superseded("").unwrap_err(),
            FindingResolutionError::MissingSuccessorAttempt
        );
    }

    #[test]
    fn mark_superseded_succeeds() {
        let mut finding = anchored_finding(FindingStatus::Open);
        finding.mark_superseded("attempt-3").unwrap();
        assert_eq!(finding.status, FindingStatus::Superseded);
        assert_eq!(finding.successor_attempt_id.as_deref(), Some("attempt-3"));
    }

    #[test]
    fn cannot_resubmit_unchanged_subject_with_open_findings() {
        let findings = vec![anchored_finding(FindingStatus::Open)];
        assert!(!can_resubmit_for_review(false, &findings));
    }

    #[test]
    fn can_resubmit_when_subject_changed_even_with_open_findings() {
        let findings = vec![anchored_finding(FindingStatus::Open)];
        assert!(can_resubmit_for_review(true, &findings));
    }

    #[test]
    fn can_resubmit_unchanged_subject_once_all_findings_closed() {
        let findings = vec![anchored_finding(FindingStatus::Resolved)];
        assert!(can_resubmit_for_review(false, &findings));
    }

    #[test]
    fn carried_bundle_flags_required_steps_missing_a_fresh_receipt() {
        let required = vec![step("contract_review"), step("graph_review")];
        let mut fresh = HashMap::new();
        fresh.insert(step("contract_review"), "receipt-9".to_string());
        let violations = validate_carried_planning_review_bundle(&required, &fresh);
        assert_eq!(
            violations,
            vec![CarriedPlanningReviewError {
                step: step("graph_review")
            }]
        );
    }

    #[test]
    fn carried_bundle_passes_when_every_required_step_has_a_fresh_receipt() {
        let required = vec![step("contract_review")];
        let mut fresh = HashMap::new();
        fresh.insert(step("contract_review"), "receipt-9".to_string());
        assert!(validate_carried_planning_review_bundle(&required, &fresh).is_empty());
    }
}
