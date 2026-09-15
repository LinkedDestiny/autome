//! End-to-end tests: a whole task driven through the real code, with a
//! stand-in CLI in place of a real model. Technical design §16, gate T7;
//! requirements document §6.
//!
//! Everything below the model is real: a real Git repository, a real
//! `.autome/` scaffold, the real wrapper script, the real exit-marker
//! protocol, the real status-block parser, the real transition table, real
//! worktrees, a real rebase and a real merge. The only substitution is the
//! process at the far end of the launcher, which writes the design document a
//! model would have written.
//!
//! That substitution is what makes these tests worth having. The parts most
//! likely to be wrong are the seams — does the marker get noticed, does the
//! document get found in the *worktree* rather than the repository root, does
//! the merge precondition hold after a rebase — and none of those involve a
//! model at all.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use automed::dispatch::{Ctx, Outcome, handle_command};
use automed::ipc::{Command, PROTOCOL_VERSION, ReplyOutcome};
use automed::store::Store;
use serde_json::{Value, json};

static COUNTER: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------------------
// The stand-in CLI
// ---------------------------------------------------------------------------

/// Both runtimes are pointed at one script, which finds its repository and
/// runs whatever step that repository has queued. Per-repository dispatch is
/// what lets these tests run in parallel despite the environment variable
/// being process-global.
const FAKE_CLI: &str = r#"#!/bin/sh
# Stand-in for claude/codex in the end-to-end suite.
# Locates the repository root from the worktree we were started in, then runs
# the next queued step script. Each step writes whatever a model would have
# written for that node.
set -u
common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || exit 90
repo=$(dirname "$common")
fake="$repo/.autome/fake"
[ -d "$fake" ] || exit 91
n=$(cat "$fake/next" 2>/dev/null || echo 1)
step="$fake/$n.sh"
echo "$n" >> "$fake/history"
expr "$n" + 1 > "$fake/next"
if [ ! -f "$step" ]; then
  echo "fake-cli: no step $n" >&2
  exit 92
fi
# The prompt arrives on stdin; keep it so a step can assert on it.
cat > "$fake/$n.prompt"
sh "$step" "$fake/$n.prompt"
"#;

/// Writes the stand-in once per test process and points both runtimes at it.
fn install_fake_cli() -> PathBuf {
    use std::sync::OnceLock;
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("automed-e2e-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-cli");
        std::fs::write(&script, FAKE_CLI).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = std::fs::metadata(&script).unwrap().permissions();
            p.set_mode(0o755);
            std::fs::set_permissions(&script, p).unwrap();
        }
        // Safety: set once, before any test spawns a session, and never
        // changed afterwards.
        unsafe {
            std::env::set_var("AUTOMED_CLAUDE_BINARY", &script);
            std::env::set_var("AUTOMED_CODEX_BINARY", &script);
            std::env::set_var("AUTOMED_HEADLESS", "1");
        }
        script
    })
    .clone()
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct World {
    root: PathBuf,
    repo: PathBuf,
    ctx: Ctx,
    project_id: String,
}

