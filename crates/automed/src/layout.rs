//! Where a task's files are, once "the project" can be more than one
//! repository.
//!
//! Until workspaces existed there was one answer to two different questions —
//! `<repo>/.worktree/<slug>` was both *where the session runs* and *where the
//! documents live* — and eleven call sites in the scheduler used the same
//! helper for whichever of the two they meant. For a workspace they are not
//! the same place: the session runs in a directory holding one checkout per
//! member repository, and the documents live inside the member that was
//! designated to hold them.
//!
//! So the two questions get two names. `TaskLayout` answers both, and for a
//! single-repository project it answers them identically — the property
//! `project.rs` pins with an assertion, because a call site that reaches for
//! the wrong one will not fail to compile and will not fail loudly: it will
//! write the design document somewhere nobody reads and surface, a round
//! later, as a missing artefact pointing at the wrong layer entirely.

use std::path::{Path, PathBuf};

use autome_domain::project::Project;

/// One repository a task is working in.
///
/// `Repo` projects have exactly one of these; a workspace task has one per
/// repository it touches, the document repository always among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSlot {
    /// The member's directory name, and the identifier used everywhere else.
    /// Empty for a single-repository project, which has no member to name.
    pub name: String,
    /// The repository itself — where `git worktree add`, `git merge` and
    /// `git branch -d` run.
    pub repo_root: PathBuf,
    /// This task's checkout of it.
    pub worktree: PathBuf,
    /// `autome/<slug>`, the same name in every member.
    pub branch: String,
    /// The branch this one was cut from and merges back into. Members need
    /// not agree: one repository's `main` is another's `master`.
    pub base: String,
    /// Whether this is the repository holding the task's documents.
    pub is_docs: bool,
}

/// Where one task's work happens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLayout {
    /// The session's working directory.
    ///
    /// For a single repository this *is* the checkout. For a workspace it is
    /// the directory the member checkouts sit in, so that an agent sees the
    /// same relative layout it would see in the real workspace.
    pub cwd: PathBuf,
    /// The checkout holding `<doc_root>/<slug>/…` — the design document, the
    /// briefs, the frozen protocol and the evidence.
    pub docs_root: PathBuf,
    /// Every repository this task is working in, documents first.
    pub repos: Vec<RepoSlot>,
}

impl TaskLayout {
    /// The layout of a task in a single-repository project.
    ///
    /// Named rather than inferred from an empty member list: a caller that
    /// means "the old shape" should say so, and Stage-3's workspace
    /// constructor will sit beside this one rather than replacing it.
    pub fn single(project: &Project, slug: &str) -> TaskLayout {
        let repo_root = PathBuf::from(&project.path);
        let worktree = PathBuf::from(project.worktree_path(slug));
        TaskLayout {
            cwd: worktree.clone(),
            docs_root: worktree.clone(),
            repos: vec![RepoSlot {
                name: String::new(),
                repo_root,
                worktree,
                branch: Project::branch_name(slug),
                base: project.default_branch.clone(),
                is_docs: true,
            }],
        }
    }

    /// The layout of a task in a workspace project.
    ///
    /// `working` is the members the task actually touches, as the intake round
    /// named them. The document repository is always present and always first:
    /// a task that could not record what it did is not a task, and merging the
    /// record before the code it describes would put a claim on `main` that
    /// nothing else backs (see the merge order in `run_core_step`).
    ///
    /// Names that are not members of this workspace are dropped rather than
    /// guessed at. They come from a document a model wrote, and creating a
    /// checkout for `../../etc` because it appeared in a `repos:` line is not
    /// a thing this should be able to do.
    pub fn workspace(project: &Project, slug: &str, working: &[String]) -> TaskLayout {
        let docs_name = project.docs_repo.clone().unwrap_or_default();
        let mut names: Vec<String> = vec![docs_name.clone()];
        for name in working {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }

        let repos = names
            .into_iter()
            .filter_map(|name| {
                let member = project.members.iter().find(|m| m.name == name)?;
                Some(RepoSlot {
                    repo_root: PathBuf::from(project.member_path(&member.name)),
                    worktree: PathBuf::from(project.member_worktree(slug, &member.name)),
                    branch: Project::branch_name(slug),
                    base: member.default_branch.clone(),
                    is_docs: member.name == docs_name,
                    name: member.name.clone(),
                })
            })
            .collect();

        TaskLayout {
            cwd: PathBuf::from(project.worktree_path(slug)),
            docs_root: PathBuf::from(project.docs_worktree(slug)),
            repos,
        }
    }

