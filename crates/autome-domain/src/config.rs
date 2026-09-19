//! Loop configuration: global defaults, sparse project overrides, and the
//! resolved view a session is launched from. Technical design §9, §3.3;
//! requirements C-04 through C-09.
//!
//! Two rules shape everything here:
//!
//! 1. **Sparse overrides.** A project file stores only the fields it
//!    overrides. "Restore default" is implemented as *removing* a field, not
//!    as copying the global value in — otherwise a later change to the global
//!    default would silently stop propagating.
//! 2. **Validation is a pure function of the resolved view plus the skill
//!    inventory.** The SAME-MODEL discipline (C-06) and the skill-visibility
//!    rule (S-04) are checked here, in the domain, so that the IPC layer, the
//!    launcher and the test suite all reach the same verdict.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::role::{Role, Runtime};

/// Inclusive bounds on a project's parallel task limit (requirement P-04).
pub const PARALLEL_MIN: u32 = 1;
pub const PARALLEL_MAX: u32 = 5;

/// Shipped defaults, used when `~/.autome/config.toml` does not exist yet.
pub const DEFAULT_PARALLEL: u32 = 3;
pub const DEFAULT_DESIGN_ROUNDS: u32 = 15;
pub const DEFAULT_BUDGET_FACTOR: u32 = 5;

/// Loop-wide numeric settings. Every field is overridable per project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopDefaults {
    pub parallel: u32,
    pub design_rounds: u32,
    pub budget_factor: u32,
}

impl Default for LoopDefaults {
    fn default() -> Self {
        Self {
            parallel: DEFAULT_PARALLEL,
            design_rounds: DEFAULT_DESIGN_ROUNDS,
            budget_factor: DEFAULT_BUDGET_FACTOR,
        }
    }
}

/// The sparse counterpart of `LoopDefaults`: `None` means "inherit".
///
/// `protocol` has no global counterpart on purpose. There is no sensible
/// machine-wide "everyone runs v5"; the useful default is "whatever the
/// protocol repository's newest tag is", and that is what absence means. A
/// project pins only when it has a reason to stay behind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_rounds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_factor: Option<u32>,
    /// A `protocol/vN` tag this project stays on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

/// One role's fully-resolved execution profile (requirement C-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleConfig {
    pub enabled: bool,
    pub runtime: Runtime,
    pub model: String,
    /// `None` means "use whatever the CLI defaults to" — deliberately not
    /// normalised to a string, so that "unset" and "explicitly set to the
    /// value that happens to be the default" stay distinguishable.
    pub effort: Option<String>,
    /// Skills this role *must* use (requirement S-03). Order is the user's;
    /// duplicates are removed on save.
    pub skills: Vec<String>,
}

impl RoleConfig {
    /// The identity SAME-MODEL compares (design §9: "比较 `runtime:model` 字面值").
    pub fn model_identity(&self) -> String {
        format!("{}:{}", self.runtime, self.model)
    }
}

/// The sparse counterpart of `RoleConfig`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<Runtime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Two levels of optionality, and they mean different things:
    /// `None` = inherit the global effort; `Some(None)` = override it to
    /// "CLI default"; `Some(Some(e))` = override it to `e`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
}

impl RoleOverrides {
    pub fn is_empty(&self) -> bool {
        self.enabled.is_none()
            && self.runtime.is_none()
            && self.model.is_none()
            && self.effort.is_none()
            && self.skills.is_none()
    }
}

/// Which side of the overlay a resolved field came from. Surfaced so the
/// project view can offer "restore default" only where there is something to
/// restore (requirement C-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Global,
    Project,
}

/// `~/.autome/config.toml` — always complete, never sparse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub loop_defaults: LoopDefaults,
    pub roles: BTreeMap<Role, RoleConfig>,
    /// Presentation, not Loop behaviour — but it lives here for the same
    /// reason everything else does: the core owns state, and a preference the
    /// renderer kept to itself would be one more place state can disagree
    /// with the core about what is true.
    #[serde(default)]
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default)]
    pub theme: Theme,
}

