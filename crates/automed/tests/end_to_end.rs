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

/// Shared hang detector for scheduling waits, not a performance assertion.
/// Each poll advances scheduling and checks its own observable condition.
fn scheduling_wait<W>(
    world: &mut W,
    mut poll: impl FnMut(&mut W) -> bool,
    diagnostic: impl Fn(&W) -> String,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        if poll(world) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "scheduling wait did not finish within 90s: {}",
            diagnostic(world)
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn has_pending_work(ctx: &Ctx) -> bool {
    if !ctx
        .store
        .all_running_sessions()
        .unwrap_or_default()
        .is_empty()
    {
        return true;
    }
    ctx
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
                state == "queued"
                    || matches!(
                        node,
                        "rebase" | "merging" | "cleanup" | "gate" | "merge_gate" | "main_check" | "repro_check" | "fix_gate"
                    )
                    // A scheduled retry remains work even before it is due.
                    || matches!(
                        ctx.store.pending_retry(&t.id),
                        Ok(Some(_))
                    )
            })
}

/// Incomplete stand-in scripts deliberately stop at a retry, not at quiet.
fn wait_for_retry(ctx: &mut Ctx, task_id: &str) {
    scheduling_wait(
        ctx,
        |ctx| {
            let report = automed::scheduler::tick(ctx);
            assert!(report.errors.is_empty(), "{report:?}");
            ctx.store.pending_retry(task_id).unwrap().is_some()
                && ctx.store.running_session(task_id).unwrap().is_none()
        },
        |ctx| {
            format!(
                "waiting for retry: {:?}",
                ctx.store.get_task(task_id).unwrap()
            )
        },
    );
}

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
# Finds the project root by walking up from wherever we were started, then
# runs the next queued step script. Each step writes whatever a model would
# have written for that node.
#
# Walking up rather than asking git: a workspace task's working directory is
# not inside any repository — it is the directory the member checkouts sit in
# — and `git rev-parse` there answers nothing (or, worse, names a stray
# repository above it). `.autome/` sits at the project root in both shapes,
# so looking for it finds the same place either way.
set -u
dir=$PWD
fake=""
while [ "$dir" != "/" ]; do
  if [ -d "$dir/.autome/fake" ]; then fake="$dir/.autome/fake"; break; fi
  dir=$(dirname "$dir")
done
[ -n "$fake" ] || exit 90
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

    /// A design-document step that runs `prelude` first, in the worktree.
    fn doc_step_for_prelude(&self, n: u32, slug: &str, body: &str, prelude: &str) {
        self.step(n, &Self::doc_script(slug, n, body, prelude));
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

    fn doc_step_with_cost(&self, n: u32, slug: &str, body: &str, cost: f64) {
        let usage = Self::FAKE_USAGE.replace(
            "\"total_cost_usd\":0.25",
            &format!("\"total_cost_usd\":{cost}"),
        );
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
        scheduling_wait(
            self,
            |w| {
                automed::scheduler::tick(&mut w.ctx);
                !w.has_pending_work()
            },
            |w| {
                format!(
                    "sessions: {:?}, tasks: {:?}",
                    w.sessions_run(),
                    w.task_states()
                )
            },
        );
    }

    fn has_pending_work(&self) -> bool {
        has_pending_work(&self.ctx)
    }

    /// Adds lines to the project's `.autome/config.toml` and commits them, so
    /// the main worktree stays clean for the merge.
    fn configure(&self, toml: &str) {
        let path = self.repo.join(".autome/config.toml");
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        text.push('\n');
        text.push_str(toml);
        std::fs::write(&path, text).unwrap();
        automed::git::commit_paths(&self.repo, &[".autome/config.toml"], "test config").unwrap();
    }

    /// The prompt the Nth session was given.
    fn prompt(&self, n: u32) -> String {
        std::fs::read_to_string(self.repo.join(format!(".autome/fake/{n}.prompt")))
            .unwrap_or_default()
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
fn model_not_found_fails_as_config_without_retry() {
    use autome_domain::task::{FailureReason, TaskState};

    // Do not skip when Git is unavailable: that is a broken reproduction.
    let mut w = World::new("model-not-found");
    w.configure(
        "[roles.plan]\nruntime = \"codex\"\nmodel = \"missing-model-regression\"\n\
         [retry]\nattempts = 2\nbackoff_seconds = 0\n",
    );
    // Synthetic fixture based on the task card, not a captured provider run.
    // Queue all three attempts so the old behavior fails on the contract,
    // rather than on an absent fake-CLI step or a backoff timeout.
    let request = "missing model regression";
    w.doc_step(1, &slug_for(request), &doc("设计中", 0, 0, &[]));
    for n in 2..=4 {
        w.step(
            n,
            r#"cat <<'MODEL_ERROR'
{"type":"turn.failed","error":{"message":"model_not_found: The model `missing-model-regression` does not exist"}}
MODEL_ERROR
exit 1
"#,
        );
    }
    let id = create_task(&mut w, request);
    w.settle();
    let state = w.ctx.store.get_task(&id).unwrap().state;
    let retries = w.ctx.store.count_events(&id, "session.retry").unwrap();
    assert_eq!(
        retries, 0,
        "model error must not retry; final state: {state:?}"
    );
    assert_eq!(w.sessions_run(), vec!["1", "2"]);
    let TaskState::Failed {
        reason: FailureReason::Config { detail },
        ..
    } = state
    else {
        panic!("expected Failed(Config), got {state:?}");
    };
    for expected in [
        "plan",
        "codex",
        "missing-model-regression",
        "~/.autome/config.toml",
        "roles.plan.model",
    ] {
        assert!(
            detail.contains(expected),
            "missing {expected:?} in {detail:?}"
        );
    }
}

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
    wait_for_retry(&mut w.ctx, &task_id);

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
fn an_intake_fenced_status_block_reports_the_fence_location() {
    needs_git!();
    let mut w = World::new("fenced-status");
    let request = "fenced intake header";
    let bare = doc("设计中", 0, 0, &[]);
    assert!(autome_domain::status_block::parse(&bare).is_ok());
    let (_, header) = bare.split_once('\n').unwrap();
    w.doc_step(
        1,
        &slug_for(request),
        &format!("# 标题\n```text\n{header}```\n"),
    );
    let task_id = create_task(&mut w, request);
    w.settle();

    assert_eq!(w.state(&task_id), "failed");
    let task = w.ctx.store.get_task(&task_id).unwrap();
    let state = serde_json::to_value(&task.state).unwrap();
    assert_eq!(state["reason"]["kind"], json!("protocol"));
    let reason = state["reason"].to_string();
    assert!(
        reason.contains("代码围栏") && reason.contains("第 2 行"),
        "{reason}"
    );
    assert!(!reason.contains("缺少状态字段"), "{reason}");
    assert_eq!(w.sessions_run().len(), 1, "no retry");
}

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
    // Retried twice, with no wait, before the task fails. The two retries run
    // steps 2 and 3, which do not exist — the stand-in exits 92 — so all
    // three attempts crash.
    w.configure("[retry]\nattempts = 2\nbackoff_seconds = 0\n");
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

    assert_eq!(
        w.sessions_run(),
        vec!["1", "2", "3"],
        "one try and two retries"
    );
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
    wait_for_retry(&mut w.ctx, &task_id);

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
    wait_for_retry(&mut w.ctx, &task_id);

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
    wait_for_retry(&mut w.ctx, &task_id);

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
    assert_eq!(protocol_ref, automed::protocol::current().0.to_wire());
    assert!(protocol_ref.starts_with("protocol/v6@"), "{protocol_ref}");
    // Taken from the binary: nothing on this machine holds the protocol.
    assert!(
        !w.ctx.autome_home.join("protocol").exists(),
        "a protocol repository was created"
    );

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
// Scenario: the protocol is built in
// ---------------------------------------------------------------------------

/// An older build registered `~/.autome/protocol/` as a project of its own so
/// a meta task could edit it. The protocol now ships in the binary; recovery
/// unregisters that project and cancels what was queued on it, and leaves the
/// directory alone.
#[test]
fn recovery_retires_the_old_built_in_protocol_project() {
    needs_git!();
    let mut w = World::new("retire-protocol");
    let legacy = w.ctx.autome_home.join("protocol");
    std::fs::create_dir_all(&legacy).unwrap();
    automed::git::init(&legacy, "main").unwrap();
    std::fs::write(legacy.join("loop-protocol.md"), "# Loop 协议\n").unwrap();
    automed::git::commit_paths(&legacy, &["loop-protocol.md"], "v1").unwrap();

    let added = w.call("project.add", json!({ "path": legacy.to_str().unwrap() }));
    let legacy_id = ok(&added)["project"]["id"].as_str().unwrap().to_string();
    let created = w.call(
        "task.create",
        json!({ "project_id": legacy_id, "request": "改进 Loop 协议" }),
    );
    let task_id = ok(&created)["task"]["id"].as_str().unwrap().to_string();

    automed::scheduler::recover(&mut w.ctx);

    let projects = w.ctx.store.list_projects().unwrap();
    assert!(
        projects.iter().all(|p| p.id != legacy_id),
        "the protocol project is still registered"
    );
    assert!(
        projects.iter().any(|p| p.id == w.project_id),
        "an ordinary project was removed with it"
    );
    assert_eq!(w.state(&task_id), "cancelled");
    assert!(
        legacy.join("loop-protocol.md").exists(),
        "the directory was touched"
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

/// The gap this feature sat in for its whole life.
///
/// Everything needed to *run* a removal experiment existed — the method, the
/// table, the card, the test above — and nothing produced the arguments, so
/// no screen could ever start one. `rules.retire` refuses a body that is not
/// already in the file, and neither the proposal list (which drops a
/// proposal once it is answered) nor the rule-file list (filenames only) can
/// supply one. The candidate list closes it, and the join is the thing worth
/// asserting: what the panel offers has to be exactly what retire accepts.
#[test]
fn a_rule_gone_quiet_is_offered_for_removal_with_arguments_retire_accepts() {
    needs_git!();
    let mut w = World::new("retire-offer");
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

    // Freshly approved, it is not offered: the second task needed it.
    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let fresh = ok(&out).clone();
    assert_eq!(
        fresh["retirement_candidates"].as_array().unwrap().len(),
        0,
        "a rule needed one task ago is not a retirement candidate:\n{fresh}"
    );

    // Tasks that learn something else leave it unmentioned. Each lesson here
    // is distinct, so none of them corroborates into a rule of its own.
    for i in 0..6 {
        run_a_task(&mut w, &format!("later task {i}"), &format!("无关教训 {i}"));
    }

    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let listed = ok(&out).clone();
    let candidates = listed["retirement_candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 1, "{listed}");
    let c = &candidates[0];
    assert_eq!(c["idle_tasks"], 6, "{c}");
    assert_eq!(c["file"], ".autome/rules/verification.md", "{c}");
    assert_eq!(c["body"], lesson, "{c}");
    // The prediction is that nothing gets worse — not that anything improves.
    assert_eq!(c["direction"], "flat", "{c}");
    assert_eq!(c["metric"], "verification_gaps", "{c}");

    // The join. These two fields, handed straight back, have to be accepted.
    let out = w.call(
        "rules.retire",
        json!({
            "project_id": w.project_id,
            "file": c["file"],
            "body": c["body"],
        }),
    );
    let started = ok(&out).clone();
    assert_eq!(started["metric"], "verification_gaps", "{started}");

    // And having become an experiment, it stops being offered — it is not in
    // the file to remove twice.
    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let after = ok(&out).clone();
    assert_eq!(
        after["retirement_candidates"].as_array().unwrap().len(),
        0,
        "{after}"
    );
    assert_eq!(after["experiments"].as_array().unwrap().len(), 1, "{after}");

    // Putting it back must not put it straight back on the offer list. An
    // experiment that ended in a restore is the strongest evidence this
    // mechanism can produce that the rule is doing something — and the rule is
    // still exactly as quiet as it was, so filtering on "an experiment is
    // *running*" rather than "an experiment has *happened*" would ask the user
    // to re-run the one that already answered.
    let id = after["experiments"][0]["id"].as_str().unwrap().to_string();
    w.call(
        "rules.restore",
        json!({ "project_id": w.project_id, "id": id }),
    );
    assert!(
        std::fs::read_to_string(w.repo.join(".autome/rules/verification.md"))
            .unwrap()
            .contains(lesson),
        "the rule is back in the file"
    );
    let out = w.call("rules.proposals", json!({ "project_id": w.project_id }));
    let restored = ok(&out).clone();
    assert_eq!(
        restored["retirement_candidates"].as_array().unwrap().len(),
        0,
        "a restored rule is not offered for removal again:\n{restored}"
    );
}

// ---------------------------------------------------------------------------
// Workspaces: one project, several independent repositories
// ---------------------------------------------------------------------------

/// A workspace and its member repositories, laid out the way people actually
/// have one: a plain directory, each child its own repository with its own
/// history, nothing at the root.
struct Workspace {
    root: PathBuf,
    ctx: Ctx,
    project_id: String,
}

impl Workspace {
    fn new(tag: &str, members: &[&str]) -> Self {
        install_fake_cli();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("automed-ws-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let autome_home = root.join("autome-home");
        let home = root.join("home");
        let ws = root.join("ws");
        for d in [&autome_home, &home, &ws] {
            std::fs::create_dir_all(d).unwrap();
        }
        for m in members {
            let dir = ws.join(m);
            std::fs::create_dir_all(&dir).unwrap();
            automed::git::init(&dir, "main").unwrap();
            std::fs::write(dir.join("README.md"), format!("{m}\n")).unwrap();
            automed::git::commit_paths(&dir, &["README.md"], "initial").unwrap();
        }

        let store = Store::open_in_memory().unwrap();
        let mut ctx = Ctx::new(store, &autome_home, &home).headless();
        let added = call(
            &mut ctx,
            "project.add",
            json!({ "path": ws.to_str().unwrap(), "workspace": true, "docs_repo": "docs" }),
        );
        let project_id = ok(&added)["project"]["id"].as_str().unwrap().to_string();
        call(
            &mut ctx,
            "project.onboarding.skip",
            json!({ "project_id": project_id }),
        );

        std::fs::create_dir_all(ws.join(".autome/fake")).unwrap();
        std::fs::write(ws.join(".autome/fake/next"), "1").unwrap();

        Workspace {
            root,
            ctx,
            project_id,
        }
    }

    fn ws(&self) -> PathBuf {
        self.root.join("ws")
    }

    fn member(&self, name: &str) -> PathBuf {
        self.ws().join(name)
    }

    fn step(&self, n: u32, script: &str) {
        std::fs::write(self.ws().join(format!(".autome/fake/{n}.sh")), script).unwrap();
    }

    /// What a round writes, from wherever that round runs.
    ///
    /// `docs_prefix` is the path from this round's working directory to the
    /// document checkout: empty for every round but intake, which runs at the
    /// workspace root and therefore has to reach down into `.worktree/`.
    fn doc_step(&self, n: u32, slug: &str, docs_prefix: &str, body: &str, extra: &str) {
        let doc_dir = if docs_prefix.is_empty() {
            format!("docs/autome/{slug}")
        } else {
            format!("{docs_prefix}/docs/autome/{slug}")
        };
        let docs_checkout = if docs_prefix.is_empty() {
            "docs".to_string()
        } else {
            format!("{docs_prefix}/docs")
        };
        self.step(
            n,
            &format!(
                r#"set -e
mkdir -p "{doc_dir}/evidence"
cat > "{doc_dir}/{slug}.md" <<'AUTOME_EOF'
{body}
AUTOME_EOF
k=$(grep -m1 '^implementation-round:' "{doc_dir}/{slug}.md" | sed 's#[^0-9]*\([0-9]*\)/.*#\1#')
if [ -n "${{k:-}}" ] && [ "$k" != "0" ]; then
  printf '命令：fake\n结果：通过\n' > "{doc_dir}/evidence/M-01-r$k-impl.md"
  printf '复验：fake\n结论：通过\n' > "{doc_dir}/evidence/M-01-r$k-audit.md"
  printf '轮次 | M-01 | 通过 | e | 无\n' >> "{doc_dir}/retro.md"
fi
{extra}
cd "{docs_checkout}"
git add -A . >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "docs {n}" >/dev/null 2>&1 || true
"#
            ),
        );
    }

    /// The same settling rule the single-repository harness uses, rather than
    /// a second one: a test that waits differently from every other test here
    /// is a test whose failures mean something different from theirs.
    fn settle(&mut self) {
        scheduling_wait(
            self,
            |w| {
                automed::scheduler::tick(&mut w.ctx);
                !has_pending_work(&w.ctx)
            },
            |w| format!("tasks: {:?}", w.ctx.store.list_unfinished().unwrap()),
        );
    }

    /// Everything known about why a task is not where it should be.
    ///
    /// This test failed twice in CI and never once on this machine, across
    /// twenty repeats and a full `ci-local.sh` run. Rather than guess at a
    /// cause that cannot be reproduced, the next failure is made to say what
    /// happened: the state, the rounds the stand-in actually ran, and the
    /// events the core recorded.
    fn why(&self, task_id: &str) -> String {
        let task = self.ctx.store.get_task(task_id).unwrap();
        let history = std::fs::read_to_string(self.ws().join(".autome/fake/history"))
            .unwrap_or_else(|_| "(没有 history)".into())
            .replace('\n', " ");
        let events: Vec<String> = self
            .ctx
            .store
            .events_since(0, 200)
            .unwrap_or_default()
            .into_iter()
            .map(|(seq, kind, payload)| format!("{seq} {kind} {payload}"))
            .collect();
        // The session logs say what the stand-in's git actually printed,
        // which is the difference between "exit 128" and a cause.
        let mut logs = String::new();
        let dir = self.ws().join(".autome/output/sessions").join(task_id);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut names: Vec<_> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "log"))
                .collect();
            names.sort();
            for path in names {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let tail: Vec<&str> = text.lines().rev().take(12).collect();
                logs.push_str(&format!(
                    "\n  --- {} ---\n  {}",
                    path.file_name().unwrap().to_string_lossy(),
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n  ")
                ));
            }
        }
        format!(
            "state={:?}\n跑过的步骤={history}\n事件=\n  {}\n会话日志:{logs}",
            task.state,
            events.join("\n  ")
        )
    }

    fn node(&self, task_id: &str) -> Option<String> {
        match self.ctx.store.get_task(task_id).unwrap().state {
            autome_domain::task::TaskState::Active { node } => Some(node.as_str().to_string()),
            _ => None,
        }
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn a_workspace_task_branches_in_every_repository_it_names_and_merges_them_all() {
    needs_git!();
    let mut w = Workspace::new("multi", &["docs", "backend", "untouched"]);
    let request = "add a dashboard";
    let slug = slug_for(request);

    // 1 intake runs at the workspace root — it is the round that decides which
    //   repositories the task touches, and it says so in `repos:`.
    w.doc_step(
        1,
        &slug,
        ".worktree/add-a-dashboard",
        &ws_doc("设计中", 0, 0, &[], "backend"),
        "",
    );
    // Every later round runs in the task's own directory, beside the checkouts.
    w.doc_step(2, &slug, "", &ws_doc("设计中", 1, 0, &[], "backend"), "");
    w.doc_step(3, &slug, "", &ws_doc("设计中", 1, 0, &[], "backend"), "");
    w.doc_step(
        4,
        &slug,
        "",
        &ws_doc("实现中", 2, 0, &[("M-01", "开放")], "backend"),
        "",
    );
    // The implementation round changes code in the *other* repository and
    // commits it there. That commit is the thing the merge has to carry.
    w.doc_step(
        5,
        &slug,
        "",
        &ws_doc("实现中", 2, 1, &[("M-01", "待审")], "backend"),
        r#"printf 'dashboard\n' > backend/feature.txt
(cd backend && git add -A . && git -c user.name=fake -c user.email=f@f commit -q -m "feat: dashboard")"#,
    );
    w.doc_step(
        6,
        &slug,
        "",
        &ws_doc("实现中", 2, 1, &[("M-01", "已完成")], "backend"),
        "",
    );
    w.doc_step(
        7,
        &slug,
        "",
        &ws_doc("已完成", 2, 1, &[("M-01", "已完成")], "backend"),
        &format!("printf '教训\n' > docs/autome/{slug}/lessons.md"),
    );

    let created = call(
        &mut w.ctx,
        "task.create",
        json!({ "project_id": w.project_id, "request": request }),
    );
    let task_id = ok(&created)["task"]["id"].as_str().unwrap().to_string();
    w.settle();

    // The intake round named `backend`, so it has a checkout and a branch; the
    // repository nobody named has neither.
    let dir = w.ws().join(".worktree").join(&slug);
    assert!(dir.join("docs").exists(), "documents");
    assert!(
        dir.join("backend").exists(),
        "the repository the round named"
    );
    assert!(
        !dir.join("untouched").exists(),
        "a repository nobody named is left alone"
    );
    assert!(automed::git::branch_exists(
        &w.member("backend"),
        &format!("autome/{slug}")
    ));
    assert!(!automed::git::branch_exists(
        &w.member("untouched"),
        &format!("autome/{slug}")
    ));

    assert_eq!(
        w.node(&task_id).as_deref(),
        Some("await_design_approval"),
        "{}",
        w.why(&task_id)
    );
    call(&mut w.ctx, "task.approve", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(
        w.node(&task_id).as_deref(),
        Some("await_merge"),
        "{}",
        w.why(&task_id)
    );

    // One press, every repository.
    call(&mut w.ctx, "task.merge", json!({ "task_id": task_id }));
    w.settle();

    let task = w.ctx.store.get_task(&task_id).unwrap();
    assert!(
        matches!(task.state, autome_domain::task::TaskState::Done),
        "{}",
        w.why(&task_id)
    );
    // The code landed on the member's own default branch…
    assert!(
        w.member("backend").join("feature.txt").exists(),
        "the code repository's main carries the change"
    );
    // …and so did the record of what happened.
    assert!(
        w.member("docs")
            .join(format!("autome/{slug}/{slug}.md"))
            .exists(),
        "the document repository's main carries the design document"
    );
    // Cleanup removed every checkout and every branch, in every member.
    assert!(!dir.exists(), "the task directory is gone");
    for m in ["docs", "backend"] {
        assert!(
            !automed::git::branch_exists(&w.member(m), &format!("autome/{slug}")),
            "{m} still has the task branch"
        );
    }
}

#[test]
fn a_round_that_forgets_the_repos_line_does_not_lose_the_repository() {
    // The set is the union of what is checked out and what the document says.
    // A round that rewrites the status block without `repos:` would otherwise
    // drop a repository that has a branch and commits — silently out of the
    // rebase, the merge and the cleanup, with its work done and never landed.
    needs_git!();
    let mut w = Workspace::new("forgets", &["docs", "backend"]);
    let request = "add a dashboard";
    let slug = slug_for(request);

    w.doc_step(
        1,
        &slug,
        ".worktree/add-a-dashboard",
        &ws_doc("设计中", 0, 0, &[], "backend"),
        "",
    );
    // From here on the document says nothing about repositories at all.
    for n in 2..=3 {
        w.doc_step(n, &slug, "", &doc("设计中", 1, 0, &[]), "");
    }

    let created = call(
        &mut w.ctx,
        "task.create",
        json!({ "project_id": w.project_id, "request": request }),
    );
    let task_id = ok(&created)["task"]["id"].as_str().unwrap().to_string();
    wait_for_retry(&mut w.ctx, &task_id);

    let dir = w.ws().join(".worktree").join(&slug);
    assert!(
        dir.join("backend").exists(),
        "the checkout the first round asked for is still there: {}",
        w.why(&task_id)
    );
}

#[test]
fn cancelling_a_workspace_task_takes_every_checkout_and_branch_with_it() {
    // Cancel used to be written for one repository: it removed a checkout
    // that was not one, deleted a branch in a directory that was not a
    // repository, and archived the documents to the workspace root where
    // nothing tracks them. Every call was `let _ =`, so all of it failed in
    // silence and left the checkouts and branches behind.
    needs_git!();
    let mut w = Workspace::new("cancel", &["docs", "backend"]);
    let request = "add a dashboard";
    let slug = slug_for(request);

    w.doc_step(
        1,
        &slug,
        ".worktree/add-a-dashboard",
        &ws_doc("设计中", 0, 0, &[], "backend"),
        "",
    );
    w.doc_step(2, &slug, "", &ws_doc("设计中", 1, 0, &[], "backend"), "");

    let created = call(
        &mut w.ctx,
        "task.create",
        json!({ "project_id": w.project_id, "request": request }),
    );
    let task_id = ok(&created)["task"]["id"].as_str().unwrap().to_string();
    wait_for_retry(&mut w.ctx, &task_id);

    let dir = w.ws().join(".worktree").join(&slug);
    assert!(dir.join("backend").exists(), "{}", w.why(&task_id));

    call(&mut w.ctx, "task.cancel", json!({ "task_id": task_id }));
    w.settle();

    assert!(!dir.exists(), "the task's directory is gone");
    for m in ["docs", "backend"] {
        assert!(
            !automed::git::branch_exists(&w.member(m), &format!("autome/{slug}")),
            "{m} still has the cancelled task's branch"
        );
    }
    // A repository the task only checked out is left exactly as it was.
    assert!(
        automed::git::is_clean(&w.member("backend")).unwrap_or(false),
        "backend was left dirty"
    );
    // The record of what was tried goes to the repository that holds the
    // documents, not to the workspace root.
    // Deliberately uncommitted, as the single-repository archive has always
    // been: moving a directory is Autome's, committing it is the user's.
    assert!(
        w.member("docs")
            .join(format!("autome/.archive/{slug}"))
            .exists(),
        "the documents were archived in the document repository"
    );
    assert!(
        !w.ws().join(format!("autome/.archive/{slug}")).exists(),
        "and not at the workspace root"
    );
}

/// A design document for a workspace task: the status block plus the `repos:`
/// line that names what it works in.
fn ws_doc(
    status: &str,
    design_round: u32,
    impl_round: u32,
    milestones: &[(&str, &str)],
    repos: &str,
) -> String {
    let base = doc(status, design_round, impl_round, milestones);
    base.replacen(
        "next-action: 无",
        &format!("next-action: 无\nrepos: {repos}"),
        1,
    )
}

// ---------------------------------------------------------------------------
// Decisions are a lease: carried into the round that applies them
// ---------------------------------------------------------------------------

/// The regression the 2026-10-11 plan named and nothing tested: an included
/// Backlog item has to appear in the prompt of the round meant to apply it,
/// and a round that crashes must hand it to the next one rather than spend it.
#[test]
fn an_included_backlog_item_reaches_the_next_prompt_and_survives_a_crash() {
    needs_git!();
    let mut w = World::new("lease");
    w.configure("[retry]\nattempts = 1\nbackoff_seconds = 0\n");
    let request = "configurable limits";
    let slug = slug_for(request);
    let backlog = "\n## Backlog\n\n- B-01 次数上限要可配置\n";
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(
        4,
        &slug,
        &format!("{}{backlog}", doc("实现中", 2, 0, &[("M-01", "开放")])),
    );
    // 5: the design round carrying the decision crashes before writing.
    w.step(5, "exit 3\n");
    // 6: its retry finishes the job; the review after it is not queued.
    w.doc_step(
        6,
        &slug,
        &doc("实现中", 3, 0, &[("M-01", "开放"), ("M-02", "开放")]),
    );

    let task_id = create_task(&mut w, request);
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_design_approval"));

    w.call(
        "task.decide",
        json!({ "task_id": task_id, "kind": "backlog", "item_id": "B-01", "disposition": "include" }),
    );
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();

    assert!(
        w.prompt(5).contains("B-01 次数上限要可配置"),
        "the round meant to apply the decision is told about it:\n{}",
        w.prompt(5)
    );
    assert!(
        w.prompt(6).contains("B-01 次数上限要可配置"),
        "the retry after the crash carries it again:\n{}",
        w.prompt(6)
    );
    let decisions = w.ctx.store.list_decisions(&task_id).unwrap();
    let b01 = decisions.iter().find(|d| d.item_id == "B-01").unwrap();
    assert!(
        b01.consumed_at.is_some(),
        "spent by the round that ended well"
    );
    assert!(b01.carried_by.is_none());
}

// ---------------------------------------------------------------------------
// The host-run gate
// ---------------------------------------------------------------------------

/// The project's gate as a script outside the repository: passes when the
/// commit has a `gate-ok` file, and — when told to — fails on the checkout
/// the core makes of the merge commit.
#[test]
fn gate_quick_feature_two_milestones_run_quick_each_round_and_full_once() {
    needs_git!();
    let mut w = World::new("quick-two");
    let quick_calls = w.root.join("quick-calls");
    let full_calls = w.root.join("full-calls");
    let quick = w.root.join("quick.sh");
    let full = w.root.join("full.sh");
    std::fs::write(
        &quick,
        format!(
            "echo \"$AUTOME_GATE_NODE $AUTOME_GATE_KIND\" >> '{}'",
            quick_calls.display()
        ),
    )
    .unwrap();
    std::fs::write(
        &full,
        format!(
            "echo \"$AUTOME_GATE_NODE $AUTOME_GATE_KIND\" >> '{}'",
            full_calls.display()
        ),
    )
    .unwrap();
    w.configure(&format!("[gate]\nreuse_for_main_check = false\nrun = 'before_merge'\ncommand = 'sh {}'\nquick = 'sh {}'\n",full.display(),quick.display()));
    let request = "quick two milestones";
    let slug = slug_for(request);
    for (n, body) in [
        (1, doc("设计中", 0, 0, &[])),
        (2, doc("设计中", 1, 0, &[])),
        (3, doc("设计中", 1, 0, &[])),
        (
            4,
            doc("实现中", 2, 0, &[("M-01", "开放"), ("M-02", "开放")]),
        ),
        (
            5,
            doc("实现中", 2, 1, &[("M-01", "待审"), ("M-02", "开放")]),
        ),
        (
            6,
            doc("实现中", 2, 1, &[("M-01", "已完成"), ("M-02", "开放")]),
        ),
        (
            7,
            doc("实现中", 2, 2, &[("M-01", "已完成"), ("M-02", "待审")]),
        ),
        (
            8,
            doc("实现中", 2, 2, &[("M-01", "已完成"), ("M-02", "已完成")]),
        ),
    ] {
        w.doc_step(n, &slug, &body);
    }
    w.retro_step(9, &slug);
    let id = create_task(&mut w, request);
    w.settle();
    ok(&w.call("task.approve", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(
        std::fs::read_to_string(&quick_calls).unwrap(),
        "gate quick\ngate quick\n"
    );
    assert_eq!(
        std::fs::read_to_string(&full_calls).unwrap(),
        "merge_gate full\n"
    );
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    for name in ["gate.started", "gate.result"] {
        let kinds: Vec<_> = events
            .iter()
            .filter(|(_, k, s, _)| s == &id && k == name)
            .map(|(_, _, _, p)| p["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["quick", "quick", "full"]);
    }
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    assert_eq!(
        std::fs::read_to_string(&full_calls).unwrap(),
        "merge_gate full\nmain_check full\n"
    );
}

#[test]
fn gate_quick_feature_failing_quick_returns_to_implement_with_its_tail() {
    needs_git!();
    let mut w = World::new("quick-failure");
    let quick_calls = w.root.join("quick-calls");
    let full_calls = w.root.join("full-calls");
    let quick = w.root.join("quick.sh");
    let full = w.root.join("full.sh");
    std::fs::write(
        &quick,
        format!(
            "echo quick >> '{}'\ntest -f gate-ok || {{ echo quick-missing-marker; exit 1; }}",
            quick_calls.display()
        ),
    )
    .unwrap();
    std::fs::write(
        &full,
        format!("echo $AUTOME_GATE_NODE >> '{}'", full_calls.display()),
    )
    .unwrap();
    let command = format!("sh {}", quick.display());
    w.configure(&format!(
        "[gate]\nrun = 'before_merge'\ncommand = 'sh {}'\nquick = '{command}'\n",
        full.display()
    ));
    let request = "quick repair";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step_for_prelude(
        6,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "待审")]),
        "echo ok > gate-ok\ngit add gate-ok",
    );
    w.doc_step(7, &slug, &doc("实现中", 2, 2, &[("M-01", "已完成")]));
    w.retro_step(8, &slug);
    let id = resume_setup(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert!(w.prompt(6).contains("quick-missing-marker"));
    assert!(w.prompt(6).contains(&command));
    assert_eq!(
        std::fs::read_to_string(quick_calls).unwrap(),
        "quick\nquick\n"
    );
    assert_eq!(std::fs::read_to_string(full_calls).unwrap(), "merge_gate\n");
}

#[test]
fn gate_quick_each_round_ignores_quick_event_for_event() {
    needs_git!();
    let mut sequences = Vec::new();
    for enabled in [false, true] {
        let mut w = World::new("quick-ignored");
        let calls = w.root.join("quick-calls");
        let extra = if enabled {
            format!("quick = 'echo unexpected >> {}'\n", calls.display())
        } else {
            String::new()
        };
        w.configure(&format!("[gate]\ncommand = 'true'\n{extra}"));
        let request = "quick ignored";
        let slug = slug_for(request);
        w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
        w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
        w.retro_step(7, &slug);
        let id = resume_setup(&mut w, request);
        w.settle();
        assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
        ok(&w.call("task.merge", json!({"task_id":id})));
        w.settle();
        assert_eq!(w.state(&id), "done");
        assert!(!calls.exists());
        let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
        sequences.push(
            events
                .into_iter()
                .filter(|(_, k, s, _)| s == &id && k.starts_with("gate."))
                .map(|(_, k, _, p)| (k, p["node"].clone(), p["kind"].clone()))
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(sequences[0], sequences[1]);
}

#[test]
fn feature_before_merge_two_milestones_run_the_gate_once_before_the_merge() {
    needs_git!();
    let mut w = World::new("before-merge-two");
    let calls = w.root.join("calls");
    w.configure(&format!(
        "[gate]\nreuse_for_main_check = false\nrun = \"before_merge\"\ncommand = \"echo $AUTOME_GATE_NODE >> {}\"\n",
        calls.display()
    ));
    let request = "two milestones before merge";
    let slug = slug_for(request);
    for (n, body) in [
        (1, doc("设计中", 0, 0, &[])),
        (2, doc("设计中", 1, 0, &[])),
        (3, doc("设计中", 1, 0, &[])),
        (
            4,
            doc("实现中", 2, 0, &[("M-01", "开放"), ("M-02", "开放")]),
        ),
        (
            5,
            doc("实现中", 2, 1, &[("M-01", "待审"), ("M-02", "开放")]),
        ),
        (
            6,
            doc("实现中", 2, 1, &[("M-01", "已完成"), ("M-02", "开放")]),
        ),
        (
            7,
            doc("实现中", 2, 2, &[("M-01", "已完成"), ("M-02", "待审")]),
        ),
        (
            8,
            doc("实现中", 2, 2, &[("M-01", "已完成"), ("M-02", "已完成")]),
        ),
    ] {
        w.doc_step(n, &slug, &body);
    }
    w.retro_step(9, &slug);
    let id = create_task(&mut w, request);
    w.settle();
    ok(&w.call("task.approve", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(std::fs::read_to_string(&calls).unwrap(), "merge_gate\n");
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    assert!(
        !events
            .iter()
            .any(|(_, k, s, p)| s == &id && k == "task.updated" && p["state"]["node"] == "gate")
    );
    let starts: Vec<_> = events
        .iter()
        .filter(|(_, k, s, _)| s == &id && k == "gate.started")
        .map(|(_, _, _, p)| p["node"].as_str().unwrap())
        .collect();
    assert_eq!(starts, ["merge_gate"]);
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    assert_eq!(
        std::fs::read_to_string(calls).unwrap(),
        "merge_gate\nmain_check\n"
    );
    // The reverse switch takes effect at the next implementation completion.
    let mut w = World::new("before-merge-reverse");
    w.configure("[gate]\nrun = \"before_merge\"\ncommand = \"true\"\n");
    let request = "reverse gate mode";
    let slug = slug_for(request);
    w.doc_step_for_prelude(
        5,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
        &format!(
            "touch '{}'\nwhile [ ! -f '{}' ] && [ -d '{}' ]; do sleep 0.02; done",
            w.root.join("impl-ready").display(),
            w.root.join("impl-release").display(),
            w.root.display()
        ),
    );
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("impl-ready").exists());
    let config = w.repo.join(".autome/config.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .replace("run = \"before_merge\"", "run = \"each_round\"");
    std::fs::write(config, text).unwrap();
    automed::git::commit_paths(&w.repo, &[".autome/config.toml"], "restore each round").unwrap();
    std::fs::write(w.root.join("impl-release"), "go").unwrap();
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    assert!(
        events
            .iter()
            .any(|(_, k, s, p)| s == &id && k == "gate.started" && p["node"] == "gate")
    );
    assert_eq!(w.ctx.store.count_events(&id, "gate.skipped").unwrap(), 0);
}

#[test]
fn feature_before_merge_a_failing_merge_gate_returns_to_implement_and_merges_after_the_fix() {
    needs_git!();
    let mut w = World::new("before-merge-failure");
    let script = write_gate_script(&w);
    let command = format!("sh {}", script.display());
    w.configure(&format!(
        "[gate]\nrun = \"before_merge\"\ncommand = \"{command}\"\n"
    ));
    let request = "before merge repair";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    w.doc_step_for_prelude(
        8,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "已完成")]),
        "echo ok > gate-ok\ngit add gate-ok",
    );
    w.doc_step(9, &slug, &doc("实现中", 2, 2, &[("M-01", "已完成")]));
    w.retro_step(10, &slug);
    let id = resume_setup(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert!(w.prompt(8).contains("gate says no: gate-ok is missing"));
    assert!(w.prompt(8).contains(&command));
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    let failed = events
        .iter()
        .find(|(_, k, s, p)| s == &id && k == "gate.result" && p["ok"] == false)
        .unwrap();
    assert_eq!(failed.3["node"], "merge_gate");
    let nodes: Vec<_> = events
        .iter()
        .filter(|(seq, k, s, _)| *seq > failed.0 && s == &id && k == "task.updated")
        .filter_map(|(_, _, _, p)| p["state"]["node"].as_str())
        .collect();
    assert_eq!(
        nodes,
        [
            "implement",
            "audit",
            "retro",
            "rebase",
            "merge_gate",
            "await_merge"
        ]
    );
    assert!(
        !events
            .iter()
            .any(|(_, k, s, p)| s == &id && k == "gate.started" && p["node"] == "gate")
    );
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
}

#[test]
fn brief_before_merge_audit_brief_never_shows_a_stale_result() {
    needs_git!();
    let mut w = World::new("brief-before-merge");
    let script = write_gate_script(&w);
    let command = format!("sh {}", script.display());
    w.configure(&format!(
        "[gate]\nrun = \"before_merge\"\ncommand = \"{command}\"\n"
    ));
    let request = "before merge repair";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    w.doc_step_for_prelude(
        8,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "已完成")]),
        "echo ok > gate-ok\ngit add gate-ok",
    );
    w.doc_step(9, &slug, &doc("实现中", 2, 2, &[("M-01", "已完成")]));
    w.retro_step(10, &slug);
    let id = resume_setup(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert!(w.prompt(8).contains("gate says no: gate-ok is missing"));
    assert!(w.prompt(8).contains(&command));
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    let failed = events
        .iter()
        .find(|(_, k, s, p)| s == &id && k == "gate.result" && p["ok"] == false)
        .unwrap();
    assert_eq!(failed.3["node"], "merge_gate");
    let failed_sha = failed.3["sha"].as_str().unwrap();
    let brief_dir = w.repo.join(format!(".worktree/{slug}/docs/{slug}/brief"));
    for round in [1, 2] {
        let text = std::fs::read_to_string(brief_dir.join(format!("audit-{round}.md"))).unwrap();
        assert!(text.contains("## 宿主门禁"), "{text}");
        assert!(
            text.contains(automed::brief::GATE_ONLY_BEFORE_MERGE),
            "{text}"
        );
        assert!(!text.contains(&failed_sha[..10]), "{text}");
        assert!(!text.contains("：未通过"), "{text}");
    }

    let nodes: Vec<_> = events
        .iter()
        .filter(|(seq, k, s, _)| *seq > failed.0 && s == &id && k == "task.updated")
        .filter_map(|(_, _, _, p)| p["state"]["node"].as_str())
        .collect();
    assert_eq!(
        nodes,
        [
            "implement",
            "audit",
            "retro",
            "rebase",
            "merge_gate",
            "await_merge"
        ]
    );
    assert!(
        !events
            .iter()
            .any(|(_, k, s, p)| s == &id && k == "gate.started" && p["node"] == "gate")
    );
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
}

#[test]
fn gate_quick_feature_removing_quick_while_running_paused_or_stopped() {
    needs_git!();
    for interrupt in [None, Some("task.pause"), Some("task.stop")] {
        quick_feature_remove_pending(interrupt);
    }
}

fn quick_feature_remove_pending(interrupt: Option<&str>) {
    let mut w = World::new("before-merge-switch");
    let script = w.root.join("gate.sh");
    std::fs::write(&script, format!("if [ \"$AUTOME_GATE_NODE\" = gate ]; then\n touch '{}'\n while [ -d '{}' ]; do sleep 0.02; done\nfi\n", w.root.join("gate-ready").display(), w.root.display())).unwrap();
    w.configure(&format!(
        "[gate]\nrun = \"before_merge\"\ncommand = \"true\"\nquick = \"sh {}\"\n",
        script.display()
    ));
    let request = "switch pending gate";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step_for_prelude(
        6,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "已完成")]),
        &format!(
            "touch '{}'\nwhile [ ! -f '{}' ] && [ -d '{}' ]; do sleep 0.02; done",
            w.root.join("audit-ready").display(),
            w.root.join("audit-release").display(),
            w.root.display()
        ),
    );
    w.retro_step(7, &slug);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("gate-ready").exists());
    assert_eq!(w.node(&id).as_deref(), Some("gate"));
    let head = automed::git::head_sha(&w.repo.join(format!(".worktree/{slug}"))).unwrap();
    let job = automed::gate::Job::at(&w.repo, &id, "gate", &head[..12]);
    let pid: i32 = std::fs::read_to_string(&job.pid)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    if let Some(method) = interrupt {
        ok(&w.call(method, json!({"task_id":id})));
    }
    let config = w.repo.join(".autome/config.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("quick ="))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(config, text).unwrap();
    automed::git::commit_paths(&w.repo, &[".autome/config.toml"], "switch gate mode").unwrap();
    if interrupt.is_some() {
        ok(&w.call("task.resume", json!({"task_id":id})));
    }
    resume_wait(&mut w, |w| w.root.join("audit-ready").exists());
    assert_eq!(w.node(&id).as_deref(), Some("audit"));
    assert!(!automed::launcher::pid_alive(pid));
    #[cfg(unix)]
    assert_ne!(
        unsafe { libc::kill(-pid, 0) },
        0,
        "gate process group survived"
    );
    assert!(!job.started.exists() && !job.pid.exists() && !job.exit.exists() && !job.log.exists());
    let skipped = w
        .ctx
        .store
        .last_event(&id, "gate.skipped")
        .unwrap()
        .unwrap();
    assert_eq!(skipped["node"], "gate");
    assert_eq!(skipped["reason"], "门禁只在合并前运行");
    assert_eq!(w.ctx.store.count_events(&id, "gate.started").unwrap(), 1);
    assert_eq!(w.ctx.store.count_events(&id, "gate.result").unwrap(), 0);
    std::fs::write(w.root.join("audit-release"), "go").unwrap();
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(w.ctx.store.count_events(&id, "gate.started").unwrap(), 2);
    assert_eq!(
        w.ctx.store.last_event(&id, "gate.result").unwrap().unwrap()["node"],
        "merge_gate"
    );
}

fn feature_before_merge_switch_gate(interrupt: Option<&str>) {
    let mut w = World::new("before-merge-switch");
    let script = w.root.join("gate.sh");
    std::fs::write(&script, format!("if [ \"$AUTOME_GATE_NODE\" = gate ]; then\n touch '{}'\n while [ -d '{}' ]; do sleep 0.02; done\nfi\n", w.root.join("gate-ready").display(), w.root.display())).unwrap();
    w.configure(&format!(
        "[gate]\nrun = \"each_round\"\ncommand = \"sh {}\"\n",
        script.display()
    ));
    let request = "switch pending gate";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step_for_prelude(
        6,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "已完成")]),
        &format!(
            "touch '{}'\nwhile [ ! -f '{}' ] && [ -d '{}' ]; do sleep 0.02; done",
            w.root.join("audit-ready").display(),
            w.root.join("audit-release").display(),
            w.root.display()
        ),
    );
    w.retro_step(7, &slug);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("gate-ready").exists());
    assert_eq!(w.node(&id).as_deref(), Some("gate"));
    let head = automed::git::head_sha(&w.repo.join(format!(".worktree/{slug}"))).unwrap();
    let job = automed::gate::Job::at(&w.repo, &id, "gate", &head[..12]);
    let pid: i32 = std::fs::read_to_string(&job.pid)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    if let Some(method) = interrupt {
        ok(&w.call(method, json!({"task_id":id})));
    }
    let config = w.repo.join(".autome/config.toml");
    let text = std::fs::read_to_string(&config)
        .unwrap()
        .replace("run = \"each_round\"", "run = \"before_merge\"");
    std::fs::write(config, text).unwrap();
    automed::git::commit_paths(&w.repo, &[".autome/config.toml"], "switch gate mode").unwrap();
    if interrupt.is_some() {
        ok(&w.call("task.resume", json!({"task_id":id})));
    }
    resume_wait(&mut w, |w| w.root.join("audit-ready").exists());
    assert_eq!(w.node(&id).as_deref(), Some("audit"));
    assert!(!automed::launcher::pid_alive(pid));
    #[cfg(unix)]
    assert_ne!(
        unsafe { libc::kill(-pid, 0) },
        0,
        "gate process group survived"
    );
    assert!(!job.started.exists() && !job.pid.exists() && !job.exit.exists() && !job.log.exists());
    let skipped = w
        .ctx
        .store
        .last_event(&id, "gate.skipped")
        .unwrap()
        .unwrap();
    assert_eq!(skipped["node"], "gate");
    assert_eq!(skipped["reason"], "门禁只在合并前运行");
    assert_eq!(w.ctx.store.count_events(&id, "gate.started").unwrap(), 1);
    assert_eq!(w.ctx.store.count_events(&id, "gate.result").unwrap(), 0);
    std::fs::write(w.root.join("audit-release"), "go").unwrap();
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(w.ctx.store.count_events(&id, "gate.started").unwrap(), 2);
    assert_eq!(
        w.ctx.store.last_event(&id, "gate.result").unwrap().unwrap()["node"],
        "merge_gate"
    );
}

