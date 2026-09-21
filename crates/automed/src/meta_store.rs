//! Gathering the evidence a meta task is given, out of the store and the
//! repositories.
//!
//! Split from [`crate::meta`] so that the rendering of `inputs/` stays pure
//! and testable without a database, and everything that reaches for a file or
//! a table sits here.

use std::path::Path;

use autome_domain::lesson::{self, AggregatedLesson, Lesson};
use autome_domain::project::{AddDisposition, Onboarding, Project};
use autome_domain::task::TaskState;

use crate::dispatch::Ctx;
use crate::meta::{self, TaskInput};
use crate::store::{TaskRecord, new_id, now_iso};

pub type Result<T> = std::result::Result<T, crate::scheduler::SchedulerError>;

fn err(detail: impl Into<String>) -> crate::scheduler::SchedulerError {
    crate::scheduler::SchedulerError {
        detail: detail.into(),
    }
}

/// The protocol repository as a project the scheduler already knows how to
/// run.
///
/// Registered rather than special-cased: rebase, the merge precondition, the
/// worktree cleanup and the `docs/<slug>/` archive all work because nothing
/// about this project is unusual. `parallel_limit` is 1 because two meta tasks
/// editing the protocol at once would produce a merge conflict in the one file
/// neither of them can afford to get wrong.
pub fn ensure_project(ctx: &mut Ctx) -> Result<Project> {
    let path = crate::protocol::repo_path(&ctx.autome_home);
    crate::protocol::ensure(&ctx.autome_home).map_err(|e| err(e.to_string()))?;
    let path_str = path.to_string_lossy().to_string();

    if let Ok(Some(existing)) = ctx.store.project_by_path(&path_str) {
        return Ok(existing);
    }
    let project = Project {
        id: new_id("prj"),
        path: path_str,
        display_name: "Loop 协议".into(),
        default_branch: "main".into(),
        parallel_limit: 1,
        // There is nothing to onboard: the repository is the protocol, and a
        // project profile describing it would be a fourth copy of the rules.
        onboarding: Onboarding::Skipped,
        disposition: AddDisposition::CreatedAndInitialised,
        added_at: now_iso(),
        removed_at: None,
        // The protocol repository is one repository, and the only project
        // Autome creates for itself.
        kind: autome_domain::project::ProjectKind::Repo,
        members: Vec::new(),
        docs_repo: None,
    };
    ctx.store.insert_project(&project)?;
    ctx.store.append_event(
        "project.added",
        &project.id,
        serde_json::json!({ "path": project.path, "builtin": "protocol" }),
    )?;
    Ok(project)
}

/// Whether a project is the protocol repository.
pub fn is_protocol_project(ctx: &Ctx, project: &Project) -> bool {
    Path::new(&project.path) == crate::protocol::repo_path(&ctx.autome_home)
}

/// Every terminal task across every project, with what it learned.
///
/// Cross-project on purpose: the protocol is one thing, and a lesson that
/// turns up in two different repositories is much better evidence than one
/// that turns up twice in the same one. The metrics table keeps the projects
/// in separate rows and never averages across them.
pub fn collect(ctx: &mut Ctx) -> Result<Vec<TaskInput>> {
    let protocol_path = crate::protocol::repo_path(&ctx.autome_home);
    let mut out = Vec::new();
    for project in ctx.store.list_projects()? {
        if Path::new(&project.path) == protocol_path {
            // A meta task's own numbers are not evidence about the protocol;
            // including them would let an iteration cite itself.
            continue;
        }
        let repo = std::path::PathBuf::from(&project.path);
        for task in ctx.store.list_tasks(&project.id)? {
            let Some(metrics) = task.metrics.clone() else {
                continue;
            };
            out.push(TaskInput {
                project: project.display_name.clone(),
                slug: task.slug.clone(),
                title: task.title.clone(),
                metrics,
                lessons: read_lessons(ctx, &repo, &task),
                retro_tail: read_retro_tail(&repo, &task),
                failure: failure_detail(&task),
            });
        }
    }
    Ok(out)
}

/// A task's lessons, from the event the core recorded when it read them.
///
/// The event rather than the file, because the file lives in a worktree that
/// cleanup removes after a merge — and the merged branch put it on the default
/// branch under `docs/<slug>/`, which is where the fallback looks.
fn read_lessons(ctx: &Ctx, repo: &Path, task: &TaskRecord) -> Vec<Lesson> {
    if let Ok(Some(payload)) = ctx.store.last_event(&task.id, "task.lessons")
        && let Some(list) = payload.get("lessons")
        && let Ok(lessons) = serde_json::from_value::<Vec<Lesson>>(list.clone())
    {
        return lessons;
    }
    let path = repo.join(task.doc_dir()).join("lessons.md");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| lesson::parse(&t).ok())
        .unwrap_or_default()
}

