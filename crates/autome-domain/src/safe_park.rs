//! `SafeParkReceipt` per plan §6.2 (line 112, 1112-1121).
//!
//! "hold=Paused 只能通过 SafeParkReceipt 建立": a Run may only be parked once
//! every guard below is true — no active tool call, no verifier/preview
//! process, no open Environment/Skill/git transaction. `PrepareShutdown` and
//! Environment/Skill updates that touch an active Run reuse this exact gate
//! rather than inventing separate parking semantics (plan's explicit
//! instruction not to special-case those callers).
//!
//! `provider_session`/`outcome`/"candidate tree" are modeled as one
//! `Option<ProviderSessionSnapshot>` rather than three independently-optional
//! fields: `attempt::AttemptOutcome` already carries `candidate_tree_hash`
//! inside its `ProducedCandidate` arm, so pairing a `provider_session_id`
//! with that existing type is enough — a park can also happen with no
//! Attempt in flight at all (e.g. parking a merely-queued Task), hence the
//! outer `Option`.
//!
//! `receipt_digest`/`parked_at` are opaque caller-supplied strings, the same
//! pattern `review::build_human_review_receipt` uses for `receipt_digest`/
//! `decided_at` — this crate has no I/O and does not hash or timestamp
//! anything itself (plan D2).

use serde::{Deserialize, Serialize};

use crate::attempt::{AttemptOutcome, RunId};
use crate::run::RunPhase;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSessionSnapshot {
    pub provider_session_id: String,
    pub outcome: AttemptOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeParkReceipt {
    pub run: RunId,
    pub planning_spec_hash: String,
    pub execution_spec_hash: Option<String>,
    pub prior_phase: RunPhase,
    pub durable_checkpoint: String,
    pub provider_session: Option<ProviderSessionSnapshot>,
    pub no_active_tool_call: bool,
    pub no_verifier_or_preview_process: bool,
    pub no_environment_skill_git_transaction: bool,
    pub released_leases: Vec<String>,
    pub parked_at: String,
    pub receipt_digest: String,
}

impl SafeParkReceipt {
    /// Re-derivable from stored data rather than trusted purely by
    /// construction: a receipt loaded back from SQLite/JSON has already
    /// bypassed the constructor once, so callers that gate a lease release
    /// on this (see `execution_queue::release_lease_via_safe_park`) must be
    /// able to re-check it at the boundary.
    pub fn all_guards_satisfied(&self) -> bool {
        self.no_active_tool_call
            && self.no_verifier_or_preview_process
            && self.no_environment_skill_git_transaction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeParkReceiptError {
    ActiveToolCallStillPresent,
    VerifierOrPreviewProcessStillRunning,
    EnvironmentSkillGitTransactionStillOpen,
}

/// Sole constructor for `SafeParkReceipt`. Refuses to build a receipt whose
/// own guards are not all true — a receipt only ever exists to *record* a
/// successful park, never a failed attempt at one.
#[allow(clippy::too_many_arguments)]
pub fn build_safe_park_receipt(
    run: RunId,
    planning_spec_hash: &str,
    execution_spec_hash: Option<&str>,
    prior_phase: RunPhase,
    durable_checkpoint: &str,
    provider_session: Option<ProviderSessionSnapshot>,
    no_active_tool_call: bool,
    no_verifier_or_preview_process: bool,
    no_environment_skill_git_transaction: bool,
    released_leases: Vec<String>,
    parked_at: &str,
    receipt_digest: &str,
) -> Result<SafeParkReceipt, SafeParkReceiptError> {
    if !no_active_tool_call {
        return Err(SafeParkReceiptError::ActiveToolCallStillPresent);
    }
    if !no_verifier_or_preview_process {
        return Err(SafeParkReceiptError::VerifierOrPreviewProcessStillRunning);
    }
    if !no_environment_skill_git_transaction {
        return Err(SafeParkReceiptError::EnvironmentSkillGitTransactionStillOpen);
    }
    Ok(SafeParkReceipt {
        run,
        planning_spec_hash: planning_spec_hash.to_string(),
        execution_spec_hash: execution_spec_hash.map(str::to_string),
        prior_phase,
        durable_checkpoint: durable_checkpoint.to_string(),
        provider_session,
        no_active_tool_call,
        no_verifier_or_preview_process,
        no_environment_skill_git_transaction,
        released_leases,
        parked_at: parked_at.to_string(),
        receipt_digest: receipt_digest.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_receipt() -> Result<SafeParkReceipt, SafeParkReceiptError> {
        build_safe_park_receipt(
            RunId("run-1".to_string()),
            "planning-hash-1",
            Some("execution-hash-1"),
            RunPhase::Executing,
            "checkpoint-1",
            Some(ProviderSessionSnapshot {
                provider_session_id: "session-1".to_string(),
                outcome: AttemptOutcome::ProducedCandidate {
                    candidate_tree_hash: "tree-1".to_string(),
                },
            }),
            true,
            true,
            true,
            vec!["lease-1".to_string()],
            "2026-09-14T00:00:00Z",
            "digest-1",
        )
    }

    #[test]
    fn all_true_guards_build_a_receipt_whose_guards_report_satisfied() {
        let receipt = valid_receipt().unwrap();
        assert!(receipt.all_guards_satisfied());
        assert_eq!(receipt.released_leases, vec!["lease-1".to_string()]);
    }

    #[test]
    fn a_receipt_can_be_built_with_no_attempt_in_flight() {
        let receipt = build_safe_park_receipt(
            RunId("run-1".to_string()),
            "planning-hash-1",
            None,
            RunPhase::Received,
            "checkpoint-0",
            None,
            true,
            true,
            true,
            vec![],
            "2026-09-14T00:00:00Z",
            "digest-2",
        )
        .unwrap();
        assert!(receipt.provider_session.is_none());
        assert!(receipt.execution_spec_hash.is_none());
    }

    #[test]
    fn an_active_tool_call_refuses_to_build_a_receipt() {
        let err = build_safe_park_receipt(
            RunId("run-1".to_string()),
            "planning-hash-1",
            None,
            RunPhase::Executing,
            "checkpoint-1",
            None,
            false,
            true,
            true,
            vec![],
            "2026-09-14T00:00:00Z",
            "digest-3",
        )
        .unwrap_err();
        assert_eq!(err, SafeParkReceiptError::ActiveToolCallStillPresent);
    }

    #[test]
    fn a_lingering_verifier_or_preview_process_refuses_to_build_a_receipt() {
        let err = build_safe_park_receipt(
            RunId("run-1".to_string()),
            "planning-hash-1",
            None,
            RunPhase::Executing,
            "checkpoint-1",
            None,
            true,
            false,
            true,
            vec![],
            "2026-09-14T00:00:00Z",
            "digest-4",
        )
        .unwrap_err();
        assert_eq!(
            err,
            SafeParkReceiptError::VerifierOrPreviewProcessStillRunning
        );
    }

    #[test]
    fn an_open_environment_skill_git_transaction_refuses_to_build_a_receipt() {
        let err = build_safe_park_receipt(
            RunId("run-1".to_string()),
            "planning-hash-1",
            None,
            RunPhase::Executing,
            "checkpoint-1",
            None,
            true,
            true,
            false,
            vec![],
            "2026-09-14T00:00:00Z",
            "digest-5",
        )
        .unwrap_err();
        assert_eq!(
            err,
            SafeParkReceiptError::EnvironmentSkillGitTransactionStillOpen
        );
    }
}
