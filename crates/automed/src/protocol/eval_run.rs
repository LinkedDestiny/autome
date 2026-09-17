//! Layer 3: running a case against a real CLI.
//!
//! This is the only layer that spends money, so it is the only one that is
//! never run automatically. Layers 1 and 2 are part of an implementation
//! round's acceptance command; this one runs at the audit round of a meta
//! task, and `--changed` narrows it to the cases the proposed changes
//! reference plus three baselines.
//!
//! ## What a run is
//!
//! A scratch directory, the case's scaffold script, one session of the role
//! the case names, then one grader session per grader file. The role session
//! is started the same way a real one is — same adapter flags, same prompt
//! template from the version under test, prompt on stdin — because a case
//! that ran the CLI differently from production would be testing something
//! else.
//!
//! ## What a grader sees
//!
//! The worktree after the round, the diff against before, and the transcript.
//! It answers `VERDICT: 1` or `VERDICT: 0` and nothing else matters. A model
//! is doing the grading, which is why every grader file is required to carry a
//! positive control — a described negative sample it must mark 0 — and why the
//! verdict is a majority over `runs` rather than a single draw.
//!
//! ## What bounds it
//!
//! `timeout_seconds` per session, and `max_turns` checked *after* the fact
//! against the turn count in the stream. Neither CLI has a turn flag (checked
//! 2026-09-17), so a run that needed more turns than the case allows is
//! reported as a failure of the case rather than silently accepted: a case
//! that suddenly needs twenty turns is no longer asserting what it was written
//! to assert.

use std::path::{Path, PathBuf};

use autome_domain::protocol::ProtocolFiles;
use autome_domain::role::Runtime;

use super::case::Case;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraderVerdict {
    pub grader: String,
    /// One entry per run, in order.
    pub votes: Vec<bool>,
}