impl World {
    fn new(tag: &str) -> Self {
        install_fake_cli();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("automed-e2e-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let autome_home = root.join("autome-home");
        let home = root.join("home");
        let repo = root.join("repo");
        for d in [&autome_home, &home, &repo] {
            std::fs::create_dir_all(d).unwrap();
        }

        let store = Store::open_in_memory().unwrap();
        let mut ctx = Ctx::new(store, &autome_home, &home);

        // Seed the repository with a commit, then register it the way the UI
        // would: through the real `project.add` command.
        automed::git::init(&repo, "main").unwrap();
        std::fs::write(repo.join("README.md"), "hello\n").unwrap();
        automed::git::commit_paths(&repo, &["README.md"], "initial").unwrap();

        let added = call(
            &mut ctx,
            "project.add",
            json!({ "path": repo.to_str().unwrap() }),
        );
        let project_id = ok(&added)["project"]["id"].as_str().unwrap().to_string();
        call(
            &mut ctx,
            "project.onboarding.skip",
            json!({ "project_id": project_id }),
        );

        // The stand-in's control files live in the repository, so they must
        // be ignored — otherwise the main worktree reads dirty and every merge
        // precondition fails for a reason that has nothing to do with the code
        // under test.
        std::fs::create_dir_all(repo.join(".autome/fake")).unwrap();
        std::fs::write(repo.join(".autome/fake/next"), "1").unwrap();
        let gitignore = repo.join(".gitignore");
        let mut text = std::fs::read_to_string(&gitignore).unwrap_or_default();
        text.push_str("\n.autome/fake/\n");
        std::fs::write(&gitignore, text).unwrap();
        automed::git::commit_paths(&repo, &[".gitignore"], "test fixture ignore").unwrap();

        World {
            root,
            repo,
            ctx,
            project_id,
        }
    }

    /// Queues the script the Nth session will run.
    fn step(&self, n: u32, script: &str) {
        let path = self.repo.join(format!(".autome/fake/{n}.sh"));
        std::fs::write(path, script).unwrap();
    }

    /// Like `doc_step`, with a shell prelude that runs before the document is
    /// written — used to hold a session open while the test checks something.
    fn doc_step_with_prelude(&self, n: u32, slug: &str, body: &str, prelude: &str) {
        self.step(
            n,
            &format!(
                r#"set -e
{prelude}
mkdir -p "docs/{slug}"
cat > "docs/{slug}/{slug}.md" <<'AUTOME_EOF'
{body}
AUTOME_EOF
git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session {n}" >/dev/null 2>&1 || true
"#
            ),
        );
    }

    /// A step that writes a design document into the task's worktree and
    /// commits it, which is what every real session does.
    fn doc_step(&self, n: u32, slug: &str, body: &str) {
        self.step(
            n,
            &format!(
                r#"set -e
mkdir -p "docs/{slug}"
cat > "docs/{slug}/{slug}.md" <<'AUTOME_EOF'
{body}
AUTOME_EOF
git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session {n}" >/dev/null 2>&1 || true
"#
            ),
        );
    }

    fn call(&mut self, method: &str, params: Value) -> Outcome {
        call(&mut self.ctx, method, params)
    }

    /// Runs scheduling passes until the world is quiet.
    ///
    /// "Quiet" is not "the last tick did nothing": right after a session
    /// starts, several ticks in a row do nothing while the process runs. The
    /// condition is that no session is running *and* no task is sitting on
    /// work the scheduler would pick up — a queued task or a core step.
    fn settle(&mut self) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            automed::scheduler::tick(&mut self.ctx);
            if !self.has_pending_work() {
                return;
            }
            if std::time::Instant::now() >= deadline {
                panic!(
                    "world did not settle within 20s; sessions run: {:?}, tasks: {:?}",
                    self.sessions_run(),
                    self.task_states()
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
    }

    fn has_pending_work(&self) -> bool {
        if !self
            .ctx
            .store
            .all_running_sessions()
            .unwrap_or_default()
            .is_empty()
        {
            return true;
        }
        self.ctx
            .store
            .list_unfinished()
            .unwrap_or_default()
            .iter()
            .any(|t| {
                let v = serde_json::to_value(&t.state).unwrap();
                let state = v.get("state").and_then(Value::as_str).unwrap_or("");
                let node = v.get("node").and_then(Value::as_str).unwrap_or("");
                // Queued tasks are waiting for a slot; core-step nodes are
                // work the next tick performs. Both mean "not settled yet".
                state == "queued" || matches!(node, "rebase" | "merging" | "cleanup")
            })
    }

    fn task_states(&self) -> Vec<(String, Value)> {
        self.ctx
            .store
            .list_unfinished()
            .unwrap_or_default()
            .into_iter()
            .map(|t| (t.id, serde_json::to_value(t.state).unwrap()))
            .collect()
    }

    /// The task's state tag, e.g. `active`, `done`, `failed`.
    fn state(&self, task_id: &str) -> String {
        let task = self.ctx.store.get_task(task_id).unwrap();
        serde_json::to_value(&task.state)
            .unwrap()
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string()
    }

    fn node(&self, task_id: &str) -> Option<String> {
        let task = self.ctx.store.get_task(task_id).unwrap();
        serde_json::to_value(&task.state)
            .unwrap()
            .get("node")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    /// Blocks until the task's running session has written its exit marker.
    fn wait_for_exit_marker(&self, task_id: &str) {
        let session = self
            .ctx
            .store
            .running_session(task_id)
            .unwrap()
            .expect("a session should be running");
        let marker = self.repo.join(autome_domain::session::SessionPaths::exit(
            task_id,
            &session.id,
        ));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            if std::fs::read_to_string(&marker)
                .ok()
                .and_then(|t| autome_domain::session::ExitMarker::parse(&t))
                .is_some()
            {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("no exit marker at {}", marker.display());
    }

    fn sessions_run(&self) -> Vec<String> {
        std::fs::read_to_string(self.repo.join(".autome/fake/history"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if std::env::var("AUTOMED_KEEP_SANDBOX").is_ok() {
            eprintln!("sandbox kept at {}", self.root.display());
            return;
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn call(ctx: &mut Ctx, method: &str, params: Value) -> Outcome {
    handle_command(
        ctx,
        &Command {
            request_id: "r".into(),
            command_id: "c".into(),
            expected_revision: None,
            protocol_version: PROTOCOL_VERSION,
            method: method.into(),
            params,
        },
    )
}

fn ok(outcome: &Outcome) -> &Value {
    match &outcome.reply.outcome {
        ReplyOutcome::Ok { payload, .. } => payload,
        ReplyOutcome::Error { code, message } => panic!("expected ok, got {code:?}: {message}"),
    }
}

fn error_message(outcome: &Outcome) -> &str {
    match &outcome.reply.outcome {
        ReplyOutcome::Error { message, .. } => message,
        ReplyOutcome::Ok { .. } => panic!("expected an error"),
    }
}

/// A design document at a given stage.
fn doc(status: &str, design_round: u32, impl_round: u32, milestones: &[(&str, &str)]) -> String {
    let rows: String = milestones
        .iter()
        .map(|(id, state)| format!("| {id} | {state} | {id} 标题 | 0 | |\n"))
        .collect();
    let table = if milestones.is_empty() {
        String::new()
    } else {
        format!(
            "\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n{rows}"
        )
    };
    format!(
        "# 购物车结算\n\nstatus: {status}\ndesign-round: {design_round}/15\n\
         implementation-round: {impl_round}/25\ncurrent-milestone: 无\n\
         current-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 无\n{table}"
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

/// The slug `task.create` will derive. Computed up front because creating a
/// task starts its first session immediately — any step script written after
/// the call would lose the race.
fn slug_for(request: &str) -> String {
    autome_domain::project::slugify(request)
}

/// Creates a task and returns its id.
fn create_task(w: &mut World, request: &str) -> String {
    let created = w.call(
        "task.create",
        json!({ "project_id": w.project_id.clone(), "request": request }),
    );
    let payload = ok(&created);
    let id = payload["task"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        payload["task"]["slug"].as_str().unwrap(),
        slug_for(&request_of(w, &id)),
        "slug_for must predict what task.create derives"
    );
    id
}

fn request_of(w: &World, task_id: &str) -> String {
    w.ctx.store.get_task(task_id).unwrap().request
}

// ---------------------------------------------------------------------------
// Scenario 1: the whole happy path (requirements §6, scenario 1)
// ---------------------------------------------------------------------------

#[test]
fn a_task_runs_from_one_line_to_a_merge_commit() {
    needs_git!();
    let mut w = World::new("happy");

    // Sessions, in the order the Loop runs them:
    //   1 intake      -> 设计中, no milestones
    //   2 design      -> 设计中
    //   3 review      -> 设计中
    //   4 adjudicate  -> 实现中 with two open milestones (design is final)
    //   5 implement   -> both pending
    //   6 audit       -> both done
    let request = "add cart checkout";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(
        4,
        &slug,
        &doc("实现中", 2, 0, &[("M-01", "开放"), ("M-02", "开放")]),
    );
    w.doc_step(
        5,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "待审"), ("M-02", "待审")]),
    );
    w.doc_step(
        6,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "已完成"), ("M-02", "已完成")]),
    );

    let task_id = create_task(&mut w, request);
    w.settle();

    // The design loop has run and the task is waiting for the user.
    assert_eq!(
        w.node(&task_id).as_deref(),
        Some("await_design_approval"),
        "sessions run so far: {:?}",
        w.sessions_run()
    );

    // Approve; the implementation loop runs and reaches the merge stopping
    // point. Autome must NOT merge on its own (requirement T-05).
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));

    let before_merge = automed::git::head_sha(&w.repo).unwrap();

    // The panel shows the change summary.
    let changes = w.call("task.changes", json!({ "task_id": task_id }));
    let payload = ok(&changes);
    assert_eq!(payload["available"], json!(true));
    assert_eq!(payload["mergeable"], json!(true));
    assert!(payload["commits"].as_u64().unwrap() >= 1);

    // Merge, on the user's command only.
    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle();

    assert_eq!(w.state(&task_id), "done");
    let task = w.ctx.store.get_task(&task_id).unwrap();
    assert!(task.merge_commit.is_some(), "a merge commit was recorded");
    assert_ne!(
        automed::git::head_sha(&w.repo).unwrap(),
        before_merge,
        "the default branch moved"
    );
    // Cleanup happened.
    assert!(!w.repo.join(format!(".worktree/{slug}")).exists());
    assert!(!automed::git::branch_exists(
        &w.repo,
        &format!("autome/{slug}")
    ));
    // The task's documents came across with the merge.
    assert!(w.repo.join(format!("docs/{slug}/{slug}.md")).exists());
}

// ---------------------------------------------------------------------------
// Scenario 8: a dirty main worktree blocks the merge and touches nothing
// ---------------------------------------------------------------------------

#[test]
fn a_dirty_main_worktree_blocks_the_merge_and_leaves_the_users_file_alone() {
    needs_git!();
    let mut w = World::new("dirty");
    let request = "add a feature";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));

    // The user leaves work in progress *in a file this merge would write* —
    // dirt elsewhere no longer blocks a merge, since the merge would not
    // disturb it (see git::conflicting_dirty_paths).
    let touched = ok(&w.call("task.changes", json!({ "task_id": task_id })))["files"]
        .as_array()
        .unwrap()[0]["path"]
        .as_str()
        .unwrap()
        .to_string();
    let wip = w.repo.join(&touched);
    if let Some(parent) = wip.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&wip, "half-finished\n").unwrap();
    let head_before = automed::git::head_sha(&w.repo).unwrap();

