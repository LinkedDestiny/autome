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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopOverrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_rounds: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_factor: Option<u32>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bug: Option<BugConfig>,
    #[serde(default, rename = "loop")]
    pub loop_overrides: LoopOverrides,
    #[serde(default)]
    pub roles: BTreeMap<Role, RoleOverrides>,
    /// Where this project's task documents go inside the repository that
    /// holds them.
    ///
    /// Project-only, with no global counterpart: there is no sensible
    /// machine-wide answer. It is a
    /// fact about one repository's layout — whether `docs/` is free, whether
    /// it is somebody else's independent repository, whether the team already
    /// keeps `prd/` and `reports/` at the root.
    ///
    /// `None` means the project's own default: `docs/` for a single
    /// repository, `autome/` inside a workspace's document repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_root: Option<String>,
    /// The command the core runs itself, on the exact commit a round left
    /// behind, before anyone is asked to believe that round. Project-only for
    /// the same reason as `doc_root`: what "the checks pass" means is a fact
    /// about one repository. `None` means the project has no gate and the
    /// Loop runs exactly as it did before gates existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<GateConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_same_failure_limit: Option<u32>,
