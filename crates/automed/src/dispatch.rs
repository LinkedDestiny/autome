//! IPC command dispatch. Technical design §14.
//!
//! One function, `handle_command`, maps a `Command` frame to a `Reply` frame
//! and any `Event` frames the change produced. Everything the Renderer can ask
//! for goes through here, and nothing else does.
//!
//! Two rules the whole surface follows:
//!
//! - **Every command produces exactly one reply, even on failure.** A caller
//!   waiting on a response never hangs (design §14, inherited from the
//!   framing layer's contract).
//! - **Reads never mutate.** The read and write channels are separated in
//!   Electron Main by two independent allowlists; a method that appears on the
//!   read side must be side-effect free, or that separation means nothing.

use autome_domain::config::{self, ConfigViolation, ProjectConfig, RoleOverrides};
use autome_domain::environment::{self, Component};
use autome_domain::project::{self, AddDisposition, Onboarding, Project, ProjectKind};
use autome_domain::role::{Role, Runtime};
use autome_domain::status_block::{self, StatusBlock};
use autome_domain::task::{Disposition, Node, TaskState, Trigger};
use serde_json::{Value, json};

use crate::ipc::{Command, Event, PROTOCOL_VERSION, Reply, ReplyErrorCode, ReplyOutcome};
use crate::store::{Store, StoreError, new_id, now_iso};
use crate::{config_io, env_probe, git, init, scheduler, skills};

/// What a dispatched command produced.
pub struct Outcome {
    pub reply: Reply,
    pub events: Vec<Event>,
}

/// The mutable world a command runs against.
///
/// The two roots are fields rather than environment lookups: everything below
/// this point is then a pure function of the context, which is what lets the
/// test suite run commands against throwaway directories in parallel.
pub struct Ctx {
    pub store: Store,
    /// `~/.autome` — where the global config lives.
    pub autome_home: std::path::PathBuf,
    /// `$HOME` — the root of the two global skill directories.
    pub home: std::path::PathBuf,
    /// The environment as last observed, updated by a background thread.
    pub environment: EnvCache,
    /// Where sessions are started. Production runs them headless — a Loop
    /// opens six or more sessions and a window each would interrupt the user
    /// every time. The test suites set this rather than a process-global
    /// switch (see `launcher::LaunchMode`).
    pub launch_mode: crate::launcher::LaunchMode,
    /// Whether this process has run its one-time recovery pass yet. The core
    /// owns this rather than the shell: `recover()` had no production caller
    /// at all, so the reconciliation the design promises after a restart —
    /// pruning dead worktree registrations, reaping a session that ended while
    /// the app was closed, bringing the scaffold up to `SCAFFOLD_VERSION` —
    /// never ran outside the tests.
    pub recovered: bool,
}

impl Ctx {
    pub fn new(
        store: Store,
        autome_home: impl Into<std::path::PathBuf>,
        home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            store,
            autome_home: autome_home.into(),
            home: home.into(),
            environment: EnvCache::default(),
            launch_mode: crate::launcher::LaunchMode::Headless,
            recovered: false,
        }
    }

    /// Starts nothing: the prompt is written and the launch recorded. For
    /// tests whose subject is the state transition.
    pub fn dry(mut self) -> Self {
        self.launch_mode = crate::launcher::LaunchMode::Dry;
        self
    }

    /// Runs the wrapper directly, with no window — now the default, so this
    /// only states the end-to-end suites' intent explicitly.
    pub fn headless(mut self) -> Self {
        self.launch_mode = crate::launcher::LaunchMode::Headless;
        self
    }
}

/// The environment snapshot, and whether a probe is in flight.
///
/// Probing runs four external programs, and the login probes try several argv
/// forms each because the CLIs move their flags around. That is tens of
/// seconds in the worst case. The core's stdio loop is strictly serial — one
/// command at a time — so doing it inside a request blocks *every* later
/// request behind it, including the scheduler tick.
///
/// The packaged app demonstrated this on first launch: the dashboard's first
/// read sat on the probe, the IPC layer gave up after five seconds, and every
/// subsequent request queued behind the same probe and timed out too. The app
/// was unusable and the log said only "timed out".
///
/// So the probe runs on its own thread and callers read whatever is there.
/// "Not probed yet" is a legitimate answer and the UI says so, which is the
/// honest thing to show for a fact nobody has observed yet.
#[derive(Clone, Default)]
pub struct EnvCache {
    inner: std::sync::Arc<std::sync::Mutex<EnvState>>,
}

#[derive(Default)]
struct EnvState {
    snapshot: Option<autome_domain::environment::Environment>,
    probing: bool,
    /// Bumped on every completed probe, so a poller can tell a fresh result
    /// from the same one it already has.
    generation: u64,
    /// When the last probe finished. Drives the rate limit below.
    last_finished: Option<std::time::Instant>,
}

/// How stale the environment may get before an unforced probe runs again.
///
/// The desktop asks for a probe whenever its window regains focus, and a probe
/// spawns four subprocesses and takes seconds. Without a floor here, a user
/// switching between the app and a terminal re-probed every few seconds, and
/// each landing probe pushed a change event that made the renderer rebuild the
/// whole screen under their cursor. The environment changes when someone
/// installs a CLI or a login lapses — minutes, not seconds. E-04's button
/// forces a probe for the case where the user just did install something.
const ENV_PROBE_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);

impl EnvCache {
    pub fn snapshot(&self) -> Option<autome_domain::environment::Environment> {
        self.inner.lock().ok().and_then(|s| s.snapshot.clone())
    }

    pub fn generation(&self) -> u64 {
        self.inner.lock().map(|s| s.generation).unwrap_or(0)
    }

    pub fn is_probing(&self) -> bool {
        self.inner.lock().map(|s| s.probing).unwrap_or(false)
    }

    /// Starts a probe unless one is already running or the last one finished
    /// less than `ENV_PROBE_MIN_INTERVAL` ago. Returns whether it started one,
    /// so a caller can say "probing" rather than "unknown".
    pub fn start_probe(&self) -> bool {
        self.start_probe_inner(false)
    }

    /// E-04's manual re-probe: ignores the rate limit, because the user
    /// pressing "重新检测" has usually just changed something and is waiting
    /// to see it.
    pub fn force_probe(&self) -> bool {
        self.start_probe_inner(true)
    }

    fn start_probe_inner(&self, force: bool) -> bool {
        {
            let mut state = match self.inner.lock() {
                Ok(s) => s,
                Err(_) => return false,
            };
            if state.probing {
                return false;
            }
            if !force
                && state
                    .last_finished
                    .is_some_and(|t| t.elapsed() < ENV_PROBE_MIN_INTERVAL)
            {
                return false;
            }
            state.probing = true;
        }
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let observed = env_probe::probe_all();
            if let Ok(mut state) = inner.lock() {
                state.snapshot = Some(observed);
                state.probing = false;
                state.generation += 1;
                state.last_finished = Some(std::time::Instant::now());
            }
        });
        true
    }

    /// For tests and for the recovery path: probe on this thread.
    pub fn probe_blocking(&self) {
        let observed = env_probe::probe_all();
        if let Ok(mut state) = self.inner.lock() {
            state.snapshot = Some(observed);
            state.probing = false;
            state.generation += 1;
            state.last_finished = Some(std::time::Instant::now());
        }
    }
}

pub fn handle_command(ctx: &mut Ctx, command: &Command) -> Outcome {
    if command.protocol_version != PROTOCOL_VERSION {
        return fail(
            command,
            ReplyErrorCode::ProtocolViolation,
            format!(
                "协议版本不匹配：界面 {}，内核 {PROTOCOL_VERSION}",
                command.protocol_version
            ),
        );
    }

    match dispatch(ctx, command) {
        Ok((payload, events)) => {
            let snapshot_seq = ctx.store.latest_seq().unwrap_or(0);
            Outcome {
                reply: Reply {
                    request_id: command.request_id.clone(),
                    command_id: command.command_id.clone(),
                    protocol_version: PROTOCOL_VERSION,
                    outcome: ReplyOutcome::Ok {
                        snapshot_seq,
                        payload,
                    },
                },
                events,
            }
        }
        Err(err) => fail(command, err.code, err.message),
    }
}

pub struct DispatchError {
    pub code: ReplyErrorCode,
    pub message: String,
}

fn bad_params(msg: impl Into<String>) -> DispatchError {
    DispatchError {
        code: ReplyErrorCode::InvalidParams,
        message: msg.into(),
    }
}

fn internal(msg: impl Into<String>) -> DispatchError {
    DispatchError {
        code: ReplyErrorCode::Internal,
        message: msg.into(),
    }
}

pub fn rejected(msg: impl Into<String>) -> DispatchError {
    DispatchError {
        code: ReplyErrorCode::TransitionRejected,
        message: msg.into(),
    }
}

impl From<StoreError> for DispatchError {
    fn from(e: StoreError) -> Self {
        let code = match e {
            StoreError::NotFound { .. } => ReplyErrorCode::NotFound,
            StoreError::Conflict { .. } => ReplyErrorCode::TransitionRejected,
            _ => ReplyErrorCode::Internal,
        };
        DispatchError {
            code,
            message: e.to_string(),
        }
    }
}

impl From<config_io::ConfigError> for DispatchError {
    fn from(e: config_io::ConfigError) -> Self {
        DispatchError {
            code: ReplyErrorCode::InvalidParams,
            message: e.to_string(),
        }
    }
}

impl From<git::GitError> for DispatchError {
    fn from(e: git::GitError) -> Self {
        DispatchError {
            code: ReplyErrorCode::Internal,
            message: e.to_string(),
        }
    }
}

impl From<crate::protocol::ProtocolError> for DispatchError {
    fn from(e: crate::protocol::ProtocolError) -> Self {
        DispatchError {
            code: ReplyErrorCode::Internal,
            message: e.to_string(),
        }
    }
}

impl From<init::InitError> for DispatchError {
    fn from(e: init::InitError) -> Self {
        DispatchError {
            code: ReplyErrorCode::Internal,
            message: e.to_string(),
        }
    }
}

impl From<scheduler::SchedulerError> for DispatchError {
    fn from(e: scheduler::SchedulerError) -> Self {
        // A rejected trigger is the common case here — "任务不在运行中",
        // "驳回必须附意见" — and the UI renders it as a refusal, not a bug.
        DispatchError {
            code: ReplyErrorCode::TransitionRejected,
            message: e.to_string(),
        }
    }
}

pub type DispatchResult = std::result::Result<(Value, Vec<Event>), DispatchError>;

fn dispatch(ctx: &mut Ctx, command: &Command) -> DispatchResult {
    let p = &command.params;
    match command.method.as_str() {
        // ---- projects ----------------------------------------------------
        "project.list" => project_list(ctx),
        "project.get" => project_get(ctx, str_param(p, "project_id")?),
        "project.probe" => project_probe(ctx, str_param(p, "path")?),
        "project.add" => project_add(
            ctx,
            str_param(p, "path")?,
            bool_param(p, "workspace"),
            opt_str_param(p, "docs_repo").map(str::to_string),
        ),
        "project.remove" => project_remove(ctx, str_param(p, "project_id")?),
        "project.onboarding.advance" => onboarding_step(ctx, str_param(p, "project_id")?, true),
        "project.onboarding.skip" => onboarding_step(ctx, str_param(p, "project_id")?, false),
        "project.onboarding.run" => onboarding_run(ctx, str_param(p, "project_id")?),
        "project.onboarding.artefacts" => onboarding_artefacts(ctx, str_param(p, "project_id")?),
        "project.onboarding.save" => onboarding_save(ctx, p),

        // ---- tasks -------------------------------------------------------
        "task.create" => task_create(ctx, p),
        "task.get" => task_get(ctx, str_param(p, "task_id")?),
        "task.approve" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Approve),
        "task.reject" => task_trigger(
            ctx,
            str_param(p, "task_id")?,
            Trigger::Reject {
                feedback: str_param(p, "feedback")?.to_string(),
            },
        ),
        "task.merge" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Merge),
        "task.pause" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Pause),
        "task.resume" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Resume),
        "task.stop" => task_stop(ctx, str_param(p, "task_id")?),
        "task.cancel" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Cancel),
        "task.extend_budget" => task_trigger(
            ctx,
            str_param(p, "task_id")?,
            Trigger::ExtendBudget {
                extra_rounds: u32_param(p, "extra_rounds")?,
            },
        ),
        "task.retro" => task_retro(ctx, str_param(p, "task_id")?),
        "task.rerun_from" => {
            let raw = str_param(p, "node")?;
            let node = Node::parse(raw).ok_or_else(|| bad_params(format!("未知节点 `{raw}`")))?;
            task_trigger(ctx, str_param(p, "task_id")?, Trigger::RerunFrom { node })
        }
        "task.decide" => task_decide(ctx, p),
        "task.archive" => task_archive(ctx, str_param(p, "task_id")?, true),
        "task.restore" => task_archive(ctx, str_param(p, "task_id")?, false),
        "task.changes" => task_changes(ctx, str_param(p, "task_id")?),
        "session.log" => session_log(ctx, str_param(p, "session_id")?),
        "dashboard.get" => dashboard_get(ctx),

        // ---- scheduler ---------------------------------------------------
        "scheduler.tick" => scheduler_tick(ctx),

        // ---- config ------------------------------------------------------
        "config.get" => config_get(ctx, opt_str_param(p, "project_id")),
        "config.validate" => config_validate(ctx, opt_str_param(p, "project_id")),
        "config.set_role" => config_set_role(ctx, p),
        "config.reset_role" => config_reset_role(ctx, p),
        "config.set_loop" => config_set_loop(ctx, p),
        "config.set_theme" => config_set_theme(ctx, p),

        // ---- protocol ------------------------------------------------
        "protocol.get" => crate::dispatch_protocol::get(ctx),
        "protocol.eval" => crate::dispatch_protocol::eval(ctx),
        "protocol.triggers" => crate::dispatch_protocol::triggers(ctx),
        "protocol.improve" => crate::dispatch_protocol::improve(ctx),

        // ---- curation --------------------------------------------------
        "rules.proposals" => crate::dispatch_curation::proposals(ctx, str_param(p, "project_id")?),
        "rules.decide" => crate::dispatch_curation::decide(ctx, p),
        "rules.retire" => crate::dispatch_curation::retire(ctx, p),
        "rules.restore" => crate::dispatch_curation::restore(ctx, p),
        "protocol.versions" => crate::dispatch_protocol::versions(ctx, str_param(p, "project_id")?),
        "protocol.pin" => crate::dispatch_protocol::pin(ctx, p),
        "protocol.rollback" => crate::dispatch_protocol::rollback(ctx, p),

        // ---- environment -------------------------------------------------
        "env.get" => env_get(ctx),
        "env.detect" => env_detect(ctx, bool_param(p, "force")),
        "env.install_recipe" => env_install_recipe(p),

        // ---- skills ------------------------------------------------------
        "skills.list" => skills_list(ctx, opt_str_param(p, "project_id")),

        // ---- stream ------------------------------------------------------
        "events.since" => events_since(ctx, p),

        other => Err(DispatchError {
            code: ReplyErrorCode::UnknownMethod,
            message: format!("未知命令 {other}"),
        }),
    }
}

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

