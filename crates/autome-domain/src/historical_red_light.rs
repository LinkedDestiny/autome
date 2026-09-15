//! §7.1 失败语义 and §7.2 已有仓库的历史红灯.
//!
//! §7.1's table names 13 failure states and asserts every one of them
//! "否" (may never count as a pass) — most already have a concrete home
//! elsewhere in this crate rather than a new parallel enum here:
//! `CheckFailed`/`Inconclusive` are `evidence::CheckOutcome::{Fail,
//! Inconclusive}`; `Blocked`/`ConfigurationInvalidated`/`BudgetExhausted`/
//! `Stalled` are `run::RunHold` variants; `Infeasible`/`Cancelled` are
//! `run::RunTerminal` variants (its `ProtocolFailed` is the same real event
//! as this table's `ProtocolViolation`, named slightly differently at the
//! Run-terminal layer). None of those types have a "this counts as green"
//! escape hatch anywhere in the crate, which is how "may never count as
//! pass" is actually enforced — not by a lookup table.
//!
//! `AttemptFailed` ("Harness 未产生有效候选") is deliberately not modeled
//! here: it is a single boolean fact recorded by whichever `automed`-side
//! code drives the Harness, with no further pure-domain rule attached to
//! it, so inventing a type for it here would add a name without adding a
//! check.
//!
//! What *is* new: the three states that only make sense relative to a
//! pre-existing baseline failure — `Unreproduced`, `KnownBaselineFailure`,
//! `NewRegression` — and §7.2's gate for completing a task in a repository
//! that already has red lights, which is exactly the mechanical rule that
//! turns a `KnownBaselineFailure` classification into permission to
//! disclose-and-proceed instead of blocking.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureClassification {
    Unreproduced,
    KnownBaselineFailure,
    NewRegression,
}

/// "`Unreproduced`：在指定条件下没有复现报告现象——不等于不存在."
/// "`KnownBaselineFailure`：修改前已记录，最终 fingerprint 相同且证明不在
/// 影响面." "`NewRegression`：相对 baseline 新增或变化的失败."
pub fn classify_pre_existing_failure(
    reproduced_under_specified_conditions: bool,
    final_fingerprint_matches_baseline: bool,
) -> FailureClassification {
    if !reproduced_under_specified_conditions {
        FailureClassification::Unreproduced
    } else if final_fingerprint_matches_baseline {
        FailureClassification::KnownBaselineFailure
    } else {
        FailureClassification::NewRegression
    }
}

/// The seven conditions from §7.2, each a fact the caller has already
/// established elsewhere (Core-side capture, independent reviewer
/// confirmation, final auditor decision, etc.) — "单独的 LLM 看起来不相关
/// 判断不够", so there is deliberately no field here for a bare LLM
/// opinion; only `impact_scope_check_confirmed_by_independent_reviewer`
/// (a real Check confirmed by a real independent reviewer) counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HistoricalRedLightAssessment {
    pub failure_captured_by_core_before_change: bool,
    pub final_fingerprint_matches_baseline: bool,
    pub no_new_failures_skips_or_filtered: bool,
    pub impact_scope_check_confirmed_by_independent_reviewer: bool,
    pub impact_scope_check_passes_on_final_tree: bool,
    pub final_auditor_explicitly_accepted: bool,
    pub completion_certificate_fully_discloses: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoricalRedLightViolation {
    FailureNotCapturedBeforeChange,
    FingerprintDiffersFromBaseline,
    NewFailuresSkipsOrFilteredIntroduced,
    ImpactScopeCheckNotIndependentlyConfirmed,
    ImpactScopeCheckDidNotPassOnFinalTree,
    FinalAuditorDidNotExplicitlyAccept,
    CompletionCertificateDoesNotFullyDisclose,
}

/// Collects every unmet condition rather than stopping at the first, in
/// keeping with the rest of this crate's validation functions.
pub fn evaluate_historical_red_light(
    assessment: &HistoricalRedLightAssessment,
) -> Vec<HistoricalRedLightViolation> {
    let mut violations = Vec::new();
    if !assessment.failure_captured_by_core_before_change {
        violations.push(HistoricalRedLightViolation::FailureNotCapturedBeforeChange);
    }
    if !assessment.final_fingerprint_matches_baseline {
        violations.push(HistoricalRedLightViolation::FingerprintDiffersFromBaseline);
    }
    if !assessment.no_new_failures_skips_or_filtered {
        violations.push(HistoricalRedLightViolation::NewFailuresSkipsOrFilteredIntroduced);
    }
    if !assessment.impact_scope_check_confirmed_by_independent_reviewer {
        violations.push(HistoricalRedLightViolation::ImpactScopeCheckNotIndependentlyConfirmed);
    }
    if !assessment.impact_scope_check_passes_on_final_tree {
        violations.push(HistoricalRedLightViolation::ImpactScopeCheckDidNotPassOnFinalTree);
    }
    if !assessment.final_auditor_explicitly_accepted {
        violations.push(HistoricalRedLightViolation::FinalAuditorDidNotExplicitlyAccept);
    }
    if !assessment.completion_certificate_fully_discloses {
        violations.push(HistoricalRedLightViolation::CompletionCertificateDoesNotFullyDisclose);
    }
    violations
}

