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
}
