//! The IPC surface for the protocol repository and the version page.
//!
//! Kept out of `dispatch` because it is a self-contained group — versions,
//! their changelog, what each one cost, and the one setting a project has
//! about them — and `dispatch` is already the longest file in the crate.

use autome_domain::changelog;
use autome_domain::metrics::TaskMetrics;
use serde_json::{Value, json};

use crate::dispatch::{Ctx, DispatchResult, rejected};
use crate::{config_io, protocol, version_page};

/// `protocol.get` — the versions on this machine and what is in the current
/// one.
pub fn get(ctx: &mut Ctx) -> DispatchResult {
    // A read never creates the repository; see `protocol::open`.
    let Some(repo) = protocol::open(&ctx.autome_home) else {
        return Ok((
            json!({
                "initialised": false,
                "path": protocol::repo_path(&ctx.autome_home).to_string_lossy(),
            }),
            vec![],
        ));
    };
    let tags = repo.tags()?;
    let (current, files) = repo.resolve(None)?;
    let log = files
        .get("CHANGELOG.md")
        .map(changelog::parse)
        .transpose()
        .map_err(|e| rejected(format!("CHANGELOG.md {e}")))?;

    // A version whose files no longer satisfy the kernel contract cannot be
    // run, and saying so here is cheaper than finding out at the next session.
    let breaches = autome_domain::protocol::verify_contract(protocol::expected_contract(), &files);

    Ok((
        json!({
            "initialised": true,
            "path": repo.path.to_string_lossy(),
            "tags": tags,
            "current": { "tag": current.tag, "hash": current.hash, "wire": current.to_wire() },
            "files": files.paths().collect::<Vec<_>>(),
            "bytes": files.sized_bytes(),
            "byte_budget": autome_domain::protocol::SIZE_BUDGET_BYTES,
            "contract_breaches": breaches.iter().map(|b| b.to_string()).collect::<Vec<_>>(),
            "changelog": log,
            // Uncommitted edits in the protocol repository. They do not affect
            // any task — a task reads its own frozen copy — but the user has
            // to be able to see that the working tree and the newest tag have
            // drifted apart, or "I changed the protocol and nothing happened"
            // has no explanation.
            "working_tree_differs": repo.working_files()?.hash() != current.hash,
        }),
        vec![],
    ))
}

/// `protocol.eval` — layers 1 and 2 over the protocol repository's working
/// tree.
///
/// The working tree rather than the newest tag, because the question the user
/// is asking when they press this is "would what I have here be accepted",
/// and what they have here is usually uncommitted.
pub fn eval(ctx: &mut Ctx) -> DispatchResult {
    let Some(repo) = protocol::open(&ctx.autome_home) else {
        return Ok((json!({ "initialised": false }), vec![]));
    };
    let files = repo.working_files()?;
    let report = crate::protocol::eval::check(&files);
    Ok((
        json!({
            "initialised": true,
            "passed": report.passed,
            "failures": report.fails().map(|p| json!({
                "layer": p.layer,
                "detail": p.detail,
            })).collect::<Vec<_>>(),
            "warnings": report.warnings().map(|p| json!({
                "layer": p.layer,
                "detail": p.detail,
            })).collect::<Vec<_>>(),
            "ok": !report.failed(),
        }),
        vec![],
    ))
}

/// `protocol.versions` — the version page's table for one project.
pub fn versions(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let tasks: Vec<TaskMetrics> = ctx
        .store
        .tasks_with_metrics(project_id)?
        .into_iter()
        .map(|(_, m)| m)
        .collect();
    let rows = version_page::rows(&tasks);

    let repo = protocol::open(&ctx.autome_home);
    let mut entries_by_tag: serde_json::Map<String, Value> = serde_json::Map::new();
    for row in &rows {
        let Some(repo) = repo.as_ref() else { break };
        let Ok(files) = repo.files_at(&row.tag) else {
            continue;
        };
        let Some(text) = files.get("CHANGELOG.md") else {
            continue;
        };
        let Ok(log) = changelog::parse(text) else {
            continue;
        };
        if let Some(version) = log.version(&row.tag) {
            entries_by_tag.insert(row.tag.clone(), json!(version.entries));
        }
    }

    Ok((
        json!({
            "rows": rows.iter().map(|r| json!({
                "protocol_ref": r.protocol_ref,
                "tag": r.tag,
                "samples": r.samples,
                "enough_samples": r.enough_samples(),
                "means": r.means,
                "warnings": r.warnings,
            })).collect::<Vec<_>>(),
            "metric_names": TaskMetrics::METRIC_NAMES,
            "min_samples": version_page::MIN_SAMPLES,
            "changelog_by_tag": entries_by_tag,
        }),
        vec![],
    ))
}

