//! Delivery receipt chain per plan §5.12.
//!
//! Each receipt in the chain is signed off by a different real-world actor
//! (verifier, Core IPC command handler, capability broker, Core read-only
//! checker, Project service) — *which actor is allowed to call which
//! append method* is an authorization concern enforced by `automed`'s
//! command routing, not something this data-only crate can check (there is
//! no authenticated-caller-identity concept here). What this module does
//! enforce mechanically is the chain's own shape: receipts can only be
//! appended in the plan's fixed order, a failed DeliveryReceipt can never
//! be silently overwritten or treated as a pass, DeliveredTreeCheckReceipt
//! must actually match before anything downstream can proceed, and
//! ProjectTargetTransitionReceipt is only reachable for a Greenfield
//! subject (rather than a runtime check on an enum that happens to be
//! ExistingRepo).

use serde::{Deserialize, Serialize};

/// §5.12: "两类 subject 是穷举 tagged union，不使用 nullable `target_head`、
/// 虚构 Git baseline 或 sentinel 值."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExistingRepoDelivery {
    pub repository_identity_hash: String,
    pub target_head: String,
    pub target_worktree_fingerprint: String,
    pub new_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GreenfieldDelivery {
    pub parent_directory_identity_hash: String,
    pub destination: String,
    pub destination_absent_proof: String,
    pub template_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliverySubject {
    ExistingRepo(ExistingRepoDelivery),
    Greenfield(GreenfieldDelivery),
}

/// §5.12: "`CandidateCertificate` 的 candidate envelope 为 ... 其后
/// ...才使用 ... delivery envelope" — two distinct envelope shapes rather
/// than one self-referential signed envelope for everything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateEnvelope {
    pub run_id: String,
    pub execution_origin_chain_hash: String,
    pub execution_spec_hash: String,
    pub contract_hash: String,
    pub policy_hash: String,
    pub nonce: String,
    pub issued_at: String,
    pub certificate_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryEnvelope {
    pub run_id: String,
    pub contract_hash: String,
    pub candidate_certificate_hash: String,
    pub policy_hash: String,
    pub nonce: String,
    pub issued_at: String,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryRehearsalReceipt {
    pub envelope: DeliveryEnvelope,
    pub subject: DeliverySubject,
    pub target_head_or_parent: String,
    pub delivery_tree_hash: String,
    pub check_receipt_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryApprovalReceipt {
    pub envelope: DeliveryEnvelope,
    pub rehearsal_receipt_digest: String,
    pub display_summary: String,
    pub destination_or_new_ref: String,
    pub artifact_destinations: Vec<String>,
    pub valid_until: String,
    pub operator_decision_ref: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryOutcome {
    Succeeded,
    Failed,
    UnknownOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub envelope: DeliveryEnvelope,
    pub approval_receipt_digest: String,
    pub before_identity_hash: String,
    pub after_identity_hash: String,
    pub outcome: DeliveryOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveredTreeCheckReceipt {
    pub envelope: DeliveryEnvelope,
    pub delivery_receipt_digest: String,
    pub observed_ref_or_tree: String,
    pub artifact_hashes: Vec<String>,
    pub worktree_fingerprint: String,
    pub matches_delivery: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTargetTransitionReceipt {
    pub envelope: DeliveryEnvelope,
    pub greenfield_destination: String,
    pub delivered_tree_hash: String,
    pub new_repository_identity_hash: String,
    pub pre_transition_project_revision: u32,
    pub post_transition_project_revision: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryChainError {
    RehearsalRequiredBeforeApproval,
    ApprovalRequiredBeforeDelivery,
    DeliveryAlreadyRecorded,
    DeliveryRequiredBeforeTreeCheck,
    DeliveryDidNotSucceed,
    TreeCheckAlreadyRecorded,
    TreeCheckRequiredBeforeProjectTargetTransition,
    TreeCheckDidNotMatch,
    ProjectTargetTransitionOnlyValidForGreenfieldSubject,
    TreeCheckRequiredBeforeCompletion,
    GreenfieldCompletionRequiresProjectTargetTransition,
}

/// §5.12's table walked as a single append-only object: `new()` fixes the
/// subject once, and each `append_*` requires the previous rung to be
/// present (and, for delivery, to have actually succeeded) before it will
/// accept the next receipt. Nothing here can be un-set or replaced —
/// there is no `remove_*` or `&mut` field access from outside the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryChain {
    subject: DeliverySubject,
    rehearsal: Option<DeliveryRehearsalReceipt>,
    approval: Option<DeliveryApprovalReceipt>,
    delivery: Option<DeliveryReceipt>,
    tree_check: Option<DeliveredTreeCheckReceipt>,
    project_target_transition: Option<ProjectTargetTransitionReceipt>,
}

impl DeliveryChain {
    pub fn new(subject: DeliverySubject) -> Self {
        Self {
            subject,
            rehearsal: None,
            approval: None,
            delivery: None,
            tree_check: None,
            project_target_transition: None,
        }
    }

    pub fn subject(&self) -> &DeliverySubject {
        &self.subject
    }

    pub fn append_rehearsal(&mut self, receipt: DeliveryRehearsalReceipt) {
        self.rehearsal = Some(receipt);
    }

    pub fn append_approval(
        &mut self,
        receipt: DeliveryApprovalReceipt,
    ) -> Result<(), DeliveryChainError> {
        if self.rehearsal.is_none() {
            return Err(DeliveryChainError::RehearsalRequiredBeforeApproval);
        }
        self.approval = Some(receipt);
        Ok(())
    }

    /// §5.12: DeliveryReceipt is "只追加，不覆盖；失败不能当通过" — a Failed
    /// or UnknownOutcome receipt is still recorded permanently; it just
    /// blocks `append_tree_check` from proceeding. A second delivery
    /// attempt after failure needs a fresh `DeliveryChain`, not a mutation
    /// of this one.
    pub fn append_delivery(&mut self, receipt: DeliveryReceipt) -> Result<(), DeliveryChainError> {
        if self.approval.is_none() {
            return Err(DeliveryChainError::ApprovalRequiredBeforeDelivery);
        }
        if self.delivery.is_some() {
            return Err(DeliveryChainError::DeliveryAlreadyRecorded);
        }
        self.delivery = Some(receipt);
        Ok(())
    }

    pub fn append_tree_check(
        &mut self,
        receipt: DeliveredTreeCheckReceipt,
    ) -> Result<(), DeliveryChainError> {
        match &self.delivery {
            None => return Err(DeliveryChainError::DeliveryRequiredBeforeTreeCheck),
            Some(d) if d.outcome != DeliveryOutcome::Succeeded => {
                return Err(DeliveryChainError::DeliveryDidNotSucceed);
            }
            Some(_) => {}
        }
        if self.tree_check.is_some() {
            return Err(DeliveryChainError::TreeCheckAlreadyRecorded);
        }
        self.tree_check = Some(receipt);
        Ok(())
    }

    /// §5.12: "仅适用于首个绿地交付" — reachable only when the chain's own
    /// subject is Greenfield, not merely when the caller claims it is.
    pub fn append_project_target_transition(
        &mut self,
        receipt: ProjectTargetTransitionReceipt,
    ) -> Result<(), DeliveryChainError> {
        if !matches!(self.subject, DeliverySubject::Greenfield(_)) {
            return Err(DeliveryChainError::ProjectTargetTransitionOnlyValidForGreenfieldSubject);
        }
        match &self.tree_check {
            None => {
                return Err(DeliveryChainError::TreeCheckRequiredBeforeProjectTargetTransition);
            }
            Some(tc) if !tc.matches_delivery => {
                return Err(DeliveryChainError::TreeCheckDidNotMatch);
            }
            Some(_) => {}
        }
        self.project_target_transition = Some(receipt);
        Ok(())
    }

    /// §5.12: greenfield's first delivery is "唯一 Project revision 例外" —
    /// the completion gate must not fire for it without a
    /// ProjectTargetTransitionReceipt already appended in the same chain.
    pub fn is_ready_for_completion(&self) -> Result<(), DeliveryChainError> {
        match &self.tree_check {
            None => return Err(DeliveryChainError::TreeCheckRequiredBeforeCompletion),
            Some(tc) if !tc.matches_delivery => {
                return Err(DeliveryChainError::TreeCheckDidNotMatch);
            }
            Some(_) => {}
        }
        if matches!(self.subject, DeliverySubject::Greenfield(_))
            && self.project_target_transition.is_none()
        {
            return Err(DeliveryChainError::GreenfieldCompletionRequiresProjectTargetTransition);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn existing_repo_subject() -> DeliverySubject {
        DeliverySubject::ExistingRepo(ExistingRepoDelivery {
            repository_identity_hash: "repo-hash".into(),
            target_head: "head".into(),
            target_worktree_fingerprint: "wt-1".into(),
            new_ref: "refs/heads/delivered".into(),
        })
    }

    fn greenfield_subject() -> DeliverySubject {
        DeliverySubject::Greenfield(GreenfieldDelivery {
            parent_directory_identity_hash: "parent-hash".into(),
            destination: "dest".into(),
            destination_absent_proof: "absent-proof".into(),
            template_hash: "template-hash".into(),
        })
    }

    fn envelope() -> DeliveryEnvelope {
        DeliveryEnvelope {
            run_id: "run-1".into(),
            contract_hash: "contract-1".into(),
            candidate_certificate_hash: "candidate-1".into(),
            policy_hash: "policy-1".into(),
            nonce: "nonce-1".into(),
            issued_at: "2026-09-14T00:00:00Z".into(),
            receipt_digest: "digest-1".into(),
        }
    }

    fn rehearsal() -> DeliveryRehearsalReceipt {
        DeliveryRehearsalReceipt {
            envelope: envelope(),
            subject: existing_repo_subject(),
            target_head_or_parent: "head".into(),
            delivery_tree_hash: "tree-1".into(),
            check_receipt_ids: vec!["check-1".into()],
        }
    }

    fn approval() -> DeliveryApprovalReceipt {
        DeliveryApprovalReceipt {
            envelope: envelope(),
            rehearsal_receipt_digest: "digest-1".into(),
            display_summary: "summary".into(),
            destination_or_new_ref: "refs/heads/delivered".into(),
            artifact_destinations: vec![],
            valid_until: "2026-09-15T00:00:00Z".into(),
            operator_decision_ref: "decision:1".into(),
        }
    }

    fn delivery(outcome: DeliveryOutcome) -> DeliveryReceipt {
        DeliveryReceipt {
            envelope: envelope(),
            approval_receipt_digest: "digest-1".into(),
            before_identity_hash: "before-1".into(),
            after_identity_hash: "after-1".into(),
            outcome,
        }
    }

    fn tree_check(matches_delivery: bool) -> DeliveredTreeCheckReceipt {
        DeliveredTreeCheckReceipt {
            envelope: envelope(),
            delivery_receipt_digest: "digest-1".into(),
            observed_ref_or_tree: "refs/heads/delivered".into(),
            artifact_hashes: vec![],
            worktree_fingerprint: "wt-1".into(),
            matches_delivery,
        }
    }

    fn project_target_transition() -> ProjectTargetTransitionReceipt {
        ProjectTargetTransitionReceipt {
            envelope: envelope(),
            greenfield_destination: "dest".into(),
            delivered_tree_hash: "tree-1".into(),
            new_repository_identity_hash: "new-repo-hash".into(),
            pre_transition_project_revision: 1,
            post_transition_project_revision: 2,
        }
    }

    #[test]
    fn approval_before_rehearsal_is_rejected() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        let err = chain.append_approval(approval()).unwrap_err();
        assert_eq!(err, DeliveryChainError::RehearsalRequiredBeforeApproval);
    }

    #[test]
    fn delivery_before_approval_is_rejected() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        chain.append_rehearsal(rehearsal());
        let err = chain
            .append_delivery(delivery(DeliveryOutcome::Succeeded))
            .unwrap_err();
        assert_eq!(err, DeliveryChainError::ApprovalRequiredBeforeDelivery);
    }

    #[test]
    fn delivery_cannot_be_recorded_twice() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        chain.append_rehearsal(rehearsal());
        chain.append_approval(approval()).unwrap();
        chain
            .append_delivery(delivery(DeliveryOutcome::Failed))
            .unwrap();
        let err = chain
            .append_delivery(delivery(DeliveryOutcome::Succeeded))
            .unwrap_err();
        assert_eq!(err, DeliveryChainError::DeliveryAlreadyRecorded);
    }

    #[test]
    fn failed_delivery_blocks_tree_check_rather_than_counting_as_a_pass() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        chain.append_rehearsal(rehearsal());
        chain.append_approval(approval()).unwrap();
        chain
            .append_delivery(delivery(DeliveryOutcome::Failed))
            .unwrap();
        let err = chain.append_tree_check(tree_check(true)).unwrap_err();
        assert_eq!(err, DeliveryChainError::DeliveryDidNotSucceed);
    }

    #[test]
    fn unknown_outcome_delivery_also_blocks_tree_check() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        chain.append_rehearsal(rehearsal());
        chain.append_approval(approval()).unwrap();
        chain
            .append_delivery(delivery(DeliveryOutcome::UnknownOutcome))
            .unwrap();
        let err = chain.append_tree_check(tree_check(true)).unwrap_err();
        assert_eq!(err, DeliveryChainError::DeliveryDidNotSucceed);
    }

    #[test]
    fn successful_chain_for_existing_repo_is_ready_for_completion_without_transition() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        chain.append_rehearsal(rehearsal());
        chain.append_approval(approval()).unwrap();
        chain
            .append_delivery(delivery(DeliveryOutcome::Succeeded))
            .unwrap();
        chain.append_tree_check(tree_check(true)).unwrap();
        assert!(chain.is_ready_for_completion().is_ok());
    }

    #[test]
    fn mismatched_tree_check_blocks_completion() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        chain.append_rehearsal(rehearsal());
        chain.append_approval(approval()).unwrap();
        chain
            .append_delivery(delivery(DeliveryOutcome::Succeeded))
            .unwrap();
        chain.append_tree_check(tree_check(false)).unwrap();
        assert_eq!(
            chain.is_ready_for_completion().unwrap_err(),
            DeliveryChainError::TreeCheckDidNotMatch
        );
    }

    #[test]
    fn greenfield_completion_requires_project_target_transition() {
        let mut chain = DeliveryChain::new(greenfield_subject());
        chain.append_rehearsal(DeliveryRehearsalReceipt {
            subject: greenfield_subject(),
            ..rehearsal()
        });
        chain.append_approval(approval()).unwrap();
        chain
            .append_delivery(delivery(DeliveryOutcome::Succeeded))
            .unwrap();
        chain.append_tree_check(tree_check(true)).unwrap();
        assert_eq!(
            chain.is_ready_for_completion().unwrap_err(),
            DeliveryChainError::GreenfieldCompletionRequiresProjectTargetTransition
        );
        chain
            .append_project_target_transition(project_target_transition())
            .unwrap();
        assert!(chain.is_ready_for_completion().is_ok());
    }

    #[test]
    fn project_target_transition_is_rejected_for_existing_repo_subject() {
        let mut chain = DeliveryChain::new(existing_repo_subject());
        chain.append_rehearsal(rehearsal());
        chain.append_approval(approval()).unwrap();
        chain
            .append_delivery(delivery(DeliveryOutcome::Succeeded))
            .unwrap();
        chain.append_tree_check(tree_check(true)).unwrap();
        let err = chain
            .append_project_target_transition(project_target_transition())
            .unwrap_err();
        assert_eq!(
            err,
            DeliveryChainError::ProjectTargetTransitionOnlyValidForGreenfieldSubject
        );
    }
}
