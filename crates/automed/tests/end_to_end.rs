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
# The counter is per-repository by default, which is what a single-task test
# wants: steps 1,2,3... are that task's successive rounds. A test running
# several tasks in one repository needs one counter each, or task A consumes
# task B's step. Such a test creates `$fake/<slug>/` and we use it instead.
slug=$(basename "$PWD")
[ -d "$fake/$slug" ] && fake="$fake/$slug"
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
        // A fixed name rather than one per process: the directory is shared by
        // every test here, so it cannot be removed when any one of them
        // finishes. Reusing the name means the next run overwrites it instead
        // of the suite leaving one behind each time.
        let dir = std::env::temp_dir().join("automed-e2e-bin");
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
        // changed afterwards. The launch *mode* is no longer an environment
        // variable — it is a field on Ctx (see `World::new`).
        unsafe {
            std::env::set_var("AUTOMED_CLAUDE_BINARY", &script);
            std::env::set_var("AUTOMED_CODEX_BINARY", &script);
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
        // Headless: the wrapper runs directly rather than being handed to a
        // terminal. Everything else is the production path; the terminal hop
        // itself has its own test.
        let mut ctx = Ctx::new(store, &autome_home, &home).headless();

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
    /// What every loop round leaves behind, which the core now checks for
    /// (plan §4.D): `docs/<slug>/evidence/M-xx-r<k>-<role>.md`, where `k` is
    /// the implementation round in the status block the session just wrote.
    ///
    /// Both suffixes, because a step script does not know which role is
    /// running it — the same script stands in for the implementation round and
    /// the audit round of the same `k`, and writing only one of the two names
    /// is exactly the collision the suffix exists to prevent.
    const EVIDENCE: &'static str = r#"
k=$(grep -m1 '^implementation-round:' "docs/$SLUG/$SLUG.md" 2>/dev/null \
  | sed 's#[^0-9]*\([0-9]*\)/.*#\1#')
if [ -n "${k:-}" ]; then
  mkdir -p "docs/$SLUG/evidence"
  printf '命令：fake\n结果：通过\n' > "docs/$SLUG/evidence/M-01-r$k-impl.md"
  printf '复验：fake\n结论：通过\n' > "docs/$SLUG/evidence/M-01-r$k-audit.md"
  printf '轮次 | M-01 | 通过 | e | 无\n' >> "docs/$SLUG/retro.md"
fi
"#;

    fn step(&self, n: u32, script: &str) {
        let path = self.repo.join(format!(".autome/fake/{n}.sh"));
        std::fs::write(path, script).unwrap();
    }

    /// Queues the Nth script for *one task's* worktree rather than for the
    /// repository. Needed only by tests that run several tasks at once: they
    /// share a repository, so they would otherwise share a step counter and
    /// consume each other's scripts.
    fn step_for(&self, slug: &str, n: u32, script: &str) {
        let dir = self.repo.join(format!(".autome/fake/{slug}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("next"), "1").unwrap();
        std::fs::write(dir.join(format!("{n}.sh")), script).unwrap();
    }

    /// `doc_step_with_prelude`, scoped to one task's worktree.
    fn doc_step_for(&self, slug: &str, n: u32, body: &str, prelude: &str) {
        self.step_for(slug, n, &Self::doc_script(slug, n, body, prelude));
    }

    /// What a session writes: the design document for its task, committed.
    fn doc_script(slug: &str, n: u32, body: &str, prelude: &str) -> String {
        let evidence = Self::EVIDENCE;
        format!(
            r#"set -e
SLUG={slug}
{prelude}
mkdir -p "docs/{slug}"
cat > "docs/{slug}/{slug}.md" <<'AUTOME_EOF'
{body}
AUTOME_EOF
{evidence}
git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session {n}" >/dev/null 2>&1 || true
"#
        )
    }

    /// What a CLI prints alongside its work.
    ///
    /// Both runtimes point at the same stand-in, and which renderer the
    /// wrapper uses depends on the role's configured runtime — so a step
    /// prints both vocabularies and lets each side pick out its own. The other
    /// line passes through as an unknown event, which is what the renderers do
    /// with anything they do not recognise.
    const FAKE_USAGE: &'static str = concat!(
        r#"{"type":"assistant","message":{"id":"msg_1","usage":{"input_tokens":10,"#,
        r#""cache_creation_input_tokens":100,"cache_read_input_tokens":0,"output_tokens":5}}}"#,
        "\n",
        r#"{"type":"result","subtype":"success","total_cost_usd":0.25,"num_turns":3,"#,
        r#""duration_api_ms":4000,"usage":{"input_tokens":10,"cache_creation_input_tokens":100,"#,
        r#""cache_read_input_tokens":40,"output_tokens":5}}"#,
        "\n",
        r#"{"type":"turn.completed","usage":{"input_tokens":150,"cached_input_tokens":40,"#,
        r#""cache_write_input_tokens":100,"output_tokens":5,"reasoning_output_tokens":0}}"#,
    );

    /// A step that also prints usage, the way a real CLI does.
    ///
    /// Both runtimes point at the same stand-in, and which renderer the
    /// wrapper uses depends on the role's configured runtime — so the step
    /// prints both vocabularies and lets each side pick out its own. The
    /// other line passes through as an unknown event, which is what the
    /// renderers do with anything they do not recognise.
    fn doc_step_with_usage(&self, n: u32, slug: &str, body: &str) {
        let usage = Self::FAKE_USAGE;
        self.step(
            n,
            &format!(
                "{}cat <<'AUTOME_USAGE'\n{usage}\nAUTOME_USAGE\n",
                Self::doc_script(slug, n, body, "")
            ),
        );
    }

    /// What the retro round does: write `docs/<slug>/lessons.md` and leave
    /// the design document alone.
    fn retro_step(&self, n: u32, slug: &str) {
        self.retro_step_learning(n, slug, "证据文件必须逐字写出跑过的命令");
    }

    /// The same, with the lesson's sentence chosen by the caller — so two
    /// tasks can learn the same thing, which is what turns it into a rule.
    fn retro_step_learning(&self, n: u32, slug: &str, proposal: &str) {
        let usage = Self::FAKE_USAGE;
        self.step(
            n,
            &format!(
                r#"set -e
mkdir -p "docs/{slug}"
cat > "docs/{slug}/lessons.md" <<'AUTOME_EOF'
# 教训

```yaml
- id: L-01
  domain: verification
  symptom: 审计 #1 在 M-01 上要自己跑一遍验收命令
  root_cause: 实现轮的证据只写了结论，没有写命令
  evidence: docs/{slug}/lessons.md
  level: rule
  proposal: {proposal}
  predicted_impact: {{metric: verification_gaps, direction: down, scope: task, horizon: 3}}
```
AUTOME_EOF
git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session {n} retro" >/dev/null 2>&1 || true
cat <<'AUTOME_USAGE'
{usage}
AUTOME_USAGE
"#
            ),
        );
    }

    /// A step that writes a design document into the task's worktree and
    /// commits it, which is what every real session does.
    fn doc_step(&self, n: u32, slug: &str, body: &str) {
        self.step(n, &Self::doc_script(slug, n, body, ""));
    }

    /// A round that writes its document and leaves no evidence file. What
    /// every round did before the core started checking.
    fn doc_step_without_evidence(&self, n: u32, slug: &str, body: &str) {
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
    //   7 retro       -> lessons.md
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
    w.retro_step(7, &slug);

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
    w.retro_step(7, &slug);
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

/// The panel and the core must agree about whether a merge can proceed.
///
/// They did not: `merge_task_branch` was narrowed to "files this merge would
/// also write" while `task.changes` still reported any dirt at all, so the
/// panel showed 不可合并 for a merge the core would have performed. A real run
/// stalled there, blamed on a config file edited twenty minutes earlier.
#[test]
fn the_merge_panel_reports_what_the_core_would_actually_do() {
    needs_git!();
    let mut w = World::new("panel-agrees");
    let request = "panel agreement";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));

    // Uncommitted work in a file this merge does not touch — exactly what
    // changing a Loop setting leaves behind.
    std::fs::write(
        w.repo.join(".autome/config.toml"),
        "[roles.impl]\neffort = \"low\"\n",
    )
    .unwrap();

    let changes = w.call("task.changes", json!({ "task_id": task_id }));
    assert_eq!(
        ok(&changes)["mergeable"],
        json!(true),
        "the panel must not block on a file the merge never writes: {:#?}",
        ok(&changes)["blocked_by"]
    );

    // And the core agrees.
    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.state(&task_id), "done");
    assert_eq!(
        std::fs::read_to_string(w.repo.join(".autome/config.toml")).unwrap(),
        "[roles.impl]\neffort = \"low\"\n",
        "the user's uncommitted config is untouched"
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
        let slug = slug_for(request);
        w.doc_step_for(&slug, 1, &doc("设计中", 0, 0, &[]), "sleep 3");
    }

    let mut ids = Vec::new();
    for request in requests {
        ids.push(create_task(&mut w, request));
    }

    // A crashed session frees its slot, so a task that dies for an unrelated
    // reason makes the rest of this test pass for the wrong reason. That is
    // exactly what a shared step counter in the stand-in used to cause.
    for id in &ids {
        let panel = w.call("task.get", json!({ "task_id": id.clone() }));
        let state = &ok(&panel)["task"]["state"];
        assert_ne!(
            state["state"], "failed",
            "{id} must not have crashed: {state}"
        );
    }

    let running = w.ctx.store.all_running_sessions().unwrap().len();
    assert_eq!(running, 2, "both slots are held: {running} running");

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
    // The probe runs in the background, so a dashboard read may legitimately
    // answer before it has landed. What must always hold is that the payload
    // says which of the two it is rather than implying "fine".
    assert!(payload["environment"]["probed"].is_boolean());
    if payload["environment"]["probed"] == json!(true) {
        assert!(payload["environment"]["severity"].is_string());
    }
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
        // The renderer slot; `-` pipes the stream through unchanged, which is
        // what these two tests want — their subject is the wrapper's exit
        // handling, not the rendering.
        .arg("-")
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
        .arg("-")
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

// ---------------------------------------------------------------------------
// Scenario: the observation layer (plan §4.A)
// ---------------------------------------------------------------------------

/// The core ran for months recording that a session happened and nothing about
/// what it cost. Both CLIs were writing it the whole time.
#[test]
fn every_session_records_what_it_cost_and_the_finished_task_is_measured() {
    needs_git!();
    let mut w = World::new("usage");

    let request = "measure me";
    let slug = slug_for(request);
    w.doc_step_with_usage(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step_with_usage(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step_with_usage(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step_with_usage(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step_with_usage(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step_with_usage(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));

    // Every finished session has tokens and turns. Neither number existed
    // before; both come out of the CLI's own stream.
    let sessions = w.ctx.store.list_sessions(&task_id).unwrap();
    assert!(sessions.len() >= 6, "sessions: {}", sessions.len());
    for s in &sessions {
        assert!(
            s.metrics.total_tokens().unwrap_or(0) > 0,
            "{} ({}) recorded no tokens",
            s.id,
            s.runtime
        );
        assert!(s.metrics.turns.is_some(), "{} recorded no turns", s.id);
        assert!(
            s.protocol_ref.is_some(),
            "{} is not attributed to a protocol version",
            s.id
        );
    }

    // Cost is recorded for Claude and deliberately absent for Codex: Codex
    // reports no price, and inventing one from a table we maintain would
    // produce a number that looks authoritative and is not.
    for s in &sessions {
        match s.runtime {
            autome_domain::role::Runtime::Claude => {
                assert!(s.metrics.cost_usd.is_some(), "{} has no cost", s.id)
            }
            autome_domain::role::Runtime::Codex => {
                assert_eq!(s.metrics.cost_usd, None, "{} invented a price", s.id)
            }
        }
    }

    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle();

    let metrics = w.ctx.store.task_metrics(&task_id).unwrap().unwrap();
    assert_eq!(metrics.milestones, 1);
    assert_eq!(metrics.impl_rounds_used, 1);
    assert!(metrics.total_tokens > 0);
    assert!(metrics.total_turns > 0);
    assert!(
        metrics.protocol_ref.is_some(),
        "the task is not attributed to a protocol version"
    );
}

/// A closed milestone that is later taken back is the direct measurement of an
/// audit going soft — and it cannot be read off the finished document, which
/// shows the milestone as open and says nothing about it having been closed.
#[test]
fn a_milestone_closed_and_then_reopened_is_counted_as_contradicted() {
    needs_git!();
    let mut w = World::new("contradicted");

    let request = "soft audit";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(
        4,
        &slug,
        &doc("实现中", 2, 0, &[("M-01", "开放"), ("M-02", "开放")]),
    );
    // implement M-01, audit closes it, implement M-02 …
    w.doc_step(
        5,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "待审"), ("M-02", "开放")]),
    );
    w.doc_step(
        6,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "已完成"), ("M-02", "开放")]),
    );
    w.doc_step(
        7,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "已完成"), ("M-02", "待审")]),
    );
    // … and this audit takes M-01 back, having closed it two rounds ago.
    w.doc_step(
        8,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "开放"), ("M-02", "已完成")]),
    );
    w.doc_step(
        9,
        &slug,
        &doc("实现中", 2, 3, &[("M-01", "待审"), ("M-02", "已完成")]),
    );
    w.doc_step(
        10,
        &slug,
        &doc("实现中", 2, 3, &[("M-01", "已完成"), ("M-02", "已完成")]),
    );
    w.retro_step(11, &slug);

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));
    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle();

    let metrics = w.ctx.store.task_metrics(&task_id).unwrap().unwrap();
    assert_eq!(
        metrics.closed_then_contradicted, 1,
        "M-01 was closed and then taken back"
    );
}

