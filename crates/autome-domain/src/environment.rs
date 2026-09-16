//! Local environment: the four things 2.0 checks for. Technical design §11;
//! requirements E-01 through E-05.
//!
//! The 09-13 design had an Environment Center with install transactions,
//! qualification receipts and a four-axis matrix (exists / integrity / auth /
//! qualified) over eight components. The redesign cut that to four components
//! and two facts each: is it there, and (for the two CLIs) is it logged in.
//! Anything more was ceremony around `--version`.

use serde::{Deserialize, Serialize};

use crate::role::Runtime;

/// The four checked components.
///
/// Serde goes through `as_str`/`parse` rather than a derive, so the enum has
/// exactly one name on the wire. `rename_all = "snake_case"` turned `ITerm2`
/// into `i_term2` while `as_str()` said `iterm2`, and the renderer — which
/// matched on `as_str`'s spelling — could never find the component. iTerm2
/// showed as missing on a machine that had it installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Component {
    Git,
    Claude,
    Codex,
    ITerm2,
}

impl Serialize for Component {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Component {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Component::parse(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown component `{raw}`; expected one of git, claude, codex, iterm2"
            ))
        })
    }
}

impl Component {
    pub const ALL: [Component; 4] = [
        Component::Git,
        Component::Claude,
        Component::Codex,
        Component::ITerm2,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Component::Git => "git",
            Component::Claude => "claude",
            Component::Codex => "codex",
            Component::ITerm2 => "iterm2",
        }
    }

    pub fn parse(s: &str) -> Option<Component> {
        Component::ALL.into_iter().find(|c| c.as_str() == s)
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Component::Git => "Git",
            Component::Claude => "Claude Code",
            Component::Codex => "Codex",
            Component::ITerm2 => "iTerm2",
        }
    }

    /// The runtime this component provides, if it is a CLI harness.
    pub const fn runtime(self) -> Option<Runtime> {
        match self {
            Component::Claude => Some(Runtime::Claude),
            Component::Codex => Some(Runtime::Codex),
            _ => None,
        }
    }

    /// Whether a missing or broken install stops every task, rather than
    /// degrading one thing. Git is the only hard requirement: without it there
    /// are no worktrees at all. A missing iTerm2 falls back to Terminal
    /// (requirement E-02), and a broken CLI only blocks the roles routed to it.
    pub const fn is_required(self) -> bool {
        matches!(self, Component::Git)
    }
}

/// Login state, for the two CLIs. `NotApplicable` for Git and iTerm2 rather
/// than an `Option`, so a UI cannot accidentally render "not logged in" for a
/// component that has no concept of login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "login", rename_all = "snake_case")]
pub enum Login {
    NotApplicable,
    Ok {
        account_hint: Option<String>,
    },
    Expired,
    /// The probe itself could not run, e.g. the CLI changed its flags. Not the
    /// same as `Expired`: the fix is different, and calling it expired would
    /// send the user to re-authenticate for no reason.
    Unknown {
        detail: String,
    },
}

impl Login {
    pub fn is_blocking(&self) -> bool {
        matches!(self, Login::Expired)
    }
}

/// What one probe found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub component: Component,
    pub present: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub login: Login,
    /// When this component was last probed.
    pub checked_at: String,
}

impl ComponentStatus {
    pub fn missing(component: Component, checked_at: impl Into<String>) -> Self {
        Self {
            component,
            present: false,
            version: None,
            path: None,
            login: Login::NotApplicable,
            checked_at: checked_at.into(),
        }
    }

    /// Whether this component is usable right now.
    pub fn is_ready(&self) -> bool {
        self.present && !self.login.is_blocking()
    }

    /// Severity for the dashboard banner (requirement E-05).
    pub fn severity(&self) -> Severity {
        if self.is_ready() {
            Severity::Ok
        } else if !self.present
            && !self.component.is_required()
            && self.component == Component::ITerm2
        {
            // iTerm2 has a working fallback, so its absence is a warning.
            Severity::Warning
        } else {
            Severity::Blocking
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Ok,
    Warning,
    Blocking,
}

/// The whole environment as last observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub components: Vec<ComponentStatus>,
    pub checked_at: String,
}

impl Environment {
    pub fn get(&self, component: Component) -> Option<&ComponentStatus> {
        self.components.iter().find(|c| c.component == component)
    }

