//! Run/Attempt/AgentClaim/AttemptPermissionProfile per plan §5.6.
//!
//! Two invariants from §5.6 are mechanically enforced here rather than left
//! to convention:
//!
//! 1. "执行/repair/evaluation/final audit 不得绑定 PlanningRunSpec；批准前
//!    五个只读步骤不得获得 ExecutionRunSpec 的 candidate-write profile" —
//!    an `Attempt`'s `purpose` and `spec_binding` must agree (enforced by
//!    `Attempt::validate_shape`), and a Planning-bound `Attempt` can never
//!    be paired with a permission profile that grants filesystem writes
//!    (enforced by `validate_planning_attempt_is_read_only`).
//! 2. "AgentClaim...始终视为不可信输入" — `AgentClaim` has no method and no
//!    trait impl that produces an `EvidenceReceipt` or `AuditVerdict`; the
//!    only way evidence enters the system is through the check-execution
//!    path in `evidence.rs`, never through what an agent merely claims.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RunId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttemptId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LoopStepId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionProfileId(pub String);

/// An opaque reference to another aggregate/receipt whose full type is not
/// modeled yet (e.g. a `PlanApprovalReceipt` or a prior `ExecutionRunSpec`).
/// Kept distinct from a plain `String` so call sites can't accidentally
/// pass a hash where a ref was expected or vice versa.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Ref(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecHash(pub String);

/// §5.6's tagged origin for an `ExecutionRunSpec`: every non-initial Run
/// must reference the approval chain it came from rather than fabricating
/// a fresh initial planning story.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunOrigin {
    InitialPlanning {
        planning_spec_ref: Ref,
        plan_approval_receipt_ref: Ref,
    },
    PolicyAmendment {
        prior_execution_spec_ref: Ref,
        amendment_ref: Ref,
    },
    GraphReplan {
        prior_execution_spec_ref: Ref,
        replan_approval_ref: Ref,
    },
    ContractAmendment {
        prior_execution_spec_ref: Ref,
        contract_amendment_ref: Ref,
    },
}

/// What an `Attempt` is for. Only `Planning` may bind a `PlanningRunSpec`;
/// every other purpose is execution-phase and must bind an
/// `ExecutionRunSpec` (§5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptPurpose {
    Planning,
    Execution,
    Repair,
    Evaluation,
    FinalAudit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecBinding {
    Planning(SpecHash),
    Execution(SpecHash),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    pub id: AttemptId,
    pub loop_step_id: LoopStepId,
    pub node_id: Option<crate::graph::NodeId>,
    pub purpose: AttemptPurpose,
    pub spec_binding: SpecBinding,
    pub agent_execution_profile_hash: String,
    pub permission_profile_id: PermissionProfileId,
    pub harness_id: String,
    pub model_selection_identity_ref: Ref,
    pub qualification_receipt_ref: Ref,
    pub input_commit: String,
    pub input_tree_hash: String,
    pub skill_projection_fingerprint: String,
    pub provider_session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptShapeError {
    /// A `Planning`-purpose Attempt bound to an `Execution` spec, or vice
    /// versa — the two must always agree.
    PurposeSpecBindingMismatch {
        purpose: AttemptPurpose,
        binding_is_planning: bool,
    },
}

impl Attempt {
    pub fn validate_shape(&self) -> Result<(), AttemptShapeError> {
        let binding_is_planning = matches!(self.spec_binding, SpecBinding::Planning(_));
        let purpose_is_planning = matches!(self.purpose, AttemptPurpose::Planning);
        if binding_is_planning != purpose_is_planning {
            return Err(AttemptShapeError::PurposeSpecBindingMismatch {
                purpose: self.purpose,
                binding_is_planning,
            });
        }
        Ok(())
    }
}

/// §5.6: "AgentClaim 只包含声称完成的工作、声称执行的检查、阻塞项和建议下
/// 一步，始终视为不可信输入." Deliberately has no conversion to
/// `EvidenceReceipt` or `AuditVerdict` anywhere in this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentClaim {
    pub attempt_id: AttemptId,
    pub claimed_work: Vec<String>,
    pub claimed_checks_executed: Vec<String>,
    pub blockers: Vec<String>,
    pub suggested_next_steps: Vec<String>,
}

