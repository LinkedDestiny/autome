//! The core, in this process.
//!
//! The command line does not start `automed` and talk to it over a pipe: it
//! links the same library and calls `handle_command` directly, with the same
//! ledger the desktop app uses. Every command therefore goes through exactly
//! the code the desktop's requests go through — the same validation, the same
//! transitions, the same driver lock — and there is no second implementation
//! of anything to drift.

use std::path::{Path, PathBuf};

use automed::dispatch::{Ctx, handle_command};
use automed::ipc::{Command, PROTOCOL_VERSION, ReplyOutcome};
use automed::store::Store;
use serde_json::{Value, json};

/// Why a command did not do what it was asked.
#[derive(Debug)]
pub enum CliError {
    /// The command line itself was wrong. Exit code 2.
    Usage(String),
    /// The core refused, or something failed. Exit code 1.
    Failed(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Usage(_) => 2,
            CliError::Failed(_) => 1,
        }
    }
}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError::Usage(s)
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

pub struct Client {
    pub ctx: Ctx,
    pub home: PathBuf,
    pub db: PathBuf,
}

impl Client {
    /// Opens the ledger under `AUTOME_HOME` (`~/.autome`), or `AUTOMED_DB_PATH`.
    pub fn open() -> Result<Client> {
        let home = automed::config_io::default_global_dir();
        let db = automed::entry::ledger_path(&home);
        let store = Store::open(&db).map_err(|e| CliError::Failed(format!("打不开状态库：{e}")))?;
        let user_home = std::env::var("HOME").unwrap_or_else(|_| "/".into());
        Ok(Client {
            ctx: Ctx::new(store, &home, user_home),
            home,
            db,
        })
    }

    /// One JSON-RPC call, answered in-process.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = format!("cli-{}", std::process::id());
        let command = Command {
            request_id: id.clone(),
            command_id: id,
            expected_revision: None,
            protocol_version: PROTOCOL_VERSION,
            method: method.to_string(),
            params,
        };
        match handle_command(&mut self.ctx, &command).reply.outcome {
            ReplyOutcome::Ok { payload, .. } => Ok(payload),
            ReplyOutcome::Error { message, .. } => Err(CliError::Failed(message)),
        }
    }

    // ---- name resolution ---------------------------------------------

    /// Every registered project, as `project.list` reports them.
    pub fn projects(&mut self) -> Result<Vec<Value>> {
        let list = self.call("project.list", json!({}))?;
        Ok(list
            .get("projects")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|e| e["project"].clone()).collect())
            .unwrap_or_default())
    }

    /// A project named by id, display name or path — or, when none is named,
    /// the registered project the working directory is inside.
    pub fn resolve_project(&mut self, named: Option<&str>) -> Result<Value> {
        let projects = self.projects()?;
        let active: Vec<&Value> = projects
            .iter()
            .filter(|p| p.get("removed_at").is_none_or(Value::is_null))
            .collect();
        if let Some(name) = named {
            let as_path = canonical(Path::new(name));
            return active
                .iter()
                .find(|p| {
                    p["id"] == name
                        || p["display_name"] == name
                        || (as_path.is_some()
                            && p["path"].as_str().and_then(|s| canonical(Path::new(s))) == as_path)
                })
                .map(|p| (*p).clone())
                .ok_or_else(|| {
                    CliError::Failed(format!(
                        "没有叫 `{name}` 的项目；`autome project ls` 看已注册的"
                    ))
                });
        }
        let cwd = std::env::current_dir()
            .ok()
            .and_then(|d| canonical(&d))
            .unwrap_or_default();
        // The deepest registered project containing the working directory, so
        // a workspace member registered on its own wins over the workspace.
        active
            .iter()
            .filter_map(|p| {
                let root = canonical(Path::new(p["path"].as_str()?))?;
                cwd.starts_with(&root)
                    .then(|| (root.components().count(), (*p).clone()))
            })
            .max_by_key(|(depth, _)| *depth)
            .map(|(_, p)| p)
            .ok_or_else(|| {
                CliError::Failed(
                    "当前目录不在任何已注册的项目里：用 -p 指定，或先 `autome project add <路径>`"
                        .into(),
                )
            })
    }

    /// Every task, across projects.
    pub fn all_tasks(&mut self) -> Result<Vec<(Value, Value)>> {
        let mut out = Vec::new();
        for p in self.projects()? {
            let id = p["id"].as_str().unwrap_or_default().to_string();
            let got = self.call("project.get", json!({ "project_id": id }))?;
            for t in got["tasks"].as_array().cloned().unwrap_or_default() {
                out.push((p.clone(), t));
            }
        }
        Ok(out)
    }

    /// A task named by its id or by an unambiguous prefix of its id or slug.
    pub fn resolve_task(&mut self, named: &str) -> Result<String> {
        let tasks = self.all_tasks()?;
        if let Some((_, t)) = tasks
            .iter()
            .find(|(_, t)| t["id"] == named || t["slug"] == named)
        {
            return Ok(t["id"].as_str().unwrap_or_default().to_string());
        }
        let hits: Vec<&Value> = tasks
            .iter()
            .map(|(_, t)| t)
            .filter(|t| {
                t["id"].as_str().is_some_and(|s| s.starts_with(named))
                    || t["slug"].as_str().is_some_and(|s| s.starts_with(named))
            })
            .collect();
        match hits.as_slice() {
            [one] => Ok(one["id"].as_str().unwrap_or_default().to_string()),
            [] => Err(CliError::Failed(format!("没有任务匹配 `{named}`"))),
            many => Err(CliError::Failed(format!(
                "`{named}` 匹配到 {} 个任务：{}",
                many.len(),
                many.iter()
                    .filter_map(|t| t["id"].as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            ))),
        }
    }
}

fn canonical(p: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(p).ok()
}
