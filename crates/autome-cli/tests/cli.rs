//! The command line, as a person would use it: the real `autome` binary, run
//! once per command, against a ledger of its own.
//!
//! The core's end-to-end suite already proves the Loop; what is left to prove
//! here is the part only this binary has — that separate processes, each
//! opening the ledger afresh, drive a task from one line to a merge commit,
//! and that `autome run` both starts sessions and collects them. The model is
//! the same stand-in the core's suite uses: a script that writes the design
//! document each round would have written.

use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::Value;

/// Finds the repository by walking up to `.autome/fake`, then runs the next
/// numbered step there. The same contract as the core's end-to-end stand-in.
const FAKE_CLI: &str = r#"#!/bin/sh
set -u
dir=$PWD
fake=""
while [ "$dir" != "/" ]; do
  if [ -d "$dir/.autome/fake" ]; then fake="$dir/.autome/fake"; break; fi
  dir=$(dirname "$dir")
done
[ -n "$fake" ] || exit 90
n=$(cat "$fake/next" 2>/dev/null || echo 1)
echo "$n" >> "$fake/history"
expr "$n" + 1 > "$fake/next"
[ -f "$fake/$n.sh" ] || { echo "fake-cli: no step $n" >&2; exit 92; }
cat > "$fake/$n.prompt"
sh "$fake/$n.sh"
"#;

struct Sandbox {
    root: PathBuf,
    repo: PathBuf,
    fake_cli: PathBuf,
}

impl Sandbox {
    fn new(tag: &str) -> Sandbox {
        let root = std::env::temp_dir().join(format!("autome-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let repo = root.join("repo");
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::create_dir_all(&repo).unwrap();

        let fake_cli = root.join("fake-cli");
        std::fs::write(&fake_cli, FAKE_CLI).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake_cli, std::fs::Permissions::from_mode(0o755)).unwrap();

        automed::git::init(&repo, "main").unwrap();
        // The stand-in's control files must be ignored, or the main worktree
        // reads dirty and the merge is refused for a reason unrelated to this.
        std::fs::write(repo.join(".gitignore"), ".autome/fake/\n").unwrap();
        std::fs::write(repo.join("README.md"), "hello\n").unwrap();
        automed::git::commit_paths(&repo, &[".gitignore", "README.md"], "initial").unwrap();
        std::fs::create_dir_all(repo.join(".autome/fake")).unwrap();
        std::fs::write(repo.join(".autome/fake/next"), "1").unwrap();

        Sandbox {
            root,
            repo,
            fake_cli,
        }
    }

    /// Runs `autome <args>` from the repository, as a person in it would.
    fn autome(&self, args: &[&str]) -> Output {
        std::process::Command::new(env!("CARGO_BIN_EXE_autome"))
            .args(args)
            .current_dir(&self.repo)
            .env("AUTOME_HOME", self.root.join("home"))
            .env("AUTOMED_DB_PATH", self.root.join("home/ledger.sqlite3"))
            .env("AUTOMED_CLAUDE_BINARY", &self.fake_cli)
            .env("AUTOMED_CODEX_BINARY", &self.fake_cli)
            .output()
            .unwrap()
    }

    /// `autome <args>`, which must succeed; returns its stdout.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.autome(args);
        assert!(
            out.status.success(),
            "autome {args:?} exited {:?}\nstdout:\n{}\nstderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn json(&self, args: &[&str]) -> Value {
        let mut args = args.to_vec();
        args.push("--json");
        serde_json::from_str(&self.ok(&args)).unwrap()
    }

    /// Queues the Nth session's work: write `body` as the design document and
    /// commit it, with the evidence files a loop round must leave.
    fn step(&self, n: u32, slug: &str, body: &str) {
        let script = format!(
            r#"set -e
mkdir -p "docs/{slug}"
cat > "docs/{slug}/{slug}.md" <<'AUTOME_EOF'
{body}
AUTOME_EOF
k=$(grep -m1 '^implementation-round:' "docs/{slug}/{slug}.md" | sed 's#[^0-9]*\([0-9]*\)/.*#\1#')
if [ "${{k:-0}}" != "0" ]; then
  mkdir -p "docs/{slug}/evidence"
  printf '命令：fake\n结果：通过\n' > "docs/{slug}/evidence/M-01-r$k-impl.md"
  printf '复验：fake\n结论：通过\n' > "docs/{slug}/evidence/M-01-r$k-audit.md"
  printf '轮次 | M-01 | 通过 | e | 无\n' >> "docs/{slug}/retro.md"
fi
git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session {n}" >/dev/null 2>&1 || true
"#
        );
        std::fs::write(self.repo.join(format!(".autome/fake/{n}.sh")), script).unwrap();
    }

    fn retro(&self, n: u32, slug: &str) {
        let script = format!(
            r#"set -e
mkdir -p "docs/{slug}"
printf '# 教训\n\n没有。\n' > "docs/{slug}/lessons.md"
git add -A docs >/dev/null 2>&1 || true
git -c user.name=fake -c user.email=f@f commit -q -m "session {n} retro" >/dev/null 2>&1 || true
"#
        );
        std::fs::write(self.repo.join(format!(".autome/fake/{n}.sh")), script).unwrap();
    }

    fn task_state(&self, task: &str) -> Value {
        self.json(&["status", task])["task"]["state"].clone()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if std::env::var("AUTOMED_KEEP_SANDBOX").is_err() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

fn visible_status_session(tag: &str, timed_out: bool) -> String {
    use autome_domain::role::{Role, Runtime};
    use autome_domain::session::{Session, SessionKind, SessionLifecycle};
    let s = Sandbox::new(tag);
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let created = s.json(&["task", "new", "timeout status", "--draft"]);
    let id = created["task"]["id"].as_str().unwrap();
    let db = s.root.join("home/ledger.sqlite3");
    let store = automed::store::Store::open(&db).unwrap();
    store
        .insert_session(&Session {
            id: "visible-session".into(),
            task_id: id.into(),
            kind: SessionKind::Role { role: Role::Impl },
            runtime: Runtime::Claude,
            model: "fixture".into(),
            effort: None,
            skills: vec![],
            round: 1,
            started_at: String::new(),
            ended_at: None,
            lifecycle: if timed_out {
                SessionLifecycle::TimedOut { limit_minutes: 5 }
            } else {
                SessionLifecycle::Running
            },
            limit_minutes: Some(if timed_out { 5 } else { 120 }),
            log_path: String::new(),
            pid: None,
            protocol_ref: None,
            rules_hash: None,
            metrics: Default::default(),
        })
        .unwrap();
    drop(store);
    rusqlite::Connection::open(&db).unwrap().execute("UPDATE sessions SET started_at = strftime('%Y-%m-%dT%H:%M:%SZ','now','-37 minutes') WHERE id = 'visible-session'", []).unwrap();
    // The CLI must print the kernel's launch snapshot, not this current config.
    std::fs::write(
        s.repo.join(".autome/config.toml"),
        "[loop]\nsession_minutes = 30\n",
    )
    .unwrap();
    s.ok(&["status", id])
}

#[test]
fn timeout_visible_status_running() {
    let output = visible_status_session("visible-running", false);
    assert!(output.contains("已运行 37 分钟 / 上限 120"), "{output}");
    assert!(!output.contains("超时（"), "{output}");
}

#[test]
fn timeout_visible_status_timed_out() {
    let output = visible_status_session("visible-timeout", true);
    assert!(output.contains("超时（上限 5 分钟）"), "{output}");
    assert!(!output.contains("已运行"), "{output}");
}

#[test]
fn status_shows_latest_five_sessions_in_chronological_order() {
    use autome_domain::role::{Role, Runtime};
    use autome_domain::session::{Session, SessionKind, SessionLifecycle};

    let s = Sandbox::new("status-latest-five");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let created = s.json(&["task", "new", "session history", "--draft"]);
    let id = created["task"]["id"].as_str().unwrap();
    let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();

    // Fixed distinct timestamps and shuffled insertion avoid depending on
    // scheduler timing, row insertion order, or a real model process.
    for n in [4, 1, 7, 2, 6, 3, 5] {
        store
            .insert_session(&Session {
                id: format!("history-{n}"),
                task_id: id.into(),
                kind: SessionKind::Role { role: Role::Impl },
                runtime: Runtime::Claude,
                model: "fixture".into(),
                effort: None,
                skills: vec![],
                round: n,
                started_at: format!("2026-01-{n:02}T00:00:00Z"),
                ended_at: Some(format!("2026-01-{n:02}T00:01:00Z")),
                lifecycle: SessionLifecycle::Exited { exit_code: 0 },
                log_path: String::new(),
                pid: None,
                limit_minutes: None,
                protocol_ref: None,
                rules_hash: None,
                metrics: Default::default(),
            })
            .unwrap();
    }
    drop(store);

    let json = s.json(&["status", id]);
    let all: Vec<_> = json["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|session| session["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        all,
        [
            "history-7",
            "history-6",
            "history-5",
            "history-4",
            "history-3",
            "history-2",
            "history-1"
        ],
        "kernel history must be complete and newest first"
    );

    let output = s.ok(&["status", id]);
    assert!(output.contains("会话（最近 5 个）"), "{output}");
    let shown: Vec<_> = output
        .lines()
        .filter_map(|line| {
            let first = line.split_whitespace().next()?;
            first.starts_with("history-").then_some(first)
        })
        .collect();
    assert_eq!(
        shown,
        [
            "history-3",
            "history-4",
            "history-5",
            "history-6",
            "history-7"
        ],
        "status must show exactly the newest five, oldest to newest\n{output}"
    );
}

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

fn head(repo: &Path) -> String {
    automed::git::head_sha(repo).unwrap()
}

#[test]
fn write_command_waits_for_a_driver_lock_held_for_fifteen_seconds() {
    use std::time::{Duration, Instant};

    let s = Sandbox::new("long-driver-lock");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    // Draft creation is a real write through the same dispatch lock as
    // merge/approve, without launching a model or requiring a full Loop.
    s.ok(&["task", "new", "before contention", "--draft"]);
    let db = s.root.join("home/ledger.sqlite3");
    let held = automed::driver_lock::try_acquire(&db).unwrap().unwrap();
    assert!(automed::driver_lock::try_acquire(&db).unwrap().is_none());

    // Establish contention before launching the CLI; do not race two starts.
    // Release only after the CLI has been spawned and fifteen seconds elapse.
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_autome"))
        .args(["task", "new", "during contention", "--draft", "--json"])
        .current_dir(&s.repo)
        .env("AUTOME_HOME", s.root.join("home"))
        .env("AUTOMED_DB_PATH", &db)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(15));
        let released_at = Instant::now();
        drop(held);
        released_at
    });
    let deadline = Instant::now() + Duration::from_secs(130);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            release.join().unwrap();
            panic!("CLI did not finish within 130 seconds; this is not the expected regression");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    let finished_at = Instant::now();
    let released_at = release.join().unwrap();