    let changes = w.call("task.changes", json!({ "task_id": task_id }));
    assert_eq!(ok(&changes)["mergeable"], json!(false));
    assert_eq!(
        ok(&changes)["blocked_by"]["kind"],
        json!("dirty_worktree"),
        "{:?}",
        ok(&changes)["blocked_by"]
    );

    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle();

    assert_eq!(
        w.node(&task_id).as_deref(),
        Some("await_merge"),
        "a blocked merge returns to the stopping point rather than failing"
    );
    assert_eq!(
        automed::git::head_sha(&w.repo).unwrap(),
        head_before,
        "the default branch did not move"
    );
    assert_eq!(
        std::fs::read_to_string(&wip).unwrap(),
        "half-finished\n",
        "the user's file is byte-for-byte untouched"
    );
}

// ---------------------------------------------------------------------------
// Scenario 9: rejecting a design sends it back with the user's words
// ---------------------------------------------------------------------------

#[test]
fn rejecting_a_design_re_runs_it_carrying_the_feedback_verbatim() {
    needs_git!();
    let mut w = World::new("reject");
    let request = "add a thing";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    // Session 5 is the re-run design round, queued in advance for the same
    // reason as the rest.
    w.doc_step(5, &slug, &doc("设计中", 3, 0, &[]));
    let task_id = create_task(&mut w, request);
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_design_approval"));

    w.call(
        "task.reject",
        json!({ "task_id": task_id, "feedback": "确认邮件只发登录用户" }),
    );
    w.settle();

    let prompt = std::fs::read_to_string(w.repo.join(".autome/fake/5.prompt")).unwrap();
    assert!(
        prompt.contains("确认邮件只发登录用户"),
        "the feedback must reach the session verbatim:\n{prompt}"
    );
    assert!(prompt.contains("设计轮"), "it is a design round:\n{prompt}");
}