/// The closing summary of `retro.md`: everything after the last one-line
/// round record. The protocol allows exactly this part to run to paragraphs.
fn read_retro_tail(repo: &Path, task: &TaskRecord) -> Option<String> {
    let path = repo.join(task.doc_dir()).join("retro.md");
    let text = std::fs::read_to_string(path).ok()?;
    let lines: Vec<&str> = text.lines().collect();
    let last_record = lines
        .iter()
        .rposition(|l| l.matches('|').count() >= 4)
        .map(|i| i + 1)
        .unwrap_or(0);
    let tail = lines[last_record..].join("\n").trim().to_string();
    (!tail.is_empty()).then_some(tail)
}

fn failure_detail(task: &TaskRecord) -> Option<String> {
    let TaskState::Failed { reason, at } = &task.state else {
        return None;
    };
    let detail = serde_json::to_value(reason)
        .ok()?
        .get("detail")?
        .as_str()?
        .to_string();
    Some(format!("停在 {}：{detail}", at.as_str()))
}

/// The aggregated protocol-level lessons, for the trigger check.
pub fn aggregated(tasks: &[TaskInput]) -> Vec<AggregatedLesson> {
    lesson::aggregate(
        tasks
            .iter()
            .map(|t| (t.slug.as_str(), t.lessons.as_slice()))
            .collect::<Vec<_>>(),
    )
}

/// Tasks that stopped on a protocol problem, for the trigger check.
pub fn protocol_failures(tasks: &[TaskInput]) -> Vec<String> {
    tasks
        .iter()
        .filter(|t| t.failure.is_some())
        .map(|t| t.slug.clone())
        .collect()
}

/// How many tasks finished since the newest protocol tag was released.
///
/// Counted by version rather than by date: a task that ran under the current
/// version is one this version has been tried on, and that is what "enough has
/// happened to have something to say" means.
pub fn tasks_since_release(tasks: &[TaskInput], current: &str) -> usize {
    tasks
        .iter()
        .filter(|t| t.metrics.protocol_ref.as_deref() == Some(current))
        .count()
}

/// The Backlog of the last meta task, which is this one's `deferred.md`.
///
/// Backlog means something specific in a meta task: not "a nice idea someone
/// had" but "a change we decided to put off until next time".
pub fn deferred(ctx: &mut Ctx, protocol_project: &str) -> Result<Vec<String>> {
    let mut tasks = ctx.store.list_tasks(protocol_project)?;
    tasks.reverse();
    for task in tasks {
        let items = ctx.store.list_decisions(&task.id)?;
        let backlog: Vec<String> = items
            .iter()
            .filter(|d| d.kind == "backlog")
            .map(|d| format!("{} {}", d.item_id, d.text))
            .collect();
        if !backlog.is_empty() {
            return Ok(backlog);
        }
    }
    Ok(vec![])
}

/// Changelog entries whose measured outcome went against their prediction.
pub fn contradicted(ctx: &mut Ctx) -> Result<Vec<String>> {
    let Some(repo) = crate::protocol::open(&ctx.autome_home) else {
        return Ok(vec![]);
    };
    let Ok(files) = repo.working_files() else {
        return Ok(vec![]);
    };
    let Some(text) = files.get("CHANGELOG.md") else {
        return Ok(vec![]);
    };
    let Ok(log) = autome_domain::changelog::parse(text) else {
        return Ok(vec![]);
    };
    Ok(log
        .entries()
        .filter(|e| e.held_up() == Some(false))
        .map(|e| {
            let r = e.realized_impact.as_ref().expect("held_up implies one");
            format!(
                "{} 预测 {} 会 {}，实际从 {} 变成 {}（前 {} 个任务 / 后 {} 个）",
                e.id,
                r.metric,
                e.predicted_impact.direction.as_str(),
                r.before,
                r.after,
                r.samples_before,
                r.samples_after
            )
        })
        .collect())
}

/// What the settings screen shows: whether there is anything worth iterating
/// on, and why.
pub fn triggers(ctx: &mut Ctx) -> Result<Vec<meta::Trigger>> {
    let tasks = collect(ctx)?;
    let current = crate::protocol::open(&ctx.autome_home)
        .and_then(|r| r.resolve(None).ok())
        .map(|(r, _)| r.to_wire())
        .unwrap_or_default();
    Ok(meta::triggers(
        tasks_since_release(&tasks, &current),
        &aggregated(&tasks),
        &protocol_failures(&tasks),
    ))
}