/// `protocol.improve` — start a meta task on the protocol repository.
///
/// Nothing starts by itself. The triggers are shown as a suggestion and this
/// is the button: a protocol iteration costs a full Loop's worth of sessions,
/// and "there is enough evidence to have a conversation" is not the same as
/// "have it now".
pub fn improve(ctx: &mut Ctx) -> DispatchResult {
    let project = crate::meta_store::ensure_project(ctx)?;
    if let Ok(tasks) = ctx.store.list_tasks(&project.id)
        && let Some(running) = tasks.iter().find(|t| !t.state.is_terminal())
    {
        return Err(rejected(format!(
            "协议仓库上已经有一个没结束的任务：{}（{}）。两个元任务同时改协议，             合并时会在最不能出错的那个文件上冲突。",
            running.id, running.title
        )));
    }

    let (payload, events) = crate::dispatch::task_create_in(
        ctx,
        &project.id,
        crate::meta::REQUEST,
        "改进 Loop 协议",
    )?;
    Ok((payload, events))
}

/// `protocol.triggers` — why the app might suggest an iteration.
pub fn triggers(ctx: &mut Ctx) -> DispatchResult {
    let triggers = crate::meta_store::triggers(ctx)?;
    Ok((
        json!({
            "triggers": triggers.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
            "suggest": !triggers.is_empty(),
        }),
        vec![],
    ))
}

/// `protocol.pin` — which version a project's *new* tasks start on.
///
/// Running tasks are unaffected by construction: each holds its own copy.
pub fn pin(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let project_id = params
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 project_id"))?;
    let tag = params
        .get("tag")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let project = ctx.store.get_project(project_id)?;
    let repo_path = std::path::PathBuf::from(&project.path);

    if let Some(tag) = &tag {
        let repo = protocol::ensure(&ctx.autome_home)?;
        if !repo.tags()?.contains(tag) {
            return Err(rejected(format!("协议仓库里没有标签 `{tag}`")));
        }
    }

    let mut config = config_io::load_project(&repo_path)?;
    config.loop_overrides.protocol = tag.clone();
    config_io::save_project(&repo_path, &config)?;

    let seq = ctx.store.append_event(
        "project.protocol_pinned",
        project_id,
        json!({ "tag": tag }),
    )?;
    Ok((
        json!({ "tag": tag }),
        vec![crate::dispatch::event(
            seq,
            "project.protocol_pinned",
            project_id,
            json!({}),
        )],
    ))
}

/// `protocol.rollback` — a *forward* revert to an earlier version's content.
///
/// Moving the tag would take the eval cases back with the text and leave the
/// metrics table pointing at a version whose content had changed under it. The
/// old rows have to stay true, so the rollback is a new version with its own
/// number and its own hash.
pub fn rollback(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let tag = params
        .get("tag")
        .and_then(Value::as_str)
        .ok_or_else(|| rejected("缺少 tag"))?;
    let repo = protocol::ensure(&ctx.autome_home)?;
    let new_ref = repo.revert_to(tag)?;
    let seq = ctx.store.append_event(
        "protocol.rolled_back",
        "protocol",
        json!({ "to": tag, "new": new_ref.to_wire() }),
    )?;
    Ok((
        json!({ "tag": new_ref.tag, "hash": new_ref.hash, "from": tag }),
        vec![crate::dispatch::event(
            seq,
            "protocol.rolled_back",
            "protocol",
            json!({}),
        )],
    ))
}