/// §7.1's `AttemptFailed`: "Harness 未产生有效候选" — a Core-observed fact
/// about whether this Attempt yielded anything to evaluate at all,
/// established independently of `AgentClaim` (which is never trusted, see
/// module doc). §7.1 marks this state "可作为通过：否" with no
/// recoverability caveat; that is enforced here by construction rather
/// than by a lookup table — `AttemptFailed` simply carries no
/// `candidate_tree_hash`, so nothing downstream that requires one (an
/// `EvidenceReceipt`, a `CandidateCertificate`) can be built from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttemptOutcome {
    ProducedCandidate { candidate_tree_hash: String },
    AttemptFailed,
}

impl AttemptOutcome {
    pub fn candidate_tree_hash(&self) -> Option<&str> {
        match self {
            AttemptOutcome::ProducedCandidate {
                candidate_tree_hash,
            } => Some(candidate_tree_hash),
            AttemptOutcome::AttemptFailed => None,
        }
    }

    /// §7.1: an `AttemptFailed` outcome may never proceed to evidence
    /// collection or audit — there is no candidate for either to examine.
    pub fn may_proceed_to_evaluation(&self) -> bool {
        self.candidate_tree_hash().is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ToolSurface {
    pub provider_available_tools: Vec<String>,
    pub provider_allowed_tools: Vec<String>,
    pub provider_denied_tools: Vec<String>,
    pub autome_control_tools: Vec<String>,
    pub dynamic_tool_or_mcp_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FilesystemPolicy {
    pub read_roots: Vec<String>,
    pub write_roots: Vec<String>,
    pub deny_roots: Vec<String>,
    pub nofollow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CommandPolicy {
    pub qualified_runner_ids: Vec<String>,
    pub argv_policy_hash: String,
    pub shell_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkMode {
    Denied,
    BrokeredOnly,
    AllowedDestinations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub mode: NetworkMode,
    pub allowed_brokers: Vec<String>,
    pub allowed_destinations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub mechanism: String,
    pub required_capabilities: Vec<String>,
    pub fail_closed: bool,
}

/// §5.6: "AttemptPermissionProfile 是每个步骤实际权限的唯一权威，而不是
/// Prompt 中的建议." Skills share one Attempt-level profile and can never
/// union permissions onto it or add tools — there is deliberately no
/// merge/union constructor anywhere in this module, only validation of a
/// profile that was built whole by the Core before the step ever runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptPermissionProfile {
    pub id: PermissionProfileId,
    pub loop_step_id: LoopStepId,
    pub node_id: Option<crate::graph::NodeId>,
    pub adapter_id: String,
    pub installation_id: String,
    pub subject_scope_hash: String,
    pub skill_set_snapshot_hash: String,
    pub tool_surface: ToolSurface,
    pub filesystem_policy: FilesystemPolicy,
    pub command_policy: CommandPolicy,
    pub network_policy: NetworkPolicy,
    pub sandbox_policy: SandboxPolicy,
    pub secret_policy_hash: String,
    pub safety_policy_hash: String,
    pub profile_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionProfileViolation {
    /// `provider_allowed_tools` contains a tool that is not in
    /// `provider_available_tools` — the profile is claiming a grant the
    /// CLI never actually exposed.
    AllowedToolsExceedAvailable(Vec<String>),
    /// A tool appears in both `provider_allowed_tools` and
    /// `provider_denied_tools` — the profile contradicts itself.
    AllowedAndDeniedOverlap(Vec<String>),
}

impl AttemptPermissionProfile {
    pub fn validate(&self) -> Vec<PermissionProfileViolation> {
        let mut violations = Vec::new();

        let available: std::collections::HashSet<&String> =
            self.tool_surface.provider_available_tools.iter().collect();
        let exceeding: Vec<String> = self
            .tool_surface
            .provider_allowed_tools
            .iter()
            .filter(|t| !available.contains(t))
            .cloned()
            .collect();
        if !exceeding.is_empty() {
            violations.push(PermissionProfileViolation::AllowedToolsExceedAvailable(
                exceeding,
            ));
        }

        let denied: std::collections::HashSet<&String> =
            self.tool_surface.provider_denied_tools.iter().collect();
        let overlap: Vec<String> = self
            .tool_surface
            .provider_allowed_tools
            .iter()
            .filter(|t| denied.contains(t))
            .cloned()
            .collect();
        if !overlap.is_empty() {
            violations.push(PermissionProfileViolation::AllowedAndDeniedOverlap(overlap));
        }

        violations
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanningWriteViolation {
    /// A `Planning`-bound Attempt was paired with a profile that grants at
    /// least one filesystem write root — §5.6: "批准前五个只读步骤不得获得
    /// ExecutionRunSpec 的 candidate-write profile."
    PlanningAttemptHasWriteRoots(Vec<String>),
}

pub fn validate_planning_attempt_is_read_only(
    attempt: &Attempt,
    profile: &AttemptPermissionProfile,
) -> Result<(), PlanningWriteViolation> {
    if matches!(attempt.spec_binding, SpecBinding::Planning(_))
        && !profile.filesystem_policy.write_roots.is_empty()
    {
        return Err(PlanningWriteViolation::PlanningAttemptHasWriteRoots(
            profile.filesystem_policy.write_roots.clone(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeId;

    fn base_attempt(purpose: AttemptPurpose, binding: SpecBinding) -> Attempt {
        Attempt {
            id: AttemptId("attempt-1".into()),
            loop_step_id: LoopStepId("step-1".into()),
            node_id: Some(NodeId("node-1".into())),
            purpose,
            spec_binding: binding,
            agent_execution_profile_hash: "hash-agent".into(),
            permission_profile_id: PermissionProfileId("perm-1".into()),
            harness_id: "claude-code".into(),
            model_selection_identity_ref: Ref("model-1".into()),
            qualification_receipt_ref: Ref("qual-1".into()),
            input_commit: "deadbeef".into(),
            input_tree_hash: "treehash".into(),
            skill_projection_fingerprint: "skillfp".into(),
            provider_session_id: "session-1".into(),
        }
    }

    fn base_profile(write_roots: Vec<String>) -> AttemptPermissionProfile {
        AttemptPermissionProfile {
            id: PermissionProfileId("perm-1".into()),
            loop_step_id: LoopStepId("step-1".into()),
            node_id: Some(NodeId("node-1".into())),
            adapter_id: "claude-code".into(),
            installation_id: "install-1".into(),
            subject_scope_hash: "scope".into(),
            skill_set_snapshot_hash: "skillset".into(),
            tool_surface: ToolSurface {
                provider_available_tools: vec!["Read".into(), "Bash".into()],
                provider_allowed_tools: vec!["Read".into()],
                provider_denied_tools: vec!["Bash".into()],
                autome_control_tools: vec![],
                dynamic_tool_or_mcp_allowlist: vec![],
            },
            filesystem_policy: FilesystemPolicy {
                read_roots: vec!["/project".into()],
                write_roots,
                deny_roots: vec![],
                nofollow: true,
            },
            command_policy: CommandPolicy::default(),
            network_policy: NetworkPolicy {
                mode: NetworkMode::Denied,
                allowed_brokers: vec![],
                allowed_destinations: vec![],
            },
            sandbox_policy: SandboxPolicy {
                mechanism: "seatbelt".into(),
                required_capabilities: vec![],
                fail_closed: true,
            },
            secret_policy_hash: "secret".into(),
            safety_policy_hash: "safety".into(),
            profile_hash: "profile".into(),
        }
    }

    #[test]
    fn planning_purpose_with_planning_binding_is_valid() {
        let attempt = base_attempt(
            AttemptPurpose::Planning,
            SpecBinding::Planning(SpecHash("plan-hash".into())),
        );
        assert!(attempt.validate_shape().is_ok());
    }

    #[test]
    fn execution_purpose_with_planning_binding_is_rejected() {
        let attempt = base_attempt(
            AttemptPurpose::Execution,
            SpecBinding::Planning(SpecHash("plan-hash".into())),
        );
        assert!(matches!(
            attempt.validate_shape(),
            Err(AttemptShapeError::PurposeSpecBindingMismatch { .. })
        ));
    }

    #[test]
    fn planning_purpose_with_execution_binding_is_rejected() {
        let attempt = base_attempt(
            AttemptPurpose::Planning,
            SpecBinding::Execution(SpecHash("exec-hash".into())),
        );
        assert!(matches!(
            attempt.validate_shape(),
            Err(AttemptShapeError::PurposeSpecBindingMismatch { .. })
        ));
    }

    #[test]
    fn repair_purpose_with_execution_binding_is_valid() {
        let attempt = base_attempt(
            AttemptPurpose::Repair,
            SpecBinding::Execution(SpecHash("exec-hash".into())),
        );
        assert!(attempt.validate_shape().is_ok());
    }

    #[test]
    fn allowed_tools_beyond_available_is_flagged() {
        let mut profile = base_profile(vec![]);
        profile
            .tool_surface
            .provider_allowed_tools
            .push("Write".into());
        let violations = profile.validate();
        assert!(violations.iter().any(
            |v| matches!(v, PermissionProfileViolation::AllowedToolsExceedAvailable(t) if t == &vec!["Write".to_string()])
        ));
    }

    #[test]
    fn allowed_and_denied_overlap_is_flagged() {
        let mut profile = base_profile(vec![]);
        profile
            .tool_surface
            .provider_denied_tools
            .push("Read".into());
        let violations = profile.validate();
        assert!(violations.iter().any(
            |v| matches!(v, PermissionProfileViolation::AllowedAndDeniedOverlap(t) if t == &vec!["Read".to_string()])
        ));
    }

    #[test]
    fn well_formed_profile_has_no_violations() {
        let profile = base_profile(vec!["/workdir".into()]);
        assert!(profile.validate().is_empty());
    }

    #[test]
    fn planning_attempt_with_write_roots_is_rejected() {
        let attempt = base_attempt(
            AttemptPurpose::Planning,
            SpecBinding::Planning(SpecHash("plan-hash".into())),
        );
        let profile = base_profile(vec!["/workdir".into()]);
        assert!(matches!(
            validate_planning_attempt_is_read_only(&attempt, &profile),
            Err(PlanningWriteViolation::PlanningAttemptHasWriteRoots(_))
        ));
    }

    #[test]
    fn planning_attempt_with_no_write_roots_is_accepted() {
        let attempt = base_attempt(
            AttemptPurpose::Planning,
            SpecBinding::Planning(SpecHash("plan-hash".into())),
        );
        let profile = base_profile(vec![]);
        assert!(validate_planning_attempt_is_read_only(&attempt, &profile).is_ok());
    }

    #[test]
    fn execution_attempt_with_write_roots_is_accepted() {
        let attempt = base_attempt(
            AttemptPurpose::Execution,
            SpecBinding::Execution(SpecHash("exec-hash".into())),
        );
        let profile = base_profile(vec!["/workdir".into()]);
        assert!(validate_planning_attempt_is_read_only(&attempt, &profile).is_ok());
    }

    #[test]
    fn produced_candidate_exposes_its_tree_hash_and_may_proceed() {
        let outcome = AttemptOutcome::ProducedCandidate {
            candidate_tree_hash: "treehash-1".into(),
        };
        assert_eq!(outcome.candidate_tree_hash(), Some("treehash-1"));
        assert!(outcome.may_proceed_to_evaluation());
    }

    #[test]
    fn attempt_failed_has_no_candidate_and_may_not_proceed() {
        let outcome = AttemptOutcome::AttemptFailed;
        assert_eq!(outcome.candidate_tree_hash(), None);
        assert!(!outcome.may_proceed_to_evaluation());
    }
}
