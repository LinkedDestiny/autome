//! Requirement per plan §5.3. A Requirement's `passed` state is never stored
//! here — per the plan it "不存储可由 Agent 修改的 `passed` 字段；状态由当前
//! 有效 EvidenceReceipt 动态计算" (a Requirement never carries a mutable
//! pass/fail flag; that's derived elsewhere from EvidenceReceipts). This
//! module only holds the immutable-once-frozen shape of a Requirement.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequirementId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CheckId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RequirementKind {
    Functional,
    NonFunctional,
    Constraint,
    Prohibition,
    Deliverable,
    HumanJudgement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Necessity {
    Must,
    Optional,
}

/// 2.0.0 fixes acceptance_logic to all_of (plan §5.3/§5.4): no runtime
/// any_of. This is a unit type rather than an enum on purpose — there is
/// nothing else to select, and adding a variant later is a deliberate,
/// visible plan change, not a quiet default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllOf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliverySpec {
    TrackedInTree {
        paths: Vec<String>,
    },
    ContentAddressedArtifact {
        artifact_ids: Vec<String>,
        destination_policy: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAnchor {
    /// Character range in the original raw text, an attachment location, or
    /// a user-decision event ref — kept as an opaque string at this layer;
    /// the precise anchor union type lands with TaskContract's full I/O.
    pub anchor_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    pub id: RequirementId,
    pub statement: String,
    pub kind: RequirementKind,
    pub necessity: Necessity,
    pub source_anchors: Vec<SourceAnchor>,
    pub acceptance_logic: AllOf,
    pub acceptance_check_ids: Vec<CheckId>,
    pub delivery_spec: Option<DeliverySpec>,
    pub risk_level: RiskLevel,
    pub superseded_by: Option<RequirementId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementShapeError {
    /// kind=deliverable requires delivery_spec (plan §5.3: "delivery_spec?
    /// // kind=deliverable 时必填").
    DeliverableRequiresDeliverySpec,
    /// A must requirement with zero acceptance checks can never satisfy the
    /// §7 completion gate's `all_must_requirements_have_checks`.
    MustRequirementHasNoAcceptanceChecks,
    /// source_anchors[] must be non-empty: an unanchored requirement cannot
    /// be traced back to the original request for the §7 coverage matrix.
    MissingSourceAnchor,
}

impl Requirement {
    pub fn validate_shape(&self) -> Result<(), RequirementShapeError> {
        if self.kind == RequirementKind::Deliverable && self.delivery_spec.is_none() {
            return Err(RequirementShapeError::DeliverableRequiresDeliverySpec);
        }
        if self.necessity == Necessity::Must && self.acceptance_check_ids.is_empty() {
            return Err(RequirementShapeError::MustRequirementHasNoAcceptanceChecks);
        }
        if self.source_anchors.is_empty() {
            return Err(RequirementShapeError::MissingSourceAnchor);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_requirement() -> Requirement {
        Requirement {
            id: RequirementId("R-001".into()),
            statement: "Users can export a CSV of their orders".into(),
            kind: RequirementKind::Functional,
            necessity: Necessity::Must,
            source_anchors: vec![SourceAnchor {
                anchor_ref: "raw_text[120..168]".into(),
            }],
            acceptance_logic: AllOf,
            acceptance_check_ids: vec![CheckId("C-001".into())],
            delivery_spec: None,
            risk_level: RiskLevel::Medium,
            superseded_by: None,
        }
    }

    #[test]
    fn well_formed_requirement_passes_shape_validation() {
        assert_eq!(base_requirement().validate_shape(), Ok(()));
    }

    #[test]
    fn deliverable_without_delivery_spec_is_rejected() {
        let mut req = base_requirement();
        req.kind = RequirementKind::Deliverable;
        req.delivery_spec = None;
        assert_eq!(
            req.validate_shape(),
            Err(RequirementShapeError::DeliverableRequiresDeliverySpec)
        );
    }

    #[test]
    fn deliverable_with_delivery_spec_is_accepted() {
        let mut req = base_requirement();
        req.kind = RequirementKind::Deliverable;
        req.delivery_spec = Some(DeliverySpec::TrackedInTree {
            paths: vec!["dist/report.csv".into()],
        });
        assert_eq!(req.validate_shape(), Ok(()));
    }

    #[test]
    fn must_requirement_without_checks_is_rejected() {
        let mut req = base_requirement();
        req.acceptance_check_ids.clear();
        assert_eq!(
            req.validate_shape(),
            Err(RequirementShapeError::MustRequirementHasNoAcceptanceChecks)
        );
    }

    #[test]
    fn optional_requirement_without_checks_is_allowed() {
        let mut req = base_requirement();
        req.necessity = Necessity::Optional;
        req.acceptance_check_ids.clear();
        assert_eq!(req.validate_shape(), Ok(()));
    }

    #[test]
    fn unanchored_requirement_is_rejected() {
        let mut req = base_requirement();
        req.source_anchors.clear();
        assert_eq!(
            req.validate_shape(),
            Err(RequirementShapeError::MissingSourceAnchor)
        );
    }
}
