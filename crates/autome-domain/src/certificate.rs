//! AuditVerdict, CandidateCertificate and CompletionCertificate per plan
//! §5.8, implementing the D4 split ("完成声明与完成事实分离"): a
//! CandidateCertificate proves eligibility, never task completion, and a
//! CompletionCertificate can only be minted from an already-issued
//! CandidateCertificate plus proof of delivery — there is no constructor
//! that skips straight from evidence to "done".

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::delivery::{DeliveryChain, DeliveryChainError};
use crate::evidence::ReceiptId;
use crate::readiness::{ReadinessFingerprint, ReadinessReceipt};
use crate::requirement::RequirementId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOutcome {
    Satisfied,
    NotSatisfied,
    Unverified,
}

/// Plan §5.8: "独立 evaluator 针对每项 Requirement 输出
/// satisfied/not_satisfied/unverified，引用具体 Receipt." An AuditVerdict
/// with zero evidence_receipt_ids is a contradiction in terms — a verdict
/// must point at the receipts it was formed from — so `Satisfied` requires
/// at least one, enforced at construction rather than left to callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditVerdict {
    pub requirement_id: RequirementId,
    pub outcome: AuditOutcome,
    pub evidence_receipt_ids: Vec<ReceiptId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditVerdictError {
    SatisfiedVerdictHasNoEvidence,
}

