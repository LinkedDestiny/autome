//! SQLite persistence. Technical design §13.
//!
//! The division of authority is the important part, and it is the opposite of
//! the 09-13 design's. There, SQLite was the sole authority over everything.
//! Here:
//!
//! - **Task progress lives in the repository.** The design document's status
//!   block is the source of truth for rounds, milestones, Backlog and
//!   disputes (§1, change 2). It travels with the branch, survives a machine
//!   change, and is what the agents actually write.
//! - **SQLite holds the registry, the index, the ledger and the user's own
//!   decisions** — the things that have no natural home in a repository, or
//!   that span repositories.
//!
//! When the two disagree, the file wins and an `integrity_warning` event is
//! recorded (§13). That is not a formality: it is the only way a user who
//! edits a design document by hand ends up with a system that agrees with what
//! they see.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use autome_domain::project::{AddDisposition, Onboarding, Project};
use autome_domain::role::Runtime;
use autome_domain::session::{Session, SessionKind, SessionLifecycle};
use autome_domain::task::{Disposition, PendingDecisions, TaskState};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

#[derive(Debug)]
pub enum StoreError {
    Sql(String),
    NotFound { what: String, id: String },
    Conflict { detail: String },
    Encoding { detail: String },
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sql(d) => write!(f, "数据库错误：{d}"),
            StoreError::NotFound { what, id } => write!(f, "找不到{what} {id}"),
            StoreError::Conflict { detail } => write!(f, "{detail}"),
            StoreError::Encoding { detail } => write!(f, "数据格式错误：{detail}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sql(e.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Encoding {
            detail: e.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// The schema version this binary writes. Bump it and add a `if version < N`
/// block in `migrate`; never edit an earlier block, because a database that
/// already ran it will not run it again.
pub const SCHEMA_VERSION: i64 = 2;

/// One task as the store holds it. Progress fields (rounds, milestones) are
/// deliberately absent — they come from the design document at read time.
/// `Eq` is deliberately absent: `TaskMetrics` carries a cost in dollars.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRecord {
    pub id: String,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub request: String,
    pub attachments: Vec<String>,
    pub doc_refs: Vec<String>,
    pub state: TaskState,
    /// The implementation budget, once the design has been approved.
    pub budget_n: Option<u32>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub merge_commit: Option<String>,
    pub archived_at: Option<String>,
    /// The protocol version this task is held to, in wire form. Set the first
    /// time a session starts, and never changed after — the frozen copy under
    /// `docs/<slug>/protocol/` is what the sessions read.
    pub protocol_ref: Option<String>,
    /// Hash of `.autome/rules/` when the task started.
    pub rules_hash: Option<String>,
    /// Aggregated once the task reaches a terminal state.
    pub metrics: Option<autome_domain::metrics::TaskMetrics>,
}

impl TaskRecord {
    pub fn branch(&self) -> String {
        autome_domain::project::Project::branch_name(&self.slug)
    }
    pub fn doc_dir(&self) -> String {
        autome_domain::project::Project::doc_dir(&self.slug)
    }
    /// `docs/<slug>/<slug>.md` — the design document the scheduler parses.
    pub fn design_doc(&self) -> String {
        format!("{}/{}.md", self.doc_dir(), self.slug)
    }
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

/// One Backlog item or dispute, plus the user's disposition of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRecord {
    pub task_id: String,
    /// `backlog` or `dispute`.
    pub kind: String,
    /// The stable id from the design document, e.g. `B-01`, `D3-P02`.
    pub item_id: String,
    pub text: String,
    pub disposition: Disposition,
    /// Free text for a dispute the user ruled on in their own words.
    pub ruling: Option<String>,
    /// Set once a transition has consumed this decision, so it is applied
    /// exactly once (design §5.3's `consumes_decisions`).
    pub consumed_at: Option<String>,
}

/// `(key, domain, proposal, evidence, state)` for one rule proposal.
pub type RuleProposalRow = (String, String, String, Vec<String>, String);

pub struct Store {
    conn: Connection,
    db_path: PathBuf,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Store> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path)?;
        // WAL keeps a long-running read (the project list) from blocking the
        // scheduler's writes; `foreign_keys` makes the task→project link real
        // rather than advisory.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        let store = Store {
            conn,
            db_path: path,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Store> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Store {
            conn,
            db_path: PathBuf::from(":memory:"),
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Schema migrations, applied in order and recorded by `user_version`.
    /// Deliberately additive: 2.0 does not import 1.x data, but it does have
    /// to survive its own upgrades without losing a user's project registry.
    fn migrate(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(0);
        self.refuse_a_database_that_is_not_ours(version)?;

        if version < 1 {
            // Every block below is one transaction, `user_version` included.
            // A crash between creating the tables and recording the version
            // would otherwise leave a database that is ours but cannot say
            // so — indistinguishable, to the guard above, from a stranger's.
            self.conn.execute_batch(
                r#"
                BEGIN;
                CREATE TABLE projects (
                    id              TEXT PRIMARY KEY,
                    path            TEXT NOT NULL UNIQUE,
                    display_name    TEXT NOT NULL,
                    default_branch  TEXT NOT NULL,
                    parallel_limit  INTEGER NOT NULL,
                    onboarding      TEXT NOT NULL,
                    disposition     TEXT NOT NULL,
                    added_at        TEXT NOT NULL,
                    removed_at      TEXT
                );

                CREATE TABLE tasks (
                    id            TEXT PRIMARY KEY,
                    project_id    TEXT NOT NULL REFERENCES projects(id),
                    slug          TEXT NOT NULL,
                    title         TEXT NOT NULL,
                    request       TEXT NOT NULL,
                    attachments   TEXT NOT NULL,
                    doc_refs      TEXT NOT NULL,
                    state         TEXT NOT NULL,
                    budget_n      INTEGER,
                    created_at    TEXT NOT NULL,
                    completed_at  TEXT,
                    merge_commit  TEXT,
                    archived_at   TEXT,
                    UNIQUE (project_id, slug)
                );
                CREATE INDEX tasks_by_project ON tasks (project_id, created_at);

                CREATE TABLE sessions (
                    id          TEXT PRIMARY KEY,
                    task_id     TEXT NOT NULL REFERENCES tasks(id),
                    kind        TEXT NOT NULL,
                    runtime     TEXT NOT NULL,
                    model       TEXT NOT NULL,
                    effort      TEXT,
                    skills      TEXT NOT NULL,
                    round       INTEGER NOT NULL,
                    started_at  TEXT NOT NULL,
                    ended_at    TEXT,
                    lifecycle   TEXT NOT NULL,
                    log_path    TEXT NOT NULL,
                    pid         INTEGER
                );
                CREATE INDEX sessions_by_task ON sessions (task_id, started_at);

                CREATE TABLE decisions (
                    task_id     TEXT NOT NULL REFERENCES tasks(id),
                    kind        TEXT NOT NULL,
                    item_id     TEXT NOT NULL,
                    text        TEXT NOT NULL,
                    disposition TEXT NOT NULL,
                    ruling      TEXT,
                    consumed_at TEXT,
                    PRIMARY KEY (task_id, kind, item_id)
                );

                CREATE TABLE events (
                    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_id   TEXT NOT NULL,
                    kind       TEXT NOT NULL,
                    subject_id TEXT NOT NULL,
                    payload    TEXT NOT NULL,
                    at         TEXT NOT NULL
                );
                CREATE INDEX events_by_subject ON events (subject_id, seq);
                PRAGMA user_version = 1;
                COMMIT;
                "#,
            )?;
        }

        // v2: the observation layer. Until now the ledger recorded that a
        // session happened and what it exited with, and nothing about what it
        // cost or produced — so "did that protocol change help" had no answer
        // and the whole self-improvement idea was decorative. The CLIs were
        // already writing everything needed; nobody was reading it.
        //
        // Session usage goes in real columns rather than a JSON blob because
        // the version page averages them, and a database that can do the
        // averaging is one less place to get it wrong.
        if version < 2 {
            self.conn.execute_batch(
                r#"
                BEGIN;
                ALTER TABLE tasks ADD COLUMN protocol_ref TEXT;
                ALTER TABLE tasks ADD COLUMN rules_hash TEXT;
                ALTER TABLE tasks ADD COLUMN metrics TEXT;

                ALTER TABLE sessions ADD COLUMN protocol_ref TEXT;
                ALTER TABLE sessions ADD COLUMN rules_hash TEXT;
                ALTER TABLE sessions ADD COLUMN input_tokens INTEGER;
                ALTER TABLE sessions ADD COLUMN cache_read_tokens INTEGER;
                ALTER TABLE sessions ADD COLUMN cache_write_tokens INTEGER;
                ALTER TABLE sessions ADD COLUMN output_tokens INTEGER;
                ALTER TABLE sessions ADD COLUMN cost_usd REAL;
                ALTER TABLE sessions ADD COLUMN turns INTEGER;
                ALTER TABLE sessions ADD COLUMN duration_ms INTEGER;
                ALTER TABLE sessions ADD COLUMN mean_request_input INTEGER;
                ALTER TABLE sessions ADD COLUMN design_doc_bytes INTEGER;
                ALTER TABLE sessions ADD COLUMN evidence_files INTEGER;

                -- One row per lesson key a project has seen twice. The user
                -- approves or dismisses; the row is what stops the panel
                -- proposing the same rule every tick.
                CREATE TABLE rule_proposals (
                    project_id  TEXT NOT NULL REFERENCES projects(id),
                    lesson_key  TEXT NOT NULL,
                    domain      TEXT NOT NULL,
                    proposal    TEXT NOT NULL,
                    evidence    TEXT NOT NULL,
                    state       TEXT NOT NULL,
                    decided_at  TEXT,
                    PRIMARY KEY (project_id, lesson_key)
                );

                -- A removal experiment (plan §E3). A rule is never retired
                -- because it stopped being needed — the rule is why it stopped
                -- — so removing one is an experiment with a prediction and a
                -- deadline.
                CREATE TABLE rule_experiments (
                    id          TEXT PRIMARY KEY,
                    project_id  TEXT NOT NULL REFERENCES projects(id),
                    rule_file   TEXT NOT NULL,
                    removed_at  TEXT NOT NULL,
                    metric      TEXT NOT NULL,
                    baseline    REAL NOT NULL,
                    horizon     INTEGER NOT NULL,
                    body        TEXT NOT NULL,
                    state       TEXT NOT NULL,
                    outcome     TEXT
                );
                PRAGMA user_version = 2;
                COMMIT;
                "#,
            )?;
        }
        Ok(())
    }

    /// The gate in front of `migrate`, and the reason it exists.
    ///
    /// `migrate` decides what to do from `user_version` alone. That is right
    /// for our own databases and wrong for every other file that can end up
    /// at the same path: an earlier prototype's, a newer build's, or one a
    /// user copied there by hand. A stranger's database reads as version 0,
    /// so the v1 block runs against tables that already exist and SQLite
    /// answers `table projects already exists` — a sentence about SQL, thrown
    /// from a process that then exits, to a shell that restarts it and gets
    /// the same sentence a second later, forever.
    ///
    /// So the two cases are named here instead. Neither is recoverable by
    /// retrying, and both say which file and what to do about it.
    fn refuse_a_database_that_is_not_ours(&self, version: i64) -> Result<()> {
        if version == 0 {
            let occupant: Option<String> = self
                .conn
                .query_row(
                    "SELECT name FROM sqlite_master
                     WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
                     ORDER BY name LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(table) = occupant {
                return Err(StoreError::Conflict {
                    detail: format!(
                        "{} 不是 Autome 2.0 的数据库：它已经有 `{table}` 表，却没有记录 schema 版本。\
                         把这个文件改名移开（连同 -wal 和 -shm），重新启动会新建一个空库。",
                        self.db_path.display()
                    ),
                });
            }
        }
        if version > SCHEMA_VERSION {
            return Err(StoreError::Conflict {
                detail: format!(
                    "{} 的 schema 版本是 {version}，这个 automed 只认到 {SCHEMA_VERSION}。\
                     它是更新的版本写的：用那个版本打开，或者把文件改名移开后重新启动。",
                    self.db_path.display()
                ),
            });
        }
        Ok(())
    }

    // -----------------------------------------------------------------
    // Events
    // -----------------------------------------------------------------

    /// Appends to the ledger and returns the assigned sequence number. Every
    /// state change goes through here, so `events` is a complete, ordered
    /// account of what the core did — which is what the UI resyncs against
    /// after a reconnect.
    pub fn append_event(&self, kind: &str, subject_id: &str, payload: Value) -> Result<u64> {
        self.conn.execute(
            "INSERT INTO events (event_id, kind, subject_id, payload, at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                new_id("evt"),
                kind,
                subject_id,
                payload.to_string(),
                now_iso()
            ],
        )?;
        Ok(self.conn.last_insert_rowid() as u64)
    }

    /// The most recent event of one kind about one subject.
    ///
    /// Used for the running comparisons the metrics need — "what did the
    /// milestone table look like before this session" — which have to be made
    /// as they happen: the final document shows a milestone as open and says
    /// nothing about it having once been closed.
    pub fn last_event(&self, subject_id: &str, kind: &str) -> Result<Option<Value>> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM events WHERE subject_id = ?1 AND kind = ?2
                 ORDER BY seq DESC LIMIT 1",
                params![subject_id, kind],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.as_deref().map(serde_json::from_str).transpose()?)
    }

    pub fn count_events(&self, subject_id: &str, kind: &str) -> Result<u32> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE subject_id = ?1 AND kind = ?2",
            params![subject_id, kind],
            |r| r.get(0),
        )?;
        Ok(n.max(0) as u32)
    }

    pub fn latest_seq(&self) -> Result<u64> {
        let seq: i64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |r| r.get(0))?;
        Ok(seq as u64)
    }

    /// Events after `after_seq`, oldest first. The Renderer's resync path.
    pub fn events_since(&self, after_seq: u64, limit: u32) -> Result<Vec<(u64, String, Value)>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, kind, payload FROM events WHERE seq > ?1 ORDER BY seq LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_seq as i64, limit as i64], |r| {
            let seq: i64 = r.get(0)?;
            let kind: String = r.get(1)?;
            let payload: String = r.get(2)?;
            Ok((seq as u64, kind, payload))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, kind, payload) = row?;
            out.push((
                seq,
                kind,
                serde_json::from_str(&payload).unwrap_or(Value::Null),
            ));
        }
        Ok(out)
    }

    // -----------------------------------------------------------------
    // Projects
    // -----------------------------------------------------------------

    pub fn insert_project(&self, project: &Project) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO projects (id, path, display_name, default_branch, parallel_limit,
                                       onboarding, disposition, added_at, removed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    project.id,
                    project.path,
                    project.display_name,
                    project.default_branch,
                    project.parallel_limit as i64,
                    serde_json::to_string(&project.onboarding)?,
                    serde_json::to_string(&project.disposition)?,
                    project.added_at,
                    project.removed_at,
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _)
                    if f.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    StoreError::Conflict {
                        detail: "该目录已经是一个项目".into(),
                    }
                }
                other => StoreError::Sql(other.to_string()),
            })?;
        Ok(())
    }

    pub fn get_project(&self, id: &str) -> Result<Project> {
        self.conn
            .query_row(
                "SELECT id, path, display_name, default_branch, parallel_limit, onboarding,
                        disposition, added_at, removed_at
                 FROM projects WHERE id = ?1",
                params![id],
                row_to_project,
            )
            .optional()?
            .ok_or_else(|| StoreError::NotFound {
                what: "项目".into(),
                id: id.to_string(),
            })?
            .map_err(Into::into)
    }

    /// Looks a project up by its canonical path, so "add" can tell an
    /// already-registered directory from a new one.
    pub fn project_by_path(&self, path: &str) -> Result<Option<Project>> {
        match self
            .conn
            .query_row(
                "SELECT id, path, display_name, default_branch, parallel_limit, onboarding,
                        disposition, added_at, removed_at
                 FROM projects WHERE path = ?1",
                params![path],
                row_to_project,
            )
            .optional()?
        {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Active projects, newest first.
    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, display_name, default_branch, parallel_limit, onboarding,
                    disposition, added_at, removed_at
             FROM projects WHERE removed_at IS NULL ORDER BY added_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_project)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    pub fn update_project_onboarding(&self, id: &str, onboarding: Onboarding) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE projects SET onboarding = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(&onboarding)?],
        )?;
        self.require_one(n, "项目", id)
    }

    pub fn update_project_parallel(&self, id: &str, limit: u32) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE projects SET parallel_limit = ?2 WHERE id = ?1",
            params![id, limit as i64],
        )?;
        self.require_one(n, "项目", id)
    }

    /// Soft-removes a project. The directory is never touched (P-08).
    pub fn remove_project(&self, id: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE projects SET removed_at = ?2 WHERE id = ?1 AND removed_at IS NULL",
            params![id, now_iso()],
        )?;
        self.require_one(n, "项目", id)
    }

    // -----------------------------------------------------------------
    // Tasks
    // -----------------------------------------------------------------

    pub fn insert_task(&self, task: &TaskRecord) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO tasks (id, project_id, slug, title, request, attachments, doc_refs,
                                    state, budget_n, created_at, completed_at, merge_commit,
                                    archived_at, protocol_ref, rules_hash, metrics)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
                params![
                    task.id,
                    task.project_id,
                    task.slug,
                    task.title,
                    task.request,
                    serde_json::to_string(&task.attachments)?,
                    serde_json::to_string(&task.doc_refs)?,
                    serde_json::to_string(&task.state)?,
                    task.budget_n.map(|v| v as i64),
                    task.created_at,
                    task.completed_at,
                    task.merge_commit,
                    task.archived_at,
                    task.protocol_ref,
                    task.rules_hash,
                    task.metrics
                        .as_ref()
                        .map(serde_json::to_string)
                        .transpose()?,
                ],
            )
            .map_err(|e| match &e {
                // Two different constraints can fire here and they mean
                // opposite things: a duplicate slug is a name clash the caller
                // can retry around, a foreign-key failure means the project id
                // does not exist at all. Reporting the second as the first
                // sent a real bug off looking for a task that was never there.
                rusqlite::Error::SqliteFailure(f, msg)
                    if f.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    let text = msg.clone().unwrap_or_default();
                    if text.contains("FOREIGN KEY") {
                        StoreError::Sql(format!(
                            "任务 {} 引用的项目 {} 不存在",
                            task.id, task.project_id
                        ))
                    } else {
                        StoreError::Conflict {
                            detail: format!("项目内已存在 slug `{}`", task.slug),
                        }
                    }
                }
                other => StoreError::Sql(other.to_string()),
            })?;
        Ok(())
    }

    pub fn get_task(&self, id: &str) -> Result<TaskRecord> {
        self.conn
            .query_row(TASK_SELECT, params![id], row_to_task)
            .optional()?
            .ok_or_else(|| StoreError::NotFound {
                what: "任务".into(),
                id: id.to_string(),
            })?
            .map_err(Into::into)
    }

    /// A project's tasks, oldest first — the order the queue runs in.
    pub fn list_tasks(&self, project_id: &str) -> Result<Vec<TaskRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "{} WHERE project_id = ?1 ORDER BY created_at",
            TASK_SELECT_BASE
        ))?;
        let rows = stmt.query_map(params![project_id], row_to_task)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Every task in every project that is not finished — what the dashboard
    /// and the scheduler both want.
    pub fn list_unfinished(&self) -> Result<Vec<TaskRecord>> {
        let mut out = Vec::new();
        let mut stmt = self
            .conn
            .prepare(&format!("{} ORDER BY created_at", TASK_SELECT_BASE))?;
        let rows = stmt.query_map([], row_to_task)?;
        for row in rows {
            let task = row??;
            if !task.state.is_terminal() {
                out.push(task);
            }
        }
        Ok(out)
    }

    pub fn set_task_state(&self, id: &str, state: &TaskState) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE tasks SET state = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(state)?],
        )?;
        self.require_one(n, "任务", id)
    }

    pub fn set_task_budget(&self, id: &str, budget_n: u32) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE tasks SET budget_n = ?2 WHERE id = ?1",
            params![id, budget_n as i64],
        )?;
        self.require_one(n, "任务", id)
    }

    pub fn complete_task(&self, id: &str, merge_commit: Option<&str>) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE tasks SET completed_at = ?2, merge_commit = ?3 WHERE id = ?1",
            params![id, now_iso(), merge_commit],
        )?;
        self.require_one(n, "任务", id)
    }

    /// Records which protocol version a task froze. Written once, on the first
    /// session; a second write would mean the rules changed under a running
    /// task, which is the thing freezing exists to prevent.
    pub fn set_task_protocol_ref(&self, id: &str, wire: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE tasks SET protocol_ref = ?2 WHERE id = ?1 AND protocol_ref IS NULL",
            params![id, wire],
        )?;
        // Zero rows means it was already set, which is fine and expected on
        // every session after the first.
        let _ = n;
        Ok(())
    }

    pub fn task_metrics(&self, id: &str) -> Result<Option<autome_domain::metrics::TaskMetrics>> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT metrics FROM tasks WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok(raw.as_deref().map(serde_json::from_str).transpose()?)
    }

    pub fn set_task_metrics(
        &self,
        id: &str,
        metrics: &autome_domain::metrics::TaskMetrics,
    ) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE tasks SET metrics = ?2 WHERE id = ?1",
            params![id, serde_json::to_string(metrics)?],
        )?;
        self.require_one(n, "任务", id)
    }

    /// Every terminal task of a project that recorded metrics, oldest first.
    /// The version page and the meta task's `inputs/` both read this.
    pub fn tasks_with_metrics(
        &self,
        project_id: &str,
    ) -> Result<Vec<(TaskRecord, autome_domain::metrics::TaskMetrics)>> {
        Ok(self
            .list_tasks(project_id)?
            .into_iter()
            .filter_map(|t| t.metrics.clone().map(|m| (t, m)))
            .collect())
    }

    // -----------------------------------------------------------------
    // Curation: lessons becoming rules, and rules being removed again
    // -----------------------------------------------------------------

    /// Records a proposal the panel is showing. Idempotent: the aggregation
    /// runs on every tick and must not re-ask a question the user answered.
    pub fn upsert_rule_proposal(
        &self,
        project_id: &str,
        key: &str,
        domain: &str,
        proposal: &str,
        evidence: &[String],
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO rule_proposals (project_id, lesson_key, domain, proposal, evidence, state)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending')
             ON CONFLICT (project_id, lesson_key) DO UPDATE SET evidence = excluded.evidence",
            params![
                project_id,
                key,
                domain,
                proposal,
                serde_json::to_string(evidence)?
            ],
        )?;
        Ok(())
    }

    /// One row per proposal, in [`RuleProposalRow`]'s order.
    pub fn list_rule_proposals(&self, project_id: &str) -> Result<Vec<RuleProposalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT lesson_key, domain, proposal, evidence, state FROM rule_proposals
             WHERE project_id = ?1 ORDER BY lesson_key",
        )?;
        let rows = stmt.query_map(params![project_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (key, domain, proposal, evidence, state) = row?;
            out.push((
                key,
                domain,
                proposal,
                serde_json::from_str(&evidence).unwrap_or_default(),
                state,
            ));
        }
        Ok(out)
    }

    pub fn set_rule_proposal_state(&self, project_id: &str, key: &str, state: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE rule_proposals SET state = ?3, decided_at = ?4
             WHERE project_id = ?1 AND lesson_key = ?2",
            params![project_id, key, state, now_iso()],
        )?;
        self.require_one(n, "入规建议", key)
    }

    /// Keys the user has already answered, so they are not offered again.
    pub fn answered_rule_proposals(&self, project_id: &str) -> Result<Vec<String>> {
        Ok(self
            .list_rule_proposals(project_id)?
            .into_iter()
            .filter(|(_, _, _, _, state)| state != "pending")
            .map(|(key, ..)| key)
            .collect())
    }

    /// Starts a removal experiment. `baseline` is the metric's mean before the
    /// rule came out, which is the only thing the later verdict compares to.
    #[allow(clippy::too_many_arguments)]
    pub fn start_rule_experiment(
        &self,
        project_id: &str,
        rule_file: &str,
        body: &str,
        metric: &str,
        baseline: f64,
        horizon: u32,
    ) -> Result<String> {
        let id = new_id("exp");
        self.conn.execute(
            "INSERT INTO rule_experiments
                (id, project_id, rule_file, removed_at, metric, baseline, horizon, body, state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'running')",
            params![
                id,
                project_id,
                rule_file,
                now_iso(),
                metric,
                baseline,
                horizon as i64,
                body
            ],
        )?;
        Ok(id)
    }

    /// `(id, rule_file, body, metric, baseline, horizon, removed_at, state, outcome)`.
    #[allow(clippy::type_complexity)]
    pub fn list_rule_experiments(
        &self,
        project_id: &str,
    ) -> Result<
        Vec<(
            String,
            String,
            String,
            String,
            f64,
            u32,
            String,
            String,
            Option<String>,
        )>,
    > {
        let mut stmt = self.conn.prepare(
            "SELECT id, rule_file, body, metric, baseline, horizon, removed_at, state, outcome
             FROM rule_experiments WHERE project_id = ?1 ORDER BY removed_at",
        )?;
        let rows = stmt.query_map(params![project_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, i64>(5)? as u32,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn finish_rule_experiment(&self, id: &str, state: &str, outcome: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE rule_experiments SET state = ?2, outcome = ?3 WHERE id = ?1",
            params![id, state, outcome],
        )?;
        self.require_one(n, "移除实验", id)
    }

    /// Tasks that finished after a timestamp, in order — the window a removal
    /// experiment is judged over.
    pub fn tasks_completed_after(
        &self,
        project_id: &str,
        after: &str,
    ) -> Result<Vec<autome_domain::metrics::TaskMetrics>> {
        Ok(self
            .list_tasks(project_id)?
            .into_iter()
            .filter(|t| t.completed_at.as_deref().is_some_and(|c| c > after))
            .filter_map(|t| t.metrics)
            .collect())
    }

    pub fn set_task_archived(&self, id: &str, archived: bool) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE tasks SET archived_at = ?2 WHERE id = ?1",
            params![id, if archived { Some(now_iso()) } else { None }],
        )?;
        self.require_one(n, "任务", id)
    }

    /// The next task id: `T-1`, `T-2`, … Never reused, including after a
    /// cancel, so an id in a log or a session directory always refers to the
    /// same piece of work.
    ///
    /// **Numbered across all projects, not within one.** It used to count per
    /// project, which reads better — every project starts at T-1 — and was
    /// wrong: `tasks.id` is the primary key, and `sessions` and `decisions`
    /// both reference it. Two projects each numbering from T-1 collide on the
    /// second project's first task. Nothing caught it because everything that
    /// exercised task creation used a single project; the first thing that
    /// used two was the protocol repository being registered as one, and it
    /// failed at once — with a message about a duplicate slug, because the
    /// insert mapped every constraint violation to the same complaint.
    ///
    /// The alternative was a composite primary key, which means rebuilding
    /// three tables to keep a cosmetic property.
    pub fn next_task_id(&self, _project_id: &str) -> Result<String> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))?;
        // Count alone is not enough once a task has been deleted; take the
        // highest suffix that exists as well.
        let mut stmt = self.conn.prepare("SELECT id FROM tasks")?;
        let mut max = 0i64;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for row in rows {
            let id = row?;
            if let Some(n) = id.strip_prefix("T-").and_then(|s| s.parse::<i64>().ok()) {
                max = max.max(n);
            }
        }
        Ok(format!("T-{}", max.max(count) + 1))
    }

    /// How many of a project's parallel slots are in use (design §8).
    pub fn slots_in_use(&self, project_id: &str) -> Result<u32> {
        Ok(self
            .list_tasks(project_id)?
            .iter()
            .filter(|t| t.state.occupies_slot())
            .count() as u32)
    }

    /// The queued tasks of a project, in submission order.
    pub fn queued_tasks(&self, project_id: &str) -> Result<Vec<TaskRecord>> {
        Ok(self
            .list_tasks(project_id)?
            .into_iter()
            .filter(|t| matches!(t.state, TaskState::Queued))
            .collect())
    }

    // -----------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------

    pub fn insert_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, task_id, kind, runtime, model, effort, skills, round,
                                   started_at, ended_at, lifecycle, log_path, pid,
                                   protocol_ref, rules_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                session.id,
                session.task_id,
                serde_json::to_string(&session.kind)?,
                session.runtime.as_str(),
                session.model,
                session.effort,
                serde_json::to_string(&session.skills)?,
                session.round as i64,
                session.started_at,
                session.ended_at,
                serde_json::to_string(&session.lifecycle)?,
                session.log_path,
                session.pid,
                session.protocol_ref,
                session.rules_hash,
            ],
        )?;
        Ok(())
    }

    pub fn finish_session(
        &self,
        id: &str,
        lifecycle: &SessionLifecycle,
        ended_at: &str,
    ) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE sessions SET lifecycle = ?2, ended_at = ?3 WHERE id = ?1",
            params![id, serde_json::to_string(lifecycle)?, ended_at],
        )?;
        self.require_one(n, "会话", id)
    }

    /// Records what a session cost, read out of the CLI's own stream when the
    /// session is reaped.
    pub fn set_session_metrics(
        &self,
        id: &str,
        m: &autome_domain::metrics::SessionMetrics,
    ) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE sessions SET input_tokens = ?2, cache_read_tokens = ?3,
                    cache_write_tokens = ?4, output_tokens = ?5, cost_usd = ?6,
                    turns = ?7, duration_ms = ?8, mean_request_input = ?9,
                    design_doc_bytes = ?10, evidence_files = ?11
             WHERE id = ?1",
            params![
                id,
                m.input_tokens.map(|v| v as i64),
                m.cache_read_tokens.map(|v| v as i64),
                m.cache_write_tokens.map(|v| v as i64),
                m.output_tokens.map(|v| v as i64),
                m.cost_usd,
                m.turns.map(|v| v as i64),
                m.duration_ms.map(|v| v as i64),
                m.mean_request_input.map(|v| v as i64),
                m.design_doc_bytes.map(|v| v as i64),
                m.evidence_files.map(|v| v as i64),
            ],
        )?;
        self.require_one(n, "会话", id)
    }

    /// A task's sessions, newest first — the order the history panel shows.
    pub fn list_sessions(&self, task_id: &str) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SESSION_SELECT_BASE} WHERE task_id = ?1 ORDER BY started_at DESC"
        ))?;
        let rows = stmt.query_map(params![task_id], row_to_session)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// The session currently running for a task, if any. A task has at most
    /// one (design §8), so more than one running row is a bug worth surfacing
    /// rather than silently picking from.
    pub fn running_session(&self, task_id: &str) -> Result<Option<Session>> {
        Ok(self
            .list_sessions(task_id)?
            .into_iter()
            .find(|s| s.is_running()))
    }

    /// Every running session across all tasks — the set the watcher polls.
    pub fn all_running_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self
            .conn
            .prepare(&format!("{SESSION_SELECT_BASE} ORDER BY started_at"))?;
        let rows = stmt.query_map([], row_to_session)?;
        let mut out = Vec::new();
        for row in rows {
            let s = row??;
            if s.is_running() {
                out.push(s);
            }
        }
        Ok(out)
    }

    /// The round number for a role's next session: one more than the highest
    /// round already recorded for it.
    pub fn next_round(&self, task_id: &str, kind: &SessionKind) -> Result<u32> {
        let target = serde_json::to_string(kind)?;
        let max: Option<i64> = self.conn.query_row(
            "SELECT MAX(round) FROM sessions WHERE task_id = ?1 AND kind = ?2",
            params![task_id, target],
            |r| r.get(0),
        )?;
        Ok(max.unwrap_or(0) as u32 + 1)
    }

    // -----------------------------------------------------------------
    // Decisions
    // -----------------------------------------------------------------

    /// Reconciles the decision table with what the design document now says.
    ///
    /// Items the document has dropped are removed *unless* the user has
    /// already taken a position on them — losing a ruling because an agent
    /// reworded a section would be worse than keeping a stale row.
    pub fn sync_decisions(
        &self,
        task_id: &str,
        backlog: &[(String, String)],
        disputes: &[(String, String)],
    ) -> Result<()> {
        let tx = unsafe { &*(&self.conn as *const Connection) };
        for (kind, items) in [("backlog", backlog), ("dispute", disputes)] {
            for (item_id, text) in items {
                tx.execute(
                    "INSERT INTO decisions (task_id, kind, item_id, text, disposition)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT (task_id, kind, item_id) DO UPDATE SET text = excluded.text",
                    params![
                        task_id,
                        kind,
                        item_id,
                        text,
                        serde_json::to_string(&Disposition::None)?
                    ],
                )?;
            }
            let keep: Vec<String> = items.iter().map(|(id, _)| id.clone()).collect();
            let placeholders = if keep.is_empty() {
                "''".to_string()
            } else {
                keep.iter().map(|_| "?").collect::<Vec<_>>().join(",")
            };
            let sql = format!(
                "DELETE FROM decisions WHERE task_id = ?1 AND kind = ?2
                 AND disposition = ?3 AND consumed_at IS NULL
                 AND item_id NOT IN ({placeholders})"
            );
            let mut stmt = tx.prepare(&sql)?;
            let none = serde_json::to_string(&Disposition::None)?;
            let mut binds: Vec<&dyn rusqlite::ToSql> = vec![&task_id, &kind, &none];
            for k in &keep {
                binds.push(k);
            }
            stmt.execute(rusqlite::params_from_iter(binds.into_iter()))?;
        }
        Ok(())
    }

    pub fn set_disposition(
        &self,
        task_id: &str,
        kind: &str,
        item_id: &str,
        disposition: Disposition,
        ruling: Option<&str>,
    ) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE decisions SET disposition = ?4, ruling = ?5
             WHERE task_id = ?1 AND kind = ?2 AND item_id = ?3 AND consumed_at IS NULL",
            params![
                task_id,
                kind,
                item_id,
                serde_json::to_string(&disposition)?,
                ruling
            ],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound {
                what: "待决条目".into(),
                id: item_id.to_string(),
            });
        }
        Ok(())
    }

    pub fn list_decisions(&self, task_id: &str) -> Result<Vec<DecisionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT task_id, kind, item_id, text, disposition, ruling, consumed_at
             FROM decisions WHERE task_id = ?1 ORDER BY kind, item_id",
        )?;
        let rows = stmt.query_map(params![task_id], |r| {
            let disposition: String = r.get(4)?;
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                disposition,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (task_id, kind, item_id, text, disposition, ruling, consumed_at) = row?;
            out.push(DecisionRecord {
                task_id,
                kind,
                item_id,
                text,
                disposition: serde_json::from_str(&disposition)?,
                ruling,
                consumed_at,
            });
        }
        Ok(out)
    }

    /// The counts the transition table needs (design §5.3).
    pub fn pending_decisions(&self, task_id: &str) -> Result<PendingDecisions> {
        let decisions = self.list_decisions(task_id)?;
        let open = decisions.iter().filter(|d| d.consumed_at.is_none());
        Ok(PendingDecisions {
            included: open
                .clone()
                .filter(|d| d.disposition == Disposition::Include)
                .count() as u32,
            ruled: open.filter(|d| d.disposition == Disposition::Ruled).count() as u32,
        })
    }

    /// Marks every decided-but-unconsumed item as consumed, so a transition
    /// applies them exactly once.
    pub fn consume_decisions(&self, task_id: &str) -> Result<u32> {
        let none = serde_json::to_string(&Disposition::None)?;
        let n = self.conn.execute(
            "UPDATE decisions SET consumed_at = ?2
             WHERE task_id = ?1 AND consumed_at IS NULL AND disposition != ?3",
            params![task_id, now_iso(), none],
        )?;
        Ok(n as u32)
    }

    /// The decided items a prompt should carry, for the injection the
    /// transition asked for.
    pub fn decisions_for_prompt(&self, task_id: &str) -> Result<Vec<DecisionRecord>> {
        Ok(self
            .list_decisions(task_id)?
            .into_iter()
            .filter(|d| d.consumed_at.is_none() && d.disposition != Disposition::None)
            .collect())
    }

    fn require_one(&self, affected: usize, what: &str, id: &str) -> Result<()> {
        if affected == 0 {
            Err(StoreError::NotFound {
                what: what.to_string(),
                id: id.to_string(),
            })
        } else {
            Ok(())
        }
    }
}

