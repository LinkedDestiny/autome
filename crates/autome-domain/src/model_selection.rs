//! ModelSelectionIdentity / QualificationReceipt per plan §5.10.
//!
//! `HarnessCapabilitySnapshot` itself is populated by real CLI probing
//! (Codex app-server `model/list`, Claude CLI launch probes, auth state,
//! sandbox/tool capability discovery) — live system-observation surface
//! that belongs in `automed` talking to real subprocesses. This module only
//! carries the resulting data shapes plus the mechanical rules the plan
//! pins on top of them: the producer/evaluator `model_choice_key_hash`
//! separation gate, the qualification receipt validity window, and the
//! sealed-pass² same-batch time window.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::attempt::LoopStepId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapability {
    pub opaque_id: String,
    pub resolved_wire_name: Option<String>,
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
}

/// §5.10: raw capability observation for one Harness adapter installation.
/// Never constructed by this crate — `automed` fills this in from a real
/// probe and hands it in as already-observed fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCapabilitySnapshot {
    pub adapter_id: String,
    pub installation_id: String,
    pub canonical_binary_path: String,
    pub binary_digest: String,
    pub binary_version: String,
    pub binary_signature: String,
    pub protocol_hash: String,
    pub schema_hash: String,
    pub auth_mode: String,
    pub auth_state: String,
    pub lifecycle_capabilities: Vec<String>,
    pub tool_capabilities: Vec<String>,
    pub sandbox_capabilities: Vec<String>,
    pub resume_capabilities: Vec<String>,
    pub usage_capabilities: Vec<String>,
    pub models: Vec<ModelCapability>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub valid_until: OffsetDateTime,
    pub snapshot_digest: String,
}

