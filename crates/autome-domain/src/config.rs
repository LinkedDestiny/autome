//! GlobalConfigRevision / ProjectConfigPatch / ResolvedProjectConfig /
//! GlobalConfigImpactPreview per plan §5.1's "全局默认 + 项目稀疏覆盖 + Run
//! 冻结快照" configuration model.
//!
//! Two mechanical rules from the plan text are enforced here:
//!
//! 1. "安全下限在继承层之外，项目不能覆盖" — `ProjectConfigPatch` simply has
//!    no field through which a project could supply its own
//!    `safety_policy_hash`; `resolve_project_config` always takes it
//!    straight from the `GlobalConfigRevision`, so there is no code path
//!    for a project override to reach it at all.
//! 2. "无效 CLI/model/Effort/skills 组合不得静默钳制或 fallback" —
//!    `resolve_project_config` takes an `is_profile_valid` predicate (the
//!    caller supplies it from real qualification/capability facts) and
//!    refuses to resolve at all when an override fails it, rather than
//!    silently falling back to the global default.
//!
//! `GlobalConfigImpactPreview` implements "保存命令绑定 preview hash 与
//! project-set hash；预览过期或项目集合变化即拒绝", mirroring the
//! fingerprint-staleness pattern used elsewhere in this crate
//! (`ReadinessFingerprint`, `EvidenceFingerprint`).
//!
//! ProjectIntentRevision/ProjectIntentAmendment/ProjectInitializationReceipt,
//! PlanningPolicyRestart, RunPolicyAmendment and BudgetGrantReceipt are the
//! remaining unimplemented pieces of §5.1, left for a future increment.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::attempt::LoopStepId;