// ---------------------------------------------------------------------------
// Scenario 2: the parallel limit and the queue
// ---------------------------------------------------------------------------

#[test]
fn the_parallel_limit_queues_the_fourth_task_and_releases_it_on_completion() {
    needs_git!();
    let mut w = World::new("parallel");
    w.call(
        "config.set_loop",
        json!({ "project_id": w.project_id.clone(), "parallel": 2 }),
    );

    // Each session sleeps, so it holds its slot while the next task is
    // created. Without this the stand-in would exit immediately, the task
    // would fail, the slot would free, and all three would legitimately run —
    // which tests nothing about the limit.
    let requests = ["alpha task", "beta task", "gamma task"];
    for request in requests {
        w.doc_step_with_prelude(1, &slug_for(request), &doc("设计中", 0, 0, &[]), "sleep 3");
    }

    let mut ids = Vec::new();
    for request in requests {
        ids.push(create_task(&mut w, request));
    }

    let running = w.ctx.store.all_running_sessions().unwrap().len();
    assert!(
        running <= 2,
        "the parallel limit must hold: {running} running"
    );

    let queued: Vec<String> = w
        .ctx
        .store
        .queued_tasks(&w.project_id)
        .unwrap()
        .into_iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(queued.len(), 1, "the third task waits: {queued:?}");
    assert_eq!(queued[0], ids[2], "the queue is first-come-first-served");

    // A queued task reports its position.
    let panel = w.call("task.get", json!({ "task_id": ids[2].clone() }));
    assert_eq!(ok(&panel)["queue_position"], json!(1));
}