const TASK_SELECT_BASE: &str = "SELECT id, project_id, slug, title, request, attachments, doc_refs,
            state, budget_n, created_at, completed_at, merge_commit, archived_at,
            protocol_ref, rules_hash, metrics FROM tasks";

const TASK_SELECT: &str = "SELECT id, project_id, slug, title, request, attachments, doc_refs,
            state, budget_n, created_at, completed_at, merge_commit, archived_at,
            protocol_ref, rules_hash, metrics
     FROM tasks WHERE id = ?1";

const SESSION_SELECT_BASE: &str = "SELECT id, task_id, kind, runtime, model, effort, skills, round,
            started_at, ended_at, lifecycle, log_path, pid,
            protocol_ref, rules_hash, input_tokens, cache_read_tokens, cache_write_tokens,
            output_tokens, cost_usd, turns, duration_ms, mean_request_input,
            design_doc_bytes, evidence_files FROM sessions";

type RowResult<T> = rusqlite::Result<std::result::Result<T, serde_json::Error>>;

fn row_to_project(r: &rusqlite::Row<'_>) -> RowResult<Project> {
    let onboarding: String = r.get(5)?;
    let disposition: String = r.get(6)?;
    let parallel: i64 = r.get(4)?;
    Ok((|| {
        Ok(Project {
            id: r.get(0)?,
            path: r.get(1)?,
            display_name: r.get(2)?,
            default_branch: r.get(3)?,
            parallel_limit: parallel as u32,
            onboarding: serde_json::from_str::<Onboarding>(&onboarding)?,
            disposition: serde_json::from_str::<AddDisposition>(&disposition)?,
            added_at: r.get(7)?,
            removed_at: r.get(8)?,
        })
    })()
    .map_err(|e: Box<dyn std::error::Error>| {
        serde_json::Error::io(std::io::Error::other(e.to_string()))
    }))
}