/// Which appearance the window uses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Follow macOS. The default, and what an app on this platform is expected
    /// to do unless the user says otherwise.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];

    pub const fn as_str(self) -> &'static str {
        match self {
            Theme::System => "system",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    pub fn parse(raw: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.as_str() == raw)
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Theme::System => "跟随系统",
            Theme::Light => "浅色",
            Theme::Dark => "深色",
        }
    }
}

impl Default for GlobalConfig {
    /// The first-run defaults: Claude generates, Codex evaluates.
    ///
    /// Every model is left empty, meaning "whatever that CLI defaults to".
    /// Naming specific models here would be guessing: the set a given account
    /// can actually use is not knowable from this side, model names change
    /// faster than releases, and a default that names an unavailable model
    /// fails at the first session with an error about billing rather than
    /// about configuration. The user picks models in the routing graph, where
    /// the list is theirs.
    ///
    /// SAME-MODEL still holds on a fresh install, because the three evaluating
    /// roles sit on the other runtime: `codex:` and `claude:` differ even when
    /// both models are unset.
    fn default() -> Self {
        let role = |runtime: Runtime, effort: Option<&str>| RoleConfig {
            enabled: true,
            runtime,
            model: String::new(),
            effort: effort.map(str::to_string),
            skills: Vec::new(),
        };
        let mut roles = BTreeMap::new();
        roles.insert(Role::Plan, role(Runtime::Claude, Some("high")));
        roles.insert(Role::Review, role(Runtime::Codex, Some("high")));
        roles.insert(Role::Adjudicate, role(Runtime::Claude, None));
        roles.insert(Role::Impl, role(Runtime::Claude, Some("high")));
        roles.insert(Role::Audit, role(Runtime::Codex, Some("high")));
        // The retro round evaluates what the implementation rounds produced,
        // so it sits on the other runtime for the same reason the audit does.
        roles.insert(Role::Retro, role(Runtime::Codex, Some("high")));
        Self {
            loop_defaults: LoopDefaults::default(),
            roles,
            ui: UiConfig::default(),
        }
    }
}

impl GlobalConfig {
    /// Every role is always present in a global config; a file missing one is
    /// repaired against `Default` at load time, so this cannot panic.
    pub fn role(&self, role: Role) -> &RoleConfig {
        self.roles
            .get(&role)
            .expect("global config always carries every role")
    }

    /// Fills in any role absent from a hand-edited file — including one this
    /// binary added and the file predates — so the rest of the system can rely
    /// on `role()` being total.
    pub fn repair(&mut self) {
        let defaults = GlobalConfig::default();
        for role in Role::ALL {
            self.roles
                .entry(role)
                .or_insert_with(|| defaults.role(role).clone());
        }
    }
}

/// `<repo>/.autome/config.toml` — sparse by construction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default, rename = "loop")]
    pub loop_overrides: LoopOverrides,
    #[serde(default)]
    pub roles: BTreeMap<Role, RoleOverrides>,
}

impl ProjectConfig {
    pub fn overrides_for(&self, role: Role) -> RoleOverrides {
        self.roles.get(&role).cloned().unwrap_or_default()
    }

    /// Drops role entries that no longer override anything, so a file never
    /// accumulates empty `[roles.x]` tables after repeated "restore default".
    pub fn prune(&mut self) {
        self.roles.retain(|_, o| !o.is_empty());
    }
}

/// One role as a session will actually run it, plus where each part came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRole {
    pub role: Role,
    pub config: RoleConfig,
    /// `Project` when *any* field of this role is overridden — the UI shows a
    /// single "restore default" per role, not per field (requirement C-07).
    pub provenance: Provenance,
}

/// The complete view a session is launched from (design §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedConfig {
    pub loop_defaults: LoopDefaults,
    pub roles: Vec<ResolvedRole>,
    /// The project's protocol pin, if it has one. `None` means "follow the
    /// protocol repository's newest tag", which is what most projects do.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_pin: Option<String>,
}

impl ResolvedConfig {
    pub fn role(&self, role: Role) -> &ResolvedRole {
        self.roles
            .iter()
            .find(|r| r.role == role)
            .expect("resolved config always carries every role")
    }

    pub fn is_enabled(&self, role: Role) -> bool {
        self.role(role).config.enabled
    }
}