// ---------------------------------------------------------------------------
// Scenario: the deterministic guards (plan §4.D)
// ---------------------------------------------------------------------------

/// Reads the failure reason a task stopped with.
fn failure_detail(w: &World, task_id: &str) -> String {
    let state = w.ctx.store.get_task(task_id).unwrap().state;
    serde_json::to_value(&state)
        .unwrap()
        .get("reason")
        .and_then(|r| r.get("detail"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The one rule the whole generation/evaluation split rests on. It was a
/// sentence in the protocol and nothing else; now the core checks it.
#[test]
fn an_implementation_round_that_closes_a_milestone_stops_the_task() {
    needs_git!();
    let mut w = World::new("guard-close");
    let request = "close it yourself";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    // The implementation round claims the thing only an audit may claim.
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();

    assert_eq!(w.state(&task_id), "failed");
    let detail = failure_detail(&w, &task_id);
    assert!(detail.contains("M-01"), "{detail}");
    assert!(detail.contains("只有审计轮"), "{detail}");

    // And the core did not quietly put the cell back: doing that would leave
    // the commit history and the session log telling different stories.
    let design = w
        .repo
        .join(format!(".worktree/{slug}/docs/{slug}/{slug}.md"));
    let text = std::fs::read_to_string(&design).unwrap();
    assert!(text.contains("| M-01 | 已完成"), "{text}");
}

/// A loop round with no evidence file has left nothing for the next round to
/// read, and the audit nothing to check against.
#[test]
fn a_loop_round_that_leaves_no_evidence_stops_the_task() {
    needs_git!();
    let mut w = World::new("guard-evidence");
    let request = "no evidence";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step_without_evidence(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();

    assert_eq!(w.state(&task_id), "failed");
    let detail = failure_detail(&w, &task_id);
    assert!(detail.contains("-r1-impl.md"), "{detail}");
}

/// A design document that keeps growing is a warning, not a failure: it is a
/// trend worth seeing, and stopping a task over it would cost more than it
/// saves.
#[test]
fn a_design_document_that_balloons_is_warned_about_and_the_loop_keeps_going() {
    needs_git!();
    let mut w = World::new("guard-size");
    let request = "grow the doc";
    let slug = slug_for(request);
    // 12KB of padding in one round, over the 10KB step warning.
    let padding = "证据正文。".repeat(3000);
    let fat = format!(
        "{}\n\n## 附录\n\n{padding}\n",
        doc("实现中", 2, 1, &[("M-01", "待审")])
    );
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &fat);
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();

    // The Loop reached the merge gate regardless.
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));
    let warnings = w.ctx.store.count_events(&task_id, "guard.warning").unwrap();
    assert!(warnings > 0, "the growth was not recorded");
}

// ---------------------------------------------------------------------------
// Scenario: the retro round (plan §E1)
// ---------------------------------------------------------------------------

/// What a task learned used to die in its directory. Now the loop ends with a
/// round whose whole job is to write it down in a form the core can read.
#[test]
fn the_loop_ends_with_a_retro_round_whose_lessons_the_core_can_read() {
    needs_git!();
    let mut w = World::new("retro-loop");
    let request = "learn something";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));

    // The retro round ran, as its own role, after the audit closed everything.
    let sessions = w.ctx.store.list_sessions(&task_id).unwrap();
    assert!(
        sessions.iter().any(|s| s.kind
            == autome_domain::session::SessionKind::Role {
                role: autome_domain::role::Role::Retro
            }),
        "no retro session: {:?}",
        sessions.iter().map(|s| s.kind).collect::<Vec<_>>()
    );

    // And what it wrote parsed against the schema, which is the whole point:
    // a lesson the core cannot read never reaches a rule.
    let lessons = w
        .ctx
        .store
        .last_event(&task_id, "task.lessons")
        .unwrap()
        .expect("lessons were never read");
    assert_eq!(lessons["count"], 1, "{lessons}");
    assert_eq!(lessons["lessons"][0]["domain"], "verification", "{lessons}");
}