    // Verify release and the positive control before reporting the regression.
    let reacquired = automed::driver_lock::try_acquire(&db).unwrap().unwrap();
    drop(reacquired);
    s.ok(&["task", "new", "after contention", "--draft"]);
    assert!(
        output.status.success(),
        "write must succeed after a 15-second driver lock; exit={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        finished_at >= released_at,
        "write must not bypass the held lock"
    );
    let created: Value = serde_json::from_slice(&output.stdout).unwrap();
    let id = created["task"]["id"].as_str().unwrap();
    assert_eq!(s.json(&["status", id])["task"]["id"], id);
    assert!(
        output.stderr.is_empty(),
        "--json must not emit wait notices"
    );
}

#[test]
fn a_task_goes_from_one_line_to_a_merge_commit_one_command_at_a_time() {
    let s = Sandbox::new("happy");
    let request = "add cart checkout";
    let slug = autome_domain::project::slugify(request);
    // intake, design, review, adjudicate, implement, audit, retro — the same
    // seven rounds as the core's happy path.
    s.step(1, &slug, &doc("设计中", 0, 0, &[]));
    s.step(2, &slug, &doc("设计中", 1, 0, &[]));
    s.step(3, &slug, &doc("设计中", 1, 0, &[]));
    s.step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    s.step(5, &slug, &doc("实现中", 2, 1, &[("M-01", "待审")]));
    s.step(6, &slug, &doc("实现中", 2, 1, &[("M-01", "已完成")]));
    s.retro(7, &slug);

    let repo = s.repo.to_str().unwrap();
    s.ok(&["project", "add", repo, "--skip-onboarding"]);
    assert!(s.ok(&["project", "ls"]).contains(repo));

    // No `-p`: the working directory names the project.
    let created = s.json(&["task", "new", request]);
    let id = created["task"]["id"].as_str().unwrap().to_string();

    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    assert_eq!(s.task_state(&id)["node"], "await_design_approval");
    // The dashboard view tells the person what to type next.
    assert!(s.ok(&["status"]).contains(&format!("autome approve {id}")));

    s.ok(&["approve", &slug]); // a slug works as well as an id
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    assert_eq!(s.task_state(&id)["node"], "await_merge");

    let before = head(&s.repo);
    s.ok(&["merge", &id]);
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);

    assert_eq!(s.task_state(&id)["state"], "done");
    assert_ne!(head(&s.repo), before, "the default branch moved");
    assert!(s.repo.join(format!("docs/{slug}/{slug}.md")).exists());
    // Every round's log is reachable from the command line.
    s.ok(&["logs", &id]);
}

#[test]
fn mistakes_on_the_command_line_exit_with_two_and_core_refusals_with_one() {
    let s = Sandbox::new("errors");
    let usage = s.autome(&["status", "--jsno"]);
    assert_eq!(usage.status.code(), Some(2));
    assert_eq!(s.autome(&["frobnicate"]).status.code(), Some(2));

    // Not inside a registered project, and none named.
    let refused = s.autome(&["task", "new", "anything"]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("autome project add"));

    assert_eq!(s.autome(&["approve", "T-404"]).status.code(), Some(1));
}

#[test]
fn the_binary_renders_session_logs_the_way_the_core_does() {
    // The wrapper pipes a session's stream through `current_exe()
    // render-stream`, which is this binary when the terminal started it.
    use std::io::Write;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_autome"))
        .args(["render-stream", "claude"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi there\"}]}}\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("hi there"));
}

#[test]
fn help_and_version_need_no_ledger() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_autome"))
        .arg("--version")
        .env("AUTOME_HOME", "/nonexistent/autome-home")
        .output()
        .unwrap();
    assert!(out.status.success());
    let version = String::from_utf8_lossy(&out.stdout);
    assert!(version.starts_with("autome 0."), "{version}");
    assert_eq!(version.lines().count(), 1, "the version, not the help");
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_autome"))
        .arg("help")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("run [--once | --until-idle]"));
}

fn task_card_prepare(s: &Sandbox, slug: &str) {
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    for n in 1..=4 {
        s.step(
            n,
            slug,
            &doc(
                if n == 4 { "实现中" } else { "设计中" },
                n,
                0,
                &[("M-01", "开放")],
            ),
        );
    }
}

