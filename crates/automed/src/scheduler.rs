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
use autome_domain::protocol::{ProtocolFiles, ProtocolRef};
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
use crate::guards;
use crate::layout::TaskLayout;
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
    /// Folds another pass's findings in. Used by the first tick, which runs
    /// recovery before its own work and must report both.
    pub fn merge(&mut self, other: TickReport) {
        self.sessions_reaped.extend(other.sessions_reaped);
        self.tasks_advanced.extend(other.tasks_advanced);
        self.tasks_started.extend(other.tasks_started);
        self.errors.extend(other.errors);
    }

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

    // The first tick of a process recovers first. Doing it here rather than
    // asking the shell to call `scheduler.recover` is deliberate: the shell
    // was never asked, `recover()` sat unreferenced outside its own tests, and
    // a lifecycle step that depends on a caller remembering is a step that
    // eventually does not happen.
    if !ctx.recovered {
        ctx.recovered = true;
        report.merge(recover(ctx));
    }

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
    let idle = session_idle_secs(
        &repo.join(SessionPaths::log(&s.task_id, &s.id)),
        &s.started_at,
    );

    let lifecycle = session::classify(marker.as_ref(), alive, idle);
    if lifecycle.is_running() {
        return Ok(false);
    }

    let ended_at = marker
        .as_ref()
        .map(|m| m.ended_at.clone())
        .unwrap_or_else(now_iso);
    ctx.store.finish_session(&s.id, &lifecycle, &ended_at)?;
    record_usage(ctx, s, &project, &task, &ended_at);

    // Onboarding sessions are not part of a task's Loop; the project page
    // advances its own wizard.
    if s.kind == SessionKind::Onboarding {
        return Ok(true);
    }

    // A retro run from the failure panel is not part of the Loop. The task is
    // already terminal, it stays where it is, and the transition table would
    // rightly reject a `SessionEnded` against a finished task. Handling it
    // here rather than adding a state to the machine is deliberate: "read the
    // run and write down what it taught us" changes nothing about where the
    // task is, and a state that exists only to come back from is a state.
    if s.kind.role() == Some(Role::Retro) && !matches!(task.state, TaskState::Active { .. }) {
        sweep_commit(&project, &task, s)?;
        check_lessons(ctx, &task, &project)?;
        return Ok(true);
    }

    // Sweep up anything the session left uncommitted, before reading the
    // document (§6). A real run produced a correct README edit, a correct
    // audit and a correct retro — and committed none of it, so the task
    // branch was identical to its base and the merge would have brought
    // nothing across. The prompt asks each round to commit; this is what
    // makes forgetting recoverable rather than silent.
    sweep_commit(&project, &task, s)?;

    let outcome = apply_guards(
        ctx,
        s,
        &project,
        &task,
        read_outcome(&project, &task, &lifecycle),
    )?;
    advance(ctx, &task.id, &Trigger::SessionEnded { outcome })?;
    Ok(true)
}

/// The one place a round's output is admitted (plan §4.D).
///
/// Only a document that parsed is checked: a round that could not produce a
/// readable status block has already failed, and piling a second complaint on
/// top would bury the one a person can act on.
fn apply_guards(
    ctx: &mut Ctx,
    session: &Session,
    project: &Project,
    task: &TaskRecord,
    outcome: SessionOutcome,
) -> Result<SessionOutcome> {
    let SessionOutcome::Ok { status } = &outcome else {
        return Ok(outcome);
    };
    let Some(role) = session.kind.role() else {
        return Ok(outcome);
    };

    let worktree = task_docs_root(project, &task.slug);
    let before: Option<guards::Snapshot> = ctx
        .store
        .last_event(&task.id, "task.snapshot")?
        .and_then(|v| serde_json::from_value(v).ok());

    let retro_path = worktree.join(format!("{}/retro.md", task.doc_dir()));
    let retro = std::fs::read_to_string(&retro_path).unwrap_or_default();
    let retro_lines: Vec<&str> = retro.lines().filter(|l| !l.trim().is_empty()).collect();
    let retro_added = match &before {
        Some(b) if retro_lines.len() > b.retro_lines => retro_lines[b.retro_lines..]
            .iter()
            .map(|l| l.to_string())
            .collect(),
        _ => vec![],
    };

    let design_bytes = std::fs::metadata(worktree.join(task.design_doc()))
        .map(|m| m.len())
        .unwrap_or(0);

    let round = guards::Round {
        role,
        status,
        before: before.as_ref(),
        retro_lines: retro_lines.len(),
        retro_added,
        design_bytes,
        evidence_files: evidence_filenames(&worktree, &task.doc_dir()),
        design_changed_outside_milestones: design_changed_outside_milestones(
            &worktree,
            task,
            before.as_ref(),
            ctx,
        ),
    };
    let findings = guards::check(&round);

    // Before the snapshot is replaced: a milestone that was closed and has
    // just been taken back can only be seen by comparing the two, and the
    // finished document shows it as open with nothing to say it was ever
    // closed.
    if let Some(b) = &before {
        for id in crate::task_metrics::newly_contradicted(&b.milestones, status) {
            ctx.store.append_event(
                "milestone.contradicted",
                &task.id,
                json!({ "milestone": id }),
            )?;
        }
    }

    // The snapshot is written whatever the verdict: the next round has to be
    // compared against what this one actually left, not against what it would
    // have left had it behaved.
    ctx.store.append_event(
        "task.snapshot",
        &task.id,
        json!(guards::Snapshot {
            milestones: crate::task_metrics::snapshot(status),
            retro_lines: round.retro_lines,
            design_bytes,
        }),
    )?;
    if let Ok(head) = git::head_sha(&worktree) {
        ctx.store
            .append_event("task.head", &task.id, json!({ "sha": head }))?;
    }

    for f in &findings {
        ctx.store.append_event(
            match f.level {
                guards::Level::Error => "guard.failed",
                guards::Level::Warning => "guard.warning",
            },
            &task.id,
            json!({ "code": f.code, "detail": f.detail, "role": role.as_str() }),
        )?;
    }

    if role == Role::Retro {
        check_lessons(ctx, task, project)?;
    }

    match findings.iter().find(|f| f.level == guards::Level::Error) {
        Some(f) => Ok(SessionOutcome::GuardFailed {
            detail: f.detail.clone(),
        }),
        None => Ok(outcome),
    }
}

