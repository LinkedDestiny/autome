//! Tests against the *real* CLIs. Technical design §16, gate T5.
//!
//! The end-to-end suite substitutes a stand-in for the model, which is right
//! for CI: it is deterministic, free, and it exercises every seam that does not
//! involve inference. But it cannot catch the class of bug that actually bit
//! this project — an adapter flag that does not exist, or one that exists and
//! means something other than what we assumed. `claude` without `-p` starts an
//! interactive session and never exits; no amount of stand-in testing would
//! have shown that.
//!
//! So these tests run the real binaries. They are opt-in because they cost
//! money and need a logged-in account:
//!
//! ```sh
//! AUTOMED_REAL_CLI=1 cargo test --test real_cli -- --nocapture
//! ```
//!
//! They are deliberately small. The point is to prove the *mechanism* — the
//! wrapper runs the CLI, the CLI exits, the marker appears, the document
//! parses, the transition fires — not to evaluate a model's work.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use automed::dispatch::{Ctx, Outcome, handle_command};
use automed::ipc::{Command, PROTOCOL_VERSION, ReplyOutcome};
use automed::store::Store;
use serde_json::{Value, json};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Opt-in guard. Without it these tests report as skipped rather than failing
/// on a machine with no account.
fn enabled() -> bool {
    std::env::var("AUTOMED_REAL_CLI").as_deref() == Ok("1")
}

macro_rules! needs_real_cli {
    () => {
        if !enabled() {
            eprintln!("skipping: set AUTOMED_REAL_CLI=1 to run against the real CLIs");
            return;
        }
    };
}

fn have(binary: &str) -> bool {
    automed::launcher::which(binary).is_some()
}

struct World {
    root: PathBuf,
    repo: PathBuf,
    ctx: Ctx,
    project_id: String,
}

