//! The scheduler. Technical design §2, §5, §8.
//!
//! This is where the design's central claim becomes real: **the core owns
//! every node transition.** An agent writes its result into the design
//! document and exits. The scheduler notices, reads the document, asks the
//! transition table what happens next, and starts it — or does not, because
//! the task is paused, the project's slots are full, the role is disabled, or
//! the budget ran out.
//!
//! Three entry points:
//!
//! - [`tick`] — the periodic pass: reap finished sessions, run core steps,
//!   fill free slots. Everything time-driven happens here.
//! - [`apply_trigger`] — a user action: approve, reject, merge, pause, stop,
//!   cancel, extend, rerun.
//! - [`recover`] — startup reconciliation (design §13).
//!
//! All three funnel into `advance`, so there is exactly one implementation of
//! "apply a transition and act on it".

use std::path::{Path, PathBuf};

use autome_domain::config::{self, ResolvedConfig};
use autome_domain::project::Project;
use autome_domain::role::Role;
use autome_domain::session::{
    self, ExitMarker, Session, SessionKind, SessionLifecycle, SessionPaths,
};
use autome_domain::status_block::{self, StatusBlock};
use autome_domain::task::{
    self, Action, CoreStepResult, FailureReason, Inject, Node, SessionOutcome, TaskState,
    Transition, Trigger,
};
use serde_json::json;

use crate::dispatch::Ctx;
use crate::store::{TaskRecord, now_iso};
use crate::{config_io, git, launcher, skills};

#[derive(Debug)]
pub struct SchedulerError {
    pub detail: String,
}

impl std::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for SchedulerError {}

pub type Result<T> = std::result::Result<T, SchedulerError>;

fn err(detail: impl Into<String>) -> SchedulerError {
    SchedulerError {
        detail: detail.into(),
    }
}

impl From<crate::store::StoreError> for SchedulerError {
    fn from(e: crate::store::StoreError) -> Self {
        err(e.to_string())
    }
}
impl From<config_io::ConfigError> for SchedulerError {
    fn from(e: config_io::ConfigError) -> Self {
        err(e.to_string())
    }
}
impl From<git::GitError> for SchedulerError {
    fn from(e: git::GitError) -> Self {
        err(e.to_string())
    }
}
impl From<launcher::LaunchError> for SchedulerError {
    fn from(e: launcher::LaunchError) -> Self {
        err(e.to_string())
    }
}

/// What one pass changed, for logging and for the tests.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    pub sessions_reaped: Vec<String>,
    pub tasks_advanced: Vec<String>,
    pub tasks_started: Vec<String>,
    pub errors: Vec<String>,
}

impl TickReport {
    pub fn is_empty(&self) -> bool {
        self.sessions_reaped.is_empty()
            && self.tasks_advanced.is_empty()
            && self.tasks_started.is_empty()
            && self.errors.is_empty()
    }
}

// ---------------------------------------------------------------------------
// The periodic pass
// ---------------------------------------------------------------------------

/// One scheduling pass. Safe to call as often as the watcher fires; every step
/// is idempotent, so a tick that races with another finds nothing to do.
pub fn tick(ctx: &mut Ctx) -> TickReport {
    let mut report = TickReport::default();

    // 1. Reap finished sessions and advance the tasks behind them.
    let running = match ctx.store.all_running_sessions() {
        Ok(s) => s,
        Err(e) => {
            report.errors.push(e.to_string());
            return report;
        }
    };
    for s in running {
        match reap_session(ctx, &s) {
            Ok(true) => {
                report.sessions_reaped.push(s.id.clone());
                report.tasks_advanced.push(s.task_id.clone());
            }
            Ok(false) => {}
            Err(e) => report.errors.push(format!("{}: {e}", s.id)),
        }
    }

    // 2. Run any core step a task is sitting on. These are synchronous and
    //    fast enough to do inline; a rebase on a huge repository is the worst
    //    case and it is still seconds.
    let pending = match ctx.store.list_unfinished() {
        Ok(t) => t,
        Err(e) => {
            report.errors.push(e.to_string());
            return report;
        }
    };
    for task in &pending {
        if let TaskState::Active { node } = &task.state
            && node.is_core_step()
        {
            match run_core_step(ctx, task, *node) {
                Ok(()) => report.tasks_advanced.push(task.id.clone()),
                Err(e) => report.errors.push(format!("{}: {e}", task.id)),
            }
        }
    }

    // 3. Fill free slots from each project's queue.
    match fill_slots(ctx) {
        Ok(started) => report.tasks_started.extend(started),
        Err(e) => report.errors.push(e.to_string()),
    }

    report
}

