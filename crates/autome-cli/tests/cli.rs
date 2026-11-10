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
    assert_eq!(s.task_state(id)["node"], "await_design_approval");
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
    for kind in ["feature", "bug"] {
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