#[test]
fn feature_before_merge_switching_while_blocked_on_the_gate_skips_it() {
    needs_git!();
    feature_before_merge_switch_gate(None);
    feature_before_merge_switch_gate(Some("task.pause"));
}

#[test]
fn feature_before_merge_switching_while_stopped_at_the_gate_skips_it_on_resume() {
    needs_git!();
    feature_before_merge_switch_gate(Some("task.stop"));
    for method in ["task.rerun_from", "task.extend_budget"] {
        let mut w = World::new("before-merge-tripped");
        let id = run_same_failure_task(&mut w, false);
        assert_same_failure_stopped(&mut w, &id);
        w.configure("run = \"before_merge\"\n");
        std::fs::write(w.root.join("environment-fixed"), "fixed").unwrap();
        let params = if method == "task.rerun_from" {
            json!({"task_id":id, "node":"gate"})
        } else {
            json!({"task_id":id, "extra_rounds":1})
        };
        ok(&w.call(method, params));
        w.settle();
        assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
        assert_eq!(same_failure_implementations(&w, &id), 3);
        let skipped = w
            .ctx
            .store
            .last_event(&id, "gate.skipped")
            .unwrap()
            .unwrap();
        assert_eq!(skipped["node"], "gate");
        let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|(_, k, s, p)| s == &id && k == "gate.started" && p["node"] == "gate")
                .count(),
            3
        );
        assert_eq!(
            w.ctx.store.last_event(&id, "gate.result").unwrap().unwrap()["node"],
            "merge_gate"
        );
    }
}

fn write_gate_script(w: &World) -> PathBuf {
    let script = w.root.join("gate.sh");
    std::fs::write(
        &script,
        format!(
            r#"case "$PWD" in
  *_verify-*) if [ -f "{root}/main-should-fail" ]; then echo "main is broken after the merge"; exit 1; fi ;;
esac
if [ -f gate-ok ]; then echo "gate fine on $AUTOME_HEAD"; exit 0; fi
echo "gate says no: gate-ok is missing"
exit 1
"#,
            root = w.root.display()
        ),
    )
    .unwrap();
    script
}

/// Runs a task through a gate that fails once and passes after the fix.
/// Returns the task id; the task is left at the merge stopping point.
fn run_gated_task(w: &mut World, request: &str) -> String {
    run_gated_task_with(w, request, false, None)
}

fn run_gated_task_with(w: &mut World, request: &str, count: bool, reuse: Option<bool>) -> String {
    let mut script = write_gate_script(w);
    if count {
        let wrapper = w.root.join("count-gate.sh");
        std::fs::write(
            &wrapper,
            format!(
                "echo \"$AUTOME_GATE_NODE\" >> '{}'\nexec sh '{}'\n",
                w.root.join("gate-calls").display(),
                script.display()
            ),
        )
        .unwrap();
        script = wrapper;
    }
    let reuse = reuse
        .map(|v| format!("reuse_for_main_check = {v}\n"))
        .unwrap_or_default();

    w.configure(&format!(
        "[gate]\ncommand = \"sh {}\"\ntimeout_minutes = 5\n{reuse}",
        script.display()
    ));
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    // 5: implementation round 1 claims it is done but leaves the gate red.
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    // 6: the round sent back by the gate fixes it.
    w.doc_step_for_prelude(
        6,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "待审")]),
        "echo ok > gate-ok\ngit add gate-ok",
    );
    w.doc_step(7, &slug, &doc("实现中", 2, 2, &[("M-01", "已完成")]));
    w.retro_step(8, &slug);

    let task_id = create_task(w, request);
    w.settle();
    w.call("task.approve", json!({ "task_id": task_id }));
    w.settle();
    task_id
}

#[test]
fn a_failing_gate_sends_the_work_back_and_a_passing_one_lets_it_merge() {
    needs_git!();
    let mut w = World::new("gate");
    let task_id = run_gated_task(&mut w, "gated feature");

    assert_eq!(
        w.node(&task_id).as_deref(),
        Some("await_merge"),
        "sessions: {:?}, state: {:?}",
        w.sessions_run(),
        w.ctx.store.get_task(&task_id).unwrap().state
    );
    let prompt = w.prompt(6);
    assert!(
        prompt.contains("gate says no: gate-ok is missing"),
        "the round after a red gate is shown its output:\n{prompt}"
    );
    assert!(prompt.contains("没有通过"), "{prompt}");
    // The audit round's brief carries the core's own run.
    let slug = slug_for("gated feature");
    let briefs = w.repo.join(format!(".worktree/{slug}/docs/{slug}/brief"));
    let audit_brief = std::fs::read_dir(&briefs)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains("audit"))
        .expect("an audit brief");
    let text = std::fs::read_to_string(audit_brief).unwrap();
    assert!(
        text.contains("## 宿主门禁") && text.contains("通过"),
        "{text}"
    );

    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle();
    assert_eq!(w.state(&task_id), "done");
    let result = w
        .ctx
        .store
        .last_event(&task_id, "gate.reused")
        .unwrap()
        .unwrap();
    assert_eq!(result["node"], json!("main_check"));
    assert_eq!(
        result["sha"],
        w.ctx
            .store
            .get_task(&task_id)
            .unwrap()
            .merge_commit
            .unwrap()
    );
    assert!(
        !gate_trace(&w, &task_id)
            .iter()
            .any(|e| e == "gate.started:main_check")
    );
    assert!(w.ctx.store.open_alerts().unwrap().is_empty());
    assert!(
        !w.repo
            .join(".worktree")
            .read_dir()
            .map(|mut d| d.any(|e| {
                e.map(|e| e.file_name().to_string_lossy().starts_with("_verify"))
                    .unwrap_or(false)
            }))
            .unwrap_or(false),
        "the checkout of the merge commit is removed"
    );
}