#[test]
fn task_card_a_brief_piped_on_stdin_creates_a_bug_draft_that_starts_with_its_card() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let s = Sandbox::new("card-stdin");
    let slug = "stdin-card";
    task_card_prepare(&s, slug);
    let contract = include_str!("../../../protocol/bug-protocol.md");
    let example = contract
        .split("```text\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap()
        .replace("已定位", "无法复现");
    std::fs::write(
        s.repo.join(".autome/fake/1.sh"),
        format!("mkdir -p docs/{slug}\ncat > docs/{slug}/{slug}-bug.md <<'EOF'\n{example}\nEOF\n"),
    )
    .unwrap();
    let brief = "# stdin card\n\n中文 {request}\n";
    let mut child = Command::new(env!("CARGO_BIN_EXE_autome"))
        .args([
            "task", "new", "--brief", "-", "--kind", "bug", "--draft", "--json",
        ])
        .current_dir(&s.repo)
        .env("AUTOME_HOME", s.root.join("home"))
        .env("AUTOMED_DB_PATH", s.root.join("home/ledger.sqlite3"))
        .env("AUTOMED_CLAUDE_BINARY", &s.fake_cli)
        .env("AUTOMED_CODEX_BINARY", &s.fake_cli)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(brief.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    let created: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(created["task"]["kind"], "bug");
    assert_eq!(created["task"]["state"]["state"], "draft");
    let id = created["task"]["id"].as_str().unwrap();
    assert!(s.ok(&["status", id]).contains("待开始"));
    let started = s.json(&["task", "start", slug]);
    assert_eq!(started["task"]["id"], id);
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    assert_eq!(s.task_state(id)["node"], "await_info");
    let card = s
        .repo
        .join(format!(".worktree/{slug}/docs/{slug}/{slug}-card.md"));
    assert_eq!(std::fs::read(card).unwrap(), brief.as_bytes());
}

#[test]
fn task_card_a_brief_from_a_file_names_the_task_and_an_oversized_one_is_refused() {
    let s = Sandbox::new("card-file");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let file = s.root.join("card.md");
    let path = file.to_str().unwrap();
    std::fs::write(&file, "# 文件标题\n正文").unwrap();
    let created = s.json(&["task", "new", "--brief", path, "--draft"]);
    assert_eq!(created["task"]["request"], "文件标题");
    assert_eq!(created["task"]["kind"], "feature");
    std::fs::write(&file, "a".repeat(65536)).unwrap();
    s.ok(&["task", "new", "边界", "--brief", path, "--draft"]);
    let before = s.json(&["ls"]).as_array().unwrap().len();
    for (bytes, message) in [
        (vec![b'a'; 65537], "64KB"),
        (vec![0xff], "任务卡正文不是 UTF-8 文本"),
        (b" \n".to_vec(), "任务卡正文不能为空"),
    ] {
        std::fs::write(&file, bytes).unwrap();
        let out = s.autome(&["task", "new", "--brief", path, "--draft"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&out.stderr).contains(message));
        assert_eq!(s.json(&["ls"]).as_array().unwrap().len(), before);
    }
    std::fs::remove_file(&file).unwrap();
    let out = s.autome(&["task", "new", "--brief", path, "--draft"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("读取任务卡正文"));
    assert_eq!(out.status.code(), Some(1));
    let missing = s.autome(&["task", "new"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("一句话需求或 --brief"));
    let human = s.ok(&["task", "new", "human card", "--draft"]);
    assert!(human.contains("已存为待开始。用 `autome task start"));
}

#[test]
fn task_card_reply_rejects_the_design_with_the_text_verbatim() {
    // New Bug tasks use Locate; Bug reply coverage is planned separately in M-04.
    for kind in ["feature"] {
        let s = Sandbox::new(&format!("card-reply-{kind}"));
        let slug = "reply-card";
        task_card_prepare(&s, slug);
        let created = s.json(&["task", "new", "reply card", "--kind", kind]);
        let id = created["task"]["id"].as_str().unwrap();
        s.ok(&["run", "--until-idle", "--interval", "0.05"]);
        assert_eq!(s.task_state(id)["node"], "await_design_approval");
        let missing = s.autome(&["reply", id]);
        assert_eq!(missing.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&missing.stderr).contains("缺少参数：补充信息"));
        for n in 5..=7 {
            s.step(n, slug, &doc("实现中", 3, 0, &[("M-01", "开放")]));
        }
        let feedback = "请保留  双空格\n以及中文原文。";
        s.ok(&["reply", id, feedback]);
        s.ok(&["run", "--until-idle", "--interval", "0.05"]);
        let prompt = std::fs::read_to_string(s.repo.join(".autome/fake/5.prompt")).unwrap();
        assert!(prompt.contains(feedback), "{prompt}");
        assert_eq!(s.task_state(id)["node"], "await_design_approval");
    }
}

#[test]
fn task_card_start_refuses_a_task_that_is_not_a_draft() {
    let s = Sandbox::new("card-start-refuse");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let created = s.json(&["task", "new", "cancelled card", "--draft"]);
    let id = created["task"]["id"].as_str().unwrap();
    let reply = s.autome(&["reply", id, "不应启动"]);
    assert_eq!(reply.status.code(), Some(1));
    assert_eq!(s.task_state(id)["state"], "draft");
    s.ok(&["cancel", id]);
    let out = s.autome(&["task", "start", id]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("待开始"));
    assert_eq!(s.task_state(id)["state"], "cancelled");
}

fn bug_wait_step(s: &Sandbox, n: u32, slug: &str, status: &str) {
    let body = format!("status: {status}\nfix-round: 0/3\n复现命令: 无\n根因: 无\n复核: 待复核\n");
    std::fs::write(
        s.repo.join(format!(".autome/fake/{n}.sh")),
        format!(
            "set -e\nmkdir -p docs/{slug}\ncat > docs/{slug}/{slug}-bug.md <<'EOF'\n{body}\nEOF\n"
        ),
    )
    .unwrap();
}

#[test]
fn bug_loop_reply_returns_a_bug_to_locate_with_the_text_verbatim() {
    let s = Sandbox::new("bug-reply");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let slug = "bug-reply";
    bug_wait_step(&s, 1, slug, "无法复现");
    bug_wait_step(&s, 2, slug, "不是 Bug");
    let created = s.json(&["task", "new", "bug reply", "--kind", "bug"]);
    let id = created["task"]["id"].as_str().unwrap();
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    assert_eq!(s.task_state(id)["node"], "await_info");
    let text = "中文  双空格\n{request} 保留\n";
    s.ok(&["reply", id, text]);
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    assert_eq!(s.task_state(id)["node"], "not_a_bug");
    let replies = std::fs::read_to_string(
        s.repo
            .join(format!(".worktree/{slug}/docs/{slug}/{slug}-replies.md")),
    )
    .unwrap();
    assert!(replies.ends_with(&format!("\n\n{text}\n\n")), "{replies}");
    assert!(automed::git::is_clean(&s.repo.join(format!(".worktree/{slug}"))).unwrap());
    let prompt = std::fs::read_to_string(s.repo.join(".autome/fake/2.prompt")).unwrap();
    assert!(prompt.contains("bug-reply-replies.md"));
}

#[test]
fn bug_loop_status_shows_the_bug_nodes_and_what_to_do_next() {
    let s = Sandbox::new("bug-status");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    bug_wait_step(&s, 1, "bug-status", "无法复现");
    bug_wait_step(&s, 2, "bug-status", "不是 Bug");
    let created = s.json(&["task", "new", "bug status", "--kind", "bug"]);
    let id = created["task"]["id"].as_str().unwrap();
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    let status = s.ok(&["status", id]);
    assert!(
        status.contains("等你补充信息") && status.contains(&format!("autome reply {id}")),
        "{status}"
    );
    s.ok(&["reply", id, "补充"]);
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    let status = s.ok(&["status", id]);
    assert!(
        status.contains("等你决定（不是 Bug）")
            && status.contains(&format!("autome task convert {id}"))
            && status.contains(&format!("autome cancel {id}")),
        "{status}"
    );
}

#[test]
fn broken_pipe_status_exits_quietly() {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};

    let s = Sandbox::new("broken-pipe");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let created = s.json(&["task", "new", "pipe regression", "--draft"]);
    let id = created["task"]["id"].as_str().unwrap();
    let normal = s.autome(&["status", id]);
    assert!(normal.status.success());
    assert!(normal.stderr.is_empty());
    assert!(String::from_utf8_lossy(&normal.stdout).lines().count() > 1);

    // Close the only reader BEFORE starting autome: unlike `| head -1`,
    // this cannot race with the producer finishing its small output.
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let reader = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let writer = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    drop(reader);
    let out = Command::new(env!("CARGO_BIN_EXE_autome"))
        .args(["status", id])
        .current_dir(&s.repo)
        .env("AUTOME_HOME", s.root.join("home"))
        .env("AUTOMED_DB_PATH", s.root.join("home/ledger.sqlite3"))
        .env("RUST_BACKTRACE", "1")
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        out.stderr.is_empty(),
        "closed stdout must be quiet; status={:?}, stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.status.success()
            || out.status.code() == Some(141)
            || out.status.signal() == Some(libc::SIGPIPE),
        "{:?}",
        out.status
    );
}