/// Checks one running session against the filesystem and, if it has finished,
/// records the outcome and advances its task. Returns whether it finished.
fn reap_session(ctx: &mut Ctx, s: &Session) -> Result<bool> {
    let task = ctx.store.get_task(&s.task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    let repo = PathBuf::from(&project.path);

    let marker_path = repo.join(SessionPaths::exit(&s.task_id, &s.id));
    let marker = std::fs::read_to_string(&marker_path)
        .ok()
        .and_then(|t| ExitMarker::parse(&t));
    let alive = s.pid.map(launcher::pid_alive).unwrap_or(false);
    let idle = log_idle_secs(&repo.join(SessionPaths::log(&s.task_id, &s.id)));

    let lifecycle = session::classify(marker.as_ref(), alive, idle);
    if lifecycle.is_running() {
        return Ok(false);
    }

    let ended_at = marker
        .as_ref()
        .map(|m| m.ended_at.clone())
        .unwrap_or_else(now_iso);
    ctx.store.finish_session(&s.id, &lifecycle, &ended_at)?;

    // Onboarding sessions are not part of a task's Loop; the project page
    // advances its own wizard.
    if s.kind == SessionKind::Onboarding {
        return Ok(true);
    }

    // Sweep up anything the session left uncommitted, before reading the
    // document (§6). A real run produced a correct README edit, a correct
    // audit and a correct retro — and committed none of it, so the task
    // branch was identical to its base and the merge would have brought
    // nothing across. The prompt asks each round to commit; this is what
    // makes forgetting recoverable rather than silent.
    sweep_commit(&repo, &task, s)?;

    let outcome = read_outcome(&repo, &task, &lifecycle);
    advance(ctx, &task.id, &Trigger::SessionEnded { outcome })?;
    Ok(true)
}

/// Commits whatever a session left behind in its own worktree.
///
/// Scoped to the worktree, so it can only ever pick up the task's own work.
/// Labelled as a sweep, so a reader can tell it apart from a commit the agent
/// made deliberately.
fn sweep_commit(repo: &Path, task: &TaskRecord, session: &Session) -> Result<()> {
    let worktree = worktree_path(repo, &task.slug);
    if !worktree.exists() {
        return Ok(());
    }
    match git::is_clean(&worktree) {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        // A worktree we cannot inspect is not one we should commit into.
        Err(e) => {
            tracing::warn!(task = %task.id, error = %e, "could not check the worktree");
            return Ok(());
        }
    }
    let label = session.kind.label();
    let message = format!("chore(autome): {label} #{} 未提交的剩余改动", session.round);
    match git::commit_paths(&worktree, &["."], &message) {
        Ok(Some(sha)) => {
            tracing::info!(task = %task.id, %sha, "swept up uncommitted session work");
        }
        Ok(None) => {}
        Err(e) => tracing::warn!(task = %task.id, error = %e, "sweep commit failed"),
    }
    Ok(())
}

/// Seconds since the log was last written, for the heartbeat backstop
/// (design §17). A missing log counts as maximally idle: a session that never
/// wrote anything is not one we should wait on forever.
fn log_idle_secs(log: &Path) -> u64 {
    let Ok(meta) = std::fs::metadata(log) else {
        return u64::MAX;
    };
    let Ok(modified) = meta.modified() else {
        return 0;
    };
    modified.elapsed().map(|d| d.as_secs()).unwrap_or(0)
}

/// Turns a finished session into the outcome the transition table expects
/// (design §7.3): a clean exit means "read the document", anything else is a
/// crash, and a document that will not parse is a protocol failure.
fn read_outcome(repo: &Path, task: &TaskRecord, lifecycle: &SessionLifecycle) -> SessionOutcome {
    if !lifecycle.should_parse_document() {
        return SessionOutcome::Crashed {
            detail: match lifecycle {
                SessionLifecycle::Exited { exit_code } => format!("会话退出码 {exit_code}"),
                SessionLifecycle::Killed => "会话被停止".into(),
                SessionLifecycle::Vanished => "会话消失且长时间没有输出".into(),
                SessionLifecycle::Running => unreachable!("checked above"),
            },
        };
    }

    // The document lives on the task branch, so it is read from the worktree,
    // not from the repository root.
    let doc = worktree_path(repo, &task.slug).join(task.design_doc());
    let Ok(text) = std::fs::read_to_string(&doc) else {
        return SessionOutcome::MissingArtifact {
            path: task.design_doc(),
        };
    };
    match status_block::parse(&text) {
        Ok(status) => SessionOutcome::Ok {
            status: Box::new(status),
        },
        Err(error) => SessionOutcome::Unparseable { error },
    }
}

fn worktree_path(repo: &Path, slug: &str) -> PathBuf {
    repo.join(".worktree").join(slug)
}

/// Starts queued tasks while their project has a free slot (design §8).
fn fill_slots(ctx: &mut Ctx) -> Result<Vec<String>> {
    let mut started = Vec::new();
    for project in ctx.store.list_projects()? {
        let limit = project.parallel_limit.max(1);
        let mut in_use = ctx.store.slots_in_use(&project.id)?;
        for task in ctx.store.queued_tasks(&project.id)? {
            if in_use >= limit {
                break;
            }
            match advance(ctx, &task.id, &Trigger::SlotAvailable) {
                Ok(true) => {
                    started.push(task.id.clone());
                    in_use += 1;
                }
                Ok(false) => {}
                Err(e) => {
                    // A task that cannot start must not block the queue behind
                    // it; it is already recorded as Failed by `advance`.
                    tracing::warn!(task = %task.id, error = %e, "task failed to start");
                }
            }
        }
    }
    Ok(started)
}

// ---------------------------------------------------------------------------
// Triggers
// ---------------------------------------------------------------------------

/// Applies a user-initiated trigger. Thin wrapper over `advance`, present so
/// the IPC layer has one obvious call and cannot accidentally bypass the
/// transition table.
pub fn apply_trigger(ctx: &mut Ctx, task_id: &str, trigger: &Trigger) -> Result<()> {
    advance(ctx, task_id, trigger)?;
    Ok(())
}

/// The single implementation of "apply a transition and act on it".
///
/// Order matters and is deliberate: persist the new state *before* performing
/// the action. If the process dies between the two, recovery sees a task in a
/// node with no running session and re-dispatches it — which is recoverable.
/// The other order would leave a running session the store knows nothing
/// about, which is not.
fn advance(ctx: &mut Ctx, task_id: &str, trigger: &Trigger) -> Result<bool> {
    let task = ctx.store.get_task(task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    let resolved = resolve_config(ctx, &project)?;

    // A session end is also when the design document's Backlog and disputes
    // are reconciled into the store, so the user's panel matches the document
    // they would see in an editor.
    if let Trigger::SessionEnded {
        outcome: SessionOutcome::Ok { status },
    } = trigger
    {
        sync_decisions(ctx, &task, status)?;
    }

    let decisions = ctx.store.pending_decisions(task_id)?;
    let context = task::Context {
        config: &resolved,
        budget_n: task.budget_n,
        decisions,
    };

    let transition = match task::apply(&task.state, trigger, &context) {
        Ok(t) => t,
        Err(rejected) => return Err(err(rejected.reason)),
    };

    // Entering the implementation loop for the first time is when N is
    // computed, and it needs the milestone count the design just produced.
    let budget = resolve_budget(&transition, trigger, &resolved, task.budget_n, || {
        read_status(&project, &task)
    });

    ctx.store.set_task_state(task_id, &transition.next)?;
    if let Some(n) = budget {
        ctx.store.set_task_budget(task_id, n)?;
    }
    if transition.consumes_decisions {
        ctx.store.consume_decisions(task_id)?;
    }
    if transition.next == TaskState::Done {
        let merge_commit = task.merge_commit.clone();
        ctx.store.complete_task(task_id, merge_commit.as_deref())?;
    }

    ctx.store.append_event(
        "task.updated",
        task_id,
        json!({ "state": transition.next, "trigger": trigger_name(trigger) }),
    )?;

    perform(ctx, &task, &project, &resolved, &transition)?;
    Ok(true)
}

/// N is computed from the *initial* milestone count when the design is
/// approved, and only then (design §5.5). Later transitions carry their own
/// budget when they change it.
///
/// `status` is a closure rather than a value because reading the design
/// document costs a file read, and every transition but this one already knows
/// its budget.
fn resolve_budget(
    transition: &Transition,
    trigger: &Trigger,
    resolved: &ResolvedConfig,
    current: Option<u32>,
    status: impl FnOnce() -> Option<StatusBlock>,
) -> Option<u32> {
    // A transition that changed the budget carries the new value.
    if let Some(n) = transition.budget_n
        && n > 0
    {
        return Some(n);
    }
    let entering_implementation = matches!(trigger, Trigger::Approve)
        && transition.next
            == (TaskState::Active {
                node: Node::Implement,
            });
    if !entering_implementation || current.is_some() {
        return None;
    }
    // factor × the milestone count the design produced. Using the factor alone
    // — which an earlier version did — gives a five-round budget to a
    // five-milestone task, and the loop runs out partway through the second
    // milestone.
    Some(match status() {
        Some(s) => task::compute_budget(resolved, &s),
        None => resolved.loop_defaults.budget_factor,
    })
}

/// Reads and parses the task's design document, if it is there and valid.
fn read_status(project: &Project, task: &TaskRecord) -> Option<StatusBlock> {
    let repo = Path::new(&project.path);
    let doc = worktree_path(repo, &task.slug).join(task.design_doc());
    let text = std::fs::read_to_string(doc).ok()?;
    status_block::parse(&text).ok()
}

fn trigger_name(t: &Trigger) -> &'static str {
    match t {
        Trigger::SlotAvailable => "slot_available",
        Trigger::SessionEnded { .. } => "session_ended",
        Trigger::Approve => "approve",
        Trigger::Reject { .. } => "reject",
        Trigger::Merge => "merge",
        Trigger::CoreStepDone { .. } => "core_step_done",
        Trigger::Pause => "pause",
        Trigger::Resume => "resume",
        Trigger::Stop => "stop",
        Trigger::Cancel => "cancel",
        Trigger::ExtendBudget { .. } => "extend_budget",
        Trigger::RerunFrom { .. } => "rerun_from",
    }
}

/// Mirrors the design document's Backlog and disputes into the store, so the
/// "待你决定" panel is showing what the document actually says.
fn sync_decisions(ctx: &mut Ctx, task: &TaskRecord, status: &StatusBlock) -> Result<()> {
    let backlog: Vec<(String, String)> = status
        .backlog
        .iter()
        .map(|b| (b.id.clone(), b.text.clone()))
        .collect();
    let disputes: Vec<(String, String)> = status
        .disputes
        .iter()
        .map(|d| (d.id.clone(), d.text.clone()))
        .collect();
    ctx.store.sync_decisions(&task.id, &backlog, &disputes)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

fn perform(
    ctx: &mut Ctx,
    task: &TaskRecord,
    project: &Project,
    resolved: &ResolvedConfig,
    transition: &Transition,
) -> Result<()> {
    match &transition.action {
        Action::None => Ok(()),
        Action::StartIntake => {
            start_session(ctx, task, project, resolved, SessionKind::Intake, None)
        }
        Action::StartRole { role, inject } => start_session(
            ctx,
            task,
            project,
            resolved,
            SessionKind::Role { role: *role },
            inject.as_ref(),
        ),
        Action::RunCoreStep { node } => {
            // Core steps run on the next tick rather than inline, so a long
            // rebase does not block the IPC reply the user is waiting on.
            let _ = node;
            Ok(())
        }
        Action::RunCancel => cancel_task(ctx, task, project),
    }
}

/// Starts a session for a node. Failures here become a task failure rather
/// than an error the user has to interpret: a missing CLI or a blocked
/// configuration is a fact about the task's situation, not a bug.
fn start_session(
    ctx: &mut Ctx,
    task: &TaskRecord,
    project: &Project,
    resolved: &ResolvedConfig,
    kind: SessionKind,
    inject: Option<&Inject>,
) -> Result<()> {
    let repo = PathBuf::from(&project.path);

    // The worktree is created lazily, on the first session that needs it: a
    // queued task should not hold a checkout.
    let worktree = worktree_path(&repo, &task.slug);
    if !worktree.exists() {
        git::worktree_prune(&repo)?;
        let rel = format!(".worktree/{}", task.slug);
        git::worktree_add(&repo, &rel, &task.branch(), &project.default_branch)?;
        write_task_inputs(&repo, &worktree, task)?;
    }

    let role_config = match kind.role() {
        Some(role) => resolved.role(role).config.clone(),
        None => launcher::system_role_config(),
    };

    let decisions = ctx.store.decisions_for_prompt(&task.id)?;
    let prompt = launcher::build_prompt(&launcher::PromptSpec {
        kind,
        slug: &task.slug,
        request: &task.request,
        skills: &role_config.skills,
        inject,
        decisions: &decisions,
        attachments: &task.attachments,
        doc_refs: &task.doc_refs,
    });

    let round = ctx.store.next_round(&task.id, &kind)?;
    let session_id = crate::store::new_id("ses");
    let launched = launcher::launch(&launcher::LaunchSpec {
        session_id: &session_id,
        task_id: &task.id,
        cwd: &worktree,
        repo: &repo,
        runtime: role_config.runtime,
        args: launcher::build_args(&role_config),
        prompt,
        title: launcher::tab_title(&task.id, kind, round),
        mode: ctx.launch_mode,
    })?;

    let pid = read_pid(&repo, &task.id, &session_id);
    ctx.store.insert_session(&Session {
        id: session_id.clone(),
        task_id: task.id.clone(),
        kind,
        runtime: role_config.runtime,
        model: role_config.model.clone(),
        effort: role_config.effort.clone(),
        skills: role_config.skills.clone(),
        round,
        started_at: now_iso(),
        ended_at: None,
        lifecycle: SessionLifecycle::Running,
        log_path: launched.log_path.clone(),
        pid,
    })?;
    ctx.store.append_event(
        "session.started",
        &task.id,
        json!({
            "session_id": session_id,
            "kind": kind,
            "round": round,
            "terminal": launched.terminal.as_str(),
        }),
    )?;
    Ok(())
}

/// The wrapper writes its pid as its first act, but the terminal takes a
/// moment to start it. A brief poll avoids recording `None` and losing the
/// ability to stop the session.
fn read_pid(repo: &Path, task_id: &str, session_id: &str) -> Option<i32> {
    let path = repo.join(SessionPaths::pid(task_id, session_id));
    for _ in 0..40 {
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(pid) = text.trim().parse::<i32>()
        {
            return Some(pid);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    None
}

/// Copies attachments into the worktree's task directory and commits them, so
/// the agent can read them and they travel with the branch (requirement T-01).
fn write_task_inputs(repo: &Path, worktree: &Path, task: &TaskRecord) -> Result<()> {
    if task.attachments.is_empty() {
        return Ok(());
    }
    let dest = worktree.join(task.doc_dir()).join("attachments");
    std::fs::create_dir_all(&dest).map_err(|e| err(format!("无法创建附件目录：{e}")))?;
    for source in &task.attachments {
        let name = Path::new(source)
            .file_name()
            .ok_or_else(|| err(format!("附件路径无效：{source}")))?;
        std::fs::copy(source, dest.join(name))
            .map_err(|e| err(format!("无法复制附件 {source}：{e}")))?;
    }
    let rel = format!("{}/attachments", task.doc_dir());
    git::commit_paths(
        worktree,
        &[&rel],
        &format!("chore(autome): {} inputs", task.id),
    )?;
    let _ = repo;
    Ok(())
}

/// Runs a core step and feeds the result back through the transition table.
fn run_core_step(ctx: &mut Ctx, task: &TaskRecord, node: Node) -> Result<()> {
    let project = ctx.store.get_project(&task.project_id)?;
    let repo = PathBuf::from(&project.path);
    let worktree = worktree_path(&repo, &task.slug);

    let result = match node {
        Node::Rebase => match git::rebase(&worktree, &project.default_branch) {
            Ok(git::RebaseOutcome::Clean) => CoreStepResult::Ok,
            Ok(git::RebaseOutcome::Conflict { files, detail }) => {
                CoreStepResult::Conflict { files, detail }
            }
            Err(e) => CoreStepResult::Failed {
                detail: e.to_string(),
            },
        },
        Node::Merging => {
            let message = format!("merge(autome): {} {}", task.id, task.title);
            match git::merge_task_branch(&repo, &project.default_branch, &task.branch(), &message) {
                Ok(git::MergeOutcome::Merged { commit }) => {
                    ctx.store.complete_task(&task.id, Some(&commit))?;
                    CoreStepResult::Ok
                }
                Ok(git::MergeOutcome::Blocked { detail }) => CoreStepResult::Blocked { detail },
                Err(e) => CoreStepResult::Failed {
                    detail: e.to_string(),
                },
            }
        }
        Node::Cleanup => match cleanup(&repo, &worktree, task) {
            Ok(()) => CoreStepResult::Ok,
            Err(e) => CoreStepResult::Failed {
                detail: e.to_string(),
            },
        },
        other => {
            return Err(err(format!("{other:?} 不是内核步骤")));
        }
    };

    advance(ctx, &task.id, &Trigger::CoreStepDone { node, result })?;
    Ok(())
}

/// Removes the worktree and branch after a successful merge (requirement
/// P-05). Not forced: a dirty worktree at this point means something
/// unexpected, and the transition table treats a cleanup failure as
/// non-fatal precisely so it can be surfaced rather than silently discarded.
fn cleanup(repo: &Path, worktree: &Path, task: &TaskRecord) -> Result<()> {
    if worktree.exists() {
        let rel = format!(".worktree/{}", task.slug);
        git::worktree_remove(repo, &rel, false)?;
    }
    git::worktree_prune(repo)?;
    if git::branch_exists(repo, &task.branch()) {
        git::branch_delete(repo, &task.branch(), false)?;
    }
    Ok(())
}

/// Cancel: archive the documents first, then discard the worktree and branch.
///
/// The order is the point. The task's documents live *on the branch*; deleting
/// the branch first would destroy them (design §6). Copying them to
/// `docs/.archive/` in the main worktree keeps the record of what was tried.
fn cancel_task(ctx: &mut Ctx, task: &TaskRecord, project: &Project) -> Result<()> {
    let repo = PathBuf::from(&project.path);
    let worktree = worktree_path(&repo, &task.slug);

    if worktree.exists() {
        let source = worktree.join(task.doc_dir());
        if source.exists() {
            let dest = repo.join(autome_domain::project::Project::archive_dir(&task.slug));
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = copy_dir(&source, &dest);
        }
        let rel = format!(".worktree/{}", task.slug);
        // Forced: the user asked for the work to go away, so uncommitted
        // changes inside the worktree are exactly what they are discarding.
        let _ = git::worktree_remove(&repo, &rel, true);
    }
    let _ = git::worktree_prune(&repo);
    if git::branch_exists(&repo, &task.branch()) {
        let _ = git::branch_delete(&repo, &task.branch(), true);
    }
    ctx.store.set_task_archived(&task.id, true)?;
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Config resolution
// ---------------------------------------------------------------------------

/// Resolves a project's configuration and refuses to proceed if it is invalid.
///
/// Reading it fresh on every transition is what makes "改动即生效" true
/// (requirement C-09): the next node to start picks up whatever the file says
/// now, without any cache to invalidate.
fn resolve_config(ctx: &Ctx, project: &Project) -> Result<ResolvedConfig> {
    let global = config_io::load_global(&ctx.autome_home)?;
    let project_config = config_io::load_project(Path::new(&project.path))?;
    let resolved = config::resolve(&global, &project_config);
    let inventory = skills::scan(&ctx.home.to_string_lossy(), &project.path);
    let violations = config::validate(&resolved, &inventory);
    if !violations.is_empty() {
        return Err(err(format!("配置无效，无法启动会话：{}", violations.len())));
    }
    Ok(resolved)
}

// ---------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------

/// Startup reconciliation (design §13, §2).
///
/// For every unfinished task: if its session is still alive, leave it alone;
/// if it finished while we were away, consume the result now; if it left no
/// trace at all, mark it crashed so the user gets the failure panel rather
/// than a task that appears to be running forever.
pub fn recover(ctx: &mut Ctx) -> TickReport {
    let mut report = TickReport::default();

    // A worktree registration whose directory is gone would block re-creating
    // the same path; clearing them is safe and cheap.
    //
    // The scaffold is refreshed in the same pass. `SCAFFOLD_VERSION` exists so
    // that a fix to `run_session.sh` or to the session protocol reaches
    // projects that were added before it, and `write_owned` already knows how
    // to rewrite a file carrying an older marker — but nothing ever called it
    // again: `init::init` ran on `project.add` and never afterwards, so
    // bumping the version did nothing to any project that already existed.
    // The launcher reads the wrapper from the repository root, so this is
    // where a wrapper fix has to land.
    if let Ok(projects) = ctx.store.list_projects() {
        for p in projects {
            let path = Path::new(&p.path);
            if !path.is_dir() {
                continue;
            }
            let _ = git::worktree_prune(path);
            match crate::init::init(path) {
                Ok(done) => {
                    for step in done
                        .steps
                        .iter()
                        .filter(|s| s.action == crate::init::Action::Refreshed)
                    {
                        tracing::info!(project = %p.id, file = %step.path, "scaffold refreshed");
                    }
                }
                Err(e) => report
                    .errors
                    .push(format!("{}：刷新脚手架失败 {e}", p.display_name)),
            }
        }
    }

    let tasks = match ctx.store.list_unfinished() {
        Ok(t) => t,
        Err(e) => {
            report.errors.push(e.to_string());
            return report;
        }
    };

    for task in tasks {
        let TaskState::Active { node } = task.state else {
            continue;
        };
        if node.awaits_user() || node.is_core_step() {
            // Waiting on the user survives a restart untouched; a core step is
            // re-run by the next tick, and all three are idempotent.
            continue;
        }
        match ctx.store.running_session(&task.id) {
            Ok(Some(s)) => match reap_session(ctx, &s) {
                Ok(true) => report.tasks_advanced.push(task.id.clone()),
                Ok(false) => {}
                Err(e) => report.errors.push(format!("{}: {e}", task.id)),
            },
            Ok(None) => {
                // Active at a session node with no session: we died between
                // recording the state and launching. Re-dispatch rather than
                // fail — nothing has happened yet that needs undoing.
                if let Err(e) = redispatch(ctx, &task.id, node) {
                    report.errors.push(format!("{}: {e}", task.id));
                } else {
                    report.tasks_started.push(task.id.clone());
                }
            }
            Err(e) => report.errors.push(e.to_string()),
        }
    }

    report
}

/// Re-runs the node a task is already recorded as being in.
fn redispatch(ctx: &mut Ctx, task_id: &str, node: Node) -> Result<()> {
    let task = ctx.store.get_task(task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    let resolved = resolve_config(ctx, &project)?;
    let kind = match node {
        Node::Intake => SessionKind::Intake,
        n => match n.role() {
            Some(role) => SessionKind::Role { role },
            None => return Ok(()),
        },
    };
    start_session(ctx, &task, &project, &resolved, kind, None)
}

/// Starts the Onboarding session (design §10, requirement C-02 step 3).
///
/// Unlike every other session this one has no task and no worktree: it runs in
/// the repository root, because what it produces — the project profile and
/// AGENTS.md — belongs to the project rather than to any piece of work. It
/// therefore also gets a synthetic session directory rather than a per-task
/// one.
pub fn start_onboarding(ctx: &mut Ctx, project_id: &str) -> Result<String> {
    let project = ctx.store.get_project(project_id)?;
    let repo = PathBuf::from(&project.path);
    let role_config = launcher::system_role_config();

    let prompt = launcher::build_prompt(&launcher::PromptSpec {
        kind: SessionKind::Onboarding,
        slug: "onboarding",
        request: "",
        skills: &[],
        inject: None,
        decisions: &[],
        attachments: &[],
        doc_refs: &[],
    });

    let session_id = crate::store::new_id("ses");
    let launched = launcher::launch(&launcher::LaunchSpec {
        session_id: &session_id,
        task_id: ONBOARDING_SESSION_KEY,
        cwd: &repo,
        repo: &repo,
        runtime: role_config.runtime,
        args: launcher::build_args(&role_config),
        prompt,
        title: format!("autome · {} · Onboarding", project.display_name),
        mode: ctx.launch_mode,
    })?;

    ctx.store.append_event(
        "session.started",
        project_id,
        json!({ "session_id": session_id, "kind": "onboarding", "terminal": launched.terminal.as_str() }),
    )?;
    Ok(session_id)
}

/// The synthetic task id Onboarding sessions file their logs under. Not a real
/// task, so it never appears in a task list; it exists only so the wrapper
/// script's paths are well-defined.
pub const ONBOARDING_SESSION_KEY: &str = "onboarding";

/// Whether an Onboarding session has finished, and where its log is.
pub fn onboarding_status(repo: &Path, session_id: &str) -> (bool, Option<String>) {
    let marker = repo.join(SessionPaths::exit(ONBOARDING_SESSION_KEY, session_id));
    let done = std::fs::read_to_string(&marker)
        .ok()
        .and_then(|t| ExitMarker::parse(&t))
        .is_some();
    let log = repo.join(SessionPaths::log(ONBOARDING_SESSION_KEY, session_id));
    (
        done,
        log.exists().then(|| log.to_string_lossy().into_owned()),
    )
}

/// Marks a task failed with a reason, used when a launch could not even be
/// attempted.
pub fn fail_task(ctx: &mut Ctx, task_id: &str, at: Node, reason: FailureReason) -> Result<()> {
    ctx.store
        .set_task_state(task_id, &TaskState::Failed { at, reason })?;
    ctx.store
        .append_event("task.updated", task_id, json!({ "failed": true }))?;
    Ok(())
}

/// Which role a task's next session would use, for the panel's "current
/// session" card before the session actually exists.
pub fn next_role(state: &TaskState) -> Option<Role> {
    state.node().and_then(Node::role)
}

#[cfg(test)]
mod tests {

    #[test]
    fn recovery_brings_an_existing_project_scaffold_up_to_date() {
        // `SCAFFOLD_VERSION` was a mechanism with no trigger: `init::init` ran
        // once on `project.add`, so bumping the version fixed nothing for any
        // project that already existed — including the wrapper script, which
        // the launcher reads from the repository root.
        let mut w = World::new("scaffold-refresh");
        let wrapper = w.repo.join(".autome/skill/run_session.sh");
        std::fs::write(
            &wrapper,
            "#!/bin/sh\n# autome-scaffold-version: 0\necho stale\n",
        )
        .unwrap();

        recover(&mut w.ctx);

        let after = std::fs::read_to_string(&wrapper).unwrap();
        assert!(
            !after.contains("echo stale"),
            "the stale wrapper survived recovery"
        );
        assert!(
            after.contains(&format!(
                "autome-scaffold-version: {}",
                crate::init::SCAFFOLD_VERSION
            )),
            "the wrapper was not brought to the current version"
        );
    }

    #[test]
    fn recovery_leaves_a_scaffold_file_the_user_has_taken_over() {
        // No marker means the user owns the file; the refresh must not stamp
        // over it. That escape hatch is the reason the marker exists.
        let mut w = World::new("scaffold-owned");
        let wrapper = w.repo.join(".autome/skill/run_session.sh");
        std::fs::write(&wrapper, "#!/bin/sh\necho mine\n").unwrap();

        recover(&mut w.ctx);

        assert_eq!(
            std::fs::read_to_string(&wrapper).unwrap(),
            "#!/bin/sh\necho mine\n"
        );
    }
    use super::*;
    use autome_domain::status_block::MilestoneState;
    use autome_domain::task::Disposition;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A real repository, a real store, and a fake CLI that writes whatever
    /// design document the test wants. This is as close to the production path
    /// as a unit test can get without a terminal.
    struct World {
        root: PathBuf,
        repo: PathBuf,
        ctx: Ctx,
        project_id: String,
    }

    impl World {
        fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let root = std::env::temp_dir()
                .join(format!("automed-sched-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let autome_home = root.join("autome-home");
            let home = root.join("home");
            let repo = root.join("repo");
            std::fs::create_dir_all(&autome_home).unwrap();
            std::fs::create_dir_all(&home).unwrap();
            std::fs::create_dir_all(&repo).unwrap();

            git::init(&repo, "main").unwrap();
            crate::init::init(&repo).unwrap();
            std::fs::write(repo.join("README.md"), "hi\n").unwrap();
            git::commit_paths(
                &repo,
                &["README.md", ".autome", ".gitignore", "AGENTS.md"],
                "init",
            )
            .unwrap();

            let store = crate::store::Store::open_in_memory().unwrap();
            let project = Project {
                id: "p1".into(),
                path: repo.to_string_lossy().into_owned(),
                display_name: "repo".into(),
                default_branch: "main".into(),
                parallel_limit: 3,
                onboarding: autome_domain::project::Onboarding::Skipped,
                disposition: autome_domain::project::AddDisposition::AdoptedExisting,
                added_at: now_iso(),
                removed_at: None,
            };
            store.insert_project(&project).unwrap();
            let ctx = Ctx::new(store, &autome_home, &home).dry();
            World {
                root,
                repo,
                ctx,
                project_id: "p1".into(),
            }
        }

        fn add_task(&mut self, id: &str, slug: &str) -> TaskRecord {
            let task = TaskRecord {
                id: id.into(),
                project_id: self.project_id.clone(),
                slug: slug.into(),
                title: "t".into(),
                request: "做点什么".into(),
                attachments: vec![],
                doc_refs: vec![],
                state: TaskState::Queued,
                budget_n: None,
                created_at: now_iso(),
                completed_at: None,
                merge_commit: None,
                archived_at: None,
            };
            self.ctx.store.insert_task(&task).unwrap();
            task
        }

        /// Creates the worktree and writes a design document into it, as a
        /// session would have.
        fn with_worktree(&mut self, task: &TaskRecord) -> PathBuf {
            let rel = format!(".worktree/{}", task.slug);
            let wt = self.repo.join(&rel);
            if !wt.exists() {
                git::worktree_add(&self.repo, &rel, &task.branch(), "main").unwrap();
            }
            wt
        }

        fn write_design(&mut self, task: &TaskRecord, doc: &str) {
            let wt = self.with_worktree(task);
            let path = wt.join(task.design_doc());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, doc).unwrap();
        }

        fn state(&self, task_id: &str) -> TaskState {
            self.ctx.store.get_task(task_id).unwrap().state
        }

        fn set_state(&mut self, task_id: &str, state: TaskState) {
            self.ctx.store.set_task_state(task_id, &state).unwrap();
        }
    }

    impl Drop for World {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn design_doc(status: &str, milestones: &[(&str, MilestoneState)], extra: &str) -> String {
        let rows: String = milestones
            .iter()
            .map(|(id, s)| format!("| {id} | {} | {id} 的标题 | 0 | |\n", s.as_str()))
            .collect();
        let table = if milestones.is_empty() {
            String::new()
        } else {
            format!(
                "\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n{rows}"
            )
        };
        format!(
            "# 标题\n\nstatus: {status}\ndesign-round: 1/15\nimplementation-round: 1/25\n\
             current-milestone: 无\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\n\
             next-action: 无\n{table}{extra}"
        )
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

    fn ended_ok(doc: &str) -> Trigger {
        Trigger::SessionEnded {
            outcome: SessionOutcome::Ok {
                status: Box::new(status_block::parse(doc).expect("test document must parse")),
            },
        }
    }

    // ---- the transition path ---------------------------------------------

    #[test]
    fn a_finished_intake_moves_the_task_to_design() {
        needs_git!();
        let mut w = World::new("intake");
        let task = w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Intake });
        let doc = design_doc("设计中", &[], "");
        // No session is started because Claude is not on PATH in CI; the
        // state change is what this test is about.
        let _ = advance(&mut w.ctx, "T-1", &ended_ok(&doc));
        assert_eq!(w.state("T-1"), TaskState::Active { node: Node::Design });
        let _ = task;
    }

    #[test]
    fn the_design_loop_runs_design_review_adjudicate_and_stops_for_the_user() {
        needs_git!();
        let mut w = World::new("design-loop");
        w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Design });

        let designing = design_doc("设计中", &[], "");
        let _ = advance(&mut w.ctx, "T-1", &ended_ok(&designing));
        assert_eq!(w.state("T-1"), TaskState::Active { node: Node::Review });

        let _ = advance(&mut w.ctx, "T-1", &ended_ok(&designing));
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::Adjudicate
            }
        );

        let final_doc = design_doc("实现中", &[("M-01", MilestoneState::Open)], "");
        let _ = advance(&mut w.ctx, "T-1", &ended_ok(&final_doc));
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::AwaitDesignApproval
            }
        );
    }

    #[test]
    fn approving_a_design_computes_the_budget_from_the_milestone_count() {
        needs_git!();
        let mut w = World::new("budget");
        let task = w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Adjudicate,
            },
        );
        let doc = design_doc(
            "实现中",
            &[
                ("M-01", MilestoneState::Open),
                ("M-02", MilestoneState::Open),
                ("M-03", MilestoneState::Open),
            ],
            "",
        );
        w.write_design(&task, &doc);
        let _ = advance(&mut w.ctx, "T-1", &ended_ok(&doc));
        let _ = advance(&mut w.ctx, "T-1", &Trigger::Approve);
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::Implement
            }
        );
        // factor 5 × 3 milestones — but the transition table only knows the
        // fallback here, so assert it is at least the factor.
        let n = w.ctx.store.get_task("T-1").unwrap().budget_n.unwrap();
        assert!(n >= 5, "budget {n}");
    }

    #[test]
    fn approving_a_five_milestone_design_gets_five_times_the_factor() {
        // The bug this replaced: N was the factor alone, so a five-milestone
        // task got a five-round budget and ran out partway through the second
        // milestone.
        needs_git!();
        let mut w = World::new("budget-count");
        let task = w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
        );
        w.write_design(
            &task,
            &design_doc(
                "实现中",
                &[
                    ("M-01", MilestoneState::Open),
                    ("M-02", MilestoneState::Open),
                    ("M-03", MilestoneState::Open),
                    ("M-04", MilestoneState::Open),
                    ("M-05", MilestoneState::Open),
                ],
                "",
            ),
        );
        let _ = advance(&mut w.ctx, "T-1", &Trigger::Approve);
        assert_eq!(
            w.ctx.store.get_task("T-1").unwrap().budget_n,
            Some(25),
            "factor 5 × 5 milestones"
        );
    }

    #[test]
    fn approving_a_design_with_no_milestones_still_gets_a_workable_budget() {
        needs_git!();
        let mut w = World::new("budget-empty");
        let task = w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
        );
        w.write_design(&task, &design_doc("设计中", &[], ""));
        let _ = advance(&mut w.ctx, "T-1", &Trigger::Approve);
        let n = w.ctx.store.get_task("T-1").unwrap().budget_n.unwrap();
        assert!(n >= 5, "a task must not start already out of budget: {n}");
    }

    #[test]
    fn rejecting_a_design_sends_it_back_with_the_feedback() {
        needs_git!();
        let mut w = World::new("reject");
        w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::AwaitDesignApproval,
            },
        );
        let _ = advance(
            &mut w.ctx,
            "T-1",
            &Trigger::Reject {
                feedback: "不要动支付网关".into(),
            },
        );
        assert_eq!(w.state("T-1"), TaskState::Active { node: Node::Design });
    }

    #[test]
    fn rejecting_without_feedback_is_an_error_and_changes_nothing() {
        needs_git!();
        let mut w = World::new("reject-empty");
        w.add_task("T-1", "a");
        let before = TaskState::Active {
            node: Node::AwaitDesignApproval,
        };
        w.set_state("T-1", before.clone());
        let result = advance(
            &mut w.ctx,
            "T-1",
            &Trigger::Reject {
                feedback: "  ".into(),
            },
        );
        assert!(result.is_err());
        assert_eq!(w.state("T-1"), before);
    }

    #[test]
    fn an_audit_closing_every_milestone_reaches_rebase() {
        needs_git!();
        let mut w = World::new("audit-done");
        w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Audit });
        w.ctx.store.set_task_budget("T-1", 25).unwrap();
        let doc = design_doc(
            "实现中",
            &[
                ("M-01", MilestoneState::Done),
                ("M-02", MilestoneState::Done),
            ],
            "",
        );
        let _ = advance(&mut w.ctx, "T-1", &ended_ok(&doc));
        assert_eq!(w.state("T-1"), TaskState::Active { node: Node::Rebase });
    }

    #[test]
    fn an_unparseable_design_document_fails_the_task_as_a_protocol_error() {
        needs_git!();
        let mut w = World::new("protocol");
        let task = w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Design });
        w.write_design(&task, "这不是一个状态块\n");
        let session = Session {
            id: "s1".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Plan },
            runtime: autome_domain::role::Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: now_iso(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "l".into(),
            pid: None,
        };
        w.ctx.store.insert_session(&session).unwrap();
        // Write a clean exit marker, so the document is what decides.
        let dir = w.repo.join(SessionPaths::dir("T-1"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("s1.exit"),
            ExitMarker {
                exit_code: 0,
                ended_at: now_iso(),
            }
            .render(),
        )
        .unwrap();

        reap_session(&mut w.ctx, &session).unwrap();
        assert!(
            matches!(
                w.state("T-1"),
                TaskState::Failed {
                    reason: FailureReason::Protocol { .. },
                    ..
                }
            ),
            "{:?}",
            w.state("T-1")
        );
    }

    #[test]
    fn a_non_zero_exit_is_a_crash_and_the_document_is_not_consulted() {
        needs_git!();
        let mut w = World::new("crash");
        let task = w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        // A perfectly good document that must NOT rescue a crashed session.
        w.write_design(
            &task,
            &design_doc("实现中", &[("M-01", MilestoneState::Done)], ""),
        );
        let session = Session {
            id: "s1".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Impl },
            runtime: autome_domain::role::Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: now_iso(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "l".into(),
            pid: None,
        };
        w.ctx.store.insert_session(&session).unwrap();
        let dir = w.repo.join(SessionPaths::dir("T-1"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("s1.exit"),
            ExitMarker {
                exit_code: 137,
                ended_at: now_iso(),
            }
            .render(),
        )
        .unwrap();

        reap_session(&mut w.ctx, &session).unwrap();
        assert!(
            matches!(
                w.state("T-1"),
                TaskState::Failed {
                    reason: FailureReason::SessionCrashed { .. },
                    ..
                }
            ),
            "{:?}",
            w.state("T-1")
        );
    }

    #[test]
    fn a_session_that_forgot_to_commit_has_its_work_swept_up() {
        // The failure this prevents: a round does all the work correctly,
        // never commits, the task branch stays identical to its base, and the
        // merge brings nothing across. Silent, and only visible at the end.
        needs_git!();
        let mut w = World::new("sweep");
        let task = w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        w.ctx.store.set_task_budget("T-1", 25).unwrap();
        let wt = w.with_worktree(&task);

        let before = git::head_sha(&wt).unwrap();
        // The session's work, left uncommitted.
        std::fs::write(wt.join("feature.txt"), "the work\n").unwrap();
        w.write_design(
            &task,
            &design_doc("实现中", &[("M-01", MilestoneState::Pending)], ""),
        );

        let session = Session {
            id: "s1".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Impl },
            runtime: autome_domain::role::Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round: 3,
            started_at: now_iso(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "l".into(),
            pid: None,
        };
        w.ctx.store.insert_session(&session).unwrap();
        let dir = w.repo.join(SessionPaths::dir("T-1"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("s1.exit"),
            ExitMarker {
                exit_code: 0,
                ended_at: now_iso(),
            }
            .render(),
        )
        .unwrap();

        reap_session(&mut w.ctx, &session).unwrap();

        assert_ne!(
            git::head_sha(&wt).unwrap(),
            before,
            "the branch must have moved"
        );
        assert!(
            git::is_clean(&wt).unwrap(),
            "nothing should be left uncommitted: {:?}",
            git::dirty_paths(&wt).unwrap()
        );
        let subjects = git::commit_subjects(&w.repo, "main", &task.branch()).unwrap();
        assert!(
            subjects.iter().any(|s| s.contains("实现 #3")),
            "the sweep commit names the round: {subjects:?}"
        );
    }

    #[test]
    fn a_session_that_committed_its_own_work_gets_no_extra_commit() {
        needs_git!();
        let mut w = World::new("no-sweep");
        let task = w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        w.ctx.store.set_task_budget("T-1", 25).unwrap();
        let wt = w.with_worktree(&task);

        w.write_design(
            &task,
            &design_doc("实现中", &[("M-01", MilestoneState::Pending)], ""),
        );
        std::fs::write(wt.join("feature.txt"), "the work\n").unwrap();
        git::commit_paths(&wt, &["."], "feat: the round's own commit").unwrap();
        let after_own_commit = git::head_sha(&wt).unwrap();

        let session = Session {
            id: "s2".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Impl },
            runtime: autome_domain::role::Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: now_iso(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "l".into(),
            pid: None,
        };
        w.ctx.store.insert_session(&session).unwrap();
        let dir = w.repo.join(SessionPaths::dir("T-1"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("s2.exit"),
            ExitMarker {
                exit_code: 0,
                ended_at: now_iso(),
            }
            .render(),
        )
        .unwrap();

        reap_session(&mut w.ctx, &session).unwrap();
        assert_eq!(
            git::head_sha(&wt).unwrap(),
            after_own_commit,
            "a clean worktree must not produce an empty sweep commit"
        );
    }

    #[test]
    fn a_session_with_no_marker_and_a_live_pid_is_left_running() {
        needs_git!();
        let mut w = World::new("still-running");
        w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        let session = Session {
            id: "s1".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Impl },
            runtime: autome_domain::role::Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: now_iso(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "l".into(),
            // Our own pid is certainly alive.
            pid: Some(std::process::id() as i32),
        };
        w.ctx.store.insert_session(&session).unwrap();
        assert!(!reap_session(&mut w.ctx, &session).unwrap());
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::Implement
            }
        );
    }

    // ---- core steps ------------------------------------------------------

    #[test]
    fn a_clean_rebase_advances_to_the_merge_stopping_point() {
        needs_git!();
        let mut w = World::new("rebase-clean");
        let task = w.add_task("T-1", "a");
        w.with_worktree(&task);
        w.set_state("T-1", TaskState::Active { node: Node::Rebase });
        let t = w.ctx.store.get_task("T-1").unwrap();
        run_core_step(&mut w.ctx, &t, Node::Rebase).unwrap();
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::AwaitMerge
            }
        );
    }

    #[test]
    fn a_conflicting_rebase_routes_back_to_implement() {
        needs_git!();
        let mut w = World::new("rebase-conflict");
        let task = w.add_task("T-1", "a");
        let wt = w.with_worktree(&task);
        std::fs::write(wt.join("README.md"), "branch\n").unwrap();
        git::commit_paths(&wt, &["README.md"], "branch edit").unwrap();
        std::fs::write(w.repo.join("README.md"), "main\n").unwrap();
        git::commit_paths(&w.repo, &["README.md"], "main edit").unwrap();

        w.set_state("T-1", TaskState::Active { node: Node::Rebase });
        let t = w.ctx.store.get_task("T-1").unwrap();
        // The launch will fail (no CLI), but the state must have moved first.
        let _ = run_core_step(&mut w.ctx, &t, Node::Rebase);
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::Implement
            }
        );
    }

    #[test]
    fn merging_records_the_commit_and_cleanup_finishes_the_task() {
        needs_git!();
        let mut w = World::new("merge");
        let task = w.add_task("T-1", "a");
        let wt = w.with_worktree(&task);
        std::fs::write(wt.join("feature.txt"), "x\n").unwrap();
        git::commit_paths(&wt, &["feature.txt"], "feature").unwrap();

        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Merging,
            },
        );
        let t = w.ctx.store.get_task("T-1").unwrap();
        run_core_step(&mut w.ctx, &t, Node::Merging).unwrap();
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::Cleanup
            }
        );
        assert!(w.ctx.store.get_task("T-1").unwrap().merge_commit.is_some());
        assert!(w.repo.join("feature.txt").exists());

        let t = w.ctx.store.get_task("T-1").unwrap();
        run_core_step(&mut w.ctx, &t, Node::Cleanup).unwrap();
        assert_eq!(w.state("T-1"), TaskState::Done);
        assert!(!w.repo.join(".worktree/a").exists(), "worktree removed");
        assert!(!git::branch_exists(&w.repo, "autome/a"), "branch removed");
        assert!(w.ctx.store.get_task("T-1").unwrap().completed_at.is_some());
    }

    #[test]
    fn a_dirty_main_worktree_blocks_the_merge_and_returns_to_the_stopping_point() {
        needs_git!();
        let mut w = World::new("merge-blocked");
        let task = w.add_task("T-1", "a");
        let wt = w.with_worktree(&task);
        std::fs::write(wt.join("feature.txt"), "x\n").unwrap();
        git::commit_paths(&wt, &["feature.txt"], "feature").unwrap();
        // Uncommitted work in a file this merge would also write. Dirt in an
        // unrelated file deliberately no longer blocks: see
        // `git::conflicting_dirty_paths`.
        std::fs::write(w.repo.join("feature.txt"), "in progress\n").unwrap();

        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Merging,
            },
        );
        let t = w.ctx.store.get_task("T-1").unwrap();
        run_core_step(&mut w.ctx, &t, Node::Merging).unwrap();
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::AwaitMerge
            }
        );
        assert_eq!(
            std::fs::read_to_string(w.repo.join("feature.txt")).unwrap(),
            "in progress\n",
            "the user's file is untouched"
        );
    }

    // ---- decisions -------------------------------------------------------

    #[test]
    fn a_design_documents_backlog_is_mirrored_into_the_store() {
        needs_git!();
        let mut w = World::new("backlog");
        w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Design });
        let doc = design_doc(
            "设计中",
            &[],
            "\n## Backlog\n\n- B-01 次数上限\n\n## 争议项\n\n- D3-P02 大小写\n",
        );
        let _ = advance(&mut w.ctx, "T-1", &ended_ok(&doc));
        let decisions = w.ctx.store.list_decisions("T-1").unwrap();
        assert_eq!(decisions.len(), 2);
        assert!(decisions.iter().any(|d| d.item_id == "B-01"));
        assert!(decisions.iter().any(|d| d.item_id == "D3-P02"));
    }

    #[test]
    fn merging_with_an_included_backlog_item_returns_to_implement_and_consumes_it() {
        needs_git!();
        let mut w = World::new("consume");
        w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::AwaitMerge,
            },
        );
        w.ctx.store.set_task_budget("T-1", 25).unwrap();
        w.ctx
            .store
            .sync_decisions("T-1", &[("B-01".into(), "上限".into())], &[])
            .unwrap();
        w.ctx
            .store
            .set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();

        let _ = advance(&mut w.ctx, "T-1", &Trigger::Merge);
        assert_eq!(
            w.state("T-1"),
            TaskState::Active {
                node: Node::Implement
            }
        );
        assert_eq!(w.ctx.store.pending_decisions("T-1").unwrap().included, 0);
        assert_eq!(
            w.ctx.store.get_task("T-1").unwrap().budget_n,
            Some(30),
            "the extra milestone is paid for out of a raised budget"
        );
    }

    // ---- control ---------------------------------------------------------

    #[test]
    fn pause_and_resume_move_through_queued_rather_than_jumping_the_limit() {
        needs_git!();
        let mut w = World::new("pause");
        w.add_task("T-1", "a");
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        advance(&mut w.ctx, "T-1", &Trigger::Pause).unwrap();
        assert_eq!(
            w.state("T-1"),
            TaskState::Paused {
                resume: Node::Implement
            }
        );
        advance(&mut w.ctx, "T-1", &Trigger::Resume).unwrap();
        assert_eq!(w.state("T-1"), TaskState::Queued);
    }

    #[test]
    fn cancelling_archives_the_documents_before_destroying_the_branch() {
        needs_git!();
        let mut w = World::new("cancel");
        let task = w.add_task("T-1", "a");
        let wt = w.with_worktree(&task);
        let doc_dir = wt.join(task.doc_dir());
        std::fs::create_dir_all(&doc_dir).unwrap();
        std::fs::write(doc_dir.join("a.md"), "设计内容\n").unwrap();
        git::commit_paths(&wt, &[&task.doc_dir()], "docs").unwrap();
        w.set_state(
            "T-1",
            TaskState::Active {
                node: Node::Implement,
            },
        );

        advance(&mut w.ctx, "T-1", &Trigger::Cancel).unwrap();
        assert_eq!(w.state("T-1"), TaskState::Cancelled);
        let archived = w.repo.join("docs/.archive/a/a.md");
        assert!(archived.exists(), "documents must survive a cancel");
        assert_eq!(std::fs::read_to_string(archived).unwrap(), "设计内容\n");
        assert!(!w.repo.join(".worktree/a").exists());
        assert!(!git::branch_exists(&w.repo, "autome/a"));
    }

    // ---- slots -----------------------------------------------------------

    #[test]
    fn fill_slots_respects_the_projects_parallel_limit() {
        needs_git!();
        let mut w = World::new("slots");
        w.ctx.store.update_project_parallel("p1", 2).unwrap();
        for i in 0..4 {
            w.add_task(&format!("T-{i}"), &format!("s{i}"));
        }
        // Two already running.
        w.set_state(
            "T-0",
            TaskState::Active {
                node: Node::Implement,
            },
        );
        w.set_state("T-1", TaskState::Active { node: Node::Audit });
        let started = fill_slots(&mut w.ctx).unwrap();
        assert!(started.is_empty(), "no free slots: {started:?}");

        // Free one; exactly one more should start.
        w.set_state("T-0", TaskState::Done);
        let started = fill_slots(&mut w.ctx).unwrap();
        assert_eq!(started.len(), 1);
        assert_eq!(started[0], "T-2", "the queue is first-come-first-served");
    }

    #[test]
    fn a_task_waiting_on_the_user_does_not_hold_a_slot() {
        needs_git!();
        let mut w = World::new("slots-waiting");
        w.ctx.store.update_project_parallel("p1", 1).unwrap();
        w.add_task("T-0", "s0");
        w.add_task("T-1", "s1");
        w.set_state(
            "T-0",
            TaskState::Active {
                node: Node::AwaitMerge,
            },
        );
        let started = fill_slots(&mut w.ctx).unwrap();
        assert_eq!(started, vec!["T-1".to_string()]);
    }

    // ---- recovery --------------------------------------------------------

    #[test]
    fn recovery_leaves_a_task_waiting_on_the_user_alone() {
        needs_git!();
        let mut w = World::new("recover-waiting");
        w.add_task("T-1", "a");
        let state = TaskState::Active {
            node: Node::AwaitDesignApproval,
        };
        w.set_state("T-1", state.clone());
        recover(&mut w.ctx);
        assert_eq!(w.state("T-1"), state);
    }

    #[test]
    fn recovery_consumes_a_session_that_finished_while_we_were_away() {
        needs_git!();
        let mut w = World::new("recover-finished");
        let task = w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Design });
        w.write_design(&task, &design_doc("设计中", &[], ""));
        let session = Session {
            id: "s1".into(),
            task_id: "T-1".into(),
            kind: SessionKind::Role { role: Role::Plan },
            runtime: autome_domain::role::Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: now_iso(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "l".into(),
            pid: None,
        };
        w.ctx.store.insert_session(&session).unwrap();
        let dir = w.repo.join(SessionPaths::dir("T-1"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("s1.exit"),
            ExitMarker {
                exit_code: 0,
                ended_at: now_iso(),
            }
            .render(),
        )
        .unwrap();

        recover(&mut w.ctx);
        assert_eq!(
            w.state("T-1"),
            TaskState::Active { node: Node::Review },
            "the design loop moved on"
        );
    }

    #[test]
    fn recovery_does_not_touch_a_core_step_it_can_simply_re_run() {
        needs_git!();
        let mut w = World::new("recover-core");
        w.add_task("T-1", "a");
        let state = TaskState::Active { node: Node::Rebase };
        w.set_state("T-1", state.clone());
        recover(&mut w.ctx);
        assert_eq!(w.state("T-1"), state);
    }

    // ---- config ----------------------------------------------------------

    #[test]
    fn an_invalid_configuration_stops_a_transition_rather_than_launching() {
        needs_git!();
        let mut w = World::new("bad-config");
        w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Design });
        // Move audit onto Claude, where impl already sits with the same
        // (unnamed) model — a SAME-MODEL violation.
        std::fs::write(
            w.repo.join(".autome/config.toml"),
            "[roles.audit]\nruntime = \"claude\"\n",
        )
        .unwrap();
        let doc = design_doc("设计中", &[], "");
        let result = advance(&mut w.ctx, "T-1", &ended_ok(&doc));
        assert!(result.is_err(), "a blocked configuration must not advance");
        assert_eq!(
            w.state("T-1"),
            TaskState::Active { node: Node::Design },
            "state is unchanged"
        );
    }

    #[test]
    fn a_tick_on_an_idle_world_does_nothing() {
        needs_git!();
        let mut w = World::new("idle");
        let report = tick(&mut w.ctx);
        assert!(report.is_empty(), "{report:?}");
    }

    #[test]
    fn log_idle_secs_treats_a_missing_log_as_maximally_idle() {
        assert_eq!(log_idle_secs(Path::new("/nonexistent/log")), u64::MAX);
    }

    #[test]
    fn next_role_maps_a_state_to_the_role_that_would_run() {
        assert_eq!(
            next_role(&TaskState::Active {
                node: Node::Implement
            }),
            Some(Role::Impl)
        );
        assert_eq!(
            next_role(&TaskState::Active {
                node: Node::AwaitMerge
            }),
            None
        );
        assert_eq!(next_role(&TaskState::Done), None);
    }
}
