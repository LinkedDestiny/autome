//! Facts for reviewing implementation rounds outside the milestone table.
use std::collections::BTreeSet;
use std::path::Path;

use autome_domain::role::Role;
use autome_domain::session::SessionKind;
use autome_domain::status_block::{MilestoneState, StatusBlock};
use autome_domain::task::Inject;
use serde_json::{Value, json};

use crate::{git, guards, layout::TaskLayout, store::Store};

type Result<T> = std::result::Result<T, String>;

fn read(repo: &Path, args: &[&str]) -> Result<String> {
    git::run_ok(repo, args)
        .map(|o| o.stdout)
        .map_err(|e| format!("集成范围读取失败（{}）：{e}", args[0]))
}

/// First implementation attempt since the last successfully read task head.
/// Missing start_heads belongs to an old binary; never guess its baseline.
fn first_attempt(store: &Store, task: &str) -> Result<Option<Value>> {
    let mut seq = store
        .last_event_of(task, &["task.head"])
        .map_err(|e| e.to_string())?
        .map_or(0, |e| e.0);
    loop {
        let events = store
            .events_since_with_subject(seq, 256)
            .map_err(|e| e.to_string())?;
        if events.is_empty() {
            return Ok(None);
        }
        for (at, kind, subject, value) in events {
            seq = at;
            if subject == task
                && kind == "session.started"
                && serde_json::from_value::<SessionKind>(value["kind"].clone()).ok()
                    == Some(SessionKind::Role { role: Role::Impl })
            {
                return Ok(Some(value));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn range(
    store: &Store,
    task: &str,
    kind: SessionKind,
    before: Option<&guards::Snapshot>,
    status: &StatusBlock,
    layout: &TaskLayout,
    doc_dir: &str,
    inject: Option<&Inject>,
) -> Result<Option<Value>> {
    if kind != (SessionKind::Role { role: Role::Impl })
        || !before.is_some_and(|b| {
            !b.milestones.is_empty() && b.milestones.iter().all(|(_, s)| *s == MilestoneState::Done)
        })
        || status.milestones.is_empty()
        || !status
            .milestones
            .iter()
            .all(|m| m.state == MilestoneState::Done)
    {
        return Ok(None);
    }
    let Some(attempt) = first_attempt(store, task)? else {
        return Ok(None);
    };
    let Some(heads) = attempt["start_heads"].as_array() else {
        return Ok(None);
    };
    let conflicts = match inject {
        Some(Inject::RebaseConflict { files }) => files.as_slice(),
        _ => &[],
    };
    let mut repos = Vec::new();
    for slot in &layout.repos {
        let start = heads
            .iter()
            .find(|v| v["repo"].as_str() == Some(&slot.name))
            .and_then(|v| v["head"].as_str())
            .ok_or_else(|| format!("集成范围读取失败：仓库 {} 缺少轮前 HEAD", slot.name))?;
        let repo = repo_range(slot, start, doc_dir, conflicts, &stat)?;
        if !repo["commits"].as_array().unwrap().is_empty()
            || !repo["dropped"].as_array().unwrap().is_empty()
        {
            repos.push(repo);
        }
    }
    if repos.is_empty() {
        return Ok(None);
    }
    let trigger = match inject {
        Some(Inject::RebaseConflict { files }) => json!({"kind":"rebase_conflict", "files":files}),
        Some(Inject::GateFailed { command, tail, .. }) => {
            json!({"kind":"gate_failed", "command":command, "tail":tail})
        }
        Some(Inject::Decisions) => json!({"kind":"decisions"}),
        _ => json!({"kind":"none"}),
    };
    Ok(Some(
        json!({"session_id": attempt["session_id"], "impl_round":status.impl_round,
        "evidence":format!("{}-r{}-audit.md", status.milestones.last().unwrap().id, status.impl_round),
        "trigger":trigger, "repos":repos}),
    ))
}

fn paths(repo: &Path, sha: &str) -> Result<Vec<String>> {
    // diff-tree --root also handles a rewritten root commit; merges use parent 1.
    let parents = read(repo, &["rev-list", "--parents", "-n", "1", sha])?;
    let parent = parents.split_whitespace().nth(1);
    let out = if let Some(parent) = parent {
        read(
            repo,
            &["diff", "--name-only", "--no-renames", "-z", parent, sha],
        )?
    } else {
        read(
            repo,
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "-r",
                "--name-only",
                "--no-renames",
                "-z",
                sha,
            ],
        )?
    };
    Ok(out
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect())
}

fn stat(repo: &Path, sha: &str, exclude: Option<&str>) -> Result<String> {
    let exclude = format!(
        ":(exclude){}/",
        exclude.unwrap_or_default().trim_end_matches('/')
    );
    let mut args = vec!["show", "--stat=120", "--format=", sha, "--", "."];
    if exclude != ":(exclude)/" {
        args.push(&exclude);
    }
    read(repo, &args)
}

fn test_file(repo: &Path, path: &str, sha: &str, end: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    if path.split('/').any(|p| {
        matches!(
            p,
            "tests" | "test" | "__tests__" | "golden" | "fixtures" | "evals"
        )
    }) || name.contains(".test.")
        || name.contains(".spec.")
        || name.contains("_test.")
        || name.starts_with("test_")
    {
        return true;
    }
    let diff = read(
        repo,
        &["show", "--format=", "--first-parent", sha, "--", path],
    );
    if diff.as_ref().map_or(true, |s| {
        s.lines().any(|l| {
            (l.starts_with('+') || l.starts_with('-'))
                && !l.starts_with("+++")
                && !l.starts_with("---")
                && ["#[test]", "#[cfg(test)]", "assert", "expect("]
                    .iter()
                    .any(|m| l.contains(m))
        })
    }) {
        return true;
    }
    [format!("{end}:{path}"), format!("{sha}^:{path}")]
        .iter()
        .any(|rev| {
            read(repo, &["show", rev]).map_or(true, |s| {
                s.contains('\0')
                    || s.contains('\u{fffd}')
                    || s.contains("#[cfg(test)]")
                    || s.contains("#[test]")
            })
        })
}

fn repo_range(
    slot: &crate::layout::RepoSlot,
    start: &str,
    doc_dir: &str,
    conflicts: &[String],
    read_stat: &impl Fn(&Path, &str, Option<&str>) -> Result<String>,
) -> Result<Value> {
    let repo = &slot.worktree;
    let end = read(repo, &["rev-parse", "HEAD"])?.trim().to_owned();
    let base = read(repo, &["merge-base", &end, &slot.base])?
        .trim()
        .to_owned();
    let symmetric = format!("{start}...{end}");
    let exclusion = format!("^{base}");
    let listed = read(
        repo,
        &[
            "rev-list",
            "--reverse",
            "--cherry-pick",
            "--right-only",
            &symmetric,
            &exclusion,
        ],
    )?;
    let dropped = read(
        repo,
        &[
            "rev-list",
            "--reverse",
            "--cherry-pick",
            "--left-only",
            &symmetric,
            &exclusion,
        ],
    )?;
    let mut listed: BTreeSet<String> = listed.lines().map(str::to_owned).collect();
    if !conflicts.is_empty()
        && !git::is_ancestor(repo, start, &end).map_err(|e| format!("集成范围读取失败：{e}"))?
    {
        let candidates = read(
            repo,
            &[
                "rev-list",
                "--reverse",
                &format!("{base}..{end}"),
                &format!("^{start}"),
            ],
        )?;
        for sha in candidates.lines() {
            if paths(repo, sha)?.iter().any(|p| {
                conflicts
                    .iter()
                    .any(|f| f == p || f == &format!("{}/{p}", slot.name))
            }) {
                listed.insert(sha.to_owned());
            }
        }
    }
    // Preserve chronological Git order after the conservative union.
    let order = read(repo, &["rev-list", "--reverse", &format!("{base}..{end}")])?;
    let prefix = format!("{}/", doc_dir.trim_end_matches('/'));
    let mut tests = BTreeSet::new();
    let mut collect = |shas: Vec<&str>, scan: bool| -> Result<(Vec<Value>, usize)> {
        let mut commits = Vec::new();
        for sha in shas {
            let paths: Vec<_> = paths(repo, sha)?
                .into_iter()
                .filter(|p| !slot.is_docs || !p.starts_with(&prefix))
                .collect();
            if paths.is_empty() {
                continue;
            }
            if scan {
                for p in &paths {
                    if test_file(repo, p, sha, &end) {
                        tests.insert(if slot.name.is_empty() {
                            p.clone()
                        } else {
                            format!("{}/{p}", slot.name)
                        });
                    }
                }
            }
            let subject = read(repo, &["show", "-s", "--format=%s", sha])
                .unwrap_or_else(|e| format!("（读取失败：{e}）"));
            let raw = read_stat(repo, sha, slot.is_docs.then_some(doc_dir))
                .unwrap_or_else(|e| format!("（读取失败：{e}）"));
            let mut stat = raw.lines().take(30).collect::<Vec<_>>().join("\n");
            if raw.lines().count() > 30 {
                stat.push_str(&format!(
                    "\n其余 {} 行见 git show --stat {sha}",
                    raw.lines().count() - 30
                ));
            }
            commits.push(json!({"sha":sha, "subject":subject.trim(), "paths":paths, "stat":stat}));
        }
        let omitted = commits.len().saturating_sub(50);
        commits.truncate(50);
        Ok((commits, omitted))
    };
    let (commits, commits_omitted) = collect(
        order.lines().filter(|s| listed.contains(*s)).collect(),
        true,
    )?;
    let (dropped, dropped_omitted) = collect(dropped.lines().collect(), false)?;
    Ok(
        json!({"repo":slot.name, "start":start,"end":end,"base":base,"commits":commits,
        "dropped":dropped,"test_files":tests,"commits_omitted":commits_omitted,"dropped_omitted":dropped_omitted}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::RepoSlot;
    use autome_domain::status_block;
    struct TempDir(std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct World {
        _dir: TempDir,
        slot: RepoSlot,
        store: Store,
        start: String,
    }
    impl World {
        fn new() -> Self {
            let dir = TempDir(
                std::env::temp_dir().join(format!("automed-integration-{}", uuid::Uuid::new_v4())),
            );
            std::fs::create_dir_all(&dir.0).unwrap();
            let p = dir.0.clone();
            git::run_ok(&p, &["init", "-b", "main"]).unwrap();
            let mut w = Self {
                _dir: dir,
                slot: RepoSlot {
                    name: String::new(),
                    repo_root: p.clone(),
                    worktree: p,
                    branch: "topic".into(),
                    base: "main".into(),
                    is_docs: true,
                },
                store: Store::open_in_memory().unwrap(),
                start: String::new(),
            };
            w.commit("product", "base\n");
            w.g(&["checkout", "-b", "topic"]);
            w.start = w.head();
            w
        }
        fn g(&self, args: &[&str]) -> String {
            read(&self.slot.worktree, args).unwrap().trim().to_owned()
        }
        fn head(&self) -> String {
            self.g(&["rev-parse", "HEAD"])
        }
        fn commit(&self, path: &str, content: &str) -> String {
            let path = self.slot.worktree.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
            self.g(&["add", "."]);
            self.g(&[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "commit",
                "-m",
                "change",
            ]);
            self.head()
        }
        fn started(&self, id: &str, head: &str) {
            self.store.append_event("session.started","T",json!({"session_id":id,"kind":SessionKind::Role{role:Role::Impl},"start_heads":[{"repo":self.slot.name,"head":head}]})).unwrap();
        }
        fn repo(&self, conflicts: &[String]) -> Value {
            repo_range(&self.slot, &self.start, "docs/task", conflicts, &stat).unwrap()
        }
        fn event(
            &self,
            before: &guards::Snapshot,
            after: &StatusBlock,
            kind: SessionKind,
            inject: Option<&Inject>,
        ) -> Result<Option<Value>> {
            range(
                &self.store,
                "T",
                kind,
                Some(before),
                after,
                &TaskLayout {
                    cwd: self.slot.worktree.clone(),
                    docs_root: self.slot.worktree.clone(),
                    repos: vec![self.slot.clone()],
                },
                "docs/task",
                inject,
            )
        }
        fn integration(&self) -> Option<Value> {
            self.event(
                &before(),
                &status(),
                SessionKind::Role { role: Role::Impl },
                None,
            )
            .unwrap()
        }
        fn diverge(&mut self) -> String {
            let old = self.commit("product", "topic\n");
            self.start = self.head();
            self.g(&["checkout", "main"]);
            self.commit("product", "upstream\n");
            self.g(&["checkout", "topic"]);
            old
        }
        fn resolve(&self, content: &str) {
            assert!(
                !git::run(&self.slot.worktree, &["rebase", "main"])
                    .unwrap()
                    .ok()
            );
            std::fs::write(self.slot.worktree.join("product"), content).unwrap();
            self.g(&["add", "product"]);
            self.g(&[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.test",
                "-c",
                "core.editor=true",
                "rebase",
                "--continue",
            ]);
        }
    }
    fn status() -> StatusBlock {
        status_block::parse("# Test\n\nstatus: 实现中\ndesign-round: 1/5\nimplementation-round: 9/25\ncurrent-milestone: M-01\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n| M-01 | 已完成 | Test | 0 | |\n").unwrap()
    }
    fn before() -> guards::Snapshot {
        guards::Snapshot {
            milestones: vec![("M-01".into(), MilestoneState::Done)],
            ..Default::default()
        }
    }
    fn shas(v: &Value, key: &str) -> Vec<String> {
        v[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["sha"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn integration_range_linear_repair_lists_the_fix_commit() {
        let w = World::new();
        let fix = w.commit("product", "fix\n");
        let r = w.repo(&[]);
        assert_eq!(shas(&r, "commits"), vec![fix]);
        assert!(shas(&r, "dropped").is_empty());
        for n in 1..51 {
            w.commit("product", &format!("fix {n}\n"));
        }
        let bounded = w.repo(&[]);
        assert_eq!(bounded["commits"].as_array().unwrap().len(), 50);
        assert_eq!(bounded["commits_omitted"], 1);
    }
    #[test]
    fn integration_range_rebase_lists_only_rewritten_and_new_commits() {
        let mut w = World::new();
        w.commit("clean", "clean\n");
        let old = w.diverge();
        w.resolve("resolved\n");
        let rewritten = w.head();
        let new = w.commit("extra", "new\n");
        let r = w.repo(&["product".into()]);
        assert_eq!(shas(&r, "commits"), vec![rewritten, new]);
        assert_eq!(shas(&r, "dropped"), vec![old]);
    }
    #[test]
    fn integration_range_clean_rebase_is_empty() {
        let mut w = World::new();
        w.commit("clean", "topic\n");
        w.start = w.head();
        w.g(&["checkout", "main"]);
        w.commit("other", "upstream\n");
        w.g(&["checkout", "topic"]);
        w.g(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.test",
            "rebase",
            "main",
        ]);
        let r = w.repo(&[]);
        assert!(shas(&r, "commits").is_empty());
        assert!(shas(&r, "dropped").is_empty());
    }
    #[test]
    fn integration_range_doc_only_commits_are_not_integration() {
        let w = World::new();
        w.started("first", &w.start);
        w.commit("docs/task/evidence/a.md", "evidence");
        assert!(w.integration().is_none());
        w.commit("docs/task-other/file", "outside");
        assert!(w.integration().is_some());
    }
    #[test]
    fn integration_range_counts_the_sweep_commit_and_skips_the_brief_commit() {
        let mut w = World::new();
        w.commit("docs/task/brief/impl.md", "brief");
        w.start = w.head();
        w.started("first", &w.start);
        let fix = w.commit("product", "swept\n");
        let e = w.integration().unwrap();
        assert_eq!(shas(&e["repos"][0], "commits"), vec![fix]);
        assert_eq!(e["evidence"], "M-01-r9-audit.md");
    }
    #[test]
    fn integration_range_a_crashed_attempt_keeps_its_commits() {
        let w = World::new();
        w.started("first", &w.start);
        let a = w.commit("product", "crashed\n");
        w.started("retry", &w.head());
        let b = w.commit("extra", "retry\n");
        let e = w.integration().unwrap();
        assert_eq!(e["session_id"], "first");
        assert_eq!(shas(&e["repos"][0], "commits"), vec![a, b]);
        w.store
            .append_event("task.head", "T", json!({"sha":w.head()}))
            .unwrap();
        w.started("next", &w.head());
        assert!(w.integration().is_none());
    }
    #[test]
    fn integration_range_test_files_by_path_and_by_content() {
        let mut w = World::new();
        let names = [
            "tests/a",
            "test/a",
            "__tests__/a",
            "golden/a",
            "fixtures/a",
            "evals/a",
            "x.test.js",
            "x.spec.js",
            "x_test.rs",
            "test_x",
            "inline.rs",
            "plain.rs",
        ];
        for p in names {
            w.commit(p, "before\n");
        }
        w.start = w.head();
        for p in names {
            w.commit(
                p,
                if p == "inline.rs" {
                    "assert!(true);\n"
                } else {
                    "after\n"
                },
            );
        }
        let r = w.repo(&[]);
        let files = r["test_files"].as_array().unwrap();
        assert_eq!(files.len(), 11);
        assert!(!files.contains(&json!("plain.rs")));
    }
    #[test]
    fn integration_range_workspace_member_paths_carry_the_repo_name() {
        let mut w = World::new();
        w.slot.name = "api".into();
        w.slot.is_docs = false;
        w.started("first", &w.start);
        w.commit("docs/task/tests/a", "member\n");
        let e = w.integration().unwrap();
        assert_eq!(e["repos"][0]["repo"], "api");
        assert_eq!(
            e["repos"][0]["test_files"],
            json!(["api/docs/task/tests/a"])
        );
    }
    #[test]
    fn integration_range_is_recorded_only_for_an_all_done_table_that_stays_done() {
        let w = World::new();
        w.started("first", &w.start);
        w.commit("product", "fix\n");
        let mut b = before();
        let mut s = status();
        let kind = SessionKind::Role { role: Role::Impl };
        assert!(w.integration().is_some());
        b.milestones[0].1 = MilestoneState::Pending;
        assert!(w.event(&b, &s, kind, None).unwrap().is_none());
        b = before();
        s.milestones[0].state = MilestoneState::Open;
        assert!(w.event(&b, &s, kind, None).unwrap().is_none());
        b.milestones.clear();
        assert!(w.event(&b, &status(), kind, None).unwrap().is_none());
    }
    #[test]
    fn integration_range_records_the_trigger_from_the_carried_inject() {
        let w = World::new();
        w.started("first", &w.start);
        w.commit("product", "fix\n");
        for (inject, expected) in [
            (
                Some(Inject::RebaseConflict {
                    files: vec!["product".into()],
                }),
                json!({"kind":"rebase_conflict","files":["product"]}),
            ),
            (
                Some(Inject::GateFailed {
                    head: "secret-sha".into(),
                    command: "check".into(),
                    tail: "failed".into(),
                }),
                json!({"kind":"gate_failed","command":"check","tail":"failed"}),
            ),
            (None, json!({"kind":"none"})),
            (Some(Inject::Decisions), json!({"kind":"decisions"})),
        ] {
            let e = w
                .event(
                    &before(),
                    &status(),
                    SessionKind::Role { role: Role::Impl },
                    inject.as_ref(),
                )
                .unwrap()
                .unwrap();
            assert_eq!(e["trigger"], expected);
        }
    }
    #[test]
    fn integration_range_bug_loop_records_nothing() {
        let w = World::new();
        w.started("first", &w.start);
        w.commit("product", "fix\n");
        assert!(
            w.event(
                &before(),
                &status(),
                SessionKind::Bug {
                    step: autome_domain::session::BugStep::Fix
                },
                None
            )
            .unwrap()
            .is_none()
        );
    }
    #[test]
    fn integration_range_conflict_that_drops_the_product_commit_is_still_integration() {
        let mut w = World::new();
        let old = w.diverge();
        w.started("first", &w.start);
        assert!(
            !git::run(&w.slot.worktree, &["rebase", "main"])
                .unwrap()
                .ok()
        );
        w.g(&["rebase", "--skip"]);
        let e = w.integration().unwrap();
        let r = &e["repos"][0];
        assert!(shas(r, "commits").is_empty());
        assert_eq!(shas(r, "dropped"), vec![old]);
        assert_eq!(r["dropped"][0]["paths"], json!(["product"]));
        assert!(
            r["dropped"][0]["stat"]
                .as_str()
                .unwrap()
                .contains("product")
        );
    }
    #[test]
    fn integration_range_conflict_file_commit_with_an_equivalent_patch_id_is_still_listed() {
        let mut w = World::new();
        w.commit("product", "\"hello world\"\n");
        w.start = w.head();
        w.g(&["checkout", "main"]);
        w.commit("product", "b a s e\n");
        w.g(&["checkout", "topic"]);
        w.resolve("\"helloworld\"\n");
        // Both removed and added lines differ only in whitespace. Git's
        // equivalence filter really hides this string-literal change.
        assert!(shas(&w.repo(&[]), "commits").is_empty());
        let r = w.repo(&["product".into()]);
        assert_eq!(shas(&r, "commits"), vec![w.head()]);
    }
    #[test]
    fn integration_range_inline_test_module_with_only_setup_changed_is_listed() {
        let mut w = World::new();
        w.commit(
            "source.rs",
            "#[cfg(test)]\nmod tests { const REUSE: bool = true; }\n",
        );
        w.start = w.head();
        w.commit(
            "source.rs",
            "#[cfg(test)]\nmod tests { const REUSE: bool = false; }\n",
        );
        assert_eq!(w.repo(&[])["test_files"], json!(["source.rs"]));
    }
    #[test]
    fn integration_range_unreadable_range_fails_closed() {
        let mut w = World::new();
        w.start = "not-a-commit".into();
        w.started("first", &w.start);
        w.commit("product", "fix\n");
        let e = w
            .event(
                &before(),
                &status(),
                SessionKind::Role { role: Role::Impl },
                None,
            )
            .unwrap_err();
        assert!(e.contains("集成范围读取失败"));
        assert!(
            w.store
                .last_event("T", "integration.range")
                .unwrap()
                .is_none()
        );
        let old = World::new();
        old.store
            .append_event(
                "session.started",
                "T",
                json!({"session_id":"old","kind":SessionKind::Role{role:Role::Impl}}),
            )
            .unwrap();
        old.commit("product", "fix\n");
        assert!(old.integration().is_none());
    }
    #[test]
    fn integration_range_a_stat_failure_keeps_the_range() {
        let w = World::new();
        let sha = w.commit("product", "fix\n");
        let bounded = repo_range(&w.slot, &w.start, "docs/task", &[], &|_, _, _| {
            Ok("line\n".repeat(31))
        })
        .unwrap();
        assert!(
            bounded["commits"][0]["stat"]
                .as_str()
                .unwrap()
                .ends_with(&format!("其余 1 行见 git show --stat {sha}"))
        );
        assert!(test_file(&w.slot.worktree, "missing", &sha, &w.head()));

        let r = repo_range(&w.slot, &w.start, "docs/task", &[], &|_, _, _| {
            Err("stat unavailable".into())
        })
        .unwrap();
        assert_eq!(shas(&r, "commits"), vec![sha]);
        assert!(
            r["commits"][0]["stat"]
                .as_str()
                .unwrap()
                .contains("读取失败")
        );
    }
}