/// §5.1: "2.0.0 的可配置 AI 步骤固定为" these nine — Rust verifier, receipt
/// signing, completion gate and Git/artifact delivery are not AI steps and
/// take no CLI/model/Effort configuration at all.
pub const CONFIGURABLE_AI_STEPS: &[&str] = &[
    "fact_analysis",
    "contract_drafting",
    "contract_review",
    "task_graph_planning",
    "graph_review",
    "implementation",
    "repair",
    "node_evaluation",
    "final_audit",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentExecutionProfile {
    pub adapter_id: String,
    pub installation_id: String,
    pub model_id: String,
    pub effort_id: String,
    /// Only constrains capability — never selects a Skill (§5.1: "只约束
    /// 能力，不选择 Skill").
    pub skill_policy_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepOverride {
    Inherit,
    Replace(AgentExecutionProfile),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HumanReviewSetting {
    Off,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HumanReviewOverride {
    Inherit,
    Off,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfigRevision {
    pub revision: u32,
    pub step_defaults: HashMap<LoopStepId, AgentExecutionProfile>,
    pub human_review_default: HashMap<LoopStepId, HumanReviewSetting>,
    pub environment_defaults_ref: String,
    pub budget_defaults_ref: String,
    pub skill_policy_default_ref: String,
    pub safety_policy_hash: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfigPatch {
    pub project_id: String,
    pub revision: u32,
    pub step_overrides: HashMap<LoopStepId, StepOverride>,
    pub human_review_overrides: HashMap<LoopStepId, HumanReviewOverride>,
    pub environment_override_ref: Option<String>,
    pub budget_override_ref: Option<String>,
    pub skill_policy_override_ref: Option<String>,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigProvenance {
    Global,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedStepConfig {
    pub step_id: LoopStepId,
    pub profile: AgentExecutionProfile,
    pub human_review: HumanReviewSetting,
    /// Reflects the *profile*'s override provenance; `human_review` may
    /// still independently be inherited even when `provenance` is
    /// `Project` (a project can override one without the other).
    pub provenance: ConfigProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedProjectConfig {
    pub global_revision: u32,
    pub project_patch_revision: u32,
    pub steps: Vec<ResolvedStepConfig>,
    pub environment_ref: String,
    pub budget_ref: String,
    pub skill_policy_ref: String,
    pub skill_binding_revision_ref: String,
    pub safety_policy_hash: String,
    pub snapshot_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveProjectConfigError {
    MissingGlobalDefault { step: LoopStepId },
    InvalidStepOverride { step: LoopStepId },
}

/// Sole way to compute a `ResolvedProjectConfig`. `snapshot_hash` is
/// supplied by the caller (it is a content hash of everything else here,
/// and this crate does not do hashing) rather than computed inside.
pub fn resolve_project_config(
    global: &GlobalConfigRevision,
    patch: Option<&ProjectConfigPatch>,
    is_profile_valid: impl Fn(&AgentExecutionProfile) -> bool,
    skill_binding_revision_ref: &str,
    snapshot_hash: &str,
) -> Result<ResolvedProjectConfig, Vec<ResolveProjectConfigError>> {
    let mut errors = Vec::new();
    let mut steps = Vec::new();

    for step_key in CONFIGURABLE_AI_STEPS {
        let step_id = LoopStepId((*step_key).to_string());

        let global_profile = match global.step_defaults.get(&step_id) {
            Some(profile) => profile,
            None => {
                errors.push(ResolveProjectConfigError::MissingGlobalDefault {
                    step: step_id.clone(),
                });
                continue;
            }
        };
        let global_review = global
            .human_review_default
            .get(&step_id)
            .copied()
            .unwrap_or(HumanReviewSetting::Off);

        let step_override = patch.and_then(|p| p.step_overrides.get(&step_id));
        let (profile, provenance) = match step_override {
            None | Some(StepOverride::Inherit) => {
                (global_profile.clone(), ConfigProvenance::Global)
            }
            Some(StepOverride::Replace(profile)) => {
                if !is_profile_valid(profile) {
                    errors.push(ResolveProjectConfigError::InvalidStepOverride {
                        step: step_id.clone(),
                    });
                    continue;
                }
                (profile.clone(), ConfigProvenance::Project)
            }
        };

        let human_review = match patch.and_then(|p| p.human_review_overrides.get(&step_id)) {
            None | Some(HumanReviewOverride::Inherit) => global_review,
            Some(HumanReviewOverride::Off) => HumanReviewSetting::Off,
            Some(HumanReviewOverride::Required) => HumanReviewSetting::Required,
        };

        steps.push(ResolvedStepConfig {
            step_id,
            profile,
            human_review,
            provenance,
        });
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    let environment_ref = patch
        .and_then(|p| p.environment_override_ref.clone())
        .unwrap_or_else(|| global.environment_defaults_ref.clone());
    let budget_ref = patch
        .and_then(|p| p.budget_override_ref.clone())
        .unwrap_or_else(|| global.budget_defaults_ref.clone());
    let skill_policy_ref = patch
        .and_then(|p| p.skill_policy_override_ref.clone())
        .unwrap_or_else(|| global.skill_policy_default_ref.clone());

    Ok(ResolvedProjectConfig {
        global_revision: global.revision,
        project_patch_revision: patch.map(|p| p.revision).unwrap_or(0),
        steps,
        environment_ref,
        budget_ref,
        skill_policy_ref,
        skill_binding_revision_ref: skill_binding_revision_ref.to_string(),
        safety_policy_hash: global.safety_policy_hash.clone(),
        snapshot_hash: snapshot_hash.to_string(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffectedProjectImpact {
    pub project_id: String,
    pub affected_step_ids: Vec<LoopStepId>,
    pub before_route_hash: String,
    pub after_route_hash: String,
    pub before_human_review_hash: String,
    pub after_human_review_hash: String,
    pub before_policy_hash: String,
    pub after_policy_hash: String,
    /// Installation/provider/account fingerprint or auth mode/Skill
    /// policy/readiness changes this project would see, each as an
    /// opaque description string (the concrete diffing lives with the
    /// caller, which has the real capability/readiness facts).
    pub capability_changes: Vec<String>,
    pub projected_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfigImpactPreview {
    pub base_global_revision: u32,
    pub proposed_config_hash: String,
    pub observed_project_set_hash: String,
    pub affected_projects: Vec<AffectedProjectImpact>,
    pub blocking_project_ids: Vec<String>,
    pub requires_second_confirmation: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub expires_at: OffsetDateTime,
    pub preview_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveGlobalConfigError {
    PreviewHashMismatch,
    ProjectSetChanged,
    PreviewExpired,
    SecondConfirmationRequired,
}

/// §5.1: "保存命令绑定 preview hash 与 project-set hash；预览过期或项目集合
/// 变化即拒绝" plus "存在任何 projected blocked、账号/provider 切换或
/// Global Skill 影响时需要第二次明确确认". Returns every violation found
/// rather than stopping at the first, matching this crate's other
/// multi-error validation gates.
pub fn validate_save_global_config_revision(
    preview: &GlobalConfigImpactPreview,
    submitted_preview_hash: &str,
    current_project_set_hash: &str,
    now: OffsetDateTime,
    second_confirmation_acquired: bool,
) -> Vec<SaveGlobalConfigError> {
    let mut errors = Vec::new();
    if preview.preview_hash != submitted_preview_hash {
        errors.push(SaveGlobalConfigError::PreviewHashMismatch);
    }
    if preview.observed_project_set_hash != current_project_set_hash {
        errors.push(SaveGlobalConfigError::ProjectSetChanged);
    }
    if now > preview.expires_at {
        errors.push(SaveGlobalConfigError::PreviewExpired);
    }
    if preview.requires_second_confirmation && !second_confirmation_acquired {
        errors.push(SaveGlobalConfigError::SecondConfirmationRequired);
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(model_id: &str) -> AgentExecutionProfile {
        AgentExecutionProfile {
            adapter_id: "codex".into(),
            installation_id: "install-1".into(),
            model_id: model_id.into(),
            effort_id: "medium".into(),
            skill_policy_ref: "skill-policy-1".into(),
        }
    }

    fn global_config() -> GlobalConfigRevision {
        let mut step_defaults = HashMap::new();
        let mut human_review_default = HashMap::new();
        for step in CONFIGURABLE_AI_STEPS {
            step_defaults.insert(LoopStepId((*step).to_string()), profile("global-model"));
            human_review_default.insert(LoopStepId((*step).to_string()), HumanReviewSetting::Off);
        }
        GlobalConfigRevision {
            revision: 1,
            step_defaults,
            human_review_default,
            environment_defaults_ref: "env-default".into(),
            budget_defaults_ref: "budget-default".into(),
            skill_policy_default_ref: "skill-policy-default".into(),
            safety_policy_hash: "safety-floor-1".into(),
            content_hash: "global-content-1".into(),
        }
    }

    fn always_valid(_: &AgentExecutionProfile) -> bool {
        true
    }

    #[test]
    fn no_patch_resolves_every_step_from_global() {
        let resolved =
            resolve_project_config(&global_config(), None, always_valid, "binding-1", "snap-1")
                .unwrap();
        assert_eq!(resolved.steps.len(), CONFIGURABLE_AI_STEPS.len());
        assert!(
            resolved
                .steps
                .iter()
                .all(|s| s.provenance == ConfigProvenance::Global)
        );
        assert_eq!(resolved.environment_ref, "env-default");
        assert_eq!(resolved.safety_policy_hash, "safety-floor-1");
    }

    #[test]
    fn project_override_replaces_one_step_profile() {
        let mut step_overrides = HashMap::new();
        step_overrides.insert(
            LoopStepId("contract_review".into()),
            StepOverride::Replace(profile("project-model")),
        );
        let patch = ProjectConfigPatch {
            project_id: "project-1".into(),
            revision: 1,
            step_overrides,
            human_review_overrides: HashMap::new(),
            environment_override_ref: None,
            budget_override_ref: None,
            skill_policy_override_ref: None,
            content_hash: "patch-content-1".into(),
        };
        let resolved = resolve_project_config(
            &global_config(),
            Some(&patch),
            always_valid,
            "binding-1",
            "snap-1",
        )
        .unwrap();
        let contract_review = resolved
            .steps
            .iter()
            .find(|s| s.step_id == LoopStepId("contract_review".into()))
            .unwrap();
        assert_eq!(contract_review.profile.model_id, "project-model");
        assert_eq!(contract_review.provenance, ConfigProvenance::Project);
        let other = resolved
            .steps
            .iter()
            .find(|s| s.step_id == LoopStepId("final_audit".into()))
            .unwrap();
        assert_eq!(other.profile.model_id, "global-model");
        assert_eq!(other.provenance, ConfigProvenance::Global);
    }

    #[test]
    fn invalid_override_profile_is_rejected_without_falling_back() {
        let mut step_overrides = HashMap::new();
        step_overrides.insert(
            LoopStepId("implementation".into()),
            StepOverride::Replace(profile("unqualified-model")),
        );
        let patch = ProjectConfigPatch {
            project_id: "project-1".into(),
            revision: 1,
            step_overrides,
            human_review_overrides: HashMap::new(),
            environment_override_ref: None,
            budget_override_ref: None,
            skill_policy_override_ref: None,
            content_hash: "patch-content-1".into(),
        };
        let result = resolve_project_config(
            &global_config(),
            Some(&patch),
            |p: &AgentExecutionProfile| p.model_id != "unqualified-model",
            "binding-1",
            "snap-1",
        );
        assert_eq!(
            result.unwrap_err(),
            vec![ResolveProjectConfigError::InvalidStepOverride {
                step: LoopStepId("implementation".into())
            }]
        );
    }

    #[test]
    fn human_review_override_is_independent_of_profile_override() {
        let mut human_review_overrides = HashMap::new();
        human_review_overrides.insert(
            LoopStepId("final_audit".into()),
            HumanReviewOverride::Required,
        );
        let patch = ProjectConfigPatch {
            project_id: "project-1".into(),
            revision: 1,
            step_overrides: HashMap::new(),
            human_review_overrides,
            environment_override_ref: None,
            budget_override_ref: None,
            skill_policy_override_ref: None,
            content_hash: "patch-content-1".into(),
        };
        let resolved = resolve_project_config(
            &global_config(),
            Some(&patch),
            always_valid,
            "binding-1",
            "snap-1",
        )
        .unwrap();
        let final_audit = resolved
            .steps
            .iter()
            .find(|s| s.step_id == LoopStepId("final_audit".into()))
            .unwrap();
        assert_eq!(final_audit.human_review, HumanReviewSetting::Required);
        // Profile itself stayed inherited even though human_review didn't.
        assert_eq!(final_audit.provenance, ConfigProvenance::Global);
    }

    fn preview(
        expires_at: OffsetDateTime,
        requires_second_confirmation: bool,
    ) -> GlobalConfigImpactPreview {
        GlobalConfigImpactPreview {
            base_global_revision: 1,
            proposed_config_hash: "proposed-1".into(),
            observed_project_set_hash: "project-set-1".into(),
            affected_projects: vec![],
            blocking_project_ids: vec![],
            requires_second_confirmation,
            expires_at,
            preview_hash: "preview-1".into(),
        }
    }

    fn t(seconds_from_epoch: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds_from_epoch).unwrap()
    }

    #[test]
    fn save_is_accepted_when_everything_matches_and_no_confirmation_needed() {
        let errors = validate_save_global_config_revision(
            &preview(t(1000), false),
            "preview-1",
            "project-set-1",
            t(500),
            false,
        );
        assert!(errors.is_empty());
    }

    #[test]
    fn save_rejects_preview_hash_mismatch() {
        let errors = validate_save_global_config_revision(
            &preview(t(1000), false),
            "wrong-hash",
            "project-set-1",
            t(500),
            false,
        );
        assert_eq!(errors, vec![SaveGlobalConfigError::PreviewHashMismatch]);
    }

    #[test]
    fn save_rejects_changed_project_set() {
        let errors = validate_save_global_config_revision(
            &preview(t(1000), false),
            "preview-1",
            "different-project-set",
            t(500),
            false,
        );
        assert_eq!(errors, vec![SaveGlobalConfigError::ProjectSetChanged]);
    }

    #[test]
    fn save_rejects_expired_preview() {
        let errors = validate_save_global_config_revision(
            &preview(t(1000), false),
            "preview-1",
            "project-set-1",
            t(1500),
            false,
        );
        assert_eq!(errors, vec![SaveGlobalConfigError::PreviewExpired]);
    }

    #[test]
    fn save_requires_second_confirmation_when_preview_demands_it() {
        let errors = validate_save_global_config_revision(
            &preview(t(1000), true),
            "preview-1",
            "project-set-1",
            t(500),
            false,
        );
        assert_eq!(
            errors,
            vec![SaveGlobalConfigError::SecondConfirmationRequired]
        );
    }

    #[test]
    fn save_succeeds_once_second_confirmation_is_acquired() {
        let errors = validate_save_global_config_revision(
            &preview(t(1000), true),
            "preview-1",
            "project-set-1",
            t(500),
            true,
        );
        assert!(errors.is_empty());
    }
}
