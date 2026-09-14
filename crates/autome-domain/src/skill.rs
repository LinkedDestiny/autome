//! Skill Inventory / Marketplace / SkillSetSnapshot per plan §5.11.
//!
//! Nearly all of §5.11 is real system-integration surface that belongs in
//! `automed`: filesystem inventory scanning, sandboxed fetch-to-quarantine,
//! static audit scanners, native CLI projection (Codex `agents/openai.yaml`,
//! Claude `SKILL.md`/`skillOverrides`) and projection probes against real
//! subprocesses. This module only carries the mechanical, I/O-free rules
//! the plan pins on top of that surface: the audit-verdict vocabulary that
//! forbids ever claiming "safe", the fact that installation always lands in
//! a disabled Vault entry and never a binding, the monotonic evidence
//! ladder (`Installed → Bound → Discoverable → AvailableToAttempt →
//! Invoked → Effective`), GlobalSkillBinding/ProjectSkillBinding
//! resolution, and garbage-collection eligibility.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::attempt::LoopStepId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillDigest(pub String);

/// §5.11: "结论只能是'未发现已知风险'，不能写'安全'." There is deliberately
/// no `Safe`/`Clean` variant — the vocabulary constraint is the type
/// itself, not a runtime string check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillAuditOutcome {
    NoKnownRisksFound,
    RisksFound { findings: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInstallReceipt {
    pub package_digest: String,
    pub audit_outcome: SkillAuditOutcome,
    pub plan_digest: String,
    pub user_approval_decision_ref: String,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillInstallError {
    MissingUserApprovalDecision,
}

/// Sole path from an audited package to a Vault entry. §5.11: "安装成功的
/// 默认终态只是 Vault 中 `Installed (disabled)`，不得创建 binding 或成为
/// AvailableToAttempt" — this always returns a ladder with only `installed`
/// set; there is no parameter that can produce a Bound or further ladder
/// state from here. The audit outcome does not have to be
/// `NoKnownRisksFound` to install (the plan's flow still runs installation
/// through a separate User Approval step even when risks were found); what
/// is mandatory is that a real approval decision was recorded.
pub fn issue_skill_install_receipt(
    package_digest: &str,
    audit_outcome: SkillAuditOutcome,
    plan_digest: &str,
    user_approval_decision_ref: &str,
    receipt_digest: &str,
) -> Result<(SkillInstallReceipt, SkillEvidenceLadder), SkillInstallError> {
    if user_approval_decision_ref.trim().is_empty() {
        return Err(SkillInstallError::MissingUserApprovalDecision);
    }
    let receipt = SkillInstallReceipt {
        package_digest: package_digest.to_string(),
        audit_outcome,
        plan_digest: plan_digest.to_string(),
        user_approval_decision_ref: user_approval_decision_ref.to_string(),
        receipt_digest: receipt_digest.to_string(),
    };
    let ladder = SkillEvidenceLadder::installed(SkillDigest(package_digest.to_string()));
    Ok((receipt, ladder))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillEvidenceLevel {
    Installed,
    Bound,
    Discoverable,
    AvailableToAttempt,
    Invoked,
}

/// §5.11: "证据等级分开显示：Installed/Bound/Discoverable/
/// AvailableToAttempt/Invoked/Effective... 安装或调用 Skill 都不等于任务正确."
/// Each level's `mark_*` method requires the preceding fact already holds,
/// so it is structurally impossible to claim e.g. Invoked without ever
/// having been Discoverable — but each fact is still tracked and displayed
/// independently rather than collapsed into one state enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillEvidenceLadder {
    pub skill_digest: SkillDigest,
    pub installed: bool,
    pub bound: bool,
    pub discoverable: bool,
    pub available_to_attempt: bool,
    pub invoked: bool,
    pub effective: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillEvidenceError {
    PrecedingLevelMissing(SkillEvidenceLevel),
}

impl SkillEvidenceLadder {
    pub fn installed(skill_digest: SkillDigest) -> Self {
        Self {
            skill_digest,
            installed: true,
            bound: false,
            discoverable: false,
            available_to_attempt: false,
            invoked: false,
            effective: None,
        }
    }

    pub fn mark_bound(&mut self) -> Result<(), SkillEvidenceError> {
        if !self.installed {
            return Err(SkillEvidenceError::PrecedingLevelMissing(
                SkillEvidenceLevel::Installed,
            ));
        }
        self.bound = true;
        Ok(())
    }

    pub fn mark_discoverable(&mut self) -> Result<(), SkillEvidenceError> {
        if !self.bound {
            return Err(SkillEvidenceError::PrecedingLevelMissing(
                SkillEvidenceLevel::Bound,
            ));
        }
        self.discoverable = true;
        Ok(())
    }

    pub fn mark_available_to_attempt(&mut self) -> Result<(), SkillEvidenceError> {
        if !self.discoverable {
            return Err(SkillEvidenceError::PrecedingLevelMissing(
                SkillEvidenceLevel::Discoverable,
            ));
        }
        self.available_to_attempt = true;
        Ok(())
    }

    pub fn mark_invoked(&mut self) -> Result<(), SkillEvidenceError> {
        if !self.available_to_attempt {
            return Err(SkillEvidenceError::PrecedingLevelMissing(
                SkillEvidenceLevel::AvailableToAttempt,
            ));
        }
        self.invoked = true;
        Ok(())
    }

    /// `Effective` judges whether the *task* was actually proven correct
    /// while this skill was invoked — it can only be recorded once Invoked
    /// is true, and it is a plain outcome bit, not a further ladder rung.
    pub fn record_effective(&mut self, effective: bool) -> Result<(), SkillEvidenceError> {
        if !self.invoked {
            return Err(SkillEvidenceError::PrecedingLevelMissing(
                SkillEvidenceLevel::Invoked,
            ));
        }
        self.effective = Some(effective);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationPolicy {
    ExplicitOnly,
    ImplicitAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalSkillBinding {
    pub revision: u32,
    pub skill_digest: SkillDigest,
    pub steps: Vec<LoopStepId>,
    pub cli_targets: Vec<String>,
    pub invocation: InvocationPolicy,
    pub state: BindingState,
}

/// §5.11: "ProjectSkillBinding 只记录 inherit/enable/disable/pin-version 差异."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectSkillBindingMode {
    Inherit,
    Enable,
    Disable,
    PinVersion(SkillDigest),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSkillBinding {
    pub project_id: String,
    pub revision: u32,
    pub mode: ProjectSkillBindingMode,
    pub steps: Option<Vec<LoopStepId>>,
    pub cli_targets: Option<Vec<String>>,
    pub invocation: Option<InvocationPolicy>,
    pub state: Option<BindingState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSkillBinding {
    pub skill_digest: SkillDigest,
    pub steps: Vec<LoopStepId>,
    pub cli_targets: Vec<String>,
    pub invocation: InvocationPolicy,
    pub state: BindingState,
}

/// §5.11: "GlobalSkillBinding + ProjectSkillBinding 是'选择哪些 Skill'的唯一
/// 权威." `project == None` is pure inheritance. `Enable`/`Disable` force
/// the resolved state regardless of the global binding's own state, but
/// still track the global skill_digest; `PinVersion` is the one mode that
/// substitutes a different digest than the global binding's.
pub fn resolve_project_binding(
    global: &GlobalSkillBinding,
    project: Option<&ProjectSkillBinding>,
) -> ResolvedSkillBinding {
    let Some(project) = project else {
        return ResolvedSkillBinding {
            skill_digest: global.skill_digest.clone(),
            steps: global.steps.clone(),
            cli_targets: global.cli_targets.clone(),
            invocation: global.invocation,
            state: global.state,
        };
    };
    let steps = project
        .steps
        .clone()
        .unwrap_or_else(|| global.steps.clone());
    let cli_targets = project
        .cli_targets
        .clone()
        .unwrap_or_else(|| global.cli_targets.clone());
    let invocation = project.invocation.unwrap_or(global.invocation);
    match &project.mode {
        ProjectSkillBindingMode::Inherit => ResolvedSkillBinding {
            skill_digest: global.skill_digest.clone(),
            steps,
            cli_targets,
            invocation,
            state: project.state.unwrap_or(global.state),
        },
        ProjectSkillBindingMode::Enable => ResolvedSkillBinding {
            skill_digest: global.skill_digest.clone(),
            steps,
            cli_targets,
            invocation,
            state: BindingState::Enabled,
        },
        ProjectSkillBindingMode::Disable => ResolvedSkillBinding {
            skill_digest: global.skill_digest.clone(),
            steps,
            cli_targets,
            invocation,
            state: BindingState::Disabled,
        },
        ProjectSkillBindingMode::PinVersion(pinned_digest) => ResolvedSkillBinding {
            skill_digest: pinned_digest.clone(),
            steps,
            cli_targets,
            invocation,
            state: project.state.unwrap_or(global.state),
        },
    }
}

/// §5.11: "物理 GC 只允许无当前 binding、无历史 Run 引用的 digest."
pub fn can_garbage_collect(
    digest: &SkillDigest,
    currently_bound_digests: &HashSet<SkillDigest>,
    historically_referenced_digests: &HashSet<SkillDigest>,
) -> bool {
    !currently_bound_digests.contains(digest) && !historically_referenced_digests.contains(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_requires_a_user_approval_decision() {
        let err = issue_skill_install_receipt(
            "pkg-1",
            SkillAuditOutcome::NoKnownRisksFound,
            "plan-1",
            "  ",
            "receipt-1",
        )
        .unwrap_err();
        assert_eq!(err, SkillInstallError::MissingUserApprovalDecision);
    }

    #[test]
    fn install_with_findings_still_succeeds_when_user_approved() {
        let (_receipt, ladder) = issue_skill_install_receipt(
            "pkg-1",
            SkillAuditOutcome::RisksFound {
                findings: vec!["shells out to curl".into()],
            },
            "plan-1",
            "decision:1",
            "receipt-1",
        )
        .unwrap();
        assert!(ladder.installed);
        assert!(!ladder.bound);
    }

    #[test]
    fn install_never_produces_more_than_installed() {
        let (_receipt, ladder) = issue_skill_install_receipt(
            "pkg-1",
            SkillAuditOutcome::NoKnownRisksFound,
            "plan-1",
            "decision:1",
            "receipt-1",
        )
        .unwrap();
        assert_eq!(
            ladder,
            SkillEvidenceLadder::installed(SkillDigest("pkg-1".into()))
        );
    }

    #[test]
    fn ladder_rejects_skipping_a_level() {
        let mut ladder = SkillEvidenceLadder::installed(SkillDigest("pkg-1".into()));
        let err = ladder.mark_discoverable().unwrap_err();
        assert_eq!(
            err,
            SkillEvidenceError::PrecedingLevelMissing(SkillEvidenceLevel::Bound)
        );
    }

    #[test]
    fn ladder_walks_forward_one_level_at_a_time() {
        let mut ladder = SkillEvidenceLadder::installed(SkillDigest("pkg-1".into()));
        ladder.mark_bound().unwrap();
        ladder.mark_discoverable().unwrap();
        ladder.mark_available_to_attempt().unwrap();
        ladder.mark_invoked().unwrap();
        ladder.record_effective(true).unwrap();
        assert!(ladder.invoked);
        assert_eq!(ladder.effective, Some(true));
    }

    #[test]
    fn effective_cannot_be_recorded_before_invoked() {
        let mut ladder = SkillEvidenceLadder::installed(SkillDigest("pkg-1".into()));
        ladder.mark_bound().unwrap();
        let err = ladder.record_effective(true).unwrap_err();
        assert_eq!(
            err,
            SkillEvidenceError::PrecedingLevelMissing(SkillEvidenceLevel::Invoked)
        );
    }

    fn global(digest: &str, state: BindingState) -> GlobalSkillBinding {
        GlobalSkillBinding {
            revision: 1,
            skill_digest: SkillDigest(digest.into()),
            steps: vec![LoopStepId("implementation".into())],
            cli_targets: vec!["claude-code".into()],
            invocation: InvocationPolicy::ExplicitOnly,
            state,
        }
    }

    #[test]
    fn no_project_binding_is_pure_inheritance() {
        let g = global("digest-1", BindingState::Enabled);
        let resolved = resolve_project_binding(&g, None);
        assert_eq!(resolved.skill_digest, SkillDigest("digest-1".into()));
        assert_eq!(resolved.state, BindingState::Enabled);
    }

    #[test]
    fn inherit_mode_falls_back_to_global_fields_when_unset() {
        let g = global("digest-1", BindingState::Enabled);
        let p = ProjectSkillBinding {
            project_id: "proj-1".into(),
            revision: 1,
            mode: ProjectSkillBindingMode::Inherit,
            steps: None,
            cli_targets: None,
            invocation: None,
            state: None,
        };
        let resolved = resolve_project_binding(&g, Some(&p));
        assert_eq!(resolved.skill_digest, SkillDigest("digest-1".into()));
        assert_eq!(resolved.state, BindingState::Enabled);
    }

    #[test]
    fn disable_mode_forces_disabled_even_if_global_is_enabled() {
        let g = global("digest-1", BindingState::Enabled);
        let p = ProjectSkillBinding {
            project_id: "proj-1".into(),
            revision: 1,
            mode: ProjectSkillBindingMode::Disable,
            steps: None,
            cli_targets: None,
            invocation: None,
            state: None,
        };
        let resolved = resolve_project_binding(&g, Some(&p));
        assert_eq!(resolved.state, BindingState::Disabled);
    }

    #[test]
    fn enable_mode_forces_enabled_even_if_global_is_disabled() {
        let g = global("digest-1", BindingState::Disabled);
        let p = ProjectSkillBinding {
            project_id: "proj-1".into(),
            revision: 1,
            mode: ProjectSkillBindingMode::Enable,
            steps: None,
            cli_targets: None,
            invocation: None,
            state: None,
        };
        let resolved = resolve_project_binding(&g, Some(&p));
        assert_eq!(resolved.state, BindingState::Enabled);
    }

    #[test]
    fn pin_version_mode_substitutes_the_pinned_digest() {
        let g = global("digest-1", BindingState::Enabled);
        let p = ProjectSkillBinding {
            project_id: "proj-1".into(),
            revision: 1,
            mode: ProjectSkillBindingMode::PinVersion(SkillDigest("digest-pinned".into())),
            steps: None,
            cli_targets: None,
            invocation: None,
            state: None,
        };
        let resolved = resolve_project_binding(&g, Some(&p));
        assert_eq!(resolved.skill_digest, SkillDigest("digest-pinned".into()));
    }

    #[test]
    fn garbage_collection_refuses_a_currently_bound_digest() {
        let digest = SkillDigest("digest-1".into());
        let mut bound = HashSet::new();
        bound.insert(digest.clone());
        assert!(!can_garbage_collect(&digest, &bound, &HashSet::new()));
    }

    #[test]
    fn garbage_collection_refuses_a_historically_referenced_digest() {
        let digest = SkillDigest("digest-1".into());
        let mut history = HashSet::new();
        history.insert(digest.clone());
        assert!(!can_garbage_collect(&digest, &HashSet::new(), &history));
    }

    #[test]
    fn garbage_collection_allows_an_unreferenced_digest() {
        let digest = SkillDigest("digest-1".into());
        assert!(can_garbage_collect(
            &digest,
            &HashSet::new(),
            &HashSet::new()
        ));
    }
}
