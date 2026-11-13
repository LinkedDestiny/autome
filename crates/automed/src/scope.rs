//! Path-only restoration. The caller persists `plan` before calling `apply`.
use crate::git;
use autome_domain::scope::{RestoredRepo, ScopeRestored};
use std::path::{Path, PathBuf};

pub type RestoreError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, RestoreError>;

pub struct RepoSpec {
    pub name: String,
    pub worktree: PathBuf,
    pub base: String,
    pub is_docs: bool,
}

pub struct RestoreSpec {
    pub task: String,
    pub session_id: String,
    pub role: String,
    pub label: String,
    pub round: u32,
    /// Absolute directory for this task's patches in the primary checkout.
    pub patch_dir: PathBuf,
    pub docs_prefix: PathBuf,
    pub repos: Vec<RepoSpec>,
}

fn paths(out: &str) -> Vec<String> {
    out.split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn changed(repo: &Path, base: &str, head: &str) -> Result<Vec<String>> {
    Ok(paths(
        &git::run_ok(
            repo,
            &["diff", "--name-only", "--no-renames", "-z", base, head],
        )?
        .stdout,
    ))
}

/// Shared boundary calculation for the guard and restoration planning.
pub fn outside_paths(repo: &RepoSpec, docs_prefix: &Path) -> Result<Vec<String>> {
    Ok(changed(&repo.worktree, &repo.base, "HEAD")?
        .into_iter()
        .filter(|p| !repo.is_docs || !Path::new(p).starts_with(docs_prefix))
        .collect())
}

pub fn pending(store: &crate::store::Store, task: &str) -> Result<Vec<ScopeRestored>> {
    let begun = store
        .task_events(task, "session.scope_restore_begun")?
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ScopeRestored>, _>>()?;
    let failed = store
        .task_events(task, "session.scope_restore_failed")?
        .iter()
        .filter_map(|v| v["session_id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let handled = store
        .task_events(task, "session.scope_patches_handled")?
        .iter()
        .filter_map(|v| v["patches"].as_array())
        .flatten()
        .filter_map(|v| v.as_str().map(PathBuf::from))
        .collect::<Vec<_>>();
    Ok(autome_domain::scope::pending(&begun, &failed, &handled)
        .into_iter()
        .cloned()
        .collect())
}

fn dirty(repo: &Path) -> Result<Vec<String>> {
    let mut result = Vec::new();
    for args in [
        vec!["diff", "--name-only", "--no-renames", "-z"],
        vec!["diff", "--cached", "--name-only", "--no-renames", "-z"],
        vec!["ls-files", "--others", "--exclude-standard", "-z"],
    ] {
        result.extend(paths(&git::run_ok(repo, &args)?.stdout));
    }
    Ok(result)
}

/// Computes literal paths and atomically saves binary-capable patches. No Git writes.
pub fn plan(spec: &RestoreSpec) -> Result<Option<ScopeRestored>> {
    let mut result = ScopeRestored {
        task: spec.task.clone(),
        session_id: spec.session_id.clone(),
        role: spec.role.clone(),
        label: spec.label.clone(),
        round: spec.round,
        repos: vec![],
    };
    for repo in &spec.repos {
        let pre_head = git::head_sha(&repo.worktree)?;
        let outside = outside_paths(repo, &spec.docs_prefix)?;
        if outside.is_empty() {
            continue;
        }
        if !dirty(&repo.worktree)?.is_empty() {
            return Err("worktree is dirty before restoration".into());
        }
        if !spec.patch_dir.is_absolute() {
            return Err("patch directory must be absolute".into());
        }
        let suffix = if spec.repos.len() > 1 {
            format!("--{}", repo.name)
        } else {
            String::new()
        };
        let filename = format!("{}{suffix}.patch", spec.session_id);
        if Path::new(&filename).components().count() != 1 {
            return Err("invalid patch filename".into());
        }
        std::fs::create_dir_all(&spec.patch_dir)?;
        let patch = spec.patch_dir.join(filename);
        let mut args = vec![
            "--literal-pathspecs",
            "diff",
            "--binary",
            "--no-renames",
            &repo.base,
            &pre_head,
            "--",
        ];
        args.extend(outside.iter().map(String::as_str));
        let diff = git::run_ok(&repo.worktree, &args)?.stdout;
        let temporary = spec.patch_dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&temporary, diff)?;
        std::fs::rename(&temporary, &patch)?;
        let stat = git::run_ok(
            &repo.worktree,
            &[
                "-c",
                "core.quotePath=false",
                "apply",
                "--stat",
                patch.to_str().ok_or("non UTF-8 patch path")?,
            ],
        )?
        .stdout;
        result.repos.push(RestoredRepo {
            repo: repo.name.clone(),
            worktree: repo.worktree.clone(),
            base: repo.base.clone(),
            pre_head,
            commit: None,
            paths: outside,
            patch,
            stat,
        });
    }
    Ok((!result.repos.is_empty()).then_some(result))
}

fn restore(repo: &RestoredRepo, source: &str) -> Result<()> {
    // A previous restore may already have removed a session-added file from
    // both the index and disk. Passing it again is a Git pathspec error.
    let mut known = paths(
        &git::run_ok(
            &repo.worktree,
            &["ls-tree", "-r", "--name-only", "-z", source],
        )?
        .stdout,
    );
    known.extend(paths(
        &git::run_ok(&repo.worktree, &["ls-files", "-z"])?.stdout,
    ));
    let selected: Vec<_> = repo.paths.iter().filter(|p| known.contains(p)).collect();
    for path in repo.paths.iter().filter(|p| !known.contains(p)) {
        if repo.worktree.join(path).symlink_metadata().is_ok() {
            return Err("untracked path blocks resumed restoration".into());
        }
    }
    if selected.is_empty() {
        return Ok(());
    }
    let source = format!("--source={source}");
    let mut args = vec![
        "--literal-pathspecs",
        "restore",
        &source,
        "--staged",
        "--worktree",
        "--",
    ];
    args.extend(selected.into_iter().map(String::as_str));
    git::run_ok(&repo.worktree, &args)?;
    Ok(())
}

fn commit(repo: &RestoredRepo, message: &str) -> Result<String> {
    git::run_ok(
        &repo.worktree,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "user.name=Autome",
            "-c",
            "user.email=autome@localhost",
            "-c",
            "commit.gpgSign=false",
            "commit",
            "-m",
            message,
        ],
    )?;
    Ok(git::head_sha(&repo.worktree)?)
}

fn remaining(repo: &RestoredRepo) -> Result<bool> {
    Ok(changed(&repo.worktree, &repo.base, "HEAD")?
        .iter()
        .any(|p| repo.paths.contains(p)))
}

/// Idempotently finishes a persisted plan, including staged pre-commit interruptions.
pub fn apply(intent: &ScopeRestored) -> Result<ScopeRestored> {
    let mut result = intent.clone();
    for repo in &mut result.repos {
        let dirty = dirty(&repo.worktree)?;
        if dirty.iter().any(|p| !repo.paths.contains(p)) {
            return Err("dirty paths outside restoration plan".into());
        }
        if !remaining(repo)? {
            if !dirty.is_empty() {
                return Err("completed restoration has dirty paths".into());
            }
            repo.commit = Some(git::head_sha(&repo.worktree)?);
            continue;
        }
        if git::head_sha(&repo.worktree)? != repo.pre_head {
            return Err("HEAD changed since restoration plan".into());
        }
        restore(repo, &repo.base)?;
        git::run_ok(
            &repo.worktree,
            &[
                "apply",
                "--check",
                repo.patch.to_str().ok_or("non UTF-8 patch path")?,
            ],
        )?;
        let message = format!(
            "chore(autome): {} {} #{} 越界改动已还原\n\n会话：{}\n会话开始基线：{}\n还原的文件（{} 个）：\n- {}\n补丁：{}\n文档目录内的改动原样保留，由下一个实现轮 / 修复轮决定是否采纳。",
            intent.task,
            intent.label,
            intent.round,
            intent.session_id,
            repo.base,
            repo.paths.len(),
            repo.paths.join("\n- "),
            repo.patch.display()
        );
        repo.commit = Some(commit(repo, &message)?);
    }
    for repo in &result.repos {
        if remaining(repo)? {
            return Err("outside paths remain after restoration".into());
        }
    }
    Ok(result)
}

/// Restores the session tree through forward commits, never moving branch refs back.
pub fn compensate(intent: &ScopeRestored) -> Result<()> {
    for repo in &intent.repos {
        let head = git::head_sha(&repo.worktree)?;
        let dirty = dirty(&repo.worktree)?;
        if dirty.iter().any(|p| !repo.paths.contains(p)) {
            // An untouched failing checkout must remain untouched.
            if head == repo.pre_head {
                continue;
            }
            return Err("cannot compensate unrelated dirty paths".into());
        }
        if head == repo.pre_head && dirty.is_empty() {
            continue;
        }
        if head != repo.pre_head {
            git::run_ok(
                &repo.worktree,
                &["merge-base", "--is-ancestor", &repo.pre_head, "HEAD"],
            )?;
            if changed(&repo.worktree, &repo.pre_head, "HEAD")?
                .iter()
                .any(|p| !repo.paths.contains(p))
            {
                return Err("cannot compensate unrelated commits".into());
            }
        }
        restore(repo, &repo.pre_head)?;
        if !git::run(&repo.worktree, &["diff", "--cached", "--quiet"])?.ok() {
            commit(
                repo,
                &format!(
                    "chore(autome): {} {} #{} 越界改动自动还原未完成，已恢复会话结束时的文件",
                    intent.task, intent.label, intent.round
                ),
            )?;
        }
        if git::tree_id(&repo.worktree, "HEAD")? != git::tree_id(&repo.worktree, &repo.pre_head)? {
            return Err("compensation tree mismatch".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
        spec: RestoreSpec,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    fn git(repo: &Path, args: &[&str]) -> String {
        crate::git::run_ok(repo, args).unwrap().stdout
    }
    fn save(repo: &Path) {
        git(repo, &["add", "-A"]);
        git(
            repo,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@localhost",
                "-c",
                "commit.gpgSign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "-m",
                "session",
            ],
        );
    }
    fn write(repo: &Path, name: &str, data: &[u8]) {
        std::fs::write(repo.join(name), data).unwrap();
    }
    fn fixture(count: usize) -> Fixture {
        let root = std::env::temp_dir().join(format!("autome-scope-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let mut repos = Vec::new();
        for n in 0..count {
            let repo = root.join(format!("repo{n}"));
            std::fs::create_dir_all(repo.join("docs/task")).unwrap();
            git(&repo, &["init", "-q"]);
            write(&repo, "modified.rs", b"before\n");
            write(&repo, "deleted.rs", b"deleted\n");
            write(&repo, "x1decoy.rs", b"literal path control\n");
            write(&repo, "docs/task/evidence.md", b"old docs\n");
            save(&repo);
            let base = crate::git::head_sha(&repo).unwrap();
            write(&repo, "modified.rs", b"after\n");
            write(&repo, "added.rs", b"added\n");
            std::fs::remove_file(repo.join("deleted.rs")).unwrap();
            write(&repo, "docs/task/evidence.md", b"new docs\n");
            save(&repo);
            repos.push(RepoSpec {
                name: format!("repo{n}"),
                worktree: repo,
                base,
                is_docs: n == 0,
            });
        }
        Fixture {
            spec: RestoreSpec {
                task: "T-69".into(),
                session_id: "S-12".into(),
                role: "audit".into(),
                label: "审计".into(),
                round: 1,
                patch_dir: root.join("patches"),
                docs_prefix: "docs/task".into(),
                repos,
            },
            root,
        }
    }
    fn planned(f: &Fixture) -> ScopeRestored {
        plan(&f.spec).unwrap().unwrap()
    }
    fn assert_restored(result: &ScopeRestored) {
        for repo in &result.repos {
            git(
                &repo.worktree,
                &["apply", "--check", repo.patch.to_str().unwrap()],
            );
            assert!(
                changed(&repo.worktree, &repo.base, "HEAD")
                    .unwrap()
                    .iter()
                    .all(|p| p.starts_with("docs/task/"))
            );
            assert!(dirty(&repo.worktree).unwrap().is_empty());
            assert_eq!(
                repo.commit.as_ref().unwrap(),
                &crate::git::head_sha(&repo.worktree).unwrap()
            );
        }
    }
    #[test]
    fn restore_reverts_modified_added_and_deleted_paths_and_keeps_docs() {
        let f = fixture(1);
        let result = apply(&planned(&f)).unwrap();
        assert_restored(&result);
        let repo = &result.repos[0].worktree;
        assert_eq!(
            std::fs::read(repo.join("modified.rs")).unwrap(),
            b"before\n"
        );
        assert!(!repo.join("added.rs").exists());
        assert_eq!(
            std::fs::read(repo.join("deleted.rs")).unwrap(),
            b"deleted\n"
        );
        assert_eq!(
            std::fs::read(repo.join("docs/task/evidence.md")).unwrap(),
            b"new docs\n"
        );
    }
    #[test]
    fn restore_handles_spaces_non_ascii_and_glob_characters() {
        let f = fixture(1);
        let repo = &f.spec.repos[0].worktree;
        // Include glob syntax and explicit Git pathspec magic. Git can fall
        // back to an existing literal glob name, so glob names alone do not
        // prove that --literal-pathspecs is actually used.
        std::fs::create_dir_all(repo.join("docs/tas[k]")).unwrap();
        write(repo, "docs/tas[k]/*", b"literal glob file");
        write(repo, ":(exclude)modified.rs", b"literal magic file");
        for name in [
            "space name.rs",
            "中文.rs",
            "x[1]*.rs",
            "-option.rs",
            "line\nbreak.rs",
            "docs/task-sibling.rs",
        ] {
            write(repo, name, b"new");
        }
        save(repo);
        let intent = planned(&f);
        let result = apply(&intent).unwrap();
        assert_restored(&result);
        for name in [
            "space name.rs",
            "中文.rs",
            "x[1]*.rs",
            "-option.rs",
            "line\nbreak.rs",
            "docs/task-sibling.rs",
        ] {
            assert!(intent.repos[0].paths.contains(&name.to_owned()));
            assert!(!repo.join(name).exists());
            assert!(git(repo, &["log", "-1", "--format=%B"]).contains(name));
        }
    }
    #[test]
    fn restore_round_trips_binary_and_mode_changes() {
        use std::os::unix::fs::PermissionsExt;
        let f = fixture(1);
        let repo = &f.spec.repos[0].worktree;
        write(repo, "modified.rs", &[0, 255, 128, 1, 0]);
        std::fs::set_permissions(
            repo.join("modified.rs"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        save(repo);
        let intent = planned(&f);
        let result = apply(&intent).unwrap();
        assert_restored(&result);
        git(repo, &["apply", intent.repos[0].patch.to_str().unwrap()]);
        assert_eq!(
            std::fs::read(repo.join("modified.rs")).unwrap(),
            [0, 255, 128, 1, 0]
        );
        assert_ne!(
            std::fs::metadata(repo.join("modified.rs"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
        git(repo, &["add", "-A"]);
        assert_eq!(
            git(repo, &["write-tree"]).trim(),
            crate::git::tree_id(repo, &intent.repos[0].pre_head).unwrap()
        );
    }
    #[test]
    fn restore_patch_reapplies_to_the_session_result() {
        let f = fixture(1);
        let intent = planned(&f);
        let result = apply(&intent).unwrap();
        assert_restored(&result);
        let repo = &intent.repos[0];
        git(&repo.worktree, &["apply", repo.patch.to_str().unwrap()]);
        git(&repo.worktree, &["add", "-A"]);
        assert_eq!(
            git(&repo.worktree, &["write-tree"]).trim(),
            crate::git::tree_id(&repo.worktree, &repo.pre_head).unwrap()
        );
    }
    #[test]
    fn restore_commit_message_names_session_base_paths_and_patch() {
        let f = fixture(1);
        let intent = planned(&f);
        let result = apply(&intent).unwrap();
        assert_restored(&result);
        let repo = &intent.repos[0];
        let msg = git(&repo.worktree, &["log", "-1", "--format=%B"]);
        for text in [
            &intent.session_id,
            &intent.task,
            &repo.base,
            repo.patch.to_str().unwrap(),
        ] {
            assert!(msg.contains(text));
        }
        for p in &repo.paths {
            assert!(msg.contains(p));
        }
    }
    #[test]
    fn restore_without_outside_paths_does_nothing() {
        let mut f = fixture(1);
        f.spec.repos[0].base = crate::git::head_sha(&f.spec.repos[0].worktree).unwrap();
        let head = f.spec.repos[0].base.clone();
        assert!(plan(&f.spec).unwrap().is_none());
        assert!(!f.spec.patch_dir.exists());
        assert_eq!(
            crate::git::head_sha(&f.spec.repos[0].worktree).unwrap(),
            head
        );
        f.spec.repos.clear();
        assert!(plan(&f.spec).unwrap().is_none());
    }
    #[test]
    fn restore_refuses_a_dirty_worktree_and_leaves_it_untouched() {
        let f = fixture(1);
        let intent = planned(&f);
        let repo = &intent.repos[0];
        write(&repo.worktree, "docs/task/evidence.md", b"dirty docs");
        assert!(plan(&f.spec).is_err());
        assert!(apply(&intent).is_err());
        compensate(&intent).unwrap();
        assert_eq!(crate::git::head_sha(&repo.worktree).unwrap(), repo.pre_head);
        assert_eq!(
            std::fs::read(repo.worktree.join("docs/task/evidence.md")).unwrap(),
            b"dirty docs"
        );
    }
    #[test]
    fn restore_patch_dir_blocked_changes_nothing() {
        let f = fixture(1);
        let repo = &f.spec.repos[0].worktree;
        let head = crate::git::head_sha(repo).unwrap();
        std::fs::write(&f.spec.patch_dir, b"blocked").unwrap();
        assert!(plan(&f.spec).is_err());
        assert_eq!(crate::git::head_sha(repo).unwrap(), head);
        assert!(dirty(repo).unwrap().is_empty());

        // Missing patch is a read failure after restore; compensate the index
        // without creating a commit or changing the session's HEAD.
        std::fs::remove_file(&f.spec.patch_dir).unwrap();
        let intent = planned(&f);
        std::fs::remove_file(&intent.repos[0].patch).unwrap();
        assert!(apply(&intent).is_err());
        compensate(&intent).unwrap();
        assert_eq!(crate::git::head_sha(repo).unwrap(), head);
        assert!(dirty(repo).unwrap().is_empty());

        let intent = planned(&f);
        write(repo, "docs/task/evidence.md", b"new concurrent commit");
        save(repo);
        let advanced = crate::git::head_sha(repo).unwrap();
        assert!(apply(&intent).is_err());
        assert!(compensate(&intent).is_err());
        assert_eq!(crate::git::head_sha(repo).unwrap(), advanced);
    }
    #[test]
    fn restore_workspace_writes_one_patch_per_repo() {
        let f = fixture(2);
        let intent = planned(&f);
        assert_eq!(intent.repos.len(), 2);
        assert_ne!(intent.repos[0].patch, intent.repos[1].patch);
        let result = apply(&intent).unwrap();
        assert_restored(&result);
        assert!(
            changed(&result.repos[1].worktree, &result.repos[1].base, "HEAD")
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn restore_workspace_failure_in_one_repo_compensates_the_other() {
        let f = fixture(2);
        let intent = planned(&f);
        // Corrupting the second patch fails after the first checkout committed.
        std::fs::write(&intent.repos[1].patch, b"invalid patch").unwrap();
        assert!(apply(&intent).is_err());
        assert_ne!(
            crate::git::head_sha(&intent.repos[0].worktree).unwrap(),
            intent.repos[0].pre_head
        );
        compensate(&intent).unwrap();
        for repo in &intent.repos {
            assert_eq!(
                crate::git::tree_id(&repo.worktree, "HEAD").unwrap(),
                crate::git::tree_id(&repo.worktree, &repo.pre_head).unwrap()
            );
            git(
                &repo.worktree,
                &["merge-base", "--is-ancestor", &repo.pre_head, "HEAD"],
            );
            assert!(dirty(&repo.worktree).unwrap().is_empty());
        }
        assert_ne!(
            crate::git::head_sha(&intent.repos[0].worktree).unwrap(),
            intent.repos[0].pre_head
        );
    }
    #[test]
    fn restore_apply_resumes_after_interruption_before_commit() {
        let f = fixture(1);
        let intent = planned(&f);
        let repo = &intent.repos[0];
        restore(repo, &repo.base).unwrap();
        let expected = git(&repo.worktree, &["write-tree"]);
        let result = apply(&intent).unwrap();
        assert_restored(&result);
        assert_eq!(
            crate::git::tree_id(&repo.worktree, "HEAD").unwrap(),
            expected.trim()
        );
        let again = apply(&intent).unwrap();
        assert_eq!(result, again);
        assert_eq!(
            git(
                &repo.worktree,
                &["rev-list", "--count", &format!("{}..HEAD", repo.pre_head)]
            )
            .trim(),
            "1"
        );
    }
    #[test]
    fn restore_apply_resumes_when_first_repo_already_committed() {
        let f = fixture(2);
        let intent = planned(&f);
        let mut first = intent.clone();
        first.repos.truncate(1);
        let done = apply(&first).unwrap();
        assert_restored(&done);
        let result = apply(&intent).unwrap();
        assert_restored(&result);
        assert_eq!(result.repos[0].commit, done.repos[0].commit);
        for repo in &intent.repos {
            assert_eq!(
                git(
                    &repo.worktree,
                    &["rev-list", "--count", &format!("{}..HEAD", repo.pre_head)]
                )
                .trim(),
                "1"
            );
        }
    }
}