    /// Whether the runtime a role is routed to can actually start a session.
    pub fn runtime_ready(&self, runtime: Runtime) -> bool {
        let component = match runtime {
            Runtime::Claude => Component::Claude,
            Runtime::Codex => Component::Codex,
        };
        self.get(component).is_some_and(|c| c.is_ready())
    }

    /// The single worst severity, for the dashboard banner.
    pub fn severity(&self) -> Severity {
        self.components
            .iter()
            .map(ComponentStatus::severity)
            .max()
            .unwrap_or(Severity::Blocking)
    }

    /// Components that need the user's attention, worst first, then in a
    /// stable component order so the banner text does not reshuffle between
    /// otherwise identical probes.
    pub fn problems(&self) -> Vec<&ComponentStatus> {
        let mut problems: Vec<&ComponentStatus> = self
            .components
            .iter()
            .filter(|c| c.severity() != Severity::Ok)
            .collect();
        problems.sort_by_key(|c| (std::cmp::Reverse(c.severity()), c.component));
        problems
    }

    /// Whether any task can start at all.
    pub fn can_run_anything(&self) -> bool {
        Component::ALL
            .into_iter()
            .filter(|c| c.is_required())
            .all(|c| self.get(c).is_some_and(|s| s.is_ready()))
    }
}

/// An install action the user can trigger (requirement E-02). The command runs
/// in a visible terminal; this type only describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallRecipe {
    pub component: Component,
    /// Shell command to run.
    pub command: String,
    /// What must already exist for the command to work, e.g. Homebrew or npm.
    pub prerequisite: Option<String>,
    /// Shown to the user before they confirm.
    pub note: String,
}

/// The shipped install recipes. Deliberately data, not a strategy object:
/// there are four of them and they change with the ecosystem, not with logic.
pub fn install_recipe(component: Component) -> InstallRecipe {
    match component {
        Component::Git => InstallRecipe {
            component,
            command: "xcode-select --install".into(),
            prerequisite: None,
            note: "安装 Xcode 命令行工具，其中包含 git。会弹出系统对话框。".into(),
        },
        Component::Claude => InstallRecipe {
            component,
            command: "npm install -g @anthropic-ai/claude-code".into(),
            prerequisite: Some("npm".into()),
            note: "安装后需要执行 claude 登录。".into(),
        },
        Component::Codex => InstallRecipe {
            component,
            command: "npm install -g @openai/codex".into(),
            prerequisite: Some("npm".into()),
            note: "安装后需要执行 codex 登录。".into(),
        },
        Component::ITerm2 => InstallRecipe {
            component,
            command: "brew install --cask iterm2".into(),
            prerequisite: Some("brew".into()),
            note: "会话窗口会改用 iTerm2；未安装时暂用系统 Terminal。".into(),
        },
    }
}