/// Overlays a sparse project config onto the global defaults. Total: the
/// result always carries every role, in `Role::ALL` order.
pub fn resolve(global: &GlobalConfig, project: &ProjectConfig) -> ResolvedConfig {
    let parallel = project
        .loop_overrides
        .parallel
        .unwrap_or(global.loop_defaults.parallel);
    let design_rounds = project
        .loop_overrides
        .design_rounds
        .unwrap_or(global.loop_defaults.design_rounds);
    let budget_factor = project
        .loop_overrides
        .budget_factor
        .unwrap_or(global.loop_defaults.budget_factor);

    let roles = Role::ALL
        .into_iter()
        .map(|role| {
            let base = global.role(role);
            let over = project.overrides_for(role);
            let overridden = !over.is_empty();
            ResolvedRole {
                role,
                config: RoleConfig {
                    enabled: over.enabled.unwrap_or(base.enabled),
                    runtime: over.runtime.unwrap_or(base.runtime),
                    model: over.model.unwrap_or_else(|| base.model.clone()),
                    effort: over.effort.unwrap_or_else(|| base.effort.clone()),
                    skills: over.skills.unwrap_or_else(|| base.skills.clone()),
                },
                provenance: if overridden {
                    Provenance::Project
                } else {
                    Provenance::Global
                },
            }
        })
        .collect();

    ResolvedConfig {
        loop_defaults: LoopDefaults {
            parallel,
            design_rounds,
            budget_factor,
        },
        roles,
        protocol_pin: project.loop_overrides.protocol.clone(),
    }
}

/// A reason a configuration cannot be saved. Each variant carries enough to
/// render the UI's red highlight without the caller re-deriving anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigViolation {
    /// C-06. The evaluating role shares an identity with its generating side.
    SameModel {
        evaluator: Role,
        generator: Role,
        identity: String,
    },
    /// S-04. A bound skill is invisible to the runtime this role uses.
    SkillNotVisible {
        role: Role,
        skill: String,
        runtime: Runtime,
    },
    /// S-04's other half: the binding names a skill that is not installed at
    /// all. Distinct from `SkillNotVisible`, because the fix is different —
    /// install it, versus move it or change the runtime.
    SkillNotFound {
        role: Role,
        skill: String,
    },
    /// P-04.
    ParallelOutOfRange {
        value: u32,
        min: u32,
        max: u32,
    },
    DesignRoundsOutOfRange {
        value: u32,
    },
    BudgetFactorOutOfRange {
        value: u32,
    },
    /// Two roles in a SAME-MODEL pair both left on their CLI's default, on the
    /// same runtime. Distinct from `SameModel` only in the message: there is no
    /// model name to show, and the fix is to name one on either side.
    BothDefaultModels {
        evaluator: Role,
        generator: Role,
    },
}

impl ConfigViolation {
    /// Which role the UI should mark. `None` for loop-wide numeric problems.
    pub fn role(&self) -> Option<Role> {
        match self {
            ConfigViolation::SameModel { evaluator, .. } => Some(*evaluator),
            ConfigViolation::SkillNotVisible { role, .. }
            | ConfigViolation::SkillNotFound { role, .. } => Some(*role),
            ConfigViolation::BothDefaultModels { evaluator, .. } => Some(*evaluator),
            _ => None,
        }
    }
}

/// What `validate` needs to know about the skill inventory. Kept as a trait so
/// the domain stays I/O-free: `automed` implements it over a real filesystem
/// scan, tests implement it over a literal map.
pub trait SkillVisibility {
    /// `None` when no skill by that name exists anywhere.
    fn visible_to(&self, skill: &str) -> Option<&[Runtime]>;
}

/// An inventory that knows about no skills at all. Used when validating a
/// configuration that binds none, and by tests that do not exercise S-04.
pub struct NoSkills;

impl SkillVisibility for NoSkills {
    fn visible_to(&self, _skill: &str) -> Option<&[Runtime]> {
        None
    }
}