#[test]
fn main_failing_its_gate_after_a_merge_raises_an_alert_the_user_can_dismiss() {
    needs_git!();
    let mut w = World::new("maingate");
    let task_id = run_gated_task(&mut w, "breaks main");
    assert_eq!(w.node(&task_id).as_deref(), Some("await_merge"));
    std::fs::write(w.root.join("main-should-fail"), "").unwrap();

    merge_after_a_late_branch_commit(&mut w, &task_id);
    // The merge stands: the alert is about main, not a reason to undo it.
    assert_eq!(w.state(&task_id), "done");
    let alerts = w.ctx.store.open_alerts().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, "main_gate_failed");
    assert!(alerts[0].detail.contains("main is broken after the merge"));

    let dashboard = w.call("dashboard.get", json!({}));
    let shown = ok(&dashboard)["alerts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(shown.len(), 1, "{:#}", ok(&dashboard));

    w.call("project.alert.dismiss", json!({ "alert_id": alerts[0].id }));
    assert!(w.ctx.store.open_alerts().unwrap().is_empty());
}

// Real gate jobs with changing noise, controlled outside the worktree.
fn run_same_failure_task(w: &mut World, alternating: bool) -> String {
    let script = w.root.join("same-failure.sh");
    std::fs::write(
        &script,
        format!(
            r#"set -eu
root='{}'
n=$(cat "$root/gate-count" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$root/gate-count"
if [ -f gate-ok ] || [ -f "$root/environment-fixed" ]; then exit 0; fi
package=ws
if [ '{}' = true ] && [ $((n % 2)) = 0 ]; then package=zod; fi
echo "Error: Cannot find package '$package' imported from /tmp/work/run-$n/app.js at 127.0.0.1:$((40000 + n)) after $n.5s at 12:00:$n"
echo "Duration $n.5s"
echo '  FAILED  Web test'
exit 1
"#,
            w.root.display(), alternating
        ),
    )
    .unwrap();
    w.configure(&format!("[gate]\ncommand = \"sh {}\"\n", script.display()));
    let request = "same failure feature";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    let rounds = if alternating { 5 } else { 3 };
    for round in 1..=rounds {
        w.doc_step_for_prelude(
            4 + round,
            &slug,
            &doc("实现中", 2, round, &[("M-01", "待审")]),
            if alternating && round == 5 {
                "echo ok > gate-ok\ngit add gate-ok"
            } else {
                ""
            },
        );
    }
    w.doc_step(
        5 + rounds,
        &slug,
        &doc("实现中", 2, rounds, &[("M-01", "已完成")]),
    );
    w.retro_step(6 + rounds, &slug);
    let id = create_task(w, request);
    w.settle();
    ok(&w.call("task.approve", json!({"task_id": id})));
    w.settle();
    id
}

fn same_failure_implementations(w: &World, id: &str) -> usize {
    w.ctx
        .store
        .list_sessions(id)
        .unwrap()
        .iter()
        .filter(|s| s.kind.role().is_some_and(|r| r.as_str() == "impl"))
        .count()
}

fn same_failure_results(w: &World, id: &str) -> Vec<Value> {
    w.ctx
        .store
        .events_since_with_subject(0, 10_000)
        .unwrap()
        .into_iter()
        .filter(|(_, kind, subject, payload)| {
            kind == "gate.result" && subject == id && payload["node"] == "gate"
        })
        .map(|(_, _, _, payload)| payload)
        .collect()
}

fn assert_same_failure_stopped(w: &mut World, id: &str) -> Value {
    let state = serde_json::to_value(w.ctx.store.get_task(id).unwrap().state).unwrap();
    assert_eq!(state["state"], "failed", "{state}");
    assert_eq!(state["reason"]["kind"], "gate_same_failure");
    assert_eq!(state["reason"]["count"], 3);
    assert!(
        state["reason"]["summary"]
            .as_str()
            .unwrap()
            .contains("Cannot find package 'ws'")
    );
    let results = same_failure_results(w, id);
    assert_eq!(results.len(), 3);
    for (i, result) in results.iter().enumerate() {
        assert_eq!(result["same_failures"], i + 1);
        assert_eq!(result["tripped"], i == 2);
        assert_eq!(result["fingerprint"], results[0]["fingerprint"]);
    }
    let last = results.last().unwrap().clone();
    assert_eq!(state["reason"]["log"], last["log"]);
    assert!(std::path::Path::new(last["log"].as_str().unwrap()).is_file());
    assert_eq!(same_failure_implementations(w, id), 3);
    let sessions = w.ctx.store.list_sessions(id).unwrap().len();
    w.settle();
    assert_eq!(w.ctx.store.list_sessions(id).unwrap().len(), sessions);
    assert_eq!(same_failure_results(w, id).len(), 3);
    last
}

#[test]
fn same_failure_three_times_stops_without_another_implementation() {
    needs_git!();
    let mut w = World::new("same-failure-stop");
    let id = run_same_failure_task(&mut w, false);
    assert_same_failure_stopped(&mut w, &id);
}

#[test]
fn same_failure_alternating_errors_keep_returning_to_implementation() {
    needs_git!();
    let mut w = World::new("same-failure-alternating");
    let id = run_same_failure_task(&mut w, true);
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(same_failure_implementations(&w, &id), 5);
    let results = same_failure_results(&w, &id);
    assert_eq!(results.len(), 5);
    for (i, result) in results[..4].iter().enumerate() {
        assert_eq!(result["same_failures"], 1);
        assert_eq!(result["tripped"], false);
        assert!(result["fingerprint"].is_string());
        if i > 0 {
            assert_ne!(result["fingerprint"], results[i - 1]["fingerprint"]);
        }
        assert!(
            w.prompt(6 + i as u32)
                .contains(result["tail"].as_str().unwrap())
        );
    }
    assert_eq!(results[4]["ok"], true);
    assert!(results[4].get("fingerprint").is_none());
}

#[test]
fn same_failure_rerun_gate_after_environment_repair_uses_the_same_head() {
    needs_git!();
    let mut w = World::new("same-failure-rerun");
    let id = run_same_failure_task(&mut w, false);
    let failed = assert_same_failure_stopped(&mut w, &id);
    std::fs::write(w.root.join("environment-fixed"), "fixed").unwrap();
    ok(&w.call("task.rerun_from", json!({"task_id": id, "node": "gate"})));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(same_failure_implementations(&w, &id), 3);
    let results = same_failure_results(&w, &id);
    assert_eq!(results.len(), 4);
    assert_eq!(results[3]["sha"], failed["sha"]);
    assert_eq!(results[3]["ok"], true);
    assert!(results[3].get("fingerprint").is_none());
}

// ---------------------------------------------------------------------------
// Rate limits
// ---------------------------------------------------------------------------

/// What Claude prints when the account is out of quota.
const CLAUDE_LIMIT: &str = r#"{"type":"result","subtype":"success","is_error":true,"result":"Claude AI usage limit reached|4102444800"}"#;

#[test]
fn a_rate_limited_round_moves_to_the_other_cli_when_that_keeps_roles_apart_and_waits_when_not() {
    needs_git!();
    let mut w = World::new("ratelimit");
    let request = "limited";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    // 4: the adjudication round (Claude by default) hits the usage limit.
    w.step(4, &format!("echo '{CLAUDE_LIMIT}'\nexit 1\n"));
    // 5: the same round again, on Codex — adjudication shares a CLI with no
    // role it must be kept apart from.
    w.doc_step(5, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));

    let task_id = create_task(&mut w, request);
    w.settle();

    assert_eq!(w.node(&task_id).as_deref(), Some("await_design_approval"));
    assert_ne!(w.state(&task_id), "failed");
    let rerouted = w
        .ctx
        .store
        .last_event(&task_id, "session.rerouted")
        .unwrap()
        .unwrap();
    assert_eq!(rerouted["role"], json!("adjudicate"));
    assert_eq!(rerouted["to"], json!("codex"));
    let until = w
        .ctx
        .store
        .cooling_until(autome_domain::role::Runtime::Claude)
        .unwrap()
        .expect("Claude is cooling down");
    assert!(
        until.starts_with("2100-01-01"),
        "the reset the CLI named: {until}"
    );

    // Implementation is Claude and audit is Codex: moving implementation to
    // Codex would put the round and its auditor on one CLI. So it waits.
    w.call("task.approve", json!({ "task_id": task_id }));
    wait_for_retry(&mut w.ctx, &task_id);
    assert_eq!(w.node(&task_id).as_deref(), Some("implement"));
    assert!(w.ctx.store.running_session(&task_id).unwrap().is_none());
    let deferred = w
        .ctx
        .store
        .last_event(&task_id, "session.deferred")
        .unwrap()
        .unwrap();
    assert_eq!(deferred["runtime"], json!("claude"));
    let (_, not_before, _) = w.ctx.store.pending_retry(&task_id).unwrap().unwrap();
    assert_eq!(not_before, until);
    assert_eq!(w.sessions_run(), vec!["1", "2", "3", "4", "5"]);
}

// ---------------------------------------------------------------------------
// One driver at a time
// ---------------------------------------------------------------------------

#[test]
fn a_tick_skips_while_another_driver_holds_the_ledger() {
    needs_git!();
    let root = std::env::temp_dir().join(format!("automed-e2e-driver-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("automed.sqlite3");
    let mut ctx = Ctx::new(Store::open(&db).unwrap(), root.join("home"), root.join("h"));

    // Another process — the desktop app, say — is mid-tick.
    let held = automed::driver_lock::try_acquire(&db).unwrap().unwrap();
    let report = automed::scheduler::tick(&mut ctx);
    assert!(report.skipped, "{report:?}");
    assert!(report.sessions_reaped.is_empty() && report.tasks_started.is_empty());

    // A write command waits for it rather than interleaving; with the other
    // driver gone it goes through, and so does the next tick.
    drop(held);
    assert!(!automed::scheduler::tick(&mut ctx).skipped);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------------
// Stop takes the whole session down
// ---------------------------------------------------------------------------

/// The wrapper starts in a process group of its own, which is what `stop`
/// signals. Before, it shared the core's group, `kill(-pid)` named a group
/// that did not exist, and the CLI went on running with nobody reading it.
#[test]
fn stopping_a_task_kills_the_cli_the_wrapper_started() {
    needs_git!();
    let mut w = World::new("stopgroup");
    let pid_file = w.repo.join(".autome/fake/cli.pid");
    w.step(
        1,
        &format!("echo $$ > '{}'\nsleep 60\n", pid_file.display()),
    );
    let task_id = create_task(&mut w, "a long round");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !pid_file.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let cli: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(automed::launcher::pid_alive(cli));

    w.call("task.stop", json!({ "task_id": task_id }));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while automed::launcher::pid_alive(cli) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        !automed::launcher::pid_alive(cli),
        "the CLI outlived the stop"
    );
    assert_eq!(w.state(&task_id), "stopped");
}

// Stop/resume tests use observable file barriers, never timing a stop by sleep.
fn resume_wait(w: &mut World, mut ready: impl FnMut(&World) -> bool) {
    scheduling_wait(
        w,
        |w| {
            if ready(w) {
                return true;
            }
            let report = automed::scheduler::tick(&mut w.ctx);
            assert!(report.errors.is_empty(), "{:?}", report.errors);
            ready(w)
        },
        |w| format!("resume barrier; tasks: {:?}", w.task_states()),
    );
}

fn resume_block(w: &World, step: u32) {
    w.step(
        step,
        &format!(
            "touch '{}'\nwhile [ ! -f '{}' ] && [ -d '{}' ]; do sleep 0.02; done\n",
            w.root.join(format!("ready-{step}")).display(),
            w.root.join(format!("release-{step}")).display(),
            w.root.display()
        ),
    );
}

fn resume_setup(w: &mut World, request: &str) -> String {
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    let id = create_task(w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_design_approval"));
    ok(&w.call("task.approve", json!({"task_id":id})));
    id
}

fn assert_resume_role(w: &World, id: &str, node: &str, previous: &str) {
    assert_eq!(w.node(id).as_deref(), Some(node));
    let session = w.ctx.store.running_session(id).unwrap().unwrap();
    assert_ne!(session.id, previous);
    assert_eq!(
        session.kind.role().unwrap().as_str(),
        if node == "implement" { "impl" } else { node }
    );
    assert_eq!(
        w.ctx
            .store
            .list_sessions(id)
            .unwrap()
            .iter()
            .filter(|s| s.kind == autome_domain::session::SessionKind::Intake)
            .count(),
        1
    );
}

#[test]
fn stopped_implementation_resumes_at_implementation() {
    needs_git!();
    let mut w = World::new("resume-impl");
    resume_block(&w, 5);
    resume_block(&w, 6);
    let id = resume_setup(&mut w, "resume impl");
    resume_wait(&mut w, |w| w.root.join("ready-5").exists());
    let old = w.ctx.store.running_session(&id).unwrap().unwrap().id;
    ok(&w.call("task.stop", json!({"task_id":id})));
    assert_eq!(w.state(&id), "stopped");
    ok(&w.call("task.resume", json!({"task_id":id})));
    assert_resume_role(&w, &id, "implement", &old);
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    ok(&w.call("task.stop", json!({"task_id":id})));
}

#[test]
fn stopped_audit_resumes_at_audit() {
    needs_git!();
    let mut w = World::new("resume-audit");
    let request = "resume audit";
    w.doc_step(
        5,
        &slug_for(request),
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
    );
    resume_block(&w, 6);
    resume_block(&w, 7);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    assert_eq!(w.node(&id).as_deref(), Some("audit"));
    let old = w.ctx.store.running_session(&id).unwrap().unwrap().id;
    ok(&w.call("task.stop", json!({"task_id":id})));
    ok(&w.call("task.resume", json!({"task_id":id})));
    assert_resume_role(&w, &id, "audit", &old);
    resume_wait(&mut w, |w| w.root.join("ready-7").exists());
    ok(&w.call("task.stop", json!({"task_id":id})));
}

#[test]
fn stopped_gate_resumes_at_gate_and_discards_killed_result() {
    needs_git!();
    let mut w = World::new("resume-gate");
    let script = w.root.join("gate.sh");
    std::fs::write(
        &script,
        format!(
            "touch '{}'\nwhile [ ! -f '{}' ] && [ -d '{}' ]; do sleep 0.02; done\nexit 0\n",
            w.root.join("gate-ready").display(),
            w.root.join("gate-release").display(),
            w.root.display()
        ),
    )
    .unwrap();
    w.configure(&format!(
        "[gate]\ncommand = \"sh {}\"\ntimeout_minutes = 5\n",
        script.display()
    ));
    let request = "resume gate";
    w.doc_step(
        5,
        &slug_for(request),
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
    );
    resume_block(&w, 6);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("gate-ready").exists());
    assert_eq!(w.node(&id).as_deref(), Some("gate"));
    assert_eq!(w.ctx.store.count_events(&id, "gate.started").unwrap(), 1);
    ok(&w.call("task.stop", json!({"task_id":id})));
    let head =
        automed::git::head_sha(&w.repo.join(format!(".worktree/{}", slug_for(request)))).unwrap();
    let job = automed::gate::Job::at(&w.repo, &id, "gate", &head[..12]);
    assert!(job.started.exists());
    std::fs::write(
        &job.exit,
        "exit_code=143\nended_at=2026-11-10T00:00:00Z\nmarker_end\n",
    )
    .unwrap();
    ok(&w.call("task.resume", json!({"task_id":id})));
    assert_eq!(w.node(&id).as_deref(), Some("gate"));
    assert!(!job.exit.exists());
    std::fs::write(w.root.join("gate-release"), "go").unwrap();
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    assert_eq!(w.node(&id).as_deref(), Some("audit"));
    assert_eq!(w.ctx.store.count_events(&id, "gate.started").unwrap(), 2);
    assert_eq!(
        w.ctx
            .store
            .list_sessions(&id)
            .unwrap()
            .iter()
            .filter(|s| s.kind == autome_domain::session::SessionKind::Intake)
            .count(),
        1
    );
    ok(&w.call("task.stop", json!({"task_id":id})));
}

#[test]
fn stopped_gate_feedback_resumes_at_implementation_with_original_prompt() {
    needs_git!();
    let mut w = World::new("resume-feedback");
    w.configure(
        "[gate]\ncommand = \"echo unique-resume-gate-feedback; exit 1\"\ntimeout_minutes = 5\n",
    );
    let request = "resume feedback";
    w.doc_step(
        5,
        &slug_for(request),
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
    );
    resume_block(&w, 6);
    resume_block(&w, 7);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    let old = w.ctx.store.running_session(&id).unwrap().unwrap().id;
    ok(&w.call("task.stop", json!({"task_id":id})));
    ok(&w.call("task.resume", json!({"task_id":id})));
    assert_resume_role(&w, &id, "implement", &old);
    resume_wait(&mut w, |w| w.root.join("ready-7").exists());
    let prompt = std::fs::read_to_string(w.repo.join(".autome/fake/7.prompt")).unwrap();
    assert!(prompt.contains("unique-resume-gate-feedback"), "{prompt}");
    ok(&w.call("task.stop", json!({"task_id":id})));
}

#[test]
fn stopped_task_resumes_at_original_node_after_waiting_for_slot() {
    needs_git!();
    let mut w = World::new("resume-slot");
    ok(&w.call(
        "config.set_loop",
        json!({"project_id": w.project_id, "parallel": 1}),
    ));
    resume_block(&w, 5);
    resume_block(&w, 6);
    resume_block(&w, 7);
    let id = resume_setup(&mut w, "resume slot");
    resume_wait(&mut w, |w| w.root.join("ready-5").exists());
    let old = w.ctx.store.running_session(&id).unwrap().unwrap().id;
    ok(&w.call("task.stop", json!({"task_id":id})));
    let blocker = create_task(&mut w, "slot blocker");
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    let count = w.ctx.store.list_sessions(&id).unwrap().len();
    ok(&w.call("task.resume", json!({"task_id":id})));
    let state = serde_json::to_value(w.ctx.store.get_task(&id).unwrap().state).unwrap();
    assert_eq!(state, json!({"state":"queued", "resume":"implement"}));
    assert_eq!(
        ok(&w.call("task.get", json!({"task_id":id})))["queue_position"],
        1
    );
    assert_eq!(w.ctx.store.list_sessions(&id).unwrap().len(), count);
    ok(&w.call("task.stop", json!({"task_id":blocker})));
    resume_wait(&mut w, |w| w.root.join("ready-7").exists());
    assert_resume_role(&w, &id, "implement", &old);
    ok(&w.call("task.stop", json!({"task_id":id})));
}

fn pause_prelude(w: &World, step: u32) -> String {
    format!(
        "touch '{}'\nwhile [ ! -f '{}' ] && [ -d '{}' ]; do sleep 0.02; done\n",
        w.root.join(format!("ready-{step}")).display(),
        w.root.join(format!("release-{step}")).display(),
        w.root.display()
    )
}

fn assert_one_role(w: &World, id: &str, role: autome_domain::role::Role) {
    let sessions = w.ctx.store.list_sessions(id).unwrap();
    assert_eq!(
        sessions
            .iter()
            .filter(|s| s.kind.role() == Some(role))
            .count(),
        1
    );
    assert_eq!(
        sessions
            .iter()
            .filter(|s| s.kind == autome_domain::session::SessionKind::Intake)
            .count(),
        1
    );
}

fn paused_implementation_case(mode: &str) {
    let mut w = World::new(mode);
    let request = "paused implementation";
    let slug = slug_for(request);
    w.doc_step_for_prelude(
        5,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
        &pause_prelude(&w, 5),
    );
    resume_block(&w, 6);
    resume_block(&w, 7);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("ready-5").exists());
    let original = w.ctx.store.running_session(&id).unwrap().unwrap().id;
    ok(&w.call("task.pause", json!({"task_id":id})));
    let blocker = if mode == "pause-slot" {
        ok(&w.call(
            "config.set_loop",
            json!({"project_id": w.project_id, "parallel": 1}),
        ));
        let b = create_task(&mut w, "pause blocker");
        resume_wait(&mut w, |w| w.root.join("ready-6").exists());
        Some(b)
    } else {
        None
    };
    if mode == "pause-running" || blocker.is_some() {
        ok(&w.call("task.resume", json!({"task_id":id})));
        assert_eq!(
            w.ctx.store.running_session(&id).unwrap().unwrap().id,
            original
        );
        if blocker.is_some() {
            assert_eq!(w.state(&id), "queued");
            assert_eq!(
                ok(&w.call("task.get", json!({"task_id":id})))["queue_position"],
                1
            );
        }
    }
    std::fs::write(w.root.join("release-5"), "go").unwrap();
    resume_wait(&mut w, |w| {
        w.ctx
            .store
            .list_sessions(&id)
            .unwrap()
            .iter()
            .any(|s| s.id == original && s.ended_at.is_some())
    });
    if mode != "pause-running" {
        assert_eq!(
            w.ctx
                .store
                .count_events(&id, "session.outcome_held")
                .unwrap(),
            1
        );
        assert_eq!(
            w.state(&id),
            if blocker.is_some() {
                "queued"
            } else {
                "paused"
            }
        );
        if mode == "pause-stop" {
            ok(&w.call("task.stop", json!({"task_id":id})));
        }
        if let Some(b) = blocker {
            ok(&w.call("task.stop", json!({"task_id":b})));
        } else {
            ok(&w.call("task.resume", json!({"task_id":id})));
        }
        assert_eq!(
            w.ctx
                .store
                .count_events(&id, "session.outcome_held_applied")
                .unwrap(),
            1
        );
    }
    resume_wait(&mut w, |w| w.node(&id).as_deref() == Some("audit"));
    assert_one_role(&w, &id, autome_domain::role::Role::Impl);
    ok(&w.call("task.stop", json!({"task_id":id})));
}

#[test]
fn paused_implementation_resumes_at_successor_without_rerun() {
    needs_git!();
    paused_implementation_case("pause-held");
}
#[test]
fn paused_running_session_resumes_at_same_session() {
    needs_git!();
    paused_implementation_case("pause-running");
}
#[test]
fn paused_then_stopped_resumes_at_held_outcome() {
    needs_git!();
    paused_implementation_case("pause-stop");
}
#[test]
fn paused_queued_session_resumes_at_original_outcome() {
    needs_git!();
    paused_implementation_case("pause-slot");
}

#[test]
fn paused_audit_resumes_at_successor_without_rerun() {
    needs_git!();
    let mut w = World::new("pause-audit");
    let request = "paused audit";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step_for_prelude(
        6,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "已完成")]),
        &pause_prelude(&w, 6),
    );
    w.retro_step(7, &slug);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    ok(&w.call("task.pause", json!({"task_id":id})));
    std::fs::write(w.root.join("release-6"), "go").unwrap();
    w.settle();
    assert_eq!(w.state(&id), "paused");
    ok(&w.call("task.resume", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_one_role(&w, &id, autome_domain::role::Role::Audit);
}

#[test]
fn paused_gate_resumes_at_gate_without_restarting_job() {
    needs_git!();
    let mut w = World::new("pause-gate");
    let script = w.root.join("gate.sh");
    std::fs::write(&script, pause_prelude(&w, 99)).unwrap();
    w.configure(&format!(
        "[gate]\ncommand = \"sh {}\"\ntimeout_minutes = 5\n",
        script.display()
    ));
    let request = "paused gate";
    w.doc_step(
        5,
        &slug_for(request),
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
    );
    resume_block(&w, 6);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("ready-99").exists());
    ok(&w.call("task.pause", json!({"task_id":id})));
    std::fs::write(w.root.join("release-99"), "go").unwrap();
    let head =
        automed::git::head_sha(&w.repo.join(format!(".worktree/{}", slug_for(request)))).unwrap();
    let job = automed::gate::Job::at(&w.repo, &id, "gate", &head[..12]);
    resume_wait(&mut w, |_| job.exit.exists());
    assert_eq!(w.state(&id), "paused");
    ok(&w.call("task.resume", json!({"task_id":id})));
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    assert_eq!(w.node(&id).as_deref(), Some("audit"));
    assert_eq!(w.ctx.store.count_events(&id, "gate.started").unwrap(), 1);
    assert_one_role(&w, &id, autome_domain::role::Role::Impl);
    ok(&w.call("task.stop", json!({"task_id":id})));
}

#[test]
fn manual_retro_resumes_at_original_node_only_after_finishing() {
    needs_git!();
    let mut w = World::new("pause-manual");
    let request = "paused manual";
    let slug = slug_for(request);
    w.doc_step_for_prelude(
        5,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
        &pause_prelude(&w, 5),
    );
    w.retro_step(6, &slug);
    let script = std::fs::read_to_string(w.repo.join(".autome/fake/6.sh")).unwrap();
    w.step(6, &(pause_prelude(&w, 6) + &script));
    resume_block(&w, 7);
    let id = resume_setup(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("ready-5").exists());
    ok(&w.call("task.pause", json!({"task_id":id})));
    std::fs::write(w.root.join("release-5"), "go").unwrap();
    w.settle();
    ok(&w.call("task.retro", json!({"task_id":id})));
    resume_wait(&mut w, |w| w.root.join("ready-6").exists());
    let response = w.call("task.resume", json!({"task_id":id}));
    assert!(error_message(&response).contains("手动复盘轮还在跑"));
    assert_eq!(w.state(&id), "paused");
    std::fs::write(w.root.join("release-6"), "go").unwrap();
    w.settle();
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "session.outcome_held")
            .unwrap(),
        1
    );
    ok(&w.call("task.resume", json!({"task_id":id})));
    resume_wait(&mut w, |w| w.root.join("ready-7").exists());
    assert_eq!(w.node(&id).as_deref(), Some("audit"));
    assert_one_role(&w, &id, autome_domain::role::Role::Impl);
    ok(&w.call("task.stop", json!({"task_id":id})));
}

