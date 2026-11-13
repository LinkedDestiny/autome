//! Ledger-local host-job occupancy, FIFO admission and bounded reclamation.
use std::path::Path;

use crate::{config_io, dispatch::Ctx, gate, store::TaskRecord};
use autome_domain::{
    config,
    gate_queue::{self, Admission, Lane, Waiter},
    project::Project,
    task::{Node, TaskState},
};
use serde_json::json;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const QUEUE_EVENTS: &[&str] = &[
    "gate.queued",
    "gate.started",
    "setup.started",
    "session.started",
    "session.deferred",
    "task.updated",
    "gate.queue_dropped",
];

fn limit(ctx: &Ctx, project: &Project, lane: Lane) -> Result<u32> {
    let resolved = config::resolve(
        &config_io::load_global(&ctx.autome_home)?,
        &config_io::load_project(Path::new(&project.path))?,
    );
    Ok(match lane {
        Lane::Heavy => resolved.gate_concurrency,
        Lane::Light => resolved.gate_quick_concurrency,
    })
}

pub fn queued_seq(ctx: &Ctx, task: &str) -> Result<Option<u64>> {
    Ok(ctx
        .store
        .last_event_of(task, QUEUE_EVENTS)?
        .filter(|(_, kind, _)| kind == "gate.queued")
        .map(|(seq, _, _)| seq))
}

pub fn waiters(ctx: &Ctx) -> Result<Vec<Waiter>> {
    let mut queue = Vec::new();
    for task in ctx.store.list_unfinished()? {
        if let Some((seq, kind, payload)) = ctx.store.last_event_of(&task.id, QUEUE_EVENTS)?
            && kind == "gate.queued"
        {
            if task.state
                == (TaskState::Active {
                    node: Node::MergeGate,
                })
                && crate::merge_queue::waiting_for(ctx, &task)?.is_some()
            {
                continue;
            }
            let project = ctx.store.get_project(&task.project_id)?;

            let lane = serde_json::from_value(payload["lane"].clone()).unwrap_or_default();
            queue.push(Waiter {
                task_id: task.id,
                seq: seq as i64,
                limit: limit(ctx, &project, lane).unwrap_or(1),
                lane,
            });
        }
    }
    queue.sort_by_key(|w| w.seq);
    Ok(queue)
}

pub fn drop_on_error(ctx: &mut Ctx, task: &TaskRecord, error: &str) -> Result<()> {
    if ctx
        .store
        .last_event_of(&task.id, QUEUE_EVENTS)?
        .is_some_and(|(_, kind, _)| kind == "gate.queued")
    {
        ctx.store.append_event(
            "gate.queue_dropped",
            &task.id,
            json!({"node":task.state.node().map(Node::as_str), "error":error}),
        )?;
    }
    Ok(())
}

fn jobs(repo: &Path, task: &str) -> std::io::Result<Vec<(String, gate::Job)>> {
    let entries = match std::fs::read_dir(gate::dir_for(repo, task)) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // A missing task directory is normal; a missing project is unknown.
            std::fs::read_dir(repo)?;
            return Ok(Vec::new());
        }
        Err(e) => return Err(e),
    };
    let mut found = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("started") {
            let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
            found.push((
                stem,
                gate::Job {
                    log: path.with_extension("log"),
                    pid: path.with_extension("pid"),
                    exit: path.with_extension("exit"),
                    started: path,
                },
            ));
        }
    }
    Ok(found)
}

fn recent_exit(ctx: &Ctx, job: &gate::Job) -> bool {
    job.exit.exists()
        && ctx.gate_prev_tick.is_none_or(|prev| {
            std::fs::metadata(&job.exit)
                .and_then(|m| m.modified())
                .map_or(true, |ended| ended >= prev)
        })
}

fn occupies(ctx: &Ctx, task: &TaskRecord, stem: &str, job: &gate::Job) -> Result<bool> {
    if gate::running(job) {
        return Ok(true);
    }
    let TaskState::Active { node } = task.state else {
        return Ok(false);
    };
    if !recent_exit(ctx, job) {
        return Ok(false);
    }
    if stem.starts_with("setup-") {
        return Ok(true);
    }
    Ok(stem.starts_with(&format!("{}-", node.as_str()))
        && ctx
            .store
            .last_event_in(
                &task.id,
                &["gate.started", "gate.result"],
                "node",
                node.as_str(),
            )?
            .is_some_and(|(_, kind, payload)| {
                kind == "gate.started" && payload["log"].as_str() == job.log.to_str()
            }))
}