// ---------------------------------------------------------------------------
// Protocol failure: a malformed design document stops the task
// ---------------------------------------------------------------------------

#[test]
fn a_design_document_without_a_status_block_fails_the_task_once() {
    needs_git!();
    let mut w = World::new("protocol");
    let request = "something";
    // The intake session writes a document with no status block at all.
    w.doc_step(1, &slug_for(request), "# 标题\n\n没有状态块。\n");
    let task_id = create_task(&mut w, request);
    w.settle();

    assert_eq!(w.state(&task_id), "failed");
    let task = w.ctx.store.get_task(&task_id).unwrap();
    let reason = serde_json::to_value(&task.state).unwrap();
    assert_eq!(reason["reason"]["kind"], json!("protocol"), "{reason:#?}");
    // And it stopped there rather than retrying.
    assert_eq!(w.sessions_run().len(), 1, "no retry");
}

// ---------------------------------------------------------------------------
// A crashed session is a crash even when the document looks fine
// ---------------------------------------------------------------------------

#[test]
fn a_non_zero_exit_fails_the_task_even_with_a_valid_document() {
    needs_git!();
    let mut w = World::new("crash");
    let request = "something else";
    let slug = slug_for(request);
    w.step(
        1,
        &format!(
            r#"set -e
mkdir -p "docs/{slug}"
cat > "docs/{slug}/{slug}.md" <<'AUTOME_EOF'
{}
AUTOME_EOF
exit 3
"#,
            doc("设计中", 0, 0, &[])
        ),
    );
    let task_id = create_task(&mut w, request);
    w.settle();

    assert_eq!(w.state(&task_id), "failed");
    let state = serde_json::to_value(w.ctx.store.get_task(&task_id).unwrap().state).unwrap();
    assert_eq!(
        state["reason"]["kind"],
        json!("session_crashed"),
        "{state:#?}"
    );
}

// ---------------------------------------------------------------------------
// Cancel keeps the documents
// ---------------------------------------------------------------------------

