//! TaskContract per plan §5.2/§5.4, and the D5 amendment mechanics
//! ("任务原文不可覆盖，契约只能版本化修订"): a frozen contract cannot be
//! mutated in place — the only way to change one is `apply_amendment`,
//! which returns a brand-new, higher-versioned `TaskContract` linked back
//! to its predecessor. There is deliberately no `&mut self` mutator and no
//! "remove requirement" operation anywhere in this module's public API;
//! semantic removal is modeled as `Requirement::superseded_by`, which keeps
//! the superseded requirement in the record rather than deleting it.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::requirement::{CheckId, Necessity, Requirement, RequirementId, RequirementShapeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ContractVersion(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractStatus {
    Draft,
    Frozen,
}

/// §5.4: 2.0.0 只实现四种通用检查. Each variant carries only the fields that
/// distinguish it; the declarations every check must make regardless of
/// kind (mandatory, expected observation, ...) live on `AcceptanceCheck`
/// itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckKind {
    Process,
    File,
    Git,
    UserDecision,
}

/// §5.4: "每项 Check 必须声明：关联 Requirement、mandatory、预期观察、负向
/// 场景、所需环境等级、隔离策略、重复策略、库存策略和 freshness policy."
/// `ExecutableOracleSnapshot` binding for ProcessCheck is deferred to the
/// Run/verifier layer (§5.4 continues into execution-time binding, which
/// this crate — pure domain, no I/O — does not own).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCheck {
    pub id: CheckId,
    pub kind: CheckKind,
    pub requirement_id: RequirementId,
    pub mandatory: bool,
    pub expected_observation: String,
    pub negative_scenario: String,
    pub required_environment_level: String,
    pub isolation_policy: String,
    pub repeat_policy: String,
    pub inventory_policy: String,
    pub freshness_policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskContract {
    pub id: String,
    pub version: ContractVersion,
    pub content_hash: String,
    pub status: ContractStatus,
    pub previous_version: Option<ContractVersion>,
    pub requirements: Vec<Requirement>,
    pub acceptance_checks: Vec<AcceptanceCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractFreezeViolation {
    RequirementShape {
        requirement: RequirementId,
        error: RequirementShapeError,
    },
    RequirementReferencesUnknownCheck {
        requirement: RequirementId,
        check: CheckId,
    },
    MustRequirementHasNoMandatoryCheck {
        requirement: RequirementId,
    },
    CheckReferencesUnknownRequirement {
        check: CheckId,
        requirement: RequirementId,
    },
}

/// §5.2/§5.3 freeze-time validation. This is the contract-level analogue of
/// `graph::validate_for_freeze`: it does not decide policy, it mechanically
/// checks that the draft is internally consistent before it becomes
/// immutable.
pub fn validate_for_freeze(contract: &TaskContract) -> Vec<ContractFreezeViolation> {
    let mut violations = Vec::new();
    let check_ids: HashMap<&CheckId, &AcceptanceCheck> = contract
        .acceptance_checks
        .iter()
        .map(|c| (&c.id, c))
        .collect();
    let requirement_ids: HashMap<&RequirementId, &Requirement> =
        contract.requirements.iter().map(|r| (&r.id, r)).collect();

    for requirement in &contract.requirements {
        if let Err(error) = requirement.validate_shape() {
            violations.push(ContractFreezeViolation::RequirementShape {
                requirement: requirement.id.clone(),
                error,
            });
        }
        for check_id in &requirement.acceptance_check_ids {
            if !check_ids.contains_key(check_id) {
                violations.push(ContractFreezeViolation::RequirementReferencesUnknownCheck {
                    requirement: requirement.id.clone(),
                    check: check_id.clone(),
                });
            }
        }
        if requirement.necessity == Necessity::Must && requirement.superseded_by.is_none() {
            let has_mandatory_check = requirement
                .acceptance_check_ids
                .iter()
                .any(|check_id| check_ids.get(check_id).is_some_and(|check| check.mandatory));
            if !has_mandatory_check {
                violations.push(
                    ContractFreezeViolation::MustRequirementHasNoMandatoryCheck {
                        requirement: requirement.id.clone(),
                    },
                );
            }
        }
    }

    for check in &contract.acceptance_checks {
        if !requirement_ids.contains_key(&check.requirement_id) {
            violations.push(ContractFreezeViolation::CheckReferencesUnknownRequirement {
                check: check.id.clone(),
                requirement: check.requirement_id.clone(),
            });
        }
    }

    violations
}

/// Freezes a draft contract. Consumes the draft by value and returns an
/// owned, `Frozen`-status contract on success — there is no in-place
/// `freeze(&mut self)`, so a caller cannot hold onto a stale `&mut Draft`
/// reference and mutate it after this call.
pub fn freeze(mut contract: TaskContract) -> Result<TaskContract, Vec<ContractFreezeViolation>> {
    let violations = validate_for_freeze(&contract);
    if !violations.is_empty() {
        return Err(violations);
    }
    contract.status = ContractStatus::Frozen;
    Ok(contract)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmendmentChange {
    AddRequirement(Requirement),
    SupersedeRequirement {
        requirement_id: RequirementId,
        superseded_by: RequirementId,
        replacement: Requirement,
    },
    AddAcceptanceCheck(AcceptanceCheck),
}

/// D5: append-only. `reason` and `user_decision_ref` are required so that
/// "delete a requirement / lower environment level / accept a functional
/// gap / relax acceptance" always carries the explicit user decision the
/// plan demands — this type has no variant that changes contract semantics
/// without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractAmendment {
    pub base_version: ContractVersion,
    pub reason: String,
    pub user_decision_ref: String,
    pub change: AmendmentChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmendmentError {
    ContractNotFrozen,
    BaseVersionMismatch {
        expected: ContractVersion,
        actual: ContractVersion,
    },
    MissingUserDecisionRef,
    UnknownRequirement(RequirementId),
}

/// Applies an amendment to a frozen contract, producing a new *Draft*
/// contract one version higher (it must pass `freeze` again before it is
/// authoritative — an amendment does not get to skip the same shape checks
/// the original contract had to pass). The input contract is left
/// untouched; nothing here mutates `contract` in place.
pub fn apply_amendment(
    contract: &TaskContract,
    amendment: ContractAmendment,
) -> Result<TaskContract, AmendmentError> {
    if contract.status != ContractStatus::Frozen {
        return Err(AmendmentError::ContractNotFrozen);
    }
    if contract.version != amendment.base_version {
        return Err(AmendmentError::BaseVersionMismatch {
            expected: contract.version,
            actual: amendment.base_version,
        });
    }
    if amendment.user_decision_ref.trim().is_empty() {
        return Err(AmendmentError::MissingUserDecisionRef);
    }

    let mut next = contract.clone();
    next.previous_version = Some(contract.version);
    next.version = ContractVersion(contract.version.0 + 1);
    next.status = ContractStatus::Draft;

    match amendment.change {
        AmendmentChange::AddRequirement(requirement) => {
            next.requirements.push(requirement);
        }
        AmendmentChange::SupersedeRequirement {
            requirement_id,
            superseded_by,
            replacement,
        } => {
            let existing = next
                .requirements
                .iter_mut()
                .find(|r| r.id == requirement_id)
                .ok_or(AmendmentError::UnknownRequirement(requirement_id))?;
            existing.superseded_by = Some(superseded_by);
            next.requirements.push(replacement);
        }
        AmendmentChange::AddAcceptanceCheck(check) => {
            next.acceptance_checks.push(check);
        }
    }

    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirement::{Necessity, RequirementKind, SourceAnchor};

    fn anchored_requirement(id: &str, necessity: Necessity, check_id: Option<&str>) -> Requirement {
        Requirement {
            id: RequirementId(id.into()),
            statement: "does something".into(),
            kind: RequirementKind::Functional,
            necessity,
            source_anchors: vec![SourceAnchor {
                anchor_ref: "raw_text:0-10".into(),
            }],
            acceptance_logic: crate::requirement::AllOf,
            acceptance_check_ids: check_id
                .map(|c| vec![CheckId(c.into())])
                .unwrap_or_default(),
            delivery_spec: None,
            risk_level: crate::requirement::RiskLevel::Low,
            superseded_by: None,
        }
    }

    fn mandatory_check(id: &str, requirement_id: &str) -> AcceptanceCheck {
        AcceptanceCheck {
            id: CheckId(id.into()),
            kind: CheckKind::Process,
            requirement_id: RequirementId(requirement_id.into()),
            mandatory: true,
            expected_observation: "exit code 0".into(),
            negative_scenario: "non-zero exit".into(),
            required_environment_level: "base".into(),
            isolation_policy: "worktree".into(),
            repeat_policy: "once".into(),
            inventory_policy: "track".into(),
            freshness_policy: "must-be-current".into(),
        }
    }

    fn draft_contract(
        requirements: Vec<Requirement>,
        checks: Vec<AcceptanceCheck>,
    ) -> TaskContract {
        TaskContract {
            id: "T-1".into(),
            version: ContractVersion(1),
            content_hash: "hash".into(),
            status: ContractStatus::Draft,
            previous_version: None,
            requirements,
            acceptance_checks: checks,
        }
    }

    #[test]
    fn well_formed_draft_freezes_cleanly() {
        let contract = draft_contract(
            vec![anchored_requirement(
                "R-001",
                Necessity::Must,
                Some("C-001"),
            )],
            vec![mandatory_check("C-001", "R-001")],
        );
        let frozen = freeze(contract).expect("should freeze");
        assert_eq!(frozen.status, ContractStatus::Frozen);
    }

    #[test]
    fn must_requirement_without_mandatory_check_blocks_freeze() {
        let contract = draft_contract(
            vec![anchored_requirement("R-001", Necessity::Must, None)],
            vec![],
        );
        let violations = freeze(contract).unwrap_err();
        assert!(violations.iter().any(|v| matches!(
            v,
            ContractFreezeViolation::MustRequirementHasNoMandatoryCheck { .. }
        )));
    }

    #[test]
    fn requirement_referencing_unknown_check_blocks_freeze() {
        let contract = draft_contract(
            vec![anchored_requirement(
                "R-001",
                Necessity::Must,
                Some("C-ghost"),
            )],
            vec![],
        );
        let violations = freeze(contract).unwrap_err();
        assert!(violations.iter().any(|v| matches!(
            v,
            ContractFreezeViolation::RequirementReferencesUnknownCheck { .. }
        )));
    }

    #[test]
    fn check_referencing_unknown_requirement_blocks_freeze() {
        let contract = draft_contract(vec![], vec![mandatory_check("C-001", "R-ghost")]);
        let violations = freeze(contract).unwrap_err();
        assert!(violations.iter().any(|v| matches!(
            v,
            ContractFreezeViolation::CheckReferencesUnknownRequirement { .. }
        )));
    }

    fn frozen_baseline() -> TaskContract {
        freeze(draft_contract(
            vec![anchored_requirement(
                "R-001",
                Necessity::Must,
                Some("C-001"),
            )],
            vec![mandatory_check("C-001", "R-001")],
        ))
        .unwrap()
    }

    #[test]
    fn amendment_on_non_frozen_contract_is_rejected() {
        let draft = draft_contract(vec![], vec![]);
        let amendment = ContractAmendment {
            base_version: ContractVersion(1),
            reason: "test".into(),
            user_decision_ref: "decision:1".into(),
            change: AmendmentChange::AddAcceptanceCheck(mandatory_check("C-002", "R-001")),
        };
        assert_eq!(
            apply_amendment(&draft, amendment).unwrap_err(),
            AmendmentError::ContractNotFrozen
        );
    }

    #[test]
    fn amendment_with_wrong_base_version_is_rejected() {
        let frozen = frozen_baseline();
        let amendment = ContractAmendment {
            base_version: ContractVersion(99),
            reason: "test".into(),
            user_decision_ref: "decision:1".into(),
            change: AmendmentChange::AddAcceptanceCheck(mandatory_check("C-002", "R-001")),
        };
        assert!(matches!(
            apply_amendment(&frozen, amendment).unwrap_err(),
            AmendmentError::BaseVersionMismatch { .. }
        ));
    }

    #[test]
    fn amendment_without_user_decision_ref_is_rejected() {
        let frozen = frozen_baseline();
        let amendment = ContractAmendment {
            base_version: ContractVersion(1),
            reason: "test".into(),
            user_decision_ref: "   ".into(),
            change: AmendmentChange::AddAcceptanceCheck(mandatory_check("C-002", "R-001")),
        };
        assert_eq!(
            apply_amendment(&frozen, amendment).unwrap_err(),
            AmendmentError::MissingUserDecisionRef
        );
    }

    #[test]
    fn amendment_bumps_version_and_links_previous() {
        let frozen = frozen_baseline();
        let amendment = ContractAmendment {
            base_version: ContractVersion(1),
            reason: "add non-functional requirement".into(),
            user_decision_ref: "decision:42".into(),
            change: AmendmentChange::AddRequirement(anchored_requirement(
                "R-002",
                Necessity::Optional,
                None,
            )),
        };
        let amended = apply_amendment(&frozen, amendment).unwrap();
        assert_eq!(amended.version, ContractVersion(2));
        assert_eq!(amended.previous_version, Some(ContractVersion(1)));
        assert_eq!(amended.status, ContractStatus::Draft);
        assert_eq!(amended.requirements.len(), 2);
    }

    #[test]
    fn superseding_a_requirement_keeps_it_in_the_record() {
        let frozen = frozen_baseline();
        let replacement = anchored_requirement("R-001b", Necessity::Must, Some("C-001"));
        let amendment = ContractAmendment {
            base_version: ContractVersion(1),
            reason: "user narrowed scope".into(),
            user_decision_ref: "decision:7".into(),
            change: AmendmentChange::SupersedeRequirement {
                requirement_id: RequirementId("R-001".into()),
                superseded_by: RequirementId("R-001b".into()),
                replacement,
            },
        };
        let amended = apply_amendment(&frozen, amendment).unwrap();
        // Append-only: the original requirement is still present and now
        // carries superseded_by, it is never removed from the vector.
        assert_eq!(amended.requirements.len(), 2);
        let original = amended
            .requirements
            .iter()
            .find(|r| r.id == RequirementId("R-001".into()))
            .unwrap();
        assert_eq!(original.superseded_by, Some(RequirementId("R-001b".into())));
    }

    #[test]
    fn superseding_unknown_requirement_is_rejected() {
        let frozen = frozen_baseline();
        let replacement = anchored_requirement("R-999", Necessity::Must, Some("C-001"));
        let amendment = ContractAmendment {
            base_version: ContractVersion(1),
            reason: "test".into(),
            user_decision_ref: "decision:1".into(),
            change: AmendmentChange::SupersedeRequirement {
                requirement_id: RequirementId("R-ghost".into()),
                superseded_by: RequirementId("R-999".into()),
                replacement,
            },
        };
        assert_eq!(
            apply_amendment(&frozen, amendment).unwrap_err(),
            AmendmentError::UnknownRequirement(RequirementId("R-ghost".into()))
        );
    }
}