/// Uses the ordinary intake/design/implementation/audit/retro wrappers, with
/// task-local scripts so two waiting tasks cannot consume each other's steps.
fn wait_for_automatic_merge(w: &mut World, request: &str) -> String {
    let slug = slug_for(request);
    for (n, body) in [
        (1, doc("设计中", 0, 0, &[])),
        (2, doc("设计中", 1, 0, &[])),
        (3, doc("设计中", 1, 0, &[])),
        (4, doc("实现中", 2, 0, &[("M-01", "开放")])),
        (5, doc("实现中", 2, 1, &[("M-01", "待审")])),
        (6, doc("实现中", 2, 1, &[("M-01", "已完成")])),
    ] {
        w.doc_step_for(&slug, n, &body, "");
    }
    w.retro_step(7, &slug);
    let retro = std::fs::read_to_string(w.repo.join(".autome/fake/7.sh")).unwrap();
    w.step_for(&slug, 7, &retro);
    let id = create_task(w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_design_approval"));
    ok(&w.call("task.approve", json!({"task_id": id})));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    id
}

#[test]
fn two_tasks_waiting_to_merge_both_land_and_the_second_rebases_through_the_gate_by_itself() {
    needs_git!();
    let mut w = World::new("two-auto-merges");
    w.configure("[gate]\ncommand = \"git diff --exit-code\"\ntimeout_minutes = 5\n");
    let a = wait_for_automatic_merge(&mut w, "auto first");
    let b = wait_for_automatic_merge(&mut w, "auto second");
    assert_eq!(w.node(&a).as_deref(), Some("await_merge"));
    assert_eq!(w.node(&b).as_deref(), Some("await_merge"));
    ok(&w.call("task.merge", json!({"task_id": a})));
    w.settle();
    assert_eq!(w.state(&a), "done");
    let a_commit = w.ctx.store.get_task(&a).unwrap().merge_commit.unwrap();
    let before = w
        .ctx
        .store
        .events_since(0, u32::MAX)
        .unwrap()
        .last()
        .unwrap()
        .0;
    ok(&w.call("task.merge", json!({"task_id": b})));
    w.settle();
    assert_eq!(w.state(&b), "done");
    let b_commit = w.ctx.store.get_task(&b).unwrap().merge_commit.unwrap();
    assert!(automed::git::is_ancestor(&w.repo, &a_commit, &b_commit).unwrap());
    let rebased = w
        .ctx
        .store
        .last_event(&b, "merge.rebased")
        .unwrap()
        .unwrap();
    let head = rebased["repos"][0]["head"].as_str().unwrap();
    assert!(automed::git::is_ancestor(&w.repo, head, &b_commit).unwrap());
    let events = w
        .ctx
        .store
        .events_since_with_subject(before, u32::MAX)
        .unwrap();
    let rebase_seq = events
        .iter()
        .find(|(_, kind, subject, _)| subject == &b && kind == "merge.rebased")
        .unwrap()
        .0;
    assert!(
        events.iter().any(|(seq, kind, subject, payload)| {
            *seq > rebase_seq
                && subject == &b
                && kind == "gate.result"
                && payload["node"] == "merge_gate"
                && payload["ok"] == true
                && payload["sha"] == head
        }),
        "the rebased commit must actually pass the merge gate: {events:?}"
    );
    assert!(
        !events.iter().any(|(_, kind, subject, payload)| {
            subject == &b && kind == "task.updated" && payload["state"]["node"] == "await_merge"
        }),
        "the user's single merge press must last through the gate"
    );
}

#[test]
fn main_moving_after_the_press_takes_the_same_automatic_path() {
    needs_git!();
    let mut w = World::new("main-after-press");
    let id = wait_for_automatic_merge(&mut w, "moving main");
    // task.merge calls apply_trigger then ticks before replying. Split those
    // same production calls to move main in their deterministic race window.
    automed::scheduler::apply_trigger(&mut w.ctx, &id, &autome_domain::task::Trigger::Merge)
        .unwrap();
    assert_eq!(w.node(&id).as_deref(), Some("merging"));
    std::fs::write(w.repo.join("main-moved"), "concurrent change\n").unwrap();
    automed::git::commit_paths(&w.repo, &["main-moved"], "main after press").unwrap();
    let main = automed::git::head_sha(&w.repo).unwrap();
    w.settle();
    assert_eq!(w.state(&id), "done");
    assert_eq!(w.ctx.store.count_events(&id, "merge.rebased").unwrap(), 1);
    let merged = w.ctx.store.get_task(&id).unwrap().merge_commit.unwrap();
    assert!(automed::git::is_ancestor(&w.repo, &main, &merged).unwrap());
    assert_eq!(
        std::fs::read_to_string(w.repo.join("main-moved")).unwrap(),
        "concurrent change\n"
    );
}

#[test]
fn a_gate_failing_after_the_automatic_rebase_goes_back_to_implement_and_stops_at_await_merge_again()
{
    needs_git!();
    let mut w = World::new("auto-gate-failure");
    let gate = w.root.join("combination-gate.sh");
    std::fs::write(&gate, "[ ! -f from-a ] || [ ! -f from-b ]\n").unwrap();
    w.configure(&format!(
        "[gate]\ncommand = \"sh {}\"\ntimeout_minutes = 5\n",
        gate.display()
    ));
    let mut ids = Vec::new();
    for (request, file) in [("gate first", "from-a"), ("gate second", "from-b")] {
        let slug = slug_for(request);
        for (n, body) in [
            (1, doc("设计中", 0, 0, &[])),
            (2, doc("设计中", 1, 0, &[])),
            (3, doc("设计中", 1, 0, &[])),
            (4, doc("实现中", 2, 0, &[("M-01", "开放")])),
            (5, doc("实现中", 2, 1, &[("M-01", "待审")])),
            (6, doc("实现中", 2, 1, &[("M-01", "已完成")])),
        ] {
            let prelude = if n == 5 {
                format!(
                    "echo feature > {file}\ngit add {file}\ngit -c user.name=fake -c user.email=f@f commit -qm feature\n"
                )
            } else {
                String::new()
            };
            w.doc_step_for(&slug, n, &body, &prelude);
        }
        w.retro_step(7, &slug);
        let retro = std::fs::read_to_string(w.repo.join(".autome/fake/7.sh")).unwrap();
        w.step_for(&slug, 7, &retro);
        if file == "from-b" {
            w.doc_step_for(&slug, 8, &doc("实现中", 2, 2, &[("M-01", "待审")]),
                "test -f from-a && test -f from-b\ngit rm from-b\ngit -c user.name=fake -c user.email=f@f commit -qm repair\n");
            w.doc_step_for(&slug, 9, &doc("实现中", 2, 2, &[("M-01", "已完成")]), "");
            w.step_for(&slug, 10, &retro);
        }
        let id = create_task(&mut w, request);
        w.settle();
        ok(&w.call("task.approve", json!({"task_id":id})));
        w.settle();
        assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
        ids.push(id);
    }
    let (a, b) = (&ids[0], &ids[1]);
    ok(&w.call("task.merge", json!({"task_id":a})));
    w.settle();
    assert_eq!(w.state(a), "done");
    let main = automed::git::head_sha(&w.repo).unwrap();
    ok(&w.call("task.merge", json!({"task_id":b})));
    w.settle();
    assert_eq!(w.node(b).as_deref(), Some("await_merge"));
    assert_eq!(automed::git::head_sha(&w.repo).unwrap(), main);
    assert!(w.ctx.store.get_task(b).unwrap().merge_commit.is_none());
    assert_eq!(
        w.ctx.store.last_event(b, "merge.auto").unwrap().unwrap()["active"],
        false
    );
    assert_eq!(w.ctx.store.count_events(b, "merge.rebased").unwrap(), 1);
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    let rebase = events
        .iter()
        .find(|(_, k, s, _)| s == b && k == "merge.rebased")
        .unwrap();
    let failure = events
        .iter()
        .find(|(seq, k, s, p)| {
            *seq > rebase.0
                && s == b
                && k == "gate.result"
                && p["node"] == "merge_gate"
                && p["ok"] == false
        })
        .unwrap();
    assert_eq!(failure.3["sha"], rebase.3["repos"][0]["head"]);
    let repair = events
        .iter()
        .find(|(seq, k, s, p)| {
            *seq > failure.0 && s == b && k == "task.updated" && p["state"]["node"] == "implement"
        })
        .unwrap();
    for node in ["gate", "merge_gate"] {
        assert!(events.iter().any(|(seq, k, s, p)| *seq > repair.0
            && s == b
            && k == "gate.result"
            && p["node"] == node
            && p["ok"] == true));
    }
    let wt = w
        .repo
        .join(format!(".worktree/{}", slug_for("gate second")));
    assert!(wt.join("from-a").exists());
    assert!(!wt.join("from-b").exists());
    assert_eq!(
        std::fs::read_to_string(
            w.repo
                .join(".autome/fake")
                .join(slug_for("gate second"))
                .join("history")
        )
        .unwrap(),
        "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    );
}

fn workspace_waiting_to_merge(w: &mut Workspace) -> (String, String) {
    let request = "workspace automatic merge";
    let slug = slug_for(request);
    for (n, body) in [
        (1, ws_doc("设计中", 0, 0, &[], "backend")),
        (2, ws_doc("设计中", 1, 0, &[], "backend")),
        (3, ws_doc("设计中", 1, 0, &[], "backend")),
        (4, ws_doc("实现中", 2, 0, &[("M-01", "开放")], "backend")),
        (5, ws_doc("实现中", 2, 1, &[("M-01", "待审")], "backend")),
        (6, ws_doc("实现中", 2, 1, &[("M-01", "已完成")], "backend")),
        (7, ws_doc("已完成", 2, 1, &[("M-01", "已完成")], "backend")),
    ] {
        let prefix = if n == 1 {
            format!(".worktree/{slug}")
        } else {
            String::new()
        };
        let extra = if n == 5 {
            "printf 'feature\\n' > backend/feature.txt\n(cd backend && git add feature.txt && git -c user.name=fake -c user.email=f@f commit -qm feature)"
        } else {
            ""
        };
        w.doc_step(n, &slug, &prefix, &body, extra);
    }
    let created = call(
        &mut w.ctx,
        "task.create",
        json!({"project_id": w.project_id, "request": request}),
    );
    let id = ok(&created)["task"]["id"].as_str().unwrap().to_string();
    w.settle();
    assert_eq!(
        w.node(&id).as_deref(),
        Some("await_design_approval"),
        "{}",
        w.why(&id)
    );
    ok(&call(&mut w.ctx, "task.approve", json!({"task_id": id})));
    w.settle();
    assert_eq!(
        w.node(&id).as_deref(),
        Some("await_merge"),
        "{}",
        w.why(&id)
    );
    (id, slug)
}

#[test]
fn a_workspace_with_one_member_behind_rebases_every_member_and_merges_them_all() {
    needs_git!();
    let mut w = Workspace::new("one-behind", &["docs", "backend"]);
    let (id, slug) = workspace_waiting_to_merge(&mut w);
    std::fs::write(w.member("backend").join("main-moved"), "new main\n").unwrap();
    automed::git::commit_paths(&w.member("backend"), &["main-moved"], "advance main").unwrap();
    let onto = automed::git::head_sha(&w.member("backend")).unwrap();
    ok(&call(&mut w.ctx, "task.merge", json!({"task_id": id})));
    w.settle();
    assert!(
        matches!(
            w.ctx.store.get_task(&id).unwrap().state,
            autome_domain::task::TaskState::Done
        ),
        "{}",
        w.why(&id)
    );
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    let (seq, _, _, rebased) = events
        .iter()
        .find(|(_, k, s, _)| s == &id && k == "merge.rebased")
        .unwrap();
    let repos = rebased["repos"].as_array().unwrap();
    assert_eq!(repos.len(), 2);
    for member in ["backend", "docs"] {
        let repo = repos.iter().find(|r| r["repo"] == member).unwrap();
        assert!(
            automed::git::is_ancestor(&w.member(member), repo["head"].as_str().unwrap(), "main")
                .unwrap()
        );
    }
    let merges: Vec<_> = events
        .iter()
        .filter(|(_, k, s, _)| s == &id && k == "merge.repo")
        .collect();
    assert_eq!(merges.len(), 2);
    assert!(
        merges
            .iter()
            .all(|(s, _, _, p)| s > seq && p["commit"].is_string())
    );
    assert!(automed::git::is_ancestor(&w.member("backend"), &onto, "main").unwrap());
    assert!(w.member("backend").join("feature.txt").exists());
    assert!(
        w.member("docs")
            .join(format!("autome/{slug}/{slug}.md"))
            .exists()
    );
    assert!(!events.iter().any(|(s, k, subject, p)| s > seq
        && subject == &id
        && k == "task.updated"
        && p["state"]["node"] == "await_merge"));
}

#[test]
fn a_workspace_merge_left_half_done_finishes_without_a_second_merge_commit() {
    needs_git!();
    let mut w = Workspace::new("half-merged", &["docs", "backend"]);
    let (id, slug) = workspace_waiting_to_merge(&mut w);
    let path = format!("autome/{slug}/{slug}.md");
    let dirty = w.member("docs").join(&path);
    std::fs::create_dir_all(dirty.parent().unwrap()).unwrap();
    std::fs::write(&dirty, "user document\n").unwrap();
    ok(&call(&mut w.ctx, "task.merge", json!({"task_id": id})));
    w.settle();
    assert_eq!(
        w.node(&id).as_deref(),
        Some("await_merge"),
        "{}",
        w.why(&id)
    );
    assert!(w.member("backend").join("feature.txt").exists());
    assert_eq!(std::fs::read_to_string(&dirty).unwrap(), "user document\n");
    assert_eq!(w.ctx.store.count_events(&id, "merge.rebased").unwrap(), 0);
    let backend_merged = automed::git::head_sha(&w.member("backend")).unwrap();
    std::fs::remove_file(&dirty).unwrap();
    std::fs::write(w.member("docs").join("main-moved"), "new docs main\n").unwrap();
    automed::git::commit_paths(&w.member("docs"), &["main-moved"], "advance docs main").unwrap();
    ok(&call(&mut w.ctx, "task.merge", json!({"task_id": id})));
    w.settle();
    assert!(
        matches!(
            w.ctx.store.get_task(&id).unwrap().state,
            autome_domain::task::TaskState::Done
        ),
        "{}",
        w.why(&id)
    );
    assert_eq!(w.ctx.store.count_events(&id, "merge.rebased").unwrap(), 1);
    assert_eq!(
        automed::git::head_sha(&w.member("backend")).unwrap(),
        backend_merged
    );
    let task = w.ctx.store.get_task(&id).unwrap();
    let message = format!("merge(autome): {} {}", id, task.title);
    for member in ["backend", "docs"] {
        let output = std::process::Command::new("git")
            .current_dir(w.member(member))
            .args(["log", "main", "--merges", "--format=%s"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .filter(|line| *line == message)
                .count(),
            1,
            "{member}"
        );
    }
    assert!(
        std::fs::read_to_string(&dirty)
            .unwrap()
            .contains("status: 已完成")
    );
    assert_eq!(
        std::fs::read_to_string(w.member("docs").join("main-moved")).unwrap(),
        "new docs main\n"
    );
}

#[test]
fn the_merge_panel_offers_the_merge_when_only_main_has_moved() {
    needs_git!();
    let mut w = World::new("panel-main-moved");
    let request = "panel automatic rebase";
    let id = wait_for_automatic_merge(&mut w, request);
    let changes = w.call("task.changes", json!({"task_id":id}));
    assert_eq!(ok(&changes)["rebase_first"], false);
    assert_eq!(ok(&changes)["repos"][0]["behind"], false);
    std::fs::write(w.repo.join("main-moved"), "concurrent change\n").unwrap();
    automed::git::commit_paths(&w.repo, &["main-moved"], "main moved").unwrap();
    let changes = w.call("task.changes", json!({"task_id":id}));
    assert_eq!(ok(&changes)["mergeable"], true);
    assert_eq!(ok(&changes)["blocked_by"], Value::Null);
    assert_eq!(ok(&changes)["rebase_first"], true);
    assert_eq!(ok(&changes)["repos"][0]["behind"], true);
    let slug = slug_for(request);
    let path = format!("docs/{slug}/{slug}.md");
    std::fs::create_dir_all(w.repo.join("docs").join(&slug)).unwrap();
    std::fs::write(w.repo.join(&path), "user's dirty document\n").unwrap();
    let changes = w.call("task.changes", json!({"task_id":id}));
    assert_eq!(ok(&changes)["mergeable"], false);
    assert_eq!(ok(&changes)["blocked_by"]["kind"], "dirty_worktree");
    assert_eq!(ok(&changes)["rebase_first"], true);
    std::fs::remove_file(w.repo.join(&path)).unwrap();
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    let changes = w.call("task.changes", json!({"task_id":id}));
    assert_eq!(ok(&changes)["available"], false);
}

#[test]
fn task_card_a_drafted_bug_waits_then_starts_with_its_card_committed_before_the_first_session() {
    needs_git!();
    let mut w = World::new("card-bug");
    let request = "card bug";
    let slug = slug_for(request);
    let brief = "# 卡片\n修复 {request}，保留 {unknown}\r\n";
    let card = format!("docs/{slug}/{slug}-card.md");
    let prelude = format!("git cat-file -e HEAD:{card} || exit 80");
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核").replace("已定位", "无法复现"),
        &prelude,
    );
    let out = w.call("task.create", json!({"project_id":w.project_id,"request":request,"kind":"bug","draft":true,"brief":brief}));
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    w.settle();
    assert!(w.sessions_run().is_empty());
    let dashboard = w.call("dashboard.get", json!({}));
    assert!(ok(&dashboard)["waiting"].as_array().unwrap().is_empty());
    assert!(ok(&dashboard)["running"].as_array().unwrap().is_empty());
    assert!(!w.repo.join(format!(".worktree/{slug}")).exists());
    let started = w.call("task.start", json!({"task_id":id}));
    assert_eq!(ok(&started)["started"], true);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_info"));
    let wt = w.repo.join(format!(".worktree/{slug}"));
    let committed = automed::git::run_ok(&wt, &["show", &format!("HEAD:{card}")]).unwrap();
    assert_eq!(committed.stdout.as_bytes(), brief.as_bytes());
    let history = automed::git::run_ok(&wt, &["log", "--format=%s", "--", &card]).unwrap();
    assert_eq!(history.stdout.trim(), format!("chore(autome): {id} inputs"));
    let history = automed::git::run_ok(&wt, &["log", "--reverse", "--format=%s"])
        .unwrap()
        .stdout;
    assert!(
        history.find(&format!("{id} inputs")).unwrap()
            < history.find(&format!("{id} 固定协议")).unwrap()
    );
    let prompt = w.prompt(1);
    assert!(prompt.find(&card).unwrap() < prompt.find(request).unwrap());
    let task = w.call("task.get", json!({"task_id":id}));
    assert_eq!(ok(&task)["task"]["kind"], "bug");
    let project = w.call("project.get", json!({"project_id":w.project_id}));
    assert_eq!(ok(&project)["tasks"][0]["kind"], "bug");
    let dashboard = w.call("dashboard.get", json!({}));
    assert_eq!(ok(&dashboard)["waiting"][0]["task"]["kind"], "bug");
}

#[test]
fn task_card_a_draft_is_cancelled_without_ever_having_a_worktree() {
    needs_git!();
    let mut w = World::new("card-cancel");
    let out = w.call(
        "task.create",
        json!({"project_id":w.project_id,"request":"cancel card","brief":"# 卡片","draft":true}),
    );
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    let task = w.ctx.store.get_task(&id).unwrap();
    let cancelled = w.call("task.cancel", json!({"task_id":id}));
    ok(&cancelled);
    w.settle();
    assert_eq!(w.state(&id), "cancelled");
    assert!(w.ctx.store.get_task(&id).unwrap().archived_at.is_some());
    assert!(!w.repo.join(format!(".worktree/{}", task.slug)).exists());
    assert!(
        !automed::git::run_ok(&w.repo, &["branch", "--list", &task.branch()])
            .unwrap()
            .stdout
            .contains(&task.branch())
    );
    assert!(w.sessions_run().is_empty());
}

#[test]
fn task_card_a_started_draft_waits_behind_the_parallel_limit() {
    needs_git!();
    let mut w = World::new("card-queue");
    w.configure("[retry]\nattempts = 0\n");
    let configured = w.call(
        "config.set_loop",
        json!({"project_id":w.project_id,"parallel":1}),
    );
    ok(&configured);
    let out = w.call(
        "task.create",
        json!({"project_id":w.project_id,"request":"older draft","draft":true}),
    );
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    let draft = w.ctx.store.get_task(&id).unwrap();
    // Insert queued fixtures with exactly the draft's timestamp, taking real queue sequence numbers.
    for (id, slug) in [
        ("queued-first", "queued-first"),
        ("queued-second", "queued-second"),
    ] {
        let mut task = draft.clone();
        task.id = id.into();
        task.slug = slug.into();
        task.state = autome_domain::task::TaskState::Queued;
        w.ctx.store.create_task(&task, None).unwrap();
    }
    let release = w.root.join("release");
    w.step_for(
        "queued-first",
        1,
        &format!(
            "while [ ! -f '{}' ]; do sleep 0.05; done\nexit 1",
            release.display()
        ),
    );
    w.step_for("queued-second", 1, "echo queued >> ../order\nexit 1");
    w.step_for(&draft.slug, 1, "echo draft >> ../order\nexit 1");
    automed::scheduler::tick(&mut w.ctx);
    assert_eq!(w.ctx.store.all_running_sessions().unwrap().len(), 1);
    let started = w.call("task.start", json!({"task_id":id}));
    assert_eq!(ok(&started)["started"], false);
    assert_eq!(ok(&started)["queue_position"], 2);
    assert_eq!(
        w.ctx.store.get_task("queued-second").unwrap().created_at,
        draft.created_at
    );
    std::fs::write(release, "go").unwrap();
    w.settle();
    assert_eq!(
        std::fs::read_to_string(w.repo.join(".worktree/order")).unwrap(),
        "queued\ndraft\n"
    );
    for id in ["queued-first", "queued-second", &id] {
        assert!(!w.ctx.store.list_sessions(id).unwrap().is_empty());
    }
}

// Derive the fake document from the same frozen contract models receive.
fn bug_doc(review: &str) -> String {
    let contract = include_str!("../../../protocol/bug-protocol.md");
    let region = contract
        .split("<!-- kernel-contract: bug-status-block -->")
        .nth(1)
        .unwrap();
    let example = region
        .split("```text\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    example
        .lines()
        .filter(|l| !l.starts_with("repos:"))
        .map(|line| {
            if line.starts_with("复现命令:") {
                "复现命令: sh regression.sh".to_owned()
            } else if line.starts_with("复核:") {
                format!("复核: {review}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n\n## 现象\n产品标记错误。\n\n## 复现\nsh regression.sh\n\n## 根因分析\nproduct.txt 应为 fixed。\n\n## 修复说明\n修复只改变产品标记。\n\n## 复核意见\n检查回归测试与产品差异。\n"
}

fn bug_step(w: &World, n: u32, slug: &str, body: &str, prelude: &str) {
    w.step(n, &format!("set -e\n{prelude}\nmkdir -p docs/{slug}\ncat > docs/{slug}/{slug}-bug.md <<'BUG_EOF'\n{body}\nBUG_EOF\ngit add -A\ngit -c user.name=fake -c user.email=f@f commit -qm 'bug round {n}' || true\n"));
}

const BUG_LOCATE: &str = "printf 'broken\\n' > product.txt\nprintf 'echo regression-output; test \"$(cat product.txt)\" = fixed\\n' > regression.sh";
const BUG_FIX: &str = "printf 'fixed\\n' > product.txt";

fn create_bug(w: &mut World, request: &str) -> String {
    let out = w.call("task.create", json!({"project_id": w.project_id, "request": request, "kind": "bug", "brief": "# Bug card\nExpected fixed"}));
    ok(&out)["task"]["id"].as_str().unwrap().to_owned()
}

#[test]
fn bug_review_policy_disabled_audit_must_not_silently_skip_review() {
    assert_disabled_bug_audit_is_explicit(false);
}

#[test]
fn bug_review_policy_disabling_audit_before_start_must_not_silently_skip_review() {
    assert_disabled_bug_audit_is_explicit(true);
}

fn assert_disabled_bug_audit_is_explicit(draft: bool) {
    needs_git!();
    let mut w = World::new("bug-disabled-audit");
    let request = "disabled audit bug";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    if !draft {
        w.configure("[roles.audit]\nenabled = false\n");
    }
    let created = w.call(
        "task.create",
        json!({
            "project_id": w.project_id, "request": request,
            "kind": "bug", "draft": draft, "brief": "# Bug card\nExpected fixed"
        }),
    );
    // Both remedies in the task card are allowed: an actionable refusal,
    // or an explicit review-skipped event when the loop proceeds.
    if matches!(created.reply.outcome, ReplyOutcome::Error { .. }) {
        let message = error_message(&created);
        assert!(
            message.contains("审计") && message.contains("启用"),
            "{message}"
        );
        return;
    }
    let id = ok(&created)["task"]["id"].as_str().unwrap().to_owned();
    if draft {
        w.configure("[roles.audit]\nenabled = false\n");
        let started = w.call("task.start", json!({"task_id": id}));
        if matches!(started.reply.outcome, ReplyOutcome::Error { .. }) {
            let message = error_message(&started);
            assert!(
                message.contains("审计") && message.contains("启用"),
                "{message}"
            );
            assert!(w.ctx.store.list_sessions(&id).unwrap().is_empty());
            return;
        }
    }
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let sessions = w.ctx.store.list_sessions(&id).unwrap();
    let steps: Vec<_> = sessions.iter().map(|s| s.kind).collect();
    assert_eq!(steps.len(), 2);
    for step in [
        autome_domain::session::BugStep::Locate,
        autome_domain::session::BugStep::Fix,
    ] {
        assert!(steps.contains(&autome_domain::session::SessionKind::Bug { step }));
    }
    assert!(
        w.ctx.store.count_events(&id, "bug.review_skipped").unwrap() > 0,
        "Bug reached await_merge without fix_review or bug.review_skipped (draft={draft}); sessions={steps:?}"
    );
}

#[test]
fn bug_review_policy_same_model_is_refused_for_bug_start() {
    needs_git!();
    let mut w = World::new("bug-same-model");
    w.configure("[roles.impl]\nruntime = \"claude\"\nmodel = \"shared-model\"\n[roles.audit]\nruntime = \"claude\"\nmodel = \"shared-model\"\n");
    let out = w.call(
        "task.create",
        json!({
            "project_id": w.project_id, "request": "same model bug",
            "kind": "bug", "draft": true, "brief": "# Bug card\nExpected independent review"
        }),
    );
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    let started = w.call("task.start", json!({"task_id": id}));
    let message = error_message(&started);
    assert!(message.contains("配置无效"), "{message}");
    assert_eq!(w.state(&id), "draft");
    assert!(w.ctx.store.all_running_sessions().unwrap().is_empty());
}

#[test]
fn gate_quick_bug_fix_gate_runs_repro_and_quick_and_merge_gate_runs_repro_and_full() {
    quick_bug_scenario(false);
}

#[test]
fn gate_quick_bug_failing_quick_returns_to_fix_with_its_tail() {
    quick_bug_scenario(true);
}

fn quick_bug_scenario(fail_first: bool) {
    needs_git!();
    let mut w = World::new("quick-bug");
    let quick_count = w.root.join("quick-count");
    let full_count = w.root.join("full-count");
    let quick_script = w.root.join("quick.sh");
    let full_script = w.root.join("full.sh");
    std::fs::write(
        &quick_script,
        format!(
            "echo \"$AUTOME_GATE_NODE $AUTOME_GATE_KIND\" >> '{}'\necho quick-bug-marker\n{}\n",
            quick_count.display(),
            if fail_first {
                "test -f quick-ready"
            } else {
                "true"
            }
        ),
    )
    .unwrap();
    std::fs::write(
        &full_script,
        format!(
            "echo \"$AUTOME_GATE_NODE $AUTOME_GATE_KIND\" >> '{}'\n",
            full_count.display()
        ),
    )
    .unwrap();
    let quick = format!("sh {}", quick_script.display());
    let full = format!("sh {}", full_script.display());
    w.configure(&format!("[gate]\ncommand = '{full}'\nquick = '{quick}'\nrun = 'before_merge'\ntimeout_minutes = 1\nreuse_for_main_check = false\n"));
    let request = "quick bug";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    let review_round = if fail_first {
        bug_step(&w, 3, &slug, &bug_doc("待复核"), "touch quick-ready");
        4
    } else {
        3
    };
    bug_step(&w, review_round, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let expected_quick = if fail_first { 2 } else { 1 };
    assert_eq!(
        std::fs::read_to_string(&quick_count).unwrap(),
        "fix_gate quick\n".repeat(expected_quick)
    );
    assert_eq!(
        std::fs::read_to_string(&full_count).unwrap(),
        "merge_gate full\n"
    );
    if fail_first {
        assert!(w.prompt(3).contains("quick-bug-marker"));
        assert!(w.prompt(3).contains(&quick));
    }
    let events = w.ctx.store.events_since_with_subject(0, 1000).unwrap();
    let starts: Vec<_> = events
        .iter()
        .filter(|e| e.1 == "gate.started" && e.2 == id)
        .collect();
    assert_eq!(starts.len(), expected_quick + 2);
    for e in starts {
        let command = e.3["command"].as_str().unwrap();
        match e.3["node"].as_str().unwrap() {
            "fix_gate" => {
                assert_eq!(command, format!("( sh regression.sh ) && ( {quick} )"));
                assert_eq!(e.3["kind"], "quick");
                assert!(!command.contains(&full));
            }
            "merge_gate" => {
                assert_eq!(command, format!("( sh regression.sh ) && ( {full} )"));
                assert_eq!(e.3["kind"], "full");
            }
            "repro_check" => assert_eq!(e.3["kind"], "repro"),
            node => panic!("unexpected gate {node}"),
        }
    }
}

#[test]
fn bug_loop_before_merge_fix_gate_runs_only_the_repro_and_merge_gate_runs_both() {
    needs_git!();
    let mut w = World::new("bug-before-merge");
    let gate = "echo project-gate-output; test -f gate-ready";
    w.configure(&format!(
        "[gate]\ncommand = '{gate}'\nrun = 'before_merge'\ntimeout_minutes = 1\n"
    ));
    let slug = slug_for("before merge bug");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    bug_step(&w, 4, &slug, &bug_doc("待复核"), "touch gate-ready");
    bug_step(&w, 5, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "before merge bug");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert!(w.prompt(4).contains("project-gate-output"));
    let events = w.ctx.store.events_since_with_subject(0, 1000).unwrap();
    let nodes: Vec<_> = events
        .iter()
        .filter(|e| e.1 == "task.updated" && e.2 == id)
        .filter_map(|e| e.3["state"]["node"].as_str())
        .collect();
    assert_eq!(
        nodes,
        [
            "locate",
            "repro_check",
            "fix",
            "fix_gate",
            "fix_review",
            "rebase",
            "merge_gate",
            "fix",
            "fix_gate",
            "fix_review",
            "rebase",
            "merge_gate",
            "await_merge"
        ]
    );
    let starts: Vec<_> = events
        .iter()
        .filter(|e| e.1 == "gate.started" && e.2 == id)
        .collect();
    for node in ["fix_gate", "merge_gate"] {
        let commands: Vec<_> = starts
            .iter()
            .filter(|e| e.3["node"] == node)
            .map(|e| e.3["command"].as_str().unwrap())
            .collect();
        assert_eq!(commands.len(), 2);
        let expected = if node == "fix_gate" {
            "sh regression.sh".to_owned()
        } else {
            format!("( sh regression.sh ) && ( {gate} )")
        };
        assert!(commands.iter().all(|c| *c == expected));
    }
    let merge_results: Vec<_> = events
        .iter()
        .filter(|e| e.1 == "gate.result" && e.2 == id && e.3["node"] == "merge_gate")
        .map(|e| e.3["ok"].as_bool().unwrap())
        .collect();
    assert_eq!(merge_results, [false, true]);
    assert_eq!(w.ctx.store.count_events(&id, "gate.reused").unwrap(), 0);
}

#[test]
fn bug_loop_before_merge_an_ineffective_fix_fails_the_fix_gate() {
    needs_git!();
    let mut w = World::new("bug-before-merge-ineffective");
    w.configure("[gate]\ncommand = 'true'\nrun = 'before_merge'\n");
    let slug = slug_for("ineffective fix");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), "");
    bug_step(&w, 3, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 4, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "ineffective fix");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert!(w.prompt(3).contains("regression-output"));
    assert!(w.prompt(3).contains("2/3"));
    let events = w.ctx.store.events_since_with_subject(0, 1000).unwrap();
    let results: Vec<_> = events
        .iter()
        .filter(|e| e.1 == "gate.result" && e.2 == id && e.3["node"] == "fix_gate")
        .map(|e| {
            (
                e.3["command"].as_str().unwrap(),
                e.3["ok"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        results,
        [("sh regression.sh", false), ("sh regression.sh", true)]
    );
}

#[test]
fn bug_loop_a_bug_runs_from_creation_to_a_merge_commit() {
    needs_git!();
    let mut w = World::new("bug-merge");
    let slug = slug_for("bug merge");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "bug merge");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let sessions = w.ctx.store.list_sessions(&id).unwrap();
    assert_eq!(sessions.len(), 3);
    for (n, phrase) in [(1, "定位轮"), (2, "修复轮"), (3, "复核轮")] {
        assert!(w.prompt(n).contains(phrase), "{}", w.prompt(n));
        for absent in ["用户提供的附件", "用户指定的仓库内参考文档", "{inputs}"]
        {
            assert!(!w.prompt(n).contains(absent), "{}", w.prompt(n));
        }
    }
    assert!(
        sessions
            .iter()
            .all(|s| matches!(s.kind, autome_domain::session::SessionKind::Bug { .. }))
    );
    assert!(w.prompt(1).contains(&format!("{slug}-bug.md")));
    assert!(w.prompt(1).contains("bug-protocol.md"));
    let task = w.ctx.store.get_task(&id).unwrap();
    let wt = w.repo.join(format!(".worktree/{slug}"));
    assert!(!wt.join(task.design_doc()).exists());
    assert!(!wt.join(task.doc_dir()).join("brief").exists());
    let events = w.ctx.store.events_since_with_subject(0, 1000).unwrap();
    let nodes: Vec<_> = events
        .iter()
        .filter(|e| e.1 == "task.updated" && e.2 == id)
        .filter_map(|e| e.3["state"]["node"].as_str())
        .collect();
    assert_eq!(
        nodes,
        [
            "locate",
            "repro_check",
            "fix",
            "fix_gate",
            "fix_review",
            "rebase",
            "merge_gate",
            "await_merge"
        ]
    );
    let logs = std::fs::read_dir(w.repo.join(format!(".autome/output/gates/{id}")))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(logs.iter().any(|p| p.starts_with("repro_check-")));
    assert!(logs.iter().any(|p| p.starts_with("fix_gate-")));
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    assert_eq!(
        std::fs::read_to_string(w.repo.join("product.txt")).unwrap(),
        "fixed\n"
    );
    let merged = w.ctx.store.get_task(&id).unwrap().merge_commit.unwrap();
    assert_eq!(
        automed::git::run_ok(&w.repo, &["show", &format!("{merged}:product.txt")])
            .unwrap()
            .stdout,
        "fixed\n"
    );
}

#[test]
fn bug_loop_prompts_name_the_card_attachments_and_doc_refs() {
    needs_git!();
    let mut w = World::new("bug-inputs");
    let slug = slug_for("bug inputs");
    std::fs::write(w.root.join("crash.log"), "original crash evidence\n").unwrap();
    let attachment = std::fs::canonicalize(w.root.join("crash.log")).unwrap();
    std::fs::create_dir_all(w.repo.join("docs")).unwrap();
    std::fs::write(w.repo.join("docs/notes.md"), "# Expected behavior\n").unwrap();
    automed::git::commit_paths(&w.repo, &["docs/notes.md"], "add reference").unwrap();
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let out = w.call(
        "task.create",
        json!({
            "project_id": w.project_id,
            "request": "bug inputs",
            "kind": "bug",
            "brief": "# Bug card\nExpected fixed",
            "attachments": [attachment.to_str().unwrap()],
            "doc_refs": ["docs/notes.md"]
        }),
    );
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(w.ctx.store.list_sessions(&id).unwrap().len(), 3);
    let copied = format!("docs/{slug}/attachments/crash.log");
    for n in 1..=3 {
        let prompt = w.prompt(n);
        assert!(prompt.contains(&format!("- {copied}\n")), "{prompt}");
        assert!(prompt.contains("- docs/notes.md\n"), "{prompt}");
        assert!(
            !prompt.contains(attachment.parent().unwrap().to_str().unwrap()),
            "{prompt}"
        );
    }
    let wt = w.repo.join(format!(".worktree/{slug}"));
    assert_eq!(
        std::fs::read_to_string(wt.join(copied)).unwrap(),
        "original crash evidence\n"
    );
    assert!(wt.join("docs/notes.md").is_file());
}

#[test]
fn bug_loop_a_repro_command_that_passes_on_the_locate_commit_returns_to_locate() {
    needs_git!();
    for command in [
        "sh regression.sh",
        "missing-autome-repro-command",
        "exit 126",
    ] {
        let mut w = World::new("bug-repro-miss");
        let slug = slug_for("repro miss");
        bug_step(
            &w,
            1,
            &slug,
            &bug_doc("待复核").replace("sh regression.sh", command),
            &format!("{BUG_LOCATE}\n{BUG_FIX}"),
        );
        bug_step(
            &w,
            2,
            &slug,
            &bug_doc("待复核").replace("已定位", "无法复现"),
            "",
        );
        let id = create_bug(&mut w, "repro miss");
        w.settle();
        assert_eq!(w.node(&id).as_deref(), Some("await_info"));
        let prompt = w.prompt(2);
        assert!(
            prompt.contains("复现检查没有失败") && prompt.contains(command),
            "{prompt}"
        );
        assert!(prompt.contains(if command == "sh regression.sh" {
            "regression-output"
        } else {
            "not_runnable"
        }));
        assert_eq!(w.ctx.store.count_events(&id, "bug.reproduced").unwrap(), 0);
        assert_eq!(
            w.ctx.store.count_events(&id, "bug.repro_missed").unwrap(),
            1
        );
    }
}

#[test]
fn bug_loop_a_fix_whose_repro_command_still_fails_returns_to_fix() {
    needs_git!();
    let mut w = World::new("bug-fix-fail");
    let slug = slug_for("fix fail");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    // Changing the document cannot replace the frozen failing command.
    bug_step(
        &w,
        2,
        &slug,
        &bug_doc("待复核").replace("sh regression.sh", "true"),
        "",
    );
    bug_step(&w, 3, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 4, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "fix fail");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert!(w.prompt(3).contains("regression-output"));
    assert!(w.prompt(3).contains("2/3"));
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.fix_round_started")
            .unwrap(),
        2
    );
}

#[test]
fn bug_loop_the_project_gate_runs_after_the_repro_command_in_the_fix_gate() {
    needs_git!();
    let mut w = World::new("bug-project-gate");
    w.configure(
        "[gate]\ncommand = \"echo project-gate-output; test -f gate-ready\"\ntimeout_minutes = 1\n",
    );
    let slug = slug_for("project gate");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("待复核"), "touch gate-ready");
    bug_step(&w, 4, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "project gate");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let prompt = w.prompt(3);
    assert!(
        prompt.find("regression-output").unwrap() < prompt.find("project-gate-output\n").unwrap()
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.fix_round_started")
            .unwrap(),
        2
    );
}

#[test]
fn bug_loop_review_rejection_returns_to_fix_and_counts_a_round() {
    needs_git!();
    let mut w = World::new("bug-reject");
    let slug = slug_for("review reject");
    for (n, review, prelude) in [
        (1, "待复核", BUG_LOCATE),
        (2, "待复核", BUG_FIX),
        (3, "打回", ""),
        (4, "待复核", ""),
        (5, "通过", ""),
    ] {
        bug_step(&w, n, &slug, &bug_doc(review), prelude);
    }
    let id = create_bug(&mut w, "review reject");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(
        ok(&w.call("task.get", json!({"task_id":id})))["task"]["budget_n"],
        3
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.fix_round_started")
            .unwrap(),
        2
    );
    assert!(w.prompt(4).contains("复核意见"));
    assert!(w.prompt(5).contains("2/3"));
}

#[test]
fn bug_loop_a_crashed_fix_session_is_retried_without_counting_a_round() {
    needs_git!();
    let mut w = World::new("bug-crash");
    w.configure("[retry]\nattempts = 1\nbackoff_seconds = 0\n");
    let slug = slug_for("fix crash");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    w.step(2, "exit 17\n");
    bug_step(&w, 3, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 4, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "fix crash");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.fix_round_started")
            .unwrap(),
        1
    );
    assert!(w.prompt(3).contains("1/3"));
    assert_eq!(w.ctx.store.count_events(&id, "session.retry").unwrap(), 1);
}

#[test]
fn bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it() {
    needs_git!();
    let mut w = World::new("bug-legacy");
    let slug = slug_for("legacy bug");
    let out = w.call(
        "task.create",
        json!({"project_id": w.project_id, "request": "legacy bug", "kind": "bug", "draft": true}),
    );
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    w.ctx
        .store
        .set_task_state(
            &id,
            &autome_domain::task::TaskState::Active {
                node: autome_domain::task::Node::AwaitDesignApproval,
            },
        )
        .unwrap();
    // task.approve ticks after launching the implementation. Hold the CLI
    // until we have observed that session, even when it runs very quickly.
    w.doc_step_for_prelude(
        1,
        &slug,
        &doc("实现中", 1, 1, &[("M-01", "待审")]),
        &format!(
            "while [ ! -f '{}' ] && [ -d '{}' ]; do sleep 0.02; done\n",
            w.root.join("release-impl").display(),
            w.root.display()
        ),
    );
    w.doc_step(2, &slug, &doc("实现中", 1, 1, &[("M-01", "已完成")]));
    w.retro_step(3, &slug);
    ok(&w.call("task.approve", json!({"task_id":id})));
    assert_eq!(w.node(&id).as_deref(), Some("implement"));
    assert_eq!(
        w.ctx.store.running_session(&id).unwrap().unwrap().kind,
        autome_domain::session::SessionKind::Role {
            role: autome_domain::role::Role::Impl
        }
    );
    std::fs::write(w.root.join("release-impl"), "go").unwrap();
    resume_wait(&mut w, |w| !w.has_pending_work());
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(w.ctx.store.list_sessions(&id).unwrap().len(), 3);
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    assert_eq!(
        w.ctx.store.count_events(&id, "bug.loop_started").unwrap(),
        0
    );
}

#[test]
fn bug_loop_a_malformed_bug_doc_is_a_protocol_failure() {
    needs_git!();
    for scenario in [
        "missing-command",
        "stale-review",
        "missing-file",
        "changed-status",
        "pending-review",
    ] {
        let mut w = World::new("bug-protocol");
        let slug = slug_for("bad bug doc");
        if scenario == "missing-file" {
            w.step(1, "exit 0\n");
        } else {
            let body = if scenario == "missing-command" {
                bug_doc("待复核").replace("复现命令: sh regression.sh", "")
            } else {
                bug_doc("待复核")
            };
            bug_step(&w, 1, &slug, &body, BUG_LOCATE);
            let fix = match scenario {
                "changed-status" => bug_doc("待复核").replace("已定位", "无法复现"),
                "stale-review" => bug_doc("通过"),
                _ => bug_doc("待复核"),
            };
            bug_step(&w, 2, &slug, &fix, BUG_FIX);
            bug_step(&w, 3, &slug, &bug_doc("待复核"), "");
        }
        let id = create_bug(&mut w, "bad bug doc");
        w.settle();
        let state = w.ctx.store.get_task(&id).unwrap().state;
        let autome_domain::task::TaskState::Failed {
            reason: autome_domain::task::FailureReason::Protocol { detail },
            ..
        } = state
        else {
            panic!("{scenario}: {state:?}");
        };
        assert!(
            detail.contains(match scenario {
                "missing-command" => "复现命令",
                "missing-file" => "缺少文件",
                "changed-status" => "status",
                _ => "复核",
            }),
            "{detail}"
        );
    }
}

#[test]
fn bug_loop_cannot_reproduce_waits_for_info_and_a_reply_returns_to_locate_with_it() {
    needs_git!();
    let mut w = World::new("bug-info");
    let slug = slug_for("need info");
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核").replace("已定位", "无法复现"),
        "",
    );
    // Repro misses return to Locate too: every later Locate still sees the file.
    bug_step(
        &w,
        2,
        &slug,
        &bug_doc("待复核"),
        &format!("{BUG_LOCATE}\n{BUG_FIX}"),
    );
    bug_step(
        &w,
        3,
        &slug,
        &bug_doc("待复核").replace("已定位", "无法复现"),
        "",
    );
    let id = create_bug(&mut w, "need info");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_info"));
    let dashboard = w.call("dashboard.get", json!({}));
    assert!(ok(&dashboard)["waiting"].to_string().contains(&id));
    for text in ["", " \n\t"] {
        let out = w.call("task.reply", json!({"task_id":id,"info":text}));
        assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
    }
    assert_eq!(w.ctx.store.count_events(&id, "task.replied").unwrap(), 0);
    for method in ["task.pause", "task.stop", "task.extend_budget"] {
        assert!(matches!(
            w.call(method, json!({"task_id":id,"extra_rounds":1}))
                .reply
                .outcome,
            ReplyOutcome::Error { .. }
        ));
    }
    let text = "  请保留  双空格\n中文 {budget_line} `不是指令`\n";
    let out = w.call("task.reply", json!({"task_id":id,"info":text}));
    ok(&out);
    let relative = format!("docs/{slug}/{slug}-replies.md");
    let wt = w.repo.join(".worktree").join(&slug);
    let replies = w.ctx.store.task_replies(&id).unwrap();
    let expected = format!("## 第 1 条（{}）\n\n{text}\n\n", replies[0].0);
    assert_eq!(
        std::fs::read(wt.join(&relative)).unwrap(),
        expected.as_bytes()
    );
    let committed = std::process::Command::new("git")
        .args(["show", &format!("HEAD:{relative}")])
        .current_dir(&wt)
        .output()
        .unwrap();
    assert!(committed.status.success());
    assert_eq!(committed.stdout, expected.as_bytes());
    w.settle();
    for n in [2, 3] {
        assert!(w.prompt(n).contains(&format!("{slug}-replies.md")));
    }
    assert_eq!(
        w.ctx.store.count_events(&id, "bug.repro_missed").unwrap(),
        1
    );
    assert_eq!(w.node(&id).as_deref(), Some("await_info"));
    let mut blocked = World::new("reply-write-failure");
    let slug = slug_for("blocked reply");
    bug_step(
        &blocked,
        1,
        &slug,
        &bug_doc("待复核").replace("已定位", "无法复现"),
        "",
    );
    let id = create_bug(&mut blocked, "blocked reply");
    blocked.settle();
    let file = blocked
        .repo
        .join(format!(".worktree/{slug}/docs/{slug}/{slug}-replies.md"));
    std::fs::create_dir(&file).unwrap();
    let out = blocked.call("task.reply", json!({"task_id":id,"info":"落盘失败也保留"}));
    assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
    assert_eq!(
        blocked.ctx.store.task_replies(&id).unwrap()[0].1["text"],
        "落盘失败也保留"
    );
    assert!(matches!(
        blocked.ctx.store.get_task(&id).unwrap().state,
        autome_domain::task::TaskState::Failed { .. }
    ));
}

#[test]
fn bug_loop_not_a_bug_waits_for_a_decision_and_can_be_cancelled_or_rebuilt_as_a_feature() {
    needs_git!();
    let mut w = World::new("not-bug");
    let slug = slug_for("not bug");
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核").replace("已定位", "不是 Bug"),
        "",
    );
    bug_step(
        &w,
        2,
        &slug,
        &bug_doc("待复核").replace("已定位", "不是 Bug"),
        "",
    );
    let id = create_bug(&mut w, "not bug");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("not_a_bug"));
    for method in ["task.pause", "task.stop", "task.extend_budget"] {
        let out = w.call(method, json!({"task_id":id,"extra_rounds":1}));
        assert!(
            matches!(out.reply.outcome, ReplyOutcome::Error { .. }),
            "{method}"
        );
    }
    ok(&w.call("task.reply", json!({"task_id":id,"info":"确实是 Bug"})));
    w.settle();
    assert_eq!(w.ctx.store.count_events(&id, "task.replied").unwrap(), 1);
    assert_eq!(w.node(&id).as_deref(), Some("not_a_bug"));
    let brief = w.ctx.store.task_brief(&id).unwrap().unwrap();
    let out = w.call("task.create",json!({"project_id":w.project_id,"request":"new feature","kind":"feature","brief":brief,"draft":true}));
    let new_id = ok(&out)["task"]["id"].as_str().unwrap();
    assert_eq!(w.ctx.store.task_brief(new_id).unwrap(), Some(brief));
    ok(&w.call("task.cancel", json!({"task_id":id})));
    assert_eq!(w.state(&id), "cancelled");
    assert!(w.ctx.store.get_task(&id).unwrap().is_archived());
}

#[test]
fn bug_loop_two_repro_misses_in_a_row_wait_for_info() {
    needs_git!();
    let mut w = World::new("two-misses");
    let slug = slug_for("two misses");
    for n in 1..=4 {
        bug_step(
            &w,
            n,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_LOCATE}\n{BUG_FIX}"),
        );
    }
    let id = create_bug(&mut w, "two misses");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_info"));
    assert_eq!(
        w.ctx.store.count_events(&id, "bug.repro_missed").unwrap(),
        2
    );
    ok(&w.call("task.reply", json!({"task_id":id,"info":"再试一次"})));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_info"));
    assert_eq!(
        w.ctx.store.count_events(&id, "bug.repro_missed").unwrap(),
        4
    );
}

#[test]
fn bug_loop_fix_rounds_run_out_and_extend_adds_more() {
    needs_git!();
    let mut w = World::new("fix-budget");
    w.configure("[bug]\nfix_rounds = 2\n");
    let slug = slug_for("fix budget");
    for n in 1..=3 {
        bug_step(
            &w,
            n,
            &slug,
            &bug_doc("待复核"),
            if n == 1 { BUG_LOCATE } else { "" },
        );
    }
    bug_step(&w, 4, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 5, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "fix budget");
    w.settle();
    let failed = w.ctx.store.get_task(&id).unwrap();
    assert!(matches!(
        failed.state,
        autome_domain::task::TaskState::Failed {
            reason: autome_domain::task::FailureReason::FixBudget { limit: 2 },
            ..
        }
    ));
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.fix_round_started")
            .unwrap(),
        2
    );
    assert!(matches!(
        w.call("task.extend_budget", json!({"task_id":id,"extra_rounds":0}))
            .reply
            .outcome,
        ReplyOutcome::Error { .. }
    ));
    assert!(matches!(
        w.call(
            "task.extend_budget",
            json!({"task_id":id,"extra_rounds":u32::MAX})
        )
        .reply
        .outcome,
        ReplyOutcome::Error { .. }
    ));
    assert_eq!(w.ctx.store.get_task(&id).unwrap().budget_n, Some(2));
    ok(&w.call("task.extend_budget", json!({"task_id":id,"extra_rounds":1})));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let out = w.call("task.get", json!({"task_id":id}));
    assert_eq!(ok(&out)["task"]["fix_round"], 3);
    assert_eq!(ok(&out)["task"]["budget_n"], 3);
}

