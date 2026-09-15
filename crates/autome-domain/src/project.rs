//! Project: a local directory and its Git repository. Technical design §4;
//! requirements P-01 through P-08.
//!
//! 2.0 deliberately has *no* project state machine. The 09-13 design had an
//! eight-phase one (Registered → Inspecting → AwaitingTrust → … → Ready) with
//! a parallel hold enumeration; the redesign removed trust confirmation,
//! product intent and the Application Support ProjectHome, and what remained
//! was "a directory, possibly still being onboarded". That is a field, not a
//! machine.

use serde::{Deserialize, Serialize};

/// How far the optional onboarding wizard got (requirement C-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "onboarding", rename_all = "snake_case")]
pub enum Onboarding {
    /// Step 1 and 2 (choose directory, init) always run; the wizard proper is
    /// at step 3..=5.
    InProgress {
        step: u8,
    },
    Skipped,
    Completed,
}

impl Onboarding {
    pub const FIRST_STEP: u8 = 3;
    pub const LAST_STEP: u8 = 5;

    /// Where a freshly added project starts.
    pub const fn start() -> Self {
        Onboarding::InProgress {
            step: Self::FIRST_STEP,
        }
    }

    /// Whether the project page should offer "继续 Onboarding".
    pub const fn is_pending(self) -> bool {
        matches!(self, Onboarding::InProgress { .. })
    }

    /// Whether the init commit has been made. Both terminal outcomes make it
    /// (requirement C-03); an in-progress wizard has not yet.
    pub const fn is_settled(self) -> bool {
        matches!(self, Onboarding::Skipped | Onboarding::Completed)
    }

    /// Advances one step, settling at the end. Returns `None` when the wizard
    /// has already finished, so a stray advance is a caller error rather than
    /// a silent no-op.
    pub fn advance(self) -> Option<Self> {
        match self {
            Onboarding::InProgress { step } if step < Self::LAST_STEP => {
                Some(Onboarding::InProgress { step: step + 1 })
            }
            Onboarding::InProgress { .. } => Some(Onboarding::Completed),
            _ => None,
        }
    }
}

/// What `project.add` found at the chosen path, and therefore what it had to
/// do. Recorded so the UI can explain what happened without re-probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddDisposition {
    /// The directory did not exist and was created, then `git init`.
    CreatedAndInitialised,
    /// The directory existed but was not a repository; `git init` ran.
    InitialisedExisting,
    /// The directory was already a Git repository; nothing was initialised.
    AdoptedExisting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    /// Canonical absolute path to the repository root.
    pub path: String,
    pub display_name: String,
    /// The branch task branches are cut from and merged back into (P-03).
    pub default_branch: String,
    pub parallel_limit: u32,
    pub onboarding: Onboarding,
    pub disposition: AddDisposition,
    pub added_at: String,
    /// Set when the project is removed from Autome's registry. The directory
    /// itself is never touched (requirement P-08).
    pub removed_at: Option<String>,
}

impl Project {
    pub fn is_active(&self) -> bool {
        self.removed_at.is_none()
    }

    /// `.worktree/<slug>` relative to the repository root (requirement P-05).
    pub fn worktree_path(&self, slug: &str) -> String {
        format!("{}/.worktree/{slug}", self.path)
    }

    /// The branch name for a task's worktree.
    pub fn branch_name(slug: &str) -> String {
        format!("autome/{slug}")
    }

    /// Where a task's documents live on its own branch.
    pub fn doc_dir(slug: &str) -> String {
        format!("docs/{slug}")
    }

    /// Where a task's documents go once archived (requirement T-12).
    pub fn archive_dir(slug: &str) -> String {
        format!("docs/.archive/{slug}")
    }
}

/// Why a directory cannot become a project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AddRejection {
    /// The path exists but is a file, not a directory.
    NotADirectory {
        path: String,
    },
    /// Already registered — switching to it is the right action, not adding
    /// it twice.
    AlreadyRegistered {
        project_id: String,
    },
    /// The path is inside another registered project's tree. Nested projects
    /// would make `.worktree/` and `.autome/` ambiguous.
    NestedInProject {
        project_id: String,
        path: String,
    },
    /// The repository has no resolvable default branch and none could be
    /// created (e.g. a bare repository).
    NoDefaultBranch {
        path: String,
    },
    PermissionDenied {
        path: String,
        detail: String,
    },
}