impl World {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("automed-real-{tag}-{}-{n}", std::process::id()));
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

        automed::git::init(&repo, "main").unwrap();
        std::fs::write(
            repo.join("README.md"),
            "# 示例项目\n\n这是一个用于测试的最小仓库。\n",
        )
        .unwrap();
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

        World {
            root,
            repo,
            ctx,
            project_id,
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Outcome {
        call(&mut self.ctx, method, params)
    }

    /// Ticks until no session is running, or the deadline passes. Real model
    /// calls take tens of seconds, so the budget is generous.
    fn settle(&mut self, secs: u64) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
        loop {
            automed::scheduler::tick(&mut self.ctx);
            let running = self
                .ctx
                .store
                .all_running_sessions()
                .unwrap_or_default()
                .len();
            let queued = self
                .ctx
                .store
                .queued_tasks(&self.project_id)
                .unwrap_or_default()
                .len();
            if running == 0 && queued == 0 {
                return;
            }
            if std::time::Instant::now() >= deadline {
                eprintln!("did not settle within {secs}s; {running} running, {queued} queued");
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    }

    fn node(&self, task_id: &str) -> Option<String> {
        let task = self.ctx.store.get_task(task_id).unwrap();
        serde_json::to_value(&task.state)
            .unwrap()
            .get("node")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    fn state(&self, task_id: &str) -> Value {
        serde_json::to_value(self.ctx.store.get_task(task_id).unwrap().state).unwrap()
    }

    /// Everything the session wrote, for a legible failure.
    fn dump(&self, task_id: &str) -> String {
        let mut out = String::new();
        for s in self.ctx.store.list_sessions(task_id).unwrap_or_default() {
            out.push_str(&format!(
                "\n--- session {} ({:?}, {}) lifecycle={:?}\n",
                s.id, s.kind, s.model, s.lifecycle
            ));
            let log = std::fs::read_to_string(&s.log_path).unwrap_or_default();
            let tail: Vec<&str> = log.lines().rev().take(25).collect();
            for line in tail.into_iter().rev() {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
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

// ---------------------------------------------------------------------------
// The adapter table, checked against the binaries themselves
// ---------------------------------------------------------------------------

/// The cheapest test that would have caught the `-p` bug: ask each CLI for its
/// own help and confirm every flag we pass is one it documents.
///
/// This costs nothing and needs no account, so it runs whenever the binary is
/// present rather than behind the opt-in guard.
#[test]
fn every_adapter_flag_exists_in_the_cli_it_is_passed_to() {
    for adapter in automed::launcher::ADAPTERS.iter() {
        if !have(adapter.binary) {
            eprintln!("skipping {}: not installed", adapter.binary);
            continue;
        }
        // `codex exec` documents its flags on the subcommand, not the root.
        let subcommand = adapter
            .autonomous_flags
            .first()
            .filter(|f| !f.starts_with('-'));
        let mut cmd = std::process::Command::new(adapter.binary);
        if let Some(sub) = subcommand {
            cmd.arg(sub);
        }
        let help = cmd
            .arg("--help")
            .output()
            .unwrap_or_else(|e| panic!("{} --help: {e}", adapter.binary));
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&help.stdout),
            String::from_utf8_lossy(&help.stderr)
        );

        let mut expected: Vec<&str> = adapter
            .autonomous_flags
            .iter()
            .copied()
            .filter(|f| f.starts_with('-'))
            .collect();
        expected.push(adapter.model_flag);
        if let Some(effort) = adapter.effort_flag {
            expected.push(effort);
        }
        for flag in expected {
            assert!(
                text.contains(flag),
                "{} does not document `{flag}`; the adapter table is wrong",
                adapter.binary
            );
        }
    }
}

/// The specific property the `-p` bug violated: the invocation must *finish*.
/// An interactive session would hang here rather than fail, which is why the
/// timeout is the assertion.
#[test]
fn each_cli_exits_on_its_own_when_invoked_the_way_the_launcher_invokes_it() {
    needs_real_cli!();
    for adapter in automed::launcher::ADAPTERS.iter() {
        if !have(adapter.binary) {
            eprintln!("skipping {}: not installed", adapter.binary);
            continue;
        }
        let config = autome_domain::config::RoleConfig {
            enabled: true,
            runtime: adapter.runtime,
            model: String::new(),
            effort: None,
            skills: vec![],
        };
        let args = automed::launcher::build_args(&config, std::path::Path::new("/nonexistent-so-git-cannot-answer"));

        let dir = std::env::temp_dir().join(format!(
            "automed-exit-{}-{}",
            adapter.binary,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        automed::git::init(&dir, "main").unwrap();

        let started = std::time::Instant::now();
        let mut child = std::process::Command::new(adapter.binary)
            .args(&args)
            .current_dir(&dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"Reply with the two letters OK and nothing else. Do not use any tools.")
            .unwrap();
        drop(child.stdin.take());

        // Poll rather than `wait()`, so an interactive session is reported as
        // a hang instead of blocking the suite forever.
        let deadline = started + std::time::Duration::from_secs(120);
        let status = loop {
            match child.try_wait().unwrap() {
                Some(s) => break Some(s),
                None if std::time::Instant::now() >= deadline => {
                    let _ = child.kill();
                    break None;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(200)),
            }
        };
        let _ = std::fs::remove_dir_all(&dir);

        assert!(
            status.is_some(),
            "{} {:?} did not exit within 120s — it is probably interactive, which means \
             the wrapper would never write its marker and the task would hang forever",
            adapter.binary,
            args
        );
        eprintln!(
            "{} exited in {:?} with {:?}",
            adapter.binary,
            started.elapsed(),
            status.unwrap().code()
        );
    }
}

// ---------------------------------------------------------------------------
// One real session through the production path
// ---------------------------------------------------------------------------

/// A real intake session: the launcher writes the prompt, the wrapper runs the
/// real CLI, the CLI writes a design document and exits, the wrapper writes the
/// marker, the scheduler notices and advances the task.
///
/// Everything here is the production path except the terminal hop.
#[test]
fn a_real_intake_session_produces_a_parseable_document_and_advances_the_task() {
    needs_real_cli!();
    if !have("claude") {
        eprintln!("skipping: claude is not installed");
        return;
    }
    let mut w = World::new("intake");

    // Keep it to one round: the point is the mechanism, not the model's work.
    w.call(
        "config.set_role",
        json!({ "project_id": w.project_id.clone(), "role": "plan", "model": "sonnet" }),
    );

    let created = w.call(
        "task.create",
        json!({
            "project_id": w.project_id.clone(),
            "request": "在 README.md 末尾加一行「hello from autome」。只改这一个文件。"
        }),
    );
    let task_id = ok(&created)["task"]["id"].as_str().unwrap().to_string();

    w.settle(300);

    let sessions = w.ctx.store.list_sessions(&task_id).unwrap();
    assert!(!sessions.is_empty(), "a session must have been launched");

    let intake = sessions
        .iter()
        .find(|s| s.kind == autome_domain::session::SessionKind::Intake)
        .expect("the intake session is recorded");
    assert!(
        !intake.is_running(),
        "the intake session must have finished; lifecycle {:?}{}",
        intake.lifecycle,
        w.dump(&task_id)
    );
    assert_eq!(
        intake.lifecycle,
        autome_domain::session::SessionLifecycle::Exited { exit_code: 0 },
        "the CLI must exit cleanly{}",
        w.dump(&task_id)
    );

    // The document it wrote must parse. This is where an agent that ignored
    // the status-block format would be caught.
    let task = w.ctx.store.get_task(&task_id).unwrap();
    let doc_path = w
        .repo
        .join(".worktree")
        .join(&task.slug)
        .join(task.design_doc());
    let text = std::fs::read_to_string(&doc_path).unwrap_or_else(|e| {
        panic!(
            "no design document at {}: {e}{}",
            doc_path.display(),
            w.dump(&task_id)
        )
    });
    autome_domain::status_block::parse(&text)
        .unwrap_or_else(|e| panic!("the design document does not parse: {e}\n\n{text}"));

    // And the task moved off intake, which is the transition firing.
    assert_ne!(
        w.node(&task_id).as_deref(),
        Some("intake"),
        "the task should have advanced; state {}{}",
        w.state(&task_id),
        w.dump(&task_id)
    );
    eprintln!("task is now at {:?}", w.node(&task_id));
}

/// The whole loop, with real models, to a real merge commit.
///
/// This is the test the project exists to pass. It is slow (six or more model
/// calls) and it costs money, so it sits behind a second opt-in on top of
/// `AUTOMED_REAL_CLI`:
///
/// ```sh
/// AUTOMED_REAL_CLI=1 AUTOMED_REAL_LOOP=1 \
///   cargo test --test real_cli the_whole_loop -- --nocapture --test-threads=1
/// ```
///
/// Generation runs on Claude and evaluation on Codex, which is both the
/// shipped default and the only way to satisfy SAME-MODEL without naming two
/// models on one runtime.
#[test]
fn the_whole_loop_reaches_a_merge_commit_with_real_models() {
    needs_real_cli!();
    if std::env::var("AUTOMED_REAL_LOOP").as_deref() != Ok("1") {
        eprintln!("skipping: set AUTOMED_REAL_LOOP=1 as well (this one costs money)");
        return;
    }
    if !have("claude") || !have("codex") {
        eprintln!("skipping: needs both CLIs");
        return;
    }
    let mut w = World::new("loop");

    // Cheap models on both sides; the task is trivial and the point is the
    // mechanism.
    w.call(
        "config.set_role",
        json!({ "project_id": w.project_id.clone(), "role": "plan", "model": "sonnet" }),
    );
    w.call(
        "config.set_role",
        json!({ "project_id": w.project_id.clone(), "role": "adjudicate", "model": "sonnet" }),
    );
    w.call(
        "config.set_role",
        json!({ "project_id": w.project_id.clone(), "role": "impl", "model": "sonnet" }),
    );

    let created = w.call(
        "task.create",
        json!({
            "project_id": w.project_id.clone(),
            "request": "在 README.md 末尾加一行 hello from autome。只改这一个文件。"
        }),
    );
    let task_id = ok(&created)["task"]["id"].as_str().unwrap().to_string();

    // Phase 1: design, up to the approval stop.
    w.settle(2400);
    assert_eq!(
        w.node(&task_id).as_deref(),
        Some("await_design_approval"),
        "the design loop should have stopped for approval; state {}{}",
        w.state(&task_id),
        w.dump(&task_id)
    );
    eprintln!("== design approved by the user ==");

    // Phase 2: implementation, up to the merge stop.
    let approved = w.call("task.approve", json!({ "task_id": task_id }));
    assert!(matches!(approved.reply.outcome, ReplyOutcome::Ok { .. }));
    w.settle(3600);
    assert_eq!(
        w.node(&task_id).as_deref(),
        Some("await_merge"),
        "the implementation loop should have reached the merge stop; state {}{}",
        w.state(&task_id),
        w.dump(&task_id)
    );

    // The change is real and the panel can describe it.
    let changes = w.call("task.changes", json!({ "task_id": task_id }));
    let payload = ok(&changes).clone();
    eprintln!(
        "== {} commits, {} files, +{} -{} ==",
        payload["commits"],
        payload["files"].as_array().map(Vec::len).unwrap_or(0),
        payload["total_added"],
        payload["total_deleted"]
    );
    assert_eq!(payload["mergeable"], json!(true), "{payload:#?}");
    assert!(
        payload["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "README.md"),
        "the requested file should be among the changes: {payload:#?}"
    );

    // Phase 3: the user merges. Nothing before this point may have touched the
    // default branch.
    let before = automed::git::head_sha(&w.repo).unwrap();
    w.call("task.merge", json!({ "task_id": task_id }));
    w.settle(120);

    let task = w.ctx.store.get_task(&task_id).unwrap();
    assert_eq!(
        serde_json::to_value(&task.state).unwrap()["state"],
        json!("done"),
        "{}{}",
        w.state(&task_id),
        w.dump(&task_id)
    );
    assert!(task.merge_commit.is_some());
    assert_ne!(automed::git::head_sha(&w.repo).unwrap(), before);

    // The requested change is on the default branch.
    let readme = std::fs::read_to_string(w.repo.join("README.md")).unwrap();
    assert!(
        readme.contains("hello from autome"),
        "the merged README should carry the requested line:\n{readme}"
    );

    // And the worktree and branch are gone.
    assert!(!w.repo.join(".worktree").join(&task.slug).exists());
    assert!(!automed::git::branch_exists(&w.repo, &task.branch()));
    eprintln!("== merged as {} ==", task.merge_commit.unwrap());
}

// ---------------------------------------------------------------------------
// The terminal hop
// ---------------------------------------------------------------------------

/// The one thing headless mode skips: handing the command to a terminal.
///
/// Runs a harmless command rather than a CLI, so it costs nothing and proves
/// only what it claims — that the AppleScript we generate is accepted and that
/// the wrapper runs under it.
#[test]
fn the_terminal_hop_starts_the_wrapper() {
    needs_real_cli!();
    if cfg!(not(target_os = "macos")) {
        eprintln!("skipping: macOS only");
        return;
    }
    let dir = std::env::temp_dir().join(format!("automed-term-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    automed::init::init(&dir, &automed::protocol::seed()).unwrap();

    let session_dir = dir.join(".autome/output/sessions/T-term");
    std::fs::create_dir_all(&session_dir).unwrap();
    let prompt = session_dir.join("p.txt");
    std::fs::write(&prompt, "unused").unwrap();

    // `/bin/sh -c "..."` stands in for the CLI: the launcher does not care
    // what the binary is, only that the wrapper can run it.
    let wrapper = automed::init::wrapper_script(&dir);
    let argv: Vec<String> = vec![
        wrapper.to_string_lossy().into_owned(),
        "s-term".into(),
        session_dir.to_string_lossy().into_owned(),
        "claude".into(),
        "/bin/sh".into(),
        prompt.to_string_lossy().into_owned(),
        "-c".into(),
        format!("echo hello > {}/proof.txt", session_dir.display()),
    ];
    let command = automed::launcher::shell_command(&argv, &dir);

    let script = format!(
        r#"tell application "Terminal"
             do script {}
           end tell"#,
        automed::launcher::as_quote(&command)
    );
    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .expect("osascript runs");
    assert!(
        out.status.success(),
        "osascript refused the script: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The terminal is asynchronous, so wait for the wrapper's own artefacts.
    let marker = session_dir.join("s-term.exit");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline && !marker.exists() {
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(
        marker.exists(),
        "the wrapper never wrote its marker; the terminal hop did not start it"
    );
    let parsed =
        autome_domain::session::ExitMarker::parse(&std::fs::read_to_string(&marker).unwrap())
            .expect("the marker parses");
    assert_eq!(parsed.exit_code, 0);
    assert!(
        session_dir.join("proof.txt").exists(),
        "the command itself ran"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// The implementation round must be able to run a command.
///
/// This is the test that was missing. `every_adapter_flag_exists_in_the_cli_it_
/// is_passed_to` passed the whole time the product was broken: every flag in
/// the Claude adapter existed, and `--permission-mode acceptEdits` was spelled
/// correctly. It simply did not permit Bash, so an implementation round could
/// write code and never compile it. Flag existence is not flag meaning.
#[test]
fn a_claude_session_can_actually_run_a_command() {
    needs_real_cli!();

    let config = autome_domain::config::RoleConfig {
        runtime: autome_domain::role::Runtime::Claude,
        model: String::new(),
        effort: None,
        enabled: true,
        skills: Vec::new(),
    };
    let args = automed::launcher::build_args(&config, std::path::Path::new("/nonexistent-so-git-cannot-answer"));
    let adapter = automed::launcher::adapter(autome_domain::role::Runtime::Claude);

    let dir = std::env::temp_dir().join(format!("automed-realcli-bash-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // The command has to be one the CLI does not wave through on its own.
    // `echo` is on Claude Code's built-in read-only allowlist and runs even
    // under the broken configuration — a first version of this test used it
    // and passed against the very bug it was written for. Running a script is
    // the shape that actually failed: the blocked session's own log lists
    // `bash scripts/verify.sh` as denied.
    let marker = "AUTOME-BASH-9F3C2";
    std::fs::write(dir.join("probe.sh"), format!("#!/bin/sh\necho {marker}\n")).unwrap();
    let prompt = "运行 `bash ./probe.sh`，然后把它的输出原样贴回来。不要做别的。".to_string();

    let mut child = std::process::Command::new(adapter.binary)
        .args(&args)
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("claude must be on PATH");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(prompt.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        text.contains(marker),
        "the session could not run a command, so it cannot implement anything.\n\
         argv: {} {}\noutput:\n{text}",
        adapter.binary,
        args.join(" ")
    );
}

/// The session log must contain the work, not just its closing paragraph.
///
/// With `--output-format text` — what this project shipped first — a round that
/// edited a dozen files left a 3 KB log holding only the model's summary. The
/// user's report was exact: "只有一个启动内容，没有 AI 实际执行的内容". Nothing
/// in the stand-in suite could see it: the stand-in's output is whatever the
/// step script echoes.
#[test]
fn a_claude_session_log_records_the_tools_it_ran() {
    needs_real_cli!();

    let config = autome_domain::config::RoleConfig {
        runtime: autome_domain::role::Runtime::Claude,
        model: String::new(),
        effort: None,
        enabled: true,
        skills: Vec::new(),
    };
    let args = automed::launcher::build_args(&config, std::path::Path::new("/nonexistent-so-git-cannot-answer"));
    let adapter = automed::launcher::adapter(autome_domain::role::Runtime::Claude);

    let dir = std::env::temp_dir().join(format!("automed-realcli-log-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("probe.sh"), "#!/bin/sh\necho RENDERED-OK\n").unwrap();

    let mut child = std::process::Command::new(adapter.binary)
        .args(&args)
        .current_dir(&dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("claude must be on PATH");
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all("运行 `bash ./probe.sh`，然后贴回输出。".as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let raw = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // Render it the way the wrapper does.
    let mut rendered = Vec::new();
    automed::stream_render::render_stream(
        autome_domain::role::Runtime::Claude,
        std::io::BufReader::new(raw.as_bytes()),
        &mut rendered,
    )
        .unwrap();
    let text = String::from_utf8(rendered).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        text.contains("→ Bash"),
        "the log records no tool call, so it is not a record of the work.\n\
         argv: {} {}\nrendered:\n{text}",
        adapter.binary,
        args.join(" ")
    );
    assert!(
        text.contains("==="),
        "the log has no init/result marker:\n{text}"
    );
}