fn possibly_inflight(ctx: &Ctx, task: &TaskRecord) -> Result<bool> {
    for (started, result) in [
        ("gate.started", "gate.result"),
        ("setup.started", "setup.result"),
    ] {
        if ctx
            .store
            .last_event_of(&task.id, &[started, result])?
            .is_some_and(|(_, kind, _)| kind == started)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// None means unknown occupancy: fail closed, without failing the waiting task.
#[cfg(test)]
pub fn held(ctx: &Ctx) -> Result<Option<usize>> {
    held_in_lane(ctx, None)
}

fn held_in_lane(ctx: &Ctx, lane: Option<Lane>) -> Result<Option<usize>> {
    let mut held = 0;
    for task in ctx.store.list_unfinished()? {
        let project = ctx.store.get_project(&task.project_id)?;
        match jobs(Path::new(&project.path), &task.id) {
            Ok(jobs) => {
                for (stem, job) in jobs {
                    let job_lane = job
                        .launch()
                        .map(|l| Lane::for_kind(l.kind()))
                        .unwrap_or_default();
                    if lane.is_none_or(|lane| lane == job_lane) {
                        held += usize::from(occupies(ctx, &task, &stem, &job)?);
                    }
                }
            }
            Err(_) if possibly_inflight(ctx, &task)? => return Ok(None),
            Err(_) => {}
        }
    }
    Ok(Some(held))
}

pub fn admit(
    ctx: &mut Ctx,
    task: &TaskRecord,
    project: &Project,
    node: Node,
    job: &str,
    checkout: Option<&str>,
) -> Result<bool> {
    let lane = Lane::for_kind(job);
    let limit = limit(ctx, project, lane)?;
    let old_queue = waiters(ctx)?;
    if old_queue
        .iter()
        .any(|w| w.task_id == task.id && w.lane != lane)
    {
        ctx.store.append_event("gate.queued", &task.id, json!({"node": node.as_str(), "job": if job == "setup" { "setup" } else { "gate" }, "checkout": checkout, "lane": lane, "position": old_queue.iter().filter(|w| w.lane == lane).count() + 1}))?;
    }

    let waiters = waiters(ctx)?;
    match gate_queue::admission(
        held_in_lane(ctx, Some(lane))?.unwrap_or(limit as usize),
        &waiters,
        &task.id,
        limit,
        lane,
    ) {
        Admission::Go => Ok(true),
        Admission::Wait { position } => {
            if gate_queue::position(&waiters, &task.id).is_none() {
                ctx.store.append_event("gate.queued", &task.id, json!({"node": node.as_str(), "job": if job == "setup" { "setup" } else { "gate" }, "checkout": checkout, "position": position, "lane": lane}))?;
            }
            Ok(false)
        }
    }
}

/// Only a setup collected in this call may hand its continuously held slot on.
pub fn continuation(ctx: &Ctx, task: &TaskRecord, job: &gate::Job, checkout: &str) -> Result<bool> {
    if !matches!(
        ctx.store.get_task(&task.id)?.state,
        TaskState::Active { .. }
    ) || !recent_exit(ctx, job)
    {
        return Ok(false);
    }
    let Some((seq, _, payload)) =
        ctx.store
            .last_event_in(&task.id, &["setup.started"], "checkout", checkout)?
    else {
        return Ok(false);
    };
    Ok(payload["log"].as_str() == job.log.to_str()
        && !ctx
            .store
            .any_event_after(&task.id, &["task.updated"], seq)?)
}

/// Runs even when a task's business path cannot reach poll (or is paused).
pub fn reclaim(ctx: &Ctx) -> Result<Vec<String>> {
    let mut errors = Vec::new();
    for task in ctx.store.list_unfinished()? {
        let project = ctx.store.get_project(&task.project_id)?;
        let repo = Path::new(&project.path);
        let found = match jobs(repo, &task.id) {
            Ok(found) => found,
            Err(e) => {
                if possibly_inflight(ctx, &task)? {
                    errors.push(format!("无法确认项目 {} 的宿主作业（{e}）：任务 {} 可能仍有作业在运行，已暂停准入新的宿主作业；请恢复该目录，或取消该任务", project.display_name, task.id));
                }
                continue;
            }
        };
        let config = config_io::load_project(repo).ok();
        for (stem, job) in found {
            if !gate::running(&job) {
                continue;
            }
            let setup = stem.starts_with("setup-");
            let timeout = if setup {
                config
                    .as_ref()
                    .and_then(|c| c.setup.as_ref())
                    .map(|s| s.timeout_minutes)
                    .unwrap_or(config::DEFAULT_SETUP_TIMEOUT_MINUTES)
            } else {
                config
                    .as_ref()
                    .and_then(|c| c.gate.as_ref())
                    .map(|g| g.timeout_minutes)
                    .unwrap_or(config::DEFAULT_GATE_TIMEOUT_MINUTES)
            };
            let timeout = if setup {
                timeout
            } else {
                job.launch().map(|l| l.timeout(timeout)).unwrap_or(timeout)
            };
            gate::poll_as(&job, timeout, if setup { "准备命令" } else { "门禁" });
        }
    }
    Ok(errors)
}