#[test]
fn broken_pipe_sigpipe_remains_ignored_in_cli_process() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let mut child = Command::new(env!("CARGO_BIN_EXE_autome"))
        .args(["render-stream", "claude"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let message = b"{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"pipe-probe\"}]}}\n";
    input.write_all(message).unwrap();
    let mut line = String::new();
    assert!(output.read_line(&mut line).unwrap() > 0);
    assert!(line.contains("pipe-probe"), "{line}");
    // The output handshake proves the real CLI has passed main/startup.
    // SIG_DFL would kill it here, even though stdout is healthy.
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGPIPE) },
        0
    );
    let write = input.write_all(message);
    drop(input);
    let mut rest = String::new();
    use std::io::Read;
    output.read_to_string(&mut rest).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "SIGPIPE killed CLI: {:?}", out.status);
    write.unwrap();
    assert!(rest.contains("pipe-probe"), "{rest}");
    assert!(out.stderr.is_empty(), "{:?}", out.stderr);
}

#[test]
fn task_convert_rebuilds_a_not_a_bug_task_from_the_command_line() {
    let s = Sandbox::new("convert-draft");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    for args in [vec!["--help"], vec!["help"]] {
        assert!(s.ok(&args).contains("task convert"));
    }
    // Exercise both JSON and the three-line human output on separate originals.
    for (n, slug) in [(1, "convert-json"), (2, "convert-text")] {
        bug_wait_step(&s, n, slug, "不是 Bug");
        let created = s.json(&["task", "new", slug, "--kind", "bug"]);
        let id = created["task"]["id"].as_str().unwrap();
        s.ok(&["run", "--until-idle", "--interval", "0.05"]);
        assert!(
            s.ok(&["status", id])
                .contains(&format!("autome task convert {id}"))
        );
        if n == 1 {
            let out = s.json(&["task", "convert", "convert-j"]);
            assert_eq!(out["task"]["kind"], "feature");
            assert_eq!(out["task"]["state"]["state"], "draft");
            assert_eq!(out["converted_from"], id);
            assert_eq!(out["started"], false);
            assert!(out.get("queue_position").is_some());
        } else {
            let out = s.ok(&["task", "convert", id]);
            let lines: Vec<_> = out.lines().collect();
            assert_eq!(lines.len(), 3, "{out}");
            assert!(lines[0].contains("convert-text-2"));
            assert_eq!(lines[1], format!("由 {id} 改建，{id} 已取消。"));
            assert!(lines[2].contains("已存为待开始。用 `autome task start T-"));
        }
        assert_eq!(s.task_state(id)["state"], "cancelled");
        let refused = s.autome(&["task", "convert", id]);
        assert_eq!(refused.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&refused.stderr).contains("不是 Bug"));
    }
}

#[test]
fn task_convert_with_start_queues_and_rejects_other_tasks() {
    let s = Sandbox::new("convert-start");
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    // Each mode gets its own fake-session sequence; --start launches a session.
    for (n, slug) in [(1, "start-json"), (2, "start-text")] {
        let s = Sandbox::new(slug);
        s.ok(&[
            "project",
            "add",
            s.repo.to_str().unwrap(),
            "--skip-onboarding",
        ]);
        bug_wait_step(&s, 1, slug, "不是 Bug");
        let created = s.json(&["task", "new", slug, "--kind", "bug"]);
        let id = created["task"]["id"].as_str().unwrap();
        s.ok(&["run", "--until-idle", "--interval", "0.05"]);
        if n == 1 {
            let out = s.json(&["task", "convert", id, "--start"]);
            assert_ne!(out["task"]["state"]["state"], "draft");
            assert_eq!(out["task"]["kind"], "feature");
            assert_eq!(out["converted_from"], id);
            // Cancel the new task before preparing the next original.
            s.ok(&["cancel", out["task"]["id"].as_str().unwrap()]);
        } else {
            let out = s.ok(&["task", "convert", id, "--start"]);
            assert_eq!(out.lines().count(), 3, "{out}");
            assert!(out.contains(&format!("由 {id} 改建，{id} 已取消。")));
            assert!(out.contains("Loop 由 `autome run` 或打开的桌面端推进。"));
        }
        assert_eq!(s.task_state(id)["state"], "cancelled");
    }
    let draft = s.json(&["task", "new", "feature draft", "--draft"]);
    let id = draft["task"]["id"].as_str().unwrap();
    let refused = s.autome(&["task", "convert", id]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stderr).contains("只有 Bug"));
    assert_eq!(s.task_state(id)["state"], "draft");
    assert_eq!(s.autome(&["task", "convert"]).status.code(), Some(2));
    for name in ["", "T-999999"] {
        assert_eq!(s.autome(&["task", "convert", name]).status.code(), Some(1));
    }
}