#[test]
fn bug_loop_task_get_reports_the_fix_round_for_bugs_only() {
    needs_git!();
    let mut w = World::new("fix-json");
    let mut keys = Vec::new();
    for kind in ["feature", "bug"] {
        let out = w.call(
            "task.create",
            json!({"project_id":w.project_id,"request":kind,"kind":kind,"draft":true}),
        );
        let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
        let out = w.call("task.get", json!({"task_id":id}));
        let mut task = ok(&out)["task"].as_object().unwrap().clone();
        if kind == "bug" {
            assert_eq!(task.remove("fix_round"), Some(json!(0)));
        } else {
            assert!(!task.contains_key("fix_round"));
        }
        keys.push(task.keys().cloned().collect::<Vec<_>>());
        let out = w.call("task.reply", json!({"task_id":id,"info":"wrong state"}));
        assert!(matches!(out.reply.outcome, ReplyOutcome::Error { .. }));
        assert_eq!(w.ctx.store.count_events(&id, "task.replied").unwrap(), 0);
    }
    assert_eq!(keys[0], keys[1]);
    let mut baseline = [
        "id",
        "cost_usd",
        "cost_sessions_without_cost",
        "kind",
        "slug",
        "title",
        "request",
        "state",
        "budget_n",
        "branch",
        "created_at",
        "completed_at",
        "merge_commit",
        "archived",
        "protocol_ref",
        "metrics",
        "doc_dir",
    ];
    baseline.sort();
    assert_eq!(keys[0], baseline);
}

#[test]
fn bug_loop_a_workspace_bug_checks_out_every_member_to_locate_then_keeps_only_the_named_repos() {
    needs_git!();
    for (repos, keep_b) in [
        ("repos: a", false),
        ("repos: a", true),
        ("", false),
        ("repos: stranger", false),
    ] {
        let mut w = Workspace::new("bug-select", &["a", "b", "docs"]);
        let slug = slug_for("workspace bug");
        let doc = bug_doc("待复核").replace(
            "复现命令: sh regression.sh",
            &format!("复现命令: cd a && sh regression.sh\n{repos}"),
        );
        let extra = if keep_b {
            "(cd b; echo preserve > keep.txt; git add keep.txt; git -c user.name=fake -c user.email=f@f commit -qm keep)"
        } else {
            ""
        };
        let check_b = if keep_b {
            "test -f b/keep.txt"
        } else {
            "test ! -e b"
        };
        w.step(1,&format!("set -e\n{extra}\ntest -d a/.git || test -f a/.git\ntest -f b/.git\n(cd a; {BUG_LOCATE}; git add -A; git -c user.name=fake -c user.email=f@f commit -qm repro)\nmkdir -p docs/autome/{slug}\ncat > docs/autome/{slug}/{slug}-bug.md <<'EOF'\n{doc}\nEOF\n"));
        w.step(
            2,
            &format!(
                "set -e\n{check_b}\ntest -f a/.git\necho kept > docs/kept.txt\n(cd a; {BUG_FIX})\n"
            ),
        );
        let reviewed = doc.replace("复核: 待复核", "复核: 通过");
        w.step(
            3,
            &format!("cat > docs/autome/{slug}/{slug}-bug.md <<'EOF'\n{reviewed}\nEOF\n"),
        );
        let out = call(
            &mut w.ctx,
            "task.create",
            json!({"project_id":w.project_id,"request":"workspace bug","kind":"bug"}),
        );
        let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
        scheduling_wait(
            &mut w,
            |w| {
                let report = automed::scheduler::tick(&mut w.ctx);
                assert!(report.errors.is_empty(), "{report:?}");
                let state = w.ctx.store.get_task(&id).unwrap().state;
                state.node().is_some_and(|node| node.awaits_user())
                    || matches!(state, autome_domain::task::TaskState::Failed { .. })
            },
            |w| w.why(&id),
        );
        if repos == "repos: a" {
            assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
            assert_eq!(w.ctx.store.count_events(&id, "bug.reproduced").unwrap(), 1);
            let cwd = w.ws().join(".worktree").join(&slug);
            assert_eq!(cwd.join("b").exists(), keep_b);
            assert!(cwd.join("docs/kept.txt").exists());
        } else {
            let task = w.ctx.store.get_task(&id).unwrap();
            assert!(
                matches!(task.state, autome_domain::task::TaskState::Failed { reason: autome_domain::task::FailureReason::Protocol { ref detail }, .. } if detail.contains("repos")),
                "{:?}",
                task.state
            );
        }
    }
}

// Use sh so this path-resolution regression needs neither Node nor a network.
// The script has no test/spec naming marker and must be recognized by command.
fn assert_cd_repro_script_protected(command: &str) {
    assert!(git_available(), "this regression requires git");
    let mut w = World::new("cd-repro-script");
    let slug = slug_for("cd repro script");
    let doc = bug_doc("待复核").replace("sh regression.sh", command);
    let path = "apps/web/scripts/x-regression.sh";
    bug_step(
        &w,
        1,
        &slug,
        &doc,
        "mkdir -p apps/web/scripts\necho broken > apps/web/product.txt\ncat > apps/web/scripts/x-regression.sh <<'SCRIPT'\ntest \"$(cat \"$(dirname \"$0\")/../product.txt\")\" = fixed\nSCRIPT",
    );
    bug_step(&w, 2, &slug, &doc, "echo fixed > apps/web/product.txt");
    bug_step(&w, 3, &slug, &doc.replace("复核: 待复核", "复核: 通过"), "");
    let id = create_bug(&mut w, "cd repro script");
    w.settle();
    let snapshot = w
        .ctx
        .store
        .last_event(&id, "bug.regression_snapshot")
        .unwrap();
    let task = w.ctx.store.get_task(&id).unwrap();
    assert_eq!(
        snapshot
            .as_ref()
            .and_then(|s| s["files"].as_array())
            .map(|files| files.len()),
        Some(1),
        "command={command}; snapshot={snapshot:?}; state={:?}",
        task.state
    );
    let file = &snapshot.as_ref().unwrap()["files"][0];
    assert_eq!(file["path"], path);
    assert_eq!(file["protect"], "file");
    assert_eq!(w.ctx.store.count_events(&id, "bug.reproduced").unwrap(), 1);
    assert_eq!(
        w.node(&id).as_deref(),
        Some("await_merge"),
        "{:?}",
        task.state
    );
    let wt = w.repo.join(format!(".worktree/{slug}"));
    let head = file["head"].as_str().unwrap();
    assert_eq!(
        std::fs::read_to_string(wt.join(path)).unwrap(),
        automed::git::run_ok(&wt, &["show", &format!("{head}:{path}")])
            .unwrap()
            .stdout
    );
}

#[test]
fn bug_cd_repro_root_relative_control() {
    assert_cd_repro_script_protected("sh apps/web/scripts/x-regression.sh");
}

#[test]
fn bug_cd_repro_relative_script_is_protected() {
    assert_cd_repro_script_protected("cd apps/web && sh scripts/x-regression.sh");
}

#[test]
fn bug_cd_repro_parenthesized_relative_script_is_protected() {
    assert_cd_repro_script_protected("( cd apps/web && sh scripts/x-regression.sh )");
}

#[test]
fn bug_cd_repro_empty_protection_fails_at_locate_without_spending_fix_round() {
    assert!(git_available(), "this regression requires git");
    let mut w = World::new("cd-repro-empty");
    let slug = slug_for("cd repro empty");
    let doc = bug_doc("待复核").replace("sh regression.sh", "test -f fixed.txt");
    bug_step(&w, 1, &slug, &doc, "echo broken > product.txt");
    bug_step(&w, 2, &slug, &doc, "touch fixed.txt");
    let id = create_bug(&mut w, "cd repro empty");
    w.settle();
    let task = w.ctx.store.get_task(&id).unwrap();
    assert!(
        matches!(&task.state, autome_domain::task::TaskState::Failed {
            at: autome_domain::task::Node::Locate,
            reason: autome_domain::task::FailureReason::Protocol { detail }, ..
        } if detail.contains("定位") && detail.contains("保护集为空")
            && detail.contains("test") && detail.contains("spec")
            && detail.contains("复现") && detail.contains("路径")),
        "empty protection must fail at locate: {:?}",
        task.state
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.fix_round_started")
            .unwrap(),
        0
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.regression_snapshot")
            .unwrap(),
        0
    );
}

#[test]
fn bug_cd_repro_legacy_empty_snapshot_still_fails_at_fix() {
    assert!(git_available(), "this regression requires git");
    let mut w = World::new("cd-repro-legacy");
    let slug = slug_for("cd repro legacy");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    let id = create_bug(&mut w, "cd repro legacy");
    resume_wait(&mut w, |w| {
        w.ctx
            .store
            .last_event(&id, "bug.regression_snapshot")
            .unwrap()
            .is_some()
    });
    let mut snapshot = w
        .ctx
        .store
        .last_event(&id, "bug.regression_snapshot")
        .unwrap()
        .unwrap();
    snapshot["files"] = json!([]);
    w.ctx
        .store
        .append_event("bug.regression_snapshot", &id, snapshot)
        .unwrap();
    w.settle();
    let task = w.ctx.store.get_task(&id).unwrap();
    assert!(
        matches!(&task.state, autome_domain::task::TaskState::Failed {
        at: autome_domain::task::Node::Fix,
        reason: autome_domain::task::FailureReason::Protocol { detail }, ..
    } if detail == "回归测试保护集为空：定位轮必须新增或修改测试文件"),
        "{:?}",
        task.state
    );
}

fn assert_regression_rejected(w: &World, id: &str, path: &str) {
    let task = w.ctx.store.get_task(id).unwrap();
    assert!(
        matches!(task.state,
        autome_domain::task::TaskState::Failed {
            at: autome_domain::task::Node::Fix,
            reason: autome_domain::task::FailureReason::Protocol { ref detail }, ..
        } if detail.contains(path)),
        "{:?}",
        task.state
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(id, "bug.regression_snapshot")
            .unwrap(),
        1
    );
}

#[test]
fn bug_regression_guard_rejects_modified_or_deleted_tests() {
    needs_git!();
    for change in [
        "echo true > regression.sh",
        "rm regression.sh",
        "mv regression.sh renamed.sh",
        "chmod +x regression.sh",
    ] {
        let mut w = World::new("regression-reject");
        let slug = slug_for("guard reject");
        bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
        bug_step(
            &w,
            2,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_FIX}\n{change}"),
        );
        let id = create_bug(&mut w, "guard reject");
        w.settle();
        assert_regression_rejected(&w, &id, "regression.sh");
        let snapshot = w
            .ctx
            .store
            .last_event(&id, "bug.regression_snapshot")
            .unwrap()
            .unwrap();
        let head = snapshot["files"][0]["head"].as_str().unwrap();
        let wt = w.repo.join(format!(".worktree/{slug}"));
        let original = automed::git::run_ok(&wt, &["show", &format!("{head}:regression.sh")])
            .unwrap()
            .stdout;
        assert_eq!(
            original,
            "echo regression-output; test \"$(cat product.txt)\" = fixed\n"
        );
        if change.starts_with("rm") || change.starts_with("mv") {
            assert!(!wt.join("regression.sh").exists());
        } else if change.starts_with("echo") {
            assert_eq!(
                std::fs::read_to_string(wt.join("regression.sh")).unwrap(),
                "true\n"
            );
        } else {
            assert_eq!(
                std::fs::read_to_string(wt.join("regression.sh")).unwrap(),
                original
            );
        }
    } // Empty input is never accepted as a protected fix.
    let mut w = World::new("regression-empty");
    let slug = slug_for("guard empty");
    let doc = bug_doc("待复核").replace("sh regression.sh", "exit 1");
    bug_step(&w, 1, &slug, &doc, "echo broken > product.txt");
    bug_step(&w, 2, &slug, &doc, BUG_FIX);
    let id = create_bug(&mut w, "guard empty");
    w.settle();
    assert!(matches!(
        w.ctx.store.get_task(&id).unwrap().state,
        autome_domain::task::TaskState::Failed {
            at: autome_domain::task::Node::Locate,
            reason: autome_domain::task::FailureReason::Protocol { detail }, ..
        } if detail.contains("保护集为空")
    ));

    // Existing tests modified in locate, inline tests, names and directories.
    for path in [
        "tests/已有 测试.txt",
        "__tests__/case.js",
        "test/case.txt",
        "sample.spec.js",
        "src/lib.rs",
    ] {
        let mut w = World::new("regression-paths");
        let slug = slug_for("guard paths");
        let full = w.repo.join(path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(&full, "old test\n").unwrap();
        automed::git::commit_paths(&w.repo, &[path], "existing test").unwrap();
        bug_step(
            &w,
            1,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_LOCATE}\necho '#[test]' > '{path}'"),
        );
        bug_step(
            &w,
            2,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_FIX}\necho weakened > '{path}'"),
        );
        let id = create_bug(&mut w, "guard paths");
        w.settle();
        assert_regression_rejected(&w, &id, path);
        assert_eq!(
            std::fs::read_to_string(w.repo.join(format!(".worktree/{slug}/{path}"))).unwrap(),
            "weakened\n"
        );
    }
    let mut w = World::new("regression-script-spaces");
    let slug = slug_for("script spaces");
    let doc = bug_doc("待复核").replace("sh regression.sh", "sh 'check bug.sh'");
    bug_step(
        &w,
        1,
        &slug,
        &doc,
        &format!("{BUG_LOCATE}\nmv regression.sh 'check bug.sh'"),
    );
    bug_step(&w, 2, &slug, &doc, "echo true > 'check bug.sh'");
    let id = create_bug(&mut w, "script spaces");
    w.settle();
    assert_regression_rejected(&w, &id, "check bug.sh");
    assert_eq!(
        std::fs::read_to_string(w.repo.join(format!(".worktree/{slug}/check bug.sh"))).unwrap(),
        "true\n"
    );
}

#[test]
fn bug_regression_guard_allows_product_only_fixes() {
    needs_git!();
    let mut w = World::new("regression-product");
    let slug = slug_for("guard product");
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核"),
        &format!(
            "{BUG_LOCATE}\nmkdir -p tests __tests__ test src\necho check > tests/a.txt\necho check > __tests__/a.js\necho check > test/a.txt\necho check > sample.spec.js\necho '#[test]' > src/lib.rs"
        ),
    );
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "guard product");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let snapshot = w
        .ctx
        .store
        .last_event(&id, "bug.regression_snapshot")
        .unwrap()
        .unwrap();
    assert_eq!(snapshot["files"].as_array().unwrap().len(), 6);
    let wt = w.repo.join(format!(".worktree/{slug}"));
    for file in snapshot["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        let head = file["head"].as_str().unwrap();
        assert_eq!(
            std::fs::read(wt.join(path)).unwrap(),
            automed::git::run_ok(&wt, &["show", &format!("{head}:{path}")])
                .unwrap()
                .stdout
                .as_bytes()
        );
    }
    assert_eq!(
        std::fs::read_to_string(wt.join("product.txt")).unwrap(),
        "fixed\n"
    ); // A new locate takes a new baseline rather than retaining the first blob.
    let mut w = World::new("regression-relocate");
    let slug = slug_for("guard relocate");
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核"),
        &format!("{BUG_LOCATE}\n{BUG_FIX}"),
    );
    bug_step(
        &w,
        2,
        &slug,
        &bug_doc("待复核"),
        &format!("{BUG_LOCATE}\necho '# second locate' >> regression.sh"),
    );
    bug_step(&w, 3, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 4, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "guard relocate");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "bug.regression_snapshot")
            .unwrap(),
        2
    );
    assert!(
        std::fs::read_to_string(w.repo.join(format!(".worktree/{slug}/regression.sh")))
            .unwrap()
            .ends_with("# second locate\n")
    );
    let mut w = World::new("regression-inline-product");
    let slug = slug_for("inline product");
    std::fs::write(
        w.repo.join("lib.rs"),
        "old product\n#[cfg(test)]\nmod tests {}\n",
    )
    .unwrap();
    automed::git::commit_paths(&w.repo, &["lib.rs"], "existing inline tests").unwrap();
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核"),
        &format!(
            "{BUG_LOCATE}\nprintf 'broken product\\n#[cfg(test)]\\nmod tests {{}}\\n' > lib.rs"
        ),
    );
    bug_step(
        &w,
        2,
        &slug,
        &bug_doc("待复核"),
        &format!("{BUG_FIX}\nprintf 'fixed product\\n#[cfg(test)]\\nmod tests {{}}\\n' > lib.rs"),
    );
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "inline product");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let snapshot = w
        .ctx
        .store
        .last_event(&id, "bug.regression_snapshot")
        .unwrap()
        .unwrap();
    assert_eq!(snapshot["files"].as_array().unwrap().len(), 1);
    assert_eq!(
        std::fs::read_to_string(w.repo.join(format!(".worktree/{slug}/lib.rs"))).unwrap(),
        "fixed product\n#[cfg(test)]\nmod tests {}\n"
    );
}