/// Reads `docs/<slug>/lessons.md` and records whether it parsed.
///
/// A malformed lessons file does not fail the task — the task is over, and
/// failing it now would be punishing the wrong round for the wrong thing. But
/// it does have to be visible: a lesson the core cannot read is a lesson that
/// silently never reaches a rule, and "we wrote it down" would be false.
fn check_lessons(ctx: &mut Ctx, task: &TaskRecord, project: &Project) -> Result<()> {
    let path = task_docs_root(project, &task.slug)
        .join(task.doc_dir())
        .join("lessons.md");
    let Ok(text) = std::fs::read_to_string(&path) else {
        ctx.store.append_event(
            "guard.warning",
            &task.id,
            json!({
                "code": "lessons_missing",
                "detail": format!("复盘轮没有写 {}/lessons.md。", task.doc_dir()),
            }),
        )?;
        return Ok(());
    };
    match autome_domain::lesson::parse(&text) {
        Ok(lessons) => {
            ctx.store.append_event(
                "task.lessons",
                &task.id,
                json!({
                    "count": lessons.len(),
                    "lessons": lessons,
                }),
            )?;
        }
        Err(e) => {
            ctx.store.append_event(
                "guard.warning",
                &task.id,
                json!({
                    "code": "lessons_unparseable",
                    "detail": format!("lessons.md 读不出来：{e}"),
                }),
            )?;
        }
    }
    Ok(())
}

fn evidence_filenames(worktree: &Path, doc_dir: &str) -> Vec<String> {
    let dir = worktree.join(doc_dir).join("evidence");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    let mut out: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    out.sort();
    out
}

/// Whether the session changed the design document beyond its milestone table.
///
/// Answered from the commit history rather than by storing the previous
/// document: a design document runs to hundreds of kilobytes, and keeping a
/// copy of each one in the event stream to diff against would cost more than
/// the check is worth. `None` when there is nothing to diff against — the
/// first round, or a worktree git cannot read — because a guess here would
/// warn about rounds that did nothing wrong.
fn design_changed_outside_milestones(
    worktree: &Path,
    task: &TaskRecord,
    before: Option<&guards::Snapshot>,
    ctx: &Ctx,
) -> Option<bool> {
    before?;
    let head = ctx
        .store
        .last_event(&task.id, "task.head")
        .ok()??
        .get("sha")?
        .as_str()?
        .to_string();
    let design = task.design_doc();
    let diff = git::run(
        worktree,
        &[
            "diff",
            "--unified=0",
            &format!("{head}..HEAD"),
            "--",
            &design,
        ],
    )
    .ok()?;
    if !diff.ok() {
        return None;
    }
    // Changed lines only, and only the ones that are not milestone-table rows.
    // A table row is `| M-01 | 待审 | … |`; anything else the audit touched is
    // prose it was not asked to touch.
    let touched_prose = diff
        .stdout
        .lines()
        .filter(|l| {
            (l.starts_with('+') || l.starts_with('-'))
                && !l.starts_with("+++")
                && !l.starts_with("---")
        })
        .map(|l| l[1..].trim())
        .filter(|l| !l.is_empty())
        .any(|l| !(l.starts_with("| M-") || l.starts_with("最新证据：")));
    Some(touched_prose)
}

/// Reads what the session cost out of its raw stream and records it.
///
/// Best-effort on purpose: a session whose `.jsonl` is missing or truncated
/// still has to be reaped and its task still has to advance. A failure here
/// loses a row in a table; making it fatal would lose the task.
fn record_usage(ctx: &mut Ctx, s: &Session, project: &Project, task: &TaskRecord, ended_at: &str) {
    // The session's own output lives beside the project, not in the task's
    // checkout — one directory per task under `.autome/output/`, which is
    // where the wrapper script writes and where the panel reads.
    let repo = Path::new(&project.path);
    let stream_path = repo.join(format!("{}/{}.jsonl", SessionPaths::dir(&s.task_id), s.id));
    let stream = std::fs::read_to_string(&stream_path).unwrap_or_default();
    let wall_ms = wall_clock_ms(&s.started_at, ended_at);
    let mut metrics = crate::usage::parse(s.runtime, &stream, wall_ms);

    let worktree = task_docs_root(project, &task.slug);
    let (bytes, files) = crate::usage::measure_documents(&worktree, &task.doc_dir(), &task.slug);
    metrics.design_doc_bytes = bytes;
    metrics.evidence_files = files;

    if metrics.is_empty() {
        // Worth an event rather than a silent gap: a runtime that stops
        // reporting usage would otherwise show up months later as a version
        // with no numbers and no explanation.
        let _ = ctx.store.append_event(
            "session.usage_missing",
            &s.task_id,
            json!({ "session_id": s.id, "runtime": s.runtime.as_str() }),
        );
        return;
    }
    if let Err(e) = ctx.store.set_session_metrics(&s.id, &metrics) {
        tracing::warn!(session = %s.id, error = %e, "could not record session usage");
    }
}

/// Seconds since an RFC 3339 timestamp. `None` when it cannot be parsed, so
/// the caller can fall back rather than treat an unreadable timestamp as now.
fn secs_since(ts: &str) -> Option<u64> {
    let then = epoch_secs(ts)?;
    let now = epoch_secs(&now_iso())?;
    Some((now - then).max(0) as u64)
}