#[test]
fn run_json_prints_every_event_as_one_json_line() {
    let s = Sandbox::new("run-json");
    let empty = s.ok(&["run", "--json", "--until-idle"]);
    assert_eq!(
        serde_json::from_str::<Value>(empty.trim()).unwrap(),
        serde_json::json!({"idle":true,"tasks":[]})
    );
    assert!(
        s.ok(&["run", "--json", "--once", "--until-idle"])
            .is_empty()
    );
    let bad = s.autome(&["run", "--json", "--interval", "bad"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(bad.stdout.is_empty());
    let request = "json events";
    let slug = autome_domain::project::slugify(request);
    s.step(1, &slug, &doc("设计中", 0, 0, &[]));
    s.step(2, &slug, &doc("设计中", 1, 0, &[]));
    s.step(3, &slug, &doc("设计中", 1, 0, &[]));
    s.step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let created = s.json(&["task", "new", request]);
    let id = created["task"]["id"].as_str().unwrap();
    let before = s.json(&["rpc", "events.since"])["latest_seq"].clone();
    let output = s.autome(&["run", "--json", "--until-idle", "--interval", "0.05"]);
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty(), "{:?}", output);
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let end = lines.pop().unwrap();
    assert_eq!(end["idle"], true);
    let task = end["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .unwrap();
    assert_eq!(task["state"], s.task_state(id));
    assert_eq!(task["state"]["node"], "await_design_approval");
    let expected = s.json(&[
        "rpc",
        "events.since",
        &serde_json::json!({"after_seq":before,"limit":1000}).to_string(),
    ]);
    let events = expected["events"].as_array().unwrap();
    assert!(!lines.is_empty());
    assert!(lines.iter().any(|e| e["kind"] == "task.head"));
    assert_eq!(lines.len(), events.len());
    for (actual, expected) in lines.iter().zip(events) {
        assert_eq!(actual["seq"], expected["seq"]);
        assert_eq!(actual["time"], expected["at"]);
        assert_eq!(actual["payload"], expected["payload"]);
        if expected["subject"] == id {
            assert_eq!(actual["task_id"], id);
        }
    }
}

#[test]
fn run_json_closed_pipe_exits_quietly() {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::process::{Command, Stdio};
    let s = Sandbox::new("run-json-pipe");
    s.ok(&["run", "--json", "--once"]);
    let lock = automed::driver_lock::try_acquire(&s.root.join("home/ledger.sqlite3"))
        .unwrap()
        .unwrap();
    let skipped = s.autome(&["run", "--json", "--once"]);
    assert!(skipped.status.success());
    assert!(skipped.stdout.is_empty());
    assert!(skipped.stderr.is_empty());
    drop(lock);

    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let reader = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let writer = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    drop(reader);
    let out = Command::new(env!("CARGO_BIN_EXE_autome"))
        .args(["run", "--json", "--until-idle"])
        .current_dir(&s.repo)
        .env("AUTOME_HOME", s.root.join("home"))
        .env("AUTOMED_DB_PATH", s.root.join("home/ledger.sqlite3"))
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(out.stderr.is_empty());
}

#[test]
fn run_json_tick_error_and_interrupt_have_no_idle_trailer() {
    assert_run_json_tick_error(false);
}

#[test]
fn run_json_tick_error_reaped_session_identifies_task() {
    assert_run_json_tick_error(true);
}

fn assert_run_json_tick_error(reap_finished_session: bool) {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    let s = Sandbox::new(if reap_finished_session {
        "run-json-reap-error"
    } else {
        "run-json-error"
    });
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    // Cover both core-step errors from a draft and reaping errors from an
    // auto-started session that has finished before the first tick.
    let created = if reap_finished_session {
        s.json(&["task", "new", "broken project"])
    } else {
        s.json(&["task", "new", "broken project", "--draft"])
    };
    let id = created["task"]["id"].as_str().unwrap();
    // An invalid project configuration makes its core step report an error.
    let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();
    let expected_prefix = if reap_finished_session {
        let sessions = store.list_sessions(id).unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].lifecycle.is_running());
        let exit_path = s.repo.join(autome_domain::session::SessionPaths::exit(
            id,
            &sessions[0].id,
        ));
        // Wait for a complete exit marker before corrupting configuration.
        // This forces reaping to fail before the task's core step runs.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if std::fs::read_to_string(&exit_path)
                .ok()
                .and_then(|text| autome_domain::session::ExitMarker::parse(&text))
                .is_some()
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "fixture intake did not exit: {}",
                exit_path.display()
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        format!("{id} {}:", sessions[0].id)
    } else {
        assert!(store.all_running_sessions().unwrap().is_empty());
        format!("{id}:")
    };
    store
        .set_task_state(
            id,
            &autome_domain::task::TaskState::Active {
                node: autome_domain::task::Node::Gate,
            },
        )
        .unwrap();
    drop(store);
    std::fs::write(s.repo.join(".autome/config.toml"), "invalid = [").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_autome"))
        .args(["run", "--json", "--interval", "0.05"])
        .current_dir(&s.repo)
        .env("AUTOME_HOME", s.root.join("home"))
        .env("AUTOMED_DB_PATH", s.root.join("home/ledger.sqlite3"))
        .env("AUTOMED_CLAUDE_BINARY", &s.fake_cli)
        .env("AUTOMED_CODEX_BINARY", &s.fake_cli)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line.unwrap()).is_err() {
                break;
            }
        }
    });
    let first = rx.recv_timeout(std::time::Duration::from_secs(10));
    if first.is_err() {
        child.kill().unwrap();
    }
    let first: Value = serde_json::from_str(&first.expect("tick must emit error")).unwrap();
    assert_eq!(
        unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) },
        0
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() > deadline {
            child.kill().unwrap();
            panic!("SIGINT did not stop run");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let out = child.wait_with_output().unwrap();
    reader.join().unwrap();
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    assert!(
        first["error"]
            .as_str()
            .is_some_and(|e| e.starts_with(&expected_prefix) && e.contains("TOML parse error")),
        "{first}"
    );
    assert!(first.get("idle").is_none());
    for line in rx.try_iter() {
        let value: Value = serde_json::from_str(&line).unwrap();
        assert!(value.get("idle").is_none());
    }
}