impl std::fmt::Display for AddRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddRejection::NotADirectory { path } => write!(f, "{path} 不是目录"),
            AddRejection::AlreadyRegistered { .. } => write!(f, "该目录已经是一个项目"),
            AddRejection::NestedInProject { path, .. } => {
                write!(f, "{path} 位于另一个项目内部，不能嵌套")
            }
            AddRejection::NoDefaultBranch { path } => {
                write!(f, "{path} 没有可用的默认分支")
            }
            AddRejection::PermissionDenied { path, detail } => {
                write!(f, "没有权限访问 {path}：{detail}")
            }
        }
    }
}

/// Why a project cannot be removed from the registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoveRejection {
    /// Requirement P-08: a running task owns a worktree and possibly a live
    /// session; dropping the registration would orphan both.
    HasRunningTasks {
        task_ids: Vec<String>,
    },
    NotFound {
        project_id: String,
    },
}

/// Derives a project's display name from its path. Falls back to the whole
/// path when the last component is unusable, so the name is never empty.
pub fn display_name_from_path(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .find(|c| !c.is_empty() && *c != ".")
        .unwrap_or(path)
        .to_string()
}

/// Converts a one-line request into a branch- and directory-safe slug.
///
/// Kept in the domain because the slug is not cosmetic: it names a Git
/// branch, a worktree directory and a docs directory, all of which must agree
/// and none of which may contain a path separator or a leading dot.
pub fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = true; // suppresses a leading dash
    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch == '-' || ch == '_' || ch.is_whitespace() {
            Some('-')
        } else if !ch.is_ascii() {
            // Keep CJK and other non-ASCII word characters: they are valid in
            // Git ref names and in paths, and dropping them would collapse a
            // Chinese request to an empty slug.
            if ch.is_alphanumeric() {
                Some(ch)
            } else {
                Some('-')
            }
        } else {
            Some('-')
        };
        match mapped {
            Some('-') => {
                if !last_was_dash {
                    slug.push('-');
                    last_was_dash = true;
                }
            }
            Some(c) => {
                slug.push(c);
                last_was_dash = false;
            }
            None => {}
        }
    }
    let slug = slug.trim_matches('-').to_string();
    // Git refuses refs ending in `.lock`, containing `..`, or named `@`.
    let slug = slug.replace("..", "-");
    let slug = slug.trim_end_matches(".lock").trim_matches('-').to_string();
    if slug.is_empty() {
        "task".to_string()
    } else {
        truncate_chars(&slug, 48)
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    let truncated: String = s.chars().take(max).collect();
    truncated.trim_end_matches('-').to_string()
}