/// A failed task never reaches the retro node, and a failed task is the most
/// informative kind there is. The user can ask for one.
#[test]
fn a_stopped_task_can_be_sent_through_a_retro_round_by_hand() {
    needs_git!();
    let mut w = World::new("retro-manual");
    let request = "fail then learn";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    // The implementation round claims the milestone closed; the core stops.
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(6, &slug);

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.state(&task_id), "failed");

    let before = w.ctx.store.get_task(&task_id).unwrap().state;
    let out = w.call("task.retro", json!({ "task_id": task_id }));
    assert!(
        matches!(out.reply.outcome, ReplyOutcome::Ok { .. }),
        "{:?}",
        out.reply.outcome
    );
    w.settle();

    // The task is exactly where it was: a retro changes nothing about where a
    // task stands, it only records what the run taught.
    assert_eq!(w.ctx.store.get_task(&task_id).unwrap().state, before);
    let lessons = w
        .ctx
        .store
        .last_event(&task_id, "task.lessons")
        .unwrap()
        .expect("lessons were never read");
    assert_eq!(lessons["count"], 1, "{lessons}");
}

#[test]
fn a_running_task_refuses_a_retro_by_hand() {
    needs_git!();
    let mut w = World::new("retro-running");
    let request = "still going";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    let task_id = create_task(&mut w, request);
    w.settle();

    // Waiting for approval is still a running task; the retro round will get
    // its turn at the end of the loop.
    let out = w.call("task.retro", json!({ "task_id": task_id }));
    assert!(
        matches!(out.reply.outcome, ReplyOutcome::Error { .. }),
        "{:?}",
        out.reply.outcome
    );
}

