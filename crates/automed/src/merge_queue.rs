//! Merge-section ownership is recovered entirely from task state and ledger.
use crate::{config_io, dispatch::Ctx, store::TaskRecord};
use autome_domain::{
    merge_queue::{self, AuthSource},
    task::{Node, TaskState},
};
use serde_json::json;
use std::path::Path;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn authorization(ctx: &Ctx, task: &TaskRecord) -> Result<Option<AuthSource>> {
    let project = ctx.store.get_project(&task.project_id)?;
    let auto = config_io::load_project(Path::new(&project.path))?
        .merge_auto
        .unwrap_or(false);
    let record = ctx.store.last_event(&task.id, "merge.auto")?;
    Ok(merge_queue::authorization(
        record.as_ref().and_then(|p| {
            p["active"]
                .as_bool()
                .map(|active| (active, p["by"].as_str()))
        }),
        auto,
    ))
}
pub fn holds(ctx: &Ctx, task: &TaskRecord) -> Result<bool> {
    let active = ctx
        .store
        .last_event_of(&task.id, &["merge.locked", "merge.unlocked"])?
        .is_some_and(|(_, kind, _)| kind == "merge.locked");
    Ok(merge_queue::holds(active, &task.state))
}
pub fn holder(ctx: &Ctx, project: &str) -> Result<Option<String>> {
    for task in ctx.store.list_unfinished()? {
        if task.project_id == project && holds(ctx, &task)? {
            return Ok(Some(task.id));
        }
    }
    Ok(None)
}
pub fn waiting_for(ctx: &Ctx, task: &TaskRecord) -> Result<Option<String>> {
    let holder = holder(ctx, &task.project_id)?;
    let seq = crate::gate_queue::queued_seq(ctx, &task.id)?;
    let authorized = authorization(ctx, task)?.is_some();
    let mut earlier: Option<(String, u64)> = None;
    if holder.is_none() && authorized {
        for other in ctx.store.list_unfinished()? {
            if other.project_id != task.project_id
                || other.id == task.id
                || other.state
                    != (TaskState::Active {
                        node: Node::MergeGate,
                    })
            {
                continue;
            }
            if let Some(order) = crate::gate_queue::queued_seq(ctx, &other.id)?
                && authorization(ctx, &other)?.is_some()
                && earlier.as_ref().is_none_or(|(_, first)| order < *first)
            {
                earlier = Some((other.id, order));
            }
        }
    }
    Ok(merge_queue::turn(
        holder.as_deref(),
        earlier.as_ref().map(|(id, seq)| (id.as_str(), *seq)),
        authorized,
        &task.id,
        seq,
    )
    .map(str::to_owned))
}
pub fn wait(ctx: &mut Ctx, task: &TaskRecord, node: Node, holder: &str) -> Result<()> {
    if node == Node::MergeGate && crate::gate_queue::queued_seq(ctx, &task.id)?.is_none() {
        ctx.store.append_event(
            "gate.queued",
            &task.id,
            json!({"node": node.as_str(), "job":"gate"}),
        )?;
    }
    if ctx
        .store
        .last_event_of(&task.id, &["task.updated", "merge.waiting"])?
        .is_none_or(|(_, kind, _)| kind != "merge.waiting")
    {
        ctx.store.append_event(
            "merge.waiting",
            &task.id,
            json!({"node":node.as_str(), "holder":holder}),
        )?;
    }
    Ok(())
}
pub fn enter(ctx: &mut Ctx, task: &TaskRecord) -> Result<bool> {
    if let Some(holder) = waiting_for(ctx, task)? {
        wait(ctx, task, Node::MergeGate, &holder)?;
        return Ok(false);
    }
    if authorization(ctx, task)?.is_some() && !holds(ctx, task)? {
        ctx.store
            .append_event("merge.locked", &task.id, json!({"node":"merge_gate"}))?;
    }
    Ok(true)
}

/// Read-only projection; a missing checkout must not hide the task itself.
pub fn view(ctx: &Ctx, task: &TaskRecord) -> Result<serde_json::Value> {
    if task.state.is_terminal() {
        return Ok(serde_json::Value::Null);
    }
    let auth = authorization(ctx, task)?;
    let waiting = match task.state {
        TaskState::Active {
            node: Node::MergeGate,
        } => waiting_for(ctx, task)?,
        TaskState::Active {
            node: Node::Merging,
        } => holder(ctx, &task.project_id)?.filter(|id| id != &task.id),
        _ => None,
    };
    let stale = if task.state
        == (TaskState::Active {
            node: Node::AwaitMerge,
        }) {
        match stale_view(ctx, task) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(task_id = %task.id, %error, "cannot read merge staleness");
                serde_json::Value::Null
            }
        }
    } else {
        serde_json::Value::Null
    };
    Ok(
        json!({"authorized":auth.is_some(), "authorized_by":auth.map(AuthSource::as_str),
        "holds_section":holds(ctx, task)?, "waiting_for":waiting, "stale":stale}),
    )
}
fn stale_view(ctx: &Ctx, task: &TaskRecord) -> Result<serde_json::Value> {
    let project = ctx.store.get_project(&task.project_id)?;
    let layout = crate::layout::TaskLayout::of(
        &project,
        &task.slug,
        &crate::scheduler::working_repos(&project, task),
    );
    let mut repos = Vec::new();
    let mut total = 0u64;
    for slot in &layout.repos {
        let behind = crate::git::behind_count(&slot.repo_root, &slot.branch, &slot.base)?;
        total += u64::from(behind);
        repos.push(json!({"repo":slot.name,"base":slot.base,"behind":behind}));
    }
    if total == 0 {
        return Ok(serde_json::Value::Null);
    }
    let global = config_io::load_global(&ctx.autome_home)?;
    let local = config_io::load_project(Path::new(&project.path))?;
    let gated = task.kind == autome_domain::task::TaskKind::Bug
        || autome_domain::config::resolve(&global, &local)
            .gate
            .is_some();
    Ok(json!({"behind":total,"gated":gated,"repos":repos}))
}
