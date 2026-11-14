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

use autome_domain::project::{AddDisposition, Member, Onboarding, Project, ProjectKind};
use autome_domain::role::Runtime;
use autome_domain::session::{Session, SessionKind, SessionLifecycle};
use autome_domain::task::{Disposition, Inject, Node, PendingDecisions, TaskKind, TaskState};
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
pub const SCHEMA_VERSION: i64 = 9;

/// One task as the store holds it. Progress fields (rounds, milestones) are
/// deliberately absent — they come from the design document at read time.
/// `Eq` is deliberately absent: `TaskMetrics` carries a cost in dollars.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskRecord {
    pub kind: TaskKind,
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
    /// Where this task's documents go inside the repository that holds them,
    /// frozen at creation from the project's configuration.
    ///
    /// Frozen, like `protocol_ref`, and for the same reason: the answer names
    /// paths that are already committed on a branch. A user who changes the
    /// setting halfway through must not have a running task's documents
    /// silently split across two directories. `None` is every task written
    /// before this existed, and means `docs/`.
    pub doc_root: Option<String>,
}

impl TaskRecord {
    pub fn branch(&self) -> String {
        autome_domain::project::Project::branch_name(&self.slug)
    }
    /// The document root this task was created under.
    pub fn doc_root(&self) -> &str {
        self.doc_root
            .as_deref()
            .unwrap_or(autome_domain::project::DEFAULT_DOC_ROOT_REPO)
    }
    pub fn doc_dir(&self) -> String {
        autome_domain::project::Project::doc_dir(self.doc_root(), &self.slug)
    }
    /// Where this task's documents go once archived (requirement T-12) — under
    /// the same root they live in, never a second one.
    pub fn archive_dir(&self) -> String {
        autome_domain::project::Project::archive_dir(self.doc_root(), &self.slug)
    }
    /// `<doc_root>/<slug>/<slug>.md` — the design document the scheduler parses.
    pub fn bug_doc(&self) -> String {
        format!("{}/{}-bug.md", self.doc_dir(), self.slug)
    }
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
    /// The session currently carrying this item into its round, if any.
    pub carried_by: Option<String>,
}

/// Something a person should hear about that belongs to no running task.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AlertRecord {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub task_id: Option<String>,
    pub detail: String,
    pub created_at: String,
}

/// A rate-limited CLI is left alone this long the first time, twice that the
/// next, and never longer than an hour.
pub const COOLDOWN_BASE_MINUTES: i64 = 10;
pub const COOLDOWN_MAX_MINUTES: i64 = 60;

/// `(key, domain, proposal, evidence, state)` for one rule proposal.
pub type RuleProposalRow = (String, String, String, Vec<String>, String);

pub struct StoredEvent {
    pub seq: u64,
    pub kind: String,
    pub subject: String,
    pub payload: Value,
    pub at: String,
}

pub struct Store {
    conn: Connection,
    db_path: PathBuf,
}

impl Store {
    pub fn kernel_mark(&self) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT build_id FROM kernel_mark WHERE id = 1", [], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// The driver lock must cover this claim and all subsequent side effects.
    pub fn claim_kernel_mark(&self, current: &str) -> Result<Option<String>> {
        self.claim_kernel_mark_with_adoption(current, None)
            .map(|(rejected, _)| rejected)
    }

