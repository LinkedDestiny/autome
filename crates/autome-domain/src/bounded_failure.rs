//! Bounded failure per plan §6.7.
//!
//! Reuses `run::RunHold::{Stalled, BudgetExhausted}` directly rather than
//! inventing parallel enums — this module only decides *when* those holds
//! apply, mirroring the same reuse discipline as `task.rs`'s
//! `status_projection: RunState`.
//!
//! Deliberately not modeled here: "禁止 `completed_with_gaps`；缩小目标必
//! 须创建新的 TaskContract 版本." `RunTerminal` (see `run.rs`) already has
//! no `CompletedWithGaps` variant, and a Requirement can only leave the
//! current must-set through `contract::apply_amendment`'s
//! `SupersedeRequirement` path — so "quietly narrowing scope without a new
//! contract version" is already structurally impossible, not something a
//! new predicate here would add value by re-checking.

use serde::{Deserialize, Serialize};

use crate::run::RunHold;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureFingerprint(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureOccurrence {
    pub fingerprint: FailureFingerprint,
    pub introduces_new_fact: bool,
}

/// "相同 failure fingerprint 连续 3 次且没有新增事实 → Stalled." Evaluated
/// against the trailing 3 occurrences of `history` (oldest first) — any
/// occurrence before that trailing window is irrelevant, since a
/// fingerprint change or a new fact anywhere in the last 3 resets the run.
pub fn evaluate_stall(history: &[FailureOccurrence]) -> Option<RunHold> {
    if history.len() < 3 {
        return None;
    }
    let trailing = &history[history.len() - 3..];
    let reference = &trailing[0].fingerprint;
    let all_same_with_no_new_facts = trailing
        .iter()
        .all(|o| &o.fingerprint == reference && !o.introduces_new_fact);
    if all_same_with_no_new_facts {
        Some(RunHold::Stalled)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureCategory {
    TransientHarnessError,
    BusinessFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedBackoffPolicy {
    base_delay_ms: u64,
    max_delay_ms: u64,
    max_retries: u32,
}

impl BoundedBackoffPolicy {
    pub fn new(base_delay_ms: u64, max_delay_ms: u64, max_retries: u32) -> Self {
        Self {
            base_delay_ms,
            max_delay_ms,
            max_retries,
        }
    }

    /// `attempt_index` is 0 for the first retry. Exponential growth is
    /// capped at `max_delay_ms` — "有界指数退避", not unbounded.
    pub fn delay_ms_for_attempt(&self, attempt_index: u32) -> u64 {
        let factor = 2u64.saturating_pow(attempt_index);
        self.base_delay_ms
            .saturating_mul(factor)
            .min(self.max_delay_ms)
    }

    pub fn may_retry(&self, attempt_index: u32) -> bool {
        attempt_index < self.max_retries
    }
}

/// "transient Harness 错误使用有界指数退避；业务失败不自动重试." A business
/// failure is never auto-retried regardless of policy or attempt index.
pub fn may_auto_retry(
    category: FailureCategory,
    attempt_index: u32,
    policy: &BoundedBackoffPolicy,
) -> bool {
    match category {
        FailureCategory::TransientHarnessError => policy.may_retry(attempt_index),
        FailureCategory::BusinessFailure => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DollarBudgetKind {
    Hard,
    Soft,
}

/// "美元上限只有在 Provider 提供可流式核对或服务端强制的用量时才标为
/// hard，基于本地估算的成本只能作为 soft alert." `classify` is the only
/// constructor: callers supply the fact about the provider, not the
/// classification itself, so a locally-estimated cost cannot be
/// mislabelled `Hard` by whoever assembles the policy snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DollarBudget {
    limit_cents: u64,
    kind: DollarBudgetKind,
}

impl DollarBudget {
    pub fn classify(
        limit_cents: u64,
        provider_usage_is_streamable_or_server_enforced: bool,
    ) -> Self {
        let kind = if provider_usage_is_streamable_or_server_enforced {
            DollarBudgetKind::Hard
        } else {
            DollarBudgetKind::Soft
        };
        Self { limit_cents, kind }
    }

    pub fn limit_cents(&self) -> u64 {
        self.limit_cents
    }

    pub fn kind(&self) -> DollarBudgetKind {
        self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenPolicySnapshot {
    pub max_attempts_per_node: u32,
    pub max_replan_count: u32,
    pub max_wall_clock_seconds: u64,
    pub max_turns: u32,
    pub max_tokens: u64,
    pub dollar_budget: Option<DollarBudget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BudgetUsage {
    pub wall_clock_seconds: u64,
    pub turns: u32,
    pub tokens: u64,
    pub dollars_spent_cents: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetLimitKind {
    WallClockSeconds,
    Turns,
    Tokens,
    Dollars,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BudgetCheckOutcome {
    pub hard_exhausted: Vec<BudgetLimitKind>,
    pub soft_alerts: Vec<BudgetLimitKind>,
}

impl BudgetCheckOutcome {
    /// "达到任一硬预算即进入 BudgetExhausted，不得自动追加." Auto top-up is
    /// prevented elsewhere by construction: the only way to add budget is
    /// `policy_restart::issue_budget_grant_receipt`, which requires a
    /// non-empty human `reason`/`operator` — there is no code path that
    /// grants more budget without that receipt.
    pub fn into_run_hold(&self) -> Option<RunHold> {
        if self.hard_exhausted.is_empty() {
            None
        } else {
            Some(RunHold::BudgetExhausted)
        }
    }
}

/// "墙钟时间、turn 和 token 上限始终由 Core 硬执行" — those three are
/// always checked as hard limits; the dollar limit's hardness is whatever
/// `DollarBudget::classify` already determined.
pub fn check_budget(snapshot: &FrozenPolicySnapshot, usage: &BudgetUsage) -> BudgetCheckOutcome {
    let mut outcome = BudgetCheckOutcome::default();

    if usage.wall_clock_seconds >= snapshot.max_wall_clock_seconds {
        outcome
            .hard_exhausted
            .push(BudgetLimitKind::WallClockSeconds);
    }
    if usage.turns >= snapshot.max_turns {
        outcome.hard_exhausted.push(BudgetLimitKind::Turns);
    }
    if usage.tokens >= snapshot.max_tokens {
        outcome.hard_exhausted.push(BudgetLimitKind::Tokens);
    }
    if let (Some(budget), Some(spent)) = (&snapshot.dollar_budget, usage.dollars_spent_cents)
        && spent >= budget.limit_cents()
    {
        match budget.kind() {
            DollarBudgetKind::Hard => outcome.hard_exhausted.push(BudgetLimitKind::Dollars),
            DollarBudgetKind::Soft => outcome.soft_alerts.push(BudgetLimitKind::Dollars),
        }
    }

    outcome
}

pub fn has_exceeded_node_attempt_limit(
    attempts_for_node: u32,
    snapshot: &FrozenPolicySnapshot,
) -> bool {
    attempts_for_node >= snapshot.max_attempts_per_node
}

pub fn has_exceeded_replan_limit(replan_count: u32, snapshot: &FrozenPolicySnapshot) -> bool {
    replan_count >= snapshot.max_replan_count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occ(fingerprint: &str, introduces_new_fact: bool) -> FailureOccurrence {
        FailureOccurrence {
            fingerprint: FailureFingerprint(fingerprint.to_string()),
            introduces_new_fact,
        }
    }

    #[test]
    fn fewer_than_three_occurrences_never_stalls() {
        assert_eq!(
            evaluate_stall(&[occ("fp-1", false), occ("fp-1", false)]),
            None
        );
    }

    #[test]
    fn three_identical_fingerprints_with_no_new_facts_is_stalled() {
        let history = [occ("fp-1", false), occ("fp-1", false), occ("fp-1", false)];
        assert_eq!(evaluate_stall(&history), Some(RunHold::Stalled));
    }

    #[test]
    fn a_new_fact_among_the_trailing_three_prevents_stall() {
        let history = [occ("fp-1", false), occ("fp-1", true), occ("fp-1", false)];
        assert_eq!(evaluate_stall(&history), None);
    }

    #[test]
    fn a_differing_fingerprint_among_the_trailing_three_prevents_stall() {
        let history = [occ("fp-1", false), occ("fp-2", false), occ("fp-1", false)];
        assert_eq!(evaluate_stall(&history), None);
    }

    #[test]
    fn only_the_trailing_three_matter() {
        let history = [
            occ("fp-9", false),
            occ("fp-1", false),
            occ("fp-1", false),
            occ("fp-1", false),
        ];
        assert_eq!(evaluate_stall(&history), Some(RunHold::Stalled));
    }

    #[test]
    fn transient_harness_errors_retry_within_the_bound() {
        let policy = BoundedBackoffPolicy::new(100, 1000, 3);
        assert!(may_auto_retry(
            FailureCategory::TransientHarnessError,
            0,
            &policy
        ));
        assert!(may_auto_retry(
            FailureCategory::TransientHarnessError,
            2,
            &policy
        ));
        assert!(!may_auto_retry(
            FailureCategory::TransientHarnessError,
            3,
            &policy
        ));
    }

    #[test]
    fn business_failures_never_auto_retry() {
        let policy = BoundedBackoffPolicy::new(100, 1000, 10);
        assert!(!may_auto_retry(
            FailureCategory::BusinessFailure,
            0,
            &policy
        ));
    }

    #[test]
    fn backoff_delay_grows_exponentially_but_is_capped() {
        let policy = BoundedBackoffPolicy::new(100, 450, 10);
        assert_eq!(policy.delay_ms_for_attempt(0), 100);
        assert_eq!(policy.delay_ms_for_attempt(1), 200);
        assert_eq!(policy.delay_ms_for_attempt(2), 400);
        assert_eq!(policy.delay_ms_for_attempt(3), 450); // would be 800, capped
    }

    #[test]
    fn dollar_budget_is_hard_only_when_provider_usage_is_authoritative() {
        let hard = DollarBudget::classify(1000, true);
        let soft = DollarBudget::classify(1000, false);
        assert_eq!(hard.kind(), DollarBudgetKind::Hard);
        assert_eq!(soft.kind(), DollarBudgetKind::Soft);
    }

    fn snapshot(dollar_budget: Option<DollarBudget>) -> FrozenPolicySnapshot {
        FrozenPolicySnapshot {
            max_attempts_per_node: 5,
            max_replan_count: 2,
            max_wall_clock_seconds: 3600,
            max_turns: 100,
            max_tokens: 1_000_000,
            dollar_budget,
        }
    }

    #[test]
    fn within_all_limits_is_not_exhausted() {
        let outcome = check_budget(
            &snapshot(Some(DollarBudget::classify(1000, true))),
            &BudgetUsage {
                wall_clock_seconds: 10,
                turns: 1,
                tokens: 10,
                dollars_spent_cents: Some(1),
            },
        );
        assert!(outcome.hard_exhausted.is_empty());
        assert!(outcome.soft_alerts.is_empty());
        assert_eq!(outcome.into_run_hold(), None);
    }

    #[test]
    fn wall_clock_turn_and_token_caps_are_always_hard() {
        let outcome = check_budget(
            &snapshot(None),
            &BudgetUsage {
                wall_clock_seconds: 3600,
                turns: 100,
                tokens: 1_000_000,
                dollars_spent_cents: None,
            },
        );
        assert_eq!(
            outcome.hard_exhausted,
            vec![
                BudgetLimitKind::WallClockSeconds,
                BudgetLimitKind::Turns,
                BudgetLimitKind::Tokens
            ]
        );
        assert_eq!(outcome.into_run_hold(), Some(RunHold::BudgetExhausted));
    }

    #[test]
    fn a_hard_dollar_budget_exhausts_but_a_soft_one_only_alerts() {
        let hard_outcome = check_budget(
            &snapshot(Some(DollarBudget::classify(500, true))),
            &BudgetUsage {
                dollars_spent_cents: Some(500),
                ..Default::default()
            },
        );
        assert_eq!(hard_outcome.hard_exhausted, vec![BudgetLimitKind::Dollars]);
        assert_eq!(hard_outcome.into_run_hold(), Some(RunHold::BudgetExhausted));

        let soft_outcome = check_budget(
            &snapshot(Some(DollarBudget::classify(500, false))),
            &BudgetUsage {
                dollars_spent_cents: Some(500),
                ..Default::default()
            },
        );
        assert!(soft_outcome.hard_exhausted.is_empty());
        assert_eq!(soft_outcome.soft_alerts, vec![BudgetLimitKind::Dollars]);
        assert_eq!(soft_outcome.into_run_hold(), None);
    }

    #[test]
    fn node_attempt_and_replan_limits_are_checked_independently() {
        let snap = snapshot(None);
        assert!(!has_exceeded_node_attempt_limit(4, &snap));
        assert!(has_exceeded_node_attempt_limit(5, &snap));
        assert!(!has_exceeded_replan_limit(1, &snap));
        assert!(has_exceeded_replan_limit(2, &snap));
    }
}
