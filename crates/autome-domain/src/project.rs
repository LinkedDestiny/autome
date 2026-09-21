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
    /// The directory holds several independent repositories and was adopted as
    /// a workspace. **Nothing was initialised** — the whole point.
    AdoptedWorkspace,
}

/// What a project *is*, structurally.
///
/// P-01 has one answer — "a directory; if it is not a repository, `git init`".
/// That is wrong for a directory whose children are each their own repository
/// with their own remote, a layout people use to develop across repositories.
/// Running `git init` over one of those produces an outer repository that
/// records each child as a gitlink, and every later `git add child/…` fails
/// with "is in submodule". It happened on a real machine and cost a task.
///
/// So a project is one of two shapes, decided once at `project.add` and never
/// inferred again: guessing on every read would mean the answer could change
/// under a running task.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    /// One Git repository. Everything 2.0 shipped with; `members` is empty.
    #[default]
    Repo,
    /// A plain directory holding several independent repositories. Autome
    /// never initialises it and never commits at its root.
    Workspace,
}

/// One repository inside a workspace.
///
/// `name` is both the directory name under the workspace root and the
/// identifier the rest of the system uses, so there is exactly one string to
/// keep true. The absolute path is derived (`Project::member_path`) rather
/// than stored: two spellings of the same location is how a project that was
/// moved on disk starts lying.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    /// This repository's own default branch — they need not agree across a
    /// workspace, and a task branches from each one separately.
    pub default_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    /// Canonical absolute path to the repository root, or to the workspace
    /// directory when `kind` is `Workspace`.
    pub path: String,
    pub display_name: String,
    /// The branch task branches are cut from and merged back into (P-03).
    ///
    /// For a workspace this is the docs repository's default branch — each
    /// member carries its own in `members`, and this field stays populated so
    /// that everything reading it keeps getting a truthful answer rather than
    /// an empty string.
    pub default_branch: String,
    pub parallel_limit: u32,
    pub onboarding: Onboarding,
    pub disposition: AddDisposition,
    pub added_at: String,
    /// Set when the project is removed from Autome's registry. The directory
    /// itself is never touched (requirement P-08).
    pub removed_at: Option<String>,
    /// All three default so that a row written before workspaces existed
    /// deserialises as the single-repository project it is.
    #[serde(default)]
    pub kind: ProjectKind,
    /// The repositories inside a workspace, in discovery order. Empty for a
    /// `Repo` project.
    #[serde(default)]
    pub members: Vec<Member>,
    /// Which member holds the task documents. `None` for a `Repo` project,
    /// where the project itself does.
    #[serde(default)]
    pub docs_repo: Option<String>,
}

/// Where task documents go inside the repository that holds them, when
/// nothing says otherwise.
///
/// A workspace's docs repository is a real repository with a real layout of
/// its own, so Autome takes a named subdirectory in it rather than scattering
/// `<slug>/` directories through someone's `prd/` and `reports/`. A
/// single-repository project keeps `docs/`, which is where its documents have
/// always been.
pub const DEFAULT_DOC_ROOT_REPO: &str = "docs";
pub const DEFAULT_DOC_ROOT_WORKSPACE: &str = "autome";

impl Project {
    pub fn is_active(&self) -> bool {
        self.removed_at.is_none()
    }

    pub fn is_workspace(&self) -> bool {
        self.kind == ProjectKind::Workspace
    }

    /// The task's own directory: `.worktree/<slug>`, and the session's cwd
    /// (requirement P-05).
    ///
    /// For a `Repo` project it *is* the checkout. For a workspace it is a
    /// plain directory holding one checkout per member, named after the
    /// member — so the layout inside matches the workspace itself and an
    /// agent's relative paths mean the same thing in both.
    pub fn worktree_path(&self, slug: &str) -> String {
        format!("{}/.worktree/{slug}", self.path)
    }

    /// Absolute path of one member repository.
    pub fn member_path(&self, member: &str) -> String {
        format!("{}/{member}", self.path)
    }

    /// Where one member's checkout goes inside the task's directory.
    pub fn member_worktree(&self, slug: &str, member: &str) -> String {
        format!("{}/{member}", self.worktree_path(slug))
    }

    /// The checkout that holds the task's documents.
    ///
    /// The identity that makes a workspace the general case and a single
    /// repository the N=1 case of it: for a `Repo` project this is exactly
    /// `worktree_path`, so every caller can be written once.
    pub fn docs_worktree(&self, slug: &str) -> String {
        match &self.docs_repo {
            Some(member) => self.member_worktree(slug, member),
            None => self.worktree_path(slug),
        }
    }

    /// The branch name for a task's worktree.
    pub fn branch_name(slug: &str) -> String {
        format!("autome/{slug}")
    }

    /// Where a task's documents live inside the repository that holds them.
    pub fn doc_dir(root: &str, slug: &str) -> String {
        format!("{}/{slug}", root.trim_end_matches('/'))
    }

    /// Where a task's documents go once archived (requirement T-12).
    pub fn archive_dir(root: &str, slug: &str) -> String {
        format!("{}/.archive/{slug}", root.trim_end_matches('/'))
    }