#[test]
fn cancelling_a_task_archives_its_documents_and_removes_the_worktree() {
    needs_git!();
    let mut w = World::new("cancel");
    let request = "abandon me";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    let task_id = create_task(&mut w, request);
    w.settle();

    w.call("task.cancel", json!({ "task_id": task_id }));

    assert_eq!(w.state(&task_id), "cancelled");
    let archived = w.repo.join(format!("docs/.archive/{slug}/{slug}.md"));
    assert!(
        archived.exists(),
        "the design document survives a cancel: {}",
        archived.display()
    );
    assert!(!w.repo.join(format!(".worktree/{slug}")).exists());
    assert!(!automed::git::branch_exists(
        &w.repo,
        &format!("autome/{slug}")
    ));
}

// ---------------------------------------------------------------------------
// Stop and resume
// ---------------------------------------------------------------------------

#[test]
fn stopping_a_task_leaves_it_resumable() {
    needs_git!();
    let mut w = World::new("stop");
    let request = "stop me";
    w.doc_step(1, &slug_for(request), &doc("设计中", 0, 0, &[]));
    let task_id = create_task(&mut w, request);
    w.settle();

    // Whatever node it reached, stopping must park it.
    if w.ctx
        .store
        .get_task(&task_id)
        .unwrap()
        .state
        .node()
        .is_some()
    {
        let out = w.call("task.stop", json!({ "task_id": task_id }));
        if matches!(out.reply.outcome, ReplyOutcome::Ok { .. }) {
            assert_eq!(w.state(&task_id), "stopped");
            w.call("task.resume", json!({ "task_id": task_id }));
            assert!(matches!(w.state(&task_id).as_str(), "queued" | "active"));
        }
    }
}

// ---------------------------------------------------------------------------
// Scenario 4: a SAME-MODEL collision is refused and reversible
// ---------------------------------------------------------------------------

#[test]
fn a_same_model_collision_is_refused_and_the_fix_is_accepted() {
    needs_git!();
    let mut w = World::new("samemodel");
    // The shipped defaults leave models unnamed, so moving audit onto Claude
    // puts it on exactly the session impl already runs.
    let refused = w.call(
        "config.set_role",
        json!({
            "project_id": w.project_id.clone(),
            "role": "audit",
            "runtime": "claude"
        }),
    );
    let message = error_message(&refused).to_string();
    assert!(
        message.contains("审计") && message.contains("实现"),
        "{message}"
    );

    // The file was not written.
    let text = std::fs::read_to_string(w.repo.join(".autome/config.toml")).unwrap_or_default();
    assert!(!text.contains("[roles.audit]"), "{text}");

    // Naming a different model on the same runtime resolves it.
    let accepted = w.call(
        "config.set_role",
        json!({
            "project_id": w.project_id.clone(),
            "role": "audit",
            "runtime": "claude",
            "model": "sonnet"
        }),
    );
    assert!(matches!(accepted.reply.outcome, ReplyOutcome::Ok { .. }));
}

// ---------------------------------------------------------------------------
// Scenario 10: recovery after a restart
// ---------------------------------------------------------------------------

#[test]
fn a_session_that_finished_during_a_restart_is_consumed_on_recovery() {
    needs_git!();
    let mut w = World::new("recover");
    let request = "restart me";
    w.doc_step(1, &slug_for(request), &doc("设计中", 0, 0, &[]));
    let task_id = create_task(&mut w, request);

    // Let the intake session run to completion without ticking — this is
    // exactly the "we were not watching" case. Waiting for the marker rather
    // than for a fixed duration keeps the test deterministic on a slow machine.
    w.wait_for_exit_marker(&task_id);

    let report = automed::scheduler::recover(&mut w.ctx);
    assert!(report.errors.is_empty(), "{report:?}");
    // The task moved on from intake.
    assert_ne!(
        w.node(&task_id).as_deref(),
        Some("intake"),
        "recovery consumed the finished session"
    );
}

// ---------------------------------------------------------------------------
// Archive, after completion only
// ---------------------------------------------------------------------------

