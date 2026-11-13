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
use autome_domain::metrics::CostSummary;
use autome_domain::project::{self, AddDisposition, Onboarding, Project, ProjectKind};
use autome_domain::role::{Role, Runtime};
use autome_domain::status_block::{self, StatusBlock};
use autome_domain::task::{Disposition, Node, TaskKind, TaskState, Trigger};
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
    pub build_id: String,
    pub allow_older_kernel: bool,
    pub adopted_older_kernel: bool,
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
    /// Start of the last tick that completed core steps; bounds uncollected jobs.
    pub gate_prev_tick: Option<std::time::SystemTime>,
    /// The driver lock while this context holds it (see `driver_lock`).
    /// Held for one tick or one write command, so the desktop app and the
    /// `autome` command can drive the same ledger without both starting the
    /// same round. A field rather than a guard on the stack because write
    /// commands tick on their way out and the tick must see the lock is
    /// already ours.
    pub driver_lock: Option<crate::driver_lock::DriverLock>,
    /// Optional frontend feedback for prolonged driver lock contention.
    pub driver_lock_notice: Option<fn()>,
}

impl Ctx {
    pub fn new(
        store: Store,
        autome_home: impl Into<std::path::PathBuf>,
        home: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            build_id: crate::build_id::BUILD_ID.into(),
            allow_older_kernel: false,
            adopted_older_kernel: false,
            store,
            autome_home: autome_home.into(),
            home: home.into(),
            environment: EnvCache::default(),
            launch_mode: crate::launcher::LaunchMode::Headless,
            recovered: false,
            gate_prev_tick: None,
            driver_lock: None,
            driver_lock_notice: None,
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

/// How long a write command waits for another driver's tick to finish.
const DRIVER_LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

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

    // A write command holds the driver lock for its whole length: it may
    // advance a task and start a session, and another driver must not do the
    // same thing in between. The tick takes the lock itself, without waiting.
    let writing =
        command.method != "scheduler.tick" && !READ_METHODS.contains(&command.method.as_str());
    if writing {
        match crate::kernel_guard::check(ctx) {
            Ok(Some(old)) => return fail(command, ReplyErrorCode::OlderKernel, old.message),
            Err(e) => return fail(command, ReplyErrorCode::Internal, e.to_string()),
            Ok(None) => {}
        }
    }
    let mut took_lock = false;

    if writing && ctx.driver_lock.is_none() {
        match crate::driver_lock::acquire_with_notice(
            ctx.store.path(),
            DRIVER_LOCK_WAIT,
            ctx.driver_lock_notice,
        ) {
            Ok(Some(lock)) => {
                ctx.driver_lock = Some(lock);
                took_lock = true;
            }
            Ok(None) => {
                return fail(
                    command,
                    ReplyErrorCode::Internal,
                    "另一个 Autome（桌面端或命令行）正占着状态库，稍后再试".into(),
                );
            }
            Err(e) => {
                return fail(
                    command,
                    ReplyErrorCode::Internal,
                    format!("无法取得驱动锁：{e}"),
                );
            }
        }
    }
    if writing {
        let rejection = match crate::kernel_guard::claim(ctx, &command.method) {
            Ok(Some(old)) => Some((ReplyErrorCode::OlderKernel, old.message)),
            Err(e) => Some((ReplyErrorCode::Internal, e.to_string())),
            Ok(None) => None,
        };
        if let Some((code, message)) = rejection {
            if took_lock {
                ctx.driver_lock = None;
            }
            return fail(command, code, message);
        }
    }
    let result = dispatch(ctx, command);

    if took_lock {
        ctx.driver_lock = None;
    }

    match result {
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
        "project.alert.dismiss" => alert_dismiss(ctx, str_param(p, "alert_id")?),

        // ---- tasks -------------------------------------------------------
        "task.create" => task_create(ctx, p),
        "task.start" => task_start(ctx, str_param(p, "task_id")?),
        "task.set_after" => task_set_after(ctx, p),
        "task.move" => task_move(ctx, p),
        "task.get" => task_get(ctx, str_param(p, "task_id")?),
        "task.approve" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Approve),
        "task.reject" => task_trigger(
            ctx,
            str_param(p, "task_id")?,
            Trigger::Reject {
                feedback: str_param(p, "feedback")?.to_string(),
            },
        ),
        "task.note" => task_note(ctx, p),
        "task.reply" => {
            let id = str_param(p, "task_id")?;
            let info = str_param(p, "info")?.to_string();
            let task = ctx.store.get_task(id)?;
            let trigger = if scheduler::uses_bug_loop(ctx, &task)? {
                if info.trim().is_empty() {
                    return Err(rejected("补充信息不能为空"));
                }
                Trigger::Reply { info }
            } else {
                Trigger::Reject { feedback: info }
            };
            task_trigger(ctx, id, trigger)
        }
        "task.merge" => task_merge(ctx, p),
        "task.pause" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Pause),
        "task.resume" => task_trigger(ctx, str_param(p, "task_id")?, Trigger::Resume),
        "task.stop" => task_stop(ctx, str_param(p, "task_id")?),
        "task.convert" => task_convert(ctx, p),
        "task.cancel" => task_cancel(ctx, str_param(p, "task_id")?),
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
        "protocol.versions" => crate::dispatch_protocol::versions(ctx, str_param(p, "project_id")?),