fn row_to_task(r: &rusqlite::Row<'_>) -> RowResult<TaskRecord> {
    let attachments: String = r.get(5)?;
    let doc_refs: String = r.get(6)?;
    let state: String = r.get(7)?;
    let budget: Option<i64> = r.get(8)?;
    let base = (
        r.get::<_, String>(0)?,
        r.get::<_, String>(1)?,
        r.get::<_, String>(2)?,
        r.get::<_, String>(3)?,
        r.get::<_, String>(4)?,
        r.get::<_, String>(9)?,
        r.get::<_, Option<String>>(10)?,
        r.get::<_, Option<String>>(11)?,
        r.get::<_, Option<String>>(12)?,
        r.get::<_, Option<String>>(13)?,
        r.get::<_, Option<String>>(14)?,
    );
    let metrics: Option<String> = r.get(15)?;
    Ok(
        (|| -> std::result::Result<TaskRecord, serde_json::Error> {
            Ok(TaskRecord {
                id: base.0,
                project_id: base.1,
                slug: base.2,
                title: base.3,
                request: base.4,
                attachments: serde_json::from_str(&attachments)?,
                doc_refs: serde_json::from_str(&doc_refs)?,
                state: serde_json::from_str(&state)?,
                budget_n: budget.map(|v| v as u32),
                created_at: base.5,
                completed_at: base.6,
                merge_commit: base.7,
                archived_at: base.8,
                protocol_ref: base.9,
                rules_hash: base.10,
                metrics: metrics.as_deref().map(serde_json::from_str).transpose()?,
            })
        })(),
    )
}