impl AuditVerdict {
    pub fn new(
        requirement_id: RequirementId,
        outcome: AuditOutcome,
        evidence_receipt_ids: Vec<ReceiptId>,
    ) -> Result<Self, AuditVerdictError> {
        if outcome == AuditOutcome::Satisfied && evidence_receipt_ids.is_empty() {
            return Err(AuditVerdictError::SatisfiedVerdictHasNoEvidence);
        }
        Ok(Self {
            requirement_id,
            outcome,
            evidence_receipt_ids,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateCertificate {
    pub run_id: String,
    pub contract_version: u32,
    pub candidate_commit: String,
    pub candidate_tree_hash: String,
    pub covered_requirement_ids: Vec<RequirementId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateCertificateError {
    MissingVerdict {
        requirement: RequirementId,
    },
    VerdictNotSatisfied {
        requirement: RequirementId,
        outcome: AuditOutcome,
    },
    VerdictReferencesInvalidatedReceipt {
        requirement: RequirementId,
        receipt: ReceiptId,
    },
    ReadinessNotReady,
    ReadinessNotCurrent,
}

/// Plan §5.8: "只有事实收据与独立审计同时通过，才签发 CandidateCertificate."
/// This is the sole issuance path — there is no other way to construct a
/// `CandidateCertificate` in this crate. It requires, for every must
/// requirement: a verdict exists, that verdict is Satisfied, and every
/// receipt it cites is still valid (`valid_receipt_ids` is expected to be
/// pre-filtered by the caller via `EvidenceReceipt::is_valid_against`).
///
/// Plan §5.9 cross-cutting rule: a CandidateCertificate must bind a
/// *current* ReadinessReceipt revision, never a stale baseline preflight —
/// `readiness` must both be `Ready` and current against
/// `current_environment_fingerprint` (recomputed by the caller at
/// issuance time), or issuance is refused regardless of how the audit
/// verdicts look.
#[allow(clippy::too_many_arguments)]
pub fn issue_candidate_certificate(
    run_id: &str,
    contract_version: u32,
    candidate_commit: &str,
    candidate_tree_hash: &str,
    must_requirement_ids: &[RequirementId],
    verdicts: &[AuditVerdict],
    valid_receipt_ids: &HashSet<ReceiptId>,
    readiness: &ReadinessReceipt,
    current_environment_fingerprint: &ReadinessFingerprint,
) -> Result<CandidateCertificate, Vec<CandidateCertificateError>> {
    let mut errors = Vec::new();
    if !readiness.is_ready() {
        errors.push(CandidateCertificateError::ReadinessNotReady);
    }
    if !readiness.is_current_against(current_environment_fingerprint) {
        errors.push(CandidateCertificateError::ReadinessNotCurrent);
    }
    let verdict_by_requirement: HashMap<&RequirementId, &AuditVerdict> =
        verdicts.iter().map(|v| (&v.requirement_id, v)).collect();

    for requirement_id in must_requirement_ids {
        match verdict_by_requirement.get(requirement_id) {
            None => errors.push(CandidateCertificateError::MissingVerdict {
                requirement: requirement_id.clone(),
            }),
            Some(verdict) => {
                if verdict.outcome != AuditOutcome::Satisfied {
                    errors.push(CandidateCertificateError::VerdictNotSatisfied {
                        requirement: requirement_id.clone(),
                        outcome: verdict.outcome,
                    });
                    continue;
                }
                for receipt_id in &verdict.evidence_receipt_ids {
                    if !valid_receipt_ids.contains(receipt_id) {
                        errors.push(
                            CandidateCertificateError::VerdictReferencesInvalidatedReceipt {
                                requirement: requirement_id.clone(),
                                receipt: receipt_id.clone(),
                            },
                        );
                    }
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(CandidateCertificate {
        run_id: run_id.to_string(),
        contract_version,
        candidate_commit: candidate_commit.to_string(),
        candidate_tree_hash: candidate_tree_hash.to_string(),
        covered_requirement_ids: must_requirement_ids.to_vec(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionCertificate {
    pub contract_version: u32,
    pub candidate_commit: String,
    pub candidate_tree_hash: String,
    pub delivery_subject: crate::delivery::DeliverySubject,
    pub delivery_tree_hash: String,
    pub user_approval_decision_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionCertificateError {
    DeliveryChainNotReady(DeliveryChainError),
    MissingUserApprovalDecision,
}

/// Plan §5.8/D4: a CandidateCertificate "只表示这个候选具备交付资格，不表示
/// 用户任务 Completed". This function is the only way to obtain a
/// `CompletionCertificate`, and it can only be called *with* an already
/// issued `CandidateCertificate` (never with raw evidence/verdicts) plus
/// proof delivery actually happened and the user approved it.
///
/// Plan §5.12 cross-cutting rule: "proof delivery actually happened" means
/// the caller's `DeliveryChain` must itself already satisfy
/// `is_ready_for_completion()` — a matching DeliveredTreeCheckReceipt, and
/// (for a Greenfield subject) an already-appended
/// ProjectTargetTransitionReceipt. There is no boolean shortcut that lets a
/// caller assert delivery happened without the chain itself agreeing.
pub fn issue_completion_certificate(
    candidate: &CandidateCertificate,
    delivery: &DeliveryChain,
    delivery_tree_hash: &str,
    user_approval_decision_ref: &str,
) -> Result<CompletionCertificate, CompletionCertificateError> {
    delivery
        .is_ready_for_completion()
        .map_err(CompletionCertificateError::DeliveryChainNotReady)?;
    if user_approval_decision_ref.trim().is_empty() {
        return Err(CompletionCertificateError::MissingUserApprovalDecision);
    }
    Ok(CompletionCertificate {
        contract_version: candidate.contract_version,
        candidate_commit: candidate.candidate_commit.clone(),
        candidate_tree_hash: candidate.candidate_tree_hash.clone(),
        delivery_subject: delivery.subject().clone(),
        delivery_tree_hash: delivery_tree_hash.to_string(),
        user_approval_decision_ref: user_approval_decision_ref.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str) -> RequirementId {
        RequirementId(id.into())
    }

    fn receipt(id: &str) -> ReceiptId {
        ReceiptId(id.into())
    }

    fn ready_subject() -> crate::readiness::ReadinessSubject {
        crate::readiness::ReadinessSubject::ExistingRepo(crate::readiness::ExistingRepoSubject {
            repository_identity_hash: "repo-hash".into(),
            base_commit: "base".into(),
            target_head: "head".into(),
            worktree_fingerprint: "wt-1".into(),
        })
    }

    fn ready_readiness() -> ReadinessReceipt {
        ReadinessReceipt {
            revision: 1,
            scope: crate::readiness::ReadinessScope::Execution,
            profile_hash: "profile-1".into(),
            environment_relevant_inputs_digest: "env-digest-1".into(),
            observed_at: "2026-09-14T00:00:00Z".into(),
            valid_until: "2026-09-15T00:00:00Z".into(),
            subject: ready_subject(),
            programs: vec![],
            lockfile_hashes: vec![],
            result: crate::readiness::ReadinessResult::Ready,
            missing: vec![],
            receipt_digest: "receipt-digest-1".into(),
        }
    }

    fn current_fingerprint() -> ReadinessFingerprint {
        ReadinessFingerprint {
            profile_hash: "profile-1".into(),
            environment_relevant_inputs_digest: "env-digest-1".into(),
            subject: ready_subject(),
        }
    }

    #[test]
    fn satisfied_verdict_without_evidence_is_rejected() {
        assert_eq!(
            AuditVerdict::new(req("R-001"), AuditOutcome::Satisfied, vec![]).unwrap_err(),
            AuditVerdictError::SatisfiedVerdictHasNoEvidence
        );
    }

    #[test]
    fn not_satisfied_verdict_without_evidence_is_allowed() {
        assert!(AuditVerdict::new(req("R-001"), AuditOutcome::NotSatisfied, vec![]).is_ok());
    }

    #[test]
    fn candidate_certificate_requires_a_verdict_for_every_must_requirement() {
        let must = vec![req("R-001")];
        let result = issue_candidate_certificate(
            "run-1",
            1,
            "commit-1",
            "tree-1",
            &must,
            &[],
            &HashSet::new(),
            &ready_readiness(),
            &current_fingerprint(),
        );
        assert_eq!(
            result.unwrap_err(),
            vec![CandidateCertificateError::MissingVerdict {
                requirement: req("R-001")
            }]
        );
    }

    #[test]
    fn candidate_certificate_rejects_non_satisfied_verdict() {
        let must = vec![req("R-001")];
        let verdict = AuditVerdict::new(req("R-001"), AuditOutcome::Unverified, vec![]).unwrap();
        let result = issue_candidate_certificate(
            "run-1",
            1,
            "commit-1",
            "tree-1",
            &must,
            &[verdict],
            &HashSet::new(),
            &ready_readiness(),
            &current_fingerprint(),
        );
        assert_eq!(
            result.unwrap_err(),
            vec![CandidateCertificateError::VerdictNotSatisfied {
                requirement: req("R-001"),
                outcome: AuditOutcome::Unverified,
            }]
        );
    }

    #[test]
    fn candidate_certificate_rejects_invalidated_receipt() {
        let must = vec![req("R-001")];
        let verdict =
            AuditVerdict::new(req("R-001"), AuditOutcome::Satisfied, vec![receipt("EV-1")])
                .unwrap();
        // valid_receipt_ids deliberately does not contain EV-1: it went
        // stale (e.g. the candidate tree changed after the receipt issued).
        let result = issue_candidate_certificate(
            "run-1",
            1,
            "commit-1",
            "tree-1",
            &must,
            &[verdict],
            &HashSet::new(),
            &ready_readiness(),
            &current_fingerprint(),
        );
        assert_eq!(
            result.unwrap_err(),
            vec![
                CandidateCertificateError::VerdictReferencesInvalidatedReceipt {
                    requirement: req("R-001"),
                    receipt: receipt("EV-1"),
                }
            ]
        );
    }

    #[test]
    fn candidate_certificate_issues_when_all_must_requirements_satisfied() {
        let must = vec![req("R-001"), req("R-002")];
        let verdicts = vec![
            AuditVerdict::new(req("R-001"), AuditOutcome::Satisfied, vec![receipt("EV-1")])
                .unwrap(),
            AuditVerdict::new(req("R-002"), AuditOutcome::Satisfied, vec![receipt("EV-2")])
                .unwrap(),
        ];
        let mut valid = HashSet::new();
        valid.insert(receipt("EV-1"));
        valid.insert(receipt("EV-2"));
        let cert = issue_candidate_certificate(
            "run-1",
            1,
            "commit-1",
            "tree-1",
            &must,
            &verdicts,
            &valid,
            &ready_readiness(),
            &current_fingerprint(),
        )
        .unwrap();
        assert_eq!(cert.covered_requirement_ids, must);
    }

    #[test]
    fn candidate_certificate_rejects_not_ready_environment() {
        let must = vec![req("R-001")];
        let verdicts = vec![
            AuditVerdict::new(req("R-001"), AuditOutcome::Satisfied, vec![receipt("EV-1")])
                .unwrap(),
        ];
        let mut valid = HashSet::new();
        valid.insert(receipt("EV-1"));
        let mut not_ready = ready_readiness();
        not_ready.result = crate::readiness::ReadinessResult::NotReady;
        let result = issue_candidate_certificate(
            "run-1",
            1,
            "commit-1",
            "tree-1",
            &must,
            &verdicts,
            &valid,
            &not_ready,
            &current_fingerprint(),
        );
        assert_eq!(
            result.unwrap_err(),
            vec![CandidateCertificateError::ReadinessNotReady]
        );
    }

    #[test]
    fn candidate_certificate_rejects_stale_readiness_receipt() {
        let must = vec![req("R-001")];
        let verdicts = vec![
            AuditVerdict::new(req("R-001"), AuditOutcome::Satisfied, vec![receipt("EV-1")])
                .unwrap(),
        ];
        let mut valid = HashSet::new();
        valid.insert(receipt("EV-1"));
        let mut stale_fingerprint = current_fingerprint();
        stale_fingerprint.environment_relevant_inputs_digest = "env-digest-2".into();
        let result = issue_candidate_certificate(
            "run-1",
            1,
            "commit-1",
            "tree-1",
            &must,
            &verdicts,
            &valid,
            &ready_readiness(),
            &stale_fingerprint,
        );
        assert_eq!(
            result.unwrap_err(),
            vec![CandidateCertificateError::ReadinessNotCurrent]
        );
    }

    fn candidate() -> CandidateCertificate {
        CandidateCertificate {
            run_id: "run-1".into(),
            contract_version: 1,
            candidate_commit: "commit-1".into(),
            candidate_tree_hash: "tree-1".into(),
            covered_requirement_ids: vec![req("R-001")],
        }
    }

    fn delivery_subject() -> crate::delivery::DeliverySubject {
        crate::delivery::DeliverySubject::ExistingRepo(crate::delivery::ExistingRepoDelivery {
            repository_identity_hash: "repo-hash".into(),
            target_head: "head".into(),
            target_worktree_fingerprint: "wt-1".into(),
            new_ref: "refs/heads/delivered".into(),
        })
    }

    fn delivery_envelope() -> crate::delivery::DeliveryEnvelope {
        crate::delivery::DeliveryEnvelope {
            run_id: "run-1".into(),
            contract_hash: "contract-1".into(),
            candidate_certificate_hash: "candidate-1".into(),
            policy_hash: "policy-1".into(),
            nonce: "nonce-1".into(),
            issued_at: "2026-09-14T00:00:00Z".into(),
            receipt_digest: "digest-1".into(),
        }
    }

    fn not_ready_delivery_chain() -> DeliveryChain {
        DeliveryChain::new(delivery_subject())
    }

    fn ready_delivery_chain() -> DeliveryChain {
        let mut chain = DeliveryChain::new(delivery_subject());
        chain.append_rehearsal(crate::delivery::DeliveryRehearsalReceipt {
            envelope: delivery_envelope(),
            subject: delivery_subject(),
            target_head_or_parent: "head".into(),
            delivery_tree_hash: "tree-2".into(),
            check_receipt_ids: vec!["check-1".into()],
        });
        chain
            .append_approval(crate::delivery::DeliveryApprovalReceipt {
                envelope: delivery_envelope(),
                rehearsal_receipt_digest: "digest-1".into(),
                display_summary: "summary".into(),
                destination_or_new_ref: "refs/heads/delivered".into(),
                artifact_destinations: vec![],
                valid_until: "2026-09-15T00:00:00Z".into(),
                operator_decision_ref: "decision:0".into(),
            })
            .unwrap();
        chain
            .append_delivery(crate::delivery::DeliveryReceipt {
                envelope: delivery_envelope(),
                approval_receipt_digest: "digest-1".into(),
                before_identity_hash: "before-1".into(),
                after_identity_hash: "after-1".into(),
                outcome: crate::delivery::DeliveryOutcome::Succeeded,
            })
            .unwrap();
        chain
            .append_tree_check(crate::delivery::DeliveredTreeCheckReceipt {
                envelope: delivery_envelope(),
                delivery_receipt_digest: "digest-1".into(),
                observed_ref_or_tree: "refs/heads/delivered".into(),
                artifact_hashes: vec![],
                worktree_fingerprint: "wt-1".into(),
                matches_delivery: true,
            })
            .unwrap();
        chain
    }

    #[test]
    fn completion_certificate_requires_delivery_chain_to_be_ready() {
        let result = issue_completion_certificate(
            &candidate(),
            &not_ready_delivery_chain(),
            "tree-2",
            "decision:1",
        );
        assert_eq!(
            result.unwrap_err(),
            CompletionCertificateError::DeliveryChainNotReady(
                DeliveryChainError::TreeCheckRequiredBeforeCompletion
            )
        );
    }

    #[test]
    fn completion_certificate_requires_user_approval_decision() {
        let result =
            issue_completion_certificate(&candidate(), &ready_delivery_chain(), "tree-2", "  ");
        assert_eq!(
            result.unwrap_err(),
            CompletionCertificateError::MissingUserApprovalDecision
        );
    }

    #[test]
    fn completion_certificate_issues_from_a_candidate_certificate() {
        let cert = issue_completion_certificate(
            &candidate(),
            &ready_delivery_chain(),
            "tree-2",
            "decision:1",
        )
        .unwrap();
        assert_eq!(cert.contract_version, 1);
        assert_eq!(cert.delivery_subject, delivery_subject());
        assert_eq!(cert.delivery_tree_hash, "tree-2");
    }
}