        // ---- curation --------------------------------------------------
        "rules.proposals" => crate::dispatch_curation::proposals(ctx, str_param(p, "project_id")?),
        "rules.decide" => crate::dispatch_curation::decide(ctx, p),
        "rules.retire" => crate::dispatch_curation::retire(ctx, p),
        "rules.restore" => crate::dispatch_curation::restore(ctx, p),

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

fn project_costs(
    ctx: &Ctx,
    project_id: &str,
) -> Result<std::collections::BTreeMap<String, CostSummary>, DispatchError> {
    let sessions = ctx.store.sessions_in_project(project_id)?;
    let mut groups = std::collections::BTreeMap::<String, Vec<_>>::new();
    for session in &sessions {
        groups
            .entry(session.task_id.clone())
            .or_default()
            .push(session);
    }
    Ok(groups
        .into_iter()
        .map(|(id, rows)| (id, CostSummary::of(rows)))
        .collect())
}

fn project_cost_json(
    tasks: &[crate::store::TaskRecord],
    costs: &std::collections::BTreeMap<String, CostSummary>,
) -> (Value, u32) {
    let (mut unfinished, mut finished, mut missing) = (0.0, 0.0, 0);
    for task in tasks {
        if let Some(cost) = costs.get(&task.id) {
            if task.state.is_terminal() {
                finished += cost.usd_total;
            } else {
                unfinished += cost.usd_total;
            }
            missing += cost.sessions_without_cost;
        }
    }
    (
        json!({"unfinished": unfinished, "finished": finished}),
        missing,
    )
}

fn project_list(ctx: &mut Ctx) -> DispatchResult {
    let projects = ctx.store.list_projects()?;
    let mut out = Vec::new();
    for p in projects {
        let tasks = ctx.store.list_tasks(&p.id)?;
        let costs = project_costs(ctx, &p.id)?;
        let (total, missing) = project_cost_json(&tasks, &costs);
        out.push(json!({
            "project": p,
            "cost_usd_total": total,
            "cost_sessions_without_cost": missing,
            "counts": crate::store::task_counts(&tasks),
            "slots_in_use": tasks.iter().filter(|t| t.state.occupies_slot()).count(),
        }));
    }
    Ok((json!({ "projects": out }), vec![]))
}

fn project_get(ctx: &mut Ctx, project_id: &str) -> DispatchResult {
    let project = ctx.store.get_project(project_id)?;
    let tasks = ctx.store.list_tasks(project_id)?;
    let costs = project_costs(ctx, project_id)?;
    let (total, missing) = project_cost_json(&tasks, &costs);
    let global = config_io::load_global(&ctx.autome_home)?;
    let (resolved, config_error) =
        match config_io::load_project(std::path::Path::new(&project.path)) {
            Ok(local) => (Some(config::resolve(&global, &local)), None),
            Err(error) => (None, Some(error.to_string())),
        };
    let home = ctx.home.to_string_lossy().into_owned();
    let inventory = skills::scan(&home, &project.path);
    let violations = resolved
        .as_ref()
        .map(|r| config::validate(r, &inventory))
        .unwrap_or_default();

    let positions: std::collections::BTreeMap<_, _> = ctx
        .store
        .queued_tasks(project_id)?
        .into_iter()
        .enumerate()
        .map(|(i, t)| (t.id, i + 1))
        .collect();
    let tasks_json = tasks
        .iter()
        .map(|t| {
            let mut value = task_json_with_config(
                ctx,
                t,
                costs.get(&t.id).unwrap_or(&CostSummary::default()),
                config_error.is_none(),
            )?;
            value["queue_position"] = json!(positions.get(&t.id));
            Ok(value)
        })
        .collect::<Result<Vec<_>, DispatchError>>()?;

    Ok((
        json!({
            "project": project,
            "cost_usd_total": total,
            "cost_sessions_without_cost": missing,
            "tasks": tasks_json,
            "counts": crate::store::task_counts(&tasks),
            "config": resolved,
            "config_error": config_error,
            "violations": violations,
            "rules": rule_files(&project.path),
        }),
        vec![],
    ))
}

fn task_json(
    ctx: &Ctx,
    t: &crate::store::TaskRecord,
    cost: &CostSummary,
) -> Result<Value, DispatchError> {
    task_json_with_config(ctx, t, cost, true)
}

fn task_json_with_config(
    ctx: &Ctx,
    t: &crate::store::TaskRecord,
    cost: &CostSummary,
    config_ok: bool,
) -> Result<Value, DispatchError> {
    let after = ctx.store.task_after(&t.id)?;
    let mut blocked_by = Vec::new();
    for id in &after {
        let dependency = ctx.store.get_task(id)?;
        if !autome_domain::deps::is_satisfied(&dependency.state) {
            blocked_by.push(json!({
                "id": id,
                "state": dependency.state,
                "reason": autome_domain::deps::block_reason(&dependency.state),
            }));
        }
    }
    let mut value = json!({
        "id": t.id,
        "after": after,
        "blocked_by": blocked_by,
        "cost_usd": cost.usd_total,
        "cost_sessions_without_cost": cost.sessions_without_cost,
        "kind": t.kind,
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
        // Where this task's documents live inside the repository holding
        // them, frozen when it was created. The panel opens paths under it.
        "doc_dir": t.doc_dir(),
        "merge": if config_ok { crate::merge_queue::view(ctx, t).map_err(|e| internal(e.to_string()))? } else { Value::Null },
    });
    if t.kind == TaskKind::Bug {
        value["test_fix"] = crate::scheduler::test_fix::view(&ctx.store, &t.id)?;
        value["fix_round"] = json!(ctx.store.count_events(&t.id, "bug.fix_round_started")?);
    }
    Ok(value)
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
        return Err(rejected(add_rejection(&ctx.store, &path)?.unwrap().message));
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

    if let Some(reason) = add_rejection(&ctx.store, &path)? {
        return Err(rejected(reason.message));
    }
    let readding = ctx.store.project_by_path(&canonical_str)?.map(|p| p.id);

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

    let report = init::init_kind(&canonical, workspace)?;

    let global = config_io::load_global(&ctx.autome_home)?;
    let project = Project {
        id: readding.clone().unwrap_or_else(|| new_id("prj")),
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
    match &readding {
        Some(_) => ctx.store.replace_project(&project)?,
        None => ctx.store.insert_project(&project)?,
    }

    let seq = ctx.store.append_event(
        "project.added",
        &project.id,
        json!({ "path": project.path, "disposition": project.disposition }),
    )?;

    Ok((
        json!({
            "project": project,
            "protocol_ref": crate::protocol::current().0.to_wire(),
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
#[derive(serde::Serialize)]
struct Rejection {
    kind: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
}

fn add_rejection(
    store: &Store,
    path: &std::path::Path,
) -> Result<Option<Rejection>, DispatchError> {
    if path.exists() && !path.is_dir() {
        return Ok(Some(Rejection {
            kind: "not_a_directory",
            message: format!("{} 不是目录", path.display()),
            project_id: None,
        }));
    }
    // Resolve the existing ancestor too, so probing a not-yet-created child
    // gives the same answer as add after create_dir_all, without writing it.
    let canonical = if path.exists() {
        path.canonicalize()
            .map_err(|e| internal(format!("无法解析路径：{e}")))?
    } else {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| internal(e.to_string()))?
                .join(path)
        };
        let ancestor = absolute.ancestors().find(|p| p.exists()).unwrap();
        let mut resolved = ancestor
            .canonicalize()
            .map_err(|e| internal(e.to_string()))?;
        for component in absolute.strip_prefix(ancestor).unwrap().components() {
            match component {
                std::path::Component::ParentDir => {
                    resolved.pop();
                }
                std::path::Component::CurDir => {}
                other => resolved.push(other.as_os_str()),
            }
        }
        resolved
    };
    if let Some(existing) = store.project_by_path(&canonical.to_string_lossy())?
        && existing.is_active()
    {
        return Ok(Some(Rejection {
            kind: "already_project",
            message: "该目录已经是一个项目".into(),
            project_id: Some(existing.id),
        }));
    }
    for other in store.list_projects()? {
        let other_path = std::path::Path::new(&other.path);
        let kind = if canonical.starts_with(other_path) {
            "nested_in_project"
        } else if other_path.starts_with(&canonical) {
            "contains_project"
        } else {
            continue;
        };
        return Ok(Some(Rejection {
            kind,
            message: format!(
                "{} 与已有项目「{}」嵌套，不能同时管理",
                canonical.display(),
                other.display_name
            ),
            project_id: Some(other.id),
        }));
    }
    Ok(None)
}

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
            "rejection": add_rejection(&ctx.store, &path)?,
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
    let mut paths: Vec<String> = vec![".autome".into(), ".gitignore".into()];
    // Stage only the wizard's artefacts, never the user's entire docs tree.
    paths.extend(ONBOARDING_FILES.iter().map(|path| (*path).to_string()));
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
    let mut resolved_json = json!(resolved);
    resolved_json["gate_run"] = json!(resolved.gate_run);
    resolved_json["gate_quick"] = json!(resolved.gate_quick);
    resolved_json["setup"] = json!(resolved.setup);
    let defaults = config_io::resolved_defaults(&ctx.autome_home, &project_config)?;
    let home = ctx.home.to_string_lossy().into_owned();
    let inventory = match &repo {
        Some(path) => skills::scan(&home, path),
        None => skills::scan_global(&home),
    };
    let violations = config::validate(&resolved, &inventory);
    Ok((
        json!({
            "global": global,
            "resolved": resolved_json,
            "defaults": defaults,
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
    let session_minutes = params
        .get("session_minutes")
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
            if let Some(v) = session_minutes {
                next.loop_defaults.session_minutes = v;
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
            if params.get("session_minutes").is_some() {
                project_config.loop_overrides.session_minutes = session_minutes;
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
        ConfigViolation::SessionMinutesOutOfRange { value, min } => {
            format!("会话时长上限 {value} 分钟太短，至少 {min} 分钟")
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
    let after: Vec<String> = match params.get("after") {
        None => Vec::new(),
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|_| bad_params("after 必须是任务编号列表"))?,
    };
    let kind = match params.get("kind") {
        None | Some(Value::Null) => autome_domain::task::TaskKind::Feature,
        Some(value) => value
            .as_str()
            .and_then(autome_domain::task::TaskKind::parse)
            .ok_or_else(|| bad_params(format!("未知的任务类型 {value}：只能是 feature 或 bug")))?,
    };
    let brief = params
        .get("brief")
        .map(|v| {
            v.as_str()
                .ok_or_else(|| bad_params("任务卡正文必须是字符串"))
        })
        .transpose()?;
    if let Some(body) = brief {
        if body.trim().is_empty() {
            return Err(bad_params("任务卡正文不能为空"));
        }
        if body.len() > 65536 {
            return Err(bad_params(format!(
                "任务卡正文超过 64KB（{} 字节，上限 65536 字节）",
                body.len()
            )));
        }
    }
    let draft = params
        .get("draft")
        .map(|v| v.as_bool().ok_or_else(|| bad_params("draft 必须是布尔值")))
        .transpose()?
        .unwrap_or(false);
    let request = if let Some(body) = brief {
        match params.get("request") {
            Some(value) => value
                .as_str()
                .ok_or_else(|| bad_params("request 必须是字符串"))?
                .trim()
                .to_string(),
            None => body
                .lines()
                .find_map(|line| line.strip_prefix("# "))
                .or_else(|| {
                    body.lines()
                        .find(|line| !line.trim().is_empty())
                        .map(|line| {
                            line.trim_start_matches(|c: char| c == '#' || c.is_whitespace())
                        })
                })
                .unwrap_or("")
                .trim()
                .to_string(),
        }
    } else {
        str_param(params, "request")?.trim().to_string()
    };
    if request.is_empty() {
        return Err(bad_params(if brief.is_some() {
            "任务卡正文缺少标题：第一行写 `# 标题`"
        } else {
            "需求不能为空"
        }));
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

    create(
        ctx,
        project_id,
        &request,
        attachments,
        doc_refs,
        (kind, brief, draft),
        &after,
    )
}

fn prepare_task(
    ctx: &Ctx,
    project_id: &str,
    request: &str,
    attachments: Vec<String>,
    doc_refs: Vec<String>,
    card: (TaskKind, bool),
) -> Result<crate::store::TaskRecord, DispatchError> {
    let (kind, draft) = card;
    let project = ctx.store.get_project(project_id)?;
    let project_config = config_io::load_project(std::path::Path::new(&project.path))?;
    if kind == TaskKind::Bug && !draft {
        let global = config_io::load_global(&ctx.autome_home)?;
        if !config::resolve(&global, &project_config).is_enabled(Role::Audit) {
            return Err(rejected("Bug 任务需要独立复核，请启用审计角色后启动"));
        }
    }
    let doc_root = resolve_doc_root(&project, &project_config)?;

    let base = autome_domain::project::slugify(request);
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
        kind,
        id: id.clone(),
        project_id: project_id.to_string(),
        slug,
        // The intake session writes the real title; until then the request
        // itself is the most informative thing to show.
        title: first_line(request),
        request: request.to_string(),
        attachments,
        doc_refs,
        state: if draft {
            TaskState::Draft
        } else {
            TaskState::Queued
        },
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
    Ok(task)
}

fn create(
    ctx: &mut Ctx,
    project_id: &str,
    request: &str,
    attachments: Vec<String>,
    doc_refs: Vec<String>,
    card: (autome_domain::task::TaskKind, Option<&str>, bool),
    proposed_after: &[String],
) -> DispatchResult {
    let (kind, brief, draft) = card;
    let task = prepare_task(
        ctx,
        project_id,
        request,
        attachments,
        doc_refs,
        (kind, draft),
    )?;
    let id = task.id.clone();
    let after = validate_after(ctx, &id, project_id, proposed_after)?;
    ctx.store.create_task_after(&task, brief, &after)?;
    let mut created = json!({ "project_id": project_id });
    if !after.is_empty() {
        created["after"] = json!(after);
    }
    let seq = ctx.store.append_event("task.created", &id, created)?;

    // Start it immediately if the project has a free slot.
    let report = if draft {
        Default::default()
    } else {
        scheduler::tick(ctx)
    };
    let started = !draft && report.tasks_started.contains(&id);

    let task = ctx.store.get_task(&id)?;
    Ok((
        json!({
            "task": task_json(ctx, &task, &CostSummary::of(&ctx.store.list_sessions(&task.id)?))?,
            "started": started,
            "queue_position": queue_position(ctx, &task)?,
        }),
        vec![event(seq, "task.created", &id, json!({}))],
    ))
}

fn validate_after(
    ctx: &Ctx,
    id: &str,
    project_id: &str,
    proposed_after: &[String],
) -> Result<Vec<String>, DispatchError> {
    let mut lookup = std::collections::BTreeMap::new();
    if !proposed_after.is_empty() {
        for (id, edges) in ctx.store.project_after(project_id)? {
            lookup.insert(id, (project_id.to_string(), edges));
        }
        for id in proposed_after {
            if !lookup.contains_key(id) {
                match ctx.store.get_task(id) {
                    Ok(task) => {
                        lookup.insert(id.clone(), (task.project_id, Vec::new()));
                    }
                    Err(crate::store::StoreError::NotFound { .. }) => {}
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    autome_domain::deps::check_after(id, project_id, proposed_after, |id| lookup.get(id).cloned())
        .map_err(|e| rejected(e.to_string()))
}

fn task_set_after(ctx: &mut Ctx, p: &Value) -> DispatchResult {
    let id = str_param(p, "task_id")?;
    let proposed: Vec<String> =
        serde_json::from_value(p.get("after").cloned().unwrap_or(Value::Null))
            .map_err(|_| bad_params("after 必须是任务编号列表"))?;
    let task = ctx.store.get_task(id)?;
    autome_domain::deps::check_editable(
        id,
        task.kind,
        &task.state,
        ctx.store.has_impl_session(id)?,
    )
    .map_err(|e| rejected(e.to_string()))?;
    let after = validate_after(ctx, id, &task.project_id, &proposed)?;
    ctx.store.set_task_after(id, &after)?;
    let payload = json!({"after": after});
    let seq = ctx
        .store
        .append_event("task.after_set", id, payload.clone())?;
    let report = scheduler::tick(ctx);
    let task = ctx.store.get_task(id)?;
    Ok((
        json!({
            "task": task_json(ctx, &task, &CostSummary::of(&ctx.store.list_sessions(id)?))?,
            "started": report.tasks_started.iter().any(|started| started == id),
            "queue_position": queue_position(ctx, &task)?,
        }),
        vec![event(seq, "task.after_set", id, payload)],
    ))
}

fn task_move(ctx: &mut Ctx, p: &Value) -> DispatchResult {
    use autome_domain::deps::{Placement, reposition};
    let id = str_param(p, "task_id")?;
    let invalid = || bad_params("before、after、top 必须且只能给一个");
    let before = p
        .get("before")
        .map(|v| v.as_str().ok_or_else(invalid))
        .transpose()?;
    let after = p
        .get("after")
        .map(|v| v.as_str().ok_or_else(invalid))
        .transpose()?;
    let top = p
        .get("top")
        .map(|v| v.as_bool().ok_or_else(invalid))
        .transpose()?
        .unwrap_or(false);
    if usize::from(before.is_some()) + usize::from(after.is_some()) + usize::from(top) != 1 {
        return Err(invalid());
    }
    let movable = |state: &TaskState| {
        matches!(
            state,
            TaskState::Draft | TaskState::Queued | TaskState::Requeued { .. }
        )
    };
    let task = ctx.store.get_task(id)?;
    if !movable(&task.state) {
        return Err(rejected("只有排队或待开始的任务可以移动"));
    }
    let reference = before.or(after);
    if let Some(reference) = reference {
        if reference == id {
            return Err(rejected("不能以任务自己为参照移动"));
        }
        let missing = || {
            rejected(format!(
                "{reference} 不在队列里：只能参照同项目排队中或待开始的任务"
            ))
        };
        match ctx.store.get_task(reference) {
            Ok(other) if other.project_id == task.project_id && movable(&other.state) => {}
            Ok(_) | Err(crate::store::StoreError::NotFound { .. }) => return Err(missing()),
            Err(e) => return Err(e.into()),
        }
    }
    let mut order: Vec<String> = ctx
        .store
        .queue_order(&task.project_id)?
        .into_iter()
        .map(|t| t.id)
        .collect();
    for candidate in reference.into_iter().chain(std::iter::once(id)) {
        if !order.iter().any(|item| item == candidate) {
            order.push(candidate.into());
        }
    }
    let placement = match (before, after) {
        (Some(id), _) => Placement::Before(id),
        (_, Some(id)) => Placement::After(id),
        _ => Placement::Top,
    };
    let order = reposition(&order, id, placement).map_err(|e| rejected(e.to_string()))?;
    ctx.store.reorder_queue(&task.project_id, &order)?;
    let position = queue_position(ctx, &task)?;
    let payload = json!({"task": id, "project_id": task.project_id, "before": before, "after": after, "top": top, "queue_position": position});
    let seq = ctx.store.append_event("task.moved", id, payload.clone())?;
    Ok((
        json!({"task": task_json(ctx, &task, &CostSummary::of(&ctx.store.list_sessions(id)?))?, "queue_position": position, "order": order}),
        vec![event(seq, "task.moved", id, payload)],
    ))
}

fn task_start(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    scheduler::apply_trigger(ctx, task_id, &Trigger::Start)?;
    if ctx.store.queued_seq(task_id)?.is_none() {
        ctx.store.mark_queued(task_id)?;
    }
    let task = ctx.store.get_task(task_id)?;
    let seq = ctx.store.append_event(
        "task.started",
        task_id,
        json!({"project_id": task.project_id}),
    )?;
    let report = scheduler::tick(ctx);
    let task = ctx.store.get_task(task_id)?;
    Ok((
        json!({
            "task": task_json(ctx, &task, &CostSummary::of(&ctx.store.list_sessions(&task.id)?))?,
            "started": report.tasks_started.iter().any(|id| id == task_id),
            "queue_position": queue_position(ctx, &task)?,
        }),
        vec![event(seq, "task.started", task_id, json!({}))],
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
    if !matches!(task.state, TaskState::Queued | TaskState::Requeued { .. }) {
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
    if ctx.store.get_task(task_id)?.kind == TaskKind::Bug {
        return Err(rejected("Bug 任务没有复盘"));
    }
    scheduler::start_retro(ctx, task_id)?;
    let seq = ctx
        .store
        .append_event("task.retro_started", task_id, json!({}))?;
    Ok((
        json!({ "started": true }),
        vec![event(seq, "task.retro_started", task_id, json!({}))],
    ))
}

fn project_warnings(ctx: &Ctx, project: &Project) -> Result<Vec<String>, DispatchError> {
    let global = config_io::load_global(&ctx.autome_home)?;
    let local = config_io::load_project(std::path::Path::new(&project.path))?;
    Ok(config::warnings(&config::resolve(&global, &local)))
}

fn task_note(ctx: &mut Ctx, p: &Value) -> DispatchResult {
    let id = str_param(p, "task_id")?;
    let text = str_param(p, "text")?;
    if p.get("reopen").is_some_and(|v| !v.is_boolean()) {
        return Err(bad_params("reopen 必须是布尔值"));
    }
    if text.trim().is_empty() {
        return Err(rejected("备注不能为空"));
    }
    // Match the web gateway's JavaScript string length (UTF-16 code units).
    if text.encode_utf16().count() > 8000 {
        return Err(rejected("备注不能超过 8000 字符"));
    }
    let task = ctx.store.get_task(id)?;
    match task.state {
        TaskState::Done => {
            return Err(rejected(
                "任务已经完成，备注不会再被任何一轮读到；有新问题请新建任务",
            ));
        }
        TaskState::Cancelled => {
            return Err(rejected(
                "任务已经取消，备注不会再被任何一轮读到；有新问题请新建任务",
            ));
        }
        _ => {}
    }
    let reopen = matches!(
        task.state,
        TaskState::Active {
            node: Node::AwaitMerge
        }
    );
    if reopen && p["reopen"] != true {
        return Err(rejected(if task.kind == TaskKind::Bug {
            "任务已在等你合并；加 --reopen 会把它送回修复轮处理这条备注，之后重新复核与过门禁"
        } else {
            "任务已在等你合并；加 --reopen 会把它送回实现轮处理这条备注，之后重新审计与过门禁"
        }));
    }
    if reopen && task.kind == TaskKind::Bug {
        let project = ctx.store.get_project(&task.project_id)?;
        let config = scheduler::resolve_config(ctx, &project)?;
        if scheduler::bug_progress(ctx, &task)?.fix_round
            >= task.budget_n.unwrap_or(config.bug.fix_rounds)
        {
            return Err(rejected("修复预算已用完，请先 autome extend"));
        }
    }
    let late = matches!(
        task.state,
        TaskState::Active {
            node: Node::Merging | Node::MainCheck | Node::Cleanup
        }
    );
    let n = ctx.store.count_events(id, "task.note_added")? + 1;
    let mut payload = json!({"n":n, "text":text});
    if late {
        payload["late"] = json!(true);
    }
    let seq = ctx
        .store
        .append_event("task.note_added", id, payload.clone())?;
    if reopen {
        scheduler::apply_trigger(ctx, id, &Trigger::ReopenForNote)?;
    }
    let mut out = json!({"n":n});
    if late {
        out["warning"] = json!("任务已经在合并，这条备注不会被任何一轮读到；要处理请新建任务");
    }
    Ok((out, vec![event(seq, "task.note_added", id, payload)]))
}

fn task_get(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    let task = ctx.store.get_task(task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    // The status block is read from the checkout holding the documents, which
    // is a different place from the session's working directory once a project
    // can be a workspace. This used to spell the path out again by hand,
    // independently of the scheduler's copy of the same rule.
    // `of`, not `single`: a workspace task's documents live inside the member
    // repository that holds them, and `single` would look one directory up —
    // where the panel found nothing and said "无法查看设计稿" about a design
    // document that was right there.
    let layout = crate::layout::TaskLayout::of(&project, &task.slug, &[]);

    let source = DocumentSource::for_task(&project, &task);
    let doc = source.status(&task);
    let status = doc.block();
    let sessions = ctx.store.list_sessions(task_id)?;
    let decisions = ctx.store.list_decisions(task_id)?;
    let mut last = Value::Null;
    if let Some((seq, _, mut payload)) = ctx
        .store
        .last_event_of(task_id, &["session.scope_restore_begun"])?
    {
        if let Some(event) = ctx.store.events_since_with_at(seq - 1, 1)?.first() {
            payload["at"] = json!(event.at);
        }
        last = payload;
    }
    let pending = crate::scope::pending(&ctx.store, task_id)
        .map_err(|e| internal(e.to_string()))?
        .into_iter()
        .flat_map(|plan| plan.repos.into_iter().map(|repo| repo.patch))
        .collect::<Vec<_>>();

    Ok((
        json!({
            "task": task_json(ctx, &task, &CostSummary::of(&sessions))?,
            "project": { "id": project.id, "name": project.display_name, "default_branch": project.default_branch },
            "scope_restore": {"last": last, "pending": pending},
            "notes": ctx.store.operator_notes(task_id)?,
            "brief": ctx.store.task_brief(task_id)?,
            "gate_queue_lane": crate::gate_queue::waiters(ctx).map_err(|e| internal(e.to_string()))?.iter().find(|w| w.task_id == task_id).map(|w| w.lane),
            "gate_queue_position": autome_domain::gate_queue::position(&crate::gate_queue::waiters(ctx).map_err(|e| internal(e.to_string()))?, task_id),
            "queue_position": queue_position(ctx, &task)?,
            "status_block": doc.json(),
            "status_error": doc.error().is_some(),
            "status_error_detail": doc.error(),
            "sessions": sessions.iter().map(|s| session_json(ctx, &project, s)).collect::<Vec<_>>(),
            "cost": autome_domain::metrics::CostSummary::of(&sessions),
            "decisions": decisions
                .iter()
                .map(|d| decision_json(d, status))
                .collect::<Vec<_>>(),
            "pending_decisions": ctx.store.pending_decisions(task_id)?,
            "documents": source.documents(&task),
            // What "打开 worktree" opens: the session's working directory,
            // which for a workspace is the directory the member checkouts sit
            // in rather than a checkout itself.
            "worktree": layout.cwd.to_string_lossy(),
            "next_role": scheduler::next_role(&task.state),
            // Running totals, for a task still in flight. `task.metrics` is
            // the final aggregate and only exists once the task has ended.
            "so_far": running_totals(&sessions),
            // A crashed round waiting to start again, or one deferred by a
            // rate-limit cooldown.
            "retry": retry_json(ctx, task_id)?,
            // The core's last run of the project gate on this task.
            "gate": ctx.store.last_event(task_id, "gate.result")?,
            "config_warnings": project_warnings(ctx, &project)?,
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
    Bug(autome_domain::bug_doc::BugDoc),
}

impl DocRead {
    fn json(&self) -> Value {
        match self {
            Self::Ok(block) => status_json(block),
            Self::Bug(block) => json!(block),
            _ => Value::Null,
        }
    }

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

/// After cleanup, membership and content come from the default branch, never HEAD.
struct DocumentSource {
    root: std::path::PathBuf,
    branch: Option<String>,
}

impl DocumentSource {
    fn for_task(project: &Project, task: &crate::store::TaskRecord) -> Self {
        let worktree = std::path::PathBuf::from(project.docs_worktree(&task.slug));
        if worktree.join(task.doc_dir()).is_dir() {
            return Self {
                root: worktree,
                branch: None,
            };
        }
        let root = project.docs_repo.as_ref().map_or_else(
            || project.path.clone(),
            |member| project.member_path(member),
        );
        Self {
            root: root.into(),
            branch: Some(project.default_branch.clone()),
        }
    }

    fn status(&self, task: &crate::store::TaskRecord) -> DocRead {
        let path = if task.kind == TaskKind::Bug {
            task.bug_doc()
        } else {
            task.design_doc()
        };
        let text = match &self.branch {
            Some(branch) => git::run_ok(&self.root, &["show", &format!("{branch}:{path}")])
                .ok()
                .map(|out| out.stdout),
            None => std::fs::read_to_string(self.root.join(path)).ok(),
        };
        let Some(text) = text else {
            return DocRead::Absent;
        };
        if task.kind == TaskKind::Bug {
            return match autome_domain::bug_doc::parse(&text) {
                Ok(block) => DocRead::Bug(block),
                Err(error) => DocRead::Unreadable(error.to_string()),
            };
        }
        match status_block::parse(&text) {
            Ok(block) => DocRead::Ok(Box::new(block)),
            Err(error) => DocRead::Unreadable(error.to_string()),
        }
    }

    fn documents(&self, task: &crate::store::TaskRecord) -> Vec<Value> {
        let mut docs = documents(&self.root, task);
        if let Some(branch) = &self.branch {
            let Ok(tree) = git::run_ok(
                &self.root,
                &[
                    "ls-tree",
                    "-r",
                    "--name-only",
                    "-z",
                    branch,
                    "--",
                    &format!("{}/", task.doc_dir()),
                ],
            ) else {
                return vec![];
            };
            let paths: std::collections::HashSet<_> = tree.stdout.split('\0').collect();
            docs.retain(|doc| paths.contains(doc["path"].as_str().unwrap_or_default()));
            for doc in &mut docs {
                doc["source"] = json!("default_branch");
            }
        }
        docs
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

fn session_json(ctx: &Ctx, project: &Project, s: &autome_domain::session::Session) -> Value {
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
        "timed_out": matches!(s.lifecycle, autome_domain::session::SessionLifecycle::TimedOut { .. }),
        "limit_minutes": scheduler::effective_limit(ctx, s, project),
        "elapsed_minutes": s.is_running().then(|| scheduler::secs_since(&s.started_at).map(|secs| secs / 60)).flatten(),
        // Where the readable log is, for `autome logs --follow`. Read-only:
        // nothing the renderer sends back ever names a path.
        "log_path": s.log_path,
        "protocol_ref": s.protocol_ref,
        // Absent rather than zeroed when the stream could not be read: a card
        // showing "0 tokens" for a session that produced work is worse than a
        // card showing nothing.
        "metrics": (!s.metrics.is_empty()).then(|| s.metrics.clone()),
    })
}

/// What a task has spent so far, summed across its finished sessions.
/// This legacy measured-only view is preserved; `cost` uses the task's
/// missing-price-as-zero convention and separately counts missing prices.
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

/// One decision, with its text taken from the document rather than from the
/// row.
///
/// The row remembers what the *user* decided; the prose it is about belongs to
/// the design document, which is where every other fact about a task's
/// progress lives. Reading it back from there means the panel shows what the
/// document says today — including the rest of a sentence that an older
/// parser cut off, without waiting for a session to end and re-sync it.
///
/// Falls back to the stored copy when the item is no longer in the document:
/// a decision the user already made is still theirs to see.
fn decision_json(d: &crate::store::DecisionRecord, status: Option<&StatusBlock>) -> Value {
    let fresh = status.and_then(|s| match d.kind.as_str() {
        "backlog" => s
            .backlog
            .iter()
            .find(|b| b.id == d.item_id)
            .map(|b| b.text.clone()),
        "dispute" => s
            .disputes
            .iter()
            .find(|x| x.id == d.item_id)
            .map(|x| x.text.clone()),
        _ => None,
    });
    json!({
        "kind": d.kind,
        "item_id": d.item_id,
        "text": fresh.unwrap_or_else(|| d.text.clone()),
        "disposition": d.disposition,
        "ruling": d.ruling,
        "consumed": d.consumed_at.is_some(),
        // A round has taken it and has not finished yet.
        "carried": d.carried_by.is_some(),
    })
}

/// The five protocol documents plus the task file, with their timestamps —
/// the panel's "产物" card (requirement T-13).
fn documents(worktree: &std::path::Path, task: &crate::store::TaskRecord) -> Vec<Value> {
    let dir = task.doc_dir();
    let slug = &task.slug;
    let names = [
        (format!("{slug}-card.md"), "任务卡"),
        (
            if task.kind == TaskKind::Bug {
                format!("{slug}-bug.md")
            } else {
                format!("{slug}.md")
            },
            if task.kind == TaskKind::Bug {
                "Bug 文档"
            } else {
                "设计文档"
            },
        ),
        (format!("{slug}-task.md"), "任务文件"),
        (format!("{slug}-review.md"), "评审"),
        (format!("{slug}-adjudication.md"), "裁决"),
        (format!("{slug}-audit.md"), "审计"),
        ("retro.md".to_string(), "运行记录"),
        ("lessons.md".to_string(), "复盘教训"),
    ];
    let mut out: Vec<Value> = names
        .iter()
        .filter_map(|(name, label)| {
            let path = worktree.join(&dir).join(name);
            let meta = std::fs::metadata(&path).ok()?;
            if !meta.is_file() {
                return None;
            }
            Some(json!({
                "name": name,
                "label": label,
                "path": format!("{dir}/{name}"),
                "absolute": path.to_string_lossy(),
                "size": meta.len(), "source": "worktree",
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
                let meta = entry.metadata().ok()?;
                if !meta.is_file() {
                    return None;
                }
                let size = meta.len();
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
                "size": size, "source": "worktree",
            })
        }));
    }
    out
}

/// Records merge intent without bypassing approval or changing nodes directly.
fn task_merge(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    use autome_domain::merge_queue::{MergePress, cancel_press, merge_press};
    let id = str_param(params, "task_id")?;
    let cancel = match params.get("cancel") {
        None => false,
        Some(value) => value
            .as_bool()
            .ok_or_else(|| bad_params("cancel 必须是布尔值"))?,
    };
    let task = ctx.store.get_task(id)?;
    let result;
    if cancel {
        cancel_press(&task.state).map_err(rejected)?;
        if crate::merge_queue::authorization(ctx, &task)
            .map_err(|e| internal(e.to_string()))?
            .is_none()
        {
            return Err(rejected("没有可撤销的合并授权"));
        }
        ctx.store
            .append_event("merge.auto", id, json!({"active":false,"by":"user"}))?;
        if crate::merge_queue::holds(ctx, &task).map_err(|e| internal(e.to_string()))? {
            ctx.store
                .append_event("merge.unlocked", id, json!({"reason":"cancel"}))?;
        }
        result = "revoked";
    } else {
        match merge_press(&task.state).map_err(rejected)? {
            MergePress::Merge => {
                scheduler::apply_trigger(ctx, id, &Trigger::Merge)?;
                result = "merging";
            }
            MergePress::Authorize => {
                if crate::merge_queue::authorization(ctx, &task)
                    .map_err(|e| internal(e.to_string()))?
                    .is_some()
                {
                    result = "already_authorized";
                } else {
                    ctx.store
                        .append_event("merge.auto", id, json!({"active":true,"by":"user"}))?;
                    result = "authorized";
                }
            }
        }
    }
    scheduler::tick(ctx);
    let (mut payload, _) = task_get(ctx, id)?;
    payload["merge_request"] = json!(result);
    let seq = ctx.store.latest_seq()?;
    Ok((payload, vec![event(seq, "task.updated", id, json!({}))]))
}

fn task_trigger(ctx: &mut Ctx, task_id: &str, trigger: Trigger) -> DispatchResult {
    if matches!(trigger, Trigger::Resume)
        && let Some(s) = ctx.store.running_session(task_id)?
        && scheduler::is_manual_retro(ctx, &s)?
    {
        return Err(rejected("手动复盘轮还在跑，结束后再恢复"));
    }
    let previous_session = ctx.store.running_session(task_id)?;
    scheduler::apply_trigger(ctx, task_id, &trigger)?;
    let current_session = ctx.store.running_session(task_id)?;
    let started_session = current_session.as_ref().is_some_and(|current| {
        previous_session.as_ref().map(|previous| &previous.id) != Some(&current.id)
    });
    // Return the node this command started, even if its session finishes
    // immediately. The next scheduling pass will reap it. Other triggers
    // still tick to fill a freed slot or run a pending core step.
    if !started_session {
        scheduler::tick(ctx);
    }
    let seq = ctx.store.latest_seq()?;
    let (payload, _) = task_get(ctx, task_id)?;
    Ok((
        payload,
        vec![event(seq, "task.updated", task_id, json!({}))],
    ))
}

fn task_convert(ctx: &mut Ctx, params: &Value) -> DispatchResult {
    let id = str_param(params, "task_id")?;
    match params.get("kind") {
        None => return Err(bad_params("缺少 kind：目前只能改建成需求（feature）")),
        Some(v) if v == "feature" => {}
        _ => return Err(bad_params("只能改建成需求任务：kind 必须是 feature")),
    }
    let start = params
        .get("start")
        .map(|v| v.as_bool().ok_or_else(|| bad_params("start 必须是布尔值")))
        .transpose()?
        .unwrap_or(false);
    let source = ctx.store.get_task(id)?;
    if source.kind != TaskKind::Bug {
        return Err(rejected(format!(
            "只有 Bug 任务可以改建成需求：{id} 是需求任务"
        )));
    }
    if source.state
        != (TaskState::Active {
            node: Node::NotABug,
        })
        || source.is_archived()
    {
        return Err(rejected(format!(
            "只有停在『不是 Bug』等你决定的任务可以改建：{id} 现在是 {}",
            serde_json::to_string(&source.state).map_err(|e| internal(e.to_string()))?
        )));
    }
    let project = ctx.store.get_project(&source.project_id)?;
    if !project.is_active() {
        return Err(rejected("项目已移除"));
    }
    for attachment in &source.attachments {
        let path = std::path::Path::new(attachment);
        if !path.is_file() {
            let copy = crate::layout::TaskLayout::of(&project, &source.slug, &[])
                .docs_root
                .join(source.doc_dir())
                .join("attachments")
                .join(path.file_name().unwrap_or_default());
            let hint = if copy.is_file() {
                format!("；原任务里的副本在 {}，放回原处后重试", copy.display())
            } else {
                String::new()
            };
            return Err(rejected(format!("附件不存在：{attachment}{hint}")));
        }
    }
    let brief = ctx.store.task_brief(id)?;
    let mut task = prepare_task(
        ctx,
        &source.project_id,
        &source.request,
        source.attachments.clone(),
        source.doc_refs.clone(),
        (TaskKind::Feature, !start),
    )?;
    task.title.clone_from(&source.title);
    scheduler::check_convert_cancel(ctx, &source, &project)?;
    stop_running_session(ctx, id)?;
    let seqs = ctx.store.convert_task(id, &task, brief.as_deref())?;
    if let Err(e) = scheduler::write_task_metrics(ctx, id) {
        tracing::warn!("改建后写指标失败：{e}");
    }
    scheduler::discard_checkout(ctx, &source, &project);
    let report = scheduler::tick(ctx);
    let task = ctx.store.get_task(&task.id)?;
    Ok((
        json!({"task": task_json(ctx, &task, &CostSummary::of(&ctx.store.list_sessions(&task.id)?))?, "started": report.tasks_started.contains(&task.id), "queue_position": queue_position(ctx, &task)?, "converted_from": id}),
        vec![
            event(
                seqs[0],
                "task.created",
                &task.id,
                json!({"project_id": task.project_id, "converted_from": id}),
            ),
            event(
                seqs[1],
                "task.converted",
                id,
                json!({"from": id, "to": task.id}),
            ),
            event(
                seqs[2],
                "task.updated",
                id,
                json!({"state": TaskState::Cancelled, "trigger": "cancel"}),
            ),
        ],
    ))
}

/// Cancel kills the running session first, for the same reason stop does —
/// and more urgently, because cancel then *deletes the checkout underneath
/// it*. A cancelled task used to leave its session running: a CLI writing
/// into a directory the core was removing, still costing money, for a task
/// the user had just said they did not want.
fn task_cancel(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    stop_running_session(ctx, task_id)?;
    task_trigger(ctx, task_id, Trigger::Cancel)
}

/// Stop kills the running session first, then applies the trigger — the
/// reverse order would leave a process writing to a log for a task the store
/// has already moved on from (requirement T-08).
fn task_stop(ctx: &mut Ctx, task_id: &str) -> DispatchResult {
    if matches!(
        ctx.store.get_task(task_id)?.state,
        TaskState::Requeued { .. }
    ) {
        return task_trigger(ctx, task_id, Trigger::Stop);
    }
    stop_running_session(ctx, task_id)?;
    task_trigger(ctx, task_id, Trigger::Stop)
}

/// Kills whatever session this task has running and records it as killed.
fn stop_running_session(ctx: &mut Ctx, task_id: &str) -> Result<(), DispatchError> {
    // A gate the task was waiting on goes with it: nobody will read the
    // result, and a test suite must not keep running for nothing.
    if let Ok(task) = ctx.store.get_task(task_id)
        && let Ok(project) = ctx.store.get_project(&task.project_id)
    {
        crate::gate::stop_all(std::path::Path::new(&project.path), task_id);
        let layout = crate::layout::TaskLayout::of(
            &project,
            &task.slug,
            &scheduler::working_repos(&project, &task),
        );
        scheduler::test_fix::cleanup(&project, &task, &layout)
            .map_err(|e| internal(e.to_string()))?;
        crate::gate::discard_node(std::path::Path::new(&project.path), task_id, "setup")
            .map_err(|e| internal(e.to_string()))?;
        crate::gate::discard_node(std::path::Path::new(&project.path), task_id, "after_merge")
            .map_err(|e| internal(e.to_string()))?;
    }
    if let Some(session) = ctx.store.running_session(task_id)? {
        if let Some(pid) = session.pid {
            let _ = crate::launcher::stop_session(pid);
        }
        let ended_at = now_iso();
        let lifecycle = autome_domain::session::SessionLifecycle::Killed;
        ctx.store
            .finish_session(&session.id, &lifecycle, &ended_at)?;
        let task = ctx.store.get_task(task_id)?;
        let project = ctx.store.get_project(&task.project_id)?;
        scheduler::record_usage(ctx, &session, &project, &task, &ended_at);
        scheduler::record_session_ended(&ctx.store, &session, task_id, &lifecycle, &ended_at)?;
        // The round never finished, so whatever it carried was not applied.
        ctx.store.release_decisions(&session.id)?;
    }
    Ok(())
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
    let merge_records = ctx.store.task_events(task_id, "merge.repo")?;
    let mut working = crate::scheduler::working_repos(&project, &task);
    if project.is_workspace() {
        // Cleanup removes the worktrees and their repos declaration. The merge
        // ledger still identifies every member whose history must be shown.
        for record in &merge_records {
            if record["commit"].is_string()
                && let Some(name) = record["repo"].as_str()
                && !working.iter().any(|n| n == name)
            {
                working.push(name.to_owned());
            }
        }
    }
    let layout = crate::layout::TaskLayout::of(&project, &task.slug, &working);

    // One row per repository the task worked in, in the order the merge will
    // take them. A single-repository project has exactly one, which is what
    // the panel drew before workspaces existed.
    let mut repos: Vec<Value> = Vec::new();
    for slot in &layout.repos {
        let archived = !git::branch_exists(&slot.repo_root, &slot.branch);
        let merge_commit = if archived && project.is_workspace() {
            merge_records
                .iter()
                .rev()
                .find(|e| e["repo"] == slot.name && e["commit"].is_string())
                .and_then(|e| e["commit"].as_str().map(str::to_owned))
        } else {
            task.merge_commit.clone()
        };
        let (base, branch) = if archived {
            let Some(commit) =
                merge_commit.filter(|c| git::second_parent(&slot.repo_root, c).is_ok())
            else {
                continue;
            };
            (format!("{commit}^1"), commit)
        } else {
            (slot.base.clone(), slot.branch.clone())
        };
        let summary = git::change_summary(&slot.repo_root, &base, &branch)?;
        let subjects = git::commit_subjects(&slot.repo_root, &base, &branch)?;
        // Already in, from an earlier press or because the task changed
        // nothing here. `merge_all` skips exactly this case, so the panel has
        // to show it rather than offering to merge nothing.
        let merged = archived
            || git::is_ancestor(&slot.repo_root, &slot.branch, &slot.base).unwrap_or(false);
        // The same judgement `merge_task_branch` makes, not a stricter one:
        // the panel must not tell the user a merge is blocked that the core
        // would happily perform, nor the reverse.
        let blocking = git::conflicting_dirty_paths(&slot.repo_root, &slot.base, &slot.branch)
            .unwrap_or_default();
        let on_top = git::is_ancestor(&slot.repo_root, &slot.base, &slot.branch).unwrap_or(false);
        repos.push(json!({
            "name": slot.name,
            "branch": slot.branch,
            "into": slot.base,
            "commits": summary.commits,
            "files": summary.files.iter().map(|f| { let mut row = json!({
                "path": f.path, "added": f.added, "deleted": f.deleted, "status": f.status
            }); if let Some(from) = &f.from { row["from"] = json!(from); } row }).collect::<Vec<_>>(),
            "deleted_count": summary.files.iter().filter(|f| f.status == "deleted").count(),
            "total_added": summary.total_added,
            "total_deleted": summary.total_deleted,
            "subjects": subjects,
            "merged": merged,
            "unchanged": summary.commits == 0,
            "mergeable": blocking.is_empty(),
            "behind": !on_top,
            "blocked_by": if !blocking.is_empty() {
                json!({ "kind": "dirty_worktree", "paths": blocking })
            } else {
                Value::Null
            },
        }));
    }

    if repos.is_empty() {
        return Ok((json!({ "available": false }), vec![]));
    }

    // What is left to do decides whether the button is offered: a repository
    // already merged, or with nothing in it, is not a reason to refuse.
    let outstanding: Vec<&Value> = repos
        .iter()
        .filter(|r| r["merged"] != json!(true) && r["unchanged"] != json!(true))
        .collect();
    let mergeable =
        !outstanding.is_empty() && outstanding.iter().all(|r| r["mergeable"] == json!(true));
    let blocked_by = outstanding
        .iter()
        .find(|r| r["mergeable"] != json!(true))
        .map(|r| r["blocked_by"].clone())
        .unwrap_or(Value::Null);

    let archived = layout
        .repos
        .iter()
        .all(|slot| !git::branch_exists(&slot.repo_root, &slot.branch));
    let total = |key: &str| -> i64 {
        repos
            .iter()
            .filter(|r| archived || r["merged"] != json!(true))
            .filter_map(|r| r[key].as_i64())
            .sum()
    };

    Ok((
        json!({
            "available": true,
            "repos": repos,
            // The single-repository shape, kept so every existing caller and
            // test reads the same fields: for one repository these *are* that
            // repository's numbers, and for a workspace they are the totals of
            // everything still to merge.
            "branch": task.branch(),
            "into": layout.docs().base,
            "commits": total("commits"),
            "files": repos.iter().flat_map(|r| r["files"].as_array().cloned().unwrap_or_default()).collect::<Vec<_>>(),
            "total_added": total("total_added"),
            "total_deleted": total("total_deleted"),
            "deleted_count": repos.iter().filter_map(|r| r["deleted_count"].as_u64()).sum::<u64>(),
            "merged": repos.iter().all(|r| r["merged"] == true),
            "subjects": repos.iter().flat_map(|r| r["subjects"].as_array().cloned().unwrap_or_default()).collect::<Vec<_>>(),
            "mergeable": mergeable,
            "rebase_first": outstanding.iter().any(|r| r["behind"] == json!(true)),
            // What pressing merge will actually do. A Backlog item marked
            // 纳入 turns into new work at this stopping point (T-09), so the
            // task goes back to the implementation loop instead of merging —
            // and the panel that says 合并到 main has to know, or it promises
            // something the core will not do.
            "pending_decisions": ctx.store.pending_decisions(task_id)?,
            "blocked_by": blocked_by,
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

    let task = ctx.store.get_task(&session.task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
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
            "session": session_json(ctx, &project, &session),
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
    let gate_waiters =
        crate::gate_queue::display_waiters(ctx).map_err(|e| internal(e.to_string()))?;
    let mut waiting = Vec::new();
    let mut running = Vec::new();
    let mut queued = Vec::new();
    let mut config_warnings = Vec::new();
    let mut projects = Vec::new();

    for project in ctx.store.list_projects()? {
        let costs = project_costs(ctx, &project.id)?;
        let (warnings, config_error) = match project_warnings(ctx, &project) {
            Ok(warnings) => (warnings, None),
            Err(error) => (Vec::new(), Some(error.message)),
        };
        projects.push(json!({"project": project, "config_error": config_error}));
        if !warnings.is_empty() {
            config_warnings.push(json!({"project_id":project.id,"project_name":project.display_name,"warnings":warnings}));
        }
        for (index, task) in ctx.store.queued_tasks(&project.id)?.iter().enumerate() {
            queued.push(json!({
                "project_id": project.id,
                "project_name": project.display_name,
                "queue_position": index + 1,
                "gate_queue_lane": gate_waiters.iter().find(|w| w.task_id == task.id).map(|w| w.lane),
                "gate_queue_position": autome_domain::gate_queue::position(&gate_waiters, &task.id),
                "task": task_json_with_config(ctx, task, costs.get(&task.id).unwrap_or(&CostSummary::default()), config_error.is_none())?,
                "pending_decisions": ctx.store.pending_decisions(&task.id)?,
                "session": ctx.store.running_session(&task.id)?.as_ref().map(|s| session_json(ctx, &project, s)),
                "retry": retry_json(ctx, &task.id)?,
            }));
        }
        for task in ctx.store.list_tasks(&project.id)? {
            if task.state.is_terminal() {
                continue;
            }
            let pending = ctx.store.pending_decisions(&task.id)?;
            let entry = json!({
                "project_id": project.id,
                "project_name": project.display_name,
                "gate_queue_lane": gate_waiters.iter().find(|w| w.task_id == task.id).map(|w| w.lane),
                "gate_queue_position": autome_domain::gate_queue::position(&gate_waiters, &task.id),
                "task": task_json_with_config(ctx, &task, costs.get(&task.id).unwrap_or(&CostSummary::default()), config_error.is_none())?,
                "pending_decisions": pending,
                "session": ctx.store.running_session(&task.id)?.as_ref().map(|s| session_json(ctx, &project, s)),
                "retry": retry_json(ctx, &task.id)?,
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

    // Project-level alerts sit beside "waiting": they want a person and belong
    // to no running task — main failing its gate after a merge, so far.
    let alerts: Vec<Value> = ctx.store.open_alerts()?.iter().map(alert_json).collect();
    let cooldowns: Vec<Value> = ctx
        .store
        .active_cooldowns()?
        .into_iter()
        .map(|(runtime, until, reason)| json!({ "runtime": runtime, "until": until, "reason": reason }))
        .collect();

    Ok((
        json!({
            "projects": projects,
            "config_warnings": config_warnings,
            "waiting": waiting,
            "running": running,
            "queued": queued,
            "alerts": alerts,
            "cooldowns": cooldowns,
            "environment": environment,
        }),
        vec![],
    ))
}

fn alert_json(a: &crate::store::AlertRecord) -> Value {
    json!({
        "id": a.id,
        "project_id": a.project_id,
        "kind": a.kind,
        "task_id": a.task_id,
        "detail": a.detail,
        "created_at": a.created_at,
    })
}

/// A crashed or deferred round waiting to start again, for the panel's
/// "重试 / 冷却到 hh:mm" line. `null` when there is none.
fn retry_json(ctx: &Ctx, task_id: &str) -> Result<Value, DispatchError> {
    Ok(match ctx.store.pending_retry(task_id)? {
        Some((node, not_before, _)) => json!({
            "node": node.as_str(),
            "not_before": not_before,
            "attempts": ctx.store.retry_attempts(task_id)?,
        }),
        None => Value::Null,
    })
}

/// `project.alert.dismiss`: the user has seen it.
fn alert_dismiss(ctx: &mut Ctx, alert_id: &str) -> DispatchResult {
    ctx.store.dismiss_alert(alert_id)?;
    let seq = ctx
        .store
        .append_event("project.alert_dismissed", alert_id, json!({}))?;
    Ok((
        json!({ "dismissed": alert_id }),
        vec![event(seq, "dashboard.updated", alert_id, json!({}))],
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
            // Another driver held the ledger for this tick; nothing ran.
            "skipped": report.skipped,
            "older_kernel": report.older_kernel,
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
    let mut payload = env_json(&ctx.environment);
    let ledger = ctx.store.kernel_mark()?;
    let old = crate::kernel_guard::check_recorded(ctx, ledger.clone());
    payload["kernel"] = json!({
        "build_id": ctx.build_id,
        "origin": if ctx.build_id.starts_with("unknown-") { "unknown" } else { "git" },
        "ledger_build_id": ledger,
        "may_advance": old.is_none(),
        "message": old.map(|o| o.message),
    });
    Ok((payload, vec![]))
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
    let events = match params.get("subject") {
        None => ctx.store.events_since_with_at(after, limit)?,
        Some(Value::String(subject)) => ctx.store.events_by_subject(subject, after, limit)?,
        Some(_) => return Err(bad_params("subject 必须是字符串")),
    };
    Ok((
        json!({
            "events": events.iter().map(|e| json!({
                "seq": e.seq, "kind": e.kind, "subject": e.subject, "payload": e.payload, "at": e.at
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
pub const READ_METHODS: [&str; 17] = [
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
];

#[cfg(test)]
mod tests {
    #[test]
    fn session_minutes_config_validate() {
        assert!(git_available());
        let mut sb = Sandbox::new("session-minutes-validate");
        let id = sb.add_project("p");
        let out = handle_command(sb.ctx(), &cmd("config.validate", json!({"project_id": id})));
        assert_eq!(ok_payload(&out)["ok"], true);
        std::fs::write(
            sb.path("p/.autome/config.toml"),
            "[loop]\nsession_minutes = 1\n",
        )
        .unwrap();
        let out = handle_command(sb.ctx(), &cmd("config.validate", json!({"project_id": id})));
        let payload = ok_payload(&out);
        assert_eq!(payload["ok"], false);
        assert_eq!(
            payload["violations"][0]["kind"],
            "session_minutes_out_of_range"
        );
    }

    #[test]
    fn session_minutes_set_loop_round_trip_and_rejection() {
        assert!(git_available());
        let mut sb = Sandbox::new("session-minutes-set");
        let id = sb.add_project("p");
        for project in [None, Some(id.as_str())] {
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "config.set_loop",
                    json!({"project_id": project, "session_minutes": 30}),
                ),
            );
            assert_eq!(
                ok_payload(&out)["resolved"]["loop_defaults"]["session_minutes"],
                30
            );
            let out = handle_command(sb.ctx(), &cmd("config.get", json!({"project_id": project})));
            assert_eq!(
                ok_payload(&out)["resolved"]["loop_defaults"]["session_minutes"],
                30
            );
            let path = if project.is_some() {
                sb.path("p/.autome/config.toml")
            } else {
                config_io::global_config_path(&sb.ctx.autome_home)
            };
            let before = std::fs::read(&path).unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "config.set_loop",
                    json!({"project_id": project, "session_minutes": 4}),
                ),
            );
            assert!(err_message(&out).contains("至少 5 分钟"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "config.set_loop",
                json!({"project_id": id, "session_minutes": null}),
            ),
        );
        assert_eq!(
            ok_payload(&out)["resolved"]["loop_defaults"]["session_minutes"],
            30
        );
        assert_eq!(
            config_io::load_project(&sb.path("p"))
                .unwrap()
                .loop_overrides
                .session_minutes,
            None
        );
    }

    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn move_fixture(sb: &mut Sandbox, project: &str, name: &str, state: TaskState) -> String {
        let id = cost_task(sb, project, name, state.clone());
        if matches!(state, TaskState::Queued | TaskState::Requeued { .. }) {
            sb.ctx.store.mark_queued(&id).unwrap();
        }
        id
    }

    fn move_call(sb: &mut Sandbox, id: &str, mut params: Value) -> Value {
        params["task_id"] = json!(id);
        let out = handle_command(sb.ctx(), &cmd("task.move", params));
        ok_payload(&out).clone()
    }

    #[test]
    fn queue_move_positions_and_event() {
        use autome_domain::task::Node;
        let mut sb = Sandbox::new("move-positions");
        let p = sb.add_project("repo");
        let a = move_fixture(&mut sb, &p, "a", TaskState::Queued);
        let b = move_fixture(&mut sb, &p, "b", TaskState::Queued);
        let c = move_fixture(
            &mut sb,
            &p,
            "c",
            TaskState::Requeued {
                resume: Node::Design,
            },
        );
        for (params, order, position) in [
            (json!({"top":true}), json!([c, a, b]), 1),
            (json!({"after":b}), json!([a, b, c]), 3),
            (json!({"before":b}), json!([a, c, b]), 2),
        ] {
            let out = move_call(&mut sb, &c, params.clone());
            assert_eq!(out["order"], order);
            assert_eq!(out["queue_position"], position);
            let events = sb.ctx.store.events_since(0, 100).unwrap();
            let (_, _, payload) = events.iter().rfind(|(_, k, _)| k == "task.moved").unwrap();
            assert_eq!(
                *payload,
                json!({"task":c,"project_id":p,"before":params.get("before"),"after":params.get("after"),"top":params.get("top").and_then(Value::as_bool).unwrap_or(false),"queue_position":position})
            );
            assert_eq!(
                sb.ctx.store.get_task(&c).unwrap().state,
                TaskState::Requeued {
                    resume: Node::Design
                }
            );
            assert!(sb.ctx.store.all_running_sessions().unwrap().is_empty());
        }
        assert!(!READ_METHODS.contains(&"task.move"));
    }

    #[test]
    fn queue_move_rejects_without_changes() {
        use autome_domain::task::{FailureReason, Node};
        let mut sb = Sandbox::new("move-reject");
        let p = sb.add_project("repo");
        let q = sb.add_project("other");
        let a = move_fixture(&mut sb, &p, "a", TaskState::Queued);
        let foreign = move_fixture(&mut sb, &q, "foreign", TaskState::Draft);
        let invalid_states = [
            TaskState::Active { node: Node::Design },
            TaskState::Paused {
                resume: Node::Design,
            },
            TaskState::Stopped { at: Node::Design },
            TaskState::Failed {
                at: Node::Design,
                reason: FailureReason::SessionCrashed {
                    detail: "test".into(),
                },
            },
            TaskState::Done,
            TaskState::Cancelled,
        ];
        let seq = sb.ctx.store.queued_seq(&a).unwrap();

        for params in [
            json!({}),
            json!({"top":false}),
            json!({"top":true,"before":a}),
            json!({"before":a,"after":foreign}),
            json!({"before":a}),
            json!({"after":foreign}),
            json!({"before":"T-99999"}),
            json!({"before":""}),
            json!({"before":"T-0001"}),
            json!({"top":"true"}),
            json!({"before":null}),
            json!({"after":1}),
        ] {
            let mut params = params;
            params["task_id"] = json!(a);
            let out = handle_command(sb.ctx(), &cmd("task.move", params));
            match out.reply.outcome {
                ReplyOutcome::Error { message, .. } => assert!(
                    !message.contains('\n')
                        && message
                            .chars()
                            .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
                ),
                _ => panic!("must reject"),
            }
        }
        for state in invalid_states {
            sb.ctx.store.set_task_state(&foreign, &state).unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd("task.move", json!({"task_id":foreign,"top":true})),
            );
            assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
            let local = move_fixture(&mut sb, &p, "invalid", state);
            let out = handle_command(
                sb.ctx(),
                &cmd("task.move", json!({"task_id":a,"after":local})),
            );
            assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
        }
        assert_eq!(sb.ctx.store.queued_seq(&a).unwrap(), seq);
        assert!(
            !sb.ctx
                .store
                .events_since(0, 100)
                .unwrap()
                .iter()
                .any(|(_, kind, _)| kind == "task.moved")
        );
    }

    #[test]
    fn queue_move_unpositioned_draft_reference() {
        for before in [true, false] {
            let mut sb = Sandbox::new("move-reference");
            let p = sb.add_project("repo");
            let blocker = move_fixture(&mut sb, &p, "block", TaskState::Draft);
            let a = move_fixture(&mut sb, &p, "a", TaskState::Draft);
            let b = move_fixture(&mut sb, &p, "b", TaskState::Draft);
            for id in [&a, &b] {
                sb.ctx
                    .store
                    .set_task_after(id, std::slice::from_ref(&blocker))
                    .unwrap();
            }
            let params = if before {
                json!({"before":a})
            } else {
                json!({"after":a})
            };
            let expected = if before { json!([b, a]) } else { json!([a, b]) };
            assert_eq!(move_call(&mut sb, &b, params)["order"], expected);
            for id in [&a, &b] {
                let out = handle_command(sb.ctx(), &cmd("task.start", json!({"task_id":id})));
                ok_payload(&out);
            }
            let ids: Vec<_> = sb
                .ctx
                .store
                .queued_tasks(&p)
                .unwrap()
                .into_iter()
                .map(|t| t.id)
                .collect();
            assert_eq!(json!(ids), expected);
        }
    }

    #[test]
    fn queue_move_other_project_unchanged() {
        let mut sb = Sandbox::new("move-isolation");
        let p = sb.add_project("repo");
        let q = sb.add_project("other");
        let a = move_fixture(&mut sb, &p, "a", TaskState::Queued);
        let foreign = move_fixture(&mut sb, &q, "foreign", TaskState::Queued);
        let b = move_fixture(&mut sb, &p, "b", TaskState::Queued);
        let seq = sb.ctx.store.queued_seq(&foreign).unwrap();
        assert_eq!(
            move_call(&mut sb, &b, json!({"top":true}))["order"],
            json!([b, a])
        );
        assert_eq!(sb.ctx.store.queued_seq(&foreign).unwrap(), seq);
    }

    #[test]
    fn queue_move_draft_keeps_position_on_start() {
        let mut sb = Sandbox::new("move-draft");
        let p = sb.add_project("repo");
        let blocker = move_fixture(&mut sb, &p, "block", TaskState::Draft);
        let a = move_fixture(&mut sb, &p, "a", TaskState::Queued);
        let b = move_fixture(&mut sb, &p, "b", TaskState::Draft);
        for id in [&a, &b] {
            sb.ctx
                .store
                .set_task_after(id, std::slice::from_ref(&blocker))
                .unwrap();
        }
        let out = move_call(&mut sb, &b, json!({"top":true}));
        assert!(out["queue_position"].is_null());
        let seq = sb.ctx.store.queued_seq(&b).unwrap();
        let out = handle_command(sb.ctx(), &cmd("task.start", json!({"task_id":b})));
        assert_eq!(ok_payload(&out)["queue_position"], 1);
        assert_eq!(sb.ctx.store.queued_seq(&b).unwrap(), seq);
    }

    #[test]
    fn queue_move_unmoved_draft_starts_at_tail_and_singleton() {
        let mut sb = Sandbox::new("move-tail");
        let p = sb.add_project("repo");
        let blocker = move_fixture(&mut sb, &p, "block", TaskState::Draft);
        let b = move_fixture(&mut sb, &p, "b", TaskState::Draft);
        let a = move_fixture(&mut sb, &p, "a", TaskState::Queued);
        for id in [&a, &b] {
            sb.ctx
                .store
                .set_task_after(id, std::slice::from_ref(&blocker))
                .unwrap();
        }
        assert_eq!(
            move_call(&mut sb, &a, json!({"top":true}))["order"],
            json!([a])
        );
        assert!(sb.ctx.store.queued_seq(&b).unwrap().is_none());
        let out = handle_command(sb.ctx(), &cmd("task.start", json!({"task_id":b})));
        assert_eq!(ok_payload(&out)["queue_position"], 2);
    }

    #[test]
    fn after_blocked_set_rejects_without_side_effects() {
        use autome_domain::{role::Role, session::SessionKind, task::Node};
        let mut sb = Sandbox::new("after-set-reject");
        let project = sb.add_project("repo");
        let other = sb.add_project("other");
        let a = cost_task(&mut sb, &project, "a", TaskState::Draft);
        let b = cost_task(&mut sb, &project, "b", TaskState::Draft);
        let foreign = cost_task(&mut sb, &other, "foreign", TaskState::Draft);
        sb.ctx
            .store
            .set_task_after(&b, std::slice::from_ref(&a))
            .unwrap();
        for (after, message) in [
            (json!([a]), "自己"),
            (json!([b]), "依赖环"),
            (json!([foreign]), "另一个项目"),
            (json!(["T-99999"]), "不存在"),
            (json!([""]), "不存在"),
            (json!(["T-0001"]), "不存在"),
            (json!(vec![b.clone(); 9]), "最多 8"),
            (json!(null), "任务编号列表"),
            (json!([1]), "任务编号列表"),
            (json!("T-1"), "任务编号列表"),
        ] {
            let out = handle_command(
                sb.ctx(),
                &cmd("task.set_after", json!({"task_id": a, "after": after})),
            );
            assert!(err_message(&out).contains(message), "{}", err_message(&out));
            assert!(!err_message(&out).contains('\n'));
            assert!(sb.ctx.store.task_after(&a).unwrap().is_empty());
            assert_eq!(
                sb.ctx.store.task_after(&b).unwrap(),
                std::slice::from_ref(&a)
            );
            assert_eq!(sb.ctx.store.count_events(&a, "task.after_set").unwrap(), 0);
        }
        let out = handle_command(sb.ctx(), &cmd("task.set_after", json!({"task_id": a})));
        assert!(err_message(&out).contains("任务编号列表"));
        for state in [
            TaskState::Active {
                node: Node::Implement,
            },
            TaskState::Done,
            TaskState::Cancelled,
        ] {
            sb.ctx.store.set_task_state(&b, &state).unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd("task.set_after", json!({"task_id": b, "after": []})),
            );
            assert!(!err_message(&out).is_empty());
            assert_eq!(
                sb.ctx.store.task_after(&b).unwrap(),
                std::slice::from_ref(&a)
            );
        }
        cost_session(&sb, &b, "seed", None, false);
        let mut session = sb.ctx.store.list_sessions(&b).unwrap().remove(0);
        session.id = "historical-impl".into();
        session.kind = SessionKind::Role { role: Role::Impl };
        sb.ctx.store.insert_session(&session).unwrap();
        for state in [
            TaskState::Stopped {
                at: Node::Implement,
            },
            TaskState::Active { node: Node::Design },
        ] {
            sb.ctx.store.set_task_state(&b, &state).unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd("task.set_after", json!({"task_id": b, "after": []})),
            );
            assert!(err_message(&out).contains("已经开始实现轮"));
            assert_eq!(
                sb.ctx.store.task_after(&b).unwrap(),
                std::slice::from_ref(&a)
            );
            assert_eq!(sb.ctx.store.count_events(&b, "task.after_set").unwrap(), 0);
        }
        assert!(!READ_METHODS.contains(&"task.set_after"));
    }

    #[test]
    fn after_blocked_set_accepts_design_states_and_clears() {
        use autome_domain::task::{FailureReason, Node};
        let mut sb = Sandbox::new("after-set-ok");
        let project = sb.add_project("repo");
        let a = cost_task(&mut sb, &project, "a", TaskState::Draft);
        let b = cost_task(&mut sb, &project, "b", TaskState::Draft);
        for state in [
            TaskState::Draft,
            TaskState::Queued,
            TaskState::Active { node: Node::Design },
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
            TaskState::Paused {
                resume: Node::Design,
            },
            TaskState::Stopped { at: Node::Design },
            TaskState::Failed {
                at: Node::Design,
                reason: FailureReason::Config {
                    detail: "test".into(),
                },
            },
            TaskState::Requeued {
                resume: Node::Design,
            },
        ] {
            sb.ctx.store.set_task_state(&b, &state).unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "task.set_after",
                    json!({"task_id": b, "after": vec![a.clone(); 8]}),
                ),
            );
            assert_eq!(ok_payload(&out)["task"]["after"], json!([a]));
            assert_eq!(sb.ctx.store.get_task(&b).unwrap().state, state);
            assert_eq!(
                sb.ctx
                    .store
                    .last_event(&b, "task.after_set")
                    .unwrap()
                    .unwrap(),
                json!({"after": [a]})
            );
        }
        sb.ctx.store.set_task_state(&b, &TaskState::Draft).unwrap();
        let mut ids = vec![a.clone()];
        for n in 0..7 {
            ids.push(cost_task(
                &mut sb,
                &project,
                &format!("dep-{n}"),
                TaskState::Draft,
            ));
        }
        let out = handle_command(
            sb.ctx(),
            &cmd("task.set_after", json!({"task_id": b, "after": ids})),
        );
        assert_eq!(ok_payload(&out)["task"]["after"], json!(ids));
        let out = handle_command(
            sb.ctx(),
            &cmd("task.set_after", json!({"task_id": b, "after": []})),
        );
        assert_eq!(ok_payload(&out)["task"]["after"], json!([]));
        assert_eq!(ok_payload(&out)["started"], false);
        assert_eq!(
            sb.ctx
                .store
                .last_event(&b, "task.after_set")
                .unwrap()
                .unwrap(),
            json!({"after": []})
        );
    }

    #[test]
    fn after_blocked_dashboard_preserves_project_queue_order() {
        use autome_domain::task::Node;
        let mut sb = Sandbox::new("after-dashboard");
        let p = sb.add_project("repo");
        let q = sb.add_project("other");
        for project in [&p, &q] {
            let a = cost_task(&mut sb, project, "a", TaskState::Cancelled);
            let b = cost_task(&mut sb, project, "b", TaskState::Queued);
            let c = cost_task(
                &mut sb,
                project,
                "c",
                TaskState::Requeued {
                    resume: Node::Design,
                },
            );
            sb.ctx.store.mark_queued(&c).unwrap();
            sb.ctx.store.mark_queued(&b).unwrap();
            sb.ctx
                .store
                .set_task_after(&b, std::slice::from_ref(&a))
                .unwrap();
            sb.ctx.store.set_task_after(&c, &[a]).unwrap();
        }
        let out = handle_command(sb.ctx(), &cmd("dashboard.get", json!({})));
        let payload = ok_payload(&out);
        assert_eq!(payload["running"], json!([]));
        assert_eq!(payload["waiting"], json!([]));
        let entries = payload["queued"].as_array().unwrap();
        assert_eq!(entries.len(), 4);
        let mut offset = 0;
        for project in sb.ctx.store.list_projects().unwrap() {
            for (index, task) in sb
                .ctx
                .store
                .queued_tasks(&project.id)
                .unwrap()
                .iter()
                .enumerate()
            {
                let entry = &entries[offset];
                offset += 1;
                assert_eq!(entry["task"]["id"], task.id);
                assert_eq!(entry["project_id"], project.id);
                assert_eq!(entry["queue_position"], index + 1);
                assert_eq!(entry["task"]["blocked_by"][0]["reason"], "cancelled");
                assert_eq!(
                    entry["task"]["blocked_by"][0]["state"]["state"],
                    "cancelled"
                );
                assert!(entry.get("session").is_some());
                assert!(entry.get("pending_decisions").is_some());
            }
        }
    }

    #[test]
    fn after_create_rejects_invalid_dependencies_without_writing() {
        let mut sb = Sandbox::new("after-invalid");
        let project = sb.add_project("repo");
        let other = sb.add_project("other");
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "task.create",
                json!({
                    "project_id": other, "request": "foreign", "draft": true
                }),
            ),
        );
        let foreign = ok_payload(&out)["task"]["id"].clone();
        let next = sb.ctx.store.next_task_id(&project).unwrap();
        for (after, message) in [
            (json!([next]), "任务不能依赖自己"),
            (json!(["T-9999"]), "不存在"),
            (json!([foreign]), "另一个项目"),
            (json!(vec!["T-9999"; 9]), "依赖最多 8 个"),
            (json!(null), "after 必须是任务编号列表"),
            (json!("T-1"), "after 必须是任务编号列表"),
            (json!([1]), "after 必须是任务编号列表"),
            (json!([""]), "不存在"),
            (json!(["T-0001"]), "不存在"),
        ] {
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "task.create",
                    json!({
                        "project_id": project, "request": "invalid", "after": after
                    }),
                ),
            );
            assert!(err_message(&out).contains(message), "{}", err_message(&out));
            assert!(!err_message(&out).contains('\n'));
            assert!(sb.ctx.store.list_tasks(&project).unwrap().is_empty());
            assert_eq!(sb.ctx.store.count_events(&next, "task.created").unwrap(), 0);
        }
    }

    #[test]
    fn after_create_reads_dependencies_and_input_boundaries() {
        let mut sb = Sandbox::new("after-read");
        let project = sb.add_project("repo");
        let mut ids = Vec::new();
        for i in 0..8 {
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "task.create",
                    json!({
                        "project_id": project, "request": format!("dependency {i}"), "draft": true
                    }),
                ),
            );
            assert_eq!(ok_payload(&out)["task"]["after"], json!([]));
            assert_eq!(ok_payload(&out)["task"]["blocked_by"], json!([]));
            ids.push(ok_payload(&out)["task"]["id"].as_str().unwrap().to_string());
        }
        sb.ctx
            .store
            .set_task_state(&ids[0], &TaskState::Done)
            .unwrap();
        sb.ctx
            .store
            .set_task_state(&ids[1], &TaskState::Cancelled)
            .unwrap();
        for after in [
            vec![],
            vec![ids[0].clone()],
            vec![ids[1].clone(); 8],
            ids.clone(),
        ] {
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "task.create",
                    json!({
                        "project_id": project, "request": "dependent", "draft": true, "after": after
                    }),
                ),
            );
            let task = &ok_payload(&out)["task"];
            let mut expected = after.clone();
            expected.dedup();
            assert_eq!(task["after"], json!(expected));
            let created = sb
                .ctx
                .store
                .last_event(task["id"].as_str().unwrap(), "task.created")
                .unwrap()
                .unwrap();
            if expected.is_empty() {
                assert_eq!(created, json!({"project_id": project}));
            } else {
                assert_eq!(created, json!({"project_id": project, "after": expected}));
            }
            let blocked: Vec<_> = expected.iter().filter(|id| *id != &ids[0]).collect();
            assert_eq!(task["blocked_by"].as_array().unwrap().len(), blocked.len());
            for (entry, id) in task["blocked_by"].as_array().unwrap().iter().zip(blocked) {
                assert_eq!(entry["id"], json!(id));
                assert_eq!(
                    entry["state"],
                    json!(sb.ctx.store.get_task(id).unwrap().state)
                );
                assert_eq!(
                    entry["reason"],
                    if id == &ids[1] {
                        json!("cancelled")
                    } else {
                        Value::Null
                    }
                );
            }
            let get = handle_command(sb.ctx(), &cmd("task.get", json!({"task_id": task["id"]})));
            assert_eq!(ok_payload(&get)["task"], *task);
            let get = handle_command(
                sb.ctx(),
                &cmd("project.get", json!({"project_id": project})),
            );
            let found = ok_payload(&get)["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["id"] == task["id"])
                .unwrap();
            let mut expected_project_task = task.clone();
            expected_project_task["queue_position"] = Value::Null;
            assert_eq!(found, &expected_project_task);
        }
    }

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

    fn gate_queue_fixture(sb: &mut Sandbox, project: &str, name: &str) -> String {
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "task.create",
                json!({"project_id":project, "request":name, "draft":true}),
            ),
        );
        let id = ok_payload(&out)["task"]["id"].as_str().unwrap().to_owned();
        sb.ctx
            .store
            .set_task_state(&id, &TaskState::Active { node: Node::Gate })
            .unwrap();
        id
    }

    fn gate_queue_get(sb: &mut Sandbox, id: &str) -> Value {
        let out = handle_command(sb.ctx(), &cmd("task.get", json!({"task_id":id})));
        ok_payload(&out).clone()
    }

    fn note_fixture() -> (Sandbox, String) {
        let mut sb = Sandbox::new("operator-note");
        let project = sb.add_project("repo");
        let id = gate_queue_fixture(&mut sb, &project, "note");
        (sb, id)
    }

    #[test]
    fn operator_note_is_recorded_in_order_without_changing_state() {
        let (mut sb, id) = note_fixture();
        let mut states = vec![
            TaskState::Draft,
            TaskState::Queued,
            TaskState::Requeued {
                resume: Node::Implement,
            },
            TaskState::Paused {
                resume: Node::Implement,
            },
            TaskState::Stopped { at: Node::Fix },
            TaskState::Failed {
                at: Node::Implement,
                reason: autome_domain::task::FailureReason::DesignBudget { limit: 1 },
            },
        ];
        for node in Node::ALL
            .into_iter()
            .chain(Node::BUG)
            .filter(|n| *n != Node::AwaitMerge)
        {
            states.push(TaskState::Active { node });
        }
        for (i, state) in states.into_iter().enumerate() {
            sb.ctx.store.set_task_state(&id, &state).unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd("task.note", json!({"task_id":id,"text":" 原样\n备注 "})),
            );
            assert_eq!(ok_payload(&out)["n"], i + 1);
            assert!(sb.ctx.store.list_sessions(&id).unwrap().is_empty());
            assert_eq!(sb.ctx.store.get_task(&id).unwrap().state, state);
        }
        let notes = sb.ctx.store.operator_notes(&id).unwrap();
        assert!(
            notes
                .iter()
                .all(|n| n["text"] == " 原样\n备注 " && n["delivered"].is_null())
        );
    }

    #[test]
    fn operator_note_after_merge_started_is_recorded_with_a_warning() {
        let (mut sb, id) = note_fixture();
        for node in [
            Node::Rebase,
            Node::MergeGate,
            Node::Merging,
            Node::MainCheck,
            Node::Cleanup,
        ] {
            sb.ctx
                .store
                .set_task_state(&id, &TaskState::Active { node })
                .unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd("task.note", json!({"task_id":id,"text":"备注"})),
            );
            let late = matches!(node, Node::Merging | Node::MainCheck | Node::Cleanup);
            assert_eq!(ok_payload(&out)["warning"].is_string(), late);
            let event = sb
                .ctx
                .store
                .last_event(&id, "task.note_added")
                .unwrap()
                .unwrap();
            assert_eq!(event["late"] == true, late);
            assert_eq!(out.events.len(), 1);
            sb.ctx
                .store
                .append_event(
                    "task.note_delivered",
                    &id,
                    json!({"notes":[event["n"]],"session_id":"s","role":"impl","round":1}),
                )
                .unwrap();
            let notes = sb.ctx.store.operator_notes(&id).unwrap();
            assert_eq!(notes.last().unwrap()["delivered"].is_null(), late);
        }
    }

    #[test]
    fn operator_note_rejects_done_and_cancelled_plainly() {
        let (mut sb, id) = note_fixture();
        for (state, message) in [
            (TaskState::Done, "已经完成"),
            (TaskState::Cancelled, "已经取消"),
        ] {
            sb.ctx.store.set_task_state(&id, &state).unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd("task.note", json!({"task_id":id,"text":"备注"})),
            );
            assert!(err_message(&out).contains(message));
            assert_eq!(sb.ctx.store.get_task(&id).unwrap().state, state);
        }
        assert!(sb.ctx.store.operator_notes(&id).unwrap().is_empty());
    }

    #[test]
    fn operator_note_rejects_blank_and_over_8000_chars() {
        let (mut sb, id) = note_fixture();
        for text in [
            "".into(),
            " \n\t".into(),
            "中".repeat(8001),
            "😀".repeat(4001),
        ] {
            let out = handle_command(
                sb.ctx(),
                &cmd("task.note", json!({"task_id":id,"text":text})),
            );
            assert!(err_message(&out).contains("备注"));
        }
        for params in [
            json!({"task_id":id,"text":7}),
            json!({"task_id":id,"text":"x","reopen":"yes"}),
            json!({"text":"x"}),
        ] {
            let out = handle_command(sb.ctx(), &cmd("task.note", params));
            assert!(!err_message(&out).is_empty());
        }
        assert!(sb.ctx.store.operator_notes(&id).unwrap().is_empty());
        for text in ["中".repeat(8000), "😀".repeat(4000), "x".into()] {
            let out = handle_command(
                sb.ctx(),
                &cmd("task.note", json!({"task_id":id,"text":text})),
            );
            ok_payload(&out);
        }
    }

    #[test]
    fn operator_note_task_get_lists_notes_with_delivery() {
        let (mut sb, id) = note_fixture();
        for text in ["first", "second"] {
            let out = handle_command(
                sb.ctx(),
                &cmd("task.note", json!({"task_id":id,"text":text})),
            );
            ok_payload(&out);
        }
        sb.ctx
            .store
            .append_event(
                "task.note_delivered",
                &id,
                json!({"notes":[1],"session_id":"s1","role":"impl","round":3}),
            )
            .unwrap();
        let got = gate_queue_get(&mut sb, &id);
        assert_eq!(got["notes"].as_array().unwrap().len(), 2);
        assert_eq!(got["notes"][0]["text"], "first");
        assert_eq!(got["notes"][0]["delivered"]["session_id"], "s1");
        assert_eq!(got["notes"][0]["delivered"]["role"], "impl");
        assert_eq!(got["notes"][0]["delivered"]["round"], 3);
        assert!(got["notes"][0]["at"].is_string());
        assert!(got["notes"][0]["delivered"]["at"].is_string());
        assert!(got["notes"][1]["delivered"].is_null());
    }

    #[test]
    fn task_get_scope_restore_reports_last_and_pending() {
        assert!(git_available());
        let mut sb = Sandbox::new("scope-get");
        let project = sb.add_project("repo");
        let id = gate_queue_fixture(&mut sb, &project, "scope");
        assert_eq!(
            gate_queue_get(&mut sb, &id)["scope_restore"],
            json!({"last":null,"pending":[]})
        );
        let plan = json!({"task":id,"session_id":"S-1","role":"audit","label":"审计","round":3,"repos":[{"repo":"repo","worktree":"/tmp/repo","base":"base","pre_head":"head","commit":null,"paths":["a.rs"],"patch":"/tmp/a.patch","stat":"1 file"}]});
        sb.ctx
            .store
            .append_event("session.scope_restore_begun", &id, plan.clone())
            .unwrap();
        let first = gate_queue_get(&mut sb, &id)["scope_restore"].clone();
        assert_eq!(first["pending"], json!(["/tmp/a.patch"]));
        assert!(
            first["last"]["at"]
                .as_str()
                .is_some_and(|at| !at.is_empty())
        );
        let mut without_at = first["last"].clone();
        without_at.as_object_mut().unwrap().remove("at");
        assert_eq!(without_at, plan);
        sb.ctx
            .store
            .append_event(
                "session.scope_restore_failed",
                &id,
                json!({"session_id":"S-1","detail":"failed"}),
            )
            .unwrap();
        assert_eq!(
            gate_queue_get(&mut sb, &id)["scope_restore"]["pending"],
            json!([])
        );
        let mut second = plan.clone();
        second["session_id"] = json!("S-2");
        second["repos"][0]["patch"] = json!("/tmp/b.patch");
        sb.ctx
            .store
            .append_event("session.scope_restore_begun", &id, second.clone())
            .unwrap();
        sb.ctx
            .store
            .append_event("session.scope_restored", &id, second)
            .unwrap();
        let before = gate_queue_get(&mut sb, &id)["scope_restore"].clone();
        assert_eq!(before["last"]["session_id"], "S-2");
        assert_eq!(before["pending"], json!(["/tmp/b.patch"]));
        sb.ctx
            .store
            .append_event(
                "session.scope_patches_handled",
                &id,
                json!({"session_id":"S-3","patches":["/tmp/b.patch"]}),
            )
            .unwrap();
        let after = gate_queue_get(&mut sb, &id)["scope_restore"].clone();
        assert_eq!(after["pending"], json!([]));
        assert_eq!(after["last"], before["last"]);
    }

    #[test]
    fn gate_lane_task_get_reports_lane_and_same_lane_position() {
        let mut sb = Sandbox::new("lane-get");
        let project = sb.add_project("repo");
        let a = gate_queue_fixture(&mut sb, &project, "a");
        let b = gate_queue_fixture(&mut sb, &project, "b");
        let c = gate_queue_fixture(&mut sb, &project, "c");
        for (id, lane) in [(&a, "heavy"), (&b, "light"), (&c, "light")] {
            sb.ctx
                .store
                .append_event("gate.queued", id, json!({"lane":lane,"position":99}))
                .unwrap();
        }
        for (id, lane, position) in [(&a, "heavy", 1), (&b, "light", 1), (&c, "light", 2)] {
            let got = gate_queue_get(&mut sb, id);
            assert_eq!(got["gate_queue_lane"], lane);
            assert_eq!(got["gate_queue_position"], position);
        }
        sb.ctx
            .store
            .append_event("gate.started", &b, json!({}))
            .unwrap();
        assert!(gate_queue_get(&mut sb, &b)["gate_queue_lane"].is_null());
        assert_eq!(gate_queue_get(&mut sb, &c)["gate_queue_position"], 1);
    }

    #[test]
    fn gate_queue_task_get_reports_live_rank_without_reusing_slot_position() {
        assert!(git_available());
        let mut sb = Sandbox::new("gate-queue-get");
        let project = sb.add_project("repo");
        let a = gate_queue_fixture(&mut sb, &project, "first");
        let b = gate_queue_fixture(&mut sb, &project, "second");
        // Reverse task creation order; the reader must use event order, not task order.
        for id in [&b, &a] {
            sb.ctx
                .store
                .append_event("gate.queued", id, json!({"node":"gate","position":99}))
                .unwrap();
        }
        assert_eq!(gate_queue_get(&mut sb, &b)["gate_queue_position"], 1);
        let got = gate_queue_get(&mut sb, &a);
        assert_eq!(got["gate_queue_position"], 2);
        assert_eq!(got.get("queue_position"), Some(&Value::Null));
        assert_eq!(
            got["task"]["state"],
            json!({"state":"active","node":"gate"})
        );
        sb.ctx
            .store
            .append_event("gate.started", &b, json!({}))
            .unwrap();
        assert_eq!(gate_queue_get(&mut sb, &a)["gate_queue_position"], 1);
        assert_eq!(
            gate_queue_get(&mut sb, &b).get("gate_queue_position"),
            Some(&Value::Null)
        );
    }

    #[test]
    fn gate_queue_task_get_null_and_all_queue_invalidation_events() {
        assert!(git_available());
        let mut sb = Sandbox::new("gate-queue-null");
        let project = sb.add_project("repo");
        let id = gate_queue_fixture(&mut sb, &project, "task");
        assert_eq!(
            gate_queue_get(&mut sb, &id).get("gate_queue_position"),
            Some(&Value::Null)
        );
        for kind in [
            "gate.started",
            "setup.started",
            "session.started",
            "session.deferred",
            "task.updated",
            "gate.queue_dropped",
        ] {
            sb.ctx
                .store
                .append_event("gate.queued", &id, json!({"position":1}))
                .unwrap();
            assert_eq!(gate_queue_get(&mut sb, &id)["gate_queue_position"], 1);
            sb.ctx
                .store
                .append_event("gate.result", &id, json!({"ok":false,"exit_code":1}))
                .unwrap();
            assert_eq!(gate_queue_get(&mut sb, &id)["gate_queue_position"], 1);
            sb.ctx.store.append_event(kind, &id, json!({})).unwrap();
            assert_eq!(
                gate_queue_get(&mut sb, &id).get("gate_queue_position"),
                Some(&Value::Null),
                "{kind}"
            );
        }
    }

    #[test]
    fn gate_queue_dashboard_reports_cross_project_rank_and_null_on_waiting() {
        assert!(git_available());
        let mut sb = Sandbox::new("gate-queue-dashboard");
        let p = sb.add_project("one");
        let q = sb.add_project("two");
        let a = gate_queue_fixture(&mut sb, &p, "a");
        let b = gate_queue_fixture(&mut sb, &q, "b");
        let c = gate_queue_fixture(&mut sb, &p, "c");
        sb.ctx
            .store
            .set_task_state(
                &c,
                &TaskState::Active {
                    node: Node::AwaitMerge,
                },
            )
            .unwrap();
        for (i, id) in [&a, &b].into_iter().enumerate() {
            sb.ctx
                .store
                .append_event("gate.queued", id, json!({"position":i+1}))
                .unwrap();
        }
        for started in [false, true] {
            if started {
                sb.ctx
                    .store
                    .append_event("setup.started", &a, json!({}))
                    .unwrap();
            }
            let out = handle_command(sb.ctx(), &cmd("dashboard.get", json!({})));
            let d = ok_payload(&out);
            for (id, expected) in [
                (&a, if started { Value::Null } else { json!(1) }),
                (&b, json!(if started { 1 } else { 2 })),
            ] {
                let row = d["running"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["task"]["id"] == *id)
                    .unwrap();
                assert_eq!(row.get("gate_queue_position"), Some(&expected));
            }
            assert_eq!(d["waiting"][0]["task"]["id"], c);
            assert_eq!(
                d["waiting"][0].get("gate_queue_position"),
                Some(&Value::Null)
            );
        }
    }

    #[test]
    fn bug_loop_reply_on_a_feature_task_is_exactly_reject() {
        needs_git!();
        for kind in ["feature", "bug"] {
            for state in [
                TaskState::Draft,
                TaskState::Active {
                    node: Node::AwaitDesignApproval,
                },
            ] {
                if kind == "bug" && state == TaskState::Draft {
                    continue;
                }
                let mut results = Vec::new();
                for method in ["task.reply", "task.reject"] {
                    let mut sb = Sandbox::new("reply-parity");
                    let project = sb.add_project("repo");
                    std::fs::write(sb.path("repo/README.md"), "parity").unwrap();
                    git::commit_paths(&sb.path("repo"), &["."], "initial").unwrap();
                    let out = handle_command(
                        sb.ctx(),
                        &cmd(
                            "task.create",
                            json!({"project_id":project,"request":"parity","kind":kind,"draft":true}),
                        ),
                    );
                    let id = ok_payload(&out)["task"]["id"].as_str().unwrap().to_owned();
                    sb.ctx.store.set_task_state(&id, &state).unwrap();
                    let seq = sb.ctx.store.latest_seq().unwrap();
                    let out = handle_command(
                        sb.ctx(),
                        &cmd(
                            method,
                            json!({"task_id":id,"info":"原文  中文","feedback":"原文  中文"}),
                        ),
                    );
                    let result = match out.reply.outcome {
                        ReplyOutcome::Ok { .. } => "ok".to_owned(),
                        ReplyOutcome::Error { message, .. } => message,
                    };
                    let events = sb
                        .ctx
                        .store
                        .events_since(seq, 100)
                        .unwrap()
                        .into_iter()
                        .map(|(_, kind, _)| kind)
                        .collect::<Vec<_>>();
                    assert!(!events.iter().any(|e| e == "task.replied"));
                    results.push((result, sb.ctx.store.get_task(&id).unwrap().state, events));
                }
                assert_eq!(results[0], results[1], "{kind} {state:?}");
            }
        }
    }

    #[test]
    fn bug_loop_reply_is_a_write_method_not_a_read_method() {
        assert!(!READ_METHODS.contains(&"task.reply"));
        let mut sb = Sandbox::new("reply-invalid");
        let out = handle_command(
            sb.ctx(),
            &cmd("task.reply", json!({"task_id":"missing","info":7})),
        );
        assert!(err_message(&out).contains("info"));
    }

    #[test]
    fn bug_loop_a_bug_task_is_refused_a_retro() {
        needs_git!();
        let mut sb = Sandbox::new("bug-retro");
        let project = sb.add_project("repo");
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "task.create",
                json!({"project_id":project,"request":"bug","kind":"bug","draft":true}),
            ),
        );
        let id = ok_payload(&out)["task"]["id"].as_str().unwrap().to_owned();
        let out = handle_command(sb.ctx(), &cmd("task.retro", json!({"task_id":id})));
        assert_eq!(err_message(&out), "Bug 任务没有复盘");
        assert_eq!(
            sb.ctx
                .store
                .count_events(&id, "task.retro_started")
                .unwrap(),
            0
        );
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
    fn a_workspace_task_starts_in_its_document_repository() {
        // Before the intake round has named anything, the only repository a
        // workspace task works in is the one holding its documents — and the
        // documents go under the workspace's own document root, not the
        // member's top level, which already belongs to whoever owns that
        // repository.
        needs_git!();
        let mut sb = Sandbox::new("ws-first-task");
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
                json!({ "project_id": id, "request": "加一个后台看板" }),
            ),
        );
        let payload = ok_payload(&out);
        let slug = payload["task"]["slug"].as_str().unwrap().to_string();
        assert_eq!(
            payload["task"]["doc_dir"],
            json!(format!("autome/{slug}")),
            "the workspace default keeps Autome out of the document repository's top level"
        );

        // Creating the task starts its intake round, which checks out only
        // what it needs: the document repository. `backend` is not checked out
        // because nothing has said the task touches it yet — that is the
        // intake round's job, and it says so in the document.
        let dir = target.join(".worktree").join(&slug);
        assert!(
            dir.join("docs").join(".git").exists(),
            "the document repository is checked out"
        );
        assert!(
            !dir.join("backend").exists(),
            "a repository nobody has named is not checked out"
        );
        // And the checkout sits *beside* the others rather than containing
        // them, so an agent's relative paths mean what they mean in the real
        // workspace.
        assert!(
            dir.join("docs").join("autome").join(&slug).exists(),
            "the task's documents are under the workspace's document root"
        );
        // Every member repository still has exactly one branch of its own plus
        // the task's, and `backend` has not been touched at all.
        assert!(
            !git::branch_exists(&target.join("backend"), &format!("autome/{slug}")),
            "no branch is cut in a repository the task has not named"
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
    fn a_removed_directory_can_be_added_back() {
        // P-08: removing a project touches nothing on disk, so the user is
        // entitled to change their mind. `projects.path` is UNIQUE, so the
        // removed row still sits on the path — and the insert used to come
        // back as "该目录已经是一个项目" about a project that is not there.
        needs_git!();
        let mut sb = Sandbox::new("re-add");
        let target = sb.path("p");
        std::fs::create_dir_all(&target).unwrap();
        let params = json!({ "path": target.to_str().unwrap() });

        let first = handle_command(sb.ctx(), &cmd("project.add", params.clone()));
        let id = ok_payload(&first)["project"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        handle_command(
            sb.ctx(),
            &cmd("project.remove", json!({ "project_id": id })),
        );

        let again = handle_command(sb.ctx(), &cmd("project.add", params));
        let payload = ok_payload(&again);
        assert_eq!(
            payload["project"]["id"],
            json!(id),
            "the same row comes back, so the removed project's tasks still point at something"
        );
        assert_eq!(payload["project"]["removed_at"], json!(null));
        assert_eq!(
            sb.ctx().store.list_projects().unwrap().len(),
            1,
            "one project, not two rows fighting over one path"
        );
    }

    #[test]
    fn a_workspace_can_change_shape_when_it_is_added_back() {
        // The repair path for a directory that was added as a repository by
        // mistake: remove it, clean up, add it back as what it actually is.
        needs_git!();
        let mut sb = Sandbox::new("re-add-ws");
        let target = sb.path("ws");
        workspace_at(&target, &[("docs", "main"), ("backend", "main")]);
        let first = handle_command(
            sb.ctx(),
            &cmd("project.add", json!({ "path": target.to_str().unwrap() })),
        );
        let id = ok_payload(&first)["project"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(ok_payload(&first)["project"]["kind"], json!("repo"));
        handle_command(
            sb.ctx(),
            &cmd("project.remove", json!({ "project_id": id })),
        );

        let again = handle_command(
            sb.ctx(),
            &cmd(
                "project.add",
                json!({ "path": target.to_str().unwrap(), "workspace": true, "docs_repo": "docs" }),
            ),
        );
        let payload = ok_payload(&again);
        assert_eq!(payload["project"]["kind"], json!("workspace"));
        assert_eq!(payload["project"]["docs_repo"], json!("docs"));
    }

    #[test]
    fn the_scaffold_never_writes_through_a_symlinked_agents_file() {
        // A workspace root's AGENTS.md is routinely a link into one of the
        // member repositories. Following it means editing a tracked file in a
        // repository Autome does not own the commits for, and the user finds
        // out from `git status` somewhere they did not ask it to touch.
        needs_git!();
        let mut sb = Sandbox::new("symlink-agents");
        let target = sb.path("ws");
        workspace_at(&target, &[("docs", "main"), ("backend", "main")]);
        let real = target.join("docs/WORKSPACE_AGENTS.md");
        std::fs::write(&real, "用户自己的规范\n").unwrap();
        std::os::unix::fs::symlink("docs/WORKSPACE_AGENTS.md", target.join("AGENTS.md")).unwrap();

        handle_command(
            sb.ctx(),
            &cmd(
                "project.add",
                json!({ "path": target.to_str().unwrap(), "workspace": true, "docs_repo": "docs" }),
            ),
        );

        assert_eq!(
            std::fs::read_to_string(&real).unwrap(),
            "用户自己的规范\n",
            "the linked-to file is untouched"
        );
        assert!(
            std::fs::symlink_metadata(target.join("AGENTS.md"))
                .unwrap()
                .file_type()
                .is_symlink(),
            "and the link itself was not replaced by a regular file"
        );
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
    fn project_add_init_commit_preserves_untracked_user_docs() {
        assert!(git_available(), "this regression requires Git");
        let mut sb = Sandbox::new("init-user-docs");
        let repo = sb.path("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git::init(&repo, "main").unwrap();
        std::fs::write(repo.join("README.md"), "existing project\n").unwrap();
        git::commit_paths(&repo, &["README.md"], "initial").unwrap();
        let user_files = ["docs/draft.md", "docs/research/notes.md", "personal.txt"];
        std::fs::create_dir_all(repo.join("docs/research")).unwrap();
        for path in user_files {
            std::fs::write(repo.join(path), "private draft\n").unwrap();
        }
        let git_output = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&repo)
                .args(args)
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            String::from_utf8(output.stdout).unwrap()
        };
        assert!(git_output(&["diff", "--cached", "--name-only"]).is_empty());
        let before = git_output(&["ls-files", "--others", "--exclude-standard"]);
        for path in user_files {
            assert!(before.lines().any(|line| line == path));
        }

        // Exercise the same dispatch sequence as project add --skip-onboarding.
        let id = sb.add_project("repo");
        assert!(git_output(&["diff", "--cached", "--name-only"]).is_empty());
        let outcome = handle_command(
            sb.ctx(),
            &cmd("project.onboarding.skip", json!({ "project_id": id })),
        );
        assert!(ok_payload(&outcome)["init_commit"].is_string());
        assert_eq!(
            git_output(&["log", "-1", "--format=%s"]).trim(),
            "chore(autome): initialise project"
        );
        let committed = git_output(&["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]);
        for path in ["AGENTS.md", ".gitignore"] {
            assert!(committed.lines().any(|line| line == path), "{committed}");
        }
        assert!(committed.lines().any(|line| line.starts_with(".autome/")));
        let untracked = git_output(&["ls-files", "--others", "--exclude-standard"]);
        for path in user_files {
            assert_eq!(
                std::fs::read_to_string(repo.join(path)).unwrap(),
                "private draft\n"
            );
            assert!(
                !committed.lines().any(|line| line == path)
                    && untracked.lines().any(|line| line == path),
                "user file {path} must remain untracked; init commit files:\n{committed}\nuntracked:\n{untracked}"
            );
        }
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

    fn onboarding_workspace(sb: &mut Sandbox) -> (String, std::path::PathBuf) {
        let root = sb.path("ws");
        workspace_at(&root, &[("docs", "main"), ("app", "main")]);
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "project.add",
                json!({
                    "path": root, "workspace": true, "docs_repo": "docs"
                }),
            ),
        );
        let id = ok_payload(&out)["project"]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        std::fs::write(root.join("docs/agent-project-profile.md"), "profile\n").unwrap();
        (id, root)
    }

    #[test]
    fn workspace_onboarding_commit_skip_succeeds_without_a_root_repo() {
        needs_git!();
        let mut sb = Sandbox::new("ws-skip");
        let (id, root) = onboarding_workspace(&mut sb);
        let before = git::head_sha(&root.join("docs")).unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd("project.onboarding.skip", json!({"project_id":id})),
        );
        assert!(ok_payload(&out)["init_commit"].is_string());
        assert_ne!(before, git::head_sha(&root.join("docs")).unwrap());
        assert!(!root.join(".git").exists());
    }

    #[test]
    fn workspace_onboarding_commit_finish_succeeds_and_records_the_event() {
        needs_git!();
        let mut sb = Sandbox::new("ws-finish");
        let (id, root) = onboarding_workspace(&mut sb);
        for _ in 0..3 {
            let out = handle_command(
                sb.ctx(),
                &cmd("project.onboarding.advance", json!({"project_id":id})),
            );
            ok_payload(&out);
        }
        assert!(
            sb.ctx
                .store
                .get_project(&id)
                .unwrap()
                .onboarding
                .is_settled()
        );
        assert!(!root.join(".git").exists());
        assert!(
            sb.ctx
                .store
                .events_since(0, 100)
                .unwrap()
                .iter()
                .any(|e| e.1 == "project.onboarding" && e.2["init_commit"].is_string())
        );
    }

    fn onboarding_locked_commit(workspace: bool) {
        let mut sb = Sandbox::new("onboarding-lock");
        let (id, root) = if workspace {
            onboarding_workspace(&mut sb)
        } else {
            let id = sb.add_project("p");
            (id, sb.path("p"))
        };
        let repo = if workspace { root.join("docs") } else { root };
        let before = sb.ctx.store.get_project(&id).unwrap().onboarding;
        std::fs::write(repo.join(".git/index.lock"), "locked").unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd("project.onboarding.skip", json!({"project_id":id})),
        );
        assert_eq!(sb.ctx.store.get_project(&id).unwrap().onboarding, before);
        assert!(format!("{:?}", out.reply).contains("状态没有改"));
        std::fs::remove_file(repo.join(".git/index.lock")).unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd("project.onboarding.skip", json!({"project_id":id})),
        );
        assert!(ok_payload(&out)["init_commit"].is_string());
    }

    #[test]
    fn workspace_onboarding_commit_failure_keeps_state_and_can_retry() {
        needs_git!();
        onboarding_locked_commit(true);
    }

    #[test]
    fn workspace_onboarding_commit_single_repo_index_lock_keeps_state() {
        needs_git!();
        onboarding_locked_commit(false);
    }

    fn cost_task(sb: &mut Sandbox, project: &str, name: &str, state: TaskState) -> String {
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "task.create",
                json!({"project_id":project,"request":name,"draft":true}),
            ),
        );
        let id = ok_payload(&out)["task"]["id"].as_str().unwrap().to_owned();
        assert_eq!(ok_payload(&out)["task"]["cost_usd"], 0.0);
        sb.ctx.store.set_task_state(&id, &state).unwrap();
        id
    }

    fn visible_session(
        sb: &Sandbox,
        task: &str,
        limit: Option<u32>,
    ) -> autome_domain::session::Session {
        cost_session(sb, task, "seed", None, false);
        let mut session = sb.ctx.store.list_sessions(task).unwrap().remove(0);
        session.id = format!("{task}-visible");
        session.limit_minutes = limit;
        session.lifecycle = autome_domain::session::SessionLifecycle::Running;
        session.ended_at = None;
        session.started_at = rusqlite::Connection::open_in_memory()
            .unwrap()
            .query_row(
                "SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now','-37 minutes')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        sb.ctx.store.insert_session(&session).unwrap();
        session
    }

    #[test]
    fn timeout_visible_finished_entries_and_missing_log() {
        use autome_domain::session::SessionLifecycle;
        let mut sb = Sandbox::new("visible-finished");
        let project = sb.add_project("p");
        let task = cost_task(
            &mut sb,
            &project,
            "visible",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        let session = visible_session(&sb, &task, Some(5));
        sb.ctx
            .store
            .finish_session(
                &session.id,
                &SessionLifecycle::TimedOut { limit_minutes: 5 },
                &now_iso(),
            )
            .unwrap();
        let got = task_get(sb.ctx(), &task)
            .unwrap_or_else(|e| panic!("{}", e.message))
            .0;
        for entry in got["sessions"].as_array().unwrap() {
            let timeout = entry["id"] == session.id;
            assert_eq!(entry["timed_out"], timeout);
            let log = session_log(sb.ctx(), entry["id"].as_str().unwrap())
                .unwrap_or_else(|e| panic!("{}", e.message))
                .0;
            assert_eq!(log["session"]["timed_out"], timeout);
            assert_eq!(log["log"], "");
            if timeout {
                assert_eq!(entry["limit_minutes"], 5);
                assert_eq!(log["session"]["limit_minutes"], 5);
                assert_eq!(entry["running"], false);
                assert!(entry["elapsed_minutes"].is_null());
            }
        }
    }

    #[test]
    fn timeout_visible_running_and_legacy_limits_across_endpoints() {
        for limit in [Some(120), None] {
            let mut sb = Sandbox::new("visible-running");
            let project = sb.add_project("p");
            let task = cost_task(
                &mut sb,
                &project,
                "visible",
                TaskState::Active {
                    node: Node::Implement,
                },
            );
            let session = visible_session(&sb, &task, limit);
            std::fs::write(
                sb.path("p/.autome/config.toml"),
                "[loop]\nsession_minutes = 30\n",
            )
            .unwrap();
            let expected = limit.unwrap_or(30);
            let got = task_get(sb.ctx(), &task)
                .unwrap_or_else(|e| panic!("{}", e.message))
                .0;
            let log = session_log(sb.ctx(), &session.id)
                .unwrap_or_else(|e| panic!("{}", e.message))
                .0;
            let dashboard = dashboard_get(sb.ctx())
                .unwrap_or_else(|e| panic!("{}", e.message))
                .0;
            let entry = got["sessions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["id"] == session.id)
                .unwrap();
            for value in [entry, &log["session"], &dashboard["running"][0]["session"]] {
                assert_eq!(value["elapsed_minutes"], 37);
                assert_eq!(value["limit_minutes"], expected);
                assert_eq!(value["timed_out"], false);
                assert_eq!(value["running"], true);
            }
            let project = sb.ctx.store.get_project(&project).unwrap();
            std::fs::write(sb.path("p/.autome/config.toml"), "invalid [").unwrap();
            assert_eq!(
                session_json(&sb.ctx, &project, &session)["limit_minutes"],
                120
            );
            let mut boundary = session.clone();
            boundary.started_at = String::new();
            assert!(session_json(&sb.ctx, &project, &boundary)["elapsed_minutes"].is_null());
            boundary.started_at = "9999-01-01T00:00:00Z".into();
            boundary.limit_minutes = Some(u32::MAX);
            let value = session_json(&sb.ctx, &project, &boundary);
            assert_eq!(value["elapsed_minutes"], 0);
            assert_eq!(value["limit_minutes"], u32::MAX);
        }
    }

    #[test]
    fn timeout_visible_dashboard_failure_reason() {
        let mut sb = Sandbox::new("visible-failed");
        let project = sb.add_project("p");
        let detail = "会话运行超过 5 分钟，已被 Autome 终止（超时）";
        let task = cost_task(
            &mut sb,
            &project,
            "visible",
            TaskState::Failed {
                at: Node::Implement,
                reason: autome_domain::task::FailureReason::SessionCrashed {
                    detail: detail.into(),
                },
            },
        );
        let got = dashboard_get(sb.ctx())
            .unwrap_or_else(|e| panic!("{}", e.message))
            .0;
        assert_eq!(got["waiting"][0]["task"]["id"], task);
        assert_eq!(
            got["waiting"][0]["task"]["state"]["reason"]["detail"],
            detail
        );
    }

    fn cost_session(sb: &Sandbox, task: &str, suffix: &str, cost: Option<f64>, running: bool) {
        use autome_domain::session::{Session, SessionKind, SessionLifecycle};
        let session = Session {
            id: format!("{task}-{suffix}"),
            task_id: task.into(),
            kind: SessionKind::Intake,
            runtime: autome_domain::role::Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: "2026-11-12T00:00:00Z".into(),
            ended_at: if running {
                None
            } else {
                Some("2026-11-12T00:01:00Z".into())
            },
            lifecycle: if running {
                SessionLifecycle::Running
            } else {
                SessionLifecycle::Exited { exit_code: 0 }
            },
            log_path: "l".into(),
            pid: None,
            limit_minutes: None,
            protocol_ref: None,
            rules_hash: None,
            metrics: autome_domain::metrics::SessionMetrics {
                cost_usd: cost,
                ..Default::default()
            },
        };
        sb.ctx.store.insert_session(&session).unwrap();
        sb.ctx
            .store
            .set_session_metrics(&session.id, &session.metrics)
            .unwrap();
    }

    #[test]
    fn cost_rollup_project_get_splits_unfinished_and_finished() {
        let mut sb = Sandbox::new("cost-states");
        let project = sb.add_project("p");
        let states = [
            TaskState::Done,
            TaskState::Cancelled,
            TaskState::Failed {
                at: Node::Design,
                reason: autome_domain::task::FailureReason::DesignBudget { limit: 1 },
            },
            TaskState::Stopped { at: Node::Design },
            TaskState::Draft,
            TaskState::Active { node: Node::Design },
        ];
        let mut ids = Vec::new();
        for (i, state) in states.into_iter().enumerate() {
            let id = cost_task(&mut sb, &project, &format!("task {i}"), state);
            cost_session(&sb, &id, "paid", Some((i + 1) as f64), false);
            cost_session(&sb, &id, "missing", None, false);
            cost_session(&sb, &id, "running", Some(100.0), true);
            ids.push(id);
        }
        let out = handle_command(sb.ctx(), &cmd("project.get", json!({"project_id":project})));
        let p = ok_payload(&out);
        assert_eq!(
            p["cost_usd_total"],
            json!({"finished":3.0,"unfinished":18.0})
        );
        assert_eq!(p["cost_sessions_without_cost"], 6);
        for (i, id) in ids.iter().enumerate() {
            let task = p["tasks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["id"] == *id)
                .unwrap();
            assert_eq!(task["cost_usd"], (i + 1) as f64);
            assert_eq!(task["cost_sessions_without_cost"], 1);
        }
        // A terminal classification changes the bucket, never the amount.
        sb.ctx
            .store
            .set_task_state(&ids[5], &TaskState::Cancelled)
            .unwrap();
        let out = handle_command(sb.ctx(), &cmd("project.get", json!({"project_id":project})));
        assert_eq!(
            ok_payload(&out)["cost_usd_total"],
            json!({"finished":9.0,"unfinished":12.0})
        );
    }

    #[test]
    fn cost_rollup_project_list_keeps_projects_apart() {
        let mut sb = Sandbox::new("cost-projects");
        let mut expected = Vec::new();
        for (name, amount) in [("p", 1.25), ("q", 2.5), ("empty", 0.0)] {
            let project = sb.add_project(name);
            if amount > 0.0 {
                let id = cost_task(&mut sb, &project, name, TaskState::Draft);
                cost_session(&sb, &id, "paid", Some(amount), false);
                cost_session(&sb, &id, "missing", None, false);
            }
            let out = handle_command(sb.ctx(), &cmd("project.get", json!({"project_id":project})));
            let rows = sb.ctx.store.sessions_in_project(&project).unwrap();
            assert_eq!(rows.len(), if amount > 0.0 { 2 } else { 0 });
            let p = ok_payload(&out);
            assert_eq!(
                p["cost_usd_total"],
                json!({"unfinished":amount,"finished":0.0})
            );
            assert_eq!(
                p["cost_sessions_without_cost"],
                if amount > 0.0 { 1 } else { 0 }
            );
            expected.push((
                project,
                p["cost_usd_total"].clone(),
                p["cost_sessions_without_cost"].clone(),
            ));
        }
        let out = handle_command(sb.ctx(), &cmd("project.list", json!({})));
        let projects = ok_payload(&out)["projects"].as_array().unwrap();
        assert_eq!(projects.len(), 3);
        for (id, total, missing) in expected {
            let p = projects.iter().find(|p| p["project"]["id"] == id).unwrap();
            assert_eq!(p["cost_usd_total"], total);
            assert_eq!(p["cost_sessions_without_cost"], missing);
        }
    }

    #[test]
    fn cost_rollup_dashboard_task_cards_carry_cost() {
        let mut sb = Sandbox::new("cost-dashboard");
        let project = sb.add_project("p");
        let waiting = cost_task(
            &mut sb,
            &project,
            "waiting",
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
        );
        let running = cost_task(
            &mut sb,
            &project,
            "running",
            TaskState::Active { node: Node::Design },
        );
        for (id, amount) in [(&waiting, 1.25), (&running, 2.5)] {
            cost_session(&sb, id, "paid", Some(amount), false);
            cost_session(&sb, id, "missing", None, false);
            cost_session(&sb, id, "running", None, true);
        }
        let out = handle_command(sb.ctx(), &cmd("dashboard.get", json!({})));
        let p = ok_payload(&out);
        for (key, id, amount) in [("waiting", waiting, 1.25), ("running", running, 2.5)] {
            assert_eq!(p[key].as_array().unwrap().len(), 1);
            assert_eq!(p[key][0]["task"]["id"], id);
            assert_eq!(p[key][0]["task"]["cost_usd"], amount);
            assert_eq!(p[key][0]["task"]["cost_sessions_without_cost"], 1);
        }
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

    fn documents_fixture_task() -> crate::store::TaskRecord {
        crate::store::TaskRecord {
            kind: autome_domain::task::TaskKind::Feature,
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
        }
    }

    #[test]
    fn task_documents_lists_card_bug_doc_and_lessons_in_order() {
        let sb = Sandbox::new("documents-order");
        let mut task = documents_fixture_task();
        task.kind = TaskKind::Bug;
        let dir = sb.root.join(task.doc_dir());
        std::fs::create_dir_all(dir.join("evidence")).unwrap();
        for name in [
            "cart-card.md",
            "cart-bug.md",
            "cart.md",
            "cart-task.md",
            "cart-review.md",
            "cart-adjudication.md",
            "cart-audit.md",
            "retro.md",
            "lessons.md",
            "evidence/z.md",
            "evidence/a.md",
            "evidence/raw.log",
        ] {
            std::fs::write(dir.join(name), "内容").unwrap();
        }
        std::fs::create_dir(dir.join("evidence/directory.md")).unwrap();
        let docs = documents(&sb.root, &task);
        assert_eq!(
            docs.iter()
                .map(|d| d["label"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "任务卡",
                "Bug 文档",
                "任务文件",
                "评审",
                "裁决",
                "审计",
                "运行记录",
                "复盘教训",
                "证据",
                "证据"
            ]
        );
        assert_eq!(docs[8]["name"], "a.md");
        assert_eq!(docs[9]["name"], "z.md");
    }

    #[test]
    fn task_documents_carry_source_worktree() {
        let mut sb = Sandbox::new("documents-source");
        let id = sb.add_project("repo");
        let project = sb.ctx.store.get_project(&id).unwrap();
        let task = documents_fixture_task();
        let root = std::path::PathBuf::from(project.docs_worktree(&task.slug));
        std::fs::create_dir_all(root.join(task.doc_dir())).unwrap();
        std::fs::write(root.join(task.design_doc()), "malformed").unwrap();
        let source = DocumentSource::for_task(&project, &task);
        assert_eq!(source.documents(&task)[0]["source"], "worktree");
        assert!(source.status(&task).error().is_some());
        std::fs::remove_file(root.join(task.design_doc())).unwrap();
        assert!(source.documents(&task).is_empty());
        assert!(source.status(&task).json().is_null());
        // An existing, empty task directory must not fall back to the main checkout.
        assert!(DocumentSource::for_task(&project, &task).branch.is_none());
    }

    fn check_documents_default_branch(workspace: bool) {
        let mut sb = Sandbox::new("documents-default");
        let id = sb.add_project("repo");
        let mut project = sb.ctx.store.get_project(&id).unwrap();
        let mut task = documents_fixture_task();
        task.doc_root = Some("任务 文档".into());
        if workspace {
            project.path = sb.root.to_string_lossy().into_owned();
            project.docs_repo = Some("repo".into());
            project.kind = autome_domain::project::ProjectKind::Workspace;
        }
        let repo = sb.path("repo");
        let dir = repo.join(task.doc_dir());
        std::fs::create_dir_all(dir.join("evidence")).unwrap();
        std::fs::write(dir.join("cart-card.md"), "card").unwrap();
        std::fs::write(dir.join("lessons.md"), "lessons").unwrap();
        std::fs::write(dir.join("evidence/a.md"), "evidence").unwrap();
        std::fs::write(dir.join("cart.md"), "malformed from default").unwrap();
        git::commit_paths(&repo, &[&task.doc_dir()], "documents").unwrap();
        // HEAD and disk diverge from the default branch: no untracked/other-branch leakage.
        git::run_ok(&repo, &["checkout", "-b", "other"]).unwrap();
        std::fs::write(dir.join("cart.md"), "").unwrap();
        std::fs::write(dir.join("retro.md"), "untracked").unwrap();
        std::fs::remove_file(dir.join("lessons.md")).unwrap();
        let source = DocumentSource::for_task(&project, &task);
        let docs = source.documents(&task);
        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0]["name"], "cart-card.md");
        assert!(docs.iter().all(|d| d["source"] == "default_branch"));
        assert_eq!(
            std::fs::canonicalize(docs[0]["absolute"].as_str().unwrap()).unwrap(),
            std::fs::canonicalize(dir.join("cart-card.md")).unwrap()
        );
        assert!(source.status(&task).error().is_some());
        // Default branch read is distinct from the empty file on disk.
        assert!(matches!(source.status(&task), DocRead::Unreadable(_)));
    }

    #[test]
    fn task_documents_after_cleanup_come_from_the_default_branch() {
        check_documents_default_branch(false);
    }

    #[test]
    fn task_documents_workspace_reads_the_docs_repo_default_branch() {
        check_documents_default_branch(true);
    }

    #[test]
    fn task_documents_cancelled_unmerged_task_is_empty() {
        let mut sb = Sandbox::new("documents-cancelled");
        let id = sb.add_project("repo");
        let project = sb.ctx.store.get_project(&id).unwrap();
        let task = documents_fixture_task();
        let dir = sb.path("repo").join(task.doc_dir());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cart-card.md"), "not committed").unwrap();
        let source = DocumentSource::for_task(&project, &task);
        assert!(source.documents(&task).is_empty());
        assert!(source.status(&task).json().is_null());
        // Missing repository/read failure preserves the empty response.
        std::fs::remove_dir_all(sb.path("repo")).unwrap();
        assert!(source.documents(&task).is_empty());
        assert!(source.status(&task).json().is_null());
    }

    #[test]
    fn the_document_list_includes_the_per_round_evidence_files() {
        // Since the 2026-09-16 audit the evidence lives in its own directory
        // rather than being appended to the design document. Main resolves an
        // open by matching a name against this list, so an evidence file that
        // is not here is one the user cannot open from the panel.
        let sb = Sandbox::new("docs-evidence");
        let task = crate::store::TaskRecord {
            kind: autome_domain::task::TaskKind::Feature,
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
            kind: autome_domain::task::TaskKind::Feature,
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
    fn a_decision_shows_what_the_document_says_now() {
        // The row remembers the user's disposition; the prose belongs to the
        // document. A dispute stored by an older parser read "事实：仓库里没有"
        // and stopped — the panel put that half-sentence next to a 保存裁定
        // button. Reading the text back from the document fixes it without
        // waiting for a session to end and re-sync.
        use autome_domain::status_block::{BacklogItem, DisputeItem};
        let stored = crate::store::DecisionRecord {
            task_id: "T-1".into(),
            kind: "dispute".into(),
            item_id: "C-D3".into(),
            text: "事实：仓库里没有".into(),
            disposition: autome_domain::task::Disposition::None,
            ruling: None,
            consumed_at: None,
            carried_by: None,
        };
        let mut block = status_block::parse(
            "status: 设计中\ndesign-round: 1/15\nimplementation-round: 0/25\n\
             current-milestone: 无\ncurrent-milestone-reopens: 0\n\
             convergence-mode: normal\nnext-action: 无\n",
        )
        .unwrap();
        block.disputes = vec![DisputeItem {
            id: "C-D3".into(),
            text: "事实：仓库里没有 群内 AI 回复执行器".into(),
        }];
        block.backlog = vec![BacklogItem {
            id: "B-01".into(),
            text: "完整的一条".into(),
        }];

        let shown = decision_json(&stored, Some(&block));
        assert_eq!(shown["text"], json!("事实：仓库里没有 群内 AI 回复执行器"));

        // An item the document no longer carries keeps what the user saw.
        let gone = crate::store::DecisionRecord {
            item_id: "C-D9".into(),
            ..stored.clone()
        };
        assert_eq!(
            decision_json(&gone, Some(&block))["text"],
            json!("事实：仓库里没有")
        );
        assert_eq!(
            decision_json(&gone, None)["text"],
            json!("事实：仓库里没有")
        );
    }

    #[test]
    fn cancelling_a_task_kills_the_session_it_was_running() {
        // Cancel deletes the checkout the session is writing into. Leaving the
        // process alive meant a CLI still costing money, still writing, for a
        // task the user had just said they did not want — and `stop`, the
        // gentler button, had always killed it.
        needs_git!();
        let mut sb = Sandbox::new("cancel-kills");
        let id = sb.add_project("p");
        // A worktree is cut from the default branch, which does not exist
        // until something is committed on it.
        let repo = sb.path("p");
        std::fs::write(repo.join("README.md"), "hi\n").unwrap();
        git::commit_paths(&repo, &["README.md"], "initial").unwrap();

        let created = handle_command(
            sb.ctx(),
            &cmd(
                "task.create",
                json!({ "project_id": id, "request": "做点什么" }),
            ),
        );
        let task_id = ok_payload(&created)["task"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        handle_command(sb.ctx(), &cmd("scheduler.tick", json!({})));
        assert!(
            sb.ctx().store.running_session(&task_id).unwrap().is_some(),
            "the task should have a session to kill; state={:?}",
            sb.ctx().store.get_task(&task_id).unwrap().state
        );

        handle_command(sb.ctx(), &cmd("task.cancel", json!({ "task_id": task_id })));

        assert!(
            sb.ctx().store.running_session(&task_id).unwrap().is_none(),
            "the session is recorded as finished, not left running"
        );
    }

    #[test]
    fn removing_a_project_with_a_live_task_is_refused() {
        needs_git!();
        let mut sb = Sandbox::new("remove-live");
        let id = sb.add_project("p");
        sb.ctx()
            .store
            .insert_task(&crate::store::TaskRecord {
                kind: autome_domain::task::TaskKind::Feature,
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
    fn probe_rejection_matches_project_add_for_each_kind() {
        let mut sb = Sandbox::new("probe-rejections");
        let id = sb.add_project("parent/repo");
        let file = sb.path("file");
        std::fs::write(&file, "x").unwrap();
        for (path, kind) in [
            (file, "not_a_directory"),
            (sb.path("parent/repo"), "already_project"),
            (sb.path("parent/repo/missing/.."), "already_project"),
            (sb.path("parent/repo/new/child"), "nested_in_project"),
            (sb.path("parent"), "contains_project"),
        ] {
            let existed = path.exists();
            let out = handle_command(sb.ctx(), &cmd("project.probe", json!({"path":path})));
            let rejection = ok_payload(&out)["rejection"].clone();
            assert_eq!(rejection["kind"], kind);
            assert_eq!(path.exists(), existed, "probe must not create paths");
            if kind != "not_a_directory" {
                assert_eq!(rejection["project_id"], id);
            }
            let out = handle_command(sb.ctx(), &cmd("project.add", json!({"path":path})));
            assert_eq!(rejection["message"], err_message(&out));
        }
        // Existing symlink aliases use the same canonical identity.
        let alias = sb.path("alias");
        std::os::unix::fs::symlink(sb.path("parent/repo"), &alias).unwrap();
        let out = handle_command(sb.ctx(), &cmd("project.probe", json!({"path":alias})));
        assert_eq!(ok_payload(&out)["rejection"]["kind"], "already_project");
        sb.ctx.store.remove_project(&id).unwrap();
        let path = sb.path("parent/repo");
        let out = handle_command(sb.ctx(), &cmd("project.probe", json!({"path":path})));
        assert!(ok_payload(&out)["rejection"].is_null());
        let out = handle_command(sb.ctx(), &cmd("project.add", json!({"path":path})));
        assert_eq!(ok_payload(&out)["project"]["id"], id);
    }

    #[test]
    fn probe_rejection_normal_directory_is_null() {
        let mut sb = Sandbox::new("probe-normal");
        for path in [sb.path("中文 空目录"), sb.path("missing/deep")] {
            if path.file_name().unwrap() != "deep" {
                std::fs::create_dir_all(&path).unwrap();
            }
            let existed = path.exists();
            let out = handle_command(sb.ctx(), &cmd("project.probe", json!({"path":path})));
            assert_eq!(ok_payload(&out).get("rejection"), Some(&Value::Null));
            assert_eq!(path.exists(), existed);
        }
    }

    #[test]
    fn config_get_reports_every_resolved_key_and_marks_defaults() {
        let mut sb = Sandbox::new("config-complete");
        let id = sb.add_project("repo");
        let local = sb.path("repo/.autome/config.toml");
        let global = sb.ctx.autome_home.join("config.toml");
        std::fs::write(&local, "").unwrap();
        std::fs::write(&global, "").unwrap();
        let out = handle_command(sb.ctx(), &cmd("config.get", json!({"project_id":id})));
        let got = ok_payload(&out);
        let keys = [
            "bug",
            "gate",
            "gate_concurrency",
            "gate_quick",
            "gate_quick_concurrency",
            "gate_reuse_for_main_check",
            "gate_run",
            "gate_same_failure_limit",
            "loop_defaults",
            "merge_after",
            "merge_auto",
            "retry",
            "roles",
            "setup",
        ];
        assert_eq!(
            got["resolved"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            keys
        );
        assert_eq!(got["defaults"], json!(keys));
        assert_eq!(got["resolved"]["gate_run"], "each_round");
        assert_eq!(got["resolved"].get("gate_quick"), Some(&Value::Null));
        assert_eq!(got["resolved"].get("setup"), Some(&Value::Null));
        assert_eq!(
            got.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["defaults", "global", "resolved", "scope", "violations"]
        );
        // Explicit values equal to built-ins must not be called defaults;
        // project concurrency remains bounded by the global limit.
        std::fs::write(
            &local,
            "[gate]\nrun = 'each_round'\nconcurrency = 10\n[merge]\nauto = false\n",
        )
        .unwrap();
        std::fs::write(
            &global,
            "[gate]\nconcurrency = 1\nquick_concurrency = 10\n[loop]\nparallel = 3\n",
        )
        .unwrap();
        let out = handle_command(sb.ctx(), &cmd("config.get", json!({"project_id":id})));
        let got = ok_payload(&out);
        for key in [
            "gate_run",
            "gate_concurrency",
            "gate_quick_concurrency",
            "merge_auto",
            "loop_defaults",
            "loop_defaults.parallel",
        ] {
            assert!(
                !got["defaults"].as_array().unwrap().contains(&json!(key)),
                "{key}"
            );
        }
        assert!(
            got["defaults"]
                .as_array()
                .unwrap()
                .contains(&json!("loop_defaults.session_minutes"))
        );
        assert_eq!(got["resolved"]["gate_concurrency"], 1);
        assert_eq!(got["resolved"]["gate_quick_concurrency"], 10);
        let resolved = config::resolve(&Default::default(), &Default::default());
        assert!(
            json!(resolved).get("gate_run").is_none(),
            "shared serde must stay sparse"
        );
    }

    #[test]
    fn dashboard_tolerates_a_broken_project_config() {
        let mut sb = Sandbox::new("dashboard-config");
        let bad = sb.add_project("bad");
        let good = sb.add_project("good");
        let bad_task = cost_task(
            &mut sb,
            &bad,
            "bad-task",
            TaskState::Active {
                node: Node::MergeGate,
            },
        );
        let good_task = cost_task(&mut sb, &good, "good-task", TaskState::Queued);
        let bad_queued = cost_task(&mut sb, &bad, "bad-queued", TaskState::Queued);
        let bad_waiting = cost_task(
            &mut sb,
            &bad,
            "bad-waiting",
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
        );

        sb.ctx
            .store
            .append_event("gate.queued", &bad_task, json!({"lane":"heavy"}))
            .unwrap();
        let path = sb.path("bad/.autome/config.toml");
        for broken in ["[", "[gate]\nconcurrency = 0\n"] {
            std::fs::write(&path, broken).unwrap();
            let expected = config_io::load_project(&sb.path("bad").canonicalize().unwrap())
                .unwrap_err()
                .to_string();
            let out = handle_command(sb.ctx(), &cmd("dashboard.get", json!({})));
            let got = ok_payload(&out);
            assert_eq!(
                got.as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                [
                    "alerts",
                    "config_warnings",
                    "cooldowns",
                    "environment",
                    "projects",
                    "queued",
                    "running",
                    "waiting"
                ]
            );
            let rows = got["projects"].as_array().unwrap();
            assert_eq!(rows.len(), 2);
            assert_eq!(
                rows.iter().find(|p| p["project"]["id"] == bad).unwrap()["config_error"],
                expected
            );
            assert!(
                rows.iter().find(|p| p["project"]["id"] == good).unwrap()["config_error"].is_null()
            );
            assert_eq!(got["running"][0]["task"]["id"], bad_task);
            assert!(got["running"][0]["task"]["merge"].is_null());
            assert_eq!(got["waiting"][0]["task"]["id"], bad_waiting);
            let queued = got["queued"].as_array().unwrap();
            assert_eq!(queued.len(), 2);
            assert!(
                queued
                    .iter()
                    .find(|r| r["task"]["id"] == bad_queued)
                    .unwrap()["task"]["merge"]
                    .is_null()
            );
            let healthy = queued
                .iter()
                .find(|r| r["task"]["id"] == good_task)
                .unwrap();
            assert!(healthy["task"]["merge"].is_object());
        }
        std::fs::write(path, "").unwrap();
        let out = handle_command(sb.ctx(), &cmd("dashboard.get", json!({})));
        assert!(ok_payload(&out)["running"][0]["task"]["merge"].is_object());
    }

    #[test]
    fn dashboard_tolerates_project_get_with_config_error() {
        let mut sb = Sandbox::new("project-config");
        let id = sb.add_project("repo");
        let task = cost_task(&mut sb, &id, "task", TaskState::Queued);
        let path = sb.path("repo/.autome/config.toml");
        // A directory at the file path exercises an actual read failure.
        if path.exists() {
            std::fs::remove_file(&path).unwrap();
        }
        std::fs::create_dir(&path).unwrap();
        let expected = config_io::load_project(&sb.path("repo").canonicalize().unwrap())
            .unwrap_err()
            .to_string();
        let out = handle_command(sb.ctx(), &cmd("project.get", json!({"project_id":id})));
        let got = ok_payload(&out);
        assert!(got["config"].is_null());
        assert_eq!(got["config_error"], expected);
        assert_eq!(got["tasks"][0]["id"], task);
        assert!(got["rules"].is_array());
        let out = handle_command(sb.ctx(), &cmd("project.list", json!({})));
        assert_eq!(ok_payload(&out)["projects"].as_array().unwrap().len(), 1);
        std::fs::remove_dir(&path).unwrap();
        let out = handle_command(sb.ctx(), &cmd("project.get", json!({"project_id":id})));
        assert!(ok_payload(&out)["config"].is_object());
        assert!(ok_payload(&out)["config_error"].is_null());
    }

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
    fn project_file_parallel_override_queues_the_second_task() {
        assert!(git_available(), "this regression requires git");
        let mut sb = Sandbox::new("cfg-file-parallel");
        let mut global = config_io::load_global(&sb.ctx.autome_home).unwrap();
        global.loop_defaults.parallel = 3;
        config_io::save_global(&sb.ctx.autome_home, &global).unwrap();
        let id = sb.add_project("repo");
        let repo = sb.path("repo");
        crate::git::commit_paths(
            &repo,
            &[".autome", ".gitignore", "AGENTS.md"],
            "initialize regression fixture",
        )
        .unwrap();

        // Edit the file after registration, without going through config.set_loop.
        std::fs::write(repo.join(".autome/config.toml"), "[loop]\nparallel = 1\n").unwrap();
        let config = handle_command(sb.ctx(), &cmd("config.get", json!({"project_id": id})));
        assert_eq!(
            ok_payload(&config)["resolved"]["loop_defaults"]["parallel"],
            1
        );

        let first = handle_command(
            sb.ctx(),
            &cmd("task.create", json!({"project_id": id, "request": "first"})),
        );
        assert_eq!(
            ok_payload(&first)["started"],
            true,
            "{}",
            ok_payload(&first)
        );
        let second = handle_command(
            sb.ctx(),
            &cmd(
                "task.create",
                json!({"project_id": id, "request": "second"}),
            ),
        );
        let second = ok_payload(&second);
        assert_eq!(
            second["started"], false,
            "project file parallel=1 must queue the second task: {second}"
        );
        assert_eq!(second["task"]["state"]["state"], "queued");
        assert_eq!(second["queue_position"], 1);
        assert_eq!(sb.ctx.store.slots_in_use(&id).unwrap(), 1);
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

    #[test]
    fn events_by_subject_filters_before_pagination_and_validates_type() {
        let mut sb = Sandbox::new("subject");
        for subject in ["a", "b", "a", "project"] {
            sb.ctx()
                .store
                .append_event("session.started", subject, json!({"task_id":"wrong"}))
                .unwrap();
        }
        let out = handle_command(
            sb.ctx(),
            &cmd("events.since", json!({"subject":"a","limit":1})),
        );
        let first = ok_payload(&out)["events"][0]["seq"].as_u64().unwrap();
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "events.since",
                json!({"subject":"a","after_seq":first,"limit":1}),
            ),
        );
        assert_eq!(ok_payload(&out)["events"].as_array().unwrap().len(), 1);
        assert!(ok_payload(&out)["events"][0]["seq"].as_u64().unwrap() > first);
        for params in [
            json!({"subject":""}),
            json!({"subject":"a","limit":0}),
            json!({"subject":"missing"}),
        ] {
            let out = handle_command(sb.ctx(), &cmd("events.since", params));
            assert_eq!(ok_payload(&out)["events"], json!([]));
            assert_eq!(
                ok_payload(&out)["latest_seq"],
                sb.ctx().store.latest_seq().unwrap()
            );
        }
        for _ in 0..1001 {
            sb.ctx()
                .store
                .append_event("test", "large", json!({}))
                .unwrap();
        }
        let out = handle_command(
            sb.ctx(),
            &cmd("events.since", json!({"subject":"large","limit":u64::MAX})),
        );
        assert_eq!(ok_payload(&out)["events"].as_array().unwrap().len(), 1000);
        let out = handle_command(
            sb.ctx(),
            &cmd(
                "events.since",
                json!({"subject":"large","after_seq":u64::MAX}),
            ),
        );
        assert_eq!(ok_payload(&out)["events"], json!([]));
        let out = handle_command(sb.ctx(), &cmd("events.since", json!({"subject":123})));
        assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
    }
    #[test]
    fn events_by_subject_omitted_preserves_original_bytes() {
        let mut sb = Sandbox::new("subject-compat");
        sb.ctx()
            .store
            .append_event("test", "a", json!({"text":"中文\n"}))
            .unwrap();
        let rows = sb.ctx().store.events_since_with_at(0, 200).unwrap();
        let expected = json!({"events":rows.iter().map(|e|json!({"seq":e.seq,"kind":e.kind,"subject":e.subject,"payload":e.payload,"at":e.at})).collect::<Vec<_>>(),"latest_seq":sb.ctx().store.latest_seq().unwrap()});
        let out = handle_command(sb.ctx(), &cmd("events.since", json!({})));
        assert_eq!(
            serde_json::to_vec(&ok_payload(&out)).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
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
    fn events_since_carries_when_each_event_happened() {
        let mut sb = Sandbox::new("event-time");
        sb.ctx()
            .store
            .append_event("test", "p", json!({"text":"a\nb"}))
            .unwrap();
        let stored = sb.ctx().store.events_since_with_at(0, 100).unwrap();
        let out = handle_command(sb.ctx(), &cmd("events.since", json!({"after_seq":0})));
        let payload = ok_payload(&out);
        let events = payload["events"].as_array().unwrap();
        assert_eq!(events.len(), stored.len());
        for (event, row) in events.iter().zip(stored) {
            assert_eq!(event["at"], row.at);
            assert_eq!(event["payload"], row.payload);
        }
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

    mod task_card_read {
        use super::*;

        fn call(sb: &mut Sandbox, method: &str, params: Value) -> Value {
            ok_payload(&handle_command(sb.ctx(), &cmd(method, params))).clone()
        }

        #[test]
        fn brief_is_returned_byte_for_byte() {
            let mut sb = Sandbox::new("card-read-verbatim");
            let project = sb.add_project("repo");
            for brief in [
                " \n# 中文\r\n{request} {unknown}\n\t".to_string(),
                "字".repeat(21845) + "a",
                "x".into(),
            ] {
                let created = call(
                    &mut sb,
                    "task.create",
                    json!({
                        "project_id": project, "request": "read", "draft": true, "brief": brief
                    }),
                );
                let get = call(
                    &mut sb,
                    "task.get",
                    json!({"task_id": created["task"]["id"]}),
                );
                assert_eq!(get["brief"].as_str().unwrap().as_bytes(), brief.as_bytes());
                assert!(get["task"].get("brief").is_none());
            }
        }

        #[test]
        fn missing_brief_is_explicit_null() {
            let mut sb = Sandbox::new("card-read-null");
            let project = sb.add_project("repo");
            let created = call(
                &mut sb,
                "task.create",
                json!({
                    "project_id": project, "request": "no card", "draft": true
                }),
            );
            let get = call(
                &mut sb,
                "task.get",
                json!({"task_id": created["task"]["id"]}),
            );
            assert_eq!(get.get("brief"), Some(&Value::Null));
        }

        #[test]
        fn archived_brief_remains_readable_and_lists_omit_it() {
            let mut sb = Sandbox::new("card-read-archive");
            let project = sb.add_project("repo");
            let brief = "\n# 保留正文\n{request}\n";
            let created = call(
                &mut sb,
                "task.create",
                json!({
                    "project_id": project, "request": "archive card", "draft": true, "brief": brief
                }),
            );
            let id = created["task"]["id"].as_str().unwrap();
            // Exercise a populated dashboard, since drafts and archived tasks are omitted.
            sb.ctx
                .store
                .set_task_state(
                    id,
                    &TaskState::Active {
                        node: autome_domain::task::Node::Intake,
                    },
                )
                .unwrap();
            let dashboard = call(&mut sb, "dashboard.get", json!({}));
            assert_eq!(dashboard["running"][0]["task"]["id"], id);
            assert!(dashboard["running"][0].get("brief").is_none());
            assert!(dashboard["running"][0]["task"].get("brief").is_none());
            let listing = call(&mut sb, "project.get", json!({"project_id": project}));
            assert_eq!(listing["tasks"][0]["id"], id);
            assert!(listing["tasks"][0].get("brief").is_none());
            sb.ctx.store.set_task_state(id, &TaskState::Draft).unwrap();
            call(&mut sb, "task.cancel", json!({"task_id": id}));
            let task = sb.ctx.store.get_task(id).unwrap();
            assert_eq!(task.state, TaskState::Cancelled);
            assert!(task.is_archived());
            let get = call(&mut sb, "task.get", json!({"task_id": id}));
            assert_eq!(get["brief"], brief);
            assert!(get["task"].get("brief").is_none());
        }
    }

    mod task_card {
        use super::*;
        fn world() -> (Sandbox, String) {
            let mut sb = Sandbox::new("task-card");
            let id = sb.add_project("repo");
            (sb, id)
        }
        fn call(sb: &mut Sandbox, method: &str, params: Value) -> Value {
            ok_payload(&handle_command(sb.ctx(), &cmd(method, params))).clone()
        }
        fn new(sb: &mut Sandbox, project: &str, mut params: Value) -> Value {
            params["project_id"] = json!(project);
            call(sb, "task.create", params)
        }
        fn refuse(sb: &mut Sandbox, project: &str, mut params: Value, message: &str) {
            let before = sb.ctx.store.list_tasks(project).unwrap().len();
            params["project_id"] = json!(project);
            let out = handle_command(sb.ctx(), &cmd("task.create", params));
            assert!(err_message(&out).contains(message), "{}", err_message(&out));
            assert_eq!(sb.ctx.store.list_tasks(project).unwrap().len(), before);
        }
        fn occupy(sb: &mut Sandbox, project: &str) {
            let task = new(sb, project, json!({"request":"occupant", "draft":true}));
            let id = task["task"]["id"].as_str().unwrap();
            sb.ctx
                .store
                .set_task_state(
                    id,
                    &TaskState::Active {
                        node: autome_domain::task::Node::Intake,
                    },
                )
                .unwrap();
            call(
                sb,
                "config.set_loop",
                json!({"project_id": project, "parallel": 1}),
            );
        }
        #[test]
        fn create_without_the_new_params_is_unchanged() {
            let (mut sb, project) = world();
            occupy(&mut sb, &project);
            let out = new(&mut sb, &project, json!({"request":"old request"}));
            assert_eq!(out["task"]["kind"], "feature");
            assert_eq!(out["task"]["state"]["state"], "queued");
            assert_eq!(out["started"], false);
            assert_eq!(out["queue_position"], 1);
            assert_eq!(out.as_object().unwrap().len(), 3);
            assert!(
                sb.ctx
                    .store
                    .task_brief(out["task"]["id"].as_str().unwrap())
                    .unwrap()
                    .is_none()
            );
            let events = sb.ctx.store.events_since(0, 100).unwrap();
            for (_, kind, payload) in events {
                if kind == "task.created" {
                    assert_eq!(payload, json!({"project_id":project}));
                }
            }
        }
        #[test]
        fn create_accepts_kind_bug_and_task_get_and_project_get_carry_it() {
            let (mut sb, project) = world();
            let out = new(
                &mut sb,
                &project,
                json!({"request":"bug", "kind":"bug", "draft":true}),
            );
            let get = call(&mut sb, "task.get", json!({"task_id":out["task"]["id"]}));
            assert_eq!(get["task"]["kind"], "bug");
            let get = call(&mut sb, "project.get", json!({"project_id":project}));
            assert_eq!(get["tasks"][0]["kind"], "bug");
            let out = new(
                &mut sb,
                &project,
                json!({"request":"null kind", "kind":null,"draft":true}),
            );
            assert_eq!(out["task"]["kind"], "feature");
        }
        #[test]
        fn create_rejects_an_unknown_kind_in_chinese() {
            let (mut sb, project) = world();
            for kind in [json!("BUG"), json!(""), json!(7), json!({})] {
                refuse(
                    &mut sb,
                    &project,
                    json!({"request":"x","kind":kind}),
                    "只能是 feature 或 bug",
                );
            }
            for draft in [json!(null), json!("true"), json!(1)] {
                refuse(
                    &mut sb,
                    &project,
                    json!({"request":"x","draft":draft}),
                    "布尔",
                );
            }
            for brief in [json!(null), json!(1)] {
                refuse(
                    &mut sb,
                    &project,
                    json!({"request":"x","brief":brief}),
                    "字符串",
                );
            }
        }
        #[test]
        fn a_brief_of_exactly_64_kib_is_accepted_and_one_byte_more_is_refused() {
            let (mut sb, project) = world();
            let body = format!("{}a", "中".repeat(21845));
            assert_eq!(body.len(), 65536);
            let out = new(
                &mut sb,
                &project,
                json!({"request":"x","brief":body,"draft":true}),
            );
            assert_eq!(
                sb.ctx
                    .store
                    .task_brief(out["task"]["id"].as_str().unwrap())
                    .unwrap(),
                Some(body.clone())
            );
            refuse(
                &mut sb,
                &project,
                json!({"brief":format!("{body}b")}),
                "64KB",
            );
        }
        #[test]
        fn an_empty_brief_is_refused() {
            let (mut sb, project) = world();
            for body in ["", " \n\t", "\u{3000}"] {
                refuse(&mut sb, &project, json!({"brief":body}), "正文不能为空");
            }
        }
        #[test]
        fn a_brief_without_a_request_takes_its_request_from_the_first_heading() {
            let (mut sb, project) = world();
            for (body, title) in [
                ("intro\n# 标题\n# second", "标题"),
                ("## 次级标题\n正文", "次级标题"),
                ("x", "x"),
            ] {
                let out = new(&mut sb, &project, json!({"brief":body,"draft":true}));
                assert_eq!(out["task"]["request"], title);
                assert_eq!(out["task"]["title"], title);
            }
            refuse(&mut sb, &project, json!({"brief":"###   "}), "缺少标题");
            refuse(&mut sb, &project, json!({"brief":"#   \nbody"}), "缺少标题");
            refuse(
                &mut sb,
                &project,
                json!({"brief":"# title","request":1}),
                "字符串",
            );
            refuse(&mut sb, &project, json!({"request":"  "}), "需求不能为空");
        }
        #[test]
        fn with_both_the_request_names_the_task_and_the_brief_is_kept_verbatim() {
            let (mut sb, project) = world();
            let body = " # different\r\n{request} 中文 \n";
            let out = new(
                &mut sb,
                &project,
                json!({"request":"named","brief":body,"draft":true}),
            );
            assert_eq!(out["task"]["title"], "named");
            assert_eq!(out["task"]["slug"], "named");
            assert_eq!(
                sb.ctx
                    .store
                    .task_brief(out["task"]["id"].as_str().unwrap())
                    .unwrap()
                    .as_deref(),
                Some(body)
            );
        }
        #[test]
        fn a_draft_is_created_without_starting_or_a_queue_position_and_is_not_on_the_dashboard() {
            let (mut sb, project) = world();
            let out = new(&mut sb, &project, json!({"request":"draft","draft":true}));
            assert_eq!(out["task"]["state"]["state"], "draft");
            assert_eq!(out["started"], false);
            assert!(out["queue_position"].is_null());
            assert_eq!(sb.ctx.store.slots_in_use(&project).unwrap(), 0);
            let dashboard = call(&mut sb, "dashboard.get", json!({}));
            assert_eq!(dashboard["waiting"], json!([]));
            assert_eq!(dashboard["running"], json!([]));
            assert!(!sb.path("repo/.worktree/draft").exists());
        }
        #[test]
        fn task_start_queues_a_draft_behind_the_parallel_limit_and_records_task_started() {
            let (mut sb, project) = world();
            occupy(&mut sb, &project);
            let draft = new(&mut sb, &project, json!({"request":"draft","draft":true}));
            new(&mut sb, &project, json!({"request":"earlier"}));
            let out = call(
                &mut sb,
                "task.start",
                json!({"task_id":draft["task"]["id"]}),
            );
            assert_eq!(out["queue_position"], 2);
            assert_eq!(out["started"], false);
            assert_eq!(out["task"]["state"]["state"], "queued");
            let events = sb.ctx.store.events_since(0, 100).unwrap();
            assert!(
                events
                    .iter()
                    .any(|(_, kind, payload)| kind == "task.started"
                        && *payload == json!({"project_id":project}))
            );
            let (mut free, project) = world();
            for args in [
                vec!["add", "."],
                vec![
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.com",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "initial",
                ],
                vec!["branch", "-M", "main"],
            ] {
                // Bare git, so drop what a git hook would have exported:
                // an inherited GIT_DIR / GIT_INDEX_FILE points at the repo
                // running the suite, not at this temporary one.
                let output = std::process::Command::new("git")
                    .current_dir(free.path("repo"))
                    .env_remove("GIT_DIR")
                    .env_remove("GIT_INDEX_FILE")
                    .env_remove("GIT_WORK_TREE")
                    .args(args)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            let draft = new(
                &mut free,
                &project,
                json!({"request":"start now","draft":true}),
            );
            let out = call(
                &mut free,
                "task.start",
                json!({"task_id":draft["task"]["id"]}),
            );
            assert_eq!(out["started"], true, "{out}");
            assert!(out["queue_position"].is_null());
            assert_eq!(out["task"]["state"]["state"], "active");
        }
        #[test]
        fn task_start_refuses_a_task_that_is_not_a_draft() {
            let (mut sb, project) = world();
            occupy(&mut sb, &project);
            let out = new(&mut sb, &project, json!({"request":"queued"}));
            let out = handle_command(
                sb.ctx(),
                &cmd("task.start", json!({"task_id":out["task"]["id"]})),
            );
            assert!(err_message(&out).contains("只有待开始"));
        }
        #[test]
        fn a_draft_is_cancelled_and_archived_without_a_worktree() {
            let (mut sb, project) = world();
            let out = new(
                &mut sb,
                &project,
                json!({"request":"cancel","brief":"# keep","draft":true}),
            );
            let id = out["task"]["id"].as_str().unwrap();
            call(&mut sb, "task.cancel", json!({"task_id":id}));
            let task = sb.ctx.store.get_task(id).unwrap();
            assert_eq!(task.state, TaskState::Cancelled);
            assert!(task.is_archived());
            assert!(!sb.path("repo/.worktree/cancel").exists());
            assert_eq!(
                sb.ctx.store.task_brief(id).unwrap().as_deref(),
                Some("# keep")
            );
        }
        #[test]
        fn a_draft_is_refused_a_retro_and_task_start_is_not_a_read_method() {
            let (mut sb, project) = world();
            let out = new(&mut sb, &project, json!({"request":"retro","draft":true}));
            let out = handle_command(
                sb.ctx(),
                &cmd("task.retro", json!({"task_id":out["task"]["id"]})),
            );
            assert!(err_message(&out).contains("任务还没开始"));
            assert!(!READ_METHODS.contains(&"task.start"));
        }
    }
    mod conversion {
        use super::*;
        fn rejection_message(out: &Outcome) -> &str {
            assert!(matches!(
                out.reply.outcome,
                ReplyOutcome::Error {
                    code: ReplyErrorCode::TransitionRejected,
                    ..
                }
            ));
            err_message(out)
        }
        const BRIEF: &str = "# 中文卡片\n保留 {request}  \n";
        fn setup() -> (Sandbox, String, String) {
            let mut sb = Sandbox::new("convert");
            sb.ctx.store = Store::open(sb.path("ledger.sqlite")).unwrap();
            let project = sb.add_project("repo");
            let attachment = sb.path("input.txt");
            std::fs::write(&attachment, "附件字节").unwrap();
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "task.create",
                    json!({"project_id": project, "request":"original", "brief":BRIEF,"kind":"bug","draft":true,"attachments":[attachment],"doc_refs":["README.md"]}),
                ),
            );
            let id = ok_payload(&out)["task"]["id"].as_str().unwrap().to_string();
            sb.ctx
                .store
                .set_task_state(
                    &id,
                    &TaskState::Active {
                        node: Node::NotABug,
                    },
                )
                .unwrap();
            let task = sb.ctx.store.get_task(&id).unwrap();
            let repo = sb.path("repo");
            std::fs::write(repo.join("README.md"), "initial").unwrap();
            git::commit_paths(&repo, &["README.md"], "initial").unwrap();
            git::worktree_add(
                &repo,
                &format!(".worktree/{}", task.slug),
                &format!("autome/{}", task.slug),
                "HEAD",
            )
            .unwrap();
            let docs = repo.join(".worktree").join(&task.slug).join(task.doc_dir());
            std::fs::create_dir_all(docs.join("attachments")).unwrap();
            std::fs::write(docs.join("record.md"), "archive me").unwrap();
            std::fs::copy(&attachment, docs.join("attachments/input.txt")).unwrap();
            (sb, project, id)
        }
        fn convert(sb: &mut Sandbox, id: &str, start: bool) -> Outcome {
            handle_command(
                sb.ctx(),
                &cmd(
                    "task.convert",
                    json!({"task_id":id,"kind":"feature","start":start}),
                ),
            )
        }
        fn snapshot(sb: &Sandbox) -> (i64, i64, i64, String, Option<String>) {
            let c = rusqlite::Connection::open(sb.ctx.store.path()).unwrap();
            (
                c.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))
                    .unwrap(),
                c.query_row("SELECT count(*) FROM task_briefs", [], |r| r.get(0))
                    .unwrap(),
                c.query_row("SELECT count(*) FROM events", [], |r| r.get(0))
                    .unwrap(),
                c.query_row("SELECT state FROM tasks ORDER BY id LIMIT 1", [], |r| {
                    r.get(0)
                })
                .unwrap(),
                c.query_row(
                    "SELECT archived_at FROM tasks ORDER BY id LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap(),
            )
        }
        #[test]
        fn task_convert_rebuilds_a_not_a_bug_task_as_a_draft_feature() {
            let (mut sb, project, id) = setup();
            let old = sb.ctx.store.get_task(&id).unwrap();
            let before = sb
                .ctx
                .store
                .events_since_with_subject(0, 1000)
                .unwrap()
                .last()
                .unwrap()
                .0;
            let out = convert(&mut sb, &id, false);
            let value = ok_payload(&out);
            let new_id = value["task"]["id"].as_str().unwrap();
            let new = sb.ctx.store.get_task(new_id).unwrap();
            assert_eq!(new.kind, TaskKind::Feature);
            assert_eq!(new.state, TaskState::Draft);
            assert_eq!(new.request, old.request);
            assert_eq!(new.title, old.title);
            assert_eq!(new.attachments, old.attachments);
            assert_eq!(new.doc_refs, old.doc_refs);
            assert_ne!(new.slug, old.slug);
            assert_eq!(
                sb.ctx.store.task_brief(new_id).unwrap().as_deref(),
                Some(BRIEF)
            );
            let old = sb.ctx.store.get_task(&id).unwrap();
            assert_eq!(old.state, TaskState::Cancelled);
            assert!(old.is_archived());
            assert!(!sb.path("repo").join(".worktree").join(&old.slug).exists());
            assert!(!git::branch_exists(
                &sb.path("repo"),
                &format!("autome/{}", old.slug)
            ));
            assert_eq!(
                std::fs::read_to_string(sb.path("repo").join(old.archive_dir()).join("record.md"))
                    .unwrap(),
                "archive me"
            );
            assert_eq!(value["converted_from"], id);
            assert_eq!(value["started"], false);
            assert!(value.get("queue_position").is_some());
            assert!(!READ_METHODS.contains(&"task.convert"));
            let events = sb.ctx.store.events_since_with_subject(before, 100).unwrap();
            assert_eq!(
                events
                    .iter()
                    .take(3)
                    .map(|e| e.1.as_str())
                    .collect::<Vec<_>>(),
                vec!["task.created", "task.converted", "task.updated"]
            );
            assert_eq!(events[0].2, new_id);
            assert_eq!(
                events[0].3,
                json!({"project_id":project,"converted_from":id})
            );
            assert_eq!(events[1].2, id);
            assert_eq!(events[1].3, json!({"from":id,"to":new_id}));
            assert_eq!(events[2].2, id);
            assert_eq!(
                events[2].3,
                json!({"state":TaskState::Cancelled,"trigger":"cancel"})
            );
            // Legacy absent/empty briefs and the create limit must survive byte-for-byte.
            for brief in [None, Some(String::new()), Some("x".repeat(65536))] {
                let (mut sb, _, id) = setup();
                let c = rusqlite::Connection::open(sb.ctx.store.path()).unwrap();
                c.execute("DELETE FROM task_briefs WHERE task_id=?1", [&id])
                    .unwrap();
                if let Some(body) = &brief {
                    c.execute(
                        "INSERT INTO task_briefs(task_id,body,created_at) VALUES(?1,?2,'now')",
                        rusqlite::params![id, body],
                    )
                    .unwrap();
                }
                let out = handle_command(
                    sb.ctx(),
                    &cmd("task.convert", json!({"task_id":id,"kind":"feature"})),
                );
                let new_id = ok_payload(&out)["task"]["id"].as_str().unwrap();
                assert_eq!(
                    sb.ctx.store.get_task(new_id).unwrap().state,
                    TaskState::Draft
                );
                assert_eq!(sb.ctx.store.task_brief(new_id).unwrap(), brief);
            }
        }
        #[test]
        fn task_convert_with_start_queues_the_new_task() {
            for occupied in [false, true] {
                let (mut sb, project, id) = setup();
                if occupied {
                    let mut blocker = prepare_task(
                        &sb.ctx,
                        &project,
                        "occupant",
                        vec![],
                        vec![],
                        (TaskKind::Feature, true),
                    )
                    .unwrap_or_else(|_| panic!("prepare blocker failed"));
                    blocker.state = TaskState::Active { node: Node::Intake };
                    sb.ctx.store.create_task(&blocker, None).unwrap();
                    ok_payload(&handle_command(
                        sb.ctx(),
                        &cmd(
                            "config.set_loop",
                            json!({"project_id":project,"parallel":1}),
                        ),
                    ));
                }

                let out = convert(&mut sb, &id, true);
                let v = ok_payload(&out);
                let new_id = v["task"]["id"].as_str().unwrap();
                let state = sb.ctx.store.get_task(new_id).unwrap().state;
                assert!(matches!(
                    state,
                    TaskState::Queued | TaskState::Active { .. }
                ));
                assert_eq!(v["started"], !occupied);
                assert_eq!(matches!(state, TaskState::Queued), occupied);
                assert_eq!(
                    sb.ctx.store.count_events(new_id, "task.created").unwrap(),
                    1
                );
                assert_eq!(
                    sb.ctx.store.count_events(new_id, "task.started").unwrap(),
                    0
                );
            }
        }
        #[test]
        fn task_convert_rejects_every_state_other_than_not_a_bug() {
            let (mut sb, _, id) = setup();
            for state in [
                TaskState::Draft,
                TaskState::Queued,
                TaskState::Active { node: Node::Locate },
                TaskState::Active {
                    node: Node::AwaitInfo,
                },
                TaskState::Paused {
                    resume: Node::Locate,
                },
                TaskState::Stopped { at: Node::Locate },
                TaskState::Requeued {
                    resume: Node::Locate,
                },
                TaskState::Failed {
                    at: Node::Locate,
                    reason: autome_domain::task::FailureReason::DesignBudget { limit: 1 },
                },
                TaskState::Cancelled,
                TaskState::Done,
            ] {
                sb.ctx.store.set_task_state(&id, &state).unwrap();
                let before = snapshot(&sb);
                let out = convert(&mut sb, &id, false);
                assert!(rejection_message(&out).contains(&serde_json::to_string(&state).unwrap()));
                assert_eq!(snapshot(&sb), before);
            }
            sb.ctx.store.set_task_archived(&id, true).unwrap();
            let before = snapshot(&sb);
            assert!(rejection_message(&convert(&mut sb, &id, false)).contains("不是 Bug"));
            assert_eq!(snapshot(&sb), before);
            sb.ctx.store.set_task_archived(&id, false).unwrap();
            sb.ctx
                .store
                .set_task_state(
                    &id,
                    &TaskState::Active {
                        node: Node::NotABug,
                    },
                )
                .unwrap();
            let project = sb.ctx.store.get_task(&id).unwrap().project_id;
            sb.ctx.store.remove_project(&project).unwrap();
            let before = snapshot(&sb);
            assert!(rejection_message(&convert(&mut sb, &id, false)).contains("项目已移除"));
            assert_eq!(snapshot(&sb), before);
        }
        #[test]
        fn task_convert_rejects_a_feature_task() {
            let (mut sb, project, _) = setup();
            let out = handle_command(
                sb.ctx(),
                &cmd(
                    "task.create",
                    json!({"project_id":project,"request":"feature","draft":true}),
                ),
            );
            let id = ok_payload(&out)["task"]["id"].as_str().unwrap();
            let before = snapshot(&sb);
            assert!(rejection_message(&convert(&mut sb, id, false)).contains("只有 Bug"));
            assert_eq!(snapshot(&sb), before);
        }
        #[test]
        fn task_convert_rejects_bad_params() {
            let (mut sb, _, id) = setup();
            for p in [
                json!({"kind":"feature"}),
                json!({"task_id":id}),
                json!({"task_id":id,"kind":"bug"}),
                json!({"task_id":id,"kind":"feature","start":"yes"}),
                json!({"task_id":id,"kind":null}),
                json!({"task_id":id,"kind":"feature","start":1}),
                json!({"task_id":id,"kind":"feature","start":null}),
                json!({"task_id":123,"kind":"feature"}),
            ] {
                let before = snapshot(&sb);
                let out = handle_command(sb.ctx(), &cmd("task.convert", p));
                assert!(matches!(
                    out.reply.outcome,
                    ReplyOutcome::Error {
                        code: ReplyErrorCode::InvalidParams,
                        ..
                    }
                ));
                assert_eq!(snapshot(&sb), before);
            }
            for id in ["missing", ""] {
                let out = convert(&mut sb, id, false);
                assert!(matches!(
                    out.reply.outcome,
                    ReplyOutcome::Error {
                        code: ReplyErrorCode::NotFound,
                        ..
                    }
                ));
            }
        }
        #[test]
        fn task_convert_rejects_a_missing_attachment_without_touching_either_task() {
            let (mut sb, _, id) = setup();
            std::fs::remove_file(sb.path("input.txt")).unwrap();
            let before = snapshot(&sb);
            assert!(rejection_message(&convert(&mut sb, &id, false)).contains("原任务里的副本在"));
            assert_eq!(snapshot(&sb), before);
            let old = sb.ctx.store.get_task(&id).unwrap();
            std::fs::remove_file(
                sb.path("repo")
                    .join(".worktree")
                    .join(old.slug.clone())
                    .join(old.doc_dir())
                    .join("attachments/input.txt"),
            )
            .unwrap();
            let out = convert(&mut sb, &id, false);
            assert!(rejection_message(&out).contains("附件不存在"));
            assert!(!rejection_message(&out).contains("副本"));
            assert_eq!(snapshot(&sb), before);
        }
        #[test]
        fn task_convert_leaves_nothing_behind_when_a_commit_step_fails() {
            for trigger in [
                "BEFORE INSERT ON events WHEN NEW.kind='task.converted'".to_string(),
                "BEFORE INSERT ON task_briefs".into(),
                "BEFORE UPDATE ON tasks".into(),
            ] {
                let (mut sb, _, id) = setup();
                let old = sb.ctx.store.get_task(&id).unwrap();
                let before = snapshot(&sb);
                let c = rusqlite::Connection::open(sb.ctx.store.path()).unwrap();
                c.execute_batch(&format!("CREATE TRIGGER fail_convert {trigger} BEGIN SELECT RAISE(ABORT,'injected'); END;")).unwrap();
                let out = convert(&mut sb, &id, false);
                assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
                assert_eq!(snapshot(&sb), before);
                assert!(sb.path("repo").join(".worktree").join(&old.slug).exists());
                assert!(git::branch_exists(
                    &sb.path("repo"),
                    &format!("autome/{}", old.slug)
                ));
                c.execute_batch("DROP TRIGGER fail_convert").unwrap();
                ok_payload(&convert(&mut sb, &id, false));
            }
        }
        #[test]
        fn task_convert_writes_nothing_when_stopping_the_session_fails() {
            let (mut sb, _, id) = setup();
            let path = crate::gate::dir_for(&sb.path("repo"), &id);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "not directory").unwrap();
            let before = snapshot(&sb);
            let out = convert(&mut sb, &id, false);
            assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
            assert_eq!(snapshot(&sb), before);
            std::fs::remove_file(path).unwrap();
            ok_payload(&convert(&mut sb, &id, false));
        }
    }
}