    /// The layout for a task in either kind of project.
    pub fn of(project: &Project, slug: &str, working: &[String]) -> TaskLayout {
        if project.is_workspace() {
            TaskLayout::workspace(project, slug, working)
        } else {
            TaskLayout::single(project, slug)
        }
    }

    /// The repository holding the documents.
    ///
    /// Always present: a task with no document repository could not record
    /// what it did, so the constructors do not allow one.
    pub fn docs(&self) -> &RepoSlot {
        self.repos
            .iter()
            .find(|r| r.is_docs)
            .expect("a task layout always has a document repository")
    }

    /// Every checkout under `cwd`, for the sandbox and for the sweep.
    pub fn worktrees(&self) -> Vec<&Path> {
        self.repos.iter().map(|r| r.worktree.as_path()).collect()
    }

    /// The task's document directory **as the session sees it** — relative to
    /// the directory it is started in.
    ///
    /// Every prompt tells the round where to read and write, and it can only
    /// say so in the round's own terms. For a single repository that is
    /// `docs/<slug>`, exactly what the templates used to hard-code. For a
    /// workspace the session starts one level up, beside the member
    /// checkouts, so the same directory is `<docs-member>/<doc_root>/<slug>`.
    pub fn doc_dir_from_cwd(&self, doc_dir: &str) -> String {
        match self.docs_root.strip_prefix(&self.cwd) {
            Ok(prefix) if prefix.as_os_str().is_empty() => doc_dir.to_string(),
            Ok(prefix) => format!("{}/{doc_dir}", prefix.display()),
            // The documents are not under the working directory at all, which
            // no constructor produces. Naming the directory the session cannot
            // reach would be worse than naming the one it can.
            Err(_) => doc_dir.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::project::{AddDisposition, Onboarding, ProjectKind};

    fn project(path: &str) -> Project {
        Project {
            id: "p1".into(),
            path: path.into(),
            display_name: "p".into(),
            default_branch: "main".into(),
            parallel_limit: 3,
            onboarding: Onboarding::Skipped,
            disposition: AddDisposition::AdoptedExisting,
            added_at: "2026-09-21T00:00:00Z".into(),
            removed_at: None,
            kind: ProjectKind::Repo,
            members: Vec::new(),
            docs_repo: None,
        }
    }

    #[test]
    fn a_single_repository_runs_and_documents_in_the_same_checkout() {
        // The identity that lets every call site be written once. It is also
        // the guard on the refactor that introduced this type: as long as it
        // holds, a call site that picked the wrong one of the two is
        // invisible — which is exactly why it is asserted here rather than
        // trusted.
        let p = project("/x/repo");
        let layout = TaskLayout::single(&p, "checkout-flow");
        assert_eq!(layout.cwd, layout.docs_root);
        assert_eq!(layout.cwd, PathBuf::from("/x/repo/.worktree/checkout-flow"));
    }

    #[test]
    fn a_single_repository_has_exactly_one_slot_and_it_holds_the_documents() {
        let p = project("/x/repo");
        let layout = TaskLayout::single(&p, "s");
        assert_eq!(layout.repos.len(), 1);
        let slot = layout.docs();
        assert_eq!(slot.repo_root, PathBuf::from("/x/repo"));
        assert_eq!(slot.worktree, layout.cwd);
        assert_eq!(slot.branch, "autome/s");
        assert_eq!(slot.base, "main");
        assert_eq!(layout.worktrees(), vec![layout.cwd.as_path()]);
    }

    #[test]
    fn the_base_branch_is_the_projects_own() {
        let mut p = project("/x/repo");
        p.default_branch = "trunk".into();
        assert_eq!(TaskLayout::single(&p, "s").docs().base, "trunk");
    }

    fn workspace(members: &[(&str, &str)], docs: &str) -> Project {
        let mut p = project("/x/ws");
        p.kind = ProjectKind::Workspace;
        p.docs_repo = Some(docs.into());
        p.members = members
            .iter()
            .map(|(name, branch)| autome_domain::project::Member {
                name: (*name).into(),
                default_branch: (*branch).into(),
            })
            .collect();
        p
    }

    #[test]
    fn a_workspace_task_runs_beside_its_checkouts_not_inside_one() {
        let p = workspace(&[("docs", "main"), ("backend", "master")], "docs");
        let layout = TaskLayout::workspace(&p, "s", &["backend".into()]);

        assert_eq!(layout.cwd, PathBuf::from("/x/ws/.worktree/s"));
        assert_eq!(layout.docs_root, PathBuf::from("/x/ws/.worktree/s/docs"));
        assert_ne!(
            layout.cwd, layout.docs_root,
            "the two answers diverge here, which is the whole reason they have two names"
        );
        // The member directories are named after the members, so an agent's
        // relative paths mean the same thing here as in the real workspace.
        assert_eq!(
            layout.worktrees(),
            vec![
                Path::new("/x/ws/.worktree/s/docs"),
                Path::new("/x/ws/.worktree/s/backend"),
            ]
        );
    }

    #[test]
    fn each_member_branches_from_its_own_default() {
        let p = workspace(&[("docs", "main"), ("backend", "master")], "docs");
        let layout = TaskLayout::workspace(&p, "s", &["backend".into()]);
        let backend = layout.repos.iter().find(|r| r.name == "backend").unwrap();
        assert_eq!(
            backend.base, "master",
            "one repo's main is another's master"
        );
        assert_eq!(backend.repo_root, PathBuf::from("/x/ws/backend"));
        assert_eq!(
            backend.branch, "autome/s",
            "the same branch name everywhere"
        );
        assert!(!backend.is_docs);
    }

    #[test]
    fn the_document_repository_is_always_present_and_always_first() {
        let p = workspace(&[("docs", "main"), ("backend", "main")], "docs");
        // Even when the round names only code repositories.
        let layout = TaskLayout::workspace(&p, "s", &["backend".into()]);
        assert_eq!(layout.repos[0].name, "docs");
        assert!(layout.docs().is_docs);
        // And naming it explicitly does not give it two slots.
        let twice = TaskLayout::workspace(&p, "s", &["docs".into(), "backend".into()]);
        assert_eq!(twice.repos.len(), 2);
    }

    #[test]
    fn a_name_that_is_not_a_member_is_dropped_not_guessed_at() {
        // `repos:` comes out of a document a model wrote. Creating a checkout
        // for `../../etc` because it appeared there is not a thing this gets
        // to do.
        let p = workspace(&[("docs", "main"), ("backend", "main")], "docs");
        let layout = TaskLayout::workspace(&p, "s", &["../../etc".into(), "nope".into()]);
        assert_eq!(
            layout
                .repos
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>(),
            vec!["docs"]
        );
    }

    #[test]
    fn the_document_directory_is_named_in_the_sessions_own_terms() {
        // Every prompt tells the round where to read and write, and it can
        // only say so relative to where the round was started.
        let repo = project("/x/repo");
        assert_eq!(
            TaskLayout::single(&repo, "s").doc_dir_from_cwd("docs/s"),
            "docs/s",
            "a single repository sees exactly what the templates used to hard-code"
        );

        let ws = workspace(&[("docs", "main"), ("backend", "main")], "docs");
        assert_eq!(
            TaskLayout::workspace(&ws, "s", &[]).doc_dir_from_cwd("autome/s"),
            "docs/autome/s",
            "a workspace session starts one level up, beside the checkouts"
        );
    }

    #[test]
    fn of_picks_the_shape_from_the_project() {
        let repo = project("/x/repo");
        assert_eq!(
            TaskLayout::of(&repo, "s", &[]),
            TaskLayout::single(&repo, "s")
        );
        let ws = workspace(&[("docs", "main")], "docs");
        assert_eq!(
            TaskLayout::of(&ws, "s", &[]),
            TaskLayout::workspace(&ws, "s", &[])
        );
    }
}
