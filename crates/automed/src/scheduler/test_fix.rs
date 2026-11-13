//! Shared regression baselines and snapshots, plus test-repair permissions.
use super::*;
use std::collections::BTreeSet;

pub(super) fn enabled(project: &Project, task: &TaskRecord) -> bool {
    std::fs::read_to_string(
        task_docs_root(project, &task.slug)
            .join(task.doc_dir())
            .join("protocol/prompts/fix.md"),
    )
    .is_ok_and(|text| text.contains("<!-- regression-test-defect -->"))
}

pub(crate) fn view(store: &crate::store::Store, task: &str) -> crate::store::Result<Value> {
    Ok(
        json!({"used": store.count_events(task, "bug.test_fix_requested")?,
        "limit": bug_loop::TEST_FIX_LIMIT,
        "open": store.count_events_after(task, "bug.test_fix_requested", "bug.test_fix_settled")? > 0}),
    )
}

fn contents(repo: &Path, rev: &str, path: &str) -> Result<Option<String>> {
    let out = git::run(repo, &["show", &format!("{rev}:{path}")])?;
    Ok(out.ok().then_some(out.stdout))
}

fn changed(repo: &Path, base: &str, head: &str) -> Result<Vec<String>> {
    Ok(git::run_ok(
        repo,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            base,
            head,
            "--",
        ],
    )?
    .stdout
    .split('\0')
    .filter(|p| !p.is_empty())
    .map(str::to_owned)
    .collect())
}

fn regression_base(
    ctx: &Ctx,
    task: &TaskRecord,
    repo: &str,
    path: &Path,
    default: &str,
) -> Result<String> {
    let first = ctx
        .store
        .task_events(&task.id, "session.started")?
        .into_iter()
        .filter_map(|v| v["locate_heads"].as_array().cloned())
        .flatten()
        .find(|v| v["repo"] == repo);
    if let Some(base) = first.as_ref().and_then(|v| v["head"].as_str())
        && git::run(path, &["merge-base", "--is-ancestor", base, "HEAD"])?.ok()
    {
        return Ok(base.into());
    }
    Ok(git::run_ok(path, &["merge-base", "HEAD", default])?
        .stdout
        .trim()
        .into())
}

pub(super) fn validate(doc: &autome_domain::bug_doc::BugDoc, repairing: bool) -> Result<()> {
    if repairing && doc.regression_defect {
        return Err(err("修正回归测试结束时必须删除 回归测试: 有缺陷"));
    }
    if doc.regression_defect {
        let section = doc
            .regression_defect_section
            .as_deref()
            .ok_or_else(|| err("缺少 ## 回归测试缺陷"))?;
        for key in ["用例", "缺陷", "证据", "建议"] {
            if !section.lines().any(|line| {
                line.trim()
                    .strip_prefix("- ")
                    .and_then(|s| s.split_once([':', '：']))
                    .is_some_and(|(k, v)| k.trim() == key && !v.trim().is_empty())
            }) {
                return Err(err(format!("回归测试缺陷缺少非空列表项：{key}")));
            }
        }
    }
    Ok(())
}

/// Validate against the session boundary, then rebuild protections against the task boundary.
pub(super) fn snapshot(
    ctx: &Ctx,
    session: &Session,
    project: &Project,
    task: &TaskRecord,
    doc: &autome_domain::bug_doc::BugDoc,
) -> Result<()> {
    regression_set(ctx, Some(session), project, task, doc, "test_fix")
}