/// Milliseconds between two RFC 3339 timestamps, for the runtime that does not
/// report a duration of its own. `None` when either cannot be read, rather
/// than a zero that would read as an instant session.
fn wall_clock_ms(started_at: &str, ended_at: &str) -> Option<u64> {
    let start = epoch_secs(started_at)?;
    let end = epoch_secs(ended_at)?;
    (end >= start).then(|| ((end - start) * 1000) as u64)
}

/// Parses `YYYY-MM-DDTHH:MM:SSZ`, the one format `store::now_iso` writes.
fn epoch_secs(ts: &str) -> Option<i64> {
    let bytes = ts.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let num = |a: usize, b: usize| ts.get(a..b)?.parse::<i64>().ok();
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    // Days from civil (Howard Hinnant), the inverse of `store::format_iso`.
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// Commits whatever a session left behind in its own worktree.
///
/// Scoped to the worktree, so it can only ever pick up the task's own work.
/// Labelled as a sweep, so a reader can tell it apart from a commit the agent
/// made deliberately.
fn sweep_commit(project: &Project, task: &TaskRecord, session: &Session) -> Result<()> {
    let worktree = task_cwd(project, &task.slug);
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

/// How long a session has been idle, floored by how long it has existed.
///
/// `log_idle_secs` reports a missing log as maximally idle, which is right for
/// a log that was never written — but between the core recording a session and
/// the terminal actually starting the wrapper there is a window where the log
/// does not exist yet. On an unloaded machine that window is milliseconds; on a
/// busy one it is long enough that the very next tick declared a just-launched
/// session vanished and failed the task before it had run at all. A session
/// cannot have been idle longer than it has existed, and that is the floor that
/// closes it.
///
/// Named rather than inlined at the one call site so the tests below can
/// exercise the floor itself. They used to restate the expression and assert
/// against their own copy, which meant the floor could have been deleted
/// outright and both of them would still have passed.
fn session_idle_secs(log: &Path, started_at: &str) -> u64 {
    log_idle_secs(log).min(secs_since(started_at).unwrap_or(u64::MAX))
}

/// Turns a finished session into the outcome the transition table expects
/// (design §7.3): a clean exit means "read the document", anything else is a
/// crash, and a document that will not parse is a protocol failure.
fn read_outcome(
    project: &Project,
    task: &TaskRecord,
    lifecycle: &SessionLifecycle,
) -> SessionOutcome {
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
    let doc = task_docs_root(project, &task.slug).join(task.design_doc());
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

/// Where this task's session runs.
///
/// Distinct from `task_docs_root` even though they are equal today, and that
/// is the point of having two names: for a workspace the session runs in a
/// directory holding one checkout per member repository, and the documents
/// live inside the member designated to hold them. A call site that reaches
/// for the wrong one compiles, runs, and writes the design document where
/// nobody reads it.
fn task_cwd(project: &Project, slug: &str) -> PathBuf {
    TaskLayout::single(project, slug).cwd
}

/// The checkout holding `<doc_root>/<slug>/…`.
fn task_docs_root(project: &Project, slug: &str) -> PathBuf {
    TaskLayout::single(project, slug).docs_root
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
    let mut task = ctx.store.get_task(task_id)?;
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
        // Also on the record `perform` is about to read. It was loaded at the
        // top of this function, so without this the store says 75 and the
        // prompt the very next session gets says 35 — the panel and the round
        // disagree about the denominator, and the round is the one that acts
        // on it. A real run hit this the round after a merge added eight
        // backlog items: the budget went 35 → 75 and the session was told
        // `32/35`, four rounds from an ending that was no longer there.
        //
        // The protocol answers "who owns N" with "the core states it in the
        // prompt", so a stale number here is not a display bug — it is the
        // core telling the session something untrue about its own budget.
        task.budget_n = Some(n);
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

    // Counted rather than derived from the final state: a task can fail on a
    // protocol error, be re-run from an earlier node, and fail again. The last
    // state remembers one of those; the version page needs all of them.
    if let TaskState::Failed {
        reason: FailureReason::Protocol { detail },
        at,
    } = &transition.next
    {
        ctx.store.append_event(
            "task.protocol_failure",
            task_id,
            json!({ "node": at.as_str(), "detail": detail }),
        )?;
    }

    // A task is measured while its worktree still exists. That rules out
    // waiting for `Done`: `Done` is reached *from* cleanup, which has already
    // deleted the worktree the evidence files live in. So the measurement
    // happens one step earlier, on the way into cleanup — and on the way into
    // Failed or Cancelled, which a task reaches with its worktree intact.
    //
    // A task that went wrong is the most informative kind there is, so the
    // failing paths are measured too.
    let measure_now = matches!(
        transition.action,
        Action::RunCoreStep {
            node: Node::Cleanup
        }
    ) || matches!(
        transition.next,
        TaskState::Failed { .. } | TaskState::Cancelled
    ) || (transition.next == TaskState::Done && task.metrics.is_none());
    if measure_now {
        if let Err(e) = write_task_metrics(ctx, task_id) {
            tracing::warn!(task = %task_id, error = %e, "could not aggregate task metrics");
        }
        // A new sample just arrived, which is the only thing that can make a
        // past prediction judgeable. The core fills the outcome in, never the
        // person who proposed the change: a claim scored by its author is not
        // a claim.
        for filled in crate::backfill::run(ctx) {
            tracing::info!(
                entry = %filled.id,
                metric = %filled.metric,
                held_up = filled.held_up,
                "protocol change measured"
            );
        }
    }

    if let Err(e) = perform(ctx, &task, &project, &resolved, &transition) {
        record_failed_action(ctx, task_id, &transition, &e);
        return Err(e);
    }
    Ok(true)
}

/// Turns an action that could not be carried out into a task the user can see
/// has stopped, and why.
///
/// The state was already written (see the ordering note on `advance`), so an
/// action that then fails leaves a task *active at a node with no session*.
/// Nothing retries it: `fill_slots` only looks at queued tasks, and `recover`
/// only runs at startup. Before this existed, the error went to a
/// `tracing::warn` nobody reads and the task sat on the dashboard as "运行中"
/// forever — which is exactly how a workspace directory whose `docs/` is a
/// nested repository stalled its first task with no message at all.
///
/// `FailureReason::Config` is the right reason and not a new variant:
/// "一个配置问题挡住了启动" is what a missing CLI, a SAME-MODEL collision and
/// an unwritable document root all are, and the failure panel already renders
/// it. `start_session`'s own doc comment promised this behaviour; only the
/// code was missing.
///
/// Best-effort by construction: the caller is already returning an error, and
/// failing to *record* a failure must not replace the original one.
fn record_failed_action(
    ctx: &mut Ctx,
    task_id: &str,
    transition: &Transition,
    error: &SchedulerError,
) {
    let TaskState::Active { node } = transition.next else {
        return;
    };
    let failed = TaskState::Failed {
        reason: FailureReason::Config {
            detail: error.to_string(),
        },
        at: node,
    };
    if let Err(e) = ctx.store.set_task_state(task_id, &failed) {
        tracing::error!(task = %task_id, error = %e, "could not record a failed launch");
        return;
    }
    let _ = ctx.store.append_event(
        "task.updated",
        task_id,
        json!({ "state": failed, "trigger": "action_failed" }),
    );
}

/// Folds a finished task into `TaskMetrics` and stores it.
fn write_task_metrics(ctx: &mut Ctx, task_id: &str) -> Result<()> {
    let task = ctx.store.get_task(task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    let worktree = task_docs_root(&project, &task.slug);

    let status = read_status(&project, &task);
    let (impl_defects, verification_gaps) =
        crate::task_metrics::count_verdicts(&worktree, &task.doc_dir());

    let sessions = ctx.store.list_sessions(task_id)?;
    let mut total_tokens = 0u64;
    let mut total_turns = 0u64;
    let mut cost: Option<f64> = None;
    for s in &sessions {
        total_tokens += s.metrics.total_tokens().unwrap_or(0);
        total_turns += s.metrics.turns.unwrap_or(0);
        if let Some(c) = s.metrics.cost_usd {
            // Only Claude reports a price. Summing what exists and leaving the
            // total absent when nothing does is the honest form: a task run
            // entirely on Codex costs an unknown amount, not zero.
            cost = Some(cost.unwrap_or(0.0) + c);
        }
    }

    let counts = crate::task_metrics::Counts {
        protocol_failures: ctx.store.count_events(task_id, "task.protocol_failure")?,
        closed_then_contradicted: ctx.store.count_events(task_id, "milestone.contradicted")?,
        impl_defects,
        verification_gaps,
        manual_items_open: status
            .as_ref()
            .map(crate::task_metrics::manual_items_open)
            .unwrap_or(0),
        total_cost_usd: cost,
        total_tokens,
        total_turns,
    };

    let metrics = crate::task_metrics::aggregate(
        status.as_ref(),
        task.budget_n.unwrap_or(0),
        task.protocol_ref.clone(),
        counts,
    );
    ctx.store.set_task_metrics(task_id, &metrics)?;
    ctx.store.append_event(
        "task.metrics",
        task_id,
        serde_json::to_value(&metrics).unwrap_or_else(|_| json!({})),
    )?;
    Ok(())
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
    let doc = task_docs_root(project, &task.slug).join(task.design_doc());
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
    let worktree = task_cwd(project, &task.slug);
    if !worktree.exists() {
        git::worktree_prune(&repo)?;
        let rel = format!(".worktree/{}", task.slug);
        git::worktree_add(&repo, &rel, &task.branch(), &project.default_branch)?;
        write_task_inputs(ctx, task, project, &worktree)?;
    }

    // Which rules this task is held to, decided once and then frozen. A task
    // that started under v6 keeps running under v6 even if the user releases
    // v7 in the middle of it — protocol principle 6 as a property of the
    // filesystem rather than a sentence a session has to remember.
    let (protocol_ref, protocol) = freeze_protocol(ctx, task, resolved, &worktree)?;
    let rules_hash = rules_hash(&repo);

    let role_config = match kind.role() {
        Some(role) => resolved.role(role).config.clone(),
        None => launcher::system_role_config(),
    };

    let decisions = ctx.store.decisions_for_prompt(&task.id)?;
    // Read before the prompt is built, not after: the two rounds of the
    // implementation loop are told which round they are and what `N` is,
    // because neither is derivable inside the session (see launcher::BudgetLine).
    let round = ctx.store.next_round(&task.id, &kind)?;
    let budget = match (kind.role(), task.budget_n) {
        (Some(Role::Impl | Role::Audit), Some(limit)) => {
            Some(launcher::BudgetLine { round, limit })
        }
        _ => None,
    };
    // The retro round writes against the task's measured numbers rather than
    // its recollection of the run, so it is the one round handed them.
    let task_metrics = match kind.role() {
        Some(Role::Retro) => ctx.store.task_metrics(&task.id)?,
        _ => None,
    };
    // Written before the prompt that points at it. A round told to read a
    // brief that is not there would go and read the design document instead,
    // which is the thing the brief exists to make optional.
    let brief_path = match kind.role() {
        Some(role) => write_brief(
            ctx,
            task,
            &worktree,
            &protocol,
            role,
            round,
            budget.as_ref(),
            &decisions,
        )?,
        None => String::new(),
    };
    let prompt = launcher::build_prompt(&launcher::PromptSpec {
        kind,
        templates: &protocol,
        brief_path: &brief_path,
        slug: &task.slug,
        doc_dir: &TaskLayout::single(project, &task.slug).doc_dir_from_cwd(&task.doc_dir()),
        design_rounds: resolved.loop_defaults.design_rounds,
        task_metrics: task_metrics.as_ref(),
        budget,
        request: &task.request,
        skills: &role_config.skills,
        inject,
        decisions: &decisions,
        attachments: &task.attachments,
        doc_refs: &task.doc_refs,
    })?;

    let session_id = crate::store::new_id("ses");
    let launched = launcher::launch(&launcher::LaunchSpec {
        session_id: &session_id,
        task_id: &task.id,
        cwd: &worktree,
        repo: &repo,
        runtime: role_config.runtime,
        args: launcher::build_args(&role_config, &worktree),
        prompt,
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
        protocol_ref: Some(protocol_ref.to_wire()),
        rules_hash,
        // Filled in when the session is reaped and its stream is read.
        metrics: Default::default(),
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

/// Resolves the protocol version for a task and, the first time, freezes a
/// copy of it into `docs/<slug>/protocol/`.
///
/// After that the copy *is* the version: it is read back from the worktree
/// rather than re-resolved, so releasing a new protocol tag cannot change the
/// rules a running task is being held to. That is protocol principle 6, and it
/// used to rest on the intake round having pasted the rules into the task file
/// and every later round choosing not to look anywhere else.
fn freeze_protocol(
    ctx: &mut Ctx,
    task: &TaskRecord,
    resolved: &ResolvedConfig,
    worktree: &Path,
) -> Result<(ProtocolRef, ProtocolFiles)> {
    let dir = worktree
        .join(task.doc_dir())
        .join(crate::protocol::TASK_SUBDIR);

    if let Some(recorded) = task.protocol_ref.as_deref().and_then(ProtocolRef::parse)
        && dir.exists()
    {
        let files = read_task_protocol(&dir)?;
        // A mismatch means someone edited the frozen copy. The file wins, as
        // everywhere else in this system, but the disagreement is recorded
        // rather than smoothed over: every metric this task produces is
        // attributed to a version, and this is the one moment the attribution
        // can be seen to be wrong.
        let hash = files.hash();
        if hash != recorded.hash {
            ctx.store.append_event(
                "integrity_warning",
                &task.id,
                json!({
                    "what": "protocol_copy_edited",
                    "recorded": recorded.to_wire(),
                    "actual_hash": hash,
                }),
            )?;
            return Ok((ProtocolRef::new(recorded.tag, hash), files));
        }
        return Ok((recorded, files));
    }

    let pin = resolved.protocol_pin.clone();
    let (protocol_ref, files) = crate::protocol::ensure(&ctx.autome_home)
        .and_then(|r| r.resolve(pin.as_deref()))
        .map_err(|e| err(e.to_string()))?;

    let written = crate::protocol::copy_into_task(&files, worktree, &task.doc_dir())
        .map_err(|e| err(e.to_string()))?;
    let refs: Vec<&str> = written.iter().map(String::as_str).collect();
    git::commit_paths(
        worktree,
        &refs,
        &format!("chore(autome): {} 固定协议 {}", task.id, protocol_ref.tag),
    )?;
    ctx.store
        .set_task_protocol_ref(&task.id, &protocol_ref.to_wire())?;
    Ok((protocol_ref, files))
}

/// Assembles this round's brief and commits it.
///
/// Best-effort in one direction only: if the brief cannot be written the
/// session still starts, because a missing index is worse than no session but
/// much better than a stalled task. The prompt points at it either way, and a
/// round that finds nothing there falls back to the design document — which is
/// exactly what every round did before this existed.
#[allow(clippy::too_many_arguments)]
fn write_brief(
    ctx: &mut Ctx,
    task: &TaskRecord,
    worktree: &Path,
    protocol: &ProtocolFiles,
    role: Role,
    round: u32,
    budget: Option<&launcher::BudgetLine>,
    decisions: &[crate::store::DecisionRecord],
) -> Result<String> {
    let rel = crate::brief::path(&task.doc_dir(), role, round);
    let map = match protocol.get("brief-map.toml").map(crate::brief::parse_map) {
        Some(Ok(m)) => m,
        Some(Err(e)) => {
            // A malformed map costs the brief its protocol sections and
            // nothing else, but it is a defect in the protocol version and
            // must not pass unremarked.
            ctx.store.append_event(
                "guard.warning",
                &task.id,
                json!({ "code": "brief_map_unreadable", "detail": e }),
            )?;
            crate::brief::BriefMap::default()
        }
        None => crate::brief::BriefMap::default(),
    };

    let read = |rel: String| std::fs::read_to_string(worktree.join(rel)).ok();
    let audit_doc = read(format!("{}/{}-audit.md", task.doc_dir(), task.slug));
    let status = read(task.design_doc()).and_then(|t| status_block::parse(&t).ok());
    let evidence = evidence_filenames(worktree, &task.doc_dir());

    let text = crate::brief::build(&crate::brief::Inputs {
        role,
        slug: &task.slug,
        round,
        status: status.as_ref(),
        audit_doc: audit_doc.as_deref(),
        evidence: &evidence,
        loop_protocol: protocol.loop_protocol().unwrap_or_default(),
        session_protocol: protocol.session_protocol().unwrap_or_default(),
        map: &map,
        budget_line: budget.map(|b| {
            format!(
                "本轮是第 {} 轮，实现预算 N = {}。分母由 Autome 计算。\n",
                b.round, b.limit
            )
        }),
        decisions: (!decisions.is_empty()).then(|| {
            let mut s = String::from("\n用户已对以下待决条目作出决定：\n\n");
            for d in decisions {
                s.push_str(&format!("- {} {}\n", d.item_id, d.text));
            }
            s
        }),
    });

    let full = worktree.join(&rel);
    if let Some(parent) = full.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(&full, text).is_err() {
        tracing::warn!(task = %task.id, path = %rel, "could not write the brief");
        return Ok(rel);
    }
    let _ = git::commit_paths(
        worktree,
        &[&rel],
        &format!("chore(autome): {} {} 简报", task.id, role.as_str()),
    );
    Ok(rel)
}

/// Which changes on this branch the review and audit rounds were not
/// competent to judge — a change to their own prompts.
///
/// Recorded at the design stopping point and again at the merge gate, because
/// those are the two places a human is looking. There is no clever fix for the
/// self-reference: an evaluator judging the rules it is evaluated under is a
/// fixed point, not a check.
pub fn needs_human_approval(ctx: &mut Ctx, task_id: &str) -> Result<Vec<String>> {
    let task = ctx.store.get_task(task_id)?;
    let project = ctx.store.get_project(&task.project_id)?;
    if !crate::meta_store::is_protocol_project(ctx, &project) {
        return Ok(vec![]);
    }
    let repo = PathBuf::from(&project.path);
    let changed = git::change_summary(&repo, &project.default_branch, &task.branch())
        .map(|s| s.files.into_iter().map(|f| f.path).collect::<Vec<_>>())
        .unwrap_or_default();
    Ok(crate::meta::needs_human_approval(&changed))
}

/// Reads a task's frozen copy back. Only the files the copy contains — eval
/// fixtures are deliberately not copied, so the hash here is over the same set
/// `copy_into_task` wrote.
fn read_task_protocol(dir: &Path) -> Result<ProtocolFiles> {
    fn walk(root: &Path, dir: &Path, out: &mut ProtocolFiles) -> Result<()> {
        for entry in std::fs::read_dir(dir).map_err(|e| err(format!("读取协议副本失败：{e}")))?
        {
            let entry = entry.map_err(|e| err(format!("读取协议副本失败：{e}")))?;
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                let rel = path
                    .strip_prefix(root)
                    .map_err(|_| err("协议副本里出现了目录之外的路径"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, text);
            }
        }
        Ok(())
    }
    let mut out = ProtocolFiles::new();
    walk(dir, dir, &mut out)?;
    Ok(out)
}

/// A hash over `.autome/rules/`, so a usage number can say which rule set
/// produced it. Rules change between tasks; two numbers taken under different
/// rules are not the same measurement.
fn rules_hash(repo: &Path) -> Option<String> {
    let dir = repo.join(".autome/rules");
    let mut files = ProtocolFiles::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name()?.to_string_lossy().to_string();
        files.insert(name, std::fs::read_to_string(&path).ok()?);
    }
    Some(files.hash())
}

/// Everything the core puts into a task's directory before the first session
/// opens it: the user's attachments, and — for a meta task — the evidence it
/// is asked to propose changes from.
fn write_task_inputs(
    ctx: &mut Ctx,
    task: &TaskRecord,
    project: &Project,
    worktree: &Path,
) -> Result<()> {
    let mut paths: Vec<String> = Vec::new();

    if !task.attachments.is_empty() {
        let dest = worktree.join(task.doc_dir()).join("attachments");
        std::fs::create_dir_all(&dest).map_err(|e| err(format!("无法创建附件目录：{e}")))?;
        for source in &task.attachments {
            let name = Path::new(source)
                .file_name()
                .ok_or_else(|| err(format!("附件路径无效：{source}")))?;
            std::fs::copy(source, dest.join(name))
                .map_err(|e| err(format!("无法复制附件 {source}：{e}")))?;
        }
        paths.push(format!("{}/attachments", task.doc_dir()));
    }

    // A meta task cannot assemble its own evidence: a session cannot read the
    // store, cannot see other projects' tasks, and should not be trusted to
    // summarise its own history from memory.
    if crate::meta_store::is_protocol_project(ctx, project) {
        let tasks = crate::meta_store::collect(ctx)?;
        let deferred = crate::meta_store::deferred(ctx, &project.id)?;
        let contradicted = crate::meta_store::contradicted(ctx)?;
        for (rel, body) in crate::meta::inputs(&tasks, &deferred, &contradicted) {
            let full = worktree.join(task.doc_dir()).join(&rel);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| err(format!("无法创建 {}：{e}", parent.display())))?;
            }
            std::fs::write(&full, body)
                .map_err(|e| err(format!("无法写入 {}：{e}", full.display())))?;
        }
        paths.push(format!("{}/inputs", task.doc_dir()));
    }

    if paths.is_empty() {
        return Ok(());
    }
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    git::commit_paths(
        worktree,
        &refs,
        &format!("chore(autome): {} inputs", task.id),
    )?;
    Ok(())
}

/// Runs a core step and feeds the result back through the transition table.
fn run_core_step(ctx: &mut Ctx, task: &TaskRecord, node: Node) -> Result<()> {
    let project = ctx.store.get_project(&task.project_id)?;
    let repo = PathBuf::from(&project.path);
    let worktree = task_cwd(&project, &task.slug);

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
    let worktree = task_docs_root(project, &task.slug);

    if worktree.exists() {
        let source = worktree.join(task.doc_dir());
        if source.exists() {
            let dest = repo.join(task.archive_dir());
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
            // The protocol is resolved per project: one of them may be pinned
            // to an older version, and refreshing it to the newest would be
            // exactly the silent rule change pinning exists to prevent.
            let protocol = match crate::protocol::resolve_for_project(&ctx.autome_home, path) {
                Ok((_, files)) => files,
                Err(e) => {
                    report
                        .errors
                        .push(format!("{}：读取协议版本失败 {e}", p.display_name));
                    continue;
                }
            };
            match crate::init::init(path, &protocol) {
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

/// Starts a retro round on a task that has already stopped.
///
/// The user's button on the failure panel. A task that failed or was cancelled
/// is the most informative kind there is, and it is also the kind that never
/// reaches the retro node — so the only way its lessons get written is if
/// someone asks. Deliberately manual rather than automatic: a failed task has
/// often failed for a reason the user already understands, and spending a
/// session to have it explained back is not always worth it.
pub fn start_retro(ctx: &mut Ctx, task_id: &str) -> Result<()> {
    let task = ctx.store.get_task(task_id)?;
    if matches!(task.state, TaskState::Active { .. } | TaskState::Queued) {
        return Err(err("任务还在跑，复盘轮会在它停下时自己跑一次"));
    }
    if ctx.store.running_session(task_id)?.is_some() {
        return Err(err("这个任务已经有一个会话在跑了"));
    }
    let project = ctx.store.get_project(&task.project_id)?;
    let resolved = resolve_config(ctx, &project)?;
    if !resolved.is_enabled(Role::Retro) {
        return Err(err("复盘轮在这个项目里是关着的"));
    }
    let worktree = task_cwd(&project, &task.slug);
    if !worktree.exists() {
        // Cleanup removes the worktree after a merge, and the evidence the
        // retro round reads lives in it. Saying so beats starting a session
        // that finds an empty directory.
        return Err(err("任务的 worktree 已经清理掉了，复盘轮读不到证据"));
    }
    start_session(
        ctx,
        &task,
        &project,
        &resolved,
        SessionKind::Role { role: Role::Retro },
        None,
    )
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

    // Onboarding has no task and therefore no frozen copy; it runs under
    // whatever version the project resolves to right now, which is correct:
    // nothing it produces is attributed to a protocol version.
    let (_, protocol) = crate::protocol::resolve_for_project(&ctx.autome_home, &repo)
        .map_err(|e| err(e.to_string()))?;
    let prompt = launcher::build_prompt(&launcher::PromptSpec {
        kind: SessionKind::Onboarding,
        templates: &protocol,
        brief_path: "",
        slug: "onboarding",
        // Onboarding writes the project profile and AGENTS.md at the
        // repository root; it has no task and no document directory, and its
        // template asks for neither.
        doc_dir: "",
        design_rounds: 0,
        task_metrics: None,
        budget: None,
        request: "",
        skills: &[],
        inject: None,
        decisions: &[],
        attachments: &[],
        doc_refs: &[],
    })?;

    let session_id = crate::store::new_id("ses");
    let launched = launcher::launch(&launcher::LaunchSpec {
        session_id: &session_id,
        task_id: ONBOARDING_SESSION_KEY,
        cwd: &repo,
        repo: &repo,
        runtime: role_config.runtime,
        args: launcher::build_args(&role_config, &repo),
        prompt,
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

/// Which role a task's next session would use, for the panel's "current
/// session" card before the session actually exists.
pub fn next_role(state: &TaskState) -> Option<Role> {
    state.node().and_then(Node::role)
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_first_tick_recovers_and_later_ticks_do_not() {
        // `recover()` had no production caller: the desktop called
        // `scheduler.tick` at startup and nothing else, so a stale scaffold
        // was never refreshed and a session that ended while the app was
        // closed was only picked up incidentally. Recovery now rides the
        // first tick, and must not repeat on every one.
        let mut w = World::new("first-tick-recovers");
        let wrapper = w.repo.join(".autome/skill/run_session.sh");
        std::fs::write(
            &wrapper,
            "#!/bin/sh\n# autome-scaffold-version: 0\necho stale\n",
        )
        .unwrap();

        assert!(!w.ctx.recovered);
        tick(&mut w.ctx);
        assert!(w.ctx.recovered, "the first tick must have recovered");
        assert!(
            !std::fs::read_to_string(&wrapper)
                .unwrap()
                .contains("echo stale")
        );

        // A later tick must not redo it: write the stale file back and check
        // it is left alone.
        std::fs::write(
            &wrapper,
            "#!/bin/sh\n# autome-scaffold-version: 0\necho stale\n",
        )
        .unwrap();
        tick(&mut w.ctx);
        assert!(
            std::fs::read_to_string(&wrapper)
                .unwrap()
                .contains("echo stale"),
            "recovery ran twice"
        );
    }

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
            crate::init::init(&repo, crate::protocol::seed()).unwrap();
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
                kind: autome_domain::project::ProjectKind::Repo,
                members: Vec::new(),
                docs_repo: None,
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
                protocol_ref: None,
                rules_hash: None,
                metrics: None,
                doc_root: None,
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

    /// The prompt of the session a budget change starts must carry the new
    /// budget, not the one the task had when `advance` began.
    ///
    /// Both numbers come from the same `TaskRecord`, and it is read once at
    /// the top of `advance` — so a transition that computes a budget wrote it
    /// to the store and then handed `perform` the record from before. On the
    /// approval path `budget_n` was still `None`, which does not render as a
    /// stale number but as no budget line at all: implementation round 1 was
    /// never told what N is, on every task.
    ///
    /// The protocol puts N in the prompt precisely because a session cannot
    /// derive it, and a real run already showed what happens when the two
    /// sides disagree — the session declared the budget spent, the core
    /// scheduled another round anyway, and the round after that invented a
    /// user decision to explain the contradiction.
    #[test]
    fn the_first_implementation_round_is_told_the_budget_just_computed() {
        needs_git!();
        let mut w = World::new("budget-prompt");
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
        assert_eq!(w.ctx.store.get_task("T-1").unwrap().budget_n, Some(25));

        let dir = w.repo.join(".autome/output/sessions/T-1");
        let prompt = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "prompt"))
            .map(|p| std::fs::read_to_string(p).unwrap())
            .expect("the approval started a session, so a prompt was written");
        assert!(
            prompt.contains("N = 25"),
            "本轮 prompt 没有拿到刚算出的预算：\n{prompt}"
        );
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
    fn an_audit_closing_every_milestone_reaches_the_retro_round() {
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
        assert_eq!(w.state("T-1"), TaskState::Active { node: Node::Retro });
    }

    #[test]
    fn the_retro_round_hands_off_to_the_rebase() {
        needs_git!();
        let mut w = World::new("retro-done");
        w.add_task("T-1", "a");
        w.set_state("T-1", TaskState::Active { node: Node::Retro });
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
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
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
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
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
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
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
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
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
        // Reaping also advances the task, and the next session freezes the
        // protocol into the task directory — so HEAD legitimately moves. What
        // must not be there is a sweep commit: the round committed its own
        // work and there was nothing left over.
        let subjects = git::commit_subjects(&wt, &after_own_commit, "HEAD").unwrap();
        assert!(
            !subjects.iter().any(|s| s.contains("未提交的剩余改动")),
            "a clean worktree must not produce an empty sweep commit: {subjects:?}"
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
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
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

    // ---- a launch that fails is a failure the user can see -----------------

    /// Reproduces the real failure this behaviour was found through: `docs/`
    /// is an independent Git repository, so the outer repository holds it as a
    /// gitlink and `git add docs/<slug>/…` refuses. The first thing a session
    /// commits is its frozen protocol copy, so the task dies before its first
    /// round — with the worktree and the protocol copy already on disk, which
    /// is what made it look like it had started.
    fn nest_a_repository_under_docs(w: &mut World, task: &TaskRecord) {
        let wt = w.with_worktree(task);
        let docs = wt.join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        git::init(&docs, "main").unwrap();
        std::fs::write(docs.join("README.md"), "独立的文档仓库\n").unwrap();
        git::commit_paths(&docs, &["README.md"], "docs init").unwrap();
        // Exactly what the onboarding init commit did to the user's directory.
        git::commit_paths(&wt, &["docs"], "record docs as a gitlink").unwrap();
    }

    #[test]
    fn a_task_whose_session_cannot_start_fails_visibly_rather_than_sitting_active() {
        // The defect this pins: `advance` writes the new state *before*
        // performing the action, so an action that fails used to leave the task
        // Active at a node with no session — and nothing retries that.
        // `fill_slots` only looks at queued tasks, so it sat on the dashboard as
        // 运行中, forever, with the reason in a `tracing::warn` nobody reads.
        let mut w = World::new("launch-fails");
        let task = w.add_task("T-1", "t1");
        nest_a_repository_under_docs(&mut w, &task);

        let report = tick(&mut w.ctx);

        match w.state("T-1") {
            TaskState::Failed {
                reason: FailureReason::Config { detail },
                at,
            } => {
                assert_eq!(at, Node::Intake, "the node it failed at is named");
                assert!(
                    detail.contains("嵌套") && detail.contains("docs"),
                    "the reason names the nested repository and what to do: {detail}"
                );
            }
            other => panic!("expected a visible failure, got {other:?}"),
        }
        assert!(
            w.ctx.store.running_session("T-1").unwrap().is_none(),
            "no session was started, which is the whole problem"
        );
        assert!(
            !report.tasks_started.contains(&"T-1".to_string()),
            "a task that could not start was not started"
        );

        let kinds: Vec<String> = w
            .ctx
            .store
            .events_since(0, 100)
            .unwrap()
            .into_iter()
            .map(|(_, kind, _)| kind)
            .collect();
        assert!(
            kinds.iter().filter(|k| *k == "task.updated").count() >= 2,
            "the failure is in the event stream, not only in the row: {kinds:?}"
        );
    }

    #[test]
    fn a_failed_launch_does_not_block_the_rest_of_the_queue() {
        // The queue must survive one task it cannot start. `fill_slots` already
        // intended this — it catches the error per task — but a task left
        // Active also holds a slot it is not using, so the recorded failure is
        // what actually frees the queue.
        let mut w = World::new("launch-fails-queue");
        let broken = w.add_task("T-1", "t1");
        w.add_task("T-2", "t2");
        nest_a_repository_under_docs(&mut w, &broken);

        tick(&mut w.ctx);

        assert!(matches!(w.state("T-1"), TaskState::Failed { .. }));
        assert!(
            matches!(w.state("T-2"), TaskState::Active { node: Node::Intake }),
            "the healthy task behind it still started, got {:?}",
            w.state("T-2")
        );
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
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
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
    fn a_session_started_a_moment_ago_is_not_idle_however_absent_its_log_is() {
        // The bug: `log_idle_secs` reports a missing log as `u64::MAX`, and
        // between recording a session and the terminal starting the wrapper
        // the log does not exist. On a loaded machine that window was long
        // enough for the next tick to declare the session vanished and fail
        // the task before it had run at all. A log that is genuinely absent is
        // the whole point, so the path below is one that cannot exist.
        let started = now_iso();
        let idle = session_idle_secs(Path::new("/nonexistent/log"), &started);
        assert!(idle < autome_domain::session::VANISHED_AFTER_SECS, "{idle}");
    }

    #[test]
    fn a_session_started_long_ago_with_no_log_is_still_vanished() {
        let idle = session_idle_secs(Path::new("/nonexistent/log"), "2020-01-01T00:00:00Z");
        assert!(
            idle >= autome_domain::session::VANISHED_AFTER_SECS,
            "{idle}"
        );
    }

    #[test]
    fn an_unreadable_start_timestamp_does_not_make_a_session_look_fresh_forever() {
        assert_eq!(secs_since("not a timestamp"), None);
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