fn str_param<'a>(params: &'a Value, key: &str) -> std::result::Result<&'a str, DispatchError> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| bad_params(format!("缺少参数 `{key}`")))
}

fn opt_str_param<'a>(params: &'a Value, key: &str) -> Option<&'a str> {
    params.get(key).and_then(Value::as_str)
}

/// An absent or non-boolean value is `false`. Only `env.detect`'s `force`
/// uses this, and defaulting it to "do the expensive thing" would defeat the
/// rate limit for any caller that forgot to send it.
fn bool_param(params: &Value, key: &str) -> bool {
    params.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn u32_param(params: &Value, key: &str) -> std::result::Result<u32, DispatchError> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .map(|v| v as u32)
        .ok_or_else(|| bad_params(format!("缺少参数 `{key}`")))
}

fn role_param(params: &Value, key: &str) -> std::result::Result<Role, DispatchError> {
    let raw = str_param(params, key)?;
    Role::parse(raw).ok_or_else(|| bad_params(format!("未知角色 `{raw}`")))
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

fn project_list(ctx: &mut Ctx) -> DispatchResult {
    let projects = ctx.store.list_projects()?;
    let mut out = Vec::new();
    for p in projects {
        let tasks = ctx.store.list_tasks(&p.id)?;
        out.push(json!({
            "project": p,
            "counts": crate::store::task_counts(&tasks),
            "slots_in_use": tasks.iter().filter(|t| t.state.occupies_slot()).count(),
        }));
    }
    Ok((json!({ "projects": out }), vec![]))
}

fn project_get(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let project = ctx.store.get_project(project_id)?;
    let tasks = ctx.store.list_tasks(project_id)?;
    let global = config_io::load_global(&ctx.autome_home)?;
    let project_config = config_io::load_project(std::path::Path::new(&project.path))?;
    let resolved = config::resolve(&global, &project_config);
    let home = ctx.home.to_string_lossy().into_owned();
    let inventory = skills::scan(&home, &project.path);
    let violations = config::validate(&resolved, &inventory);

    Ok((
        json!({
            "project": project,
            "tasks": tasks.iter().map(task_json).collect::<Vec<_>>(),
            "counts": crate::store::task_counts(&tasks),
            "config": resolved,
            "violations": violations,
            "rules": rule_files(&project.path),
        }),
        vec![],
    ))
}

fn task_json(t: &crate::store::TaskRecord) -> Value {
    json!({
        "id": t.id,
        "slug": t.slug,
        "title": t.title,
        "request": t.request,
        "state": t.state,
        "budget_n": t.budget_n,
        "branch": t.branch(),
        "created_at": t.created_at,
        "completed_at": t.completed_at,
        "merge_commit": t.merge_commit,
        "archived": t.is_archived(),
        "protocol_ref": t.protocol_ref,
        "metrics": t.metrics,
    })
}

/// The rule files an agent is told to obey (requirement C-10).
fn rule_files(repo: &str) -> Vec<String> {
    let mut out = Vec::new();
    if std::path::Path::new(repo).join("AGENTS.md").exists() {
        out.push("AGENTS.md".to_string());
    }
    let rules = std::path::Path::new(repo).join(".autome/rules");
    if let Ok(entries) = std::fs::read_dir(&rules) {
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        names.sort();
        out.extend(names.into_iter().map(|n| format!(".autome/rules/{n}")));
    }
    out
}

/// Adds a directory as a project (requirements P-01, P-02, C-01, C-03).
///
/// The whole sequence is here rather than split across layers because it has
/// to be all-or-nothing from the user's point of view: they picked a
/// directory, and either it is now a project or nothing happened.
fn project_add(
    ctx: &mut Ctx,
    raw_path: &str,
    workspace: bool,
    docs_repo: Option<String>,
) -> DispatchResult {
    let path = std::path::PathBuf::from(shellexpand_home(raw_path, &ctx.home));

    if path.exists() && !path.is_dir() {
        return Err(rejected(format!("{} 不是目录", path.display())));
    }

    let existed = path.exists();
    if !existed {
        std::fs::create_dir_all(&path)
            .map_err(|e| internal(format!("无法创建目录 {}：{e}", path.display())))?;
    }
    let canonical = path
        .canonicalize()
        .map_err(|e| internal(format!("无法解析路径：{e}")))?;
    let canonical_str = canonical.to_string_lossy().into_owned();

    if let Some(existing) = ctx.store.project_by_path(&canonical_str)?
        && existing.is_active()
    {
        return Err(rejected("该目录已经是一个项目".to_string()));
    }

    // Nested projects would make `.worktree/` and `.autome/` ambiguous.
    for other in ctx.store.list_projects()? {
        let other_path = std::path::Path::new(&other.path);
        if canonical.starts_with(other_path) || other_path.starts_with(&canonical) {
            return Err(rejected(format!(
                "{} 与已有项目「{}」嵌套，不能同时管理",
                canonical.display(),
                other.display_name
            )));
        }
    }

    let was_repo = git::is_repo_root(&canonical);
    let members = discover_members(&canonical);

    // A workspace is adopted, never initialised. `git init` over a directory
    // whose children are repositories produces an outer repository holding
    // each of them as a gitlink, and every later `git add child/…` fails with
    // "is in submodule" — which is how a real directory lost its first task.
    let (disposition, default_branch) = if workspace {
        let docs = docs_repo
            .as_deref()
            .ok_or_else(|| bad_params("工作区项目要指定一个文档仓库"))?;
        let member = members
            .iter()
            .find(|m| m.name == docs)
            .ok_or_else(|| bad_params(format!("{docs} 不是这个目录下的仓库")))?;
        (
            AddDisposition::AdoptedWorkspace,
            member.default_branch.clone(),
        )
    } else {
        if !was_repo {
            git::init(&canonical, "main")?;
        }
        let branch = git::default_branch(&canonical)
            .ok_or_else(|| rejected(format!("{} 没有可用的默认分支", canonical.display())))?;
        let disposition = match (existed, was_repo) {
            (false, _) => AddDisposition::CreatedAndInitialised,
            (true, false) => AddDisposition::InitialisedExisting,
            (true, true) => AddDisposition::AdoptedExisting,
        };
        (disposition, branch)
    };

    // A brand-new project has no pin yet, so this resolves to the newest tag.
    // It still goes through the resolver so that adopting a directory whose
    // `.autome/config.toml` already pins a version honours that pin.
    let (protocol_ref, protocol) =
        crate::protocol::resolve_for_project(&ctx.autome_home, &canonical)?;
    let report = init::init_kind(&canonical, &protocol, workspace)?;

    let global = config_io::load_global(&ctx.autome_home)?;
    let project = Project {
        id: new_id("prj"),
        path: canonical_str.clone(),
        display_name: project::display_name_from_path(&canonical_str),
        default_branch,
        parallel_limit: global.loop_defaults.parallel,
        onboarding: Onboarding::start(),
        disposition,
        added_at: now_iso(),
        removed_at: None,
        kind: if workspace {
            ProjectKind::Workspace
        } else {
            ProjectKind::Repo
        },
        members: if workspace { members } else { Vec::new() },
        docs_repo: if workspace { docs_repo } else { None },
    };
    ctx.store.insert_project(&project)?;

    let seq = ctx.store.append_event(
        "project.added",
        &project.id,
        json!({ "path": project.path, "disposition": project.disposition }),
    )?;

    Ok((
        json!({
            "project": project,
            "protocol_ref": protocol_ref.to_wire(),
            "init": report.steps.iter().map(|s| json!({
                "path": s.path,
                "action": format!("{:?}", s.action).to_lowercase(),
            })).collect::<Vec<_>>(),
            "pending_commit": report.paths_to_commit(),
        }),
        vec![event(seq, "project.added", &project.id, json!({}))],
    ))
}

/// This project's document root: what its configuration says, or its own
/// default.
///
/// Validated here rather than at load: a malformed `doc_root` must stop the
/// one task being created, not make the whole project unreadable. The user
/// sees the sentence while they are looking at the thing they just typed.
fn resolve_doc_root(
    project: &Project,
    config: &autome_domain::config::ProjectConfig,
) -> Result<String, DispatchError> {
    match config.doc_root.as_deref() {
        Some(root) => {
            project::validate_doc_root(root).map_err(bad_params)?;
            Ok(root.trim_end_matches('/').to_string())
        }
        None => Ok(project.default_doc_root().to_string()),
    }
}

/// The repositories directly inside `dir`, in name order.
///
/// Immediate children only, and only real repository roots. Walking deeper
/// would find a repository vendored inside another project's `node_modules`
/// and offer it as a place to put task documents; one level is the layout
/// people actually use for "several repositories I develop across", and it is
/// the layout that can be explained in one sentence in the UI.
///
/// Name order rather than filesystem order so that the picker, the stored
/// member list and the merge report all agree, on every machine.
fn discover_members(dir: &std::path::Path) -> Vec<project::Member> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|name| !name.starts_with('.'))
        .filter(|name| git::is_repo_root(&dir.join(name)))
        .collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let default_branch =
                git::default_branch(&dir.join(&name)).unwrap_or_else(|| "main".to_string());
            project::Member {
                name,
                default_branch,
            }
        })
        .collect()
}

/// What a directory looks like, before anything is done to it.
///
/// Read-only and side-effect free, which is the point: `project.add` used to
/// be the first moment anyone learned what the directory was, and by then it
/// had already been `git init`-ed. The window asks this first and puts the
/// answer in front of the user.
///
/// The suggestion is a suggestion. A directory that is *already* a repository
/// and *also* holds several — which is exactly what a wrongly-initialised
/// workspace looks like — is reported as both, because only the user knows
/// which one they meant.
fn project_probe(ctx: &Ctx, raw_path: &str) -> DispatchResult {
    let path = std::path::PathBuf::from(shellexpand_home(raw_path, &ctx.home));
    let exists = path.is_dir();
    let is_repo_root = exists && git::is_repo_root(&path);
    let members = if exists {
        discover_members(&path)
    } else {
        Vec::new()
    };
    let suggest_workspace = !is_repo_root && members.len() >= 2;
    Ok((
        json!({
            "path": path.to_string_lossy(),
            "exists": exists,
            "is_repo_root": is_repo_root,
            "members": members,
            "suggest_workspace": suggest_workspace,
            "suggested_docs_repo": members
                .iter()
                .find(|m| m.name == project::DEFAULT_DOC_ROOT_REPO)
                .or_else(|| members.first())
                .map(|m| m.name.clone()),
        }),
        vec![],
    ))
}

/// Expands a leading `~`, which is what the user sees in the picker and what
/// they paste from a terminal.
fn shellexpand_home(path: &str, home: &std::path::Path) -> String {
    let home = home.to_string_lossy();
    if let Some(rest) = path.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else if path == "~" {
        home.into_owned()
    } else {
        path.to_string()
    }
}

fn project_remove(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let running: Vec<String> = ctx
        .store
        .list_tasks(project_id)?
        .into_iter()
        .filter(|t| !t.state.is_terminal())
        .map(|t| t.id)
        .collect();
    if !running.is_empty() {
        return Err(rejected(format!(
            "still-running tasks must finish or be cancelled first: {}",
            running.join(", ")
        )));
    }
    ctx.store.remove_project(project_id)?;
    let seq = ctx
        .store
        .append_event("project.removed", project_id, json!({}))?;
    Ok((
        json!({ "removed": project_id }),
        vec![event(seq, "project.removed", project_id, json!({}))],
    ))
}