pub(super) fn regression_set(
    ctx: &Ctx,
    session: Option<&Session>,
    project: &Project,
    task: &TaskRecord,
    doc: &autome_domain::bug_doc::BugDoc,
    source: &str,
) -> Result<()> {
    let repairing = source == "test_fix";
    let layout = TaskLayout::of(project, &task.slug, &working_repos(project, task));

    let previous = ctx
        .store
        .last_event(&task.id, "bug.regression_snapshot")?
        .unwrap_or(json!({}));
    let started = match session {
        Some(session) => ctx.store.session_started_event(&task.id, &session.id)?,
        None => None,
    }
    .unwrap_or(json!({}));

    let mut files = Vec::new();
    let mut support = previous["support"].as_array().cloned().unwrap_or_default();
    for slot in &layout.repos {
        let start = started["locate_heads"]
            .as_array()
            .and_then(|heads| heads.iter().find(|h| h["repo"] == slot.name))
            .and_then(|h| h["head"].as_str())
            .unwrap_or("HEAD");
        let base = regression_base(ctx, task, &slot.name, &slot.worktree, &slot.base)?;
        let old_files: Vec<_> = previous["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|f| f["repo"] == slot.name)
            .collect();
        let changes = changed(&slot.worktree, start, "HEAD")?;
        let docs = format!("{}/", task.doc_dir());
        for path in changes.iter().filter(|_| repairing) {
            if path.starts_with(&docs) {
                continue;
            }
            let before = contents(&slot.worktree, start, path)?.unwrap_or_default();
            let now = contents(&slot.worktree, "HEAD", path)?.unwrap_or_default();
            let original = contents(&slot.worktree, &base, path)?;
            let saved = old_files.iter().any(|f| {
                f["path"] == *path && f.get("blocks").is_none() && f.get("section").is_none()
            });
            let supported = support
                .iter()
                .any(|f| f["repo"] == slot.name && f["path"] == *path);
            let new_test = original.is_none()
                && regression_protection(path, &now, "", &doc.repro_command)
                    == Some(Protection::File);
            let strip = |s: &str| s[..test_section_start(s).unwrap_or(s.len())].to_owned();
            let inline = regression_protection(path, &now, &before, "").is_some()
                && strip(&before) == strip(&now);
            if !(saved || supported || new_test || inline) {
                return Err(err(format!(
                    "定位轮在『修正回归测试』模式下只能改回归测试和任务文档，本轮改了：{path}。产品修复已经在分支上，不要改它；只修正回归测试后 autome rerun <任务> locate。"
                )));
            }
        }
        let mut candidates: BTreeSet<String> = old_files
            .iter()
            .filter_map(|f| f["path"].as_str().map(str::to_owned))
            .collect();
        for path in changed(&slot.worktree, &base, "HEAD")? {
            if path.starts_with(&docs) {
                continue;
            }
            let original = contents(&slot.worktree, &base, &path)?;
            let now = contents(&slot.worktree, "HEAD", &path)?.unwrap_or_default();
            // A pre-existing product entry point does not become a test just because the command names it.
            let command = if original.is_none() {
                doc.repro_command.as_str()
            } else {
                ""
            };
            if regression_protection(
                &path,
                &now,
                original.as_deref().unwrap_or_default(),
                command,
            )
            .is_some()
            {
                candidates.insert(path);
            }
        }
        // Session-local classification still recognizes edited command scripts.
        for path in &changes {
            if !path.starts_with(&docs) {
                let before = contents(&slot.worktree, start, path)?.unwrap_or_default();
                let now = contents(&slot.worktree, "HEAD", path)?.unwrap_or_default();
                if regression_protection(path, &now, &before, &doc.repro_command).is_some() {
                    candidates.insert(path.clone());
                }
            }
        }
        for path in candidates {
            let original = contents(&slot.worktree, &base, &path)?.unwrap_or_default();
            let now = contents(&slot.worktree, "HEAD", &path)?.unwrap_or_default();
            let mut protection = old_files
                .iter()
                .find(|f| f["path"] == path && f["protect"] == "file")
                .map(|_| Protection::File)
                .or_else(|| {
                    regression_protection(
                        &path,
                        &now,
                        &original,
                        if original.is_empty() {
                            &doc.repro_command
                        } else {
                            ""
                        },
                    )
                })
                .unwrap_or(Protection::File);
            let head = git::head_sha(&slot.worktree)?;
            let tree = git::run_ok(&slot.worktree, &["ls-tree", "-z", "HEAD", "--", &path])?.stdout;
            let mut file = json!({"repo":slot.name,"path":path,"head":head,"tree":tree,"protect":protection.as_str()});
            if protection != Protection::File {
                let start = if protection == Protection::Section {
                    test_section_start(&now).unwrap_or(0)
                } else {
                    0
                };
                if repairing {
                    file["blocks"] = json!([&now[start..]]);
                } else {
                    let diff = git::run_ok(
                        &slot.worktree,
                        &[
                            "diff",
                            "-U0",
                            "--no-color",
                            "--no-ext-diff",
                            &base,
                            "HEAD",
                            "--",
                            &path,
                        ],
                    )?;
                    let blocks =
                        added_test_blocks(&diff.stdout, now[..start].matches('\n').count() + 1);
                    if blocks.is_empty() && protection == Protection::Blocks {
                        protection = Protection::File;
                        file["protect"] = json!(protection.as_str());
                    } else {
                        file["blocks"] = json!(blocks);
                    }
                }
            }
            files.push(file);
        }
        for path in changes {
            if !path.starts_with(&docs)
                && !files
                    .iter()
                    .any(|f| f["repo"] == slot.name && f["path"] == path)
                && !support
                    .iter()
                    .any(|f| f["repo"] == slot.name && f["path"] == path)
            {
                support.push(json!({"repo":slot.name,"path":path}));
            }
        }
    }
    if files.is_empty() {
        if source == "locate" {
            ctx.store.append_event(
                "bug.regression_empty",
                &task.id,
                json!({"session_id": session.map(|s| &s.id)}),
            )?;
            return Ok(());
        }
        return Err(err("回归测试保护集为空：定位轮必须新增或修改测试文件"));
    }
    ctx.store.append_event(
        "bug.regression_snapshot",
        &task.id,
        json!({"session_id":session.map(|s| &s.id),"source":source,"files":files,"support":support}),
    )?;
    Ok(())
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn base_dir(project: &Project, task: &TaskRecord) -> PathBuf {
    Path::new(&project.path)
        .join(".autome/output/repro-base")
        .join(&task.id)
}

pub(crate) fn cleanup(project: &Project, task: &TaskRecord, layout: &TaskLayout) -> Result<()> {
    let root = base_dir(project, task);
    if root.exists() {
        for entry in std::fs::read_dir(&root).map_err(|e| err(e.to_string()))? {
            let session = entry.map_err(|e| err(e.to_string()))?.path();
            for slot in &layout.repos {
                let path = if project.is_workspace() {
                    session.join(&slot.name)
                } else {
                    session.clone()
                };
                if path.exists() {
                    git::worktree_remove(&slot.repo_root, &path.to_string_lossy(), true)?;
                }
                git::worktree_prune(&slot.repo_root)?;
            }
        }
    }
    if root.exists() {
        std::fs::remove_dir_all(root).map_err(|e| err(e.to_string()))?;
    }
    Ok(())
}

fn overlay(repo: &Path, target: &Path, files: &[Value]) -> Result<()> {
    for file in files {
        let path = file["path"].as_str().ok_or_else(|| err("缺少测试路径"))?;
        let destination = target.join(path);
        let head = git::run(repo, &["ls-tree", "-z", "HEAD", "--", path])?;
        if head.stdout.is_empty() {
            if destination.exists() {
                std::fs::remove_file(&destination).map_err(|e| err(e.to_string()))?;
            }
            continue;
        }
        if file["protect"] == "section" || file["protect"] == "blocks" {
            let now =
                contents(repo, "HEAD", path)?.ok_or_else(|| err(format!("无法读取 {path}")))?;
            let base = std::fs::read_to_string(&destination)
                .map_err(|e| err(format!("基线无法构造 {path}: {e}")))?;
            let start = test_section_start(&now)
                .ok_or_else(|| err(format!("基线无法构造：识别不出测试段 {path}")))?;
            let prefix = &base[..test_section_start(&base).unwrap_or(base.len())];
            std::fs::write(destination, format!("{prefix}{}", &now[start..]))
                .map_err(|e| err(e.to_string()))?;
        } else {
            use std::os::unix::fs::PermissionsExt;
            let blob = std::process::Command::new("git")
                .current_dir(repo)
                .args(["show", &format!("HEAD:{path}")])
                .output()
                .map_err(|e| err(e.to_string()))?;
            if !blob.status.success() {
                return Err(err(format!("无法读取测试文件 {path}")));
            }
            std::fs::create_dir_all(destination.parent().unwrap())
                .map_err(|e| err(e.to_string()))?;
            if destination.symlink_metadata().is_ok() {
                std::fs::remove_file(&destination).map_err(|e| err(e.to_string()))?;
            }
            if head.stdout.starts_with("120000") {
                let link = std::str::from_utf8(&blob.stdout).map_err(|e| err(e.to_string()))?;
                std::os::unix::fs::symlink(link, &destination).map_err(|e| err(e.to_string()))?;
            } else {
                std::fs::write(&destination, blob.stdout).map_err(|e| err(e.to_string()))?;
                let mode = if head.stdout.starts_with("100755") {
                    0o755
                } else {
                    0o644
                };
                std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(mode))
                    .map_err(|e| err(e.to_string()))?;
            }
        }
    }
    Ok(())
}

fn dependencies(head: &Path, base: &Path, command: &str) -> Result<()> {
    for token in command
        .split(|c: char| c.is_whitespace() || "\"';&|()".contains(c))
        .filter(|t| t.contains('/') || t.contains('.'))
    {
        if head.join(token).is_file() && !base.join(token).is_file() {
            return Err(err(format!("基线无法构造：依赖缺失 {token}")));
        }
    }
    Ok(())
}

pub(super) fn prepare(
    ctx: &Ctx,
    project: &Project,
    task: &TaskRecord,
    layout: &TaskLayout,
    command: &str,
    job: &crate::gate::Job,
) -> Result<String> {
    cleanup(project, task, layout)?;
    let snapshot = ctx
        .store
        .last_event(&task.id, "bug.regression_snapshot")?
        .ok_or_else(|| err("缺少回归测试快照"))?;
    let root = base_dir(project, task).join(snapshot["session_id"].as_str().unwrap_or("current"));
    let built = (|| -> Result<()> {
        for slot in &layout.repos {
            let base = git::run_ok(&slot.worktree, &["merge-base", "HEAD", &slot.base])?;
            let target = if project.is_workspace() {
                root.join(&slot.name)
            } else {
                root.clone()
            };
            std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| err(e.to_string()))?;
            git::worktree_add_detached(
                &slot.repo_root,
                &target.to_string_lossy(),
                base.stdout.trim(),
            )?;
            let files: Vec<_> = ["files", "support"]
                .into_iter()
                .flat_map(|key| snapshot[key].as_array().into_iter().flatten())
                .filter(|f| f["repo"] == slot.name)
                .cloned()
                .collect();
            overlay(&slot.worktree, &target, &files)?;
        }
        dependencies(&layout.cwd, &root, command)?;
        Ok(())
    })();
    let pair_file = job.log.with_extension("pair");
    let _ = std::fs::remove_file(&pair_file);
    let base_command = match built {
        Ok(()) => format!(
            "(cd {} && sh -c {})",
            quote(&root.to_string_lossy()),
            quote(command)
        ),
        Err(e) => {
            cleanup(project, task, layout)?;
            format!(
                "printf '%s\\n' {}; (exit 127)",
                quote(&format!("基线无法构造：{e}"))
            )
        }
    };
    Ok(format!(
        "{base_command}; base_code=$?; printf '%s\\n' \"$base_code\" > {}; sh -c {}; head_code=$?; printf '%s\\n' \"$head_code\" >> {}; exit \"$head_code\"",
        quote(&pair_file.to_string_lossy()),
        quote(command),
        quote(&pair_file.to_string_lossy())
    ))
}

pub(super) fn pair(job: &crate::gate::Job, exit: i32) -> bug_loop::ReproPair {
    let text = std::fs::read_to_string(job.log.with_extension("pair")).unwrap_or_default();
    let mut codes = text.lines().filter_map(|s| s.parse::<i32>().ok());
    let base = codes.next();
    let head = codes.next();
    bug_loop::ReproPair {
        base_exit: base.unwrap_or(exit),
        base_timed_out: base.is_none() || base == Some(crate::gate::TIMED_OUT),
        head_exit: head.unwrap_or(exit),
        head_timed_out: head.is_none() || head == Some(crate::gate::TIMED_OUT),
    }
}