    pub(crate) fn claim_kernel_mark_with_adoption(
        &self,
        current: &str,
        adopt_method: Option<&str>,
    ) -> Result<(Option<String>, bool)> {
        use crate::build_id::{Verdict, verdict};

        let tx = rusqlite::Transaction::new_unchecked(
            &self.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        let recorded: Option<String> = tx
            .query_row("SELECT build_id FROM kernel_mark WHERE id = 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        let mut adopted = false;
        let rejected = match verdict(current, recorded.as_deref()) {
            Verdict::Advance => {
                tx.execute("INSERT INTO kernel_mark (id, build_id, updated_at) VALUES (1, ?1, ?2) ON CONFLICT(id) DO UPDATE SET build_id = excluded.build_id, updated_at = excluded.updated_at", rusqlite::params![current, now_iso()])?;
                None
            }
            Verdict::Same => None,
            Verdict::Older | Verdict::Incomparable if adopt_method.is_some() => {
                tx.execute(
                    "UPDATE kernel_mark SET build_id = ?1, updated_at = ?2 WHERE id = 1",
                    params![current, now_iso()],
                )?;
                self.append_event(
                    "kernel.adopted",
                    "kernel",
                    serde_json::json!({
                        "from": recorded, "to": current, "via": "AUTOME_ALLOW_OLDER_KERNEL",
                        "method": adopt_method.unwrap()
                    }),
                )?;
                adopted = true;
                None
            }
            Verdict::Older | Verdict::Incomparable => recorded,
        };
        tx.commit()?;
        Ok((rejected, adopted))
    }

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

        // v3: a project need not be one Git repository. Some directories hold
        // several independent ones and exist to be developed across — and
        // running `git init` over such a directory, which is what P-01 said to
        // do, records each child as a gitlink and makes every later
        // `git add child/…` fail. See `ProjectKind`.
        //
        // `doc_root` joins `protocol_ref` as a per-task frozen decision: a
        // workspace keeps its task documents in one member repository under a
        // named subdirectory, and moving that setting must not move the
        // documents of a task already running.
        //
        // Every column is nullable with no default: NULL reads as "the answer
        // from before workspaces existed", which is what every existing row
        // means.
        if version < 3 {
            self.conn.execute_batch(
                r#"
                BEGIN;
                ALTER TABLE projects ADD COLUMN kind TEXT;
                ALTER TABLE projects ADD COLUMN members TEXT;
                ALTER TABLE projects ADD COLUMN docs_repo TEXT;

                ALTER TABLE tasks ADD COLUMN doc_root TEXT;
                PRAGMA user_version = 3;
                COMMIT;
                "#,
            )?;
        }
        // v4, three additions that share one reason: the core takes on work
        // it used to leave to a session or to the user.
        //
        // `carried_by` / `consumed_by`: a decision is a lease, not a flag.
        // A session that starts carrying it marks it; only that session
        // ending well consumes it, and one that crashes hands it back. Before
        // this, decisions were consumed on the way *into* the round meant to
        // apply them — which then read the unconsumed ones and found none.
        //
        // `runtime_cooldowns`: a CLI that answered "rate limited" is left
        // alone until `until`, and a run of them backs off further.
        //
        // `project_alerts`: things a person should hear about that belong to
        // no running task — main failing its gate after a merge, the first.
        if version < 4 {
            self.conn.execute_batch(
                r#"
                BEGIN;
                ALTER TABLE decisions ADD COLUMN carried_by TEXT;
                ALTER TABLE decisions ADD COLUMN consumed_by TEXT;

                CREATE TABLE runtime_cooldowns (
                    runtime TEXT PRIMARY KEY,
                    until   TEXT NOT NULL,
                    reason  TEXT NOT NULL,
                    strikes INTEGER NOT NULL DEFAULT 1
                );

                CREATE TABLE project_alerts (
                    id           TEXT PRIMARY KEY,
                    project_id   TEXT NOT NULL REFERENCES projects(id),
                    kind         TEXT NOT NULL,
                    task_id      TEXT,
                    detail       TEXT NOT NULL,
                    created_at   TEXT NOT NULL,
                    dismissed_at TEXT
                );
                -- A crashed round waiting to be run again. One row per task:
                -- `attempts` counts the crashes since the task last moved,
                -- `not_before` is when the next attempt may start (NULL once
                -- it has), `inject` is what the crashed round was carrying.
                CREATE TABLE session_retries (
                    task_id    TEXT PRIMARY KEY REFERENCES tasks(id),
                    node       TEXT NOT NULL,
                    attempts   INTEGER NOT NULL DEFAULT 0,
                    not_before TEXT,
                    inject     TEXT,
                    reason     TEXT NOT NULL
                );
                PRAGMA user_version = 4;
                COMMIT;
                "#,
            )?;
        }
        if version < 5 {
            self.conn.execute_batch(
                "BEGIN;
                 ALTER TABLE tasks ADD COLUMN kind TEXT NOT NULL DEFAULT 'feature';
                 ALTER TABLE tasks ADD COLUMN queued_seq INTEGER;
                 CREATE TABLE task_briefs (
                     task_id TEXT PRIMARY KEY REFERENCES tasks(id),
                     body TEXT NOT NULL,
                     created_at TEXT NOT NULL
                 );
                 PRAGMA user_version = 5;
                 COMMIT;",
            )?;
        }
        if version < 6 {
            self.conn.execute_batch(
                "BEGIN;
         ALTER TABLE sessions ADD COLUMN limit_minutes INTEGER;
         PRAGMA user_version = 6;
         COMMIT;",
            )?;
        }
        if version < 7 {
            self.conn.execute_batch(
                "BEGIN;
                 CREATE TABLE IF NOT EXISTS task_after (
                     task_id TEXT NOT NULL REFERENCES tasks(id),
                     after_id TEXT NOT NULL REFERENCES tasks(id),
                     ord INTEGER NOT NULL,
                     notified TEXT,
                     PRIMARY KEY (task_id, after_id)
                 );
                 PRAGMA user_version = 7;
                 COMMIT;",
            )?;
        }
        if version < 8 {
            self.conn.execute_batch(
                "BEGIN;
                 CREATE TABLE IF NOT EXISTS kernel_mark (
                     id INTEGER PRIMARY KEY CHECK (id = 1),
                     build_id TEXT NOT NULL,
                     updated_at TEXT NOT NULL
                 );
                 PRAGMA user_version = 8;
                 COMMIT;",
            )?;
        }
        if version < 9 {
            self.conn.execute_batch(
                "BEGIN;
                 CREATE TABLE sessions_v9 (
                    id TEXT PRIMARY KEY, task_id TEXT REFERENCES tasks(id),
                    kind TEXT NOT NULL, runtime TEXT NOT NULL, model TEXT NOT NULL,
                    effort TEXT, skills TEXT NOT NULL, round INTEGER NOT NULL,
                    started_at TEXT NOT NULL, ended_at TEXT, lifecycle TEXT NOT NULL,
                    log_path TEXT NOT NULL, pid INTEGER, protocol_ref TEXT, rules_hash TEXT,
                    input_tokens INTEGER, cache_read_tokens INTEGER, cache_write_tokens INTEGER,
                    output_tokens INTEGER, cost_usd REAL, turns INTEGER, duration_ms INTEGER,
                    mean_request_input INTEGER, design_doc_bytes INTEGER, evidence_files INTEGER,
                    limit_minutes INTEGER, project_id TEXT REFERENCES projects(id),
                    CHECK ((task_id IS NULL) = (json_extract(kind, '$.kind') = 'onboarding')),
                    CHECK (task_id IS NOT NULL OR project_id IS NOT NULL)
                 );
                 INSERT INTO sessions_v9 SELECT *, NULL FROM sessions;
                 DROP TABLE sessions;
                 ALTER TABLE sessions_v9 RENAME TO sessions;
                 CREATE INDEX sessions_by_task ON sessions (task_id, started_at);
                 CREATE INDEX sessions_by_project ON sessions (project_id, started_at);
                 PRAGMA user_version = 9;
                 COMMIT;",
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

    /// Only the final successful gate on a merged branch can publish across tasks.
    /// The resolver uses the merge commit's second parent, just like MainCheck reuse.
    pub fn gate_passes(
        &self,
        project_id: &str,
        task_id: &str,
        merged_head: impl Fn(&str) -> Option<String>,
    ) -> Result<String> {
        let mut stmt = self.conn.prepare(
            "SELECT e.subject_id, e.payload, e.at, t.merge_commit FROM events e
             JOIN tasks t ON t.id = e.subject_id
             WHERE t.project_id = ?1 AND e.kind = 'gate.result'
             AND julianday(e.at) >= julianday('now', '-7 days')
             AND (e.subject_id = ?2 OR (t.merge_commit IS NOT NULL
               AND e.seq = (SELECT MAX(f.seq) FROM events f WHERE f.subject_id = e.subject_id
                 AND f.kind = 'gate.result' AND json_extract(f.payload, '$.ok') = 1
                 AND COALESCE(json_extract(f.payload, '$.kind'), '') != 'quick')))
             ORDER BY e.at DESC, e.seq DESC",
        )?;
        let rows = stmt.query_map(params![project_id, task_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut out = String::new();
        let mut count = 0;
        for row in rows {
            let (subject, payload, at, merge) = row?;
            let p: Value = serde_json::from_str(&payload)?;
            let (Some(sha), Some(node), Some(passes)) = (
                p["sha"].as_str(),
                p["node"].as_str(),
                p["passes"].as_array(),
            ) else {
                continue;
            };
            if subject != task_id && merge.as_deref().and_then(&merged_head).as_deref() != Some(sha)
            {
                continue;
            }
            if [&subject, node, sha, &at]
                .iter()
                .any(|s| s.contains(['\t', '\n', '\r']))
            {
                continue;
            }
            for pass in passes {
                let Some(key) = pass["key"].as_str().filter(|k| crate::gate::valid_key(k)) else {
                    continue;
                };
                out.push_str(&format!(
                    "{key}\t{subject}\t{node}\t{}\t{at}\n",
                    sha.chars().take(12).collect::<String>()
                ));
                count += 1;
                if count == 5000 {
                    return Ok(out);
                }
            }
        }
        Ok(out)
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

    pub fn last_event_before(
        &self,
        subject_id: &str,
        kind: &str,
        before_seq: u64,
    ) -> Result<Option<Value>> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM events WHERE subject_id = ?1 AND kind = ?2 AND seq < ?3
             ORDER BY seq DESC LIMIT 1",
                params![subject_id, kind, before_seq],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.as_deref().map(serde_json::from_str).transpose()?)
    }

    /// Complete, ordered ledger for one task and event kind (no page truncation).
    pub fn task_events(&self, task_id: &str, kind: &str) -> Result<Vec<Value>> {
        let mut query = self.conn.prepare(
            "SELECT payload FROM events WHERE subject_id = ?1 AND kind = ?2 ORDER BY seq",
        )?;
        let rows = query.query_map(params![task_id, kind], |r| r.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    /// Latest of a set of event kinds, without a payload-field restriction.
    pub fn last_event_of(
        &self,
        subject: &str,
        kinds: &[&str],
    ) -> Result<Option<(u64, String, Value)>> {
        let raw: Option<(u64, String, String)> = self
            .conn
            .query_row(
                "SELECT seq, kind, payload FROM events WHERE subject_id = ?1
             AND kind IN (SELECT value FROM json_each(?2)) ORDER BY seq DESC LIMIT 1",
                params![subject, serde_json::to_string(kinds)?],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        raw.map(|(seq, kind, payload)| Ok((seq, kind, serde_json::from_str(&payload)?)))
            .transpose()
    }

    pub fn last_event_in(
        &self,
        subject_id: &str,
        kinds: &[&str],
        field: &str,
        value: &str,
    ) -> Result<Option<(u64, String, Value)>> {
        let raw: Option<(u64, String, String)> = self
            .conn
            .query_row(
                "SELECT seq, kind, payload FROM events WHERE subject_id = ?1
             AND kind IN (SELECT value FROM json_each(?2))
             AND json_extract(payload, ?3) = ?4 ORDER BY seq DESC LIMIT 1",
                params![
                    subject_id,
                    serde_json::to_string(kinds)?,
                    format!("$.{field}"),
                    value
                ],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        raw.map(|(seq, kind, payload)| Ok((seq, kind, serde_json::from_str(&payload)?)))
            .transpose()
    }

    pub fn any_event_after(&self, subject_id: &str, kinds: &[&str], seq: u64) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM events WHERE subject_id = ?1 AND seq > ?2
             AND kind IN (SELECT value FROM json_each(?3)))",
            params![subject_id, seq, serde_json::to_string(kinds)?],
            |r| r.get(0),
        )?)
    }

    pub fn session_started_event(&self, task_id: &str, session_id: &str) -> Result<Option<Value>> {
        self.session_event(task_id, session_id, "session.started")
    }

    pub fn session_event(
        &self,
        task_id: &str,
        session_id: &str,
        kind: &str,
    ) -> Result<Option<Value>> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM events WHERE subject_id = ?1 AND kind = ?3
             AND json_extract(payload, '$.session_id') = ?2 ORDER BY seq DESC LIMIT 1",
                params![task_id, session_id, kind],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.as_deref().map(serde_json::from_str).transpose()?)
    }

    /// Input saved before suspending a retry, unless a newer session consumed it.
    pub fn resume_input(
        &self,
        task_id: &str,
        node: Node,
        session_id: Option<&str>,
    ) -> Result<Option<Value>> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM events WHERE subject_id = ?1 AND kind = 'task.resume_input'
             AND json_extract(payload, '$.node') = ?2
             AND seq > COALESCE((SELECT MAX(seq) FROM events WHERE subject_id = ?1
                 AND kind = 'session.started' AND json_extract(payload, '$.session_id') = ?3), 0)
             ORDER BY seq DESC LIMIT 1",
                params![task_id, node.as_str(), session_id],
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

    pub fn count_events_after(
        &self,
        subject_id: &str,
        kind: &str,
        after_kind: &str,
    ) -> Result<u32> {
        let n: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM events WHERE subject_id = ?1 AND kind = ?2
             AND seq > COALESCE((SELECT MAX(seq) FROM events WHERE subject_id = ?1 AND kind = ?3), 0)",
            params![subject_id, kind, after_kind], |r| r.get(0),
        )?;
        Ok(n)
    }

    pub fn latest_seq(&self) -> Result<u64> {
        let seq: i64 =
            self.conn
                .query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |r| r.get(0))?;
        Ok(seq as u64)
    }

    /// All supplementary information for this task, in arrival order.
    pub fn task_replies(&self, task_id: &str) -> Result<Vec<(String, Value)>> {
        let mut stmt = self.conn.prepare(
            "SELECT at, payload FROM events WHERE subject_id = ?1 AND kind = 'task.replied' ORDER BY seq",
        )?;
        let rows = stmt.query_map([task_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (time, payload) = row?;
            Ok((time, serde_json::from_str(&payload)?))
        })
        .collect()
    }

    pub fn operator_notes(&self, task_id: &str) -> Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT kind, at, payload FROM events WHERE subject_id = ?1 AND kind IN ('task.note_added', 'task.note_delivered') ORDER BY seq",
        )?;
        let rows = stmt.query_map([task_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut notes: Vec<Value> = Vec::new();
        for row in rows {
            let (kind, at, raw) = row?;
            let p: Value = serde_json::from_str(&raw)?;
            if kind == "task.note_added" {
                notes.push(serde_json::json!({"n":p["n"], "text":p["text"], "at":at, "delivered":null, "late":p["late"] == true}));
            } else if let Some(numbers) = p["notes"].as_array() {
                for note in &mut notes {
                    if note["late"] != true && numbers.contains(&note["n"]) {
                        note["delivered"] = serde_json::json!({"session_id":p["session_id"], "role":p["role"], "round":p["round"], "at":at});
                    }
                }
            }
        }
        for note in &mut notes {
            note.as_object_mut().unwrap().remove("late");
        }
        Ok(notes)
    }

    /// Events after `after_seq`, oldest first. The Renderer's resync path.
    /// `events_since`, with the subject each event is about — the task, the
    /// project or the alert. The command line's `run` names the task on every
    /// line it prints.
    pub fn events_since_with_at(&self, after_seq: u64, limit: u32) -> Result<Vec<StoredEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, kind, subject_id, payload, at FROM events WHERE seq > ?1 ORDER BY seq LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_seq as i64, limit as i64], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (seq, kind, subject, payload, at) = row?;
            Ok(StoredEvent {
                seq,
                kind,
                subject,
                payload: serde_json::from_str(&payload)?,
                at,
            })
        })
        .collect()
    }

    pub fn events_by_subject(
        &self,
        subject: &str,
        after: u64,
        limit: u32,
    ) -> Result<Vec<StoredEvent>> {
        let mut stmt = self.conn.prepare("SELECT seq, kind, subject_id, payload, at FROM events WHERE subject_id = ?1 AND seq > ?2 ORDER BY seq LIMIT ?3")?;
        let rows = stmt.query_map(
            params![subject, after.min(i64::MAX as u64) as i64, limit],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )?;
        rows.map(|row| {
            let (seq, kind, subject, payload, at) = row?;
            Ok(StoredEvent {
                seq,
                kind,
                subject,
                payload: serde_json::from_str(&payload)?,
                at,
            })
        })
        .collect()
    }

    pub fn events_since_with_subject(
        &self,
        after_seq: u64,
        limit: u32,
    ) -> Result<Vec<(u64, String, String, Value)>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, kind, subject_id, payload FROM events WHERE seq > ?1 ORDER BY seq LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![after_seq as i64, limit as i64], |r| {
            Ok((
                r.get::<_, i64>(0)? as u64,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, kind, subject, payload) = row?;
            out.push((seq, kind, subject, serde_json::from_str(&payload)?));
        }
        Ok(out)
    }

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
                                       onboarding, disposition, added_at, removed_at,
                                       kind, members, docs_repo)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                    serde_json::to_string(&project.kind)?,
                    serde_json::to_string(&project.members)?,
                    project.docs_repo,
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
                &format!("{PROJECT_SELECT_BASE} WHERE id = ?1"),
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
                &format!("{PROJECT_SELECT_BASE} WHERE path = ?1"),
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
        let mut stmt = self.conn.prepare(&format!(
            "{PROJECT_SELECT_BASE} WHERE removed_at IS NULL ORDER BY added_at DESC"
        ))?;
        let rows = stmt.query_map([], row_to_project)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row??);
        }
        Ok(out)
    }

    /// Writes a project over the row that already has its id.
    ///
    /// Exists for one caller: adding back a directory that was removed from
    /// the registry. The row is still there — `path` is UNIQUE, so it has to
    /// be — and the user is entitled to re-add a directory Autome never
    /// touched on its way out (P-08). Everything the add decided is written,
    /// including `removed_at: None`, which is what brings it back.
    pub fn replace_project(&self, project: &Project) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE projects SET path = ?2, display_name = ?3, default_branch = ?4,
                                 parallel_limit = ?5, onboarding = ?6, disposition = ?7,
                                 added_at = ?8, removed_at = ?9,
                                 kind = ?10, members = ?11, docs_repo = ?12
             WHERE id = ?1",
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
                serde_json::to_string(&project.kind)?,
                serde_json::to_string(&project.members)?,
                project.docs_repo,
            ],
        )?;
        self.require_one(n, "项目", &project.id)
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
                                    archived_at, protocol_ref, rules_hash, metrics, doc_root, kind)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                         ?17, ?18)",
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
                    task.doc_root,
                    task.kind.as_str(),
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

    pub fn create_task(&self, task: &TaskRecord, brief: Option<&str>) -> Result<()> {
        self.create_task_after(task, brief, &[])
    }

    pub fn create_task_after(
        &self,
        task: &TaskRecord,
        brief: Option<&str>,
        after: &[String],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        self.insert_task(task)?;
        if matches!(task.state, TaskState::Queued) {
            self.mark_queued(&task.id)?;
        }
        if let Some(body) = brief {
            tx.execute(
                "INSERT INTO task_briefs (task_id, body, created_at) VALUES (?1, ?2, ?3)",
                params![task.id, body, task.created_at],
            )?;
        }
        for (ord, id) in after.iter().enumerate() {
            tx.execute(
                "INSERT INTO task_after (task_id, after_id, ord) VALUES (?1, ?2, ?3)",
                params![task.id, id, ord as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Publish the replacement and cancellation together, including their events.
    pub fn convert_task(
        &self,
        from: &str,
        task: &TaskRecord,
        brief: Option<&str>,
    ) -> Result<[u64; 3]> {
        let tx = self.conn.unchecked_transaction()?;
        self.insert_task(task)?;
        if matches!(task.state, TaskState::Queued) {
            self.mark_queued(&task.id)?;
        }
        if let Some(body) = brief {
            tx.execute(
                "INSERT INTO task_briefs (task_id, body, created_at) VALUES (?1, ?2, ?3)",
                params![task.id, body, task.created_at],
            )?;
        }
        self.set_task_state(from, &TaskState::Cancelled)?;
        self.set_task_archived(from, true)?;
        let created = self.append_event(
            "task.created",
            &task.id,
            serde_json::json!({"project_id": task.project_id, "converted_from": from}),
        )?;
        let converted = self.append_event(
            "task.converted",
            from,
            serde_json::json!({"from": from, "to": task.id}),
        )?;
        let updated = self.append_event(
            "task.updated",
            from,
            serde_json::json!({"state": TaskState::Cancelled, "trigger": "cancel"}),
        )?;
        tx.commit()?;
        Ok([created, converted, updated])
    }

    pub fn task_brief(&self, id: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT body FROM task_briefs WHERE task_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn task_after(&self, id: &str) -> Result<Vec<String>> {
        Ok(self
            .conn
            .prepare("SELECT after_id FROM task_after WHERE task_id = ?1 ORDER BY ord")?
            .query_map([id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_task_after(&self, id: &str, after: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for old in self.task_after(id)? {
            if !after.contains(&old) {
                tx.execute(
                    "DELETE FROM task_after WHERE task_id = ?1 AND after_id = ?2",
                    params![id, old],
                )?;
            }
        }
        for (ord, on) in after.iter().enumerate() {
            tx.execute("INSERT INTO task_after (task_id, after_id, ord) VALUES (?1, ?2, ?3) ON CONFLICT(task_id, after_id) DO UPDATE SET ord = excluded.ord", params![id, on, ord as i64])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn project_after(&self, project_id: &str) -> Result<BTreeMap<String, Vec<String>>> {
        let mut result = BTreeMap::new();
        for task in self.list_tasks(project_id)? {
            result.insert(task.id.clone(), self.task_after(&task.id)?);
        }
        Ok(result)
    }

    pub fn blocked_notice(&self, task: &str, on: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT notified FROM task_after WHERE task_id = ?1 AND after_id = ?2",
                params![task, on],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    pub fn set_blocked_notice(&self, task: &str, on: &str, reason: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE task_after SET notified = ?3 WHERE task_id = ?1 AND after_id = ?2",
            params![task, on, reason],
        )?;
        Ok(())
    }

    pub fn has_impl_session(&self, task_id: &str) -> Result<bool> {
        let kind = serde_json::to_string(&SessionKind::Role {
            role: autome_domain::role::Role::Impl,
        })?;
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE task_id = ?1 AND kind = ?2)",
            params![task_id, kind],
            |r| r.get(0),
        )?)
    }

    pub fn queued_seq(&self, id: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row("SELECT queued_seq FROM tasks WHERE id = ?1", [id], |r| {
                r.get(0)
            })?)
    }

    pub fn queue_order(&self, project_id: &str) -> Result<Vec<TaskRecord>> {
        let mut stmt = self.conn.prepare(&format!("{TASK_SELECT_BASE} WHERE project_id = ?1 ORDER BY queued_seq IS NOT NULL, queued_seq, created_at"))?;
        let tasks = stmt
            .query_map([project_id], row_to_task)?
            .map(|row| Ok(row??))
            .collect::<Result<Vec<_>>>()?;
        let mut order = Vec::new();
        for task in tasks {
            if matches!(task.state, TaskState::Queued | TaskState::Requeued { .. })
                || (task.state == TaskState::Draft && self.queued_seq(&task.id)?.is_some())
            {
                order.push(task);
            }
        }
        Ok(order)
    }

    pub fn reorder_queue(&self, project_id: &str, ordered: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let base: i64 =
            tx.query_row("SELECT COALESCE(MAX(queued_seq), 0) FROM tasks", [], |r| {
                r.get(0)
            })?;
        for (i, id) in ordered.iter().enumerate() {
            let seq = base
                .checked_add(i as i64)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| StoreError::Conflict {
                    detail: "队列序号已达到上限".into(),
                })?;
            tx.execute(
                "UPDATE tasks SET queued_seq = ?3 WHERE id = ?1 AND project_id = ?2",
                params![id, project_id, seq],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn mark_queued(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET queued_seq = (SELECT COALESCE(MAX(queued_seq), 0) + 1 FROM tasks) WHERE id = ?1",
            params![id],
        )?;
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

    /// Publish the task-copy baseline and its scheme marker atomically. This
    /// changes hash attribution only; it never rewrites the frozen files.
    pub fn record_task_protocol_scheme(&self, id: &str, wire: &str, payload: Value) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        if self.last_event(id, "protocol.hash_scheme")?.is_none() {
            tx.execute(
                "UPDATE tasks SET protocol_ref = ?2 WHERE id = ?1",
                params![id, wire],
            )?;
            self.append_event("protocol.hash_scheme", id, payload)?;
        }
        tx.commit()?;
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
        let mut stmt = self.conn.prepare(&format!(
            "{TASK_SELECT_BASE} WHERE project_id = ?1 ORDER BY queued_seq IS NOT NULL, queued_seq, created_at"
        ))?;
        let tasks = stmt
            .query_map(params![project_id], row_to_task)?
            .map(|row| Ok(row??))
            .collect::<Result<Vec<_>>>()?;
        Ok(tasks
            .into_iter()
            .filter(|t| matches!(t.state, TaskState::Queued | TaskState::Requeued { .. }))
            .collect())
    }

    // -----------------------------------------------------------------
    // Sessions
    // -----------------------------------------------------------------

    pub fn set_session_pid(&self, id: &str, pid: i32) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE sessions SET pid = ?2 WHERE id = ?1",
            params![id, pid],
        )?;
        self.require_one(n, "会话", id)
    }

    pub fn insert_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, task_id, kind, runtime, model, effort, skills, round,
                                   started_at, ended_at, lifecycle, log_path, pid,
                                   protocol_ref, rules_hash, limit_minutes, project_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                session.id,
                (session.kind != SessionKind::Onboarding).then_some(&session.task_id),
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
                session.limit_minutes,
                session.project_id,
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

    pub fn get_session(&self, id: &str) -> Result<Option<Session>> {
        Ok(self
            .conn
            .query_row(
                &format!("{SESSION_SELECT_BASE} WHERE id = ?1"),
                params![id],
                row_to_session,
            )
            .optional()?
            .transpose()?)
    }

    pub fn latest_onboarding_session(&self, project_id: &str) -> Result<Option<Session>> {
        Ok(self.conn.query_row(&format!("{SESSION_SELECT_BASE} WHERE project_id = ?1 AND json_extract(kind, '$.kind') = 'onboarding' ORDER BY started_at DESC, rowid DESC LIMIT 1"), params![project_id], row_to_session).optional()?.transpose()?)
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

    /// All sessions belonging to a project, without ambiguous joined columns.
    pub fn sessions_in_project(&self, project_id: &str) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SESSION_SELECT_BASE} WHERE task_id IN (SELECT id FROM tasks WHERE project_id = ?1) ORDER BY started_at DESC"
        ))?;
        let rows = stmt.query_map(params![project_id], row_to_session)?;
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
            "SELECT MAX(round) FROM sessions WHERE task_id = ?1 AND kind = ?2 AND json_extract(lifecycle, '$.state') IS NOT 'timed_out'",
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
                 AND disposition = ?3 AND consumed_at IS NULL AND carried_by IS NULL
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
        self.query_decisions("task_id = ?1", task_id)
    }

    fn query_decisions(&self, filter: &str, key: &str) -> Result<Vec<DecisionRecord>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT task_id, kind, item_id, text, disposition, ruling, consumed_at, carried_by
             FROM decisions WHERE {filter} ORDER BY kind, item_id"
        ))?;
        let rows = stmt.query_map(params![key], |r| {
            let disposition: String = r.get(4)?;
            Ok((
                (
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ),
                disposition,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let ((task_id, kind, item_id, text), disposition, ruling, consumed_at, carried_by) =
                row?;
            out.push(DecisionRecord {
                task_id,
                kind,
                item_id,
                text,
                disposition: serde_json::from_str(&disposition)?,
                ruling,
                consumed_at,
                carried_by,
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

    /// Hands a session the decided items nobody is carrying yet, and marks
    /// them as its own. The session's prompt and brief get exactly this list.
    ///
    /// Marking rather than consuming: the decisions are applied by the round
    /// that receives them, so they are only spent when that round ends well
    /// (`settle_decisions`). A round that crashes hands them back
    /// (`release_decisions`) and the next one carries them again.
    pub fn carry_decisions(&self, task_id: &str, session_id: &str) -> Result<Vec<DecisionRecord>> {
        let none = serde_json::to_string(&Disposition::None)?;
        self.conn.execute(
            "UPDATE decisions SET carried_by = ?2
             WHERE task_id = ?1 AND consumed_at IS NULL AND carried_by IS NULL
               AND disposition != ?3",
            params![task_id, session_id, none],
        )?;
        self.query_decisions("carried_by = ?1", session_id)
    }

    /// The session carrying these ended well: they have been applied.
    pub fn settle_decisions(&self, session_id: &str) -> Result<u32> {
        let n = self.conn.execute(
            "UPDATE decisions SET consumed_at = ?2, consumed_by = ?1, carried_by = NULL
             WHERE carried_by = ?1 AND consumed_at IS NULL",
            params![session_id, now_iso()],
        )?;
        Ok(n as u32)
    }

    /// The session carrying these did not finish its round: give them back.
    pub fn release_decisions(&self, session_id: &str) -> Result<u32> {
        let n = self.conn.execute(
            "UPDATE decisions SET carried_by = NULL WHERE carried_by = ?1 AND consumed_at IS NULL",
            params![session_id],
        )?;
        Ok(n as u32)
    }

    // -----------------------------------------------------------------
    // Runtime cooldowns
    // -----------------------------------------------------------------

    /// Records that a CLI said "later". A second strike while the first is
    /// recent doubles the wait, up to an hour; `reset_at`, when the CLI said
    /// when, wins over the guess.
    pub fn cool_down(
        &self,
        runtime: Runtime,
        reason: &str,
        reset_at: Option<i64>,
    ) -> Result<String> {
        let now = epoch_now();
        let previous: Option<(String, i64)> = self
            .conn
            .query_row(
                "SELECT until, strikes FROM runtime_cooldowns WHERE runtime = ?1",
                params![runtime.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        // "Recent" is within an hour of the last cooldown ending: a limit hit
        // again the day after is a new day, not a second strike.
        let strikes = match previous {
            Some((until, n)) if until >= format_iso(now - 3600) => n + 1,
            _ => 1,
        };
        let minutes = (COOLDOWN_BASE_MINUTES << (strikes - 1).min(6)).min(COOLDOWN_MAX_MINUTES);
        let guessed = now + minutes * 60;
        let until = format_iso(reset_at.filter(|t| *t > now).unwrap_or(guessed));
        self.conn.execute(
            "INSERT INTO runtime_cooldowns (runtime, until, reason, strikes) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (runtime) DO UPDATE SET until = excluded.until,
                 reason = excluded.reason, strikes = excluded.strikes",
            params![runtime.as_str(), until, reason, strikes],
        )?;
        Ok(until)
    }

    /// When this CLI may be used again, if it is cooling down now.
    pub fn cooling_until(&self, runtime: Runtime) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT until FROM runtime_cooldowns WHERE runtime = ?1 AND until > ?2",
                params![runtime.as_str(), now_iso()],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Every CLI cooling down now, for the dashboard and `autome status`.
    pub fn active_cooldowns(&self) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT runtime, until, reason FROM runtime_cooldowns WHERE until > ?1 ORDER BY runtime",
        )?;
        let rows = stmt.query_map(params![now_iso()], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// A round on this CLI finished: whatever limit it hit is behind it.
    pub fn clear_cooldown(&self, runtime: Runtime) -> Result<()> {
        self.conn.execute(
            "DELETE FROM runtime_cooldowns WHERE runtime = ?1",
            params![runtime.as_str()],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Session retries
    // -----------------------------------------------------------------

    /// Crashes counted against the round this task is sitting at.
    pub fn retry_attempts(&self, task_id: &str) -> Result<u32> {
        let n: Option<i64> = self
            .conn
            .query_row(
                "SELECT attempts FROM session_retries WHERE task_id = ?1",
                params![task_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(n.unwrap_or(0) as u32)
    }

    /// Schedules the round at `node` to run again at `not_before`. `counted`
    /// is false for a rate limit, which is the CLI's problem, not the round's.
    pub fn schedule_retry(
        &self,
        task_id: &str,
        node: Node,
        not_before: &str,
        inject: Option<&Inject>,
        reason: &str,
        counted: bool,
    ) -> Result<()> {
        let inject = inject.map(serde_json::to_string).transpose()?;
        self.conn.execute(
            "INSERT INTO session_retries (task_id, node, attempts, not_before, inject, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (task_id) DO UPDATE SET node = excluded.node,
                 attempts = session_retries.attempts + ?3, not_before = excluded.not_before,
                 inject = excluded.inject, reason = excluded.reason",
            params![
                task_id,
                node.as_str(),
                counted as i64,
                not_before,
                inject,
                reason
            ],
        )?;
        Ok(())
    }

    /// A retry still waiting to be started: `(node, not_before, inject)`.
    pub fn pending_retry(&self, task_id: &str) -> Result<Option<(Node, String, Option<Inject>)>> {
        let row: Option<(String, String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT node, not_before, inject FROM session_retries
                 WHERE task_id = ?1 AND not_before IS NOT NULL",
                params![task_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((node, not_before, inject)) = row else {
            return Ok(None);
        };
        let Some(node) = Node::parse(&node) else {
            return Ok(None);
        };
        let inject = inject.as_deref().map(serde_json::from_str).transpose()?;
        Ok(Some((node, not_before, inject)))
    }

    /// The retry has been started; the attempt count stays until the task moves.
    pub fn retry_started(&self, task_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE session_retries SET not_before = NULL WHERE task_id = ?1",
            params![task_id],
        )?;
        Ok(())
    }

    /// The task moved on (or stopped): its crash count starts again from zero.
    pub fn clear_retry(&self, task_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM session_retries WHERE task_id = ?1",
            params![task_id],
        )?;
        Ok(())
    }

    // -----------------------------------------------------------------
    // Project alerts
    // -----------------------------------------------------------------

    /// The result is the durable completion marker; publish its alert atomically.
    pub fn conclude_merge_after(
        &self,
        project: &str,
        task: &str,
        payload: Value,
        failure: Option<&str>,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        if let Some(detail) = failure {
            let id = self.raise_alert(project, "after_merge_failed", Some(task), detail)?;
            self.append_event(
                "project.alert",
                project,
                serde_json::json!({"alert_id": id, "kind": "after_merge_failed", "task_id": task}),
            )?;
        }
        self.append_event("merge.after", task, payload)?;
        tx.commit()?;
        Ok(())
    }

    /// The alert and its deduplication/notification events commit together.
    pub fn raise_budget_alert(
        &self,
        project_id: &str,
        task_id: &str,
        detail: &str,
        usd_total: f64,
        limit_cents: u64,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let id = self.raise_alert(project_id, "budget_exceeded", Some(task_id), detail)?;
        self.append_event("task.budget_exceeded", task_id, serde_json::json!({
            "alert_id": id, "usd_total": usd_total, "limit_usd": limit_cents as f64 / 100.0, "limit_cents": limit_cents
        }))?;
        self.append_event(
            "project.alert",
            project_id,
            serde_json::json!({
                "alert_id": id, "kind": "budget_exceeded", "task_id": task_id
            }),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn raise_alert(
        &self,
        project_id: &str,
        kind: &str,
        task_id: Option<&str>,
        detail: &str,
    ) -> Result<String> {
        let id = new_id("alr");
        self.conn.execute(
            "INSERT INTO project_alerts (id, project_id, kind, task_id, detail, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, project_id, kind, task_id, detail, now_iso()],
        )?;
        Ok(id)
    }

    /// Alerts nobody has dismissed yet, newest first.
    pub fn open_alerts(&self) -> Result<Vec<AlertRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, kind, task_id, detail, created_at FROM project_alerts
             WHERE dismissed_at IS NULL ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AlertRecord {
                id: r.get(0)?,
                project_id: r.get(1)?,
                kind: r.get(2)?,
                task_id: r.get(3)?,
                detail: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn dismiss_alert(&self, id: &str) -> Result<()> {
        let n = self.conn.execute(
            "UPDATE project_alerts SET dismissed_at = ?2 WHERE id = ?1 AND dismissed_at IS NULL",
            params![id, now_iso()],
        )?;
        self.require_one(n, "报警", id)
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

/// The project columns, in the order `row_to_project` reads them. One string
/// for three queries: a column added to the table and to two of the three is a
/// project that reads differently depending on how it was looked up.
const PROJECT_SELECT_BASE: &str =
    "SELECT id, path, display_name, default_branch, parallel_limit, onboarding,
            disposition, added_at, removed_at, kind, members, docs_repo
     FROM projects";

const TASK_SELECT_BASE: &str = "SELECT id, project_id, slug, title, request, attachments, doc_refs,
            state, budget_n, created_at, completed_at, merge_commit, archived_at,
            protocol_ref, rules_hash, metrics, doc_root, kind FROM tasks";

const TASK_SELECT: &str = "SELECT id, project_id, slug, title, request, attachments, doc_refs,
            state, budget_n, created_at, completed_at, merge_commit, archived_at,
            protocol_ref, rules_hash, metrics, doc_root, kind
     FROM tasks WHERE id = ?1";

const SESSION_SELECT_BASE: &str =
    "SELECT id, COALESCE(task_id, 'onboarding'), kind, runtime, model, effort, skills, round,
            started_at, ended_at, lifecycle, log_path, pid,
            protocol_ref, rules_hash, input_tokens, cache_read_tokens, cache_write_tokens,
            output_tokens, cost_usd, turns, duration_ms, mean_request_input,
            design_doc_bytes, evidence_files, limit_minutes, project_id FROM sessions";

type RowResult<T> = rusqlite::Result<std::result::Result<T, serde_json::Error>>;

fn row_to_project(r: &rusqlite::Row<'_>) -> RowResult<Project> {
    let onboarding: String = r.get(5)?;
    let disposition: String = r.get(6)?;
    let parallel: i64 = r.get(4)?;
    // NULL in all three is a row written before workspaces existed, and means
    // the only thing a project could be then: one repository.
    let kind: Option<String> = r.get(9)?;
    let members: Option<String> = r.get(10)?;
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
            kind: match kind {
                Some(k) => serde_json::from_str::<ProjectKind>(&k)?,
                None => ProjectKind::default(),
            },
            members: match members {
                Some(m) => serde_json::from_str::<Vec<Member>>(&m)?,
                None => Vec::new(),
            },
            docs_repo: r.get(11)?,
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
    // NULL is every task created before the document root was configurable,
    // and `TaskRecord::doc_root` reads it as the `docs/` those tasks used.
    let doc_root: Option<String> = r.get(16)?;
    let kind: String = r.get(17)?;
    Ok(
        (|| -> std::result::Result<TaskRecord, serde_json::Error> {
            Ok(TaskRecord {
                kind: serde_json::from_value(Value::String(kind))?,
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
                doc_root,
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
    let project_id = r.get(26)?;
    let limit_minutes = r.get::<_, Option<u32>>(25)?;
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
            project_id,
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
            limit_minutes,
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

pub fn epoch_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `now + secs`, in the ledger's timestamp format. The format sorts as text,
/// so "has this time passed" is a string comparison against `now_iso()`.
pub fn iso_in(secs: i64) -> String {
    format_iso(epoch_now() + secs)
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
            TaskState::Draft => "draft",
            TaskState::Queued | TaskState::Requeued { .. } => "queued",
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

    #[test]
    fn gate_report_snapshot_filters_sources_age_order_and_limit() {
        let s = store();
        for id in ["p", "other"] {
            s.insert_project(&project(id, &format!("/tmp/{id}")))
                .unwrap();
        }
        for (id, project_id) in [
            ("own", "p"),
            ("merged", "p"),
            ("open", "p"),
            ("wrong", "p"),
            ("foreign", "other"),
        ] {
            s.insert_task(&task(id, project_id, id)).unwrap();
        }
        for id in ["merged", "wrong", "foreign"] {
            s.complete_task(id, Some("merge")).unwrap();
        }
        let add = |id: &str, key: char, ok: bool, kind: &str, sha: &str| {
            s.append_event("gate.result", id, serde_json::json!({"ok":ok,"kind":kind,"sha":sha,"node":"gate","passes":[{"step":"check","key":key.to_string().repeat(40)}]})).unwrap()
        };
        let old = add("own", 'a', true, "full", "head");
        s.conn
            .execute(
                "UPDATE events SET at = datetime('now', '-7 days', '-1 minute') WHERE seq = ?1",
                [old],
            )
            .unwrap();
        let recent = add("own", 'b', false, "quick", "head");
        s.conn
            .execute(
                "UPDATE events SET at = datetime('now', '-7 days', '+1 minute') WHERE seq = ?1",
                [recent],
            )
            .unwrap();
        add("merged", 'c', true, "full", "head");
        add("merged", 'd', false, "full", "head");
        add("merged", 'e', true, "quick", "head");
        add("open", 'f', true, "full", "head");
        add("wrong", '1', true, "full", "head");
        add("wrong", '2', true, "full", "wrong-head");
        add("foreign", '3', true, "full", "head");
        let snapshot = s.gate_passes("p", "own", |_| Some("head".into())).unwrap();
        let keys: Vec<_> = snapshot
            .lines()
            .map(|l| l.split('\t').next().unwrap())
            .collect();
        assert_eq!(keys, ["c".repeat(40), "b".repeat(40)]);
        assert!(
            s.gate_passes("p", "own", |_| None)
                .unwrap()
                .starts_with(&"b".repeat(40))
        );
        let passes: Vec<_> = (0..5001)
            .map(|n| serde_json::json!({"step":"check","key":format!("{n:040x}")}))
            .collect();
        s.append_event(
            "gate.result",
            "own",
            serde_json::json!({"sha":"head","node":"gate","passes":passes}),
        )
        .unwrap();
        let capped = s.gate_passes("p", "own", |_| Some("head".into())).unwrap();
        assert_eq!(capped.lines().count(), 5000);
        assert!(capped.starts_with(&format!("{:040x}\t", 0)));
        assert!(
            capped
                .lines()
                .last()
                .unwrap()
                .starts_with(&format!("{:040x}\t", 4999))
        );
    }

    #[test]
    fn gate_report_early_fake_key_never_published_after_merge() {
        let s = store();
        s.insert_project(&project("p", "/tmp")).unwrap();
        for id in ["A", "B"] {
            s.insert_task(&task(id, "p", id)).unwrap();
        }
        for (key, sha) in [('a', "final"), ('b', "final")] {
            s.append_event("gate.result", "A", serde_json::json!({"ok":true,"kind":"full","sha":sha,"node":"merge_gate","passes":[{"step":"test","key":key.to_string().repeat(40)}]})).unwrap();
        }
        assert_eq!(
            s.gate_passes("p", "B", |_| Some("final".into())).unwrap(),
            ""
        );
        s.complete_task("A", Some("merge")).unwrap();
        let b = s.gate_passes("p", "B", |_| Some("final".into())).unwrap();
        assert!(b.contains(&"b".repeat(40)));
        assert!(!b.contains(&"a".repeat(40)));
        let a = s.gate_passes("p", "A", |_| Some("final".into())).unwrap();
        assert!(a.contains(&"a".repeat(40)) && a.contains(&"b".repeat(40)));
    }

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    /// WAL mode leaves `-wal` and `-shm` siblings next to the database.
    /// Removing only the `.sqlite3` leaves two files behind per run, which is
    /// how a test suite quietly fills a temp directory.
    pub(super) fn remove_db(path: &std::path::Path) {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = path.as_os_str().to_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(p));
        }
    }

    fn downgrade_sessions_to_v7(s: &Store) {
        let schema: String = s
            .conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'sessions'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let old = schema.split(", project_id").next().unwrap();
        let old = format!(
            "{})",
            old.replace("CREATE TABLE \"sessions\"", "CREATE TABLE sessions_old")
                .replace(
                    "task_id TEXT REFERENCES",
                    "task_id TEXT NOT NULL REFERENCES"
                )
        );
        s.conn.execute_batch(&old).unwrap();
        let columns: Vec<String> = s
            .conn
            .prepare("PRAGMA table_info(sessions_old)")
            .unwrap()
            .query_map([], |r| r.get(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        s.conn.execute_batch(&format!("INSERT INTO sessions_old SELECT {} FROM sessions; DROP TABLE sessions; ALTER TABLE sessions_old RENAME TO sessions; CREATE INDEX sessions_by_task ON sessions (task_id, started_at); PRAGMA user_version = 7;", columns.join(","))).unwrap();
    }

    #[test]
    fn onboarding_session_migration_v9_keeps_existing_rows() {
        onboarding_migration_keeps_existing_rows(7);
    }

    #[test]
    fn onboarding_session_migration_from_v8_preserves_kernel_mark() {
        onboarding_migration_keeps_existing_rows(8);
    }

    fn onboarding_migration_keeps_existing_rows(version: i64) {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        let mut original = session("old", "T-1", SessionKind::Intake, 3);
        original.protocol_ref = Some("protocol/v8@frozen".into());
        original.rules_hash = Some("rules".into());
        original.limit_minutes = Some(17);
        original.metrics.cost_usd = Some(2.5);
        original.metrics.input_tokens = Some(99);
        s.insert_session(&original).unwrap();
        s.set_session_metrics(&original.id, &original.metrics)
            .unwrap();
        // Recreate the exact v7 column layout before replaying the real migration.
        downgrade_sessions_to_v7(&s);
        s.conn.pragma_update(None, "user_version", version).unwrap();
        s.claim_kernel_mark("0000002-20200101T000000Z-000000000002")
            .unwrap();
        s.migrate().unwrap();
        assert_eq!(s.get_session("old").unwrap().unwrap(), original);
        assert_eq!(
            s.conn
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            9
        );
        assert_eq!(
            s.kernel_mark().unwrap().as_deref(),
            Some("0000002-20200101T000000Z-000000000002")
        );
        assert!(
            s.conn
                .query_row(
                    "SELECT name FROM sqlite_master WHERE name = 'sessions_by_task'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .is_ok()
        );
        let mut onboarding = session("new", "onboarding", SessionKind::Onboarding, 0);
        onboarding.project_id = Some("p1".into());
        s.insert_session(&onboarding).unwrap();
        assert!(
            s.conn
                .query_row(
                    "SELECT task_id IS NULL FROM sessions WHERE id = 'new'",
                    [],
                    |r| r.get::<_, bool>(0)
                )
                .unwrap()
        );
        assert!(
            s.conn
                .execute("UPDATE sessions SET task_id = 'T-1' WHERE id = 'new'", [])
                .is_err()
        );
        assert!(
            s.conn
                .execute("UPDATE sessions SET task_id = NULL WHERE id = 'old'", [])
                .is_err()
        );
        onboarding.id = "missing-project".into();
        onboarding.project_id = None;
        assert!(s.insert_session(&onboarding).is_err());
        onboarding.project_id = Some("unknown".into());
        assert!(s.insert_session(&onboarding).is_err());
        assert!(
            s.conn
                .prepare("PRAGMA foreign_key_check")
                .unwrap()
                .query([])
                .unwrap()
                .next()
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn session_timeout_store_migration_limit_and_pid() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        let mut row = session("s1", "T-1", SessionKind::Intake, 1);
        s.insert_session(&row).unwrap();
        downgrade_sessions_to_v7(&s);
        // A genuine version-5 row survives the new nullable-column migration.
        s.conn
            .execute_batch(
                "ALTER TABLE sessions DROP COLUMN limit_minutes; PRAGMA user_version = 5;",
            )
            .unwrap();
        s.migrate().unwrap();
        assert_eq!(s.list_sessions("T-1").unwrap()[0].limit_minutes, None);
        row.id = "s2".into();
        row.limit_minutes = Some(u32::MAX);
        s.insert_session(&row).unwrap();
        s.set_session_pid("s2", 1234).unwrap();
        let rows = s.list_sessions("T-1").unwrap();
        let saved = rows.iter().find(|r| r.id == "s2").unwrap();
        assert_eq!(saved.limit_minutes, Some(u32::MAX));
        assert_eq!(saved.pid, Some(1234));
        assert!(s.set_session_pid("missing", 1234).is_err());
    }

    #[test]
    fn merge_after_failure_alert_and_result_event_are_written_in_one_transaction() {
        let s = Store::open_in_memory().unwrap();
        s.insert_project(&project("p", "/tmp/repo")).unwrap();
        s.insert_task(&task("T-1", "p", "hook")).unwrap();
        let payload = serde_json::json!({"ok": false, "exit_code": 6});
        s.conclude_merge_after("p", "T-1", payload.clone(), Some("failed"))
            .unwrap();
        assert_eq!(s.open_alerts().unwrap().len(), 1);
        assert_eq!(s.open_alerts().unwrap()[0].kind, "after_merge_failed");
        assert_eq!(s.count_events("p", "project.alert").unwrap(), 1);
        assert_eq!(s.count_events("T-1", "merge.after").unwrap(), 1);
        s.conn.execute_batch("CREATE TRIGGER reject_merge_after BEFORE INSERT ON events WHEN NEW.kind = 'merge.after' BEGIN SELECT RAISE(ABORT, 'write failure'); END;").unwrap();
        assert!(
            s.conclude_merge_after("p", "T-1", payload, Some("failed"))
                .is_err()
        );
        assert_eq!(s.open_alerts().unwrap().len(), 1);
        assert_eq!(s.count_events("p", "project.alert").unwrap(), 1);
        assert_eq!(s.count_events("T-1", "merge.after").unwrap(), 1);
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
            kind: ProjectKind::Repo,
            members: Vec::new(),
            docs_repo: None,
        }
    }

    fn task(id: &str, project_id: &str, slug: &str) -> TaskRecord {
        TaskRecord {
            kind: TaskKind::Feature,
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
            doc_root: None,
        }
    }

    fn session(id: &str, task_id: &str, kind: SessionKind, round: u32) -> Session {
        Session {
            project_id: None,
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
            limit_minutes: None,
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
        }
    }

    fn deps_fixture() -> Store {
        let s = store();
        s.insert_project(&project("p", "/p")).unwrap();
        for id in ["A", "B", "C", "D"] {
            s.create_task(&task(id, "p", id), None).unwrap();
        }
        s
    }

    #[test]
    fn adoption_event_failure_rolls_back_mark() {
        let s = Store::open_in_memory().unwrap();
        let old = "0000001-20200101T000000Z-000000000001";
        let new = "0000002-20200101T000000Z-000000000002";
        s.claim_kernel_mark(new).unwrap();
        s.conn.execute_batch("CREATE TRIGGER reject_adoption BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT, 'event unavailable'); END;").unwrap();
        assert!(
            s.claim_kernel_mark_with_adoption(old, Some("scheduler.tick"))
                .is_err()
        );
        assert_eq!(s.kernel_mark().unwrap().as_deref(), Some(new));
        assert_eq!(s.latest_seq().unwrap(), 0);
        s.conn
            .execute_batch("DROP TRIGGER reject_adoption")
            .unwrap();
        assert_eq!(
            s.claim_kernel_mark_with_adoption(old, Some("scheduler.tick"))
                .unwrap(),
            (None, true)
        );
        assert_eq!(s.kernel_mark().unwrap().as_deref(), Some(old));
        assert_eq!(s.events_since(0, 10).unwrap().len(), 1);
    }

    #[test]
    fn kernel_mark_migration_preserves_queue() {
        let fixture = deps_fixture();
        let path = std::env::temp_dir().join(format!("kernel-mark-{}.db", new_id("test")));
        let s = Store::open(&path).unwrap();
        s.insert_project(&fixture.get_project("p").unwrap())
            .unwrap();
        for task in fixture.list_tasks("p").unwrap() {
            s.create_task(&task, None).unwrap();
        }
        let before = s.queued_tasks("p").unwrap();
        assert!(!before.is_empty());
        let seqs: Vec<_> = before
            .iter()
            .map(|t| s.queued_seq(&t.id).unwrap())
            .collect();
        downgrade_sessions_to_v7(&s);
        s.conn
            .execute_batch("DROP TABLE kernel_mark; PRAGMA user_version = 7;")
            .unwrap();
        drop(s);
        let s = Store::open(&path).unwrap();
        assert_eq!(s.kernel_mark().unwrap(), None);
        assert_eq!(
            s.conn
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
        assert_eq!(s.queued_tasks("p").unwrap(), before);
        for (task, seq) in before.iter().zip(seqs) {
            assert_eq!(s.queued_seq(&task.id).unwrap(), seq);
        }
        drop(s);
        remove_db(&path);
    }

    #[test]
    fn deps_store_migration_preserves_queue() {
        let s = deps_fixture();
        let mut existing = session("existing", "A", SessionKind::Intake, 1);
        existing.limit_minutes = Some(42);
        s.insert_session(&existing).unwrap();
        s.conn
            .execute("UPDATE tasks SET queued_seq = NULL WHERE id = 'C'", [])
            .unwrap();
        let before = s.queued_tasks("p").unwrap();
        let seqs: Vec<_> = before
            .iter()
            .map(|t| s.queued_seq(&t.id).unwrap())
            .collect();
        downgrade_sessions_to_v7(&s);
        s.conn
            .execute_batch("DROP TABLE task_after; PRAGMA user_version = 6;")
            .unwrap();
        s.migrate().unwrap();
        assert_eq!(
            s.conn
                .pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
        assert_eq!(s.list_sessions("A").unwrap()[0].limit_minutes, Some(42));
        assert_eq!(s.queued_tasks("p").unwrap(), before);
        for (t, seq) in before.iter().zip(seqs) {
            assert!(s.task_after(&t.id).unwrap().is_empty());
            assert_eq!(s.queued_seq(&t.id).unwrap(), seq);
        }
        s.migrate().unwrap();
    }

    #[test]
    fn deps_store_create_is_atomic() {
        let s = deps_fixture();
        let t = task("E", "p", "E");
        assert!(
            s.create_task_after(&t, Some("brief"), &["A".into(), "missing".into()])
                .is_err()
        );
        assert!(s.get_task("E").is_err());
        assert_eq!(s.task_brief("E").unwrap(), None);
        assert!(s.task_after("E").unwrap().is_empty());
        s.create_task_after(&t, Some("brief"), &["B".into(), "A".into()])
            .unwrap();
        assert_eq!(s.task_after("E").unwrap(), ["B", "A"]);
        assert_eq!(s.task_brief("E").unwrap().as_deref(), Some("brief"));
        assert!(s.queued_seq("E").unwrap().is_some());
    }

    #[test]
    fn deps_store_replace_clear_and_impl_history() {
        let s = deps_fixture();
        s.set_task_after("A", &["B".into(), "C".into()]).unwrap();
        s.set_blocked_notice("A", "B", Some("failed")).unwrap();
        s.set_task_after("A", &["B".into(), "C".into()]).unwrap();
        assert_eq!(
            s.blocked_notice("A", "B").unwrap().as_deref(),
            Some("failed")
        );
        s.set_task_after("A", &["D".into(), "B".into()]).unwrap();
        assert_eq!(s.task_after("A").unwrap(), ["D", "B"]);
        assert_eq!(
            s.blocked_notice("A", "B").unwrap().as_deref(),
            Some("failed")
        );
        assert_eq!(s.blocked_notice("A", "D").unwrap(), None);
        assert_eq!(s.blocked_notice("A", "C").unwrap(), None);
        assert_eq!(s.project_after("p").unwrap()["A"], ["D", "B"]);
        assert!(s.set_task_after("A", &["missing".into()]).is_err());
        assert_eq!(s.task_after("A").unwrap(), ["D", "B"]);
        assert_eq!(
            s.blocked_notice("A", "B").unwrap().as_deref(),
            Some("failed")
        );
        s.set_task_after("A", &[]).unwrap();
        assert!(s.task_after("A").unwrap().is_empty());
        assert!(!s.has_impl_session("A").unwrap());
        s.insert_session(&session("intake", "A", SessionKind::Intake, 1))
            .unwrap();
        assert!(!s.has_impl_session("A").unwrap());
        s.insert_session(&session(
            "impl",
            "A",
            SessionKind::Role {
                role: autome_domain::role::Role::Impl,
            },
            1,
        ))
        .unwrap();
        s.set_task_state("A", &TaskState::Active { node: Node::Design })
            .unwrap();
        assert!(s.has_impl_session("A").unwrap());
        assert!(!s.has_impl_session("B").unwrap());
    }

    #[test]
    fn deps_store_reorder_is_project_local() {
        let s = deps_fixture();
        s.insert_project(&project("q", "/q")).unwrap();
        s.create_task(&task("X", "q", "X"), None).unwrap();
        let other = s.queued_seq("X").unwrap();
        s.set_task_state("D", &TaskState::Draft).unwrap();
        s.conn
            .execute("UPDATE tasks SET queued_seq = NULL WHERE id = 'D'", [])
            .unwrap();
        s.set_task_state(
            "C",
            &TaskState::Requeued {
                resume: Node::Design,
            },
        )
        .unwrap();
        assert_eq!(
            s.queue_order("p")
                .unwrap()
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "C"]
        );
        s.reorder_queue("p", &["D".into(), "C".into(), "B".into(), "A".into()])
            .unwrap();
        assert_eq!(
            s.queue_order("p")
                .unwrap()
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["D", "C", "B", "A"]
        );
        assert_eq!(
            s.queued_tasks("p")
                .unwrap()
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["C", "B", "A"]
        );
        assert_eq!(s.queued_seq("X").unwrap(), other);
        let before = s.queued_seq("A").unwrap();
        s.reorder_queue("p", &[]).unwrap();
        assert_eq!(s.queued_seq("A").unwrap(), before);
        s.conn
            .execute(
                "UPDATE tasks SET queued_seq = ?1 WHERE id = 'X'",
                [i64::MAX - 1],
            )
            .unwrap();
        assert!(s.reorder_queue("p", &["A".into(), "B".into()]).is_err());
        assert_eq!(s.queued_seq("A").unwrap(), before);
    }

    #[test]
    fn deps_store_blocked_notice_round_trip() {
        let s = deps_fixture();
        s.set_task_after("A", &["B".into()]).unwrap();
        for reason in [
            None,
            Some("failed"),
            None,
            Some("failed"),
            Some("cancelled"),
            None,
        ] {
            s.set_blocked_notice("A", "B", reason).unwrap();
            assert_eq!(s.blocked_notice("A", "B").unwrap().as_deref(), reason);
        }
    }

    // ---- projects --------------------------------------------------------

    #[test]
    fn budget_alert_record_is_all_or_nothing() {
        for failure in ["none", "missing_events", "second_event"] {
            let s = store();
            s.insert_project(&project("p", "/p")).unwrap();
            s.insert_task(&task("T-1", "p", "t")).unwrap();
            if failure == "missing_events" {
                s.conn.execute_batch("DROP TABLE events").unwrap();
            } else if failure == "second_event" {
                s.conn.execute_batch("CREATE TRIGGER fail_alert BEFORE INSERT ON events WHEN NEW.kind = 'project.alert' BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
            }
            let result = s.raise_budget_alert("p", "T-1", "detail", 0.5, 40);
            if failure == "none" {
                result.unwrap();
                let alerts = s.open_alerts().unwrap();
                assert_eq!(alerts.len(), 1);
                assert_eq!(s.count_events("T-1", "task.budget_exceeded").unwrap(), 1);
                assert_eq!(s.count_events("p", "project.alert").unwrap(), 1);
                let event = s
                    .last_event("T-1", "task.budget_exceeded")
                    .unwrap()
                    .unwrap();
                assert_eq!(event["alert_id"], alerts[0].id);
                assert_eq!(event["limit_cents"], 40);
                assert_eq!(event["limit_usd"], 0.4);
                assert_eq!(event["usd_total"], 0.5);
                s.dismiss_alert(&alerts[0].id).unwrap();
                assert!(!autome_domain::metrics::budget_crossed(
                    1.0,
                    40,
                    event["limit_cents"].as_u64()
                ));
            } else {
                assert!(result.is_err());
                assert!(s.open_alerts().unwrap().is_empty());
                if failure == "second_event" {
                    assert_eq!(s.count_events("T-1", "task.budget_exceeded").unwrap(), 0);
                    assert_eq!(s.count_events("p", "project.alert").unwrap(), 0);
                }
            }
        }
    }

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
    fn requeued_old_ledger_rows_and_new_rows_roundtrip() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        for (raw, expected) in [
            (r#"{"state":"queued"}"#, TaskState::Queued),
            (
                r#"{"state":"paused","resume":"audit"}"#,
                TaskState::Paused {
                    resume: Node::Audit,
                },
            ),
            (
                r#"{"state":"stopped","at":"gate"}"#,
                TaskState::Stopped { at: Node::Gate },
            ),
        ] {
            s.conn
                .execute("UPDATE tasks SET state = ?1 WHERE id = 'T-1'", params![raw])
                .unwrap();
            assert_eq!(s.get_task("T-1").unwrap().state, expected);
        }
        let state = TaskState::Requeued {
            resume: Node::Implement,
        };
        s.set_task_state("T-1", &state).unwrap();
        assert_eq!(s.get_task("T-1").unwrap().state, state);
        let raw: String = s
            .conn
            .query_row("SELECT state FROM tasks WHERE id = 'T-1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&raw).unwrap(),
            serde_json::json!({"state":"queued","resume":"implement"})
        );
        assert_eq!(SCHEMA_VERSION, 9);
    }

    #[test]
    fn requeued_counts_and_queue_order_do_not_consume_slots() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        for (i, state) in [
            TaskState::Requeued {
                resume: Node::Audit,
            },
            TaskState::Queued,
            TaskState::Active {
                node: Node::Implement,
            },
        ]
        .into_iter()
        .enumerate()
        {
            let mut t = task(&format!("T-{i}"), "p1", &format!("s{i}"));
            t.state = state;
            t.created_at = format!("2026-11-10T00:00:0{i}Z");
            s.insert_task(&t).unwrap();
        }
        assert_eq!(
            s.queued_tasks("p1")
                .unwrap()
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            vec!["T-0", "T-1"]
        );
        assert_eq!(task_counts(&s.list_tasks("p1").unwrap())["queued"], 2);
        assert_eq!(s.slots_in_use("p1").unwrap(), 1);
        assert!(s.queued_tasks("missing").unwrap().is_empty());
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
    fn a_session_carries_only_decided_unconsumed_items() {
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
        let carried = s.carry_decisions("T-1", "ses-1").unwrap();
        assert_eq!(carried.len(), 1);
        assert_eq!(carried[0].item_id, "B-01");
        assert_eq!(carried[0].carried_by.as_deref(), Some("ses-1"));
        // Carried is not consumed: the transition table still counts it.
        assert_eq!(s.pending_decisions("T-1").unwrap().included, 1);
        // And a second session does not get the same item.
        assert!(s.carry_decisions("T-1", "ses-2").unwrap().is_empty());
    }

    #[test]
    fn a_crashed_session_hands_its_decisions_back_and_a_good_one_spends_them() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions("T-1", &[("B-01".into(), "x".into())], &[])
            .unwrap();
        s.set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();

        s.carry_decisions("T-1", "ses-1").unwrap();
        assert_eq!(s.release_decisions("ses-1").unwrap(), 1);
        let again = s.carry_decisions("T-1", "ses-2").unwrap();
        assert_eq!(
            again.len(),
            1,
            "the next round carries what the crash dropped"
        );

        assert_eq!(s.settle_decisions("ses-2").unwrap(), 1);
        assert_eq!(s.pending_decisions("T-1").unwrap().included, 0);
        assert!(s.carry_decisions("T-1", "ses-3").unwrap().is_empty());
        // Settled rows are not handed back by a late release either.
        assert_eq!(s.release_decisions("ses-2").unwrap(), 0);
    }

    #[test]
    fn a_carried_item_survives_the_document_dropping_it() {
        let s = store();
        s.insert_project(&project("p1", "/a")).unwrap();
        s.insert_task(&task("T-1", "p1", "a")).unwrap();
        s.sync_decisions("T-1", &[("B-01".into(), "x".into())], &[])
            .unwrap();
        s.set_disposition("T-1", "backlog", "B-01", Disposition::Include, None)
            .unwrap();
        s.carry_decisions("T-1", "ses-1").unwrap();
        s.sync_decisions("T-1", &[], &[]).unwrap();
        assert_eq!(s.list_decisions("T-1").unwrap().len(), 1);
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
    fn stored_event_includes_original_timestamp_and_query_boundaries() {
        let s = store();
        assert!(s.events_since_with_at(0, 1).unwrap().is_empty());
        let seq = s
            .append_event("test", "subject", serde_json::json!({"a":1}))
            .unwrap();
        s.conn
            .execute("UPDATE events SET at = '2000-02-29T23:59:59Z'", [])
            .unwrap();
        let events = s.events_since_with_at(0, 1).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].seq, seq);
        assert_eq!(events[0].at, "2000-02-29T23:59:59Z");
        assert_eq!(events[0].payload, serde_json::json!({"a":1}));
        assert!(s.events_since_with_at(seq, 1).unwrap().is_empty());
        assert!(s.events_since_with_at(0, 0).unwrap().is_empty());
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
    mod task_card {
        use super::*;

        #[test]
        fn a_v4_ledger_migrates_and_every_old_task_reads_as_a_feature() {
            let s = store();
            s.insert_project(&project("p1", "/repo")).unwrap();
            let states = [
                TaskState::Queued,
                TaskState::Requeued {
                    resume: Node::Design,
                },
                TaskState::Active { node: Node::Intake },
                TaskState::Paused {
                    resume: Node::Review,
                },
                TaskState::Stopped {
                    at: Node::Implement,
                },
                TaskState::Failed {
                    at: Node::Design,
                    reason: FailureReason::Config {
                        detail: "old failure".into(),
                    },
                },
                TaskState::Done,
                TaskState::Cancelled,
            ];
            for (i, state) in states.iter().enumerate() {
                let id = format!("T-{i}");
                let mut t = task(&id, "p1", &id);
                t.state = state.clone();
                s.insert_task(&t).unwrap();
            }
            downgrade_sessions_to_v7(&s);
            s.conn.execute_batch("ALTER TABLE sessions DROP COLUMN limit_minutes; ALTER TABLE tasks DROP COLUMN kind; ALTER TABLE tasks DROP COLUMN queued_seq; DROP TABLE task_briefs; PRAGMA user_version = 4;").unwrap();
            s.migrate().unwrap();
            let version: i64 = s
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(version, SCHEMA_VERSION);
            for (i, state) in states.iter().enumerate() {
                let id = format!("T-{i}");
                let t = s.get_task(&id).unwrap();
                assert_eq!(&t.state, state);
                assert_eq!(t.kind, TaskKind::Feature);
                assert_eq!(s.task_brief(&id).unwrap(), None);
                let seq: Option<i64> = s
                    .conn
                    .query_row("SELECT queued_seq FROM tasks WHERE id = ?1", [&id], |r| {
                        r.get(0)
                    })
                    .unwrap();
                assert_eq!(seq, None);
            }
        }

        #[test]
        fn kind_round_trips_through_the_ledger() {
            let s = store();
            s.insert_project(&project("p1", "/repo")).unwrap();
            for kind in [TaskKind::Feature, TaskKind::Bug] {
                let mut t = task(kind.as_str(), "p1", kind.as_str());
                t.kind = kind;
                s.insert_task(&t).unwrap();
                assert_eq!(s.get_task(&t.id).unwrap(), t);
            }
            let tasks = s.list_tasks("p1").unwrap();
            assert!(tasks.iter().any(|t| t.kind == TaskKind::Bug));
            assert!(
                s.list_unfinished()
                    .unwrap()
                    .iter()
                    .any(|t| t.kind == TaskKind::Bug)
            );
        }

        #[test]
        fn a_brief_is_stored_with_its_task_and_read_back_byte_for_byte() {
            let s = store();
            s.insert_project(&project("p1", "/repo")).unwrap();
            let body = " # 中文 🐛\r\n{request}\n  ";
            s.create_task(&task("T-1", "p1", "one"), Some(body))
                .unwrap();
            assert_eq!(
                s.task_brief("T-1").unwrap().unwrap().as_bytes(),
                body.as_bytes()
            );
            s.create_task(&task("T-2", "p1", "two"), None).unwrap();
            assert_eq!(s.task_brief("T-2").unwrap(), None);
            // A body write failure must roll back the task and its queue sequence too.
            s.conn.execute_batch("CREATE TRIGGER reject_brief BEFORE INSERT ON task_briefs BEGIN SELECT RAISE(ABORT, 'write failure'); END;").unwrap();
            assert!(
                s.create_task(&task("T-3", "p1", "three"), Some(body))
                    .is_err()
            );
            assert!(s.get_task("T-3").is_err());
            assert_eq!(s.list_tasks("p1").unwrap().len(), 2);
        }

        #[test]
        fn a_started_draft_queues_behind_tasks_that_were_already_queued() {
            let s = store();
            s.insert_project(&project("p1", "/repo")).unwrap();
            assert!(s.queued_tasks("p1").unwrap().is_empty());
            for (id, created) in [
                ("old-later", "2026-11-10T02:00:00Z"),
                ("old-first", "2026-11-10T01:00:00Z"),
            ] {
                let mut t = task(id, "p1", id);
                t.created_at = created.into();
                s.insert_task(&t).unwrap();
            }
            for (id, created) in [
                ("draft-first", "2026-11-09T00:00:00Z"),
                ("draft-second", "2026-11-10T03:00:00Z"),
            ] {
                let mut t = task(id, "p1", id);
                t.state = TaskState::Draft;
                t.created_at = created.into();
                s.create_task(&t, None).unwrap();
            }
            assert_eq!(s.queued_tasks("p1").unwrap().len(), 2);
            assert_eq!(s.slots_in_use("p1").unwrap(), 0);
            let mut t = task("new", "p1", "new");
            t.created_at = "2026-11-10T03:00:00Z".into();
            s.create_task(&t, None).unwrap();
            for id in ["draft-second", "draft-first"] {
                s.set_task_state(id, &TaskState::Queued).unwrap();
                s.mark_queued(id).unwrap();
            }
            let ids = || {
                s.queued_tasks("p1")
                    .unwrap()
                    .into_iter()
                    .map(|t| t.id)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                ids(),
                [
                    "old-first",
                    "old-later",
                    "new",
                    "draft-second",
                    "draft-first"
                ]
            );
            s.set_task_state(
                "new",
                &TaskState::Requeued {
                    resume: Node::Design,
                },
            )
            .unwrap();
            assert_eq!(
                ids(),
                [
                    "old-first",
                    "old-later",
                    "new",
                    "draft-second",
                    "draft-first"
                ]
            );
        }

        #[test]
        fn task_counts_has_a_draft_bucket() {
            assert!(task_counts(&[]).is_empty());
            let mut t = task("T-1", "p1", "one");
            t.state = TaskState::Draft;
            assert_eq!(task_counts(&[t]), BTreeMap::from([("draft", 1)]));
        }
    }
    #[test]
    fn task_convert_commits_both_tasks_or_neither_across_connections() {
        let path = std::env::temp_dir().join(format!("convert-{}.sqlite", new_id("test")));
        let s = Store::open(&path).unwrap();
        s.insert_project(&project("p", "/a")).unwrap();
        let mut old = task("old", "p", "old");
        old.kind = TaskKind::Bug;
        old.state = TaskState::Active {
            node: autome_domain::task::Node::NotABug,
        };
        s.create_task(&old, Some("原文  \n")).unwrap();
        let observer = Store::open(&path).unwrap();
        let mut new = task("new", "p", "new");
        new.state = TaskState::Draft;
        assert!(observer.get_task("new").is_err());
        assert!(!observer.get_task("old").unwrap().is_archived());
        let before = observer.events_since_with_subject(0, 100).unwrap();
        s.conn.execute_batch("CREATE TRIGGER fail_last BEFORE INSERT ON events WHEN NEW.kind='task.updated' BEGIN SELECT RAISE(ABORT,'last event'); END;").unwrap();
        assert!(s.convert_task("old", &new, Some("原文  \n")).is_err());
        assert!(observer.get_task("new").is_err());
        assert_eq!(observer.task_brief("new").unwrap(), None);
        assert_eq!(observer.get_task("old").unwrap().state, old.state);
        assert!(!observer.get_task("old").unwrap().is_archived());
        assert_eq!(observer.events_since_with_subject(0, 100).unwrap(), before);
        s.conn.execute_batch("DROP TRIGGER fail_last").unwrap();
        // The archive flag must already be part of the event-producing transaction,
        // not a follow-up write after commit (which an after-return read would miss).
        s.conn.execute_batch("CREATE TRIGGER require_archive BEFORE INSERT ON events WHEN NEW.kind='task.created' BEGIN SELECT CASE WHEN (SELECT archived_at FROM tasks WHERE id='old') IS NULL THEN RAISE(ABORT,'archive not in transaction') END; END;").unwrap();

        s.convert_task("old", &new, Some("原文  \n")).unwrap();
        assert_eq!(observer.get_task("new").unwrap().state, TaskState::Draft);
        assert_eq!(
            observer.task_brief("new").unwrap().as_deref(),
            Some("原文  \n")
        );
        assert_eq!(
            observer.get_task("old").unwrap().state,
            TaskState::Cancelled
        );
        assert!(observer.get_task("old").unwrap().is_archived());
        let events = observer.events_since_with_subject(0, 100).unwrap();
        assert_eq!(
            events.iter().map(|e| e.1.as_str()).collect::<Vec<_>>(),
            vec!["task.created", "task.converted", "task.updated"]
        );
        drop(observer);
        drop(s);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod kernel_mark_tests {
    use super::*;
    const OLD: &str = "0000001-20200101T000000Z-000000000001";
    const NEW: &str = "0000002-20200101T000000Z-000000000002";

    #[test]
    fn kernel_mark_claim_matrix() {
        let unknown = "unknown-0.1.0-20200101T000000Z";
        let other = "unknown-0.1.0-20210101T000000Z";
        for (current, recorded, accepted, changed) in [
            (OLD, None, true, true),
            (OLD, Some(OLD), true, false),
            (NEW, Some(OLD), true, true),
            (OLD, Some(NEW), false, false),
            (
                "0000001-20200101T000000Z-000000000003",
                Some(OLD),
                false,
                false,
            ),
            (NEW, Some(unknown), true, true),
            (unknown, Some(NEW), false, false),
            (unknown, Some(other), false, false),
            (NEW, Some(""), false, false),
            (NEW, Some("future-format"), false, false),
            (unknown, None, true, true),
            (unknown, Some(unknown), true, false),
            (
                "9999999-29991231T235959Z-ffffffffffff",
                Some(OLD),
                true,
                true,
            ),
            ("0000000-20200229T000000Z-000000000000", None, true, true),
        ] {
            let s = Store::open_in_memory().unwrap();
            if let Some(m) = recorded {
                s.conn
                    .execute("INSERT INTO kernel_mark VALUES (1, ?1, 'sentinel')", [m])
                    .unwrap();
            }
            assert_eq!(
                s.claim_kernel_mark(current).unwrap().is_none(),
                accepted,
                "{current} / {recorded:?}"
            );
            assert_eq!(
                s.kernel_mark().unwrap().as_deref(),
                if changed { Some(current) } else { recorded }
            );
            let time: String = s
                .conn
                .query_row("SELECT updated_at FROM kernel_mark", [], |r| r.get(0))
                .unwrap();
            assert_eq!(time != "sentinel", changed);
            assert_eq!(s.latest_seq().unwrap(), 0);
        }
    }

    #[test]
    fn kernel_mark_reserved_lock_prevents_stale_claim() {
        let path = std::env::temp_dir().join(format!("kernel-mark-{}.db", new_id("test")));
        let s = Store::open(&path).unwrap();
        s.claim_kernel_mark(OLD).unwrap();
        let other = Store::open(&path).unwrap();
        other.conn.pragma_update(None, "busy_timeout", 0).unwrap();
        s.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        s.conn
            .execute("UPDATE kernel_mark SET build_id = ?1", [NEW])
            .unwrap();
        assert!(other.claim_kernel_mark(OLD).is_err());
        s.conn.execute_batch("COMMIT").unwrap();
        assert_eq!(other.claim_kernel_mark(OLD).unwrap().as_deref(), Some(NEW));
        drop(other);
        drop(s);
        super::tests::remove_db(&path);
    }
}