// ---------------------------------------------------------------------------
// Scenario: the per-round brief and the single protocol copy (plan §B)
// ---------------------------------------------------------------------------

/// Every session used to open by reading the whole design document, which
/// reached 230–335KB across three real runs and sat in the context for every
/// turn after. The brief is the index the core can assemble instead.
#[test]
fn every_round_is_handed_a_brief_and_the_protocol_is_frozen_once_per_task() {
    needs_git!();
    let mut w = World::new("brief");
    let request = "brief me";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);

    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));

    let wt = w.repo.join(format!(".worktree/{slug}"));

    // The task holds exactly one copy of the protocol, frozen at creation.
    let frozen = wt.join(format!("docs/{slug}/protocol/loop-protocol.md"));
    assert!(frozen.exists(), "{}", frozen.display());
    let task_file =
        std::fs::read_to_string(wt.join(format!("docs/{slug}/{slug}-task.md"))).unwrap_or_default();
    // The fake CLI writes no task file; what matters is that the scaffold no
    // longer carries a second copy to disagree with the first.
    let _ = task_file;
    assert!(
        !w.repo.join(".autome/skill/loop-protocol.md").exists(),
        "the project scaffold still mirrors the protocol"
    );

    // And the task recorded which version it is being held to.
    let task = w.ctx.store.get_task(&task_id).unwrap();
    let protocol_ref = task.protocol_ref.expect("no protocol_ref");
    assert!(protocol_ref.starts_with("protocol/v1@"), "{protocol_ref}");

    // Each role that ran got a brief of its own, naming the milestone it is
    // about and carrying the protocol sections its role is mapped to.
    let briefs = std::fs::read_dir(wt.join(format!("docs/{slug}/brief")))
        .expect("no brief directory")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    for role in ["plan", "review", "adjudicate", "impl", "audit", "retro"] {
        assert!(
            briefs.iter().any(|b| b.starts_with(role)),
            "{role} got no brief: {briefs:?}"
        );
    }

    let impl_brief =
        std::fs::read_to_string(wt.join(format!("docs/{slug}/brief/impl-1.md"))).unwrap();
    assert!(impl_brief.contains("| M-01 |"), "{impl_brief}");
    assert!(impl_brief.contains("## 实现循环"), "{impl_brief}");
    assert!(impl_brief.contains("不是设计文档的替代品"), "{impl_brief}");

    // The audit round is given the evidence *path* and told not to start
    // there: reading the implementation round's reasoning is what an
    // independent re-verification must not do.
    let audit_brief =
        std::fs::read_to_string(wt.join(format!("docs/{slug}/brief/audit-1.md"))).unwrap();
    assert!(audit_brief.contains("M-01-r1-impl.md"), "{audit_brief}");
    assert!(audit_brief.contains("先不要读它"), "{audit_brief}");
}