impl GraderVerdict {
    pub fn passed(&self) -> bool {
        let yes = self.votes.iter().filter(|v| **v).count();
        yes * 2 > self.votes.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseResult {
    pub name: String,
    pub graders: Vec<GraderVerdict>,
    /// Runs that could not be completed at all, with the reason.
    pub errors: Vec<String>,
    /// Runs whose turn count exceeded the case's cap.
    pub over_budget: Vec<u64>,
}

impl CaseResult {
    pub fn passed(&self) -> bool {
        self.errors.is_empty()
            && self.over_budget.is_empty()
            && !self.graders.is_empty()
            && self.graders.iter().all(GraderVerdict::passed)
    }

    pub fn render(&self) -> String {
        let mut s = format!(
            "{} {}\n",
            if self.passed() { "✓" } else { "✗" },
            self.name
        );
        for g in &self.graders {
            s.push_str(&format!(
                "    {} {} [{}]\n",
                if g.passed() { "✓" } else { "✗" },
                g.grader,
                g.votes
                    .iter()
                    .map(|v| if *v { "1" } else { "0" })
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        for t in &self.over_budget {
            s.push_str(&format!("    ✗ 用了 {t} 个 turn，超过用例允许的上限\n"));
        }
        for e in &self.errors {
            s.push_str(&format!("    ! {e}\n"));
        }
        s
    }
}

/// How to run the sessions. Split out so a test can substitute something that
/// does not cost money, and so the real one stays a thin shell over the same
/// adapter table production uses.
pub trait Runner {
    /// Runs one role session in `dir` with `prompt`, returning the raw JSONL
    /// stream. Errors are a failure of the run, not of the case.
    fn role_session(
        &self,
        dir: &Path,
        runtime: Runtime,
        prompt: &str,
        timeout_seconds: u64,
    ) -> Result<String, String>;

    /// Runs one grading session and returns its text output.
    fn grader_session(&self, prompt: &str, timeout_seconds: u64) -> Result<String, String>;
}

/// The real one: the CLIs, invoked the way the launcher invokes them.
pub struct CliRunner {
    /// Which runtime grades.
    ///
    /// Set to the other side from the round being graded, for the same reason
    /// SAME-MODEL exists: a model marking its own homework agrees with itself.
    /// An eval is not a Loop round and the core does not enforce this, but
    /// choosing otherwise would be choosing a weaker check on purpose.
    pub grader_runtime: Runtime,
}

impl CliRunner {
    pub fn grading_against(role_runtime: Runtime) -> Self {
        CliRunner {
            grader_runtime: match role_runtime {
                Runtime::Claude => Runtime::Codex,
                Runtime::Codex => Runtime::Claude,
            },
        }
    }
}

impl Runner for CliRunner {
    fn role_session(
        &self,
        dir: &Path,
        runtime: Runtime,
        prompt: &str,
        timeout_seconds: u64,
    ) -> Result<String, String> {
        run_cli(dir, runtime, prompt, timeout_seconds)
    }

    fn grader_session(&self, prompt: &str, timeout_seconds: u64) -> Result<String, String> {
        let dir = std::env::temp_dir();
        run_cli(&dir, self.grader_runtime, prompt, timeout_seconds)
    }
}

fn run_cli(
    dir: &Path,
    runtime: Runtime,
    prompt: &str,
    timeout_seconds: u64,
) -> Result<String, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let adapter = crate::launcher::adapter(runtime);
    let binary = crate::launcher::resolve_binary(runtime)
        .ok_or_else(|| format!("找不到 {}", adapter.binary))?;
    let mut child = Command::new(&binary)
        .args(adapter.autonomous_flags)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动 {binary}：{e}"))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(prompt.as_bytes())
        .map_err(|e| format!("无法写入 prompt：{e}"))?;

    // The wrapper script's timeout is the terminal's; here the process is ours
    // to wait on, so the bound is a plain poll.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_seconds);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    return Err(format!("超过 {timeout_seconds} 秒仍未结束"));
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            Err(e) => return Err(format!("等待 {binary} 失败：{e}")),
        }
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("读取 {binary} 输出失败：{e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Everything a run needs that is not the case itself.
pub struct Plan<'a> {
    /// The version under test. The role session gets *its* prompt template,
    /// which is the whole point.
    pub files: &'a ProtocolFiles,
    /// Where the case's files live: `case.yaml`, `scaffold.sh`, `fixture/`,
    /// `graders/`.
    pub case_dir: PathBuf,
    /// The slug the fixture uses, for rendering the role prompt.
    pub slug: String,
    /// Which CLI the role runs on. The case has to exercise the runtime the
    /// role is actually configured for, or it is testing a different product.
    pub role_runtime: Runtime,
}

/// Runs one case `case.runs` times and reports the majority verdict.
pub fn run_case(plan: &Plan<'_>, case: &Case, runner: &dyn Runner) -> CaseResult {
    let mut result = CaseResult {
        name: case.name.clone(),
        graders: case
            .graders
            .iter()
            .map(|g| GraderVerdict {
                grader: g.clone(),
                votes: Vec::new(),
            })
            .collect(),
        errors: Vec::new(),
        over_budget: Vec::new(),
    };

    let Some(template) = plan.files.prompt(case.role.as_str()) else {
        result.errors.push(format!(
            "本版本里没有 prompts/{}.md，用例跑不了",
            case.role.as_str()
        ));
        return result;
    };
    let prompt = render_prompt(template, &plan.slug);

    for run in 0..case.runs {
        let dir = match scaffold(plan, case, run) {
            Ok(d) => d,
            Err(e) => {
                result.errors.push(e);
                continue;
            }
        };
        let before = snapshot(&dir);
        let stream = match runner.role_session(
            &dir,
            plan.role_runtime,
            &prompt,
            case.timeout_seconds,
        ) {
            Ok(s) => s,
            Err(e) => {
                result.errors.push(format!("第 {} 次：{e}", run + 1));
                let _ = std::fs::remove_dir_all(&dir);
                continue;
            }
        };

        // `max_turns` after the fact: neither CLI takes a flag for it, and a
        // case that needs more turns than it was written for is no longer
        // asserting what it was written to assert.
        let metrics = crate::usage::parse(plan.role_runtime, &stream, None);
        if let Some(turns) = metrics.turns
            && turns > case.max_turns as u64
        {
            result.over_budget.push(turns);
        }

        let after = snapshot(&dir);
        let transcript = readable(plan.role_runtime, &stream);
        for (i, grader) in case.graders.iter().enumerate() {
            let body = std::fs::read_to_string(plan.case_dir.join(grader)).unwrap_or_default();
            let ask = grader_prompt(&body, &before, &after, &transcript);
            match runner.grader_session(&ask, case.timeout_seconds) {
                Ok(text) => result.graders[i].votes.push(verdict(&text)),
                Err(e) => result
                    .errors
                    .push(format!("第 {} 次 {grader}：{e}", run + 1)),
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
    result
}

/// Fills the placeholders a case's role prompt uses.
///
/// A case exercises one round in isolation: there is no brief, no budget and
/// no metrics, and the prompt has to say so rather than leave a literal
/// `{brief_path}` in front of the model.
fn render_prompt(template: &str, slug: &str) -> String {
    template
        .replace("{slug}", slug)
        .replace("{budget_line}", "")
        .replace(
            "{brief_path}",
            &format!("docs/{slug}/brief/（本次评估没有简报，直接读任务目录）"),
        )
        .replace("{task_metrics}", "（本次评估没有指标）")
        .replace(
            "{metric_vocabulary}",
            &autome_domain::metrics::TaskMetrics::METRIC_NAMES.join(" / "),
        )
        .replace("{request}", "")
        .replace("{inputs}", "")
        .replace("{design_rounds}", "15")
}

fn scaffold(plan: &Plan<'_>, case: &Case, run: u32) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "autome-eval-{}-{}-{}",
        case.name,
        std::process::id(),
        run
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("无法创建 {}：{e}", dir.display()))?;

    let script = plan.case_dir.join(&case.scaffold);
    let out = std::process::Command::new("sh")
        .arg(&script)
        .current_dir(&dir)
        .output()
        .map_err(|e| format!("无法运行 {}：{e}", script.display()))?;
    if !out.status.success() {
        return Err(format!(
            "{} 失败：{}",
            script.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(dir)
}

/// Every text file under a directory, for the before/after the grader reads.
fn snapshot(dir: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else if let Ok(text) = std::fs::read_to_string(&path)
                && let Ok(rel) = path.strip_prefix(root)
            {
                out.push((rel.to_string_lossy().to_string(), text));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The stream as a person would read it, which is also what a grader can judge
/// tool calls from.
fn readable(runtime: Runtime, stream: &str) -> String {
    stream
        .lines()
        .filter_map(|l| match runtime {
            Runtime::Claude => crate::stream_render::render_line(l),
            Runtime::Codex => crate::stream_render::render_codex_line(l),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn grader_prompt(
    body: &str,
    before: &[(String, String)],
    after: &[(String, String)],
    transcript: &str,
) -> String {
    let render = |files: &[(String, String)]| -> String {
        files
            .iter()
            .map(|(p, c)| format!("### {p}\n\n```\n{}\n```", truncate(c, 8000)))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let changed: Vec<String> = after
        .iter()
        .filter(|(p, c)| !before.iter().any(|(bp, bc)| bp == p && bc == c))
        .map(|(p, _)| p.clone())
        .collect();

    format!(
        "你是一个评分器。按下面的判定标准给出 0 或 1，**最后一行只写 \
         `VERDICT: 1` 或 `VERDICT: 0`**，之前可以写一两句理由。\n\n\
         判定标准里如果有「机检优先」，就按它说的去核对文件内容和 transcript，\
         不要凭印象。判定标准里的「阳性对照」描述的是应当判 0 的样本，\
         用它检查你自己的判断。\n\n\
         # 判定标准\n\n{body}\n\n\
         # 这一轮改动了哪些文件\n\n{}\n\n\
         # 这一轮结束后的工作区\n\n{}\n\n\
         # transcript\n\n```\n{}\n```\n",
        if changed.is_empty() {
            "（一个文件都没改）".to_string()
        } else {
            changed.join("\n")
        },
        render(after),
        truncate(transcript, 20000)
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max / 2).collect();
    let tail: String = s
        .chars()
        .skip(s.chars().count() - max / 2)
        .collect::<String>();
    format!("{head}\n…（中间省略）…\n{tail}")
}

/// Reads the verdict off a grader's answer.
///
/// The last `VERDICT:` line wins, because a grader that reasons out loud will
/// often quote the format before using it. Anything unreadable is a 0: a
/// grader that could not answer has not passed the round.
pub fn verdict(text: &str) -> bool {
    text.lines()
        .rev()
        .find_map(|l| {
            let t = l.trim();
            t.strip_prefix("VERDICT:").map(|v| v.trim() == "1")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::role::Role;
    use std::cell::RefCell;

    struct Fake {
        /// One canned stream per role session, consumed in order.
        streams: RefCell<Vec<Result<String, String>>>,
        /// One canned answer per grader session, consumed in order.
        answers: RefCell<Vec<Result<String, String>>>,
        /// What the role session was handed, for assertions.
        pub seen: RefCell<Vec<String>>,
    }

    impl Fake {
        fn new(streams: Vec<Result<String, String>>, answers: Vec<Result<String, String>>) -> Self {
            Fake {
                streams: RefCell::new(streams),
                answers: RefCell::new(answers),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl Runner for Fake {
        fn role_session(
            &self,
            _dir: &Path,
            _runtime: Runtime,
            prompt: &str,
            _t: u64,
        ) -> Result<String, String> {
            self.seen.borrow_mut().push(prompt.to_string());
            let mut s = self.streams.borrow_mut();
            if s.is_empty() {
                return Ok(String::new());
            }
            s.remove(0)
        }
        fn grader_session(&self, _prompt: &str, _t: u64) -> Result<String, String> {
            let mut a = self.answers.borrow_mut();
            if a.is_empty() {
                return Ok("VERDICT: 1".into());
            }
            a.remove(0)
        }
    }

    fn stream(turns: u64) -> String {
        format!(
            r#"{{"type":"result","subtype":"success","num_turns":{turns},"usage":{{"input_tokens":1,"output_tokens":1}}}}"#
        )
    }

    /// The name is per test on purpose: the scratch directory is derived from
    /// it, and two tests sharing a name delete each other's fixtures when the
    /// suite runs in parallel.
    fn case(name: &str, runs: u32, max_turns: u32) -> Case {
        Case {
            name: name.into(),
            role: Role::Impl,
            description: String::new(),
            runs,
            max_turns,
            timeout_seconds: 5,
            scaffold: "./scaffold.sh".into(),
            graders: vec!["graders/a.md".into()],
        }
    }

    /// A case directory with a scaffold that writes one file.
    fn case_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("autome-case-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("graders")).unwrap();
        std::fs::write(
            dir.join("scaffold.sh"),
            "set -e\nmkdir -p docs/demo\nprintf '起点\\n' > docs/demo/demo.md\n",
        )
        .unwrap();
        std::fs::write(dir.join("graders/a.md"), "判定目标：…\n阳性对照：…\n").unwrap();
        dir
    }

    fn plan<'a>(files: &'a ProtocolFiles, dir: &Path) -> Plan<'a> {
        Plan {
            files,
            case_dir: dir.to_path_buf(),
            slug: "demo".into(),
            role_runtime: Runtime::Claude,
        }
    }

    #[test]
    fn a_case_passes_when_the_majority_of_runs_pass() {
        let files = crate::protocol::seed();
        let dir = case_dir("majority");
        let fake = Fake::new(
            vec![Ok(stream(3)), Ok(stream(3)), Ok(stream(3))],
            vec![
                Ok("VERDICT: 1".into()),
                Ok("理由\nVERDICT: 0".into()),
                Ok("VERDICT: 1".into()),
            ],
        );
        let r = run_case(&plan(&files, &dir), &case("majority", 3, 15), &fake);
        assert!(r.passed(), "{}", r.render());
        assert_eq!(r.graders[0].votes, vec![true, false, true]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_case_fails_when_the_majority_of_runs_fail() {
        let files = crate::protocol::seed();
        let dir = case_dir("minority");
        let fake = Fake::new(
            vec![Ok(stream(3)), Ok(stream(3)), Ok(stream(3))],
            vec![
                Ok("VERDICT: 0".into()),
                Ok("VERDICT: 1".into()),
                Ok("VERDICT: 0".into()),
            ],
        );
        let r = run_case(&plan(&files, &dir), &case("minority", 3, 15), &fake);
        assert!(!r.passed(), "{}", r.render());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_that_needed_more_turns_than_the_case_allows_fails_it() {
        // A case is written to assert the first few steps of a round. One that
        // suddenly needs twenty turns is asserting something else.
        let files = crate::protocol::seed();
        let dir = case_dir("budget");
        let fake = Fake::new(vec![Ok(stream(40))], vec![Ok("VERDICT: 1".into())]);
        let r = run_case(&plan(&files, &dir), &case("budget", 1, 15), &fake);
        assert!(!r.passed(), "{}", r.render());
        assert_eq!(r.over_budget, vec![40]);
        assert!(r.render().contains("40 个 turn"), "{}", r.render());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_session_that_would_not_start_is_an_error_not_a_verdict() {
        // "The CLI is missing" must not read as "the protocol failed".
        let files = crate::protocol::seed();
        let dir = case_dir("broken");
        let fake = Fake::new(vec![Err("找不到 claude".into())], vec![]);
        let r = run_case(&plan(&files, &dir), &case("broken", 1, 15), &fake);
        assert!(!r.passed());
        assert_eq!(r.errors.len(), 1, "{r:?}");
        assert!(r.errors[0].contains("找不到 claude"), "{r:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_round_is_handed_the_template_from_the_version_under_test() {
        // The whole point: a case run against v7 must exercise v7's prompt.
        let mut files = crate::protocol::seed();
        files.insert("prompts/impl.md", "第 7 版的实现轮 prompt，{slug}。");
        let dir = case_dir("template");
        let fake = Fake::new(vec![Ok(stream(1))], vec![Ok("VERDICT: 1".into())]);
        run_case(&plan(&files, &dir), &case("template", 1, 15), &fake);
        let seen = fake.seen.borrow();
        assert_eq!(seen.len(), 1);
        assert!(seen[0].contains("第 7 版的实现轮 prompt，demo。"), "{}", seen[0]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_placeholder_reaches_the_model_even_though_a_case_has_no_brief() {
        let files = crate::protocol::seed();
        let rendered = render_prompt(files.prompt("impl").unwrap(), "demo");
        assert!(!rendered.contains("{brief_path}"), "{rendered}");
        assert!(!rendered.contains("{budget_line}"), "{rendered}");
        assert!(!rendered.contains("{slug}"), "{rendered}");
    }

    #[test]
    fn a_version_without_the_role_template_reports_that_rather_than_running() {
        let mut files = crate::protocol::seed();
        files.remove("prompts/impl.md");
        let dir = case_dir("missing");
        let fake = Fake::new(vec![], vec![]);
        let r = run_case(&plan(&files, &dir), &case("missing", 1, 15), &fake);
        assert!(!r.passed());
        assert!(r.errors[0].contains("prompts/impl.md"), "{r:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_grader_sits_on_the_other_runtime_from_the_round_it_grades() {
        // Not enforced by the core — an eval is not a Loop round — but
        // choosing the same model on both sides would be choosing a weaker
        // check on purpose.
        assert_eq!(
            CliRunner::grading_against(Runtime::Claude).grader_runtime,
            Runtime::Codex
        );
        assert_eq!(
            CliRunner::grading_against(Runtime::Codex).grader_runtime,
            Runtime::Claude
        );
    }

    #[test]
    fn the_last_verdict_line_is_the_one_that_counts() {
        // A grader reasoning out loud often quotes the format before using it.
        assert!(verdict("先说格式是 VERDICT: 0\n结论\nVERDICT: 1"));
        assert!(!verdict("VERDICT: 1 是通过\n最终 VERDICT: 0"));
    }

    #[test]
    fn an_unreadable_answer_is_a_zero() {
        // A grader that could not answer has not passed the round.
        assert!(!verdict("我不确定。"));
        assert!(!verdict(""));
    }

    #[test]
    fn a_grader_is_shown_what_changed_the_result_and_the_transcript() {
        let before = vec![("a.md".to_string(), "旧".to_string())];
        let after = vec![
            ("a.md".to_string(), "新".to_string()),
            ("b.md".to_string(), "新增".to_string()),
        ];
        let p = grader_prompt("判定标准正文", &before, &after, "→ Bash cargo test");
        assert!(p.contains("判定标准正文"), "{p}");
        assert!(p.contains("a.md"), "{p}");
        assert!(p.contains("b.md"), "{p}");
        assert!(p.contains("→ Bash cargo test"), "{p}");
        assert!(p.contains("VERDICT: 1"), "{p}");
    }

    #[test]
    fn a_run_that_changed_nothing_says_so_rather_than_showing_an_empty_list() {
        let files = vec![("a.md".to_string(), "同".to_string())];
        let p = grader_prompt("x", &files, &files, "");
        assert!(p.contains("一个文件都没改"), "{p}");
    }
}