fn row_to_session(r: &rusqlite::Row<'_>) -> RowResult<Session> {
    let kind: String = r.get(2)?;
    let runtime: String = r.get(3)?;
    let skills: String = r.get(6)?;
    let round: i64 = r.get(7)?;
    let lifecycle: String = r.get(10)?;
    let base = (
        r.get::<_, String>(0)?,
        r.get::<_, String>(1)?,
        r.get::<_, String>(4)?,
        r.get::<_, Option<String>>(5)?,
        r.get::<_, String>(8)?,
        r.get::<_, Option<String>>(9)?,
        r.get::<_, String>(11)?,
        r.get::<_, Option<i32>>(12)?,
        r.get::<_, Option<String>>(13)?,
        r.get::<_, Option<String>>(14)?,
    );
    let u = |i: usize| -> rusqlite::Result<Option<u64>> {
        Ok(r.get::<_, Option<i64>>(i)?.map(|v| v.max(0) as u64))
    };
    let metrics = autome_domain::metrics::SessionMetrics {
        input_tokens: u(15)?,
        cache_read_tokens: u(16)?,
        cache_write_tokens: u(17)?,
        output_tokens: u(18)?,
        cost_usd: r.get::<_, Option<f64>>(19)?,
        turns: u(20)?,
        duration_ms: u(21)?,
        mean_request_input: u(22)?,
        design_doc_bytes: u(23)?,
        evidence_files: u(24)?,
    };
    Ok((|| -> std::result::Result<Session, serde_json::Error> {
        Ok(Session {
            id: base.0,
            task_id: base.1,
            kind: serde_json::from_str(&kind)?,
            runtime: Runtime::parse(&runtime).unwrap_or(Runtime::Claude),
            model: base.2,
            effort: base.3,
            skills: serde_json::from_str(&skills)?,
            round: round as u32,
            started_at: base.4,
            ended_at: base.5,
            lifecycle: serde_json::from_str(&lifecycle)?,
            log_path: base.6,
            pid: base.7,
            protocol_ref: base.8,
            rules_hash: base.9,
            metrics,
        })
    })())
}