/// Advances or skips the onboarding wizard. Both terminal outcomes make the
/// init commit (requirement C-03), which is why they share a function: the
/// commit must happen exactly once, on whichever path finishes first.
fn onboarding_step(ctx: &mut Ctx, project_id: &str, advance: bool) -> DispatchResult {
    let project = ctx.store.get_project(project_id)?;
    let next = if advance {
        project
            .onboarding
            .advance()
            .ok_or_else(|| rejected("Onboarding 已经结束"))?
    } else {
        Onboarding::Skipped
    };
    ctx.store.update_project_onboarding(project_id, next)?;

    let mut committed = None;
    if next.is_settled() && !project.onboarding.is_settled() {
        committed = init_commit(&project)?;
    }

    let seq = ctx.store.append_event(
        "project.onboarding",
        project_id,
        json!({ "onboarding": next, "init_commit": committed }),
    )?;
    Ok((
        json!({ "onboarding": next, "init_commit": committed }),
        vec![event(seq, "project.updated", project_id, json!({}))],
    ))
}

/// Starts the Onboarding session (requirement C-02 step 3). Returns the
/// session id so the UI can poll for it finishing and show the log.
fn onboarding_run(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let project = ctx.store.get_project(project_id)?;
    if project.onboarding.is_settled() {
        return Err(rejected("Onboarding 已经结束"));
    }
    let session_id = scheduler::start_onboarding(ctx, project_id)?;
    Ok((
        json!({ "session_id": session_id, "project_id": project_id }),
        vec![],
    ))
}

/// The two files Onboarding produces, for the in-app confirm-and-edit step
/// (requirement C-02 step 4). Reading them here rather than making the
/// renderer open a file keeps the renderer without filesystem access.
fn onboarding_artefacts(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let project = ctx.store.get_project(project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    let files: Vec<Value> = ONBOARDING_FILES
        .iter()
        .map(|rel| {
            let path = repo.join(rel);
            json!({
                "path": rel,
                "exists": path.exists(),
                "content": std::fs::read_to_string(&path).unwrap_or_default(),
            })
        })
        .collect();

    Ok((json!({ "files": files }), vec![]))
}

/// The two artefacts, in the order the wizard presents them.
const ONBOARDING_FILES: [&str; 2] = ["docs/agent-project-profile.md", "AGENTS.md"];

/// Saves an edited artefact.
///
/// Refuses anything outside the two known files: this is a write into the
/// user's repository driven by the renderer, and the renderer must not be able
/// to choose the target.
fn onboarding_save(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let project_id = str_param(params, "project_id")?;
    let rel = str_param(params, "path")?;
    if !ONBOARDING_FILES.contains(&rel) {
        return Err(bad_params(format!(
            "只能编辑 {}",
            ONBOARDING_FILES.join(" 与 ")
        )));
    }
    let content = str_param(params, "content")?;
    let project = ctx.store.get_project(project_id)?;
    let path = std::path::Path::new(&project.path).join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| internal(format!("无法创建目录：{e}")))?;
    }
    std::fs::write(&path, content).map_err(|e| internal(format!("无法写入 {rel}：{e}")))?;
    let seq = ctx
        .store
        .append_event("project.onboarding", project_id, json!({ "saved": rel }))?;
    Ok((
        json!({ "saved": rel }),
        vec![event(seq, "project.updated", project_id, json!({}))],
    ))
}

/// The one commit Autome makes on the default branch without being asked
/// (design §6, requirement C-03).
fn init_commit(project: &Project) -> std::result::Result<Option<String>, DispatchError> {
    let repo = std::path::Path::new(&project.path);
    let mut paths: Vec<String> = vec![
        ".autome".into(),
        ".gitignore".into(),
        "AGENTS.md".into(),
        "docs".into(),
    ];
    paths.retain(|p| repo.join(p).exists());
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    Ok(git::commit_paths(
        repo,
        &refs,
        "chore(autome): initialise project",
    )?)
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Loads the global config plus, when a project is named, its overrides —
/// returning the resolved view, the provenance of each field, and any
/// violations. One call, because the UI needs all three together and deriving
/// them separately invites them to disagree.
/// The window's appearance. Global only — it is not a per-project idea, and a
/// project that could repaint the app when you opened it would be a surprise.
fn config_set_theme(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let raw = str_param(params, "theme")?;
    let theme = autome_domain::config::Theme::parse(raw)
        .ok_or_else(|| bad_params(format!("未知外观 `{raw}`，只能是 system / light / dark")))?;
    let mut global = config_io::load_global(&ctx.autome_home)?;
    global.ui.theme = theme;
    config_io::save_global(&ctx.autome_home, &global)?;
    let seq = ctx.store.append_event(
        "config.updated",
        "global",
        json!({ "theme": theme.as_str() }),
    )?;
    Ok((
        json!({ "theme": theme.as_str() }),
        vec![event(seq, "config.updated", "global", json!({}))],
    ))
}

fn config_get(ctx: &mut Ctx, project_id: Option<&str>) -> DispatchResult {
    let global = config_io::load_global(&ctx.autome_home)?;
    let (project_config, repo) = match project_id {
        Some(id) => {
            let p = ctx.store.get_project(id)?;
            (
                config_io::load_project(std::path::Path::new(&p.path))?,
                Some(p.path),
            )
        }
        None => (ProjectConfig::default(), None),
    };
    let resolved = config::resolve(&global, &project_config);
    let home = ctx.home.to_string_lossy().into_owned();
    let inventory = match &repo {
        Some(path) => skills::scan(&home, path),
        None => skills::scan_global(&home),
    };
    let violations = config::validate(&resolved, &inventory);
    Ok((
        json!({
            "global": global,
            "resolved": resolved,
            "violations": violations,
            "scope": project_id,
        }),
        vec![],
    ))
}

fn config_validate(ctx: &mut Ctx, project_id: Option<&str>) -> DispatchResult {
    let (payload, _) = config_get(ctx, project_id)?;
    Ok((
        json!({
            "violations": payload.get("violations").cloned().unwrap_or(Value::Null),
            "ok": payload
                .get("violations")
                .and_then(Value::as_array)
                .is_some_and(|a| a.is_empty()),
        }),
        vec![],
    ))
}

/// Writes one role's settings, to the global file or a project's overrides.
///
/// The write is refused when it would produce an invalid configuration
/// (requirement C-06, S-04): a SAME-MODEL collision or a skill the role's
/// runtime cannot see. That check happens here, against the *resolved* view,
/// not against the patch — a project override can collide with a global value
/// it does not itself mention.
fn config_set_role(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let role = role_param(params, "role")?;
    let project_id = opt_str_param(params, "project_id");
    let global = config_io::load_global(&ctx.autome_home)?;

    let enabled = params.get("enabled").and_then(Value::as_bool);
    let runtime = match params.get("runtime").and_then(Value::as_str) {
        Some(r) => {
            Some(Runtime::parse(r).ok_or_else(|| bad_params(format!("未知 runtime `{r}`")))?)
        }
        None => None,
    };
    let model = params
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_string);
    // `effort: null` is meaningful — "use the CLI default" — and distinct from
    // the key being absent, which means "leave as is".
    let effort = if params.get("effort").is_some() {
        Some(
            params
                .get("effort")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        )
    } else {
        None
    };
    let skills_list = params.get("skills").and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>()
    });

    match project_id {
        None => {
            let mut next = global.clone();
            let cfg = next
                .roles
                .get_mut(&role)
                .ok_or_else(|| internal("global config missing a role"))?;
            if let Some(v) = enabled {
                cfg.enabled = v;
            }
            if let Some(v) = runtime {
                cfg.runtime = v;
            }
            if let Some(v) = model {
                cfg.model = v;
            }
            if let Some(v) = effort {
                cfg.effort = v;
            }
            if let Some(v) = skills_list {
                cfg.skills = v;
            }
            let resolved = config::resolve(&next, &ProjectConfig::default());
            let inventory = skills::scan_global(&ctx.home.to_string_lossy());
            reject_if_invalid(&config::validate(&resolved, &inventory))?;
            config_io::save_global(&ctx.autome_home, &next)?;
        }
        Some(id) => {
            let p = ctx.store.get_project(id)?;
            let repo = std::path::Path::new(&p.path);
            let mut project_config = config_io::load_project(repo)?;
            let over = project_config
                .roles
                .entry(role)
                .or_insert_with(RoleOverrides::default);
            if let Some(v) = enabled {
                over.enabled = Some(v);
            }
            if let Some(v) = runtime {
                over.runtime = Some(v);
            }
            if let Some(v) = model {
                over.model = Some(v);
            }
            if let Some(v) = effort {
                over.effort = Some(v);
            }
            if let Some(v) = skills_list {
                over.skills = Some(v);
            }
            project_config.prune();
            let resolved = config::resolve(&global, &project_config);
            let inventory = skills::scan(&ctx.home.to_string_lossy(), &p.path);
            reject_if_invalid(&config::validate(&resolved, &inventory))?;
            config_io::save_project(repo, &project_config)?;
        }
    }

    let subject = project_id.unwrap_or("global");
    let seq = ctx
        .store
        .append_event("config.changed", subject, json!({ "role": role }))?;
    let (payload, _) = config_get(ctx, project_id)?;
    Ok((
        payload,
        vec![event(seq, "config.changed", subject, json!({}))],
    ))
}

/// Restores a role to inheritance by *removing* its override (requirement
/// C-07). Only meaningful for a project.
fn config_reset_role(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let role = role_param(params, "role")?;
    let project_id = str_param(params, "project_id")?;
    let p = ctx.store.get_project(project_id)?;
    let repo = std::path::Path::new(&p.path);
    let mut project_config = config_io::load_project(repo)?;
    project_config.roles.remove(&role);
    config_io::save_project(repo, &project_config)?;
    let seq = ctx.store.append_event(
        "config.changed",
        project_id,
        json!({ "role": role, "reset": true }),
    )?;
    let (payload, _) = config_get(ctx, Some(project_id))?;
    Ok((
        payload,
        vec![event(seq, "config.changed", project_id, json!({}))],
    ))
}

fn config_set_loop(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let project_id = opt_str_param(params, "project_id");
    let parallel = params
        .get("parallel")
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    let design_rounds = params
        .get("design_rounds")
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    let budget_factor = params
        .get("budget_factor")
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    let global = config_io::load_global(&ctx.autome_home)?;

    match project_id {
        None => {
            let mut next = global.clone();
            if let Some(v) = parallel {
                next.loop_defaults.parallel = v;
            }
            if let Some(v) = design_rounds {
                next.loop_defaults.design_rounds = v;
            }
            if let Some(v) = budget_factor {
                next.loop_defaults.budget_factor = v;
            }
            let resolved = config::resolve(&next, &ProjectConfig::default());
            reject_if_invalid(&config::validate(&resolved, &config::NoSkills))?;
            config_io::save_global(&ctx.autome_home, &next)?;
        }
        Some(id) => {
            let p = ctx.store.get_project(id)?;
            let repo = std::path::Path::new(&p.path);
            let mut project_config = config_io::load_project(repo)?;
            if params.get("parallel").is_some() {
                project_config.loop_overrides.parallel = parallel;
            }
            if params.get("design_rounds").is_some() {
                project_config.loop_overrides.design_rounds = design_rounds;
            }
            if params.get("budget_factor").is_some() {
                project_config.loop_overrides.budget_factor = budget_factor;
            }
            let resolved = config::resolve(&global, &project_config);
            reject_if_invalid(&config::validate(&resolved, &config::NoSkills))?;
            config_io::save_project(repo, &project_config)?;
            // The parallel limit is also cached on the project row, because
            // the scheduler reads it on every slot decision.
            ctx.store
                .update_project_parallel(id, resolved.loop_defaults.parallel)?;
        }
    }

    let subject = project_id.unwrap_or("global");
    let seq = ctx
        .store
        .append_event("config.changed", subject, json!({}))?;
    let (payload, _) = config_get(ctx, project_id)?;
    Ok((
        payload,
        vec![event(seq, "config.changed", subject, json!({}))],
    ))
}

/// Turns violations into a refusal the UI can render field by field.
fn reject_if_invalid(violations: &[ConfigViolation]) -> std::result::Result<(), DispatchError> {
    if violations.is_empty() {
        return Ok(());
    }
    let message = violations
        .iter()
        .map(describe_violation)
        .collect::<Vec<_>>()
        .join("；");
    Err(DispatchError {
        code: ReplyErrorCode::TransitionRejected,
        message,
    })
}

fn describe_violation(v: &ConfigViolation) -> String {
    match v {
        ConfigViolation::SameModel {
            evaluator,
            generator,
            identity,
        } => format!(
            "{} 与 {} 都配成了 {identity}；评测侧必须与生成侧不同模型",
            role_label(*evaluator),
            role_label(*generator)
        ),
        ConfigViolation::SkillNotVisible {
            role,
            skill,
            runtime,
        } => format!(
            "{} 用 {}，看不到技能 {skill}",
            role_label(*role),
            runtime.display_name()
        ),
        ConfigViolation::SkillNotFound { role, skill } => {
            format!("{} 绑定了不存在的技能 {skill}", role_label(*role))
        }
        ConfigViolation::ParallelOutOfRange { value, min, max } => {
            format!("并行数 {value} 超出范围 {min}..={max}")
        }
        ConfigViolation::DesignRoundsOutOfRange { value } => {
            format!("设计轮次上限 {value} 必须大于 0")
        }
        ConfigViolation::BudgetFactorOutOfRange { value } => {
            format!("预算系数 {value} 必须大于 0")
        }
        ConfigViolation::BothDefaultModels {
            evaluator,
            generator,
        } => format!(
            "{} 与 {} 都没有指定模型，会用同一个 CLI 的同一个默认模型；请至少给一边指定模型",
            role_label(*evaluator),
            role_label(*generator)
        ),
    }
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::Plan => "设计",
        Role::Review => "评审",
        Role::Adjudicate => "裁决",
        Role::Impl => "实现",
        Role::Audit => "审计",
        Role::Retro => "复盘",
    }
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