#[test]
fn bug_regression_guard_protects_only_the_test_section_of_an_inline_test_file() {
    needs_git!();
    // Rust keeps unit tests in the product file. A regression test added to
    // that module must not freeze the product code the fix has to change.
    let located = "broken product\\n#[cfg(test)]\\nmod tests { #[test] fn regression() {} }\\n";
    for (fix, allowed) in [
        (
            "fixed product\\n#[cfg(test)]\\nmod tests { #[test] fn regression() {} }\\n",
            true,
        ),
        ("fixed product\\n#[cfg(test)]\\nmod tests {}\\n", false),
        // A new test beside the protected one is not a weakening.
        (
            "fixed product\\n#[cfg(test)]\\nmod tests { #[test] fn regression() {} }\\n#[test]\\nfn also_covers_the_edge() {}\\n",
            true,
        ),
    ] {
        let mut w = World::new("regression-section");
        let slug = slug_for("inline section");
        std::fs::write(
            w.repo.join("lib.rs"),
            "old product\n#[cfg(test)]\nmod tests {}\n",
        )
        .unwrap();
        automed::git::commit_paths(&w.repo, &["lib.rs"], "existing inline tests").unwrap();
        bug_step(
            &w,
            1,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_LOCATE}\nprintf '{located}' > lib.rs"),
        );
        bug_step(
            &w,
            2,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_FIX}\nprintf '{fix}' > lib.rs"),
        );
        bug_step(&w, 3, &slug, &bug_doc("通过"), "");
        let id = create_bug(&mut w, "inline section");
        w.settle();
        let snapshot = w
            .ctx
            .store
            .last_event(&id, "bug.regression_snapshot")
            .unwrap()
            .unwrap();
        let lib = snapshot["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "lib.rs")
            .expect("lib.rs is protected");
        assert_eq!(lib["protect"], "section");
        if allowed {
            assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
            assert!(
                std::fs::read_to_string(w.repo.join(format!(".worktree/{slug}/lib.rs")))
                    .unwrap()
                    .starts_with("fixed product\n")
            );
        } else {
            assert_regression_rejected(&w, &id, "lib.rs");
        }
    }
}

#[test]
fn bug_regression_guard_protects_only_the_added_tests_of_an_existing_test_file() {
    needs_git!();
    // The bug can live in the test suite's own helpers. A regression test
    // added to that suite must not freeze the helper the fix has to change.
    let located = "function settle() { return early; }\\ntest(\"old\", () => settle());\\ntest(\"regression\", () => settle());\\n";
    for (fix, allowed) in [
        (
            "function settle() { return late; }\\ntest(\"old\", () => settle());\\ntest(\"regression\", () => settle());\\n",
            true,
        ),
        (
            "function settle() { return late; }\\ntest(\"old\", () => settle());\\n",
            false,
        ),
        (
            "function settle() { return late; }\\ntest(\"old\", () => settle());\\ntest(\"regression\", () => {});\\n",
            false,
        ),
    ] {
        let mut w = World::new("regression-suite");
        let slug = slug_for("suite helper");
        std::fs::create_dir_all(w.repo.join("tests")).unwrap();
        std::fs::write(
            w.repo.join("tests/suite.js"),
            "function settle() { return early; }\ntest(\"old\", () => settle());\n",
        )
        .unwrap();
        automed::git::commit_paths(&w.repo, &["tests/suite.js"], "existing suite").unwrap();
        bug_step(
            &w,
            1,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_LOCATE}\nprintf '{located}' > tests/suite.js"),
        );
        bug_step(
            &w,
            2,
            &slug,
            &bug_doc("待复核"),
            &format!("{BUG_FIX}\nprintf '{fix}' > tests/suite.js"),
        );
        bug_step(&w, 3, &slug, &bug_doc("通过"), "");
        let id = create_bug(&mut w, "suite helper");
        w.settle();
        let snapshot = w
            .ctx
            .store
            .last_event(&id, "bug.regression_snapshot")
            .unwrap()
            .unwrap();
        let suite = snapshot["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["path"] == "tests/suite.js")
            .expect("tests/suite.js is protected");
        assert_eq!(suite["protect"], "blocks");
        if allowed {
            assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
        } else {
            assert_regression_rejected(&w, &id, "tests/suite.js");
        }
    }
}

#[test]
fn bug_regression_guard_leaves_the_bug_doc_and_prose_alone() {
    needs_git!();
    // A Bug doc that quotes `#[test]` is still the protocol's own file: the
    // fix round must be able to write its 修复说明 into it.
    let mut w = World::new("regression-prose");
    let slug = slug_for("guard prose");
    let located = format!("{}\n## 根因分析\n漏了 `#[test]` 标注\n", bug_doc("待复核"));
    let fixed = format!("{located}\n## 修复说明\n补上了\n");
    bug_step(
        &w,
        1,
        &slug,
        &located,
        &format!("{BUG_LOCATE}\nprintf '# notes\\n#[test] is quoted here\\n' > NOTES.md"),
    );
    bug_step(
        &w,
        2,
        &slug,
        &fixed,
        &format!("{BUG_FIX}\nprintf '# notes\\nrewritten\\n' > NOTES.md"),
    );
    bug_step(
        &w,
        3,
        &slug,
        &fixed.replace("复核: 待复核", "复核: 通过"),
        "",
    );
    let id = create_bug(&mut w, "guard prose");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let snapshot = w
        .ctx
        .store
        .last_event(&id, "bug.regression_snapshot")
        .unwrap()
        .unwrap();
    let paths: Vec<&str> = snapshot["files"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert_eq!(paths, vec!["regression.sh"]);
}

#[test]
fn bug_regression_guard_ignores_the_bug_doc_in_a_snapshot_from_an_older_build() {
    needs_git!();
    // An older build put the Bug doc itself into the snapshot. A task located
    // under that build must still be able to write its 修复说明 afterwards.
    let mut w = World::new("regression-old-snapshot");
    let slug = slug_for("guard old snapshot");
    let located = bug_doc("待复核");
    let fixed = format!("{located}\n## 修复说明\n补上了\n");
    bug_step(&w, 1, &slug, &located, BUG_LOCATE);
    bug_step(&w, 2, &slug, &fixed, BUG_FIX);
    bug_step(
        &w,
        3,
        &slug,
        &fixed.replace("复核: 待复核", "复核: 通过"),
        "",
    );
    let id = create_bug(&mut w, "guard old snapshot");
    resume_wait(&mut w, |w| {
        w.ctx
            .store
            .last_event(&id, "bug.regression_snapshot")
            .unwrap()
            .is_some()
    });
    let mut snapshot = w
        .ctx
        .store
        .last_event(&id, "bug.regression_snapshot")
        .unwrap()
        .unwrap();
    snapshot["files"].as_array_mut().unwrap().push(json!({
        "repo": "",
        "path": format!("docs/{slug}/{slug}-bug.md"),
        "protect": "file",
        "tree": "stale",
    }));
    w.ctx
        .store
        .append_event("bug.regression_snapshot", &id, snapshot)
        .unwrap();
    w.settle();
    assert_eq!(
        w.node(&id).as_deref(),
        Some("await_merge"),
        "{:?}",
        w.ctx.store.get_task(&id).unwrap().state
    );
}

#[test]
fn bug_regression_guard_survives_retry_and_extend() {
    needs_git!();
    for mutate in [false, true] {
        let mut w = World::new("regression-retry");
        w.configure("[bug]\nfix_rounds = 1\n[retry]\nattempts = 1\nbackoff_seconds = 0\n");
        let slug = slug_for("guard retry");
        bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
        w.step(2, "exit 17\n");
        bug_step(&w, 3, &slug, &bug_doc("待复核"), "");
        bug_step(
            &w,
            4,
            &slug,
            &bug_doc("待复核"),
            if mutate {
                "echo true > regression.sh"
            } else {
                BUG_FIX
            },
        );
        bug_step(&w, 5, &slug, &bug_doc("通过"), "");
        let id = create_bug(&mut w, "guard retry");
        w.settle();
        assert!(matches!(
            w.ctx.store.get_task(&id).unwrap().state,
            autome_domain::task::TaskState::Failed {
                reason: autome_domain::task::FailureReason::FixBudget { .. },
                ..
            }
        ));
        let before = w
            .ctx
            .store
            .last_event(&id, "bug.regression_snapshot")
            .unwrap();
        ok(&w.call(
            "task.extend_budget",
            json!({"task_id": id, "extra_rounds": 1}),
        ));
        w.settle();
        assert_eq!(
            w.ctx
                .store
                .last_event(&id, "bug.regression_snapshot")
                .unwrap(),
            before
        );
        assert_eq!(w.ctx.store.count_events(&id, "session.retry").unwrap(), 1);
        if mutate {
            assert_regression_rejected(&w, &id, "regression.sh");
        } else {
            assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
        }
        let bytes = std::fs::read_to_string(w.repo.join(format!(".worktree/{slug}/regression.sh")))
            .unwrap();
        assert_eq!(
            bytes,
            if mutate {
                "true\n"
            } else {
                "echo regression-output; test \"$(cat product.txt)\" = fixed\n"
            }
        );
    }
    for mutate in [false, true] {
        let mut w = World::new("regression-pause");
        let slug = slug_for("guard pause");
        bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
        let wait = format!(
            "{}\n{}",
            pause_prelude(&w, 2),
            if mutate {
                "echo true > regression.sh"
            } else {
                BUG_FIX
            }
        );
        bug_step(&w, 2, &slug, &bug_doc("待复核"), &wait);
        bug_step(&w, 3, &slug, &bug_doc("通过"), "");
        let id = create_bug(&mut w, "guard pause");
        resume_wait(&mut w, |w| w.root.join("ready-2").exists());
        ok(&w.call("task.pause", json!({"task_id": id})));
        std::fs::write(w.root.join("release-2"), "go").unwrap();
        resume_wait(&mut w, |w| {
            w.ctx.store.running_session(&id).unwrap().is_none()
        });
        ok(&w.call("task.resume", json!({"task_id": id})));
        w.settle();
        if mutate {
            assert_regression_rejected(&w, &id, "regression.sh");
        } else {
            assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
        }
        assert_eq!(
            std::fs::read_to_string(w.repo.join(format!(".worktree/{slug}/regression.sh")))
                .unwrap(),
            if mutate {
                "true\n"
            } else {
                "echo regression-output; test \"$(cat product.txt)\" = fixed\n"
            }
        );
    }
}

#[test]
fn bug_regression_guard_checks_each_workspace_member() {
    needs_git!();
    for changed in ["a", "b", "none"] {
        let mut w = Workspace::new("regression-workspace", &["a", "b", "docs"]);
        let slug = slug_for("guard workspace");
        let doc = bug_doc("待复核").replace(
            "复现命令: sh regression.sh",
            "复现命令: cd a && sh regression.sh\nrepos: a,b",
        );
        w.step(1, &format!("set -e\n(cd a; {BUG_LOCATE})\n(cd b; {BUG_LOCATE})\nmkdir -p docs/autome/{slug}\ncat > docs/autome/{slug}/{slug}-bug.md <<'EOF'\n{doc}\nEOF"));
        let mutation = if changed == "none" {
            String::new()
        } else {
            format!("echo true > {changed}/regression.sh")
        };
        w.step(
            2,
            &format!("(cd a; {BUG_FIX})\n(cd b; {BUG_FIX})\n{mutation}"),
        );
        w.step(
            3,
            &format!(
                "cat > docs/autome/{slug}/{slug}-bug.md <<'EOF'\n{}\nEOF",
                doc.replace("复核: 待复核", "复核: 通过")
            ),
        );
        let out = call(
            &mut w.ctx,
            "task.create",
            json!({"project_id": w.project_id, "request":"guard workspace", "kind":"bug"}),
        );
        let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
        scheduling_wait(
            &mut w,
            |w| {
                let report = automed::scheduler::tick(&mut w.ctx);
                assert!(report.errors.is_empty(), "{report:?}");
                let state = w.ctx.store.get_task(&id).unwrap().state;
                state.node().is_some_and(|node| node.awaits_user())
                    || matches!(state, autome_domain::task::TaskState::Failed { .. })
            },
            |w| w.why(&id),
        );
        let task = w.ctx.store.get_task(&id).unwrap();
        if changed == "none" {
            assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
        } else {
            assert!(
                matches!(task.state, autome_domain::task::TaskState::Failed { at: autome_domain::task::Node::Fix, reason: autome_domain::task::FailureReason::Protocol { ref detail }, .. } if detail.contains(&format!("{changed}/regression.sh"))),
                "{:?}",
                task.state
            );
        }
        for member in ["a", "b"] {
            let bytes = std::fs::read_to_string(
                w.ws()
                    .join(format!(".worktree/{slug}/{member}/regression.sh")),
            )
            .unwrap();
            assert_eq!(
                bytes,
                if changed == member {
                    "true\n"
                } else {
                    "echo regression-output; test \"$(cat product.txt)\" = fixed\n"
                }
            );
        }
    }
}

// Preparation uses real host jobs, with only the model session substituted.
fn setup_fixture(w: &World, request: &str, script_body: &str) -> String {
    let script = w.root.join("setup.sh");
    std::fs::write(&script, format!(
        "echo \"$PWD $(cat deps.lock 2>/dev/null | tr '\n' ' ')\" >> '{root}/setup-runs'\necho setup-output\n{script_body}\nmkdir -p deps\necho ready > deps/stamp\n",
        root = w.root.display()
    )).unwrap();
    let ignore = w.repo.join(".gitignore");
    let mut text = std::fs::read_to_string(&ignore).unwrap();
    text.push_str("\ndeps/\n");
    std::fs::write(ignore, text).unwrap();
    std::fs::write(w.repo.join("deps.lock"), "v1").unwrap();
    automed::git::commit_paths(&w.repo, &[".gitignore", "deps.lock"], "setup fixture").unwrap();
    let command = format!("sh {}", script.display());
    w.configure(&format!(
        "[setup]\ncommand = \"{command}\"\nwatch = [\"deps.lock\"]\n"
    ));
    let slug = slug_for(request);
    w.doc_step_for_prelude(
        1,
        &slug,
        &doc("设计中", 0, 0, &[]),
        &format!(
            "[ -f deps/stamp ] || touch '{}'",
            w.root.join("no-stamp").display()
        ),
    );
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    command
}

fn setup_events(w: &World, id: &str, kind: &str) -> Vec<(u64, Value)> {
    w.ctx
        .store
        .events_since_with_subject(0, u32::MAX)
        .unwrap()
        .into_iter()
        .filter(|(_, k, subject, _)| k == kind && subject == id)
        .map(|(seq, _, _, payload)| (seq, payload))
        .collect()
}

fn setup_runs(w: &World) -> Vec<String> {
    std::fs::read_to_string(w.root.join("setup-runs"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn setup_runs_in_the_task_worktree_before_the_first_session() {
    needs_git!();
    let mut w = World::new("setup-first");
    let request = "setup first";
    setup_fixture(&w, request, "");
    let id = create_task(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_design_approval"));
    assert!(!w.root.join("no-stamp").exists());
    let results = setup_events(&w, &id, "setup.result");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1["ok"], true);
    assert_eq!(results[0].1["checkout"], "worktree");
    assert!(results[0].0 < setup_events(&w, &id, "session.started")[0].0);
    assert_eq!(
        setup_runs(&w),
        vec![format!(
            "{} v1",
            w.repo
                .join(format!(".worktree/{}", slug_for(request)))
                .canonicalize()
                .unwrap()
                .display()
        )]
    );
}

#[test]
fn setup_failure_before_the_first_session_fails_the_task_with_command_exit_code_and_log() {
    needs_git!();
    let mut w = World::new("setup-failure");
    let request = "setup failure";
    let command = setup_fixture(&w, request, "exit 7");
    let id = create_task(&mut w, request);
    w.settle();
    let state = serde_json::to_value(w.ctx.store.get_task(&id).unwrap().state).unwrap();
    assert_eq!(state["state"], "failed");
    assert_eq!(state["reason"]["kind"], "setup");
    assert_eq!(state["reason"]["exit_code"], 7);
    assert_eq!(state["reason"]["command"], command);
    let log = state["reason"]["log"].as_str().unwrap();
    assert!(
        std::fs::read_to_string(log)
            .unwrap()
            .contains("setup-output")
    );
    assert!(setup_events(&w, &id, "session.started").is_empty());
    assert!(setup_events(&w, &id, "gate.started").is_empty());
    assert!(!automed::setup::worktree_record(&w.repo, &id).exists());
    let script = w.root.join("setup.sh");
    let body = std::fs::read_to_string(&script)
        .unwrap()
        .replace("exit 7", "true");
    std::fs::write(script, body).unwrap();
    ok(&w.call("task.rerun_from", json!({"task_id":id,"node":"intake"})));
    w.settle();
    assert_eq!(setup_events(&w, &id, "setup.started").len(), 2);
    assert_eq!(setup_events(&w, &id, "setup.result")[1].1["ok"], true);
    assert!(!setup_events(&w, &id, "session.started").is_empty());
}

#[test]
fn setup_still_running_when_the_core_restarts_is_collected_not_run_again() {
    needs_git!();
    let mut w = World::new("setup-restart");
    let request = "setup restart";
    setup_fixture(&w, request, "sleep 1");
    let id = create_task(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("setup-runs").exists());
    assert!(setup_events(&w, &id, "setup.result").is_empty());
    automed::scheduler::recover(&mut w.ctx);
    w.settle();
    assert_eq!(setup_runs(&w).len(), 1);
    assert_eq!(setup_events(&w, &id, "setup.started").len(), 1);
    assert!(!setup_events(&w, &id, "session.started").is_empty());
}

#[test]
fn setup_stopped_by_the_user_is_killed_and_runs_again_on_resume() {
    needs_git!();
    let mut w = World::new("setup-stop");
    let request = "setup stop";
    std::fs::write(w.root.join("slow"), "").unwrap();
    setup_fixture(
        &w,
        request,
        &format!(
            "echo $$ > '{root}/setup-pid'\nif [ -f '{root}/slow' ]; then sleep 30; fi",
            root = w.root.display()
        ),
    );
    let id = create_task(&mut w, request);
    resume_wait(&mut w, |w| {
        std::fs::read_to_string(w.root.join("setup-pid"))
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
            .is_some()
    });
    let pid: i32 = std::fs::read_to_string(w.root.join("setup-pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    ok(&w.call("task.stop", json!({"task_id":id})));
    resume_wait(&mut w, |_| !automed::launcher::pid_alive(pid));
    assert!(!automed::launcher::pid_alive(pid));
    assert!(
        std::fs::read_dir(automed::gate::dir_for(&w.repo, &id))
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("setup-"))
    );
    std::fs::remove_file(w.root.join("slow")).unwrap();
    ok(&w.call("task.resume", json!({"task_id":id})));
    w.settle();
    assert_eq!(setup_events(&w, &id, "setup.started").len(), 2);
    assert_eq!(setup_events(&w, &id, "setup.result")[0].1["ok"], true);
    assert!(!setup_events(&w, &id, "session.started").is_empty());
}

#[test]
fn setup_events_are_absent_when_no_setup_is_configured() {
    needs_git!();
    let mut w = World::new("setup-none");
    let id = run_gated_task(&mut w, "without setup");
    ok(&w.call("task.merge", json!({"task_id":id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    for kind in ["setup.started", "setup.result", "setup.skipped"] {
        assert!(setup_events(&w, &id, kind).is_empty());
    }
}

#[test]
fn setup_run_in_flight_is_collected_under_its_own_fingerprint_when_the_watched_file_changes() {
    needs_git!();
    let mut w = World::new("setup-changed");
    let request = "setup changed";
    setup_fixture(&w, request, "sleep 2");
    let id = create_task(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("setup-runs").exists());
    std::fs::write(
        w.repo
            .join(format!(".worktree/{}/deps.lock", slug_for(request))),
        "v1\nv2",
    )
    .unwrap();
    w.settle();
    let starts = setup_events(&w, &id, "setup.started");
    let results = setup_events(&w, &id, "setup.result");
    assert_eq!(starts.len(), 2);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].1["fingerprint"], starts[0].1["fingerprint"]);
    assert_eq!(results[1].1["fingerprint"], starts[1].1["fingerprint"]);
    assert_ne!(starts[0].1["fingerprint"], starts[1].1["fingerprint"]);
    assert_eq!(starts[1].1["reason"], "变化");
    assert!(results.iter().all(|(_, p)| p["ok"] == true));
    assert!(results[0].0 < starts[1].0);
    assert!(results[1].0 < setup_events(&w, &id, "session.started")[0].0);
    assert_eq!(setup_runs(&w).len(), 2);
    let files: Vec<_> = std::fs::read_dir(automed::gate::dir_for(&w.repo, &id))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("setup-"))
        .collect();
    assert_eq!(files, vec!["setup-worktree.log"]);
}

#[test]
fn setup_run_in_flight_that_fails_after_the_watched_file_changed_still_fails_the_task() {
    needs_git!();
    let mut w = World::new("setup-changed-fail");
    let request = "setup changed fail";
    setup_fixture(&w, request, "sleep 2; exit 3");
    let id = create_task(&mut w, request);
    resume_wait(&mut w, |w| w.root.join("setup-runs").exists());
    std::fs::write(
        w.repo
            .join(format!(".worktree/{}/deps.lock", slug_for(request))),
        "v2",
    )
    .unwrap();
    w.settle();
    let state = serde_json::to_value(w.ctx.store.get_task(&id).unwrap().state).unwrap();
    assert_eq!(state["state"], "failed");
    assert_eq!(state["reason"]["kind"], "setup");
    assert_eq!(state["reason"]["exit_code"], 3);
    assert_eq!(setup_events(&w, &id, "setup.started").len(), 1);
    let results = setup_events(&w, &id, "setup.result");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1["ok"], false);
    assert!(setup_events(&w, &id, "session.started").is_empty());
    assert!(!automed::setup::worktree_record(&w.repo, &id).exists());
}

#[test]
fn setup_pause_cancel_vanished_and_input_boundaries_follow_the_same_job_lifecycle() {
    needs_git!();
    for action in ["pause", "cancel", "vanished", "unattributed", "finishing"] {
        let mut w = World::new(&format!("setup-{action}"));
        let request = format!("setup {action}");
        setup_fixture(&w, &request, "sleep 30");
        let id = create_task(&mut w, &request);
        resume_wait(&mut w, |w| w.root.join("setup-runs").exists());
        let job = automed::gate::Job::at(&w.repo, &id, "setup", "worktree");
        assert!(automed::gate::running(&job));
        if action == "pause" {
            ok(&w.call("task.pause", json!({"task_id":id})));
            assert!(automed::gate::running(&job));
            assert!(setup_events(&w, &id, "session.started").is_empty());
            // Resume collects the original job rather than launching another.
            ok(&w.call("task.resume", json!({"task_id":id})));
            assert_eq!(setup_events(&w, &id, "setup.started").len(), 1);
            ok(&w.call("task.stop", json!({"task_id":id})));
        } else if action == "finishing" {
            // Reproduce the exit-publication/PID-removal window with a live
            // real wrapper. Its paths cannot be collected and reused yet.
            std::fs::write(
                &job.exit,
                "exit_code=0\nended_at=2026-11-11T00:00:00Z\nmarker_end\n",
            )
            .unwrap();
            automed::scheduler::tick(&mut w.ctx);
            assert!(setup_events(&w, &id, "setup.result").is_empty());
            assert!(setup_events(&w, &id, "session.started").is_empty());
            assert_eq!(setup_events(&w, &id, "setup.started").len(), 1);
            ok(&w.call("task.stop", json!({"task_id":id})));
        } else if action == "cancel" {
            ok(&w.call("task.cancel", json!({"task_id":id})));
            assert!(!automed::gate::running(&job));
            assert!(!job.exists());
        } else {
            automed::gate::stop_all(&w.repo, &id);
            if action == "unattributed" {
                std::fs::remove_file(
                    w.repo
                        .join(".autome/output/setup")
                        .join(&id)
                        .join("worktree.running"),
                )
                .unwrap();
                std::fs::write(
                    &job.exit,
                    "exit_code=0\nended_at=2026-11-11T00:00:00Z\nmarker_end\n",
                )
                .unwrap();
            } else {
                let _ = std::fs::remove_file(&job.exit);
                // Dead pid and no exit marker must restart, not claim success.
            }
            let script = w.root.join("setup.sh");
            let text = std::fs::read_to_string(&script)
                .unwrap()
                .replace("sleep 30", "true");
            std::fs::write(script, text).unwrap();
            w.settle();
            assert_eq!(setup_events(&w, &id, "setup.started").len(), 2);
            assert_eq!(setup_events(&w, &id, "setup.result").len(), 1);
            assert!(!setup_events(&w, &id, "session.started").is_empty());
        }
    }

    // Host-side file reads: a directory and a missing file are both absent,
    // whereas an empty file has a distinct fingerprint.
    let mut w = World::new("setup-inputs");
    let request = "setup inputs";
    setup_fixture(&w, request, "");
    let id = create_task(&mut w, request);
    w.settle();
    let task = w.ctx.store.get_task(&id).unwrap();
    let project = w.ctx.store.get_project(&w.project_id).unwrap();
    let dir = w.repo.join(format!(".worktree/{}", slug_for(request)));
    let record = automed::setup::worktree_record(&w.repo, &id);
    let mut config = autome_domain::config::SetupConfig {
        command: "true".into(),
        watch: vec!["missing".into(), "directory".into()],
        timeout_minutes: 30,
    };
    std::fs::create_dir(dir.join("directory")).unwrap();
    let absent =
        autome_domain::setup::fingerprint("true", &[("missing", None), ("directory", None)]);
    std::fs::write(&record, &absent).unwrap();
    let check = |ctx: &mut Ctx, config: &autome_domain::config::SetupConfig| {
        automed::setup::ensure(
            ctx,
            &task,
            &project,
            config,
            automed::setup::Checkout {
                kind: autome_domain::setup::Checkout::Worktree,
                dir: &dir,
                record: record.clone(),
            },
            autome_domain::task::Node::Intake,
        )
        .unwrap()
    };
    assert!(matches!(
        check(&mut w.ctx, &config),
        automed::setup::Ready::Ready
    ));
    std::fs::write(dir.join("missing"), "").unwrap();
    assert!(matches!(
        check(&mut w.ctx, &config),
        automed::setup::Ready::Pending
    ));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match check(&mut w.ctx, &config) {
            automed::setup::Ready::Ready => break,
            automed::setup::Ready::Pending => {
                assert!(std::time::Instant::now() < deadline, "setup did not finish");
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            automed::setup::Ready::Failed { .. } => panic!("successful input check failed"),
        }
    }
    assert_ne!(std::fs::read_to_string(&record).unwrap(), absent);
    config.watch.clear();
    std::fs::write(&record, autome_domain::setup::fingerprint("true", &[])).unwrap();
    assert!(matches!(
        check(&mut w.ctx, &config),
        automed::setup::Ready::Ready
    ));
}

fn setup_gate_task(w: &mut World, request: &str, body: &str, fail_setup: bool) -> (String, String) {
    let command = setup_fixture(w, request, body);
    let gate = write_gate_script(w);
    w.configure(&format!("[gate]\ncommand = \"sh {}\"\n", gate.display()));
    let slug = slug_for(request);
    w.doc_step_for_prelude(
        5,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
        if fail_setup {
            "echo bad > deps.lock\ngit add deps.lock"
        } else {
            ""
        },
    );
    w.doc_step_for_prelude(
        6,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "待审")]),
        "echo v2 > deps.lock\necho ok > gate-ok\ngit add deps.lock gate-ok",
    );
    w.doc_step(7, &slug, &doc("实现中", 2, 2, &[("M-01", "已完成")]));
    w.retro_step(8, &slug);
    let id = create_task(w, request);
    w.settle();
    w.call("task.approve", json!({"task_id": id}));
    w.settle();
    (id, command)
}

#[test]
fn setup_reruns_before_the_gate_only_when_a_watched_file_changed() {
    needs_git!();
    let mut w = World::new("setup-gate-change");
    let (id, _) = setup_gate_task(&mut w, "setup gate change", "", false);
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let results = setup_events(&w, &id, "setup.result");
    let gates = setup_events(&w, &id, "gate.started");
    assert_eq!(
        setup_runs(&w).len(),
        2,
        "runs={:?}; results={results:?}; starts={:?}",
        setup_runs(&w),
        setup_events(&w, &id, "setup.started")
    );
    assert_eq!(results.len(), 2);
    assert_eq!(gates.len(), 3);
    assert_eq!(gates[2].1["node"], "merge_gate");
    assert!(results[0].0 < gates[0].0 && gates[0].0 < results[1].0 && results[1].0 < gates[1].0);
    assert_eq!(results[1].1["node"], "gate");
    assert_eq!(setup_events(&w, &id, "setup.started").len(), 2);
}

#[test]
fn setup_failure_before_a_gate_fails_the_task_and_starts_no_gate() {
    needs_git!();
    let mut w = World::new("setup-gate-fail");
    let (id, command) = setup_gate_task(
        &mut w,
        "setup gate fail",
        "if grep -q bad deps.lock; then exit 9; fi",
        true,
    );
    let task = w.ctx.store.get_task(&id).unwrap();
    let state = serde_json::to_value(task.state).unwrap();
    assert_eq!(state["state"], "failed", "{state}");
    assert_eq!(state["at"], "gate");
    assert_eq!(state["reason"]["kind"], "setup");
    assert_eq!(state["reason"]["exit_code"], 9);
    assert_eq!(state["reason"]["command"], command);
    assert!(
        state["reason"]["log"]
            .as_str()
            .unwrap()
            .ends_with("setup-worktree.log")
    );
    assert!(setup_events(&w, &id, "gate.started").is_empty());
    assert_eq!(w.ctx.store.list_sessions(&id).unwrap().len(), 5);
}

fn assert_setup_verify_removed(w: &World, id: &str) {
    let commit = w.ctx.store.get_task(id).unwrap().merge_commit.unwrap();
    assert!(
        !w.repo
            .join(format!(".worktree/_verify-{}", &commit[..12]))
            .exists()
    );
    assert!(!automed::setup::verify_record(&w.repo, id, &commit).exists());
}

#[test]
fn setup_runs_in_the_merge_commit_checkout_before_the_main_check() {
    needs_git!();
    let mut w = World::new("setup-verify");
    let (id, _) = setup_gate_task(&mut w, "setup verify", "", false);
    // The gate genuinely requires preparation in the fresh checkout.
    std::fs::write(w.root.join("gate.sh"), "test -f deps/stamp\n").unwrap();
    merge_after_a_late_branch_commit(&mut w, &id);
    assert_eq!(w.state(&id), "done");
    let results = setup_events(&w, &id, "setup.result");
    let verify = results
        .iter()
        .find(|(_, e)| e["checkout"] == "verify")
        .unwrap();
    let gates = setup_events(&w, &id, "gate.started");
    let main = gates
        .iter()
        .find(|(_, e)| e["node"] == "main_check")
        .unwrap();
    assert!(verify.0 < main.0);
    assert_eq!(verify.1["ok"], true);
    assert_eq!(
        setup_runs(&w)
            .iter()
            .filter(|line| line.contains("_verify-"))
            .count(),
        1
    );
    assert_eq!(
        setup_events(&w, &id, "gate.result").last().unwrap().1["ok"],
        true
    );
    assert!(w.ctx.store.open_alerts().unwrap().is_empty());
    assert_setup_verify_removed(&w, &id);
}

#[test]
fn setup_failure_in_the_merge_commit_checkout_raises_an_alert_and_skips_the_main_check() {
    needs_git!();
    let mut w = World::new("setup-verify-fail");
    let (id, command) = setup_gate_task(
        &mut w,
        "setup verify fail",
        "case \"$PWD\" in *_verify-*) exit 5 ;; esac",
        false,
    );
    merge_after_a_late_branch_commit(&mut w, &id);
    assert_eq!(w.state(&id), "done");
    let alerts = w.ctx.store.open_alerts().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, "setup_failed");
    for text in [&command, "退出码 5", "setup-verify.log"] {
        assert!(alerts[0].detail.contains(text), "{}", alerts[0].detail);
    }
    let skipped = setup_events(&w, &id, "gate.skipped");
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].1["node"], "main_check");
    assert!(
        setup_events(&w, &id, "gate.started")
            .iter()
            .all(|(_, e)| e["node"] != "main_check")
    );
    assert_setup_verify_removed(&w, &id);
}