#[test]
fn logs_json_prints_the_session_stream_one_json_value_per_line() {
    let s = Sandbox::new("logs-json");
    let request = "json logs";
    let slug = autome_domain::project::slugify(request);
    for n in 1..=3 {
        s.step(n, &slug, &doc("设计中", if n == 1 { 0 } else { 1 }, 0, &[]));
    }
    s.step(4, &slug, &doc("实现中", 2, 0, &[("M-01", "开放")]));
    let codex = r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Hello!"}}
{"type":"turn.completed","usage":{"input_tokens":36111,"cached_input_tokens":12928,"cache_write_input_tokens":0,"output_tokens":6,"reasoning_output_tokens":128}}"#;
    let claude = r#"{"type":"system","subtype":"init","model":"claude-opus-5","permissionMode":"acceptEdits"}
{"type":"assistant","message":{"content":[{"type":"text","text":"hi there"}]}}
{"type":"system","subtype":"hook_started"}
{"type":"result","subtype":"success","num_turns":7,"duration_ms":91500}"#;
    for (n, raw) in [(3, codex), (4, claude)] {
        let path = s.repo.join(format!(".autome/fake/{n}.sh"));
        let mut script = std::fs::read_to_string(&path).unwrap();
        script.push_str(&format!("\nprintf '%s\\n' '{raw}'\n"));
        if n == 4 {
            script.push_str("printf 'warning\\n' >&2\n");
        }
        std::fs::write(path, script).unwrap();
    }
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let created = s.json(&["task", "new", request]);
    let id = created["task"]["id"].as_str().unwrap();
    s.ok(&["run", "--until-idle", "--interval", "0.05"]);
    let status = s.json(&[
        "rpc",
        "task.get",
        &serde_json::json!({"task_id":id}).to_string(),
    ]);
    let sessions = status["sessions"].as_array().unwrap();
    let last = sessions.first().unwrap();
    let path = Path::new(last["log_path"].as_str().unwrap()).with_extension("jsonl");
    let parse = |out: String| -> Vec<Value> {
        out.lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let rows = parse(s.ok(&["logs", id, "--json"]));
    assert_eq!(
        rows.len(),
        std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
    );
    assert_eq!(
        rows.iter().map(|r| r["type"].clone()).collect::<Vec<_>>(),
        serde_json::json!(["system", "assistant", "system", "result", null])
            .as_array()
            .unwrap()
            .clone()
    );
    assert_eq!(rows[1]["text"], "hi there");
    assert!(rows[2]["text"].is_null());
    assert_eq!(rows, parse(s.ok(&["logs", id, "--json", "-f"])));
    let review = sessions[1]["id"].as_str().unwrap();
    let review_rows = parse(s.ok(&["logs", id, "--session", review, "--json"]));
    assert_eq!(review_rows[0]["text"], "Hello!");
    assert!(
        review_rows[1]["text"]
            .as_str()
            .unwrap()
            .contains("in 36111")
    );

    use std::os::fd::{FromRawFd, OwnedFd};
    use std::process::{Command, Stdio};
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let reader = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let writer = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    drop(reader);
    let out = Command::new(env!("CARGO_BIN_EXE_autome"))
        .args(["logs", id, "--json"])
        .current_dir(&s.repo)
        .env("AUTOME_HOME", s.root.join("home"))
        .env("AUTOMED_DB_PATH", s.root.join("home/ledger.sqlite3"))
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(out.stderr.is_empty());

    std::fs::write(&path, b"\nwarning").unwrap();
    let tail = parse(s.ok(&["logs", id, "--json"]));
    assert_eq!(tail.len(), 1);
    assert_eq!(tail[0]["line"], 2);
    assert_eq!(tail[0]["text"], "warning");
    std::fs::remove_file(&path).unwrap();
    let missing = s.autome(&["logs", id, "--json"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("没有原始流"));
    std::fs::create_dir(&path).unwrap();
    let unreadable = s.autome(&["logs", id, "--json"]);
    assert_eq!(unreadable.status.code(), Some(1));
    assert!(unreadable.stdout.is_empty());

    let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();
    let mut live = store.list_sessions(id).unwrap().pop().unwrap();
    live.id = "live-stream".into();
    live.lifecycle = autome_domain::session::SessionLifecycle::Running;
    live.ended_at = None;
    live.log_path = s.root.join("live.log").to_str().unwrap().into();
    store.insert_session(&live).unwrap();
    let live_args = ["logs", id, "--session", "live-", "--json"];
    assert!(s.ok(&live_args).is_empty()); // Not created yet.
    let live_path = Path::new(&live.log_path).with_extension("jsonl");
    std::fs::write(&live_path, b"one\ntwo").unwrap();
    let running = parse(s.ok(&live_args));
    assert_eq!(running.len(), 1);
    assert_eq!(running[0]["text"], "one");
    std::fs::write(&live_path, b"one\ntwo\n").unwrap();
    assert_eq!(parse(s.ok(&live_args)).len(), 2);
    std::fs::write(&live_path, b"one\ntwo").unwrap();
    store
        .finish_session(
            &live.id,
            &autome_domain::session::SessionLifecycle::Exited { exit_code: 0 },
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
    assert_eq!(parse(s.ok(&live_args)).len(), 2);
}

fn cost_fixture(tag: &str) -> (Sandbox, String) {
    use autome_domain::role::{Role, Runtime};
    use autome_domain::session::{Session, SessionKind, SessionLifecycle};
    let s = Sandbox::new(tag);
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    let created = s.json(&["task", "new", "cost fixture", "--draft"]);
    let id = created["task"]["id"].as_str().unwrap().to_owned();
    let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();
    for (n, cost) in [Some(1.25), Some(2.5), None, Some(99.0)]
        .into_iter()
        .enumerate()
    {
        let running = n == 3;
        store
            .insert_session(&Session {
                id: format!("cost-{n}"),
                task_id: id.clone(),
                kind: SessionKind::Role { role: Role::Impl },
                runtime: Runtime::Claude,
                model: "fixture".into(),
                effort: None,
                skills: vec![],
                round: n as u32 + 1,
                started_at: format!("2026-01-0{}T00:00:00Z", n + 1),
                ended_at: (!running).then(|| format!("2026-01-0{}T00:01:00Z", n + 1)),
                lifecycle: if running {
                    SessionLifecycle::Running
                } else {
                    SessionLifecycle::Exited { exit_code: 0 }
                },
                log_path: String::new(),
                pid: None,
                limit_minutes: None,
                protocol_ref: None,
                rules_hash: None,
                metrics: Default::default(),
            })
            .unwrap();
        store
            .set_session_metrics(
                &format!("cost-{n}"),
                &autome_domain::metrics::SessionMetrics {
                    cost_usd: cost,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    (s, id)
}

#[test]
fn cost_cli_status_task_prints_the_cost_line_under_state() {
    let (s, id) = cost_fixture("cost-detail");
    let output = s.ok(&["status", &id]);
    let lines: Vec<_> = output.lines().collect();
    let state = lines
        .iter()
        .position(|line| line.starts_with("  状态"))
        .unwrap();
    assert_eq!(
        lines[state + 1],
        "  费用      $3.75（3 个会话，其中 1 个无计费数据）"
    );
    let got = s.json(&["status", &id]);
    assert_eq!(got["cost"]["usd_total"], 3.75);
    assert_eq!(got["cost"]["sessions"], 3);
    assert_eq!(got["cost"]["sessions_without_cost"], 1);
    let empty = s.json(&["task", "new", "empty cost", "--draft"]);
    let output = s.ok(&["status", empty["task"]["id"].as_str().unwrap()]);
    assert!(
        output
            .lines()
            .any(|line| line == "  费用      $0.00（0 个会话，其中 0 个无计费数据）")
    );
}

#[test]
fn cost_cli_ls_appends_cost_and_a_footnote() {
    let (s, id) = cost_fixture("cost-list");
    let empty = s.json(&["task", "new", "empty cost", "--draft"]);
    let empty_id = empty["task"]["id"].as_str().unwrap();
    let output = s.ok(&["ls"]);
    assert!(
        output
            .lines()
            .find(|line| line.starts_with(&format!("{id}  ")))
            .unwrap()
            .ends_with("  $3.75*")
    );
    assert!(
        output
            .lines()
            .find(|line| line.starts_with(&format!("{empty_id}  ")))
            .unwrap()
            .ends_with("  $0.00")
    );
    assert_eq!(
        output.lines().last().unwrap(),
        "* 有会话没有计费数据（如 Codex），实际花费不低于所示金额"
    );
    let tasks = s.json(&["ls"]);
    let task = tasks
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .unwrap();
    assert_eq!(task["cost_usd"], 3.75);
    assert_eq!(task["cost_sessions_without_cost"], 1);
    let projects = s.json(&["project", "ls"]);
    assert_eq!(
        projects["projects"][0]["cost_usd_total"],
        serde_json::json!({"unfinished":3.75,"finished":0.0})
    );
    assert_eq!(projects["projects"][0]["cost_sessions_without_cost"], 1);
    let clean = Sandbox::new("cost-list-no-star");
    clean.ok(&[
        "project",
        "add",
        clean.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    assert!(!clean.ok(&["ls"]).contains("*"));
    clean.ok(&["task", "new", "no sessions", "--draft"]);
    let output = clean.ok(&["ls"]);
    assert!(!output.contains('*'));
    assert!(output.trim_end().ends_with("  $0.00"));
}

#[test]
fn cost_cli_running_row_shows_cost() {
    use autome_domain::task::{Node, TaskState};
    let (s, id) = cost_fixture("cost-running");
    let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();
    store
        .set_task_state(
            &id,
            &TaskState::Active {
                node: Node::Implement,
            },
        )
        .unwrap();
    let output = s.ok(&["status"]);
    let row = output
        .split("\n运行中\n")
        .nth(1)
        .unwrap()
        .lines()
        .find(|line| line.contains(&id))
        .unwrap();
    assert!(
        row.contains("运行中 · 实现  · $3.75*  · claude #4"),
        "{row}"
    );
    assert_eq!(s.json(&["status"])["running"][0]["task"]["cost_usd"], 3.75);
}

fn deps_sandbox(tag: &str) -> Sandbox {
    let s = Sandbox::new(tag);
    s.ok(&[
        "project",
        "add",
        s.repo.to_str().unwrap(),
        "--skip-onboarding",
    ]);
    s.ok(&["task", "new", "dependency-anchor", "--draft"]);
    s
}

fn deps_error(s: &Sandbox, args: &[&str], code: i32) {
    let out = s.autome(args);
    assert_eq!(
        out.status.code(),
        Some(code),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("autome："));
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .chars()
            .any(|c| ('一'..='龥').contains(&c))
    );
}

#[test]
fn deps_cli_create() {
    let s = deps_sandbox("deps-create");
    for name in ["T-1", "dependency-"] {
        let t = s.json(&["task", "new", &format!("wait-{name}"), "--after", name]);
        assert_eq!(t["task"]["state"]["state"], "queued");
        let id = t["task"]["id"].as_str().unwrap();
        let list = s.json(&["ls"]);
        let row = list
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == id)
            .unwrap();
        assert_eq!(row["after"], serde_json::json!(["T-1"]));
        assert_eq!(row["blocked_by"][0]["id"], "T-1");
        assert_eq!(row["blocked_by"][0]["state"]["state"], "draft");
        let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();
        assert_eq!(store.count_events(id, "task.after_set").unwrap(), 0);
    }
    deps_error(&s, &["task", "new", "self", "--after", "T-4"], 1);
    deps_error(&s, &["task", "new", "missing", "--after", "T-999"], 1);
    assert_eq!(s.json(&["ls"]).as_array().unwrap().len(), 3);
    for list in ["", ",T-1", "T-1,"] {
        deps_error(&s, &["task", "new", "empty", "--after", list], 2);
    }
    let eight = ["T-1"; 8].join(",");
    let t = s.json(&["task", "new", "eight", "--draft", "--after", &eight]);
    assert_eq!(t["task"]["after"], serde_json::json!(["T-1"]));
    deps_error(
        &s,
        &["task", "new", "nine", "--after", &["T-1"; 9].join(",")],
        1,
    );
    let t = s.json(&[
        "task",
        "new",
        "repeat",
        "--draft",
        "--after",
        "T-1",
        "--after",
        "dependency-",
    ]);
    assert_eq!(t["task"]["after"], serde_json::json!(["T-1"]));
}

#[test]
fn deps_cli_after() {
    let s = deps_sandbox("deps-after");
    s.ok(&["task", "new", "dependent", "--draft", "--after", "T-1"]);
    for args in [
        vec!["after", "T-2"],
        vec!["after", "T-2", "T-1", "--none"],
        vec!["after", "T-2", "T-1", "extra"],
    ] {
        deps_error(&s, &args, 2);
    }
    assert_eq!(
        s.json(&["status", "T-2"])["task"]["after"],
        serde_json::json!(["T-1"])
    );
    s.ok(&["after", "T-2", "--none"]);
    assert_eq!(
        s.json(&["status", "T-2"])["task"]["after"],
        serde_json::json!([])
    );
    s.ok(&["after", "T-2", "dependency-"]);
    assert_eq!(
        s.json(&["status", "T-2"])["task"]["after"],
        serde_json::json!(["T-1"])
    );
    deps_error(&s, &["after", "T-2", "T-2"], 1);
    deps_error(&s, &["after", "T-1", "T-2"], 1);
    s.ok(&["task", "start", "T-2"]);
    s.ok(&["after", "T-2", "--none"]);
    assert_ne!(s.task_state("T-2")["state"], "queued");
    s.ok(&["cancel", "T-2"]);
}

#[test]
fn deps_cli_queue() {
    let s = deps_sandbox("deps-queue");
    for name in ["first", "second", "third"] {
        s.ok(&["task", "new", name, "--after", "T-1"]);
    }
    for args in [
        vec!["queue", "move", "T-4"],
        vec!["queue", "move", "T-4", "--top", "--before", "T-2"],
        vec!["queue", "bad", "T-4", "--top"],
        vec!["queue", "move", "T-4", "--after", "T-2", "--after", "T-3"],
    ] {
        deps_error(&s, &args, 2);
    }
    for (args, order) in [
        (
            vec!["queue", "move", "T-4", "--top"],
            vec!["T-4", "T-2", "T-3"],
        ),
        (
            vec!["queue", "move", "T-4", "--after", "T-2"],
            vec!["T-2", "T-4", "T-3"],
        ),
        (
            vec!["queue", "move", "T-3", "--before", "T-2"],
            vec!["T-3", "T-2", "T-4"],
        ),
    ] {
        s.ok(&args);
        let list = s.json(&["ls"]);
        for (i, id) in order.iter().enumerate() {
            let t = list
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["id"] == *id)
                .unwrap();
            assert_eq!(t["queue_position"], i + 1);
        }
        let text = s.ok(&["ls"]);

        let section = text.split("排队（按出队顺序）").nth(1).unwrap();
        let lines: Vec<_> = section.lines().filter(|l| l.starts_with("T-")).collect();
        assert_eq!(
            lines
                .iter()
                .map(|l| l.split_whitespace().next().unwrap())
                .collect::<Vec<_>>(),
            order
        );
        assert!(lines.iter().all(|l| l.ends_with("← 等 T-1（待开始）")));
    }
    deps_error(&s, &["queue", "move", "T-2", "--before", "T-2"], 1);
    s.ok(&["cancel", "T-1"]);
    assert!(s.ok(&["ls"]).contains("← 等 T-1（已取消）"));
}

#[test]
fn deps_cli_status() {
    let s = deps_sandbox("deps-status");
    assert!(!s.ok(&["status", "T-1"]).contains("依赖"));
    assert!(!s.ok(&["status"]).contains("\n排队"));
    s.ok(&["task", "new", "waiting", "--after", "T-1"]);
    assert!(s.ok(&["status", "T-2"]).contains("依赖      T-1（待开始）"));
    let status = s.ok(&["status"]);
    assert!(status.contains("\n排队"));
    assert!(status.contains("排队 · 第 1 位  ← 等 T-1（待开始）"));
    assert_eq!(
        s.json(&["status"])["queued"][0]["task"]["after"],
        serde_json::json!(["T-1"])
    );
    s.ok(&["cancel", "T-1"]);
    assert!(s.ok(&["status", "T-2"]).contains("依赖      T-1（已取消）"));
    let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();
    store
        .set_task_state("T-1", &autome_domain::task::TaskState::Done)
        .unwrap();
    assert!(s.ok(&["status", "T-2"]).contains("依赖      T-1（已完成）"));
}

#[test]
fn build_id_version_line_matches_head() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .current_dir(dir)
            .env("TZ", "UTC")
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    let count: u64 = git(&["rev-list", "--count", "HEAD"]).parse().unwrap();
    let date = git(&[
        "show",
        "-s",
        "--date=format-local:%Y%m%dT%H%M%SZ",
        "--format=%cd",
        "HEAD",
    ]);
    let sha = git(&["rev-parse", "--short=12", "HEAD"]);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_autome"))
        .arg("--version")
        .env(
            "AUTOME_HOME",
            std::env::temp_dir().join(format!("autome-version-{}", std::process::id())),
        )
        .output()
        .unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        format!(
            "autome {} ({count:07}-{date}-{sha})",
            automed::entry::VERSION
        )
    );
}

#[test]
fn build_id_cli_and_kernel_agree() {
    let cli = std::path::Path::new(env!("CARGO_BIN_EXE_autome"));
    let version = |path: &std::path::Path| {
        let out = std::process::Command::new(path)
            .arg("--version")
            .env(
                "AUTOME_HOME",
                std::env::temp_dir().join(format!("autome-version-{}", std::process::id())),
            )
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout)
            .unwrap()
            .split_once(' ')
            .unwrap()
            .1
            .to_owned()
    };
    assert_eq!(version(cli), version(&cli.with_file_name("automed")));
}

const OLD_KERNEL: &str = "0000001-20200101T000000Z-000000000000";

fn kernel_command(s: &Sandbox, args: &[&str], old: bool) -> std::process::Command {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_autome"));
    c.args(args)
        .current_dir(&s.repo)
        .env("AUTOME_HOME", s.root.join("home"))
        .env("AUTOMED_DB_PATH", s.root.join("home/ledger.sqlite3"))
        .env("AUTOMED_CLAUDE_BINARY", &s.fake_cli)
        .env("AUTOMED_CODEX_BINARY", &s.fake_cli)
        .env_remove("AUTOME_ALLOW_OLDER_KERNEL")
        .env_remove("AUTOME_TEST_BUILD_ID");
    if old {
        c.env("AUTOME_TEST_BUILD_ID", OLD_KERNEL);
    }
    c
}

fn kernel_seed(s: &Sandbox) -> String {
    let out = kernel_command(s, &["run", "--once"], false)
        .output()
        .unwrap();
    assert!(out.status.success());
    automed::build_id::older_kernel_message(OLD_KERNEL, automed::build_id::BUILD_ID)
}

struct KernelRun {
    child: std::process::Child,
    out: PathBuf,
    err: PathBuf,
}
impl KernelRun {
    fn start(s: &Sandbox, json: bool) -> Self {
        let out = s.root.join("run.out");
        let err = s.root.join("run.err");
        let mut c = kernel_command(s, &["run", "--interval", "0.05"], true);
        if json {
            c.arg("--json");
        }
        let child = c
            .stdout(std::fs::File::create(&out).unwrap())
            .stderr(std::fs::File::create(&err).unwrap())
            .spawn()
            .unwrap();
        Self { child, out, err }
    }
    fn text(&self, json: bool) -> String {
        std::fs::read_to_string(if json { &self.out } else { &self.err }).unwrap()
    }
    fn wait_for(&mut self, json: bool, text: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !self.text(json).contains(text) {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "driver exited: {}",
                self.text(false)
            );
            assert!(
                std::time::Instant::now() < deadline,
                "missing {text}: {}",
                self.text(json)
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    fn stop(&mut self) {
        assert!(self.child.try_wait().unwrap().is_none());
        assert_eq!(
            unsafe { libc::kill(self.child.id() as i32, libc::SIGINT) },
            0
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(std::time::Instant::now() < deadline, "SIGINT timeout");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}
impl Drop for KernelRun {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn kernel_guard_run_says_it_once_and_stays_up() {
    let s = Sandbox::new("kernel-quiet");
    let message = kernel_seed(&s);
    let mut run = KernelRun::start(&s, false);
    run.wait_for(false, &message);
    // Observe many ticks: this duration is the behavior being measured.
    std::thread::sleep(std::time::Duration::from_secs(2));
    run.stop();
    let text = run.text(false);
    assert_eq!(text.matches(&message).count(), 1, "{text}");
    assert!(text.lines().count() <= 3, "{text}");
    assert!(!text.contains("另一个驱动"));
}

#[test]
fn kernel_guard_run_once_and_until_idle_exit_nonzero() {
    let s = Sandbox::new("kernel-once");
    let message = kernel_seed(&s);
    for mode in ["--once", "--until-idle"] {
        let out = kernel_command(&s, &["run", mode], true).output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(err.matches(&message).count(), 1);
        assert!(!err.contains("另一个驱动"));
    }
}

#[test]
fn kernel_guard_run_json_reports_older_kernel() {
    let s = Sandbox::new("kernel-json");
    let message = kernel_seed(&s);
    for mode in ["--once", "--until-idle"] {
        let out = kernel_command(&s, &["run", mode, "--json"], true)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["older_kernel"]["message"], message);
        assert_eq!(v["older_kernel"]["kernel"], OLD_KERNEL);
        assert_eq!(v["older_kernel"]["ledger"], automed::build_id::BUILD_ID);
        assert!(out.stderr.is_empty());
    }
    let mut run = KernelRun::start(&s, true);
    run.wait_for(true, &message);
    std::thread::sleep(std::time::Duration::from_secs(2));
    run.stop();
    assert_eq!(run.text(true).lines().count(), 1);
    assert!(run.text(false).is_empty());
}

#[test]
fn kernel_guard_run_resumes_when_the_ledger_allows_it() {
    for json in [false, true] {
        let s = Sandbox::new(if json {
            "kernel-resume-json"
        } else {
            "kernel-resume"
        });
        let message = kernel_seed(&s);
        let mut run = KernelRun::start(&s, json);
        run.wait_for(json, &message);
        let newer = "9999999-29991231T000000Z-ffffffffffff";
        let store = automed::store::Store::open(s.root.join("home/ledger.sqlite3")).unwrap();
        assert_eq!(store.claim_kernel_mark(newer).unwrap(), None);
        drop(store);
        let changed = automed::build_id::older_kernel_message(OLD_KERNEL, newer);
        run.wait_for(json, &changed);
        let out = kernel_command(&s, &["run", "--once"], true)
            .env("AUTOME_ALLOW_OLDER_KERNEL", "1")
            .output()
            .unwrap();
        assert!(out.status.success());
        let resumed = if json {
            "\"older_kernel\":null"
        } else {
            "账本已允许本内核推进，继续驱动"
        };
        run.wait_for(json, resumed);
        // A new kernel raises the mark again; the same process must stop again.
        kernel_seed(&s);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while run.text(json).matches(&message).count() < 2 {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        run.stop();
        assert_eq!(run.text(json).matches(resumed).count(), 1);
        assert_eq!(run.text(json).matches(&changed).count(), 1);
        assert_eq!(run.text(json).matches(&message).count(), 2);
    }
}

#[test]
fn kernel_guard_write_commands_refused_with_the_sentence() {
    let s = Sandbox::new("kernel-write");
    s.ok(&["project", "add", ".", "--skip-onboarding"]);
    s.ok(&["task", "new", "anchor", "--draft"]);
    let message = kernel_seed(&s);
    let before = s.json(&["ls"]);
    for args in [
        vec!["task", "new", "blocked", "--draft"],
        vec!["approve", "anchor"],
        vec!["merge", "anchor"],
        vec!["rerun", "anchor", "impl"],
    ] {
        let out = kernel_command(&s, &args, true).output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&out.stderr),
            format!("autome：{message}\n")
        );
    }
    assert_eq!(s.json(&["ls"]), before);
}

#[test]
fn kernel_guard_core_subcommand_serves_stdio() {
    use automed::ipc::{read_frame, write_frame};
    use std::process::Stdio;
    let s = Sandbox::new("kernel-core");
    for old in [false, true] {
        let mut child = kernel_command(&s, &["core"], old)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let command = serde_json::json!({"request_id":"test", "command_id":"test", "protocol_version":automed::ipc::PROTOCOL_VERSION, "method":"env.get", "params":{}});
        write_frame(
            &mut child.stdin.take().unwrap(),
            &serde_json::to_vec(&command).unwrap(),
        )
        .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        let frame = read_frame(&mut output.stdout.as_slice(), 8 * 1024 * 1024).unwrap();
        let reply: Value = serde_json::from_slice(&frame).unwrap();
        let version = kernel_command(&s, &["--version"], old).output().unwrap();
        let id = if old {
            OLD_KERNEL
        } else {
            automed::build_id::BUILD_ID
        };
        assert!(String::from_utf8_lossy(&version.stdout).contains(id));
        assert_eq!(
            reply["outcome"]["payload"]["kernel"]["build_id"], id,
            "{reply}"
        );
    } // EOF is clean, malformed commands receive an error frame, and opening
    // an unreadable ledger fails without contaminating the frame channel.
    let out = kernel_command(&s, &["core"], false).output().unwrap();
    assert!(out.status.success() && out.stdout.is_empty());
    let mut child = kernel_command(&s, &["core"], false)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    write_frame(&mut child.stdin.take().unwrap(), b"{}").unwrap();
    let out = child.wait_with_output().unwrap();
    let frame = read_frame(&mut out.stdout.as_slice(), 8 * 1024 * 1024).unwrap();
    let reply: Value = serde_json::from_slice(&frame).unwrap();
    assert_eq!(reply["outcome"]["status"], "error");
    let out = kernel_command(&s, &["core"], false)
        .env("AUTOMED_DB_PATH", &s.repo)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn kernel_guard_doctor_shows_both_ids() {
    let s = Sandbox::new("kernel-doctor");
    let empty = kernel_command(&s, &["doctor", "--json"], false)
        .output()
        .unwrap();
    let empty: Value = serde_json::from_slice(&empty.stdout).unwrap();
    assert!(empty["kernel"]["ledger_build_id"].is_null());
    let message = kernel_seed(&s);
    let current = kernel_command(&s, &["doctor"], false).output().unwrap();
    let old = kernel_command(&s, &["doctor"], true).output().unwrap();
    assert_eq!(current.status.code(), old.status.code());
    let text = String::from_utf8_lossy(&old.stdout);
    assert!(
        text.contains(OLD_KERNEL)
            && text.contains(automed::build_id::BUILD_ID)
            && text.contains(&message)
    );
    let out = kernel_command(&s, &["doctor", "--json"], true)
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["kernel"]["build_id"], OLD_KERNEL);
    assert_eq!(v["kernel"]["ledger_build_id"], automed::build_id::BUILD_ID);
    assert_eq!(v["kernel"]["may_advance"], false);
    assert_eq!(v["kernel"]["message"], message);
}

#[test]
fn kernel_guard_test_override_cannot_raise_the_id() {
    let s = Sandbox::new("kernel-override");
    for candidate in ["9999999-29991231T000000Z-ffffffffffff", "", "invalid"] {
        let out = kernel_command(&s, &["--version"], false)
            .env("AUTOME_TEST_BUILD_ID", candidate)
            .output()
            .unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout).contains(automed::build_id::BUILD_ID));
        assert!(String::from_utf8_lossy(&out.stderr).contains("忽略 AUTOME_TEST_BUILD_ID"));
    }
}