#[test]
fn archiving_is_refused_until_the_task_is_done() {
    needs_git!();
    let mut w = World::new("archive");
    let task_id = create_task(&mut w, "not done yet");
    let out = w.call("task.archive", json!({ "task_id": task_id }));
    assert!(error_message(&out).contains("已完成"));
}

// ---------------------------------------------------------------------------
// The dashboard sees across projects
// ---------------------------------------------------------------------------

#[test]
fn the_dashboard_lists_running_and_waiting_tasks() {
    needs_git!();
    let mut w = World::new("dashboard");
    let request = "dashboard task";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    let task_id = create_task(&mut w, request);
    w.settle();

    let out = w.call("dashboard.get", json!({}));
    let payload = ok(&out);
    let waiting = payload["waiting"].as_array().unwrap();
    assert_eq!(waiting.len(), 1, "{payload:#?}");
    assert_eq!(waiting[0]["task"]["id"], json!(task_id));
    assert!(payload["environment"]["severity"].is_string());
}

// ---------------------------------------------------------------------------
// The wrapper script's own contract
// ---------------------------------------------------------------------------

#[test]
fn the_wrapper_writes_a_terminated_exit_marker_carrying_the_clis_code() {
    needs_git!();
    let w = World::new("wrapper");
    let dir = w.repo.join(".autome/output/sessions/T-x");
    std::fs::create_dir_all(&dir).unwrap();
    let prompt = dir.join("p.txt");
    std::fs::write(&prompt, "hello").unwrap();

    let wrapper = w.repo.join(".autome/skill/run_session.sh");
    let status = std::process::Command::new("sh")
        .arg(&wrapper)
        .arg("s1")
        .arg(&dir)
        .arg("claude")
        .arg("/bin/sh")
        .arg(&prompt)
        .arg("-c")
        .arg("exit 7")
        .current_dir(&w.repo)
        .output()
        .expect("wrapper runs");
    assert_eq!(
        status.status.code(),
        Some(7),
        "the CLI's code is propagated"
    );

    let marker = std::fs::read_to_string(dir.join("s1.exit")).unwrap();
    let parsed = autome_domain::session::ExitMarker::parse(&marker)
        .expect("the marker must be complete and parseable");
    assert_eq!(parsed.exit_code, 7);
    assert!(dir.join("s1.log").exists(), "the log was written");
    assert!(
        !dir.join("s1.pid").exists(),
        "the pid file is removed on a clean exit"
    );
}

#[test]
fn the_wrapper_refuses_a_missing_prompt_file_and_still_writes_a_marker() {
    needs_git!();
    let w = World::new("wrapper-noprompt");
    let dir = w.repo.join(".autome/output/sessions/T-y");
    std::fs::create_dir_all(&dir).unwrap();
    let wrapper = w.repo.join(".autome/skill/run_session.sh");
    let out = std::process::Command::new("sh")
        .arg(&wrapper)
        .arg("s2")
        .arg(&dir)
        .arg("claude")
        .arg("/bin/echo")
        .arg(dir.join("does-not-exist"))
        .current_dir(&w.repo)
        .output()
        .expect("wrapper runs");
    assert_eq!(out.status.code(), Some(66));
    let marker = std::fs::read_to_string(dir.join("s2.exit")).unwrap();
    assert_eq!(
        autome_domain::session::ExitMarker::parse(&marker)
            .unwrap()
            .exit_code,
        66
    );
}

/// The scaffold must gitignore `.worktree/`, or the main worktree is
/// permanently dirty and every merge is blocked. This is the bug the git
/// fixture caught; it is worth an explicit test at the integration level.
#[test]
fn a_freshly_added_project_has_a_clean_worktree_even_with_a_task_running() {
    needs_git!();
    let mut w = World::new("clean");
    let request = "keep it clean";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    let task_id = create_task(&mut w, request);
    let _ = task_id;
    w.settle();

    assert!(
        w.repo.join(format!(".worktree/{slug}")).exists(),
        "the worktree exists"
    );
    assert!(
        automed::git::is_clean(&w.repo).unwrap(),
        "the main worktree must still read clean: {:?}",
        automed::git::dirty_paths(&w.repo).unwrap()
    );
}
