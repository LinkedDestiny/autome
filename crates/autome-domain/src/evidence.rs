//! EvidenceReceipt per plan §5.7. This crate has no I/O, so the envelope
//! fields that in production are computed by hashing real files/processes
//! (content_hash, diff_hash, etc.) are modeled as opaque, already-computed
//! strings here — this module owns only the *shape* of a receipt and the
//! staleness rule the plan states explicitly:
//! "contract、check、project-rule/oracle snapshot、candidate tree、依赖、
//!环境或 freshness 任一不匹配，收据立即失效" (if contract, check,
//! project-rule/oracle snapshot, candidate tree, or environment fingerprint
//! mismatches, the receipt is immediately invalid).

use serde::{Deserialize, Serialize};

use crate::requirement::CheckId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReceiptId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckOutcome {
    Pass,
    Fail,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessResultPayload {
    pub program: String,
    pub args: Vec<String>,
    pub exit_code: i32,
    pub assertions: Vec<String>,
    pub inventory_changes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileResultPayload {
    pub path: String,
    pub content_hash: String,
    pub assertions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitResultPayload {
    pub base: String,
    pub head: String,
    pub diff_hash: String,
    pub changed_paths: Vec<String>,
    pub clean: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDecisionResultPayload {
    pub gate_id: String,
    pub action_hash: String,
    pub decision: String,
    pub operator: String,
    pub decided_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidencePayload {
    Process(ProcessResultPayload),
    File(FileResultPayload),
    Git(GitResultPayload),
    UserDecision(UserDecisionResultPayload),
}

/// The subset of a receipt's envelope that must match the *current*
/// candidate/contract state for the receipt to still count. Plan §5.7:
/// any single mismatch invalidates the receipt outright — there is no
/// partial-credit path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFingerprint {
    pub contract_hash: String,
    pub check_hash: String,
    pub project_rule_snapshot_hash: String,
    pub candidate_tree_hash: String,
    pub environment_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReceipt {
    pub receipt_id: ReceiptId,
    pub nonce: String,
    pub run_id: String,
    pub check_id: CheckId,
    pub fingerprint: EvidenceFingerprint,
    pub verifier_version: String,
    pub payload: EvidencePayload,
    pub result: CheckOutcome,
}

impl EvidenceReceipt {
    /// Mirrors `completion::any_single_false_field_blocks_completion` in
    /// spirit: every fingerprint component is checked independently, and a
    /// mismatch on any one of them is sufficient to make the receipt stale
    /// — matching four out of five fields is not "close enough".
    pub fn is_valid_against(&self, current: &EvidenceFingerprint) -> bool {
        &self.fingerprint == current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint() -> EvidenceFingerprint {
        EvidenceFingerprint {
            contract_hash: "contract-1".into(),
            check_hash: "check-1".into(),
            project_rule_snapshot_hash: "rules-1".into(),
            candidate_tree_hash: "tree-1".into(),
            environment_class: "macos-15-arm64".into(),
        }
    }

    fn receipt() -> EvidenceReceipt {
        EvidenceReceipt {
            receipt_id: ReceiptId("EV-1".into()),
            nonce: "nonce-1".into(),
            run_id: "run-1".into(),
            check_id: CheckId("C-001".into()),
            fingerprint: fingerprint(),
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

    #[test]
    fn matching_fingerprint_is_valid() {
        assert!(receipt().is_valid_against(&fingerprint()));
    }

    #[test]
    fn any_single_fingerprint_mismatch_invalidates_receipt() {
        let r = receipt();
        let mut mismatched = fingerprint();
        mismatched.contract_hash = "contract-2".into();
        assert!(!r.is_valid_against(&mismatched));

        let mut mismatched = fingerprint();
        mismatched.check_hash = "check-2".into();
        assert!(!r.is_valid_against(&mismatched));

        let mut mismatched = fingerprint();
        mismatched.project_rule_snapshot_hash = "rules-2".into();
        assert!(!r.is_valid_against(&mismatched));

        let mut mismatched = fingerprint();
        mismatched.candidate_tree_hash = "tree-2".into();
        assert!(!r.is_valid_against(&mismatched));

        let mut mismatched = fingerprint();
        mismatched.environment_class = "linux-x86_64".into();
        assert!(!r.is_valid_against(&mismatched));
    }
}