/// Creates a task and puts it in the queue (requirements T-01, T-02).
///
/// Submission starts the work: there is no second confirmation, because the
/// first stopping point (design approval) is close enough that an extra one
/// would only add friction.
fn task_create(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let project_id = str_param(params, "project_id")?;
    let request = str_param(params, "request")?.trim().to_string();
    if request.is_empty() {
        return Err(bad_params("需求不能为空"));
    }
    let project = ctx.store.get_project(project_id)?;
    if !project.is_active() {
        return Err(rejected("项目已移除"));
    }

    let attachments: Vec<String> = params
        .get("attachments")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    for a in &attachments {
        if !std::path::Path::new(a).is_file() {
            return Err(bad_params(format!("附件不存在：{a}")));
        }
    }
    let doc_refs: Vec<String> = params
        .get("doc_refs")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    create(ctx, project_id, &request, None, attachments, doc_refs)
}

/// Creates a task with a fixed request and title, for the one caller that has
/// both: `protocol.improve`.
///
/// A meta task's request is not something a user types. The constraints in it
/// — two tasks' worth of evidence, a metric the core records, an eval case for
/// anything behavioural — are what make a proposal checkable, and a user
/// typing their own request would be typing around them.
pub fn task_create_in(
    ctx: &mut Ctx,
    project_id: &str,
    request: &str,
    title: &str,
) -> DispatchResult {
    create(ctx, project_id, request, Some(title), vec![], vec![])
}

fn create(
    ctx: &mut Ctx,
    project_id: &str,
    request: &str,
    title: Option<&str>,
    attachments: Vec<String>,
    doc_refs: Vec<String>,
) -> DispatchResult {
    let project = ctx.store.get_project(project_id)?;
    // A workspace task means one branch per repository it touches, which the
    // loop does not do yet. Refusing it by name beats starting one that
    // cannot finish — which is how this whole line of work began.
    if project.is_workspace() {
        return Err(rejected(
            "工作区项目还不能接任务：多仓任务（每个仓一条分支、一次合并）还没接通",
        ));
    }
    let doc_root = resolve_doc_root(
        &project,
        &config_io::load_project(std::path::Path::new(&project.path))?,
    )?;

    let base = autome_domain::project::slugify(title.unwrap_or(request));
    let slug = {
        let taken: Vec<String> = ctx
            .store
            .list_tasks(project_id)?
            .into_iter()
            .map(|t| t.slug)
            .collect();
        autome_domain::project::unique_slug(&base, |s| taken.iter().any(|t| t == s))
    };
    let id = ctx.store.next_task_id(project_id)?;

    let task = crate::store::TaskRecord {
        id: id.clone(),
        project_id: project_id.to_string(),
        slug,
        // The intake session writes the real title; until then the request
        // itself is the most informative thing to show.
        title: title
            .map(str::to_string)
            .unwrap_or_else(|| first_line(request)),
        request: request.to_string(),
        attachments,
        doc_refs,
        state: TaskState::Queued,
        budget_n: None,
        created_at: now_iso(),
        completed_at: None,
        merge_commit: None,
        archived_at: None,
        // All three are filled in by the scheduler: the protocol version and
        // the rules hash when the first session starts, the metrics when the
        // task reaches a terminal state.
        protocol_ref: None,
        rules_hash: None,
        metrics: None,
        // Frozen here, like the protocol version: the documents this task is
        // about to create are committed at these paths, and a setting changed
        // halfway through must not split them across two directories.
        doc_root: Some(doc_root),
    };
    ctx.store.insert_task(&task)?;
    let seq = ctx
        .store
        .append_event("task.created", &id, json!({ "project_id": project_id }))?;

    // Start it immediately if the project has a free slot.
    let report = scheduler::tick(ctx);
    let started = report.tasks_started.contains(&id);

    let task = ctx.store.get_task(&id)?;
    Ok((
        json!({
            "task": task_json(&task),
            "started": started,
            "queue_position": queue_position(ctx, &task)?,
        }),
        vec![event(seq, "task.created", &id, json!({}))],
    ))
}

fn first_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or(s).trim();
    let truncated: String = line.chars().take(40).collect();
    if truncated.chars().count() < line.chars().count() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

/// 1-based position in the project's queue, or `None` when not queued.
fn queue_position(
    ctx: &Ctx,
    task: &crate::store::TaskRecord,
) -> std::result::Result<Option<usize>, DispatchError> {
    if !matches!(task.state, TaskState::Queued) {
        return Ok(None);
    }
    Ok(ctx
        .store
        .queued_tasks(&task.project_id)?
        .iter()
        .position(|t| t.id == task.id)
        .map(|i| i + 1))
}

/// The whole task panel in one call (requirement U-06): state, progress from
/// the design document, sessions, decisions and produced files.
/// `task.retro` — run a retro round on a task that has already stopped.
fn task_retro(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    scheduler::start_retro(ctx, task_id)?;
    let seq = ctx
        .store
        .append_event("task.retro_started", task_id, json!({}))?;
    Ok((
        json!({ "started": true }),
        vec![event(seq, "task.retro_started", task_id, json!({}))],
    ))
}

fn task_get(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    let task = ctx.store.get_task(task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    let worktree = repo.join(".worktree").join(&task.slug);

    let doc = read_status_block(&worktree, &task);
    let status = doc.block();
    let sessions = ctx.store.list_sessions(task_id)?;
    let decisions = ctx.store.list_decisions(task_id)?;

    Ok((
        json!({
            "task": task_json(&task),
            "project": { "id": project.id, "name": project.display_name, "default_branch": project.default_branch },
            "queue_position": queue_position(ctx, &task)?,
            "status_block": status.map(status_json),
            "status_error": doc.error().is_some(),
            "status_error_detail": doc.error(),
            "sessions": sessions.iter().map(session_json).collect::<Vec<_>>(),
            "decisions": decisions.iter().map(decision_json).collect::<Vec<_>>(),
            "pending_decisions": ctx.store.pending_decisions(task_id)?,
            "documents": documents(&worktree, &task),
            "worktree": worktree.to_string_lossy(),
            "next_role": scheduler::next_role(&task.state),
            // Empty for every ordinary task. For a meta task it names the
            // changes the review and audit rounds were not competent to judge,
            // and both stopping points show it.
            "needs_human_approval": scheduler::needs_human_approval(ctx, task_id)?,
            // Running totals, for a task still in flight. `task.metrics` is
            // the final aggregate and only exists once the task has ended.
            "so_far": running_totals(&sessions),
        }),
        vec![],
    ))
}

/// What reading the task's design document produced.
///
/// The two failures are kept apart because the user can only act on one of
/// them. "Not written yet" is the normal state of a task whose design round
/// has not run; "written but unreadable" is a defect in a specific line of a
/// specific file, and the panel can only say which line if this type carries
/// the message that far.
///
/// Collapsing both into `None` is what the panel used to get. It offered two
/// wordings for the two cases and could never reach the second, because the
/// flag it switched on was `status.is_none()` — true for both.
enum DocRead {
    /// No design document on disk yet.
    Absent,
    /// The document exists and `parse` rejected it. Carries the rendered
    /// `ParseError`, which names the field or the row and line number.
    Unreadable(String),
    Ok(Box<StatusBlock>),
}

impl DocRead {
    fn block(&self) -> Option<&StatusBlock> {
        match self {
            DocRead::Ok(b) => Some(b),
            _ => None,
        }
    }

    /// The parse failure, if that is why there is no status block. `None` both
    /// when the document parsed and when there is no document.
    fn error(&self) -> Option<&str> {
        match self {
            DocRead::Unreadable(message) => Some(message),
            _ => None,
        }
    }
}

/// Reads the design document from the task's worktree. The panel shows the
/// node flow whatever this returns; a parse failure has already failed the
/// task through the normal path, and this is how the panel explains it.
fn read_status_block(worktree: &std::path::Path, task: &crate::store::TaskRecord) -> DocRead {
    let Ok(text) = std::fs::read_to_string(worktree.join(task.design_doc())) else {
        return DocRead::Absent;
    };
    match status_block::parse(&text) {
        Ok(block) => DocRead::Ok(Box::new(block)),
        Err(error) => DocRead::Unreadable(error.to_string()),
    }
}

fn status_json(s: &StatusBlock) -> Value {
    json!({
        "status": s.status,
        "design_round": s.design_round,
        "design_round_limit": s.design_round_limit,
        "impl_round": s.impl_round,
        "impl_round_limit": s.impl_round_limit,
        "current_milestone": s.current_milestone,
        "current_milestone_reopens": s.current_milestone_reopens,
        "convergence_mode": s.convergence_mode,
        "next_action": s.next_action,
        "milestones": s.milestones,
        "milestones_done": s.milestones_done(),
        "milestones_total": s.milestones.len(),
    })
}

fn session_json(s: &autome_domain::session::Session) -> Value {
    json!({
        "id": s.id,
        "kind": s.kind,
        "label": s.kind.label(),
        "runtime": s.runtime,
        "model": s.model,
        "effort": s.effort,
        "skills": s.skills,
        "round": s.round,
        "started_at": s.started_at,
        "ended_at": s.ended_at,
        "lifecycle": s.lifecycle,
        "running": s.is_running(),
        "protocol_ref": s.protocol_ref,
        // Absent rather than zeroed when the stream could not be read: a card
        // showing "0 tokens" for a session that produced work is worse than a
        // card showing nothing.
        "metrics": (!s.metrics.is_empty()).then(|| s.metrics.clone()),
    })
}

/// What a task has spent so far, summed across its finished sessions.
///
/// Cost is `null` unless at least one session reported one. A task run
/// entirely on Codex has an unknown cost, not a zero one — Codex reports no
/// price, and a table of prices we maintained ourselves would produce a number
/// that looks authoritative and is not.
fn running_totals(sessions: &[autome_domain::session::Session]) -> Value {
    let mut tokens = 0u64;
    let mut turns = 0u64;
    let mut cost: Option<f64> = None;
    let mut measured = 0u32;
    for s in sessions {
        if s.metrics.is_empty() {
            continue;
        }
        measured += 1;
        tokens += s.metrics.total_tokens().unwrap_or(0);
        turns += s.metrics.turns.unwrap_or(0);
        if let Some(c) = s.metrics.cost_usd {
            cost = Some(cost.unwrap_or(0.0) + c);
        }
    }
    json!({
        "sessions_measured": measured,
        "total_tokens": tokens,
        "total_turns": turns,
        "total_cost_usd": cost,
    })
}

fn decision_json(d: &crate::store::DecisionRecord) -> Value {
    json!({
        "kind": d.kind,
        "item_id": d.item_id,
        "text": d.text,
        "disposition": d.disposition,
        "ruling": d.ruling,
        "consumed": d.consumed_at.is_some(),
    })
}

/// The five protocol documents plus the task file, with their timestamps —
/// the panel's "产物" card (requirement T-13).
fn documents(worktree: &std::path::Path, task: &crate::store::TaskRecord) -> Vec<Value> {
    let dir = task.doc_dir();
    let slug = &task.slug;
    let names = [
        (format!("{slug}.md"), "设计文档"),
        (format!("{slug}-task.md"), "任务文件"),
        (format!("{slug}-review.md"), "评审"),
        (format!("{slug}-adjudication.md"), "裁决"),
        (format!("{slug}-audit.md"), "审计"),
        ("retro.md".to_string(), "运行记录"),
    ];
    let mut out: Vec<Value> = names
        .iter()
        .filter_map(|(name, label)| {
            let path = worktree.join(&dir).join(name);
            let meta = std::fs::metadata(&path).ok()?;
            Some(json!({
                "name": name,
                "label": label,
                "path": format!("{dir}/{name}"),
                "absolute": path.to_string_lossy(),
                "size": meta.len(),
            }))
        })
        .collect();

    // The per-round evidence files. Since the 2026-09-16 audit these carry
    // what the design document used to accumulate — the commands and their
    // results, the self-check, the audit's independent re-verification — so
    // leaving them out of this list would move the substance of every round
    // somewhere the panel cannot open. Main resolves an open by matching the
    // name against this list, and these names cannot collide with the six
    // above.
    let evidence = worktree.join(&dir).join("evidence");
    if let Ok(entries) = std::fs::read_dir(&evidence) {
        let mut found: Vec<(String, u64, String)> = entries
            .filter_map(|e| {
                let entry = e.ok()?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name.ends_with(".md") {
                    return None;
                }
                let size = entry.metadata().ok()?.len();
                Some((name, size, entry.path().to_string_lossy().into_owned()))
            })
            .collect();
        // By name: the milestone-then-round shape of `M-03-r7.md` sorts into
        // the order the work happened in.
        found.sort_by(|a, b| a.0.cmp(&b.0));
        out.extend(found.into_iter().map(|(name, size, absolute)| {
            json!({
                "name": name,
                "label": "证据",
                "path": format!("{dir}/evidence/{name}"),
                "absolute": absolute,
                "size": size,
            })
        }));
    }
    out
}

/// Applies a trigger and returns the refreshed panel.
fn task_trigger(ctx: &mut Ctx, task_id: &str, trigger: Trigger) -> DispatchResult {
    scheduler::apply_trigger(ctx, task_id, &trigger)?;
    // A trigger often unblocks a slot or leaves a core step to run.
    scheduler::tick(ctx);
    let seq = ctx.store.latest_seq()?;
    let (payload, _) = task_get(ctx, task_id)?;
    Ok((
        payload,
        vec![event(seq, "task.updated", task_id, json!({}))],
    ))
}

/// Stop kills the running session first, then applies the trigger — the
/// reverse order would leave a process writing to a log for a task the store
/// has already moved on from (requirement T-08).
fn task_stop(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    if let Some(session) = ctx.store.running_session(task_id)? {
        if let Some(pid) = session.pid {
            let _ = crate::launcher::stop_session(pid);
        }
        ctx.store.finish_session(
            &session.id,
            &autome_domain::session::SessionLifecycle::Killed,
            &now_iso(),
        )?;
    }
    task_trigger(ctx, task_id, Trigger::Stop)
}

/// Records the user's disposition of one Backlog item or dispute
/// (requirement T-09). Does not itself advance anything: the decision is
/// consumed at the next stopping point.
fn task_decide(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let task_id = str_param(params, "task_id")?;
    let kind = str_param(params, "kind")?;
    if kind != "backlog" && kind != "dispute" {
        return Err(bad_params("kind 只能是 backlog 或 dispute"));
    }
    let item_id = str_param(params, "item_id")?;
    let raw = str_param(params, "disposition")?;
    let disposition = match raw {
        "none" => Disposition::None,
        "include" => Disposition::Include,
        "ignore" => Disposition::Ignore,
        "ruled" => Disposition::Ruled,
        other => return Err(bad_params(format!("未知处置 `{other}`"))),
    };
    // The two vocabularies do not overlap: a dispute cannot be "included" as
    // a milestone, and a Backlog item is not something to rule on.
    match (kind, disposition) {
        ("backlog", Disposition::Ruled) => {
            return Err(bad_params("Backlog 条目只能纳入或忽略"));
        }
        ("dispute", Disposition::Include | Disposition::Ignore) => {
            return Err(bad_params("争议项只能裁定"));
        }
        _ => {}
    }
    let ruling = params.get("ruling").and_then(Value::as_str);
    ctx.store
        .set_disposition(task_id, kind, item_id, disposition, ruling)?;
    let seq = ctx.store.append_event(
        "task.decided",
        task_id,
        json!({ "item_id": item_id, "disposition": raw }),
    )?;
    let (payload, _) = task_get(ctx, task_id)?;
    Ok((
        payload,
        vec![event(seq, "task.updated", task_id, json!({}))],
    ))
}

/// Archive moves the task's documents into `docs/.archive/` and takes it off
/// the list; restore moves them back (requirement T-12).
///
/// Only a completed task can be archived: an unfinished one still owns a
/// worktree whose documents are on its own branch.
fn task_archive(ctx: &mut Ctx, task_id: &str, archive: bool) -> DispatchResult {
    let task = ctx.store.get_task(task_id)?;
    if archive && task.state != TaskState::Done {
        return Err(rejected("只有已完成的任务可以归档"));
    }
    let project = ctx.store.get_project(&task.project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    let live = repo.join(task.doc_dir());
    let archived = repo.join(task.archive_dir());

    let (from, to) = if archive {
        (&live, &archived)
    } else {
        (&archived, &live)
    };
    if from.exists() {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| internal(format!("无法创建目录：{e}")))?;
        }
        if to.exists() {
            return Err(rejected(format!("目标已存在：{}", to.display())));
        }
        std::fs::rename(from, to).map_err(|e| internal(format!("无法移动任务目录：{e}")))?;
    }
    ctx.store.set_task_archived(task_id, archive)?;
    let seq = ctx.store.append_event(
        if archive {
            "task.archived"
        } else {
            "task.restored"
        },
        task_id,
        json!({}),
    )?;
    Ok((
        json!({
            "task_id": task_id,
            "archived": archive,
            "moved": from.exists() || to.exists(),
            "note": "目录移动只落在默认分支工作树，提交由你自己做",
        }),
        vec![event(seq, "task.updated", task_id, json!({}))],
    ))
}