/// Full validation of a resolved configuration. Returns every violation, not
/// just the first — the UI highlights all offending roles at once.
///
/// A *disabled* role is still validated for skill bindings and model shape,
/// but is exempt from SAME-MODEL: a disabled evaluator never runs, so it
/// cannot share a blind spot with anything (requirement C-05 + C-06).
pub fn validate(resolved: &ResolvedConfig, skills: &impl SkillVisibility) -> Vec<ConfigViolation> {
    let mut violations = Vec::new();

    let loop_cfg = resolved.loop_defaults;
    if !(PARALLEL_MIN..=PARALLEL_MAX).contains(&loop_cfg.parallel) {
        violations.push(ConfigViolation::ParallelOutOfRange {
            value: loop_cfg.parallel,
            min: PARALLEL_MIN,
            max: PARALLEL_MAX,
        });
    }
    if loop_cfg.design_rounds == 0 {
        violations.push(ConfigViolation::DesignRoundsOutOfRange {
            value: loop_cfg.design_rounds,
        });
    }
    if loop_cfg.budget_factor == 0 {
        violations.push(ConfigViolation::BudgetFactorOutOfRange {
            value: loop_cfg.budget_factor,
        });
    }

    for resolved_role in &resolved.roles {
        let role = resolved_role.role;
        let cfg = &resolved_role.config;

        for skill in &cfg.skills {
            match skills.visible_to(skill) {
                None => violations.push(ConfigViolation::SkillNotFound {
                    role,
                    skill: skill.clone(),
                }),
                Some(runtimes) if !runtimes.contains(&cfg.runtime) => {
                    violations.push(ConfigViolation::SkillNotVisible {
                        role,
                        skill: skill.clone(),
                        runtime: cfg.runtime,
                    })
                }
                Some(_) => {}
            }
        }

        if let Some(generator) = role.same_model_counterpart() {
            let generator_cfg = &resolved.role(generator).config;
            if cfg.enabled
                && generator_cfg.enabled
                && cfg.model_identity() == generator_cfg.model_identity()
            {
                // Same runtime and both on the CLI default reads differently
                // to the user: there is no model name to point at, and the fix
                // is to name one rather than to change one.
                if cfg.model.trim().is_empty() {
                    violations.push(ConfigViolation::BothDefaultModels {
                        evaluator: role,
                        generator,
                    });
                } else {
                    violations.push(ConfigViolation::SameModel {
                        evaluator: role,
                        generator,
                        identity: cfg.model_identity(),
                    });
                }
            }
        }
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapSkills(HashMap<String, Vec<Runtime>>);

    impl MapSkills {
        fn new(pairs: &[(&str, &[Runtime])]) -> Self {
            Self(
                pairs
                    .iter()
                    .map(|(n, r)| ((*n).to_string(), r.to_vec()))
                    .collect(),
            )
        }
    }

    impl SkillVisibility for MapSkills {
        fn visible_to(&self, skill: &str) -> Option<&[Runtime]> {
            self.0.get(skill).map(Vec::as_slice)
        }
    }

    fn resolved_default() -> ResolvedConfig {
        resolve(&GlobalConfig::default(), &ProjectConfig::default())
    }

    /// A global config with every model named, for the overlay tests. The
    /// shipped defaults deliberately leave models empty (each CLI's own
    /// default), which is right for a first run but makes a poor fixture for
    /// testing inheritance — an empty string cannot be distinguished from a
    /// value that failed to propagate.
    fn named_global() -> GlobalConfig {
        let mut g = GlobalConfig::default();
        g.roles.get_mut(&Role::Plan).unwrap().model = "opus".into();
        g.roles.get_mut(&Role::Review).unwrap().model = "gpt-5.6-sol".into();
        g.roles.get_mut(&Role::Adjudicate).unwrap().model = "opus".into();
        g.roles.get_mut(&Role::Impl).unwrap().model = "opus".into();
        g.roles.get_mut(&Role::Audit).unwrap().model = "gpt-5.6-sol".into();
        g
    }

    #[test]
    fn shipped_defaults_resolve_cleanly() {
        let resolved = resolved_default();
        assert!(validate(&resolved, &NoSkills).is_empty());
        assert_eq!(resolved.loop_defaults.parallel, DEFAULT_PARALLEL);
        assert_eq!(resolved.roles.len(), Role::ALL.len());
    }

    #[test]
    fn resolved_roles_are_in_loop_order() {
        let resolved = resolved_default();
        let order: Vec<Role> = resolved.roles.iter().map(|r| r.role).collect();
        assert_eq!(order, Role::ALL.to_vec());
    }

    #[test]
    fn empty_project_config_inherits_everything() {
        let resolved = resolved_default();
        for role in &resolved.roles {
            assert_eq!(role.provenance, Provenance::Global);
        }
    }

    #[test]
    fn project_override_wins_and_is_marked_as_project_provenance() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Impl,
            RoleOverrides {
                model: Some("claude-sonnet-5".into()),
                ..Default::default()
            },
        );
        let resolved = resolve(&named_global(), &project);
        assert_eq!(resolved.role(Role::Impl).config.model, "claude-sonnet-5");
        assert_eq!(resolved.role(Role::Impl).provenance, Provenance::Project);
        // Untouched roles keep inheriting.
        assert_eq!(resolved.role(Role::Plan).provenance, Provenance::Global);
    }

    #[test]
    fn overriding_only_effort_still_inherits_runtime_and_model() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Plan,
            RoleOverrides {
                effort: Some(Some("xhigh".into())),
                ..Default::default()
            },
        );
        let resolved = resolve(&named_global(), &project);
        let plan = &resolved.role(Role::Plan).config;
        assert_eq!(plan.effort.as_deref(), Some("xhigh"));
        assert_eq!(plan.runtime, Runtime::Claude);
        assert_eq!(plan.model, "opus");
    }

    #[test]
    fn effort_can_be_overridden_back_to_cli_default() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Plan,
            RoleOverrides {
                effort: Some(None),
                ..Default::default()
            },
        );
        let resolved = resolve(&GlobalConfig::default(), &project);
        assert_eq!(resolved.role(Role::Plan).config.effort, None);
        // And that is distinguishable from not overriding at all.
        let inherited = resolve(&GlobalConfig::default(), &ProjectConfig::default());
        assert_eq!(
            inherited.role(Role::Plan).config.effort.as_deref(),
            Some("high")
        );
    }

    #[test]
    fn removing_the_override_restores_the_global_value() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                model: Some("gpt-5.9".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            resolve(&named_global(), &project)
                .role(Role::Audit)
                .config
                .model,
            "gpt-5.9"
        );
        project.roles.remove(&Role::Audit);
        let resolved = resolve(&named_global(), &project);
        assert_eq!(resolved.role(Role::Audit).config.model, "gpt-5.6-sol");
        assert_eq!(resolved.role(Role::Audit).provenance, Provenance::Global);
    }

    #[test]
    fn a_later_global_change_propagates_to_non_overridden_fields() {
        let mut global = named_global();
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Impl,
            RoleOverrides {
                effort: Some(Some("low".into())),
                ..Default::default()
            },
        );
        global.roles.get_mut(&Role::Impl).unwrap().model = "opus-next".into();
        let resolved = resolve(&global, &project);
        // Model follows the new global; effort stays overridden.
        assert_eq!(resolved.role(Role::Impl).config.model, "opus-next");
        assert_eq!(
            resolved.role(Role::Impl).config.effort.as_deref(),
            Some("low")
        );
    }

    #[test]
    fn same_model_is_rejected_for_audit_and_impl() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                runtime: Some(Runtime::Claude),
                model: Some("opus".into()),
                ..Default::default()
            },
        );
        let resolved = resolve(&named_global(), &project);
        let violations = validate(&resolved, &NoSkills);
        assert_eq!(
            violations,
            vec![ConfigViolation::SameModel {
                evaluator: Role::Audit,
                generator: Role::Impl,
                identity: "claude:opus".into(),
            }]
        );
    }

    #[test]
    fn same_model_is_rejected_for_review_and_plan() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Review,
            RoleOverrides {
                runtime: Some(Runtime::Claude),
                model: Some("opus".into()),
                ..Default::default()
            },
        );
        let resolved = resolve(&named_global(), &project);
        let violations = validate(&resolved, &NoSkills);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].role(), Some(Role::Review));
    }

    #[test]
    fn same_runtime_with_a_different_model_is_allowed() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                runtime: Some(Runtime::Claude),
                model: Some("sonnet".into()),
                ..Default::default()
            },
        );
        let resolved = resolve(&named_global(), &project);
        assert!(validate(&resolved, &NoSkills).is_empty());
    }

    #[test]
    fn adjudicate_may_share_a_model_with_anything() {
        // adjudicate == plan == impl, all Claude opus-5 by default; only the
        // two evaluator pairs are constrained.
        let resolved = resolve(&named_global(), &ProjectConfig::default());
        assert_eq!(
            resolved.role(Role::Adjudicate).config.model_identity(),
            resolved.role(Role::Plan).config.model_identity()
        );
        assert!(validate(&resolved, &NoSkills).is_empty());
    }

    #[test]
    fn a_disabled_evaluator_is_exempt_from_same_model() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                enabled: Some(false),
                runtime: Some(Runtime::Claude),
                model: Some("opus".into()),
                ..Default::default()
            },
        );
        let resolved = resolve(&named_global(), &project);
        assert!(validate(&resolved, &NoSkills).is_empty());
    }

    #[test]
    fn a_skill_invisible_to_the_roles_runtime_is_rejected() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Review, // Codex by default
            RoleOverrides {
                skills: Some(vec!["repo-facts".into()]),
                ..Default::default()
            },
        );
        let resolved = resolve(&GlobalConfig::default(), &project);
        let skills = MapSkills::new(&[("repo-facts", &[Runtime::Claude])]);
        assert_eq!(
            validate(&resolved, &skills),
            vec![ConfigViolation::SkillNotVisible {
                role: Role::Review,
                skill: "repo-facts".into(),
                runtime: Runtime::Codex,
            }]
        );
    }

    #[test]
    fn a_skill_visible_to_both_runtimes_is_accepted_anywhere() {
        let mut project = ProjectConfig::default();
        for role in Role::ALL {
            project.roles.insert(
                role,
                RoleOverrides {
                    skills: Some(vec!["conventions".into()]),
                    ..Default::default()
                },
            );
        }
        let resolved = resolve(&GlobalConfig::default(), &project);
        let skills = MapSkills::new(&[("conventions", &[Runtime::Claude, Runtime::Codex])]);
        assert!(validate(&resolved, &skills).is_empty());
    }

    #[test]
    fn an_unknown_skill_is_reported_as_not_found_not_as_invisible() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Impl,
            RoleOverrides {
                skills: Some(vec!["ghost".into()]),
                ..Default::default()
            },
        );
        let resolved = resolve(&GlobalConfig::default(), &project);
        assert_eq!(
            validate(&resolved, &NoSkills),
            vec![ConfigViolation::SkillNotFound {
                role: Role::Impl,
                skill: "ghost".into(),
            }]
        );
    }

    #[test]
    fn a_disabled_role_is_still_checked_for_skill_bindings() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Review,
            RoleOverrides {
                enabled: Some(false),
                skills: Some(vec!["ghost".into()]),
                ..Default::default()
            },
        );
        let resolved = resolve(&GlobalConfig::default(), &project);
        assert_eq!(validate(&resolved, &NoSkills).len(), 1);
    }

    #[test]
    fn parallel_out_of_range_is_rejected_at_both_ends() {
        for bad in [0u32, 6, 99] {
            let project = ProjectConfig {
                loop_overrides: LoopOverrides {
                    parallel: Some(bad),
                    ..Default::default()
                },
                ..Default::default()
            };
            let resolved = resolve(&GlobalConfig::default(), &project);
            assert_eq!(
                validate(&resolved, &NoSkills),
                vec![ConfigViolation::ParallelOutOfRange {
                    value: bad,
                    min: PARALLEL_MIN,
                    max: PARALLEL_MAX,
                }],
                "parallel = {bad} should be rejected"
            );
        }
        for ok in PARALLEL_MIN..=PARALLEL_MAX {
            let project = ProjectConfig {
                loop_overrides: LoopOverrides {
                    parallel: Some(ok),
                    ..Default::default()
                },
                ..Default::default()
            };
            let resolved = resolve(&GlobalConfig::default(), &project);
            assert!(validate(&resolved, &NoSkills).is_empty(), "parallel = {ok}");
        }
    }

    #[test]
    fn zero_rounds_or_factor_is_rejected() {
        let project = ProjectConfig {
            loop_overrides: LoopOverrides {
                design_rounds: Some(0),
                budget_factor: Some(0),
                ..Default::default()
            },
            ..Default::default()
        };
        let resolved = resolve(&GlobalConfig::default(), &project);
        let violations = validate(&resolved, &NoSkills);
        assert!(violations.contains(&ConfigViolation::DesignRoundsOutOfRange { value: 0 }));
        assert!(violations.contains(&ConfigViolation::BudgetFactorOutOfRange { value: 0 }));
    }

    #[test]
    fn every_violation_is_reported_not_just_the_first() {
        let mut project = ProjectConfig {
            loop_overrides: LoopOverrides {
                parallel: Some(9),
                ..Default::default()
            },
            ..Default::default()
        };
        project.roles.insert(
            Role::Audit,
            RoleOverrides {
                runtime: Some(Runtime::Claude),
                model: Some("opus".into()),
                skills: Some(vec!["ghost".into()]),
                ..Default::default()
            },
        );
        let resolved = resolve(&named_global(), &project);
        let violations = validate(&resolved, &NoSkills);
        assert_eq!(violations.len(), 3, "{violations:#?}");
    }

    #[test]
    fn an_empty_model_means_the_cli_default_and_is_not_a_violation_on_its_own() {
        // Naming no model is the shipped default, and it is legitimate: each
        // CLI has one, and the set a given account may use is not knowable
        // from here.
        let resolved = resolve(&GlobalConfig::default(), &ProjectConfig::default());
        assert!(resolved.role(Role::Plan).config.model.is_empty());
        assert!(validate(&resolved, &NoSkills).is_empty());
    }

    #[test]
    fn two_default_models_on_the_same_runtime_are_reported_distinctly() {
        // review on Claude with no model named is the same session as plan.
        // The user needs to be told to *name* one, not to change one.
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Review,
            RoleOverrides {
                runtime: Some(Runtime::Claude),
                ..Default::default()
            },
        );
        let resolved = resolve(&GlobalConfig::default(), &project);
        assert_eq!(
            validate(&resolved, &NoSkills),
            vec![ConfigViolation::BothDefaultModels {
                evaluator: Role::Review,
                generator: Role::Plan,
            }]
        );
    }

    #[test]
    fn naming_a_model_on_one_side_resolves_the_default_collision() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Review,
            RoleOverrides {
                runtime: Some(Runtime::Claude),
                model: Some("sonnet".into()),
                ..Default::default()
            },
        );
        let resolved = resolve(&GlobalConfig::default(), &project);
        assert!(validate(&resolved, &NoSkills).is_empty());
    }

    #[test]
    fn prune_drops_role_tables_that_no_longer_override_anything() {
        let mut project = ProjectConfig::default();
        project.roles.insert(Role::Plan, RoleOverrides::default());
        project.roles.insert(
            Role::Impl,
            RoleOverrides {
                model: Some("m".into()),
                ..Default::default()
            },
        );
        project.prune();
        assert_eq!(
            project.roles.keys().copied().collect::<Vec<_>>(),
            vec![Role::Impl]
        );
    }

    #[test]
    fn repair_fills_in_a_role_missing_from_a_hand_edited_global_file() {
        let mut global = GlobalConfig::default();
        global.roles.remove(&Role::Audit);
        global.repair();
        assert_eq!(global.role(Role::Audit).runtime, Runtime::Codex);
        assert!(global.role(Role::Audit).model.is_empty());
    }

    #[test]
    fn global_config_round_trips_through_json() {
        let global = GlobalConfig::default();
        let json = serde_json::to_string(&global).unwrap();
        assert_eq!(serde_json::from_str::<GlobalConfig>(&json).unwrap(), global);
    }

    #[test]
    fn a_sparse_project_config_serializes_without_inherited_fields() {
        let mut project = ProjectConfig::default();
        project.roles.insert(
            Role::Impl,
            RoleOverrides {
                model: Some("m".into()),
                ..Default::default()
            },
        );
        let value = serde_json::to_value(&project).unwrap();
        let impl_table = value.pointer("/roles/impl").unwrap().as_object().unwrap();
        assert_eq!(impl_table.keys().collect::<Vec<_>>(), vec!["model"]);
    }
}
