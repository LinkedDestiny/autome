//! A host job before Cleanup removes the task checkout.
use std::path::Path;

use autome_domain::{
    config::MergeAfterConfig,
    merge_after::{self, MainVerdict, Plan},
    project::Project,
};
use serde_json::json;

use crate::{dispatch::Ctx, gate, store::TaskRecord};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub enum Phase {
    Done,
    Pending,
}

fn concluded(ctx: &Ctx, task: &TaskRecord) -> Result<bool> {
    Ok(ctx.store.count_events(&task.id, "merge.after")? > 0
        || ctx.store.count_events(&task.id, "merge.after_skipped")? > 0)
}

pub fn begun(ctx: &Ctx, task: &TaskRecord, repo: &Path) -> Result<bool> {
    if concluded(ctx, task)?
        || gate::dir_for(repo, &task.id)
            .join("after_merge.prepared")
            .exists()
        || ctx.store.count_events(&task.id, "merge.after_started")? > 0
    {
        return Ok(true);
    }
    let dir = gate::dir_for(repo, &task.id);
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let name = entry?.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("after_merge-") && name.ends_with(".started") {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub fn prepared(repo: &Path, task: &TaskRecord) -> Result<()> {
    let dir = gate::dir_for(repo, &task.id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("after_merge.prepared"), "")?;
    Ok(())
}

fn started(ctx: &Ctx, task: &TaskRecord, config: &MergeAfterConfig, job: &gate::Job) -> Result<()> {
    ctx.store.append_event(
        "merge.after_started",
        &task.id,
        json!({"command": config.command, "commit": task.merge_commit, "log": job.log}),
    )?;
    Ok(())
}

pub fn phase(
    ctx: &mut Ctx,
    task: &TaskRecord,
    project: &Project,
    config: &MergeAfterConfig,
) -> Result<Phase> {
    if concluded(ctx, task)? {
        return Ok(Phase::Done);
    }
    let repo = Path::new(&project.path);
    let commit = task.merge_commit.as_deref().unwrap_or_default();
    let job = gate::Job::at(
        repo,
        &task.id,
        "after_merge",
        &commit[..commit.len().min(12)],
    );
    if job.exists() {
        if ctx.store.count_events(&task.id, "merge.after_started")? == 0 {
            started(ctx, task, config, &job)?;
        }
        return match gate::poll_as(&job, config.timeout_minutes, "合并后命令") {
            gate::Poll::Running => Ok(Phase::Pending),
            gate::Poll::Vanished => {
                job.clear();
                Ok(Phase::Pending)
            }
            gate::Poll::Done { exit_code } => {
                let tail = gate::tail(&job.log, 20);
                let failure = if exit_code == gate::TIMED_OUT {
                    format!(
                        "超时（超过 {} 分钟，已终止，退出码 124）",
                        config.timeout_minutes
                    )
                } else {
                    format!("失败（退出码 {exit_code}）")
                };
                let detail = format!(
                    "任务 {} 合并后，在主检出里合并后命令 `{}` {}。日志：{}\n\n{tail}",
                    task.id,
                    config.command,
                    failure,
                    job.log.display()
                );
                ctx.store.conclude_merge_after(
                    &project.id,
                    &task.id,
                    json!({"ok": exit_code == 0, "exit_code": exit_code, "command": config.command,
                        "commit": commit, "log": job.log, "tail": tail}),
                    (exit_code != 0).then_some(detail.as_str()),
                )?;
                Ok(Phase::Done)
            }
        };
    }
    let main = match ctx.store.last_event_in(
        &task.id,
        &["gate.result", "gate.skipped"],
        "node",
        "main_check",
    )? {
        None => MainVerdict::NotRun,
        Some((_, kind, _)) if kind == "gate.skipped" => MainVerdict::Skipped,
        Some((_, _, value)) if value["ok"] == true => MainVerdict::Passed,
        Some(_) => MainVerdict::Failed,
    };
    if let Plan::Skip { reason } =
        merge_after::plan(project.is_workspace(), task.merge_commit.is_some(), main)
    {
        ctx.store
            .append_event("merge.after_skipped", &task.id, json!({"reason": reason}))?;
        return Ok(Phase::Done);
    }
    if another_main_job_running(repo, &task.id)? {
        return Ok(Phase::Pending);
    }
    gate::start(
        &job,
        repo,
        &config.command,
        &[("AUTOME_TASK", &task.id), ("AUTOME_MERGE_COMMIT", commit)],
    )?;
    started(ctx, task, config, &job)?;
    Ok(Phase::Pending)
}

fn another_main_job_running(repo: &Path, task_id: &str) -> Result<bool> {
    let root = gate::dir_for(repo, task_id);
    let Some(root) = root.parent().filter(|path| path.exists()) else {
        return Ok(false);
    };
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || entry.file_name() == task_id {
            continue;
        }
        let other = entry.file_name();
        let other = other.to_string_lossy();
        if gate::running(&gate::Job::at(repo, &other, "setup", "main")) {
            return Ok(true);
        }
        for file in std::fs::read_dir(entry.path())? {
            let name = file?.file_name();
            if let Some(key) = name
                .to_string_lossy()
                .strip_prefix("after_merge-")
                .and_then(|name| name.strip_suffix(".started"))
                && gate::running(&gate::Job::at(repo, &other, "after_merge", key))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