#[test]
fn setup_reruns_before_the_repro_check_when_the_locate_round_changed_a_watched_file() {
    needs_git!();
    let mut w = World::new("setup-repro");
    let request = "setup repro";
    setup_fixture(&w, request, "");
    let slug = slug_for(request);
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核"),
        &format!("{BUG_LOCATE}\necho located > deps.lock"),
    );
    bug_step(
        &w,
        2,
        &slug,
        &bug_doc("待复核"),
        &format!("{BUG_FIX}\necho fixed > deps.lock"),
    );
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let results = setup_events(&w, &id, "setup.result");
    assert_eq!(results.len(), 3);
    let gates = setup_events(&w, &id, "gate.started");
    for (index, node) in [(1, "repro_check"), (2, "fix_gate")] {
        assert_eq!(results[index].1["node"], node);
        assert_eq!(results[index].1["ok"], true);
        assert!(results[index].0 < gates.iter().find(|(_, e)| e["node"] == node).unwrap().0);
    }
    assert!(gates.iter().any(|(_, e)| e["node"] == "merge_gate"));
}

fn setup_main_task(w: &mut World, request: &str, change: bool) -> String {
    let slug = slug_for(request);
    for (n, body) in [
        (1, doc("设计中", 0, 0, &[])),
        (2, doc("设计中", 1, 0, &[])),
        (3, doc("设计中", 1, 0, &[])),
        (4, doc("实现中", 2, 0, &[("M-01", "开放")])),
    ] {
        w.doc_step(n, &slug, &body);
    }
    w.doc_step_for_prelude(
        5,
        &slug,
        &doc("实现中", 2, 1, &[("M-01", "待审")]),
        if change {
            "echo v2 > deps.lock\ngit add deps.lock"
        } else {
            ""
        },
    );
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    std::fs::write(w.repo.join(".autome/fake/next"), "1").unwrap();
    let id = create_task(w, request);
    w.settle();
    ok(&w.call("task.approve", json!({"task_id": id})));
    w.settle();
    ok(&w.call("task.merge", json!({"task_id": id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    id
}

#[test]
fn setup_in_the_main_checkout_runs_after_a_merge_only_when_the_watch_changed() {
    needs_git!();
    let mut w = World::new("setup-main");
    setup_fixture(&w, "main a", "");
    let mut total = 0;
    for (request, change, expected) in [
        ("main a", false, 1),
        ("main b", false, 0),
        ("main c", true, 1),
    ] {
        let id = setup_main_task(&mut w, request, change);
        assert_eq!(
            setup_events(&w, &id, "setup.result")
                .iter()
                .filter(|(_, e)| e["checkout"] == "main" && e["ok"] == true)
                .count(),
            expected
        );
        total += expected;
        let prefix = format!("{} ", w.repo.canonicalize().unwrap().display());
        assert_eq!(
            setup_runs(&w)
                .iter()
                .filter(|line| line.starts_with(&prefix))
                .count(),
            total
        );
        assert!(automed::git::is_clean(&w.repo).unwrap());
    }
}

#[test]
fn setup_failure_in_the_main_checkout_raises_an_alert_and_the_task_is_still_done() {
    needs_git!();
    let mut w = World::new("setup-main-fail");
    let command = setup_fixture(
        &w,
        "main fail",
        "[ \"$AUTOME_SETUP_CHECKOUT\" != main ] || exit 4",
    );
    let id = setup_main_task(&mut w, "main fail", false);
    let alerts = w.ctx.store.open_alerts().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, "setup_failed");
    for text in [&command, "退出码 4", "setup-main.log"] {
        assert!(alerts[0].detail.contains(text));
    }
    assert!(!w.repo.join(".autome/output/setup/main.fp").exists());
    assert!(!w.repo.join(".worktree/main-fail").exists());
    assert!(!automed::git::branch_exists(&w.repo, "autome/main-fail"));
    let result = setup_events(&w, &id, "setup.result");
    assert!(
        result
            .iter()
            .any(|(_, e)| e["checkout"] == "main" && e["exit_code"] == 4)
    );
}

#[test]
fn setup_is_skipped_in_a_workspace_with_one_event_per_task() {
    needs_git!();
    let mut w = Workspace::new("setup-skip", &["docs", "backend"]);
    let config = w.ws().join(".autome/config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap_or_default();
    text.push_str(&format!(
        "\n[setup]\ncommand = \"touch {}\"\n",
        w.root.join("setup-runs").display()
    ));
    std::fs::write(config, text).unwrap();
    let request = "workspace setup";
    let slug = slug_for(request);
    for (n, status, d, k, milestones) in [
        (1, "设计中", 0, 0, vec![]),
        (2, "设计中", 1, 0, vec![]),
        (3, "设计中", 1, 0, vec![]),
        (4, "实现中", 2, 0, vec![("M-01", "开放")]),
        (5, "实现中", 2, 1, vec![("M-01", "待审")]),
        (6, "实现中", 2, 1, vec![("M-01", "已完成")]),
        (7, "已完成", 2, 1, vec![("M-01", "已完成")]),
    ] {
        w.doc_step(
            n,
            &slug,
            if n == 1 {
                ".worktree/workspace-setup"
            } else {
                ""
            },
            &ws_doc(status, d, k, &milestones, "backend"),
            if n == 7 {
                "echo lessons > docs/autome/workspace-setup/lessons.md"
            } else {
                ""
            },
        );
    }
    let out = call(
        &mut w.ctx,
        "task.create",
        json!({"project_id": w.project_id, "request": request}),
    );
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    w.settle();
    ok(&call(&mut w.ctx, "task.approve", json!({"task_id": id})));
    w.settle();
    ok(&call(&mut w.ctx, "task.merge", json!({"task_id": id})));
    w.settle();
    assert!(matches!(
        w.ctx.store.get_task(&id).unwrap().state,
        autome_domain::task::TaskState::Done
    ));
    assert_eq!(w.ctx.store.count_events(&id, "setup.skipped").unwrap(), 1);
    assert_eq!(w.ctx.store.count_events(&id, "setup.started").unwrap(), 0);
    assert!(!w.root.join("setup-runs").exists());
}
#[test]
fn waiting_regression_settle_keeps_future_retry_pending() {
    use autome_domain::task::{Node, TaskState};

    assert!(git_available(), "this regression requires git");
    let mut w = World::new("settle-future-retry");
    // A draft avoids launching a CLI while constructing the post-session
    // state. Exercise settle's real exit predicate without a wall-clock race
    // or waiting for its 90-second hang detector.
    let created = w.call(
        "task.create",
        json!({"project_id": w.project_id, "request": "future retry", "draft": true}),
    );
    let id = ok(&created)["task"]["id"].as_str().unwrap().to_owned();
    w.ctx
        .store
        .set_task_state(
            &id,
            &TaskState::Active {
                node: Node::Adjudicate,
            },
        )
        .unwrap();
    assert!(
        !w.has_pending_work(),
        "control: no session or retry remains"
    );

    w.ctx
        .store
        .schedule_retry(
            &id,
            Node::Adjudicate,
            "2000-01-01T00:00:00Z",
            None,
            "限流",
            false,
        )
        .unwrap();
    assert!(w.has_pending_work(), "control: a due retry is pending work");

    let future = "9999-01-01T00:00:00Z";
    w.ctx
        .store
        .schedule_retry(&id, Node::Adjudicate, future, None, "限流", false)
        .unwrap();
    let report = automed::scheduler::tick(&mut w.ctx);
    assert!(report.errors.is_empty(), "{report:?}");
    assert!(w.ctx.store.all_running_sessions().unwrap().is_empty());
    assert_eq!(w.node(&id).as_deref(), Some("adjudicate"));
    assert_eq!(w.ctx.store.pending_retry(&id).unwrap().unwrap().1, future);
    let pending = w.has_pending_work();

    w.ctx.store.clear_retry(&id).unwrap();
    assert!(
        !w.has_pending_work(),
        "control: clearing the retry makes it quiet"
    );
    assert!(
        pending,
        "settle must not return while an unfinished task has a future pending retry"
    );
}

// This is a source-contract guard, not a load benchmark. Every scheduling
// wait must use the shared hang detector: a local deadline can expire while
// the task is still making progress. Inspect actual deadline declarations,
// not comments or the non-ticking process/exit-marker waits.
#[test]
fn waiting_regression_tick_waits_do_not_own_deadlines() {
    let source = include_str!("end_to_end.rs");
    let cases = [
        "bug_loop_a_legacy_bug_task_already_in_the_design_loop_keeps_running_it",
        "bug_loop_a_workspace_bug_checks_out_every_member_to_locate_then_keeps_only_the_named_repos",
        "bug_regression_guard_checks_each_workspace_member",
        "bug_regression_guard_ignores_the_bug_doc_in_a_snapshot_from_an_older_build",
        "resume_wait",
    ];
    let mut local_deadlines = Vec::new();
    for name in cases {
        let declaration = format!("fn {name}(");
        let body = source.split_once(&declaration).expect("wait must exist").1;
        // These are top-level functions, whose closing brace is unindented.
        let body = body.split_once("\n}").expect("function must end").0;
        if body.contains("let deadline =") {
            local_deadlines.push(name);
        }
    }
    assert!(
        local_deadlines.is_empty(),
        "scheduling waits must share the 90s hang detector instead of owning deadlines: {local_deadlines:?}"
    );
}

// Regression: a successful reviewer process must not admit edits to another task.
const REVIEW_SCOPE_OTHER: &str = "docs/other-task/other-task-bug.md";

fn review_scope_seed(w: &World) {
    std::fs::create_dir_all(w.repo.join("docs/other-task")).unwrap();
    std::fs::write(w.repo.join(REVIEW_SCOPE_OTHER), bug_doc("待复核")).unwrap();
    automed::git::commit_paths(&w.repo, &[REVIEW_SCOPE_OTHER], "seed merged task").unwrap();
}

fn review_scope_assert_failure(w: &World, id: &str, slug: &str) {
    assert_eq!(w.state(id), "failed", "out-of-scope edit was admitted");
    let detail = failure_detail(w, id);
    assert!(detail.contains("只能修改"), "{detail}");
    assert!(detail.contains(&format!("docs/{slug}/")), "{detail}");
    assert!(detail.contains(REVIEW_SCOPE_OTHER), "{detail}");
}

#[test]
fn review_scope_wrong_bug_document_reports_the_offending_path() {
    needs_git!();
    let mut w = World::new("review-scope-wrong-doc");
    review_scope_seed(&w);
    let request = "review scope wrong doc";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, "other-task", &bug_doc("通过"), "");
    let id = create_bug(&mut w, request);
    w.settle();
    review_scope_assert_failure(&w, &id, &slug);
}

#[test]
fn review_scope_valid_verdict_does_not_hide_an_outside_edit() {
    needs_git!();
    let mut w = World::new("review-scope-valid-verdict");
    review_scope_seed(&w);
    let request = "review scope valid verdict";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(
        &w,
        3,
        &slug,
        &bug_doc("通过"),
        &format!("printf 'foreign review\\n' >> {REVIEW_SCOPE_OTHER}"),
    );
    let id = create_bug(&mut w, request);
    w.settle();
    review_scope_assert_failure(&w, &id, &slug);
}

#[test]
fn review_scope_own_document_only_is_allowed() {
    needs_git!();
    let mut w = World::new("review-scope-control");
    review_scope_seed(&w);
    let request = "review scope control";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
}

#[test]
fn review_scope_pending_verdict_names_the_expected_bug_document() {
    needs_git!();
    let mut w = World::new("review-scope-pending");
    let request = "review scope pending";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    w.step(3, "exit 0\n");
    let id = create_bug(&mut w, request);
    w.settle();
    assert_eq!(w.state(&id), "failed");
    let detail = failure_detail(&w, &id);
    assert!(detail.contains("本轮没有更新"), "{detail}");
    assert!(
        detail.contains(&format!("docs/{slug}/{slug}-bug.md")),
        "{detail}"
    );
}

#[test]
fn review_scope_feature_audit_rejects_other_task_edits() {
    review_scope_feature_round(false);
}

#[test]
fn review_scope_feature_retro_rejects_other_task_edits() {
    review_scope_feature_round(true);
}

fn review_scope_feature_round(retro: bool) {
    needs_git!();
    let mut w = World::new("review-scope-feature");
    review_scope_seed(&w);
    let request = "review scope feature";
    let slug = slug_for(request);
    w.doc_step(1, &slug, &doc("设计中", 0, 0, &[]));
    w.doc_step(2, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(3, &slug, &doc("设计中", 1, 0, &[]));
    w.doc_step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    let n = if retro { 7 } else { 6 };
    let script_path = w.repo.join(format!(".autome/fake/{n}.sh"));
    let mut script = std::fs::read_to_string(&script_path).unwrap();
    script.push_str(&format!("\nprintf 'foreign review\\n' >> {REVIEW_SCOPE_OTHER}\ngit add {REVIEW_SCOPE_OTHER}\ngit -c user.name=fake -c user.email=f@f commit -qm 'out of scope review'\n"));
    w.step(n, &script);
    let id = create_task(&mut w, request);
    w.settle();
    ok(&w.call("task.approve", json!({"task_id": id})));
    w.settle();
    review_scope_assert_failure(&w, &id, &slug);
}

fn gate_trace(w: &World, id: &str) -> Vec<String> {
    w.ctx
        .store
        .events_since_with_subject(0, u32::MAX)
        .unwrap()
        .into_iter()
        .filter(|(_, kind, subject, _)| {
            subject == id
                && matches!(
                    kind.as_str(),
                    "gate.started"
                        | "gate.result"
                        | "gate.reused"
                        | "gate.skipped"
                        | "setup.started"
                        | "setup.result"
                        | "setup.skipped"
                        | "merge.auto"
                        | "merge.rebased"
                        | "merge.repo"
                )
        })
        .map(|(_, kind, _, p)| match p["node"].as_str() {
            Some(node) => format!("{kind}:{node}"),
            None => kind,
        })
        .collect()
}

fn advance_main_and_merge(w: &mut World, id: &str) {
    std::fs::write(w.repo.join("main-moved"), "advance\n").unwrap();
    automed::git::commit_paths(&w.repo, &["main-moved"], "advance main").unwrap();
    ok(&w.call("task.merge", json!({"task_id": id})));
    w.settle();
    assert_eq!(w.state(id), "done");
}

fn assert_gate_calls(w: &World, main: usize) {
    let calls = std::fs::read_to_string(w.root.join("gate-calls")).unwrap();
    assert_eq!(calls.lines().count(), 4 + main, "{calls}");
    for (node, count) in [("gate", 2), ("merge_gate", 2), ("main_check", main)] {
        assert_eq!(
            calls.lines().filter(|line| *line == node).count(),
            count,
            "{calls}"
        );
    }
}

fn assert_no_verify_checkout(w: &World) {
    assert!(
        !std::fs::read_dir(w.repo.join(".worktree"))
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("_verify-"))
    );
}

#[test]
fn main_check_reuse_switched_off_matches_today_event_by_event() {
    needs_git!();
    let mut w = World::new("reuse-off");
    let id = run_gated_task_with(&mut w, "reuse disabled", true, Some(false));
    advance_main_and_merge(&mut w, &id);
    assert_eq!(
        gate_trace(&w, &id),
        [
            "gate.started:gate",
            "gate.result:gate",
            "gate.started:gate",
            "gate.result:gate",
            "gate.started:merge_gate",
            "gate.result:merge_gate",
            "merge.auto",
            "merge.rebased",
            "gate.started:merge_gate",
            "gate.result:merge_gate",
            "merge.auto",
            "merge.repo",
            "gate.started:main_check",
            "gate.result:main_check"
        ]
    );
    assert_gate_calls(&w, 1);
    assert_no_verify_checkout(&w);
}

fn merge_after_a_late_branch_commit(w: &mut World, id: &str) {
    automed::scheduler::apply_trigger(&mut w.ctx, id, &autome_domain::task::Trigger::Merge)
        .unwrap();
    assert_eq!(w.node(id).as_deref(), Some("merging"));
    let task = w.ctx.store.get_task(id).unwrap();
    let wt = w.repo.join(format!(".worktree/{}", task.slug));
    std::fs::write(wt.join("late.txt"), "late change\n").unwrap();
    automed::git::commit_paths(&wt, &["late.txt"], "late change").unwrap();
    w.settle();
}

#[test]
fn main_check_reuse_skips_the_run_when_the_merge_commit_has_the_merge_gate_tree() {
    needs_git!();
    let mut w = World::new("reuse-on");
    let id = run_gated_task_with(&mut w, "reuse default", true, None);
    advance_main_and_merge(&mut w, &id);
    let reused = setup_events(&w, &id, "gate.reused");
    let main: Vec<_> = reused
        .iter()
        .filter(|(_, p)| p["node"] == "main_check")
        .collect();
    assert_eq!(main.len(), 1);
    let commit = w.ctx.store.get_task(&id).unwrap().merge_commit.unwrap();
    let passed = w
        .ctx
        .store
        .last_event_in(&id, &["gate.result"], "node", "merge_gate")
        .unwrap()
        .unwrap()
        .2;
    assert_eq!(
        main[0].1,
        json!({"node": "main_check", "sha": commit, "from_sha": passed["sha"], "tree": automed::git::tree_id(&w.repo, &commit).unwrap()})
    );
    assert_eq!(
        passed["sha"],
        automed::git::second_parent(&w.repo, &commit).unwrap()
    );
    let rebased = w
        .ctx
        .store
        .last_event(&id, "merge.rebased")
        .unwrap()
        .unwrap();
    assert_eq!(passed["sha"], rebased["repos"][0]["head"]);
    for kind in [
        "gate.started",
        "gate.result",
        "setup.started",
        "setup.result",
        "setup.skipped",
    ] {
        assert!(
            setup_events(&w, &id, kind)
                .iter()
                .all(|(_, p)| p["node"] != "main_check")
        );
    }
    assert_gate_calls(&w, 0);
    assert_no_verify_checkout(&w);
    assert!(w.ctx.store.open_alerts().unwrap().is_empty());
}

#[test]
fn main_check_reuse_runs_when_the_merge_commit_tree_differs() {
    needs_git!();
    let mut w = World::new("reuse-different");
    let id = run_gated_task_with(&mut w, "late branch", true, None);
    merge_after_a_late_branch_commit(&mut w, &id);
    assert_eq!(w.state(&id), "done");
    let commit = w.ctx.store.get_task(&id).unwrap().merge_commit.unwrap();
    assert!(w.repo.join("late.txt").exists());
    let passed = w
        .ctx
        .store
        .last_event_in(&id, &["gate.result"], "node", "merge_gate")
        .unwrap()
        .unwrap()
        .2;
    assert_ne!(
        passed["sha"],
        automed::git::second_parent(&w.repo, &commit).unwrap()
    );
    assert_ne!(
        automed::git::tree_id(&w.repo, passed["sha"].as_str().unwrap()).unwrap(),
        automed::git::tree_id(&w.repo, &commit).unwrap()
    );
    assert!(
        setup_events(&w, &id, "gate.reused")
            .iter()
            .all(|(_, p)| p["node"] != "main_check")
    );
    for kind in ["gate.started", "gate.result"] {
        let events = setup_events(&w, &id, kind);
        let main: Vec<_> = events
            .iter()
            .filter(|(_, p)| p["node"] == "main_check")
            .collect();
        assert_eq!(main.len(), 1);
        assert_eq!(main[0].1["sha"], commit);
        if kind == "gate.result" {
            assert_eq!(main[0].1["ok"], true);
        }
    }
    let calls = std::fs::read_to_string(w.root.join("gate-calls")).unwrap();
    assert_eq!(calls.lines().filter(|s| *s == "main_check").count(), 1);
    assert_setup_verify_removed(&w, &id);
}

// Queue acceptance uses real sessions and jobs; release files are the only clock.
fn queued_gate_world(tag: &str) -> World {
    let w = World::new(tag);
    w.configure("[loop]\nparallel = 3\n");
    let script = w.root.join("queued-gate.sh");
    std::fs::write(&script, format!(
        "touch '{0}/ready-'\"$AUTOME_TASK\"\nwhile [ -d '{0}' ] && [ ! -f '{0}/release-'\"$AUTOME_TASK\" ]; do sleep 0.02; done\n",
        w.root.display()
    )).unwrap();
    w.configure(&format!(
        "[gate]\ncommand = \"sh {}\"\ntimeout_minutes = 1\n",
        script.display()
    ));
    w
}

fn queue_tick(w: &mut World) {
    let report = automed::scheduler::tick(&mut w.ctx);
    assert!(report.errors.is_empty(), "{report:?}");
    let mut running = 0;
    for task in w.ctx.store.list_unfinished().unwrap() {
        let dir = automed::gate::dir_for(&w.repo, &task.id);
        for entry in std::fs::read_dir(dir).into_iter().flatten() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "started") {
                let job = automed::gate::Job {
                    log: path.with_extension("log"),
                    pid: path.with_extension("pid"),
                    exit: path.with_extension("exit"),
                    started: path,
                };
                running += usize::from(automed::gate::running(&job));
            }
        }
    }
    assert!(running <= 1, "{running} live jobs in one ledger");
}

fn queue_wait(w: &mut World, mut ready: impl FnMut(&World) -> bool) {
    scheduling_wait(
        w,
        |w| {
            queue_tick(w);
            ready(w)
        },
        |w| format!("{:?}", w.task_states()),
    );
}

fn queue_task(w: &mut World, request: &str) -> String {
    let slug = slug_for(request);
    for (n, body) in [
        (1, doc("设计中", 0, 0, &[])),
        (2, doc("设计中", 1, 0, &[])),
        (3, doc("设计中", 1, 0, &[])),
        (4, doc("实现中", 2, 0, &[("M-01", "开放")])),
        (5, doc("实现中", 2, 1, &[("M-01", "待审")])),
        (6, doc("实现中", 2, 1, &[("M-01", "已完成")])),
    ] {
        w.doc_step_for(&slug, n, &body, "");
    }
    let id = create_task(w, request);
    queue_wait(w, |w| {
        w.node(&id).as_deref() == Some("await_design_approval")
    });
    ok(&w.call("task.approve", json!({"task_id": id})));
    queue_wait(w, |w| {
        !setup_events(w, &id, "gate.started").is_empty()
            || !setup_events(w, &id, "gate.queued").is_empty()
    });
    id
}

fn queue_release(w: &World, id: &str) {
    std::fs::write(w.root.join(format!("release-{id}")), "go").unwrap();
}

fn queue_snapshot(w: &mut World, id: &str) -> Value {
    let out = w.call("task.get", json!({"task_id": id}));
    let v = ok(&out);
    json!({"budget": v["task"]["budget_n"], "fix": v["task"]["fix_round"],
        "impl": v["status_block"]["impl_round"], "design": v["status_block"]["design_round"],
        "retry": v["retry"], "usage": v["so_far"], "sessions": v["sessions"].as_array().unwrap().len(),
        "started": setup_events(w,id,"session.started").len(),
        "results": setup_events(w,id,"gate.result"),
        "fix_started": setup_events(w,id,"bug.fix_round_started").len(),
        "attempts": w.ctx.store.retry_attempts(id).unwrap()})
}

fn queue_assert_waiting(w: &mut World, id: &str, position: u64) {
    let out = w.call("task.get", json!({"task_id": id}));
    assert_eq!(ok(&out)["gate_queue_position"], position);
    assert!(ok(&out)["queue_position"].is_null());
    assert!(setup_events(w, id, "gate.started").is_empty());
    assert_eq!(setup_events(w, id, "gate.queued").len(), 1);
    assert_eq!(
        setup_events(w, id, "gate.queued")[0].1["position"],
        position
    );
    assert_eq!(
        std::fs::read_dir(automed::gate::dir_for(&w.repo, id))
            .into_iter()
            .flatten()
            .count(),
        0
    );
}

#[test]
fn gate_queue_second_task_starts_its_gate_only_after_the_first_one_ends() {
    needs_git!();
    let mut w = queued_gate_world("queue-fifo");
    let a = queue_task(&mut w, "queue first");
    let b = queue_task(&mut w, "queue second");
    queue_assert_waiting(&mut w, &b, 1);
    assert_eq!(w.state(&b), "active");
    assert_eq!(w.node(&b).as_deref(), Some("gate"));
    let out = w.call("dashboard.get", json!({}));
    assert!(
        ok(&out)["running"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["task"]["id"] == b && t["gate_queue_position"] == 1)
    );
    let before = queue_snapshot(&mut w, &b);
    assert_eq!(before["impl"], 1);
    assert_eq!(before["design"], 2);
    for _ in 0..3 {
        queue_tick(&mut w);
    }
    assert_eq!(before, queue_snapshot(&mut w, &b));
    for _ in 0..3 {
        queue_tick(&mut w);
    }
    assert_eq!(before, queue_snapshot(&mut w, &b));
    queue_assert_waiting(&mut w, &b, 1);
    queue_release(&w, &a);
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.started").is_empty());
    assert!(setup_events(&w, &a, "gate.result")[0].0 < setup_events(&w, &b, "gate.started")[0].0);
    queue_release(&w, &b);
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.result").is_empty());
    assert_eq!(setup_events(&w, &b, "gate.result")[0].1["exit_code"], 0);
}

#[test]
fn gate_queue_bug_loop_repro_check_waits_without_spending_a_fix_round() {
    needs_git!();
    let mut w = queued_gate_world("queue-bug");
    let a = queue_task(&mut w, "queue regular");
    let slug = slug_for("queue bug");
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    let b = create_bug(&mut w, "queue bug");
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.queued").is_empty());
    assert_eq!(w.node(&b).as_deref(), Some("repro_check"));
    queue_assert_waiting(&mut w, &b, 1);
    let before = queue_snapshot(&mut w, &b);
    assert_eq!(before["fix"], 0);
    for _ in 0..3 {
        queue_tick(&mut w);
    }
    assert_eq!(before, queue_snapshot(&mut w, &b));
    for _ in 0..3 {
        queue_tick(&mut w);
    }
    assert_eq!(before, queue_snapshot(&mut w, &b));
    queue_release(&w, &a);
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.started").is_empty());
    assert!(setup_events(&w, &a, "gate.result")[0].0 < setup_events(&w, &b, "gate.started")[0].0);
}

#[test]
fn gate_queue_time_does_not_count_toward_the_gate_timeout() {
    needs_git!();
    let mut w = queued_gate_world("queue-timeout");
    // A disk-backed test ledger lets us age B's actual queue event, not just
    // A's running clock. All scheduling still uses the production Store.
    let db = w.root.join("timeout-ledger.sqlite");
    w.ctx.store = Store::open(&db).unwrap();
    let added = w.call("project.add", json!({"path": w.repo}));
    w.project_id = ok(&added)["project"]["id"].as_str().unwrap().to_owned();
    ok(&w.call(
        "project.onboarding.skip",
        json!({"project_id": w.project_id}),
    ));
    let a = queue_task(&mut w, "timeout first");
    let b = queue_task(&mut w, "timeout second");
    queue_assert_waiting(&mut w, &b, 1);
    let conn = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(conn.execute(
        "UPDATE events SET at = strftime('%Y-%m-%dT%H:%M:%fZ','now','-120 seconds') WHERE kind = 'gate.queued' AND subject_id = ?1",
        [&b],
    ).unwrap(), 1);
    // Backdate real job clocks, never wait a minute in the suite.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    for (id, age) in [(&a, 61), (&b, 120)] {
        for entry in std::fs::read_dir(automed::gate::dir_for(&w.repo, id))
            .into_iter()
            .flatten()
        {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "started") {
                std::fs::write(path, (now - age).to_string()).unwrap();
            }
        }
    }
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.started").is_empty());
    let result = &setup_events(&w, &a, "gate.result")[0].1;
    assert_eq!(result["exit_code"], 124);
    assert!(result["tail"].as_str().unwrap().contains("已终止"));
    let started = setup_events(&w, &b, "gate.started")[0].1["log"]
        .as_str()
        .unwrap()
        .to_owned();
    let result_at: u64 = conn.query_row(
        "SELECT CAST(strftime('%s',at) AS INTEGER) FROM events WHERE kind = 'gate.result' AND subject_id = ?1 ORDER BY seq LIMIT 1",
        [&a], |row| row.get(0),
    ).unwrap();
    assert!(
        std::fs::read_to_string(PathBuf::from(started).with_extension("started"))
            .unwrap()
            .trim()
            .parse::<u64>()
            .unwrap()
            >= result_at
    );
    queue_release(&w, &b);
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.result").is_empty());
    assert_eq!(setup_events(&w, &b, "gate.result")[0].1["exit_code"], 0);
    let prompt = w.repo.join(format!(
        ".autome/fake/{}/6.prompt",
        slug_for("timeout first")
    ));
    queue_wait(&mut w, |_| {
        std::fs::read_to_string(&prompt).is_ok_and(|s| s.contains("门禁超时"))
    });
}