/// The merge panel's file list and statistics (requirement T-05). Not a
/// line-level diff: the design sends the user to their editor for that.
fn task_changes(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    let task = ctx.store.get_task(task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    let repo = std::path::PathBuf::from(&project.path);
    if !git::branch_exists(&repo, &task.branch()) {
        return Ok((json!({ "available": false }), vec![]));
    }
    let summary = git::change_summary(&repo, &project.default_branch, &task.branch())?;
    let subjects = git::commit_subjects(&repo, &project.default_branch, &task.branch())?;
    // The same judgement `merge_task_branch` makes, not a stricter one: the
    // panel must not tell the user a merge is blocked that the core would
    // happily perform, nor the reverse.
    let blocking = git::conflicting_dirty_paths(&repo, &project.default_branch, &task.branch())
        .unwrap_or_default();
    let on_top = git::is_ancestor(&repo, &project.default_branch, &task.branch()).unwrap_or(false);
    Ok((
        json!({
            "available": true,
            "branch": task.branch(),
            "into": project.default_branch,
            "commits": summary.commits,
            "files": summary.files.iter().map(|f| json!({
                "path": f.path, "added": f.added, "deleted": f.deleted
            })).collect::<Vec<_>>(),
            "total_added": summary.total_added,
            "total_deleted": summary.total_deleted,
            "subjects": subjects,
            "mergeable": blocking.is_empty() && on_top,
            "blocked_by": if !blocking.is_empty() {
                json!({ "kind": "dirty_worktree", "paths": blocking })
            } else if !on_top {
                json!({ "kind": "needs_rebase" })
            } else {
                Value::Null
            },
        }),
        vec![],
    ))
}

/// The tail of a session's log (requirement T-14). Bounded, because a long
/// implementation round can produce megabytes and the panel only shows a tail.
fn session_log(ctx: &mut Ctx, session_id: &str) -> DispatchResult {
    let mut found = None;
    for task in ctx.store.list_unfinished()?.into_iter().chain(
        ctx.store
            .list_projects()?
            .into_iter()
            .flat_map(|p| ctx.store.list_tasks(&p.id).unwrap_or_default()),
    ) {
        if let Some(s) = ctx
            .store
            .list_sessions(&task.id)?
            .into_iter()
            .find(|s| s.id == session_id)
        {
            found = Some(s);
            break;
        }
    }
    let session = found.ok_or_else(|| DispatchError {
        code: ReplyErrorCode::NotFound,
        message: format!("找不到会话 {session_id}"),
    })?;

    const MAX_BYTES: usize = 256 * 1024;
    let text = std::fs::read_to_string(&session.log_path).unwrap_or_default();
    let truncated = text.len() > MAX_BYTES;
    let shown = if truncated {
        // Cut on a character boundary, then on a line boundary, so the panel
        // never renders a broken multi-byte sequence.
        let start = text.len() - MAX_BYTES;
        let start = (start..text.len())
            .find(|i| text.is_char_boundary(*i))
            .unwrap_or(text.len());
        let tail = &text[start..];
        tail.find('\n').map(|i| &tail[i + 1..]).unwrap_or(tail)
    } else {
        &text
    };
    Ok((
        json!({
            "session": session_json(&session),
            "log": shown,
            "truncated": truncated,
            "path": session.log_path,
        }),
        vec![],
    ))
}

/// The dashboard: everything waiting on the user, and everything running,
/// across every project (requirement U-02).
fn dashboard_get(ctx: &mut Ctx) -> DispatchResult {
    let mut waiting = Vec::new();
    let mut running = Vec::new();

    for project in ctx.store.list_projects()? {
        for task in ctx.store.list_tasks(&project.id)? {
            if task.state.is_terminal() {
                continue;
            }
            let pending = ctx.store.pending_decisions(&task.id)?;
            let entry = json!({
                "project_id": project.id,
                "project_name": project.display_name,
                "task": task_json(&task),
                "pending_decisions": pending,
                "session": ctx.store.running_session(&task.id)?.as_ref().map(session_json),
            });
            if task.state.awaits_user() {
                waiting.push(entry);
            } else if task.state.occupies_slot() {
                running.push(entry);
            }
        }
    }

    if ctx.environment.snapshot().is_none() {
        ctx.environment.start_probe();
    }
    // The banner is driven from whatever has been observed. Before the first
    // probe finishes there is nothing to report — which is not the same as
    // reporting that everything is fine, so `probed` says which it is.
    let environment = match ctx.environment.snapshot() {
        Some(env) => json!({
            "probed": true,
            "severity": env.severity(),
            "problems": env.problems().iter().map(|c| json!({
                "component": c.component,
                "name": c.component.display_name(),
                "present": c.present,
                "login": c.login,
            })).collect::<Vec<_>>(),
        }),
        None => json!({ "probed": false, "problems": [] }),
    };

    Ok((
        json!({
            "waiting": waiting,
            "running": running,
            "environment": environment,
        }),
        vec![],
    ))
}

/// Runs one scheduling pass on demand. The Electron shell calls this on a
/// timer; exposing it keeps the polling interval a UI decision rather than a
/// constant baked into the core.
fn scheduler_tick(ctx: &mut Ctx) -> DispatchResult {
    let report = scheduler::tick(ctx);
    Ok((
        json!({
            "sessions_reaped": report.sessions_reaped,
            "tasks_advanced": report.tasks_advanced,
            "tasks_started": report.tasks_started,
            "errors": report.errors,
            "latest_seq": ctx.store.latest_seq()?,
            // The UI polls this; a change means a background probe finished
            // and `env.get` now has something new to say.
            "env_generation": ctx.environment.generation(),
            "env_probing": ctx.environment.is_probing(),
        }),
        vec![],
    ))
}

// ---------------------------------------------------------------------------
// Environment and skills
// ---------------------------------------------------------------------------

/// Returns whatever has been observed, and starts a probe if nothing has.
/// Never waits for one (see `EnvCache`).
fn env_get(ctx: &mut Ctx) -> DispatchResult {
    if ctx.environment.snapshot().is_none() {
        ctx.environment.start_probe();
    }
    Ok((env_json(&ctx.environment), vec![]))
}

/// Asks for a fresh probe. Returns immediately with what is currently known;
/// the caller learns the new result from the next `env.get`, which the UI
/// issues when a tick reports the generation moved.
fn env_detect(ctx: &mut Ctx, force: bool) -> DispatchResult {
    let started = if force {
        ctx.environment.force_probe()
    } else {
        ctx.environment.start_probe()
    };

    // An event here means "the environment changed". Emitting one when no
    // probe even started was a lie with a visible cost: the desktop calls this
    // on every window focus, the renderer rebuilds the whole screen on any
    // event, and the user's first click after returning to the app landed on a
    // screen that was being replaced underneath it.
    //
    // A probe that *did* start reports itself when it lands, through the
    // generation counter the tick already carries. So there is nothing to
    // announce here either way.
    if !started {
        return Ok((env_json(&ctx.environment), Vec::new()));
    }
    let seq = ctx
        .store
        .append_event("env.probing", "environment", json!({}))?;
    Ok((
        env_json(&ctx.environment),
        vec![event(seq, "env.probing", "environment", json!({}))],
    ))
}

/// `probed: false` is a distinct answer from "everything is fine", and the UI
/// must not render one as the other.
fn env_json(cache: &EnvCache) -> Value {
    let Some(env) = cache.snapshot() else {
        return json!({
            "probed": false,
            "probing": cache.is_probing(),
            "generation": cache.generation(),
            "environment": Value::Null,
        });
    };
    json!({
        "probed": true,
        "probing": cache.is_probing(),
        "generation": cache.generation(),
        "environment": env,
        "severity": env.severity(),
        "problems": env.problems().iter().map(|c| c.component).collect::<Vec<_>>(),
        "can_run_anything": env.can_run_anything(),
        "prerequisites": {
            "brew": env_probe::has_prerequisite("brew"),
            "npm": env_probe::has_prerequisite("npm"),
        },
    })
}

fn env_install_recipe(params: &Value) -> DispatchResult {
    let raw = str_param(params, "component")?;
    let component = Component::parse(raw).ok_or_else(|| bad_params(format!("未知组件 `{raw}`")))?;
    let recipe = environment::install_recipe(component);
    let prerequisite_ok = recipe
        .prerequisite
        .as_deref()
        .map(env_probe::has_prerequisite)
        .unwrap_or(true);
    Ok((
        json!({
            "recipe": recipe,
            "prerequisite_ok": prerequisite_ok,
            "login_command": environment::login_command(component),
        }),
        vec![],
    ))
}

fn skills_list(ctx: &mut Ctx, project_id: Option<&str>) -> DispatchResult {
    let home = ctx.home.to_string_lossy().into_owned();
    let (inventory, repo) = match project_id {
        Some(id) => {
            let p = ctx.store.get_project(id)?;
            (skills::scan(&home, &p.path), Some(p.path))
        }
        None => (skills::scan_global(&home), None),
    };
    let global = config_io::load_global(&ctx.autome_home)?;
    let project_config = match &repo {
        Some(path) => config_io::load_project(std::path::Path::new(path))?,
        None => ProjectConfig::default(),
    };
    let bindings = config_io::skill_bindings(&global, &project_config);
    let resolved = config::resolve(&global, &project_config);

    let skills_json: Vec<Value> = inventory
        .iter()
        .map(|s| {
            let bound: Vec<Role> = bindings.get(&s.name).cloned().unwrap_or_default();
            // A binding whose role runs a runtime that cannot see the skill is
            // the S-04 conflict; surface it per skill so the card can go red.
            let conflicts: Vec<Role> = bound
                .iter()
                .copied()
                .filter(|r| !s.visible_to().contains(&resolved.role(*r).config.runtime))
                .collect();
            json!({
                "name": s.name,
                "sources": s.sources,
                "visible_to": s.visible_to(),
                "project_scoped": s.is_project_scoped(),
                "bound_roles": bound,
                "conflicts": conflicts,
            })
        })
        .collect();

    Ok((
        json!({ "skills": skills_json, "scope": project_id }),
        vec![],
    ))
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

fn events_since(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let after = params.get("after_seq").and_then(Value::as_u64).unwrap_or(0);
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .min(1000) as u32;
    let events = ctx.store.events_since(after, limit)?;
    Ok((
        json!({
            "events": events.iter().map(|(seq, kind, payload)| json!({
                "seq": seq, "kind": kind, "payload": payload
            })).collect::<Vec<_>>(),
            "latest_seq": ctx.store.latest_seq()?,
        }),
        vec![],
    ))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn event(seq: u64, kind: &str, subject: &str, payload: Value) -> Event {
    Event {
        event_seq: seq,
        event_id: new_id("evt"),
        aggregate_id: subject.to_string(),
        aggregate_revision: seq,
        event_type: kind.to_string(),
        occurred_at: now_iso(),
        payload,
    }
}

fn fail(command: &Command, code: ReplyErrorCode, message: String) -> Outcome {
    Outcome {
        reply: Reply {
            request_id: command.request_id.clone(),
            command_id: command.command_id.clone(),
            protocol_version: PROTOCOL_VERSION,
            outcome: ReplyOutcome::Error { code, message },
        },
        events: vec![],
    }
}

/// Also used by `main.rs` for the "frame is not a command" path.
pub fn protocol_error_reply(message: String) -> Reply {
    Reply {
        request_id: String::new(),
        command_id: String::new(),
        protocol_version: PROTOCOL_VERSION,
        outcome: ReplyOutcome::Error {
            code: ReplyErrorCode::InvalidParams,
            message,
        },
    }
}

/// Method names the read channel may carry: everything that cannot mutate.
/// Electron Main enforces the split, but the list lives here so it stays next
/// to the dispatch table it describes.
pub const READ_METHODS: [&str; 19] = [
    "project.list",
    "project.probe",
    "project.get",
    "project.onboarding.artefacts",
    "task.get",
    "task.changes",
    "session.log",
    "dashboard.get",
    "config.get",
    "config.validate",
    "env.get",
    "env.install_recipe",
    "skills.list",
    "events.since",
    "env.detect",
    "protocol.get",
    "protocol.versions",
    "protocol.eval",
    "protocol.triggers",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A throwaway world: its own store, its own `~/.autome` and its own
    /// `$HOME`. Nothing here touches process-global state, so the whole
    /// dispatch suite runs in parallel without interfering.
    struct Sandbox {
        root: PathBuf,
        ctx: Ctx,
    }

    impl Sandbox {
        fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let root =
                std::env::temp_dir().join(format!("automed-disp-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let autome_home = root.join("autome-home");
            let home = root.join("home");
            std::fs::create_dir_all(&autome_home).unwrap();
            std::fs::create_dir_all(&home).unwrap();
            let ctx = Ctx::new(Store::open_in_memory().unwrap(), &autome_home, &home).dry();
            Sandbox { root, ctx }
        }
        fn path(&self, rel: &str) -> PathBuf {
            self.root.join(rel)
        }
        fn ctx(&mut self) -> &mut Ctx {
            &mut self.ctx
        }
        /// Adds a project and returns its id, for the many tests that need one.
        fn add_project(&mut self, rel: &str) -> String {
            let path = self.path(rel);
            let out = handle_command(
                self.ctx(),
                &cmd("project.add", json!({ "path": path.to_str().unwrap() })),
            );
            ok_payload(&out)
                .pointer("/project/id")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string()
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn cmd(method: &str, params: Value) -> Command {
        Command {
            request_id: "r".into(),
            command_id: "c".into(),
            expected_revision: None,
            protocol_version: PROTOCOL_VERSION,
            method: method.into(),
            params,
        }
    }

    fn ok_payload(outcome: &Outcome) -> &Value {
        match &outcome.reply.outcome {
            ReplyOutcome::Ok { payload, .. } => payload,
            ReplyOutcome::Error { code, message } => {
                panic!("expected ok, got {code:?}: {message}")
            }
        }
    }

    fn err_message(outcome: &Outcome) -> &str {
        match &outcome.reply.outcome {
            ReplyOutcome::Error { message, .. } => message,
            ReplyOutcome::Ok { .. } => panic!("expected an error"),
        }
    }

    fn git_available() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    macro_rules! needs_git {
        () => {
            if !git_available() {
                eprintln!("skipping: git is not installed");
                return;
            }
        };
    }

    // ---- protocol --------------------------------------------------------

    #[test]
    fn a_version_mismatch_is_a_protocol_violation() {
        let mut sb = Sandbox::new("version");
        let mut command = cmd("project.list", json!({}));
        command.protocol_version = 999;
        let out = handle_command(sb.ctx(), &command);
        assert!(matches!(
            out.reply.outcome,
            ReplyOutcome::Error {
                code: ReplyErrorCode::ProtocolViolation,
                ..
            }
        ));
    }

    #[test]
    fn an_unknown_method_names_itself() {
        let mut sb = Sandbox::new("unknown");
        let out = handle_command(sb.ctx(), &cmd("nope.nope", json!({})));
        assert!(err_message(&out).contains("nope.nope"));
    }

    #[test]
    fn a_missing_parameter_is_reported_by_name() {
        let mut sb = Sandbox::new("missing-param");
        let out = handle_command(sb.ctx(), &cmd("project.get", json!({})));
        assert!(err_message(&out).contains("project_id"));
    }

    #[test]
    fn every_reply_carries_the_request_and_command_ids() {
        let mut sb = Sandbox::new("ids");
        let out = handle_command(sb.ctx(), &cmd("project.list", json!({})));
        assert_eq!(out.reply.request_id, "r");
        assert_eq!(out.reply.command_id, "c");
    }

    #[test]
    fn read_methods_are_all_dispatchable() {
        let mut sb = Sandbox::new("read-methods");
        for method in READ_METHODS {
            let params = match method {
                "env.install_recipe" => json!({"component": "git"}),
                _ => json!({}),
            };
            let out = handle_command(sb.ctx(), &cmd(method, params));
            if let ReplyOutcome::Error { code, message } = &out.reply.outcome {
                assert_ne!(
                    *code,
                    ReplyErrorCode::UnknownMethod,
                    "{method} is not dispatchable: {message}"
                );
            }
        }
    }

    // ---- projects --------------------------------------------------------

    #[test]
    fn adding_an_empty_directory_initialises_a_repo_and_the_scaffold() {
        needs_git!();
        let mut sb = Sandbox::new("add");
        let target = sb.path("new-project");
        let out = handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": target.to_str().unwrap() })),
        );
        let payload = ok_payload(&out);
        assert_eq!(
            payload.pointer("/project/disposition").unwrap(),
            &json!("created_and_initialised")
        );
        assert!(target.join(".git").exists());
        assert!(target.join(".autome/skill/run_session.sh").exists());
        assert!(target.join(".gitignore").exists());
        assert_eq!(out.events.len(), 1);
    }

    #[test]
    fn adding_an_existing_non_repo_directory_runs_git_init() {
        needs_git!();
        let mut sb = Sandbox::new("add-existing");
        let target = sb.path("existing");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("code.txt"), "hi").unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": target.to_str().unwrap() })),
        );
        assert_eq!(
            ok_payload(&out).pointer("/project/disposition").unwrap(),
            &json!("initialised_existing")
        );
        assert!(target.join("code.txt").exists(), "user files untouched");
    }

    #[test]
    fn adding_an_existing_repo_adopts_it_without_reinitialising() {
        needs_git!();
        let mut sb = Sandbox::new("adopt");
        let target = sb.path("repo");
        std::fs::create_dir_all(&target).unwrap();
        git::init(&target, "trunk").unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": target.to_str().unwrap() })),
        );
        let payload = ok_payload(&out);
        assert_eq!(
            payload.pointer("/project/disposition").unwrap(),
            &json!("adopted_existing")
        );
        assert_eq!(
            payload.pointer("/project/default_branch").unwrap(),
            &json!("trunk"),
            "the repository's own default branch is respected"
        );
    }

    // ---- workspaces --------------------------------------------------------

    /// A directory holding several independent repositories, as people
    /// actually lay one out: each child its own repository, nothing at the
    /// root.
    fn workspace_at(dir: &std::path::Path, members: &[(&str, &str)]) {
        std::fs::create_dir_all(dir).unwrap();
        for (name, branch) in members {
            let member = dir.join(name);
            std::fs::create_dir_all(&member).unwrap();
            git::init(&member, branch).unwrap();
            std::fs::write(member.join("README.md"), format!("{name}\n")).unwrap();
            git::commit_paths(&member, &["README.md"], "init").unwrap();
        }
    }

    #[test]
    fn a_directory_of_repositories_is_probed_as_a_workspace_without_touching_it() {
        needs_git!();
        let mut sb = Sandbox::new("probe-ws");
        let target = sb.path("ws");
        workspace_at(&target, &[("docs", "main"), ("backend", "master")]);

        let out = handle_command(
            sb.ctx(),
            &cmd("project.probe", json!({ "path": target.to_str().unwrap() })),
        );
        let payload = ok_payload(&out);
        assert_eq!(payload["suggest_workspace"], json!(true));
        assert_eq!(payload["is_repo_root"], json!(false));
        assert_eq!(
            payload["members"],
            json!([
                { "name": "backend", "default_branch": "master" },
                { "name": "docs", "default_branch": "main" },
            ]),
            "name order, and each member's own default branch"
        );
        assert_eq!(
            payload["suggested_docs_repo"],
            json!("docs"),
            "a member actually called docs is the obvious suggestion"
        );
        // Probing is a question, not an action.
        assert!(!target.join(".git").exists(), "probe must not initialise");
        assert!(!target.join(".autome").exists(), "probe must not scaffold");
    }

    #[test]
    fn a_single_repository_is_not_suggested_as_a_workspace() {
        needs_git!();
        let mut sb = Sandbox::new("probe-repo");
        let target = sb.path("repo");
        std::fs::create_dir_all(&target).unwrap();
        git::init(&target, "main").unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd("project.probe", json!({ "path": target.to_str().unwrap() })),
        );
        let payload = ok_payload(&out);
        assert_eq!(payload["is_repo_root"], json!(true));
        assert_eq!(payload["suggest_workspace"], json!(false));
    }

    #[test]
    fn adding_a_workspace_initialises_nothing_and_leaves_every_member_alone() {
        // The closing test for the defect this whole feature came from: a
        // `git init` at the workspace root recorded each member as a gitlink,
        // and the first task died committing its own documents.
        needs_git!();
        let mut sb = Sandbox::new("add-ws");
        let target = sb.path("ws");
        workspace_at(&target, &[("docs", "main"), ("backend", "master")]);
        let heads: Vec<String> = ["docs", "backend"]
            .iter()
            .map(|m| git::head_sha(&target.join(m)).unwrap())
            .collect();

        let out = handle_command(
            sb.ctx(),
            &cmd(
                "project.add",
                json!({
                    "path": target.to_str().unwrap(),
                    "workspace": true,
                    "docs_repo": "docs",
                }),
            ),
        );
        let payload = ok_payload(&out);

        assert_eq!(
            payload.pointer("/project/disposition").unwrap(),
            &json!("adopted_workspace")
        );
        assert_eq!(
            payload.pointer("/project/kind").unwrap(),
            &json!("workspace")
        );
        assert_eq!(
            payload.pointer("/project/docs_repo").unwrap(),
            &json!("docs")
        );
        assert_eq!(
            payload.pointer("/project/default_branch").unwrap(),
            &json!("main"),
            "the document repository's branch stands in for the project's"
        );

        assert!(
            !target.join(".git").exists(),
            "the whole point: no repository is created at the workspace root"
        );
        assert!(
            !target.join(".gitignore").exists(),
            "nothing tracks the root, so there is nothing to ignore"
        );
        assert!(
            !target.join("docs").join(".autome").exists(),
            "members are not scaffolded"
        );
        for (m, before) in ["docs", "backend"].iter().zip(&heads) {
            assert_eq!(
                &git::head_sha(&target.join(m)).unwrap(),
                before,
                "{m} was not committed into"
            );
        }
        // The scaffold Autome keeps beside every project is still there.
        assert!(target.join(".autome/skill/run_session.sh").exists());
    }

    #[test]
    fn a_workspace_refuses_tasks_by_name_until_multi_repo_lands() {
        needs_git!();
        let mut sb = Sandbox::new("ws-task");
        let target = sb.path("ws");
        workspace_at(&target, &[("docs", "main"), ("backend", "main")]);
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "project.add",
                json!({ "path": target.to_str().unwrap(), "workspace": true, "docs_repo": "docs" }),
            ),
        );
        let id = ok_payload(&out)["project"]["id"]
            .as_str()
            .unwrap()
            .to_string();

        let out = handle_command(
            sb.ctx(),
            &cmd(
                "task.create",
                json!({ "project_id": id, "request": "做点什么" }),
            ),
        );
        let message = err_message(&out);
        assert!(
            message.contains("工作区") && message.contains("还没接通"),
            "a refusal the user can act on, got: {message}"
        );
    }

    #[test]
    fn a_workspace_needs_a_document_repository_that_exists() {
        needs_git!();
        let mut sb = Sandbox::new("ws-nodocs");
        let target = sb.path("ws");
        workspace_at(&target, &[("a", "main"), ("b", "main")]);
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "project.add",
                json!({ "path": target.to_str().unwrap(), "workspace": true, "docs_repo": "nope" }),
            ),
        );
        assert!(err_message(&out).contains("nope"));

        let out = handle_command(
            sb.ctx(),
            &cmd(
                "project.add",
                json!({ "path": target.to_str().unwrap(), "workspace": true }),
            ),
        );
        assert!(err_message(&out).contains("文档仓库"));
    }

    #[test]
    fn adding_the_same_directory_twice_is_refused() {
        needs_git!();
        let mut sb = Sandbox::new("dup");
        let target = sb.path("p");
        let params = json!({ "path": target.to_str().unwrap() });
        handle_command(sb.ctx(), &cmd("project.add", params.clone()));
        let out = handle_command(sb.ctx(), &cmd("project.add", params));
        assert!(err_message(&out).contains("已经是一个项目"));
    }

    #[test]
    fn a_nested_directory_is_refused_with_the_outer_projects_name() {
        needs_git!();
        let mut sb = Sandbox::new("nested");
        let outer = sb.path("outer");
        handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": outer.to_str().unwrap() })),
        );
        let inner = outer.join("inner");
        let out = handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": inner.to_str().unwrap() })),
        );
        assert!(err_message(&out).contains("嵌套"), "{}", err_message(&out));
    }

    #[test]
    fn adding_a_file_rather_than_a_directory_is_refused() {
        let mut sb = Sandbox::new("file");
        let file = sb.path("a-file");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "x").unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": file.to_str().unwrap() })),
        );
        assert!(err_message(&out).contains("不是目录"));
    }

    #[test]
    fn a_new_project_starts_onboarding_and_has_not_committed_yet() {
        needs_git!();
        let mut sb = Sandbox::new("onboard-start");
        let target = sb.path("p");
        let out = handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": target.to_str().unwrap() })),
        );
        let onboarding = ok_payload(&out).pointer("/project/onboarding").unwrap();
        assert_eq!(onboarding.get("onboarding").unwrap(), &json!("in_progress"));
        assert_eq!(onboarding.get("step").unwrap(), &json!(3));
    }

    #[test]
    fn skipping_onboarding_makes_the_init_commit_exactly_once() {
        needs_git!();
        let mut sb = Sandbox::new("skip");
        let target = sb.path("p");
        let id = sb.add_project("p");

        let out = handle_command(
            sb.ctx(),
            &cmd("project.onboarding.skip", json!({ "project_id": id })),
        );
        let commit = ok_payload(&out).get("init_commit").unwrap();
        assert!(commit.is_string(), "expected a commit sha, got {commit}");
        assert!(git::has_commits(&target));

        let again = handle_command(
            sb.ctx(),
            &cmd("project.onboarding.skip", json!({ "project_id": id })),
        );
        assert_eq!(ok_payload(&again).get("init_commit").unwrap(), &Value::Null);
    }

    #[test]
    fn advancing_onboarding_to_the_end_also_commits() {
        needs_git!();
        let mut sb = Sandbox::new("advance");
        let id = sb.add_project("p");
        for _ in 0..2 {
            let out = handle_command(
                sb.ctx(),
                &cmd("project.onboarding.advance", json!({ "project_id": id })),
            );
            assert_eq!(ok_payload(&out).get("init_commit").unwrap(), &Value::Null);
        }
        let last = handle_command(
            sb.ctx(),
            &cmd("project.onboarding.advance", json!({ "project_id": id })),
        );
        assert_eq!(
            ok_payload(&last).pointer("/onboarding/onboarding").unwrap(),
            &json!("completed")
        );
        assert!(ok_payload(&last).get("init_commit").unwrap().is_string());
    }

    #[test]
    fn project_list_is_empty_then_shows_what_was_added() {
        needs_git!();
        let mut sb = Sandbox::new("list");
        let out = handle_command(sb.ctx(), &cmd("project.list", json!({})));
        assert!(ok_payload(&out)["projects"].as_array().unwrap().is_empty());
        sb.add_project("p");
        let out = handle_command(sb.ctx(), &cmd("project.list", json!({})));
        assert_eq!(ok_payload(&out)["projects"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn project_get_returns_the_resolved_config_and_the_rule_files() {
        needs_git!();
        let mut sb = Sandbox::new("get");
        let id = sb.add_project("p");
        let out = handle_command(sb.ctx(), &cmd("project.get", json!({ "project_id": id })));
        let payload = ok_payload(&out);
        assert_eq!(
            payload["config"]["roles"].as_array().unwrap().len(),
            Role::ALL.len()
        );
        assert!(payload["violations"].as_array().unwrap().is_empty());
        let rules = payload["rules"].as_array().unwrap();
        assert!(rules.iter().any(|r| r == "AGENTS.md"), "{rules:?}");
    }

    #[test]
    fn getting_a_missing_project_is_not_found() {
        let mut sb = Sandbox::new("get-missing");
        let out = handle_command(
            sb.ctx(),
            &cmd("project.get", json!({"project_id": "ghost"})),
        );
        assert!(matches!(
            out.reply.outcome,
            ReplyOutcome::Error {
                code: ReplyErrorCode::NotFound,
                ..
            }
        ));
    }

    #[test]
    fn the_document_list_includes_the_per_round_evidence_files() {
        // Since the 2026-09-16 audit the evidence lives in its own directory
        // rather than being appended to the design document. Main resolves an
        // open by matching a name against this list, so an evidence file that
        // is not here is one the user cannot open from the panel.
        let sb = Sandbox::new("docs-evidence");
        let task = crate::store::TaskRecord {
            id: "T-1".into(),
            project_id: "p".into(),
            slug: "cart".into(),
            title: "t".into(),
            request: "r".into(),
            attachments: vec![],
            doc_refs: vec![],
            state: autome_domain::task::TaskState::Queued,
            budget_n: None,
            created_at: now_iso(),
            completed_at: None,
            merge_commit: None,
            archived_at: None,
            protocol_ref: None,
            rules_hash: None,
            metrics: None,
            doc_root: None,
        };
        let worktree = sb.path("wt");
        let dir = worktree.join(task.doc_dir());
        std::fs::create_dir_all(dir.join("evidence")).unwrap();
        std::fs::write(dir.join("cart.md"), "设计").unwrap();
        std::fs::write(dir.join("retro.md"), "记录").unwrap();
        // Written out of order, and one file that is not evidence at all.
        std::fs::write(dir.join("evidence/M-02-r3.md"), "第三轮").unwrap();
        std::fs::write(dir.join("evidence/M-01-r1.md"), "第一轮").unwrap();
        std::fs::write(dir.join("evidence/raw.log"), "不是 markdown").unwrap();

        let docs = documents(&worktree, &task);
        let names: Vec<&str> = docs.iter().map(|d| d["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            vec!["cart.md", "retro.md", "M-01-r1.md", "M-02-r3.md"]
        );

        let evidence = docs.iter().find(|d| d["name"] == "M-01-r1.md").unwrap();
        assert_eq!(evidence["label"], "证据");
        assert_eq!(evidence["path"], "docs/cart/evidence/M-01-r1.md");
        // The name Main matches on must stay path-free: the write gate
        // refuses anything path-shaped.
        assert!(!evidence["name"].as_str().unwrap().contains('/'));
    }

    #[test]
    fn a_task_without_an_evidence_directory_still_lists_its_documents() {
        let sb = Sandbox::new("docs-no-evidence");
        let task = crate::store::TaskRecord {
            id: "T-1".into(),
            project_id: "p".into(),
            slug: "cart".into(),
            title: "t".into(),
            request: "r".into(),
            attachments: vec![],
            doc_refs: vec![],
            state: autome_domain::task::TaskState::Queued,
            budget_n: None,
            created_at: now_iso(),
            completed_at: None,
            merge_commit: None,
            archived_at: None,
            protocol_ref: None,
            rules_hash: None,
            metrics: None,
            doc_root: None,
        };
        let worktree = sb.path("wt");
        let dir = worktree.join(task.doc_dir());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cart-task.md"), "任务").unwrap();

        let docs = documents(&worktree, &task);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0]["label"], "任务文件");
    }

    #[test]
    fn removing_a_project_with_a_live_task_is_refused() {
        needs_git!();
        let mut sb = Sandbox::new("remove-live");
        let id = sb.add_project("p");
        sb.ctx()
            .store
            .insert_task(&crate::store::TaskRecord {
                id: "T-1".into(),
                project_id: id.clone(),
                slug: "s".into(),
                title: "t".into(),
                request: "r".into(),
                attachments: vec![],
                doc_refs: vec![],
                state: autome_domain::task::TaskState::Queued,
                budget_n: None,
                created_at: now_iso(),
                completed_at: None,
                merge_commit: None,
                archived_at: None,
                protocol_ref: None,
                rules_hash: None,
                metrics: None,
                doc_root: None,
            })
            .unwrap();
        let out = handle_command(sb.ctx(), &cmd("project.remove", json!({"project_id": id})));
        assert!(err_message(&out).contains("T-1"));
    }

    #[test]
    fn removing_a_project_leaves_the_directory_alone() {
        needs_git!();
        let mut sb = Sandbox::new("remove-ok");
        let target = sb.path("p");
        let id = sb.add_project("p");
        let out = handle_command(sb.ctx(), &cmd("project.remove", json!({"project_id": id})));
        assert!(matches!(out.reply.outcome, ReplyOutcome::Ok { .. }));
        assert!(
            target.join(".autome").exists(),
            "the directory is untouched"
        );
        let list = handle_command(sb.ctx(), &cmd("project.list", json!({})));
        assert!(ok_payload(&list)["projects"].as_array().unwrap().is_empty());
    }

    // ---- config ----------------------------------------------------------

    #[test]
    fn config_get_without_a_project_returns_the_global_defaults() {
        let mut sb = Sandbox::new("cfg-global");
        let out = handle_command(sb.ctx(), &cmd("config.get", json!({})));
        let payload = ok_payload(&out);
        assert_eq!(payload["resolved"]["loop_defaults"]["parallel"], json!(3));
        assert!(payload["violations"].as_array().unwrap().is_empty());
    }

    #[test]
    fn setting_a_global_role_persists_and_is_visible_immediately() {
        let mut sb = Sandbox::new("cfg-set");
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"role": "impl", "model": "claude-sonnet-5"}),
            ),
        );
        let payload = ok_payload(&out);
        let impl_model = payload["resolved"]["roles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["role"] == "impl")
            .unwrap()["config"]["model"]
            .clone();
        assert_eq!(impl_model, json!("claude-sonnet-5"));
        assert_eq!(out.events.len(), 1);
    }

    #[test]
    fn a_same_model_collision_is_refused_with_a_readable_message() {
        let mut sb = Sandbox::new("cfg-samemodel");
        // Both sides naming the same model on the same runtime.
        handle_command(
            sb.ctx(),
            &cmd("config.set_role", json!({"role": "impl", "model": "opus"})),
        );
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"role": "audit", "runtime": "claude", "model": "opus"}),
            ),
        );
        let message = err_message(&out);
        assert!(message.contains("审计"), "{message}");
        assert!(message.contains("实现"), "{message}");
        assert!(message.contains("不同模型"), "{message}");
    }

    #[test]
    fn two_unnamed_models_on_one_runtime_are_refused_with_their_own_message() {
        // The shipped defaults leave models empty, so the collision a user is
        // most likely to create is "I moved review onto Claude" — and the fix
        // is to name a model, not to change one. The message must say so.
        let mut sb = Sandbox::new("cfg-bothdefault");
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"role": "review", "runtime": "claude"}),
            ),
        );
        let message = err_message(&out);
        assert!(message.contains("评审"), "{message}");
        assert!(message.contains("设计"), "{message}");
        assert!(message.contains("指定模型"), "{message}");
    }

    #[test]
    fn a_refused_config_write_does_not_change_the_file() {
        let mut sb = Sandbox::new("cfg-atomic");
        let home = sb.ctx.autome_home.clone();
        let before = config_io::load_global(&home).unwrap();
        handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"role": "review", "runtime": "claude"}),
            ),
        );
        assert_eq!(config_io::load_global(&home).unwrap(), before);
    }

    #[test]
    fn binding_a_skill_that_does_not_exist_is_refused() {
        let mut sb = Sandbox::new("cfg-skill");
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"role": "impl", "skills": ["ghost"]}),
            ),
        );
        assert!(err_message(&out).contains("ghost"), "{}", err_message(&out));
    }

    #[test]
    fn an_out_of_range_parallel_limit_is_refused() {
        let mut sb = Sandbox::new("cfg-parallel");
        let out = handle_command(sb.ctx(), &cmd("config.set_loop", json!({"parallel": 9})));
        assert!(err_message(&out).contains("1..=5"), "{}", err_message(&out));
    }

    #[test]
    fn a_project_override_and_its_reset_round_trip_through_the_files() {
        needs_git!();
        let mut sb = Sandbox::new("cfg-project");
        let repo = sb.path("p");
        let id = sb.add_project("p");

        handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"project_id": id, "role": "impl", "effort": "low"}),
            ),
        );
        let text = std::fs::read_to_string(repo.join(".autome/config.toml")).unwrap();
        assert!(text.contains("effort"), "{text}");
        assert!(!text.contains("model"), "still sparse:\n{text}");

        let out = handle_command(
            sb.ctx(),
            &cmd(
                "config.reset_role",
                json!({"project_id": id, "role": "impl"}),
            ),
        );
        assert!(ok_payload(&out).get("resolved").is_some());
        let text = std::fs::read_to_string(repo.join(".autome/config.toml")).unwrap();
        assert!(!text.contains("[roles.impl]"), "{text}");
    }

    #[test]
    fn setting_a_project_parallel_limit_updates_the_cached_row() {
        needs_git!();
        let mut sb = Sandbox::new("cfg-parallel-row");
        let id = sb.add_project("p");
        handle_command(
            sb.ctx(),
            &cmd("config.set_loop", json!({"project_id": id, "parallel": 5})),
        );
        assert_eq!(sb.ctx().store.get_project(&id).unwrap().parallel_limit, 5);
    }

    #[test]
    fn config_validate_reports_ok_for_the_shipped_defaults() {
        let mut sb = Sandbox::new("cfg-validate");
        let out = handle_command(sb.ctx(), &cmd("config.validate", json!({})));
        assert_eq!(ok_payload(&out)["ok"], json!(true));
    }

    // ---- environment and skills -----------------------------------------

    #[test]
    fn env_get_answers_immediately_and_probes_in_the_background() {
        // The packaged app hung on first launch because this probe ran inside
        // the request: it takes tens of seconds, the IPC timeout is five, and
        // the core's stdio loop is serial, so every later request queued
        // behind it and timed out too.
        let mut sb = Sandbox::new("env");
        let started = std::time::Instant::now();
        let out = handle_command(sb.ctx(), &cmd("env.get", json!({})));
        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "env.get must not wait for the probe: took {:?}",
            started.elapsed()
        );
        let payload = ok_payload(&out);
        // Before the first probe lands, "not observed" is the honest answer,
        // and it is a different answer from "everything is fine".
        assert!(payload["probed"].is_boolean());
        if payload["probed"] == json!(false) {
            assert_eq!(payload["environment"], Value::Null);
        }
    }

    #[test]
    fn a_completed_probe_reports_all_four_components() {
        let mut sb = Sandbox::new("env-blocking");
        sb.ctx().environment.probe_blocking();
        let out = handle_command(sb.ctx(), &cmd("env.get", json!({})));
        let payload = ok_payload(&out);
        assert_eq!(payload["probed"], json!(true));
        assert_eq!(
            payload["environment"]["components"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert!(payload["generation"].as_u64().unwrap() >= 1);
    }

    #[test]
    fn the_dashboard_never_waits_for_a_probe_either() {
        let mut sb = Sandbox::new("env-dashboard");
        let started = std::time::Instant::now();
        let out = handle_command(sb.ctx(), &cmd("dashboard.get", json!({})));
        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "dashboard.get took {:?}",
            started.elapsed()
        );
        assert!(ok_payload(&out)["environment"]["probed"].is_boolean());
    }

    #[test]
    fn setting_a_theme_persists_it_and_refuses_an_unknown_one() {
        let mut sb = Sandbox::new("theme");
        let out = handle_command(
            sb.ctx(),
            &cmd("config.set_theme", json!({ "theme": "dark" })),
        );
        assert_eq!(ok_payload(&out)["theme"], json!("dark"));

        let got = handle_command(sb.ctx(), &cmd("config.get", json!({})));
        assert_eq!(ok_payload(&got)["global"]["ui"]["theme"], json!("dark"));

        let bad = handle_command(
            sb.ctx(),
            &cmd("config.set_theme", json!({ "theme": "midnight" })),
        );
        assert!(matches!(bad.reply.outcome, ReplyOutcome::Error { .. }));
    }

    #[test]
    fn the_theme_survives_a_reload_and_defaults_to_following_the_system() {
        let mut sb = Sandbox::new("theme-reload");
        let fresh = handle_command(sb.ctx(), &cmd("config.get", json!({})));
        assert_eq!(
            ok_payload(&fresh)["global"]["ui"]["theme"],
            json!("system"),
            "an app on macOS follows the system until told otherwise"
        );

        handle_command(
            sb.ctx(),
            &cmd("config.set_theme", json!({ "theme": "light" })),
        );
        // Read it back off disk rather than out of memory: the point of
        // putting this in the global config is that it outlives the process.
        let on_disk = config_io::load_global(&sb.ctx.autome_home).unwrap();
        assert_eq!(on_disk.ui.theme, autome_domain::config::Theme::Light);
    }

    #[test]
    fn a_production_context_never_opens_a_window() {
        // The desktop builds its Ctx with `Ctx::new` and nothing else. If the
        // default drifts back to `Terminal`, every session in a Loop pops a
        // terminal in front of whatever the user was doing.
        let ctx = Ctx::new(Store::open_in_memory().unwrap(), "/tmp/ah", "/tmp/home");
        assert_eq!(
            ctx.launch_mode,
            crate::launcher::LaunchMode::Headless,
            "Ctx::new is what the desktop uses; it must not open windows"
        );
    }

    #[test]
    fn env_detect_returns_at_once_rather_than_holding_the_protocol() {
        let mut sb = Sandbox::new("env-detect");
        let started = std::time::Instant::now();
        let out = handle_command(sb.ctx(), &cmd("env.detect", json!({})));
        assert!(
            started.elapsed() < std::time::Duration::from_millis(500),
            "env.detect took {:?}",
            started.elapsed()
        );
        assert_eq!(out.events.len(), 1, "the first call does start a probe");
    }

    #[test]
    fn env_detect_announces_nothing_when_it_did_not_start_a_probe() {
        // The desktop calls this on every window focus. It used to emit an
        // `env.changed` event unconditionally; the renderer rebuilds the whole
        // screen on any event, so returning to the app replaced the screen
        // under the user's cursor and ate their first click.
        let mut sb = Sandbox::new("env-detect-quiet");
        let first = handle_command(sb.ctx(), &cmd("env.detect", json!({})));
        assert_eq!(first.events.len(), 1);

        // A probe is already running, so nothing started and nothing is said.
        for _ in 0..5 {
            let out = handle_command(sb.ctx(), &cmd("env.detect", json!({})));
            assert!(
                out.events.is_empty(),
                "a call that started no probe must announce nothing"
            );
        }
    }

    #[test]
    fn a_completed_probe_is_not_repeated_until_the_rate_limit_lapses() {
        let cache = EnvCache::default();
        cache.probe_blocking();
        let before = cache.generation();
        assert!(before > 0);

        assert!(
            !cache.start_probe(),
            "an unforced probe must respect the interval"
        );
        assert!(
            cache.force_probe(),
            "E-04's button must not be rate limited"
        );
    }

    #[test]
    fn the_probe_interval_is_long_enough_to_survive_window_switching() {
        // The failure this guards against is a user alt-tabbing between the
        // app and a terminal: at a few seconds, every switch re-probed.
        assert!(
            ENV_PROBE_MIN_INTERVAL >= std::time::Duration::from_secs(60),
            "an interval this short does not stop focus-driven re-probing"
        );
    }

    #[test]
    fn env_install_recipe_returns_a_command_for_each_component() {
        let mut sb = Sandbox::new("env-recipe");
        for component in ["git", "claude", "codex", "iterm2"] {
            let out = handle_command(
                sb.ctx(),
                &cmd("env.install_recipe", json!({ "component": component })),
            );
            let cmd_str = ok_payload(&out)["recipe"]["command"].as_str().unwrap();
            assert!(!cmd_str.is_empty(), "{component}");
        }
    }

    #[test]
    fn an_unknown_component_is_rejected() {
        let mut sb = Sandbox::new("env-unknown");
        let out = handle_command(
            sb.ctx(),
            &cmd("env.install_recipe", json!({"component": "emacs"})),
        );
        assert!(err_message(&out).contains("emacs"));
    }

    #[test]
    fn skills_list_reports_bindings_and_refuses_an_invisible_one() {
        needs_git!();
        let mut sb = Sandbox::new("skills");
        let repo = sb.path("p");
        let id = sb.add_project("p");

        // A Claude-only project skill.
        let skill_dir = repo.join(".claude/skills/repo-facts");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# repo-facts").unwrap();

        // Binding it to the Codex-running review role must be refused (S-04).
        let refused = handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"project_id": id, "role": "review", "skills": ["repo-facts"]}),
            ),
        );
        assert!(
            err_message(&refused).contains("看不到"),
            "{}",
            err_message(&refused)
        );

        // Bound to a Claude role it is fine.
        let ok = handle_command(
            sb.ctx(),
            &cmd(
                "config.set_role",
                json!({"project_id": id, "role": "impl", "skills": ["repo-facts"]}),
            ),
        );
        assert!(matches!(ok.reply.outcome, ReplyOutcome::Ok { .. }));

        let out = handle_command(sb.ctx(), &cmd("skills.list", json!({"project_id": id})));
        let skills = ok_payload(&out)["skills"].as_array().unwrap().clone();
        let entry = skills
            .iter()
            .find(|s| s["name"] == "repo-facts")
            .expect("the project skill is listed");
        assert_eq!(entry["visible_to"], json!(["claude"]));
        assert_eq!(entry["bound_roles"], json!(["impl"]));
        assert!(entry["conflicts"].as_array().unwrap().is_empty());
        assert_eq!(entry["project_scoped"], json!(true));
    }

    // ---- events ----------------------------------------------------------

    #[test]
    fn events_since_returns_what_happened_after_a_sequence_number() {
        needs_git!();
        let mut sb = Sandbox::new("events");
        let before = sb.ctx().store.latest_seq().unwrap();
        sb.add_project("p");
        let out = handle_command(sb.ctx(), &cmd("events.since", json!({"after_seq": before})));
        let events = ok_payload(&out)["events"].as_array().unwrap().clone();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["kind"], json!("project.added"));
    }

    #[test]
    fn every_successful_reply_carries_a_snapshot_seq() {
        let mut sb = Sandbox::new("snapshot");
        let out = handle_command(sb.ctx(), &cmd("project.list", json!({})));
        match out.reply.outcome {
            ReplyOutcome::Ok { snapshot_seq, .. } => assert_eq!(snapshot_seq, 0),
            _ => panic!("expected ok"),
        }
    }

    #[test]
    fn a_read_command_appends_no_events() {
        let mut sb = Sandbox::new("read-only");
        let before = sb.ctx().store.latest_seq().unwrap();
        for method in ["project.list", "config.get", "skills.list", "events.since"] {
            let out = handle_command(sb.ctx(), &cmd(method, json!({})));
            assert!(out.events.is_empty(), "{method} emitted events");
        }
        assert_eq!(sb.ctx().store.latest_seq().unwrap(), before);
    }

    // ---- helpers ---------------------------------------------------------

    #[test]
    fn shellexpand_handles_a_leading_tilde_only() {
        let home = Path::new("/Users/test");
        assert_eq!(shellexpand_home("~/code/x", home), "/Users/test/code/x");
        assert_eq!(shellexpand_home("~", home), "/Users/test");
        assert_eq!(shellexpand_home("/abs/~/x", home), "/abs/~/x");
        assert_eq!(shellexpand_home("relative", home), "relative");
    }

    #[test]
    fn rule_files_lists_agents_md_and_the_rules_directory_in_order() {
        let sb = Sandbox::new("rules");
        let repo = sb.path("r");
        std::fs::create_dir_all(repo.join(".autome/rules")).unwrap();
        std::fs::write(repo.join("AGENTS.md"), "x").unwrap();
        std::fs::write(repo.join(".autome/rules/z.md"), "x").unwrap();
        std::fs::write(repo.join(".autome/rules/a.md"), "x").unwrap();
        std::fs::write(repo.join(".autome/rules/notes.txt"), "x").unwrap();
        let files = rule_files(repo.to_str().unwrap());
        assert_eq!(
            files,
            vec![
                "AGENTS.md".to_string(),
                ".autome/rules/a.md".to_string(),
                ".autome/rules/z.md".to_string()
            ]
        );
    }

    #[test]
    fn describe_violation_renders_every_variant_in_chinese() {
        let variants = [
            ConfigViolation::SameModel {
                evaluator: Role::Audit,
                generator: Role::Impl,
                identity: "claude:x".into(),
            },
            ConfigViolation::SkillNotVisible {
                role: Role::Review,
                skill: "s".into(),
                runtime: Runtime::Codex,
            },
            ConfigViolation::SkillNotFound {
                role: Role::Impl,
                skill: "s".into(),
            },
            ConfigViolation::ParallelOutOfRange {
                value: 9,
                min: 1,
                max: 5,
            },
            ConfigViolation::DesignRoundsOutOfRange { value: 0 },
            ConfigViolation::BudgetFactorOutOfRange { value: 0 },
            ConfigViolation::BothDefaultModels {
                evaluator: Role::Review,
                generator: Role::Plan,
            },
        ];
        for v in variants {
            let s = describe_violation(&v);
            assert!(!s.is_empty());
            assert!(!s.is_ascii(), "{s} should be Chinese");
        }
    }
}