// ---------------------------------------------------------------------------
// Scenario: the meta task (plan §6)
// ---------------------------------------------------------------------------

/// Improving the protocol is an ordinary Loop task on an ordinary project.
/// What the core adds is the evidence, because a session cannot read the store
/// and should not be trusted to summarise its own history.
#[test]
fn a_meta_task_runs_on_the_protocol_repository_with_its_evidence_assembled() {
    needs_git!();
    let mut w = World::new("meta");

    // A finished task in an ordinary project, so there is something to cite.
    let request = "teach me something";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle();

    // The protocol repository is a project the scheduler already knows how to
    // run, registered rather than special-cased.
    let out = w.call("protocol.improve", json!({}));
    let payload = ok(&out);
    let meta_id = payload["task"]["id"].as_str().unwrap().to_string();
    let meta_slug = payload["task"]["slug"].as_str().unwrap().to_string();

    let projects = w.ctx.store.list_projects().unwrap();
    let protocol = projects
        .iter()
        .find(|p| p.display_name == "Loop 协议")
        .expect("the protocol repository is not a project");
    assert_eq!(protocol.parallel_limit, 1, "two meta tasks would conflict");

    // A second one is refused while the first is unfinished.
    let again = w.call("protocol.improve", json!({}));
    assert!(
        matches!(again.reply.outcome, ReplyOutcome::Error { .. }),
        "{:?}",
        again.reply.outcome
    );

    // The evidence is in the meta task's own directory, on its own branch.
    let wt = std::path::Path::new(&protocol.path)
        .join(".worktree")
        .join(&meta_slug);
    let inputs = wt.join(format!("docs/{meta_slug}/inputs"));
    for name in [
        "metrics.md",
        "lessons.md",
        "retro-suggestions.md",
        "failures.md",
        "deferred.md",
    ] {
        assert!(inputs.join(name).exists(), "missing inputs/{name}");
    }
    let metrics = std::fs::read_to_string(inputs.join("metrics.md")).unwrap();
    assert!(
        metrics.contains(&slug),
        "the finished task is not cited:\n{metrics}"
    );
    assert!(metrics.contains("protocol/v1@"), "{metrics}");

    // And the request carries the constraints that make a proposal checkable.
    let task = w.ctx.store.get_task(&meta_id).unwrap();
    assert!(task.request.contains("契约区"), "{}", task.request);
    assert!(task.request.contains("两个任务"), "{}", task.request);
}