#[test]
fn gate_queue_counts_per_ledger_not_per_machine() {
    needs_git!();
    let mut a = queued_gate_world("queue-ledger-a");
    let mut b = queued_gate_world("queue-ledger-b");
    let x = queue_task(&mut a, "ledger first");
    let y = queue_task(&mut b, "ledger second");
    for (w, id) in [(&mut a, &x), (&mut b, &y)] {
        assert_eq!(setup_events(w, id, "gate.started").len(), 1);
        assert!(setup_events(w, id, "gate.queued").is_empty());
        queue_release(w, id);
    }
}

#[test]
fn gate_queue_holds_a_setup_command_behind_another_tasks_gate() {
    needs_git!();
    let mut w = queued_gate_world("queue-setup");
    let a = queue_task(&mut w, "setup blocker");
    setup_fixture(&w, "setup waiter", "true");
    let b = create_task(&mut w, "setup waiter");
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.queued").is_empty());
    let queued = &setup_events(&w, &b, "gate.queued")[0].1;
    assert_eq!(queued["job"], "setup");
    assert_eq!(queued["checkout"], "worktree");
    assert!(setup_events(&w, &b, "setup.started").is_empty());
    assert_eq!(w.ctx.store.retry_attempts(&b).unwrap(), 0);
    queue_release(&w, &a);
    queue_wait(&mut w, |w| !setup_events(w, &b, "setup.started").is_empty());
    assert!(setup_events(&w, &a, "gate.result")[0].0 < setup_events(&w, &b, "setup.started")[0].0);
}

#[test]
fn gate_queue_order_survives_a_core_restart() {
    needs_git!();
    let mut w = queued_gate_world("queue-restart");
    let a = queue_task(&mut w, "restart first");
    let b = queue_task(&mut w, "restart second");
    let c = queue_task(&mut w, "restart third");
    queue_assert_waiting(&mut w, &b, 1);
    queue_assert_waiting(&mut w, &c, 2);
    let store = std::mem::replace(&mut w.ctx.store, Store::open_in_memory().unwrap());
    w.ctx = Ctx::new(store, w.root.join("autome-home"), w.root.join("home")).headless();
    automed::scheduler::recover(&mut w.ctx);
    queue_tick(&mut w);
    queue_assert_waiting(&mut w, &b, 1);
    queue_assert_waiting(&mut w, &c, 2);
    assert_eq!(setup_events(&w, &a, "gate.started").len(), 1);
    assert!(setup_events(&w, &a, "gate.result").is_empty());
    queue_release(&w, &a);
    queue_wait(&mut w, |w| !setup_events(w, &b, "gate.started").is_empty());
    assert!(setup_events(&w, &c, "gate.started").is_empty());
    queue_release(&w, &b);
    queue_wait(&mut w, |w| !setup_events(w, &c, "gate.started").is_empty());
    assert!(setup_events(&w, &b, "gate.started")[0].0 < setup_events(&w, &c, "gate.started")[0].0);
    queue_release(&w, &c);
    queue_wait(&mut w, |w| !setup_events(w, &c, "gate.result").is_empty());
}

#[test]
fn prompt_before_merge_feature_repair_round_runs_only_the_failing_part() {
    needs_git!();
    let mut w = World::new("prompt-before-merge-feature");
    let script = write_gate_script(&w);
    let command = format!("sh {}", script.display());
    w.configure(&format!(
        "[gate]\nrun = 'before_merge'\ncommand = '{command}'\n"
    ));
    let request = "prompt feature repair";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    w.doc_step_for_prelude(
        8,
        &slug,
        &doc("实现中", 2, 2, &[("M-01", "已完成")]),
        "echo ok > gate-ok\ngit add gate-ok",
    );
    w.doc_step(9, &slug, &doc("实现中", 2, 2, &[("M-01", "已完成")]));
    w.retro_step(10, &slug);
    let id = resume_setup(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let prompt = w.prompt(8);
    assert!(prompt.contains("gate says no: gate-ok is missing"));
    assert!(prompt.contains(&automed::launcher::gate_line(&command)));
    assert!(prompt.contains("单独复现并修好它"));
    assert!(!prompt.contains("在同一命令下确认通过"));
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    assert!(events.iter().any(|(_, k, s, p)| s == &id
        && k == "gate.result"
        && p["node"] == "merge_gate"
        && p["ok"] == false));
}

#[test]
fn prompt_before_merge_bug_repair_round_runs_only_the_failing_part() {
    needs_git!();
    let mut w = World::new("prompt-before-merge-bug");
    let command = "echo project-gate-output; test -f gate-ready";
    w.configure(&format!(
        "[gate]\nrun = 'before_merge'\ncommand = '{command}'\n"
    ));
    let request = "prompt bug repair";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    bug_step(&w, 4, &slug, &bug_doc("待复核"), "touch gate-ready");
    bug_step(&w, 5, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let prompt = w.prompt(4);
    assert!(prompt.contains("project-gate-output"));
    assert!(prompt.contains(&automed::launcher::gate_line(command)));
    assert!(prompt.contains("复现命令和与改动直接相关的测试子集"));
    assert!(!prompt.contains("用同一命令确认通过"));
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    assert!(events.iter().any(|(_, k, s, p)| s == &id
        && k == "gate.result"
        && p["node"] == "merge_gate"
        && p["ok"] == false));
}

#[test]
fn protocol_before_merge_feature_rounds_are_told_the_host_runs_the_full_suite() {
    needs_git!();
    let mut w = World::new("protocol-before-merge-feature");
    w.configure("[gate]\nrun = 'before_merge'\ncommand = 'true'\n");
    let request = "protocol feature prompts";
    let slug = slug_for(request);
    w.doc_step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    w.doc_step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    w.retro_step(7, &slug);
    let id = resume_setup(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    for n in [2, 5, 6] {
        assert!(w.prompt(n).contains(&automed::launcher::gate_line("true")));
    }
    assert!(
        w.prompt(2)
            .contains("不要把跑全量门禁写成任何里程碑的验收命令")
    );
    assert!(w.prompt(6).contains("不要因为没跑全量而不给结论"));
    assert!(!w.prompt(6).contains("项目测试体系全量跑一次即可"));
}

#[test]
fn protocol_before_merge_bug_rounds_are_told_the_host_runs_the_full_suite() {
    needs_git!();
    let mut w = World::new("protocol-before-merge-bug");
    w.configure("[gate]\nrun = 'before_merge'\ncommand = 'true'\n");
    let request = "protocol bug prompts";
    let slug = slug_for(request);
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    for n in [2, 3] {
        assert!(w.prompt(n).contains(&automed::launcher::gate_line("true")));
    }
    assert!(w.prompt(2).contains("再跑与本轮改动直接相关的测试子集"));
    assert!(!w.prompt(2).contains("再运行配置的项目门禁"));
    assert!(
        w.prompt(3)
            .contains("本项目门禁只在合并前运行，本轮没有宿主门禁结果")
    );
    assert!(!w.prompt(3).contains("独立复验复现命令与项目门禁"));
}

#[test]
fn bug_loop_not_a_bug_task_convert_rebuilds_it_as_a_feature_with_its_attachments() {
    needs_git!();
    let mut w = World::new("convert-attachment");
    let slug = slug_for("convert attachment");
    bug_step(
        &w,
        1,
        &slug,
        &bug_doc("待复核").replace("已定位", "不是 Bug"),
        "",
    );
    let attachment = w.repo.parent().unwrap().join("attachment.txt");
    std::fs::write(&attachment, "附件内容\n").unwrap();
    let brief = "# 原始卡片\n保留 {request}  \n";
    let out=w.call("task.create",json!({"project_id":w.project_id,"request":"convert attachment","kind":"bug","brief":brief,"attachments":[attachment]}));
    let id = ok(&out)["task"]["id"].as_str().unwrap().to_owned();
    assert_eq!(w.node(&id).as_deref(), Some("locate"));
    assert!(matches!(
        w.call("task.convert", json!({"task_id":id,"kind":"feature"}))
            .reply
            .outcome,
        ReplyOutcome::Error { .. }
    ));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("not_a_bug"));
    let out = w.call(
        "task.convert",
        json!({"task_id":id,"kind":"feature","start":true}),
    );
    let new_id = ok(&out)["task"]["id"].as_str().unwrap();
    let new = w.ctx.store.get_task(new_id).unwrap();
    assert_eq!(new.kind, autome_domain::task::TaskKind::Feature);
    let get = w.call("task.get", json!({"task_id":new_id}));
    assert_eq!(ok(&get)["brief"], brief);
    assert!(
        new.attachments
            .iter()
            .all(|p| std::path::Path::new(p).is_file())
    );
    let copy = w
        .repo
        .join(".worktree")
        .join(&new.slug)
        .join(new.doc_dir())
        .join("attachments/attachment.txt");
    assert_eq!(
        std::fs::read(copy).unwrap(),
        std::fs::read(attachment).unwrap()
    );
    assert_eq!(w.state(&id), "cancelled");
    assert!(w.ctx.store.get_task(&id).unwrap().is_archived());
    assert!(!w.repo.join(".worktree").join(slug).exists());
    assert!(matches!(
        w.call("task.convert", json!({"task_id":id,"kind":"feature"}))
            .reply
            .outcome,
        ReplyOutcome::Error { .. }
    ));
}

fn configure_merge_after(w: &World, exit_code: i32) {
    let script = w.root.join("hook.sh");
    std::fs::write(
        &script,
        format!(
            r#"wt=absent
if [ -d ".worktree/$(cat '{root}/hook-slug')" ]; then wt=present; fi
printf '%s|%s|%s|wt=%s\n' "$PWD" "$AUTOME_TASK" "$AUTOME_MERGE_COMMIT" "$wt" >> '{root}/hook-runs'
mkdir -p hook-out
echo stamp > hook-out/stamp
echo hook-output
exit {exit_code}
"#,
            root = w.root.display()
        ),
    )
    .unwrap();
    let ignore = w.repo.join(".gitignore");
    let text = std::fs::read_to_string(&ignore).unwrap();
    std::fs::write(ignore, format!("{text}\nhook-out/\n")).unwrap();
    automed::git::commit_paths(&w.repo, &[".gitignore"], "ignore hook output").unwrap();
    w.configure(&format!("[merge]\nafter = 'sh {}'\n", script.display()));
}

#[test]
fn merge_after_runs_in_the_main_checkout_after_the_main_check_and_before_cleanup() {
    needs_git!();
    let mut w = World::new("merge-after-ok");
    configure_merge_after(&w, 0);
    // This case verifies a fresh MainCheck, not reuse of the pre-merge verdict.
    let id = run_gated_task_with(&mut w, "hook success", false, Some(false));
    let task = w.ctx.store.get_task(&id).unwrap();
    std::fs::write(w.root.join("hook-slug"), &task.slug).unwrap();
    w.call("task.merge", json!({"task_id": id}));
    w.settle();
    assert_eq!(w.state(&id), "done");
    let task = w.ctx.store.get_task(&id).unwrap();
    assert_eq!(
        std::fs::read_to_string(w.root.join("hook-runs")).unwrap(),
        format!(
            "{}|{}|{}|wt=present\n",
            w.repo.canonicalize().unwrap().display(),
            id,
            task.merge_commit.unwrap()
        )
    );
    assert!(w.repo.join("hook-out/stamp").exists());
    let events = w.ctx.store.events_since_with_subject(0, u32::MAX).unwrap();
    let seq = |kind: &str| {
        events
            .iter()
            .find(|(_, k, s, _)| k == kind && s == &id)
            .unwrap()
            .0
    };
    let main = events
        .iter()
        .find(|(_, k, s, p)| {
            k == "gate.result" && s == &id && p["node"] == "main_check" && p["ok"] == true
        })
        .unwrap()
        .0;
    let done = events
        .iter()
        .find(|(_, k, s, p)| k == "task.updated" && s == &id && p["state"]["state"] == "done")
        .unwrap()
        .0;
    assert!(main < seq("merge.after_started"));
    assert!(seq("merge.after_started") < seq("merge.after"));
    assert!(seq("merge.after") < done);
    let result = w.ctx.store.last_event(&id, "merge.after").unwrap().unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(result["exit_code"], 0);
    assert!(std::path::Path::new(result["log"].as_str().unwrap()).exists());
    assert!(result["tail"].as_str().unwrap().contains("hook-output"));
    assert!(automed::git::is_clean(&w.repo).unwrap());
    assert!(w.ctx.store.open_alerts().unwrap().is_empty());
}

#[test]
fn merge_after_failure_raises_one_alert_and_the_task_is_still_done() {
    needs_git!();
    let mut w = World::new("merge-after-fail");
    configure_merge_after(&w, 6);
    let id = run_gated_task(&mut w, "hook failure");
    let task = w.ctx.store.get_task(&id).unwrap();
    std::fs::write(w.root.join("hook-slug"), &task.slug).unwrap();
    w.call("task.merge", json!({"task_id": id}));
    w.settle();
    assert_eq!(w.state(&id), "done");
    assert!(!w.repo.join(".worktree").join(&task.slug).exists());
    assert!(
        !std::process::Command::new("git")
            .args([
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{}", task.branch())
            ])
            .current_dir(&w.repo)
            .status()
            .unwrap()
            .success()
    );
    let alerts = w.ctx.store.open_alerts().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, "after_merge_failed");
    for text in ["hook.sh", "退出码 6", "after_merge-"] {
        assert!(alerts[0].detail.contains(text));
    }
    assert_eq!(w.ctx.store.count_events(&id, "merge.after").unwrap(), 1);
    let result = w.ctx.store.last_event(&id, "merge.after").unwrap().unwrap();
    assert_eq!(result["ok"], false);
    assert_eq!(result["exit_code"], 6);
}

#[test]
fn merge_after_events_are_absent_when_no_merge_after_is_configured() {
    needs_git!();
    for blank in [false, true] {
        let mut w = World::new("merge-after-none");
        if blank {
            w.configure("[merge]\nafter = '  '\n");
        }
        let id = run_gated_task(&mut w, "no hook");
        w.call("task.merge", json!({"task_id": id}));
        w.settle();
        assert_eq!(w.state(&id), "done");
        for kind in ["merge.after_started", "merge.after", "merge.after_skipped"] {
            assert_eq!(w.ctx.store.count_events(&id, kind).unwrap(), 0);
        }
        assert!(w.ctx.store.open_alerts().unwrap().is_empty());
        let dir = automed::gate::dir_for(&w.repo, &id);
        assert!(!std::fs::read_dir(dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("after_merge")
        }));
    }
}

#[test]
fn merge_after_is_skipped_when_the_main_check_failed() {
    needs_git!();
    let mut w = World::new("hook-main-fail");
    configure_merge_after(&w, 0);
    // This case verifies a fresh MainCheck, not reuse of the pre-merge verdict.
    let id = run_gated_task_with(&mut w, "hook main fail", false, Some(false));
    std::fs::write(w.root.join("main-should-fail"), "").unwrap();
    ok(&w.call("task.merge", json!({"task_id": id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    let alerts = w.ctx.store.open_alerts().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].kind, "main_gate_failed");
    let skipped = setup_events(&w, &id, "merge.after_skipped");
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].1["reason"], "验证 main 未通过");
    assert!(setup_events(&w, &id, "merge.after_started").is_empty());
    assert!(!w.root.join("hook-runs").exists());
}

#[test]
fn merge_after_runs_after_the_main_checkout_setup_and_even_when_it_failed() {
    needs_git!();
    for fail in [false, true] {
        let mut w = World::new("hook-setup");
        configure_merge_after(&w, 0);
        let body = if fail {
            format!(
                "if [ \"$PWD\" = '{}' ]; then exit 4; fi",
                w.repo.canonicalize().unwrap().display()
            )
        } else {
            String::new()
        };
        let (id, _) = setup_gate_task(&mut w, "hook setup", &body, false);
        std::fs::write(w.root.join("hook.sh"), format!(
            "sleep 1\nstamp=no; [ ! -f deps/stamp ] || stamp=yes\necho stamp=$stamp >> '{}/hook-runs'\n", w.root.display()
        )).unwrap();
        ok(&w.call("task.merge", json!({"task_id": id})));
        w.settle();
        assert_eq!(w.state(&id), "done");
        let main = setup_events(&w, &id, "setup.result")
            .into_iter()
            .filter(|(_, e)| e["checkout"] == "main")
            .collect::<Vec<_>>();
        assert_eq!(main.len(), 1);
        assert_eq!(main[0].1["ok"], !fail);
        assert!(main[0].0 < setup_events(&w, &id, "merge.after_started")[0].0);
        assert_eq!(
            std::fs::read_to_string(w.root.join("hook-runs")).unwrap(),
            if fail { "stamp=no\n" } else { "stamp=yes\n" }
        );
        let results = setup_events(&w, &id, "merge.after");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1["ok"], true);
        let main_path = w.repo.canonicalize().unwrap();
        assert_eq!(
            setup_runs(&w)
                .iter()
                .filter(|line| line.starts_with(&format!("{} ", main_path.display())))
                .count(),
            1
        );
        let alerts = w.ctx.store.open_alerts().unwrap();
        assert_eq!(alerts.len(), usize::from(fail));
        if fail {
            assert_eq!(alerts[0].kind, "setup_failed");
        }
    }
}

#[test]
fn merge_after_is_skipped_in_a_workspace_with_one_event_per_task() {
    needs_git!();
    let mut w = Workspace::new("hook-workspace", &["docs", "backend"]);
    let config = w.ws().join(".autome/config.toml");
    let text = std::fs::read_to_string(&config).unwrap_or_default();
    std::fs::write(
        config,
        format!(
            "{text}\n[merge]\nafter = 'touch {}/hook-runs'\n",
            w.root.display()
        ),
    )
    .unwrap();
    let (id, _) = workspace_waiting_to_merge(&mut w);
    ok(&call(&mut w.ctx, "task.merge", json!({"task_id": id})));
    w.settle();
    assert_eq!(
        w.ctx.store.get_task(&id).unwrap().state,
        autome_domain::task::TaskState::Done
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "merge.after_skipped")
            .unwrap(),
        1
    );
    assert_eq!(
        w.ctx
            .store
            .last_event(&id, "merge.after_skipped")
            .unwrap()
            .unwrap()["reason"],
        "工作区项目不支持合并后命令"
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "merge.after_started")
            .unwrap(),
        0
    );
    assert!(!w.root.join("hook-runs").exists());
}

#[test]
fn merge_after_runs_for_a_bug_task_too() {
    needs_git!();
    let mut w = World::new("hook-bug");
    configure_merge_after(&w, 0);
    let slug = slug_for("hook bug");
    std::fs::write(w.root.join("hook-slug"), &slug).unwrap();
    bug_step(&w, 1, &slug, &bug_doc("待复核"), BUG_LOCATE);
    bug_step(&w, 2, &slug, &bug_doc("待复核"), BUG_FIX);
    bug_step(&w, 3, &slug, &bug_doc("通过"), "");
    let id = create_bug(&mut w, "hook bug");
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    ok(&w.call("task.merge", json!({"task_id": id})));
    w.settle();
    assert_eq!(w.state(&id), "done");
    let result = setup_events(&w, &id, "merge.after");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1["ok"], true);
    let runs = std::fs::read_to_string(w.root.join("hook-runs")).unwrap();
    assert_eq!(runs.lines().count(), 1);
    assert!(runs.contains(&format!("|{id}|")));
}

#[test]
fn merge_after_stopped_by_the_user_is_killed_and_runs_again_on_resume() {
    needs_git!();
    // Exercise all user lifecycle actions through the real dispatcher.
    for action in ["stop", "pause", "cancel"] {
        let mut w = World::new("hook-lifecycle");
        configure_merge_after(&w, 0);
        let id = run_gated_task(&mut w, "hook lifecycle");
        std::fs::write(w.root.join("slow"), "").unwrap();
        std::fs::write(w.root.join("hook.sh"), format!(
            "echo $$ > '{root}/hook-pid'\nwhile [ -f '{root}/slow' ]; do sleep 0.05; done\necho done\n", root = w.root.display()
        )).unwrap();
        ok(&w.call("task.merge", json!({"task_id": id})));
        resume_wait(&mut w, |w| {
            std::fs::read_to_string(w.root.join("hook-pid"))
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .is_some()
        });
        let pid: i32 = std::fs::read_to_string(w.root.join("hook-pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_eq!(setup_events(&w, &id, "merge.after_started").len(), 1);
        ok(&w.call(&format!("task.{action}"), json!({"task_id": id})));
        let dir = automed::gate::dir_for(&w.repo, &id);
        if action == "pause" {
            assert!(automed::launcher::pid_alive(pid));
            assert_eq!(w.state(&id), "paused");
        } else {
            resume_wait(&mut w, |_| !automed::launcher::pid_alive(pid));
            assert!(std::fs::read_dir(&dir).unwrap().all(|e| {
                !e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("after_merge-")
            }));
            assert_eq!(
                w.state(&id),
                if action == "stop" {
                    "stopped"
                } else {
                    "cancelled"
                }
            );
        }
        assert!(dir.join("after_merge.prepared").exists());
        assert!(setup_events(&w, &id, "merge.after").is_empty());
        std::fs::remove_file(w.root.join("slow")).unwrap();
        if action != "cancel" {
            ok(&w.call("task.resume", json!({"task_id": id})));
            w.settle();
            assert_eq!(w.state(&id), "done");
            assert_eq!(
                setup_events(&w, &id, "merge.after_started").len(),
                if action == "stop" { 2 } else { 1 }
            );
            let results = setup_events(&w, &id, "merge.after");
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].1["ok"], true);
        }
        assert!(w.ctx.store.open_alerts().unwrap().is_empty());
    }
}

#[test]
fn cost_rollup_task_get_sums_two_sessions_and_counts_missing_cost() {
    needs_git!();
    let mut w = World::new("cost-rollup");
    let request = "show task costs";
    let slug = slug_for(request);
    w.doc_step_with_cost(1, &slug, &doc("设计中", 1, 0, &[]), 1.25);
    w.doc_step_with_cost(2, &slug, &doc("设计中", 1, 0, &[]), 99.0);
    w.doc_step_with_cost(3, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]), 2.5);
    let created = w.call(
        "task.create",
        json!({
            "project_id": w.project_id, "request": request,
            "brief": "# Task\nShow cumulative reported costs.", "draft": true
        }),
    );
    let task_id = ok(&created)["task"]["id"].as_str().unwrap().to_owned();
    // Start at design so the measured scenario consists of precisely the
    // plan/review/adjudicate sessions, without an unrelated intake session.
    w.ctx
        .store
        .set_task_state(
            &task_id,
            &autome_domain::task::TaskState::Requeued {
                resume: autome_domain::task::Node::Design,
            },
        )
        .unwrap();
    w.settle();
    assert_eq!(w.node(&task_id).as_deref(), Some("await_design_approval"));
    let sessions = w.ctx.store.list_sessions(&task_id).unwrap();
    assert_eq!(sessions.len(), 3);
    assert_eq!(
        sessions
            .iter()
            .map(|s| (s.kind.cost_key(), s.metrics.cost_usd))
            .collect::<Vec<_>>(),
        vec![
            ("adjudicate", Some(2.5)),
            ("review", None),
            ("plan", Some(1.25))
        ]
    );
    let response = w.call("task.get", json!({"task_id": task_id}));
    let cost = &ok(&response)["cost"];
    assert_eq!(cost["usd_total"], 3.75);
    assert_eq!(cost["sessions"], 3);
    assert_eq!(cost["sessions_without_cost"], 1);
    assert_eq!(cost["by_role"]["plan"]["usd_total"], 1.25);
    assert_eq!(
        cost["by_role"]["review"],
        json!({"usd_total": 0.0, "sessions": 1, "sessions_without_cost": 1})
    );
    assert_eq!(
        cost["usd_total"].as_f64().unwrap(),
        sessions
            .iter()
            .filter_map(|s| s.metrics.cost_usd)
            .sum::<f64>()
    );
}

fn run_budget_alert_task(configured: bool) {
    let mut w = World::new(if configured { "budget" } else { "no-budget" });
    if configured {
        w.configure("[budget]\nper_task_usd = 0.4\n");
    }
    let request = "budget alert coverage";
    let slug = slug_for(request);
    for (n, body) in [
        (1, doc("设计中", 0, 0, &[])),
        (2, doc("设计中", 1, 0, &[])),
        (3, doc("设计中", 1, 0, &[])),
        (4, doc("实现中", 2, 0, &[("M-01", "开放")])),
        (5, doc("实现中", 2, 1, &[("M-01", "待审")])),
        (6, doc("实现中", 2, 1, &[("M-01", "已完成")])),
    ] {
        w.doc_step_with_usage(n, &slug, &body);
    }
    w.retro_step(7, &slug);
    let id = create_task(&mut w, request);
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_design_approval"));
    let before = w.ctx.store.list_sessions(&id).unwrap();
    let before_ids: Vec<_> = before.iter().map(|s| s.id.clone()).collect();
    assert!(
        before
            .iter()
            .filter_map(|s| s.metrics.cost_usd)
            .sum::<f64>()
            >= 0.5
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "task.budget_exceeded")
            .unwrap(),
        u32::from(configured)
    );
    if configured {
        let alerts = w.ctx.store.open_alerts().unwrap();
        assert_eq!(
            alerts
                .iter()
                .filter(|a| a.kind == "budget_exceeded")
                .count(),
            1
        );
        let alert = alerts.iter().find(|a| a.kind == "budget_exceeded").unwrap();
        assert_eq!(alert.task_id.as_deref(), Some(id.as_str()));
        assert!(alert.detail.contains("任务照常继续"));
        w.ctx.store.dismiss_alert(&alert.id).unwrap();
    }
    w.call("task.approve", json!({"task_id": id}));
    w.settle();
    assert_eq!(w.node(&id).as_deref(), Some("await_merge"));
    let after = w.ctx.store.list_sessions(&id).unwrap();
    assert!(
        after
            .iter()
            .any(|s| !before_ids.contains(&s.id) && s.metrics.cost_usd.is_some_and(|v| v > 0.0))
    );
    assert_eq!(
        w.ctx
            .store
            .count_events(&id, "task.budget_exceeded")
            .unwrap(),
        u32::from(configured)
    );
    assert!(
        w.ctx
            .store
            .open_alerts()
            .unwrap()
            .iter()
            .all(|a| a.kind != "budget_exceeded")
    );
    let events = w.ctx.store.events_since(0, 1000).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|(_, kind, p)| kind == "project.alert"
                && p["kind"] == "budget_exceeded"
                && p["task_id"] == id)
            .count(),
        usize::from(configured)
    );
}

#[test]
fn budget_alert_fires_once_and_the_task_keeps_going() {
    needs_git!();
    run_budget_alert_task(true);
}

#[test]
fn budget_alert_without_a_budget_section_raises_nothing() {
    needs_git!();
    run_budget_alert_task(false);
}
