//! Host preparation, collected by checkout before considering newer inputs.
use std::path::{Path, PathBuf};

use autome_domain::{config::SetupConfig, project::Project, setup, task::Node};
use serde_json::json;

use crate::{dispatch::Ctx, gate, store::TaskRecord};

pub struct Checkout<'a> {
    pub kind: setup::Checkout,
    pub dir: &'a Path,
    pub record: PathBuf,
}

pub enum Ready {
    Ready,
    Pending,
    Failed {
        command: String,
        exit_code: i32,
        log: String,
        tail: String,
    },
}

pub fn worktree_record(repo: &Path, task: &str) -> PathBuf {
    repo.join(".autome/output/setup")
        .join(task)
        .join("worktree.fp")
}

fn write_record(path: &Path, value: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, value)?;
    std::fs::rename(tmp, path)
}

pub fn ensure(
    ctx: &mut Ctx,
    task: &TaskRecord,
    project: &Project,
    config: &SetupConfig,
    checkout: Checkout<'_>,
    node: Node,
) -> Result<Ready, Box<dyn std::error::Error>> {
    let repo = Path::new(&project.path);
    let kind = checkout.kind.as_str();
    let job = gate::Job::at(repo, &task.id, "setup", kind);
    let running = repo
        .join(".autome/output/setup")
        .join(&task.id)
        .join(format!("{kind}.running"));
    if job.exists() {
        match gate::poll_as(&job, config.timeout_minutes, "准备命令") {
            gate::Poll::Running => return Ok(Ready::Pending),
            gate::Poll::Vanished => job.clear(),
            gate::Poll::Done { exit_code } => {
                // The wrapper removes its PID after publishing exit. Wait for
                // that last write before reusing the same checkout job paths.
                if std::fs::read_to_string(&job.pid)
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .is_some_and(crate::launcher::pid_alive)
                {
                    return Ok(Ready::Pending);
                }
                let fingerprint = std::fs::read_to_string(&running).ok();
                if let Some(ref fingerprint) = fingerprint {
                    let tail = gate::tail(&job.log, 20);
                    ctx.store.append_event(
                        "setup.result",
                        &task.id,
                        json!({
                            "checkout": kind, "node": node.as_str(), "ok": exit_code == 0,
                            "exit_code": exit_code, "command": config.command,
                            "fingerprint": &fingerprint[..12], "log": job.log, "tail": tail,
                        }),
                    )?;
                    if exit_code == 0 {
                        write_record(&checkout.record, fingerprint)?;
                    }
                }
                for path in [&job.started, &job.pid, &job.exit, &running] {
                    let _ = std::fs::remove_file(path);
                }
                if fingerprint.is_some() && exit_code != 0 {
                    return Ok(Ready::Failed {
                        command: config.command.clone(),
                        exit_code,
                        log: job.log.to_string_lossy().into_owned(),
                        tail: gate::tail(&job.log, 20),
                    });
                }
            }
        }
    }
    let files: Vec<_> = config
        .watch
        .iter()
        .map(|path| (path.as_str(), std::fs::read(checkout.dir.join(path)).ok()))
        .collect();
    let inputs: Vec<_> = files
        .iter()
        .map(|(path, bytes)| (*path, bytes.as_deref()))
        .collect();
    let fingerprint = setup::fingerprint(&config.command, &inputs);
    let previous = std::fs::read_to_string(&checkout.record).ok();
    if !setup::needs_run(previous.as_deref(), &fingerprint) {
        return Ok(Ready::Ready);
    }
    write_record(&running, &fingerprint)?;
    gate::start(
        &job,
        checkout.dir,
        &config.command,
        &[("AUTOME_TASK", &task.id), ("AUTOME_SETUP_CHECKOUT", kind)],
    )?;
    ctx.store.append_event(
        "setup.started",
        &task.id,
        json!({
            "checkout": kind, "node": node.as_str(), "command": config.command,
            "fingerprint": &fingerprint[..12], "log": job.log,
            "reason": if previous.is_some() { "变化" } else { "首次" },
        }),
    )?;
    Ok(Ready::Pending)
}