/// Nothing starts by itself. The triggers are a suggestion with a reason
/// attached.
#[test]
fn the_app_suggests_an_iteration_only_once_there_is_something_to_say() {
    needs_git!();
    let mut w = World::new("meta-trigger");
    let out = w.call("protocol.triggers", json!({}));
    let before = ok(&out).clone();
    assert_eq!(before["suggest"], false, "{before}");

    let request = "one finished task";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    // The implementation round claims the close; the guard stops the task.
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    let task_id = create_task(&mut w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.state(&task_id), "failed");

    // One protocol failure is enough on its own: a task that could not be
    // held to the rules is the most direct evidence there is about them.
    let out = w.call("protocol.triggers", json!({}));
    let after = ok(&out).clone();
    assert_eq!(after["suggest"], true, "{after}");
    let reasons = after["triggers"].as_array().unwrap();
    assert!(
        reasons.iter().any(|r| r.as_str().unwrap().contains(&slug)),
        "{after}"
    );
}

// ---------------------------------------------------------------------------
// Scenario: a lesson becoming a rule (plan §E2, §E3)
// ---------------------------------------------------------------------------

/// Runs one whole task to a merge, with a retro that learns `proposal`.
fn run_a_task(w: &mut World, request: &str, proposal: &str) -> String {
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step_learning(7, &slug, proposal);
    let id = create_task(w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": id }));
    w.settle();
    w.call("task.merge", json!({ "task_id": id }));
    w.settle();
    // The step counter is per repository, so the next task starts again at 1.
    std::fs::write(w.repo.join(".autome/fake/next"), "1").unwrap();
    id
}

/// One task is a bad week. Two is a rule.
#[test]
fn the_same_lesson_from_two_tasks_becomes_a_project_rule_after_the_user_approves() {
    needs_git!();
    let mut w = World::new("curation");
    let lesson = "证据文件必须逐字写出跑过的命令";

    run_a_task(&mut w, "first task", lesson);

    // After one task there is nothing to propose.
    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let after_one = ok(&out).clone();
    assert_eq!(
        after_one["proposals"].as_array().unwrap().len(),
        0,
        "{after_one}"
    );

    run_a_task(&mut w, "second task", lesson);

    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let after_two = ok(&out).clone();
    let proposals = after_two["proposals"].as_array().unwrap();
    assert_eq!(proposals.len(), 1, "{after_two}");
    let p = &proposals[0];
    assert_eq!(p["file"], ".autome/rules/verification.md");
    assert_eq!(p["tasks"].as_array().unwrap().len(), 2, "{p}");
    // What the user approves is the exact text that gets written, provenance
    // included.
    let diff = p["diff"].as_str().unwrap();
    assert!(diff.contains(lesson), "{diff}");
    assert!(diff.contains("since:"), "{diff}");
    assert!(diff.contains("first-task"), "{diff}");

    // Nothing is written until the user says so.
    let rule_file = w.repo.join(".autome/rules/verification.md");
    assert!(!rule_file.exists(), "the rule was written without approval");

    let key = p["key"].as_str().unwrap().to_string();
    let out = w.call(
        "rules.decide",
        json!({ "project_id": w.project_id, "key": key, "approve": true }),
    );
    assert!(matches!(out.reply.outcome, ReplyOutcome::Ok { .. }));
    let text = std::fs::read_to_string(&rule_file).unwrap();
    assert!(text.contains(lesson), "{text}");
    assert!(
        text.contains("移除实验"),
        "the file explains itself:\n{text}"
    );

    // And it is not offered again.
    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let after_approve = ok(&out).clone();
    assert_eq!(
        after_approve["proposals"].as_array().unwrap().len(),
        0,
        "{after_approve}"
    );
}

/// A rule is never retired for having gone quiet: its absence from recent
/// lessons is caused by its presence. Removal is an experiment.
#[test]
fn removing_a_rule_is_an_experiment_with_a_baseline_and_a_way_back() {
    needs_git!();
    let mut w = World::new("retire");
    let lesson = "证据文件必须逐字写出跑过的命令";
    run_a_task(&mut w, "first task", lesson);
    run_a_task(&mut w, "second task", lesson);

    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let key = ok(&out)["proposals"][0]["key"]
        .as_str()
        .unwrap()
        .to_string();
    w.call(
        "rules.decide",
        json!({ "project_id": w.project_id, "key": key, "approve": true }),
    );

    let rule_file = w.repo.join(".autome/rules/verification.md");
    assert!(
        std::fs::read_to_string(&rule_file)
            .unwrap()
            .contains(lesson)
    );

    let out = w.call(
        "rules.retire",
        json!({
            "project_id": w.project_id,
            "file": ".autome/rules/verification.md",
            "body": lesson,
        }),
    );
    let started = ok(&out).clone();
    assert_eq!(started["metric"], "verification_gaps", "{started}");
    assert_eq!(started["horizon"], 3, "{started}");
    let text = std::fs::read_to_string(&rule_file).unwrap();
    assert!(!text.contains(lesson), "{text}");
    assert!(
        !text.contains("first-task L-01"),
        "an orphan comment:\n{text}"
    );

    // The experiment is running and not yet judgeable.
    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let listed = ok(&out).clone();
    let experiments = listed["experiments"].as_array().unwrap();
    assert_eq!(experiments.len(), 1, "{listed}");
    assert_eq!(experiments[0]["verdict"], "还没到期", "{listed}");

    // And there is a way back.
    let id = experiments[0]["id"].as_str().unwrap().to_string();
    let out = w.call(
        "rules.restore",
        json!({ "project_id": w.project_id, "id": id }),
    );
    assert!(matches!(out.reply.outcome, ReplyOutcome::Ok { .. }));
    let text = std::fs::read_to_string(&rule_file).unwrap();
    assert!(text.contains(lesson), "{text}");
}