    /// This project's document root when its configuration does not name one.
    pub fn default_doc_root(&self) -> &'static str {
        if self.is_workspace() {
            DEFAULT_DOC_ROOT_WORKSPACE
        } else {
            DEFAULT_DOC_ROOT_REPO
        }
    }
}

/// Whether a project may use `root` as its document root, and why not.
///
/// The rules keep the documents inside the repository that is supposed to hold
/// them, and out of the directories that are not documents:
///
/// * relative, so they travel with the branch rather than landing somewhere on
///   the machine no clone will have;
/// * no `..`, for the same reason — and because a root that escapes the
///   checkout would have Autome committing outside the tree it was given;
/// * not Git's or Autome's own directory.
pub fn validate_doc_root(root: &str) -> Result<(), String> {
    if root.trim().is_empty() {
        return Err("文档目录不能为空".to_string());
    }
    if root.trim() != root {
        return Err("文档目录首尾不能有空格".to_string());
    }
    if root.starts_with('/') || root.contains(':') || root.contains('\\') {
        return Err(format!("文档目录要用仓库内的相对路径，`{root}` 不是"));
    }
    for part in root.split('/') {
        if part.is_empty() {
            return Err(format!("文档目录里有空的一段：`{root}`"));
        }
        if part == "." || part == ".." {
            return Err(format!("文档目录不能包含 `{part}`：`{root}`"));
        }
    }
    let first = root.split('/').next().unwrap_or_default();
    if matches!(first, ".git" | ".autome" | ".worktree") {
        return Err(format!(
            "`{first}` 是 Git 或 Autome 自己的目录，不能放任务文档"
        ));
    }
    Ok(())
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
        truncate_chars(&slug, SLUG_MAX_CHARS)
    }
}

/// How long a slug may be.
///
/// A real run produced `在-readme-md-末尾加一行-hello-from-autome-只改这一个文件`
/// from a one-line request — a valid path and a valid Git ref, and unusable as
/// either. The slug names a branch the user reads in `git log`, a directory
/// they `cd` into, and a docs path that ends up in the merge commit. Short
/// matters more than complete, and the task id is the stable identifier
/// anyway.
pub const SLUG_MAX_CHARS: usize = 24;

/// Truncates to `max` characters, then back to the last word boundary so the
/// slug does not end mid-word. Falls back to the hard cut when there is no
/// boundary to fall back to.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    match cut.rfind('-') {
        // Only honour a boundary that leaves something substantial; otherwise
        // a request beginning with a long word would collapse to nothing.
        Some(i) if i >= max / 2 => cut[..i].to_string(),
        _ => cut.trim_end_matches('-').to_string(),
    }
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
            kind: ProjectKind::Repo,
            members: Vec::new(),
            docs_repo: None,
        };
        assert_eq!(
            p.worktree_path("checkout-flow"),
            "/Users/x/code/island-shop/.worktree/checkout-flow"
        );
        assert_eq!(
            Project::branch_name("checkout-flow"),
            "autome/checkout-flow"
        );
        assert_eq!(
            Project::doc_dir("docs", "checkout-flow"),
            "docs/checkout-flow"
        );
        assert_eq!(
            Project::archive_dir("docs", "checkout-flow"),
            "docs/.archive/checkout-flow"
        );
        // The identity the whole workspace model rests on: a single
        // repository is the one-member case, so the checkout that holds the
        // documents *is* the task's directory. Every caller can then be
        // written once. If this ever stops holding, the design document of a
        // single-repo task starts being written somewhere nobody reads it,
        // and the task fails at round one for a reason that points at the
        // wrong layer entirely.
        assert_eq!(
            p.docs_worktree("checkout-flow"),
            p.worktree_path("checkout-flow")
        );
        assert_eq!(p.default_doc_root(), "docs");
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
        assert!(slug.chars().count() <= SLUG_MAX_CHARS);
        assert!(!slug.ends_with('-'));
    }

    #[test]
    fn a_whole_sentence_does_not_become_the_branch_name() {
        // From a real run: the entire request became the slug, and therefore
        // the branch, the worktree directory and the docs path.
        let slug = slugify("在 README.md 末尾加一行「hello from autome」。只改这一个文件。");
        assert!(
            slug.chars().count() <= SLUG_MAX_CHARS,
            "{slug} is {} chars",
            slug.chars().count()
        );
        assert!(!slug.ends_with('-'), "{slug}");
        assert!(!slug.is_empty());
    }

    #[test]
    fn truncation_prefers_a_word_boundary_but_never_yields_nothing() {
        assert_eq!(
            slugify("add cart checkout flow with promo codes and stock checks"),
            "add-cart-checkout-flow"
        );
        // A single long word has no boundary to fall back to; a hard cut is
        // better than an empty slug.
        let one_word = slugify(&"x".repeat(60));
        assert_eq!(one_word.chars().count(), SLUG_MAX_CHARS);
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
            kind: ProjectKind::Repo,
            members: Vec::new(),
            docs_repo: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), p);
    }
}