/// RFC 3339 UTC, second precision. One format everywhere so string ordering
/// equals chronological ordering, which several queries rely on.
pub fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_iso(secs)
}

pub fn format_iso(epoch_secs: i64) -> String {
    // Civil-from-days (Howard Hinnant's algorithm). Avoids a dependency for
    // the one thing we need from a date library.
    let days = epoch_secs.div_euclid(86_400);
    let secs_of_day = epoch_secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60
    )
}

/// `<prefix>-<hex>`; unique enough for a single-user local database without
/// pulling in a UUID dependency at every call site.
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4().simple())
}

/// A convenience for building the projection the IPC layer returns.
pub fn task_counts(tasks: &[TaskRecord]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for t in tasks {
        let key = match &t.state {
            TaskState::Queued => "queued",
            TaskState::Active { node } if node.awaits_user() => "awaiting_user",
            TaskState::Active { .. } => "running",
            TaskState::Paused { .. } => "paused",
            TaskState::Stopped { .. } => "stopped",
            TaskState::Failed { .. } => "failed",
            TaskState::Done => "done",
            TaskState::Cancelled => "cancelled",
        };
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::task::{FailureReason, Node};

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    /// WAL mode leaves `-wal` and `-shm` siblings next to the database.
    /// Removing only the `.sqlite3` leaves two files behind per run, which is
    /// how a test suite quietly fills a temp directory.
    fn remove_db(path: &std::path::Path) {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = path.as_os_str().to_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(p));
        }
    }

    fn project(id: &str, path: &str) -> Project {
        Project {
            id: id.into(),
            path: path.into(),
            display_name: "p".into(),
            default_branch: "main".into(),
            parallel_limit: 3,
            onboarding: Onboarding::Skipped,
            disposition: AddDisposition::AdoptedExisting,
            added_at: now_iso(),
            removed_at: None,
        }
    }

    fn task(id: &str, project_id: &str, slug: &str) -> TaskRecord {
        TaskRecord {
            id: id.into(),
            project_id: project_id.into(),
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
        }
    }

    fn session(id: &str, task_id: &str, kind: SessionKind, round: u32) -> Session {
        Session {
            id: id.into(),
            task_id: task_id.into(),
            kind,
            runtime: Runtime::Claude,
            model: "m".into(),
            effort: None,
            skills: vec![],
            round,
            started_at: now_iso(),
            ended_at: None,
            lifecycle: SessionLifecycle::Running,
            log_path: "l".into(),
            pid: Some(1),
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
        }
    }

    // ---- projects --------------------------------------------------------

    #[test]
    fn a_project_round_trips() {
        let s = store();
        let p = project("p1", "/a/b");
        s.insert_project(&p).unwrap();
        assert_eq!(s.get_project("p1").unwrap(), p);
    }

    #[test]
    fn two_projects_cannot_share_a_path() {
        let s = store();
        s.insert_project(&project("p1", "/a/b")).unwrap();
        let err = s.insert_project(&project("p2", "/a/b")).unwrap_err();
        assert!(matches!(err, StoreError::Conflict { .. }), "{err}");
    }

    #[test]
    fn two_projects_do_not_hand_out_the_same_task_id() {
        // `tasks.id` is the primary key and `sessions` references it. Per
        // project numbering collided on the second project's first task, and
        // the insert reported it as a duplicate slug.
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_project(&project("p2", "/b")).unwrap();

        let first = s.next_task_id("p1").unwrap();
        s.insert_task(&task(&first, "p1", "one")).unwrap();
        let second = s.next_task_id("p2").unwrap();
        assert_ne!(first, second);
        s.insert_task(&task(&second, "p2", "two")).unwrap();
    }

    #[test]
    fn a_cancelled_task_does_not_release_its_number() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        for _ in 0..3 {
            let id = s.next_task_id("p1").unwrap();
            s.insert_task(&task(&id, "p1", &id)).unwrap();
        }
        assert_eq!(s.next_task_id("p1").unwrap(), "T-4");
    }

    #[test]
    fn project_by_path_finds_a_registered_directory() {
        let s = store();
        s.insert_project(&project("p1", "/a/b")).unwrap();
        assert_eq!(s.project_by_path("/a/b").unwrap().unwrap().id, "p1");
        assert!(s.project_by_path("/nope").unwrap().is_none());
    }

    #[test]
    fn a_removed_project_disappears_from_the_list_but_stays_fetchable() {
        let s = store();
        s.insert_project(&project("p1", "/a/b")).unwrap();
        s.remove_project("p1").unwrap();
        assert!(s.list_projects().unwrap().is_empty());
        let p = s.get_project("p1").unwrap();
        assert!(p.removed_at.is_some());
        assert!(!p.is_active());
    }

    #[test]
    fn onboarding_progress_persists() {
        let s = store();
        s.insert_project(&project("p1", "/a/b")).unwrap();
        s.update_project_onboarding("p1", Onboarding::InProgress { step: 4 })
            .unwrap();
        assert_eq!(
            s.get_project("p1").unwrap().onboarding,
            Onboarding::InProgress { step: 4 }
        );
    }

    #[test]
    fn updating_a_missing_project_is_not_found() {
        let s = store();
        assert!(matches!(
            s.update_project_parallel("ghost", 2).unwrap_err(),
            StoreError::NotFound { .. }
        ));
    }

    // ---- tasks -----------------------------------------------------------

    #[test]
    fn a_task_round_trips_including_its_state() {
        let s = store();
        s.insert_project(&project("p1", "/a/b")).unwrap();
        let mut t = task("T-1", "p1", "checkout");
        t.state = TaskState::Active {
            node: Node::Implement,
        };
        t.attachments = vec!["a.pdf".into()];
        t.budget_n = Some(25);
        s.insert_task(&t).unwrap();
        assert_eq!(s.get_task("T-1").unwrap(), t);
    }

    #[test]
    fn a_task_cannot_reuse_a_slug_within_a_project_but_can_across_projects() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_project(&project("p2", "/b")).unwrap();
        s.insert_task(&task("T-1", "p1", "same")).unwrap();
        assert!(matches!(
            s.insert_task(&task("T-2", "p1", "same")).unwrap_err(),
            StoreError::Conflict { .. }
        ));
        s.insert_task(&task("T-1b", "p2", "same")).unwrap();
    }

    #[test]
    fn task_ids_increment_and_are_never_reused() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_project(&project("p2", "/b")).unwrap();
        assert_eq!(s.next_task_id("p1").unwrap(), "T-1");
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        assert_eq!(s.next_task_id("p1").unwrap(), "T-2");
        s.insert_task(&task("T-2", "p1", "b")).unwrap();
        // A cancelled task still holds its number.
        s.set_task_state("T-1", &TaskState::Cancelled).unwrap();
        assert_eq!(s.next_task_id("p1").unwrap(), "T-3");
        // And so does a task in another project: the id is the primary key,
        // and `sessions` and `decisions` both point at it.
        assert_eq!(s.next_task_id("p2").unwrap(), "T-3");
    }

    #[test]
    fn slots_in_use_counts_only_the_states_that_hold_one() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        for (i, state) in [
            TaskState::Active {
                node: Node::Implement,
            },
            TaskState::Active {
                node: Node::AwaitMerge,
            },
            TaskState::Queued,
            TaskState::Paused {
                resume: Node::Audit,
            },
            TaskState::Done,
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = task(&format!("T-{i}"), "p1", &format!("s{i}"));
            t.state = state;
            s.insert_task(&t).unwrap();
        }
        assert_eq!(s.slots_in_use("p1").unwrap(), 1, "only the running one");
    }

    #[test]
    fn queued_tasks_come_back_in_submission_order() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        for (i, at) in ["2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z"]
            .iter()
            .enumerate()
        {
            let mut t = task(&format!("T-{i}"), "p1", &format!("s{i}"));
            t.created_at = (*at).to_string();
            s.insert_task(&t).unwrap();
        }
        let queued = s.queued_tasks("p1").unwrap();
        assert_eq!(queued[0].id, "T-0");
        assert_eq!(queued[1].id, "T-1");
    }

    #[test]
    fn list_unfinished_excludes_done_and_cancelled_across_projects() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_project(&project("p2", "/b")).unwrap();
        let mut running = task("T-1", "p1", "a");
        running.state = TaskState::Active { node: Node::Design };
        let mut done = task("T-2", "p1", "b");
        done.state = TaskState::Done;
        let mut other = task("T-3", "p2", "c");
        other.state = TaskState::Failed {
            at: Node::Audit,
            reason: FailureReason::Infeasible,
        };
        for t in [&running, &done, &other] {
            s.insert_task(t).unwrap();
        }
        let ids: Vec<String> = s
            .list_unfinished()
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"T-1".to_string()));
        assert!(ids.contains(&"T-3".to_string()));
    }

    #[test]
    fn task_paths_are_derived_from_the_slug() {
        let t = task("T-1", "p1", "checkout-flow");
        assert_eq!(t.branch(), "autome/checkout-flow");
        assert_eq!(t.design_doc(), "docs/checkout-flow/checkout-flow.md");
    }

    #[test]
    fn completing_and_archiving_a_task_persists() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.complete_task("T-1", Some("abc123")).unwrap();
        let t = s.get_task("T-1").unwrap();
        assert_eq!(t.merge_commit.as_deref(), Some("abc123"));
        assert!(t.completed_at.is_some());
        assert!(!t.is_archived());

        s.set_task_archived("T-1", true).unwrap();
        assert!(s.get_task("T-1").unwrap().is_archived());
        s.set_task_archived("T-1", false).unwrap();
        assert!(!s.get_task("T-1").unwrap().is_archived());
    }

    // ---- sessions --------------------------------------------------------

    #[test]
    fn sessions_round_trip_and_list_newest_first() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        let mut first = session("s1", "T-1", SessionKind::Intake, 1);
        first.started_at = "2026-01-01T00:00:00Z".into();
        let mut second = session(
            "s2",
            "T-1",
            SessionKind::Role {
                role: autome_domain::role::Role::Plan,
            },
            1,
        );
        second.started_at = "2026-01-02T00:00:00Z".into();
        s.insert_session(&first).unwrap();
        s.insert_session(&second).unwrap();
        let list = s.list_sessions("T-1").unwrap();
        assert_eq!(list[0].id, "s2", "newest first");
        assert_eq!(list[1], first);
    }

    #[test]
    fn finishing_a_session_records_its_lifecycle_and_clears_it_from_running() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.insert_session(&session("s1", "T-1", SessionKind::Intake, 1))
            .unwrap();
        assert!(s.running_session("T-1").unwrap().is_some());
        assert_eq!(s.all_running_sessions().unwrap().len(), 1);

        s.finish_session("s1", &SessionLifecycle::Exited { exit_code: 0 }, "t")
            .unwrap();
        assert!(s.running_session("T-1").unwrap().is_none());
        assert!(s.all_running_sessions().unwrap().is_empty());
        assert_eq!(
            s.list_sessions("T-1").unwrap()[0].lifecycle,
            SessionLifecycle::Exited { exit_code: 0 }
        );
    }

    #[test]
    fn next_round_counts_per_role_not_per_task() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        let impl_kind = SessionKind::Role {
            role: autome_domain::role::Role::Impl,
        };
        let audit_kind = SessionKind::Role {
            role: autome_domain::role::Role::Audit,
        };
        assert_eq!(s.next_round("T-1", &impl_kind).unwrap(), 1);
        s.insert_session(&session("s1", "T-1", impl_kind, 1))
            .unwrap();
        s.insert_session(&session("s2", "T-1", impl_kind, 2))
            .unwrap();
        assert_eq!(s.next_round("T-1", &impl_kind).unwrap(), 3);
        assert_eq!(s.next_round("T-1", &audit_kind).unwrap(), 1);
    }

    // ---- decisions -------------------------------------------------------

    #[test]
    fn syncing_decisions_inserts_new_items_and_updates_their_text() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions(
            "T-1",
            &[("B-01".into(), "第一版".into())],
            &[("D-01".into(), "争议".into())],
        )
        .unwrap();
        let d = s.list_decisions("T-1").unwrap();
        assert_eq!(d.len(), 2);

        s.sync_decisions(
            "T-1",
            &[("B-01".into(), "改过的".into())],
            &[("D-01".into(), "争议".into())],
        )
        .unwrap();
        let b = s
            .list_decisions("T-1")
            .unwrap()
            .into_iter()
            .find(|d| d.item_id == "B-01")
            .unwrap();
        assert_eq!(b.text, "改过的");
    }

    #[test]
    fn syncing_drops_undecided_items_the_document_removed() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions(
            "T-1",
            &[("B-01".into(), "x".into()), ("B-02".into(), "y".into())],
            &[],
        )
        .unwrap();
        s.sync_decisions("T-1", &[("B-01".into(), "x".into())], &[])
            .unwrap();
        let ids: Vec<String> = s
            .list_decisions("T-1")
            .unwrap()
            .into_iter()
            .map(|d| d.item_id)
            .collect();
        assert_eq!(ids, vec!["B-01".to_string()]);
    }

    #[test]
    fn syncing_keeps_an_item_the_user_has_already_decided_even_if_it_vanishes() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions("T-1", &[("B-01".into(), "x".into())], &[])
            .unwrap();
        s.set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();
        // The agent reworded the section and the id disappeared.
        s.sync_decisions("T-1", &[], &[]).unwrap();
        let d = s.list_decisions("T-1").unwrap();
        assert_eq!(d.len(), 1, "a decided item is not silently dropped");
        assert_eq!(d[0].disposition, Disposition::Include);
    }

    #[test]
    fn pending_counts_separate_included_from_ruled() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions(
            "T-1",
            &[("B-01".into(), "x".into()), ("B-02".into(), "y".into())],
            &[("D-01".into(), "z".into())],
        )
        .unwrap();
        s.set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();
        s.set_disposition("T-1", "backlog", "B-02", Disposition::Ignore, None)
            .unwrap();
        s.set_disposition(
            "T-1",
            "dispute",
            "D-01",
            Disposition::Ruled,
            Some("按评审方"),
        )
        .unwrap();

        let pending = s.pending_decisions("T-1").unwrap();
        assert_eq!(pending.included, 1, "ignore does not count as included");
        assert_eq!(pending.ruled, 1);
        assert!(pending.any());
    }

    #[test]
    fn consuming_decisions_is_idempotent() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions("T-1", &[("B-01".into(), "x".into())], &[])
            .unwrap();
        s.set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();

        assert_eq!(s.consume_decisions("T-1").unwrap(), 1);
        assert_eq!(s.pending_decisions("T-1").unwrap().included, 0);
        assert_eq!(
            s.consume_decisions("T-1").unwrap(),
            0,
            "a second consume applies nothing"
        );
    }

    #[test]
    fn an_undecided_item_is_never_consumed() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions("T-1", &[("B-01".into(), "x".into())], &[])
            .unwrap();
        assert_eq!(s.consume_decisions("T-1").unwrap(), 0);
        assert!(s.list_decisions("T-1").unwrap()[0].consumed_at.is_none());
    }

    #[test]
    fn deciding_a_consumed_item_is_not_found() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions("T-1", &[("B-01".into(), "x".into())], &[])
            .unwrap();
        s.set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();
        s.consume_decisions("T-1").unwrap();
        assert!(matches!(
            s.set_disposition("T-1", "backlog", "B-01", Disposition::Ignore, None),
            Err(StoreError::NotFound { .. })
        ));
    }

    #[test]
    fn decisions_for_prompt_returns_only_decided_unconsumed_items() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions(
            "T-1",
            &[("B-01".into(), "x".into()), ("B-02".into(), "y".into())],
            &[],
        )
        .unwrap();
        s.set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();
        let for_prompt = s.decisions_for_prompt("T-1").unwrap();
        assert_eq!(for_prompt.len(), 1);
        assert_eq!(for_prompt[0].item_id, "B-01");
    }

    // ---- events ----------------------------------------------------------

    #[test]
    fn events_get_monotonic_sequence_numbers() {
        let s = store();
        let a = s
            .append_event("task.updated", "T-1", serde_json::json!({"x": 1}))
            .unwrap();
        let b = s
            .append_event("task.updated", "T-1", serde_json::json!({"x": 2}))
            .unwrap();
        assert!(b > a);
        assert_eq!(s.latest_seq().unwrap(), b);
    }

    #[test]
    fn events_since_returns_only_newer_ones_in_order() {
        let s = store();
        let first = s.append_event("a", "s", Value::Null).unwrap();
        s.append_event("b", "s", Value::Null).unwrap();
        s.append_event("c", "s", Value::Null).unwrap();
        let kinds: Vec<String> = s
            .events_since(first, 10)
            .unwrap()
            .into_iter()
            .map(|(_, k, _)| k)
            .collect();
        assert_eq!(kinds, vec!["b".to_string(), "c".to_string()]);
    }

    #[test]
    fn events_since_respects_its_limit() {
        let s = store();
        for _ in 0..5 {
            s.append_event("x", "s", Value::Null).unwrap();
        }
        assert_eq!(s.events_since(0, 2).unwrap().len(), 2);
    }

    // ---- misc ------------------------------------------------------------

    #[test]
    fn a_task_cannot_reference_a_project_that_does_not_exist() {
        let s = store();
        assert!(s.insert_task(&task("T-1", "ghost", "a")).is_err());
    }

    #[test]
    fn task_counts_bucket_every_state() {
        let mut tasks = Vec::new();
        for (i, state) in [
            TaskState::Queued,
            TaskState::Active {
                node: Node::Implement,
            },
            TaskState::Active {
                node: Node::AwaitMerge,
            },
            TaskState::Done,
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = task(&format!("T-{i}"), "p", &format!("s{i}"));
            t.state = state;
            tasks.push(t);
        }
        let counts = task_counts(&tasks);
        assert_eq!(counts.get("queued"), Some(&1));
        assert_eq!(counts.get("running"), Some(&1));
        assert_eq!(counts.get("awaiting_user"), Some(&1));
        assert_eq!(counts.get("done"), Some(&1));
    }

    #[test]
    fn format_iso_matches_known_instants() {
        assert_eq!(format_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_iso(1_758_000_000), "2025-09-16T05:20:00Z");
        // A leap day.
        assert_eq!(format_iso(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn new_id_is_prefixed_and_unique() {
        let a = new_id("evt");
        let b = new_id("evt");
        assert!(a.starts_with("evt-"));
        assert_ne!(a, b);
    }

    #[test]
    fn a_reopened_database_keeps_its_rows() {
        let path =
            std::env::temp_dir().join(format!("automed-store-{}.sqlite3", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let s = Store::open(&path).unwrap();
            s.insert_project(&project("p1", "/a")).unwrap();
        }
        {
            let s = Store::open(&path).unwrap();
            assert_eq!(s.list_projects().unwrap().len(), 1);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn migrating_an_already_migrated_database_is_a_no_op() {
        let path =
            std::env::temp_dir().join(format!("automed-store-mig-{}.sqlite3", std::process::id()));
        remove_db(&path);
        Store::open(&path).unwrap();
        Store::open(&path).unwrap();
        let s = Store::open(&path).unwrap();
        let v: i64 = s
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        drop(s);
        remove_db(&path);
    }

    /// The bug this guards, in full: the desktop app's database path held a
    /// file an earlier prototype had written — different tables, no
    /// `user_version`. `migrate` read version 0, ran the v1 block, and SQLite
    /// said `table projects already exists`. The core exited, Electron
    /// restarted it a second later, and the loop ran for as long as the app
    /// was open while the window said only "内核不可达".
    #[test]
    fn a_database_that_is_not_ours_is_refused_by_name() {
        let path = std::env::temp_dir().join(format!(
            "automed-store-foreign-{}.sqlite3",
            std::process::id()
        ));
        remove_db(&path);
        {
            let conn = Connection::open(&path).unwrap();
            // An earlier prototype's shape: event-sourced projections, and
            // one table name that collides with ours.
            conn.execute_batch(
                "CREATE TABLE task_projections (aggregate_id TEXT PRIMARY KEY, state_json TEXT);
                 CREATE TABLE projects (id TEXT PRIMARY KEY);",
            )
            .unwrap();
        }

        let message = match Store::open(&path) {
            Ok(_) => panic!("a stranger's database was opened as if it were ours"),
            Err(e) => e.to_string(),
        };
        // The file, so the user knows which one to move; the table, so they
        // can tell a stranger's database from a corrupt one of ours; and what
        // to do, because retrying is not it.
        assert!(message.contains(&path.display().to_string()), "{message}");
        assert!(message.contains("projects"), "{message}");
        assert!(message.contains("改名移开"), "{message}");
        assert!(!message.contains("already exists"), "{message}");

        remove_db(&path);
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused_rather_than_downgraded() {
        let path = std::env::temp_dir().join(format!(
            "automed-store-newer-{}.sqlite3",
            std::process::id()
        ));
        remove_db(&path);
        {
            let s = Store::open(&path).unwrap();
            s.conn
                .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
        }

        let message = match Store::open(&path) {
            Ok(_) => panic!("a newer database was opened by an older build"),
            Err(e) => e.to_string(),
        };
        assert!(
            message.contains(&(SCHEMA_VERSION + 1).to_string()),
            "{message}"
        );
        assert!(message.contains(&SCHEMA_VERSION.to_string()), "{message}");

        remove_db(&path);
    }

    /// An empty file at the path is not a stranger's database — it is where
    /// every first run starts.
    #[test]
    fn an_empty_file_at_the_path_still_migrates() {
        let path = std::env::temp_dir().join(format!(
            "automed-store-empty-{}.sqlite3",
            std::process::id()
        ));
        remove_db(&path);
        std::fs::write(&path, b"").unwrap();

        let s = Store::open(&path).unwrap();
        let v: i64 = s
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        drop(s);
        remove_db(&path);
    }
}