pub fn may_complete_with_historical_failures(assessment: &HistoricalRedLightAssessment) -> bool {
    evaluate_historical_red_light(assessment).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_reproduced_is_classified_as_unreproduced_regardless_of_fingerprint() {
        assert_eq!(
            classify_pre_existing_failure(false, true),
            FailureClassification::Unreproduced
        );
        assert_eq!(
            classify_pre_existing_failure(false, false),
            FailureClassification::Unreproduced
        );
    }

    #[test]
    fn reproduced_with_matching_fingerprint_is_known_baseline_failure() {
        assert_eq!(
            classify_pre_existing_failure(true, true),
            FailureClassification::KnownBaselineFailure
        );
    }

    #[test]
    fn reproduced_with_differing_fingerprint_is_new_regression() {
        assert_eq!(
            classify_pre_existing_failure(true, false),
            FailureClassification::NewRegression
        );
    }

    fn all_satisfied() -> HistoricalRedLightAssessment {
        HistoricalRedLightAssessment {
            failure_captured_by_core_before_change: true,
            final_fingerprint_matches_baseline: true,
            no_new_failures_skips_or_filtered: true,
            impact_scope_check_confirmed_by_independent_reviewer: true,
            impact_scope_check_passes_on_final_tree: true,
            final_auditor_explicitly_accepted: true,
            completion_certificate_fully_discloses: true,
        }
    }

    #[test]
    fn all_conditions_satisfied_may_complete() {
        assert!(may_complete_with_historical_failures(&all_satisfied()));
        assert!(evaluate_historical_red_light(&all_satisfied()).is_empty());
    }

    type BreakCondition = fn(&mut HistoricalRedLightAssessment);

    #[test]
    fn any_single_unmet_condition_blocks_completion() {
        let cases: Vec<(BreakCondition, HistoricalRedLightViolation)> = vec![
            (
                |a| a.failure_captured_by_core_before_change = false,
                HistoricalRedLightViolation::FailureNotCapturedBeforeChange,
            ),
            (
                |a| a.final_fingerprint_matches_baseline = false,
                HistoricalRedLightViolation::FingerprintDiffersFromBaseline,
            ),
            (
                |a| a.no_new_failures_skips_or_filtered = false,
                HistoricalRedLightViolation::NewFailuresSkipsOrFilteredIntroduced,
            ),
            (
                |a| a.impact_scope_check_confirmed_by_independent_reviewer = false,
                HistoricalRedLightViolation::ImpactScopeCheckNotIndependentlyConfirmed,
            ),
            (
                |a| a.impact_scope_check_passes_on_final_tree = false,
                HistoricalRedLightViolation::ImpactScopeCheckDidNotPassOnFinalTree,
            ),
            (
                |a| a.final_auditor_explicitly_accepted = false,
                HistoricalRedLightViolation::FinalAuditorDidNotExplicitlyAccept,
            ),
            (
                |a| a.completion_certificate_fully_discloses = false,
                HistoricalRedLightViolation::CompletionCertificateDoesNotFullyDisclose,
            ),
        ];
        for (break_condition, expected_violation) in cases {
            let mut assessment = all_satisfied();
            break_condition(&mut assessment);
            assert!(!may_complete_with_historical_failures(&assessment));
            assert_eq!(
                evaluate_historical_red_light(&assessment),
                vec![expected_violation]
            );
        }
    }

    #[test]
    fn multiple_unmet_conditions_are_all_collected() {
        let mut assessment = all_satisfied();
        assessment.final_auditor_explicitly_accepted = false;
        assessment.completion_certificate_fully_discloses = false;
        let violations = evaluate_historical_red_light(&assessment);
        assert_eq!(
            violations,
            vec![
                HistoricalRedLightViolation::FinalAuditorDidNotExplicitlyAccept,
                HistoricalRedLightViolation::CompletionCertificateDoesNotFullyDisclose,
            ]
        );
    }
}
