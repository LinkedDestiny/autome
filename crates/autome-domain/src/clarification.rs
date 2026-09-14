//! §6.4 提问与默认假设: Core may proceed on a default assumption only when
//! every one of the plan's seven conditions holds, and even then a
//! separately-enumerated set of situations forces a pause regardless (a
//! defense-in-depth check, in case a concrete situation is not otherwise
//! fully captured by the seven general booleans).
//!
//! "未决重大假设不能进入 Completed" is `no_unresolved_material_assumptions`
//! below, feeding the final completion gate's
//! `no_open_blocking_question_or_material_assumption` condition (§7).

use serde::{Deserialize, Serialize};

/// The plan's seven necessary conditions for a default assumption:
/// "可逆、影响局部、不改变用户目标、不删除或放宽验收、不产生高风险外部动作、
/// 能在本 Run 验证，并且有稳定惯例支持".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultAssumptionCandidate {
    pub reversible: bool,
    pub locally_scoped: bool,
    pub preserves_user_goal: bool,
    pub preserves_acceptance_criteria: bool,
    pub no_high_risk_external_action: bool,
    pub verifiable_within_this_run: bool,
    pub has_stable_convention_support: bool,
}

impl DefaultAssumptionCandidate {
    pub fn satisfies_all_conditions(&self) -> bool {
        self.reversible
            && self.locally_scoped
            && self.preserves_user_goal
            && self.preserves_acceptance_criteria
            && self.no_high_risk_external_action
            && self.verifiable_within_this_run
            && self.has_stable_convention_support
    }
}

/// The plan's enumerated situations that "必须暂停受影响节点并提问" —
/// present alongside (not instead of) the seven-condition check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MandatoryClarificationTrigger {
    MultipleChoicesAffectUserVisibleResultOrAcceptance,
    ContradictoryUserRequirements,
    NoFalsifiableCompletionCondition,
    MissingRequiredResourceCredentialAuthorizationOrTargetEnvironment,
    InvolvesReleaseExternalWriteMigrationDeletionPaymentSecurityOrPrivacy,
    RequiresDeletingRequirementLoweringEnvironmentOrAcceptingFunctionalGap,
    ConflictingCodeFactsAndUserDescriptionWithNoAuthoritativeOrder,
}

/// True only when no mandatory trigger fired *and* every general condition
/// holds. Either one alone is not sufficient — a candidate that satisfies
/// all seven conditions still must pause if a mandatory trigger is present.
pub fn may_proceed_with_default_assumption(
    candidate: &DefaultAssumptionCandidate,
    mandatory_triggers: &[MandatoryClarificationTrigger],
) -> bool {
    mandatory_triggers.is_empty() && candidate.satisfies_all_conditions()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialAssumption {
    pub id: String,
    pub statement: String,
    pub resolved: bool,
}

/// §7's `no_open_blocking_question_or_material_assumption`: true only when
/// every listed assumption has actually been resolved.
pub fn no_unresolved_material_assumptions(assumptions: &[MaterialAssumption]) -> bool {
    assumptions.iter().all(|a| a.resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_conditions_met() -> DefaultAssumptionCandidate {
        DefaultAssumptionCandidate {
            reversible: true,
            locally_scoped: true,
            preserves_user_goal: true,
            preserves_acceptance_criteria: true,
            no_high_risk_external_action: true,
            verifiable_within_this_run: true,
            has_stable_convention_support: true,
        }
    }

    #[test]
    fn assumption_is_allowed_when_every_condition_holds_and_no_trigger_fired() {
        assert!(may_proceed_with_default_assumption(
            &all_conditions_met(),
            &[]
        ));
    }

    #[test]
    fn assumption_is_refused_when_any_single_condition_fails() {
        let base = all_conditions_met();

        let mut c = base;
        c.reversible = false;
        assert!(!may_proceed_with_default_assumption(&c, &[]));

        let mut c = base;
        c.locally_scoped = false;
        assert!(!may_proceed_with_default_assumption(&c, &[]));

        let mut c = base;
        c.preserves_user_goal = false;
        assert!(!may_proceed_with_default_assumption(&c, &[]));

        let mut c = base;
        c.preserves_acceptance_criteria = false;
        assert!(!may_proceed_with_default_assumption(&c, &[]));

        let mut c = base;
        c.no_high_risk_external_action = false;
        assert!(!may_proceed_with_default_assumption(&c, &[]));

        let mut c = base;
        c.verifiable_within_this_run = false;
        assert!(!may_proceed_with_default_assumption(&c, &[]));

        let mut c = base;
        c.has_stable_convention_support = false;
        assert!(!may_proceed_with_default_assumption(&c, &[]));
    }

    #[test]
    fn a_mandatory_trigger_forces_a_pause_even_if_all_conditions_hold() {
        assert!(!may_proceed_with_default_assumption(
            &all_conditions_met(),
            &[MandatoryClarificationTrigger::ContradictoryUserRequirements]
        ));
    }

    #[test]
    fn no_unresolved_assumptions_is_true_for_an_empty_list() {
        assert!(no_unresolved_material_assumptions(&[]));
    }

    #[test]
    fn no_unresolved_assumptions_is_true_when_all_are_resolved() {
        let assumptions = vec![MaterialAssumption {
            id: "assumption-1".into(),
            statement: "Assumed default port 8080".into(),
            resolved: true,
        }];
        assert!(no_unresolved_material_assumptions(&assumptions));
    }

    #[test]
    fn an_open_assumption_blocks_completion() {
        let assumptions = vec![
            MaterialAssumption {
                id: "assumption-1".into(),
                statement: "Assumed default port 8080".into(),
                resolved: true,
            },
            MaterialAssumption {
                id: "assumption-2".into(),
                statement: "Assumed SQLite over Postgres".into(),
                resolved: false,
            },
        ];
        assert!(!no_unresolved_material_assumptions(&assumptions));
    }
}