/// §5.10: "AttemptPermissionProfile、resolved config、Skill 投影、
/// adapter/installation、账号、service tier 与 Effort 单独进入
/// `runtime_selection_hash`，不进入'不同模型'比较" — `model_choice_key_hash`
/// and `runtime_selection_hash` are deliberately two separate fields so a
/// caller can never accidentally fold runtime-only differences into the
/// producer/evaluator separation gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSelectionIdentity {
    pub adapter_id: String,
    pub installation_id: String,
    pub provider: String,
    pub cli_hash: String,
    pub protocol_hash: String,
    pub schema_hash: String,
    pub model_id: String,
    pub resolved_wire_name: Option<String>,
    pub service_tier: Option<String>,
    pub provider_native_effort: String,
    pub auth_mode: String,
    pub account_fingerprint: String,
    pub exposed_snapshot_or_fingerprint: Option<String>,
    pub qualification_batch_id: String,
    /// `None` means the alias could not be resolved to a stable identity —
    /// §5.10: "alias 无法解析到稳定 identity 时该组合不能用于分离门." Such an
    /// identity can still exist and run, it just can never satisfy
    /// `validate_model_separation`.
    pub model_choice_key_hash: Option<String>,
    pub runtime_selection_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualificationResult {
    Qualified,
    NotQualified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationReceipt {
    pub identity: ModelSelectionIdentity,
    pub harness_capability_snapshot_digest: String,
    pub account_capability_snapshot_digest: String,
    pub canary_manifest_hash: String,
    pub run_ids: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub issued_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub valid_until: OffsetDateTime,
    pub result: QualificationResult,
    pub receipt_digest: String,
}

/// §5.10: "Provider 未提供不可变 snapshot 时，资格收据最长有效 7 天."
pub const MAX_VALIDITY_WITHOUT_IMMUTABLE_SNAPSHOT: Duration = Duration::days(7);

/// §5.10: "sealed pass² 的两次 Run 必须在同一资格批次的 24 小时内完成."
pub const SEALED_PASS_WINDOW: Duration = Duration::hours(24);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualificationReceiptError {
    ValidUntilBeforeIssuedAt,
    ValidityWindowExceedsSevenDaysWithoutImmutableSnapshot,
}

/// Sole constructor for `QualificationReceipt` — enforces the plan's
/// validity-window ceiling at issuance rather than leaving it to callers to
/// remember. `provider_snapshot_is_immutable` reflects whether the Provider
/// gave an immutable capability snapshot for this batch; when it did not,
/// the window is capped at seven days.
#[allow(clippy::too_many_arguments)]
pub fn issue_qualification_receipt(
    identity: ModelSelectionIdentity,
    harness_capability_snapshot_digest: String,
    account_capability_snapshot_digest: String,
    canary_manifest_hash: String,
    run_ids: Vec<String>,
    issued_at: OffsetDateTime,
    valid_until: OffsetDateTime,
    provider_snapshot_is_immutable: bool,
    result: QualificationResult,
    receipt_digest: String,
) -> Result<QualificationReceipt, QualificationReceiptError> {
    if valid_until < issued_at {
        return Err(QualificationReceiptError::ValidUntilBeforeIssuedAt);
    }
    if !provider_snapshot_is_immutable
        && valid_until - issued_at > MAX_VALIDITY_WITHOUT_IMMUTABLE_SNAPSHOT
    {
        return Err(
            QualificationReceiptError::ValidityWindowExceedsSevenDaysWithoutImmutableSnapshot,
        );
    }
    Ok(QualificationReceipt {
        identity,
        harness_capability_snapshot_digest,
        account_capability_snapshot_digest,
        canary_manifest_hash,
        run_ids,
        issued_at,
        valid_until,
        result,
        receipt_digest,
    })
}

impl QualificationReceipt {
    pub fn is_valid_at(&self, now: OffsetDateTime) -> bool {
        matches!(self.result, QualificationResult::Qualified) && now <= self.valid_until
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealedPassWindowError {
    RunsSpanMoreThanTwentyFourHours,
}

pub fn validate_sealed_pass_window(
    first_run_at: OffsetDateTime,
    second_run_at: OffsetDateTime,
) -> Result<(), SealedPassWindowError> {
    let span = if second_run_at >= first_run_at {
        second_run_at - first_run_at
    } else {
        first_run_at - second_run_at
    };
    if span > SEALED_PASS_WINDOW {
        return Err(SealedPassWindowError::RunsSpanMoreThanTwentyFourHours);
    }
    Ok(())
}

/// §5.10: "首版要求 `contract_drafting ≠ contract_review`、
/// `task_graph_planning ≠ graph_review`、`implementation ≠
/// node_evaluation/final_audit` 的 `model_choice_key_hash`." Step ids match
/// the nine-entry table in §10.1.
pub const REQUIRED_MODEL_SEPARATION_PAIRS: &[(&str, &str)] = &[
    ("contract_drafting", "contract_review"),
    ("task_graph_planning", "graph_review"),
    ("implementation", "node_evaluation"),
    ("implementation", "final_audit"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSeparationViolation {
    pub step_a: LoopStepId,
    pub step_b: LoopStepId,
}

/// Checks every required pair that currently has both sides routed. A step
/// missing from the map entirely is not yet routed and is not this gate's
/// concern (a separate schema-completeness check owns "all nine LoopStepIds
/// must be routed before the Scheduler starts"). A step whose hash is
/// `None` (unresolved alias) can never satisfy separation, so it always
/// reports a violation for any pair it appears in — mirroring "该组合不能用
/// 于分离门" rather than silently treating it as vacuously distinct.
pub fn validate_model_separation(
    model_choice_key_hash_by_step: &HashMap<LoopStepId, Option<String>>,
) -> Vec<ModelSeparationViolation> {
    let mut violations = Vec::new();
    for (a, b) in REQUIRED_MODEL_SEPARATION_PAIRS {
        let step_a = LoopStepId((*a).to_string());
        let step_b = LoopStepId((*b).to_string());
        let hash_a = model_choice_key_hash_by_step.get(&step_a);
        let hash_b = model_choice_key_hash_by_step.get(&step_b);
        match (hash_a, hash_b) {
            (Some(Some(ha)), Some(Some(hb))) if ha != hb => {}
            (Some(_), Some(_)) => violations.push(ModelSeparationViolation { step_a, step_b }),
            _ => {}
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(model_choice_key_hash: Option<&str>) -> ModelSelectionIdentity {
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
            model_choice_key_hash: model_choice_key_hash.map(|s| s.to_string()),
            runtime_selection_hash: "runtime-1".into(),
        }
    }

    fn issued_at() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
    }

    #[test]
    fn separation_gate_passes_when_required_pairs_have_distinct_hashes() {
        let mut by_step = HashMap::new();
        by_step.insert(LoopStepId("contract_drafting".into()), Some("a".into()));
        by_step.insert(LoopStepId("contract_review".into()), Some("b".into()));
        by_step.insert(LoopStepId("task_graph_planning".into()), Some("a".into()));
        by_step.insert(LoopStepId("graph_review".into()), Some("b".into()));
        by_step.insert(LoopStepId("implementation".into()), Some("a".into()));
        by_step.insert(LoopStepId("node_evaluation".into()), Some("b".into()));
        by_step.insert(LoopStepId("final_audit".into()), Some("c".into()));
        assert_eq!(validate_model_separation(&by_step), vec![]);
    }

    #[test]
    fn separation_gate_flags_identical_hashes_on_a_required_pair() {
        let mut by_step = HashMap::new();
        by_step.insert(LoopStepId("contract_drafting".into()), Some("same".into()));
        by_step.insert(LoopStepId("contract_review".into()), Some("same".into()));
        let violations = validate_model_separation(&by_step);
        assert_eq!(
            violations,
            vec![ModelSeparationViolation {
                step_a: LoopStepId("contract_drafting".into()),
                step_b: LoopStepId("contract_review".into()),
            }]
        );
    }

    #[test]
    fn separation_gate_flags_implementation_reused_for_either_evaluator_step() {
        let mut by_step = HashMap::new();
        by_step.insert(LoopStepId("implementation".into()), Some("x".into()));
        by_step.insert(LoopStepId("node_evaluation".into()), Some("x".into()));
        by_step.insert(LoopStepId("final_audit".into()), Some("x".into()));
        let violations = validate_model_separation(&by_step);
        assert_eq!(violations.len(), 2);
        assert!(violations.contains(&ModelSeparationViolation {
            step_a: LoopStepId("implementation".into()),
            step_b: LoopStepId("node_evaluation".into()),
        }));
        assert!(violations.contains(&ModelSeparationViolation {
            step_a: LoopStepId("implementation".into()),
            step_b: LoopStepId("final_audit".into()),
        }));
    }

    #[test]
    fn separation_gate_treats_unresolved_alias_as_a_violation() {
        let mut by_step = HashMap::new();
        by_step.insert(LoopStepId("contract_drafting".into()), None);
        by_step.insert(LoopStepId("contract_review".into()), Some("b".into()));
        let violations = validate_model_separation(&by_step);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn separation_gate_ignores_pairs_where_a_step_is_not_yet_routed() {
        let mut by_step = HashMap::new();
        by_step.insert(LoopStepId("contract_drafting".into()), Some("a".into()));
        // contract_review absent entirely — not routed yet, not this gate's job.
        assert_eq!(validate_model_separation(&by_step), vec![]);
    }

    #[test]
    fn qualification_receipt_rejects_valid_until_before_issued_at() {
        let err = issue_qualification_receipt(
            identity(Some("hash-1")),
            "harness-digest".into(),
            "account-digest".into(),
            "canary-1".into(),
            vec!["run-1".into()],
            issued_at(),
            issued_at() - Duration::seconds(1),
            false,
            QualificationResult::Qualified,
            "receipt-digest".into(),
        )
        .unwrap_err();
        assert_eq!(err, QualificationReceiptError::ValidUntilBeforeIssuedAt);
    }

    #[test]
    fn qualification_receipt_rejects_more_than_seven_days_without_immutable_snapshot() {
        let err = issue_qualification_receipt(
            identity(Some("hash-1")),
            "harness-digest".into(),
            "account-digest".into(),
            "canary-1".into(),
            vec!["run-1".into()],
            issued_at(),
            issued_at() + Duration::days(8),
            false,
            QualificationResult::Qualified,
            "receipt-digest".into(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            QualificationReceiptError::ValidityWindowExceedsSevenDaysWithoutImmutableSnapshot
        );
    }

    #[test]
    fn qualification_receipt_allows_long_window_with_immutable_snapshot() {
        let receipt = issue_qualification_receipt(
            identity(Some("hash-1")),
            "harness-digest".into(),
            "account-digest".into(),
            "canary-1".into(),
            vec!["run-1".into()],
            issued_at(),
            issued_at() + Duration::days(30),
            true,
            QualificationResult::Qualified,
            "receipt-digest".into(),
        )
        .unwrap();
        assert_eq!(receipt.result, QualificationResult::Qualified);
    }

    #[test]
    fn qualification_receipt_is_valid_before_expiry_when_qualified() {
        let receipt = issue_qualification_receipt(
            identity(Some("hash-1")),
            "harness-digest".into(),
            "account-digest".into(),
            "canary-1".into(),
            vec!["run-1".into()],
            issued_at(),
            issued_at() + Duration::days(1),
            false,
            QualificationResult::Qualified,
            "receipt-digest".into(),
        )
        .unwrap();
        assert!(receipt.is_valid_at(issued_at() + Duration::hours(1)));
        assert!(!receipt.is_valid_at(issued_at() + Duration::days(2)));
    }

    #[test]
    fn qualification_receipt_never_valid_when_not_qualified() {
        let receipt = issue_qualification_receipt(
            identity(Some("hash-1")),
            "harness-digest".into(),
            "account-digest".into(),
            "canary-1".into(),
            vec!["run-1".into()],
            issued_at(),
            issued_at() + Duration::days(1),
            false,
            QualificationResult::NotQualified,
            "receipt-digest".into(),
        )
        .unwrap();
        assert!(!receipt.is_valid_at(issued_at()));
    }

    #[test]
    fn sealed_pass_window_accepts_runs_within_twenty_four_hours_in_either_order() {
        assert!(
            validate_sealed_pass_window(issued_at(), issued_at() + Duration::hours(23)).is_ok()
        );
        assert!(
            validate_sealed_pass_window(issued_at() + Duration::hours(23), issued_at()).is_ok()
        );
    }

    #[test]
    fn sealed_pass_window_rejects_runs_more_than_twenty_four_hours_apart() {
        let err = validate_sealed_pass_window(issued_at(), issued_at() + Duration::hours(25))
            .unwrap_err();
        assert_eq!(err, SealedPassWindowError::RunsSpanMoreThanTwentyFourHours);
    }
}