/// Appends `-2`, `-3`, … until the slug is unused. The caller supplies the
/// membership test so the domain does not need to know where slugs live.
pub fn unique_slug(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    for n in 2..1000 {
        let candidate = format!("{base}-{n}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    format!("{base}-{}", uuid_like_suffix(base))
}

/// Deterministic short suffix for the pathological case where a thousand
/// slugs collide. Not cryptographic; it only needs to be stable and unlikely.
fn uuid_like_suffix(seed: &str) -> String {
    let mut hash: u64 = 1469598103934665603;
    for b in seed.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:x}").chars().take(6).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_starts_at_step_three_and_completes_after_five() {
        let mut o = Onboarding::start();
        assert_eq!(o, Onboarding::InProgress { step: 3 });
        assert!(o.is_pending());
        assert!(!o.is_settled());
        o = o.advance().unwrap();
        assert_eq!(o, Onboarding::InProgress { step: 4 });
        o = o.advance().unwrap();
        assert_eq!(o, Onboarding::InProgress { step: 5 });
        o = o.advance().unwrap();
        assert_eq!(o, Onboarding::Completed);
        assert!(o.is_settled());
        assert!(!o.is_pending());
        assert_eq!(o.advance(), None);
    }

    #[test]
    fn skipped_onboarding_is_settled_and_not_pending() {
        assert!(Onboarding::Skipped.is_settled());
        assert!(!Onboarding::Skipped.is_pending());
        assert_eq!(Onboarding::Skipped.advance(), None);
    }

    #[test]
    fn paths_are_derived_consistently() {
        let p = Project {
            id: "p1".into(),
            path: "/Users/x/code/island-shop".into(),
            display_name: "island-shop".into(),
            default_branch: "main".into(),
            parallel_limit: 3,
            onboarding: Onboarding::Skipped,
            disposition: AddDisposition::AdoptedExisting,
            added_at: "2026-09-15T00:00:00Z".into(),
            removed_at: None,
        };
        assert_eq!(
            p.worktree_path("checkout-flow"),
            "/Users/x/code/island-shop/.worktree/checkout-flow"
        );
        assert_eq!(
            Project::branch_name("checkout-flow"),
            "autome/checkout-flow"
        );
        assert_eq!(Project::doc_dir("checkout-flow"), "docs/checkout-flow");
        assert_eq!(
            Project::archive_dir("checkout-flow"),
            "docs/.archive/checkout-flow"
        );
        assert!(p.is_active());
    }

    #[test]
    fn display_name_uses_the_last_path_component() {
        assert_eq!(display_name_from_path("/a/b/island-shop"), "island-shop");
        assert_eq!(display_name_from_path("/a/b/island-shop/"), "island-shop");
        assert_eq!(display_name_from_path("island-shop"), "island-shop");
    }

    #[test]
    fn slugify_lowercases_and_dashes_ascii() {
        assert_eq!(slugify("Add Cart Checkout"), "add-cart-checkout");
        assert_eq!(slugify("fix   the    bug"), "fix-the-bug");
        assert_eq!(slugify("--leading-and-trailing--"), "leading-and-trailing");
    }

    #[test]
    fn slugify_keeps_cjk_rather_than_producing_an_empty_slug() {
        assert_eq!(slugify("购物车结算"), "购物车结算");
        assert_eq!(slugify("给 岛屿商店 加购物车"), "给-岛屿商店-加购物车");
    }

    #[test]
    fn slugify_removes_characters_git_refuses_in_a_ref_name() {
        for input in [
            "a..b", "a~b", "a^b", "a:b", "a?b", "a*b", "a[b", "a\\b", "a b",
        ] {
            let slug = slugify(input);
            assert!(!slug.contains(".."), "{input} -> {slug}");
            for bad in ['~', '^', ':', '?', '*', '[', '\\', ' '] {
                assert!(!slug.contains(bad), "{input} -> {slug} contains {bad}");
            }
        }
    }

    #[test]
    fn slugify_never_yields_an_empty_or_dot_leading_slug() {
        for input in ["", "---", "!!!", "...", "   "] {
            let slug = slugify(input);
            assert!(!slug.is_empty(), "{input:?}");
            assert!(!slug.starts_with('.'), "{input:?} -> {slug}");
        }
        assert_eq!(slugify(""), "task");
    }

    #[test]
    fn slugify_does_not_end_in_dot_lock() {
        assert!(!slugify("something.lock").ends_with(".lock"));
    }

    #[test]
    fn slugify_truncates_long_input_without_a_trailing_dash() {
        let slug = slugify(&"word ".repeat(40));
        assert!(slug.chars().count() <= 48);
        assert!(!slug.ends_with('-'));
    }

    #[test]
    fn unique_slug_returns_the_base_when_free() {
        assert_eq!(unique_slug("checkout", |_| false), "checkout");
    }

    #[test]
    fn unique_slug_appends_a_counter_on_collision() {
        let taken = |s: &str| s == "checkout" || s == "checkout-2";
        assert_eq!(unique_slug("checkout", taken), "checkout-3");
    }

    #[test]
    fn rejections_render_a_message() {
        let r = AddRejection::NestedInProject {
            project_id: "p1".into(),
            path: "/a/b".into(),
        };
        assert!(r.to_string().contains("/a/b"));
    }

    #[test]
    fn project_round_trips_through_json() {
        let p = Project {
            id: "p1".into(),
            path: "/a/b".into(),
            display_name: "b".into(),
            default_branch: "main".into(),
            parallel_limit: 3,
            onboarding: Onboarding::InProgress { step: 4 },
            disposition: AddDisposition::CreatedAndInitialised,
            added_at: "t".into(),
            removed_at: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), p);
    }
}