/// The login command for a CLI component. `None` for the two that have no
/// login.
pub fn login_command(component: Component) -> Option<&'static str> {
    match component {
        Component::Claude => Some("claude /login"),
        Component::Codex => Some("codex login"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(component: Component, login: Login) -> ComponentStatus {
        ComponentStatus {
            component,
            present: true,
            version: Some("1.0".into()),
            path: Some("/usr/bin/x".into()),
            login,
            checked_at: "t".into(),
        }
    }

    fn env(components: Vec<ComponentStatus>) -> Environment {
        Environment {
            components,
            checked_at: "t".into(),
        }
    }

    fn healthy() -> Environment {
        env(vec![
            ok(Component::Git, Login::NotApplicable),
            ok(Component::Claude, Login::Ok { account_hint: None }),
            ok(Component::Codex, Login::Ok { account_hint: None }),
            ok(Component::ITerm2, Login::NotApplicable),
        ])
    }

    #[test]
    fn a_healthy_environment_is_ok_everywhere() {
        let e = healthy();
        assert_eq!(e.severity(), Severity::Ok);
        assert!(e.problems().is_empty());
        assert!(e.can_run_anything());
        assert!(e.runtime_ready(Runtime::Claude));
        assert!(e.runtime_ready(Runtime::Codex));
    }

    #[test]
    fn an_expired_login_blocks_only_its_own_runtime() {
        let mut e = healthy();
        e.components[2].login = Login::Expired;
        assert!(!e.runtime_ready(Runtime::Codex));
        assert!(e.runtime_ready(Runtime::Claude));
        assert_eq!(e.severity(), Severity::Blocking);
        assert!(e.can_run_anything(), "git is still fine");
    }

    #[test]
    fn a_missing_iterm2_is_a_warning_not_a_blocker() {
        let mut e = healthy();
        e.components[3] = ComponentStatus::missing(Component::ITerm2, "t");
        assert_eq!(e.severity(), Severity::Warning);
        assert_eq!(e.problems().len(), 1);
        assert!(e.can_run_anything());
    }

    #[test]
    fn a_missing_git_stops_everything() {
        let mut e = healthy();
        e.components[0] = ComponentStatus::missing(Component::Git, "t");
        assert!(!e.can_run_anything());
        assert_eq!(e.severity(), Severity::Blocking);
    }

    #[test]
    fn a_missing_cli_is_blocking_but_does_not_stop_git_work() {
        let mut e = healthy();
        e.components[1] = ComponentStatus::missing(Component::Claude, "t");
        assert_eq!(e.severity(), Severity::Blocking);
        assert!(!e.runtime_ready(Runtime::Claude));
        assert!(e.can_run_anything());
    }

    #[test]
    fn unknown_login_is_not_treated_as_expired() {
        let mut e = healthy();
        e.components[1].login = Login::Unknown {
            detail: "flag changed".into(),
        };
        assert!(e.runtime_ready(Runtime::Claude), "unknown is not blocking");
        assert_eq!(e.severity(), Severity::Ok);
    }

    #[test]
    fn problems_are_ordered_worst_first_then_by_component() {
        let mut e = healthy();
        e.components[1].login = Login::Expired; // Claude, blocking
        e.components[3] = ComponentStatus::missing(Component::ITerm2, "t"); // warning
        let problems = e.problems();
        assert_eq!(problems[0].component, Component::Claude);
        assert_eq!(problems[1].component, Component::ITerm2);
    }

    #[test]
    fn only_git_is_a_hard_requirement() {
        let required: Vec<Component> = Component::ALL
            .into_iter()
            .filter(|c| c.is_required())
            .collect();
        assert_eq!(required, vec![Component::Git]);
    }

    #[test]
    fn every_component_name_round_trips() {
        for c in Component::ALL {
            assert_eq!(Component::parse(c.as_str()), Some(c));
            assert!(!c.display_name().is_empty());
        }
    }

    #[test]
    fn a_component_has_exactly_one_name_on_the_wire() {
        // The bug: a serde derive spelled `ITerm2` as `i_term2` while
        // `as_str()` said `iterm2`. The renderer matched on `as_str`'s
        // spelling and never found it, so iTerm2 read as missing on a machine
        // that had it.
        for c in Component::ALL {
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(
                json,
                format!("\"{}\"", c.as_str()),
                "{c:?} serialises differently from as_str()"
            );
            assert_eq!(serde_json::from_str::<Component>(&json).unwrap(), c);
        }
        assert_eq!(
            serde_json::to_string(&Component::ITerm2).unwrap(),
            "\"iterm2\""
        );
    }

    #[test]
    fn an_unknown_component_name_names_the_real_ones() {
        let err = serde_json::from_str::<Component>("\"i_term2\"").unwrap_err();
        assert!(err.to_string().contains("iterm2"), "{err}");
    }

    #[test]
    fn each_component_has_an_install_recipe_with_a_command() {
        for c in Component::ALL {
            let r = install_recipe(c);
            assert_eq!(r.component, c);
            assert!(!r.command.is_empty());
            assert!(!r.note.is_empty());
        }
    }

    #[test]
    fn only_the_two_clis_have_a_login_command() {
        assert!(login_command(Component::Claude).is_some());
        assert!(login_command(Component::Codex).is_some());
        assert!(login_command(Component::Git).is_none());
        assert!(login_command(Component::ITerm2).is_none());
    }

    #[test]
    fn runtime_maps_back_to_exactly_the_two_cli_components() {
        let with_runtime: Vec<Component> = Component::ALL
            .into_iter()
            .filter(|c| c.runtime().is_some())
            .collect();
        assert_eq!(with_runtime, vec![Component::Claude, Component::Codex]);
    }

    #[test]
    fn an_environment_missing_a_component_entirely_reports_it_as_not_ready() {
        let e = env(vec![ok(Component::Git, Login::NotApplicable)]);
        assert!(!e.runtime_ready(Runtime::Claude));
        assert!(
            e.can_run_anything(),
            "git present is enough for the hard gate"
        );
    }

    #[test]
    fn environment_round_trips_through_json() {
        let e = healthy();
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<Environment>(&json).unwrap(), e);
    }
}
