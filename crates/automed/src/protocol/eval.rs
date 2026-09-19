//! `autome protocol eval` — the gate a protocol version has to pass.
//!
//! Three layers, cheapest first (plan §6.5):
//!
//! 1. **Static.** The kernel contract, the size budget, the required phrases,
//!    the prompt placeholders, and the changelog's own rules. Milliseconds.
//! 2. **Examples parse.** Every status block and milestone table *printed in
//!    the protocol as an example* is fed to the real parser. A protocol that
//!    shows a round a format the core cannot read is worse than one that shows
//!    none — and this is not hypothetical: the 2026-09-16 failure was a table
//!    the document's own reader could not parse.
//! 3. **Behaviour.** The eval cases, run against real CLIs. Lives in
//!    [`super::eval_run`]; this module stops before spending money.
//!
//! The plan proposed a fourth layer — "re-parse the real design documents of
//! past runs" — and it is not here. What it tested was the parser, not the
//! protocol text, and the contract-region hashes already cover the part of the
//! text the parser depends on.

use autome_domain::changelog::{self, ChangeKind, Changelog};
use autome_domain::protocol::{self, ProtocolFiles};
use autome_domain::status_block;

use super::phrases;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The version cannot be released.
    Fail,
    /// Worth reading before releasing it.
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub severity: Severity,
    /// Which layer found it, for the report's grouping.
    pub layer: &'static str,
    pub detail: String,
}

impl Problem {
    fn fail(layer: &'static str, detail: impl Into<String>) -> Self {
        Problem {
            severity: Severity::Fail,
            layer,
            detail: detail.into(),
        }
    }
    fn warn(layer: &'static str, detail: impl Into<String>) -> Self {
        Problem {
            severity: Severity::Warn,
            layer,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mark = match self.severity {
            Severity::Fail => "✗",
            Severity::Warn => "!",
        };
        write!(f, "{mark} [{}] {}", self.layer, self.detail)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub problems: Vec<Problem>,
    /// Checks that ran and passed, for a report that says what it looked at
    /// rather than only what it disliked.
    pub passed: Vec<String>,
}

impl Report {
    pub fn failed(&self) -> bool {
        self.problems.iter().any(|p| p.severity == Severity::Fail)
    }

    pub fn fails(&self) -> impl Iterator<Item = &Problem> {
        self.problems
            .iter()
            .filter(|p| p.severity == Severity::Fail)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Problem> {
        self.problems
            .iter()
            .filter(|p| p.severity == Severity::Warn)
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        for line in &self.passed {
            s.push_str(&format!("✓ {line}\n"));
        }
        for p in &self.problems {
            s.push_str(&format!("{p}\n"));
        }
        s.push_str(&format!(
            "\n{} 项通过，{} 项不通过，{} 项警告。\n",
            self.passed.len(),
            self.fails().count(),
            self.warnings().count()
        ));
        s
    }
}

/// The command line's body: read a directory, run layers 1 and 2, produce the
/// report and the exit code.
///
/// Here rather than in `main` so it is testable without spawning a process.
/// The exit code is part of the contract — a meta task's audit round reads it:
/// 0 clean, 1 the version is refused, 2 the directory could not be read.
pub fn run(dir: &std::path::Path) -> (String, i32) {
    let files = match super::read_dir_protocol(dir) {
        Ok(f) => f,
        Err(e) => return (format!("protocol eval: {e}\n"), 2),
    };
    let report = check(&files);
    let code = if report.failed() { 1 } else { 0 };
    (report.render(), code)
}

/// Which cases layer 3 should run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// The cases the newest changelog version references, plus the three
    /// baselines. What `--changed` means.
    Changed,
    /// Every case in the version.
    All,
}

/// Runs the behaviour layer over a directory and appends its results.
///
/// Separate from [`run`] because it costs money: nothing reaches this unless
/// someone asked for it by name.
pub fn run_behaviour(dir: &std::path::Path, scope: Scope, tag: &str) -> (String, i32) {
    let files = match super::read_dir_protocol(dir) {
        Ok(f) => f,
        Err(e) => return (format!("protocol eval: {e}\n"), 2),
    };
    let wanted: Vec<String> = match scope {
        Scope::Changed => changed_cases(&files, tag),
        Scope::All => files
            .paths()
            .filter(|p| p.ends_with("/case.yaml"))
            .map(|p| p.trim_end_matches("case.yaml").to_string())
            .collect(),
    };

    let mut out = String::from("\n# 第 3 层 行为\n\n");
    let mut failed = false;
    let mut ran = 0;
    for case_path in wanted {
        let yaml_path = format!("{}case.yaml", case_path);
        let Some(yaml) = files.get(&yaml_path) else {
            out.push_str(&format!("! {case_path} 不在本版本里，跳过\n"));
            continue;
        };
        let case = match super::case::parse(yaml) {
            Ok(c) => c,
            Err(e) => {
                out.push_str(&format!("✗ {yaml_path}：{e}\n"));
                failed = true;
                continue;
            }
        };
        let plan = super::eval_run::Plan {
            files: &files,
            case_dir: dir.join(case_path.trim_end_matches('/')),
            slug: "eval-case".into(),
            role_runtime: autome_domain::role::Runtime::Claude,
        };
        let runner = super::eval_run::CliRunner::grading_against(plan.role_runtime);
        let result = super::eval_run::run_case(&plan, &case, &runner);
        out.push_str(&result.render());
        failed |= !result.passed();
        ran += 1;
    }
    out.push_str(&format!("\n跑了 {ran} 个用例。\n"));
    (out, if failed { 1 } else { 0 })
}

/// Layers 1 and 2 over one version.
pub fn check(files: &ProtocolFiles) -> Report {
    let mut r = Report::default();
    layer_one(files, &mut r);
    layer_two(files, &mut r);
    r
}

// ---------------------------------------------------------------------------
// Layer 1: static
// ---------------------------------------------------------------------------

const L1: &str = "静态";

fn layer_one(files: &ProtocolFiles, r: &mut Report) {
    // The kernel contract. Checked against the expectation compiled into this
    // binary, never against anything in the repository being checked.
    let breaches = protocol::verify_contract(super::expected_contract(), files);
    if breaches.is_empty() {
        r.passed.push(format!(
            "内核契约区 {} 处，标记成对、内容未变",
            super::expected_contract().len()
        ));
    }
    for b in breaches {
        r.problems.push(Problem::fail(L1, b.to_string()));
    }

    // The size budget.
    let bytes = files.sized_bytes();
    if bytes > protocol::SIZE_BUDGET_BYTES {
        r.problems.push(Problem::fail(
            L1,
            format!(
                "协议正文 {bytes} 字节，超过 {} 字节的上限。\
                 每个会话开场都要读它，之后每次往返还带着它。",
                protocol::SIZE_BUDGET_BYTES
            ),
        ));
    } else {
        r.passed.push(format!(
            "协议正文 {bytes} / {} 字节",
            protocol::SIZE_BUDGET_BYTES
        ));
    }

    // The phrases each version must and must not contain.
    let phrase_problems = phrases::check(files);
    if phrase_problems.is_empty() {
        r.passed
            .push(format!("必含 / 禁含短语 {} 条", phrases::required().len()));
    }
    for p in phrase_problems {
        r.problems.push(Problem::fail(L1, p.to_string()));
    }

    // Every placeholder a template uses has to be one the core fills.
    for file in phrases::ROLE_PROMPTS
        .iter()
        .chain(["prompts/intake.md", "prompts/onboarding.md"].iter())
    {
        let Some(text) = files.get(file) else {
            continue;
        };
        for name in placeholders(text) {
            if !KNOWN_PLACEHOLDERS.contains(&name.as_str()) {
                r.problems.push(Problem::fail(
                    L1,
                    format!(
                        "{file} 用了 `{{{name}}}`，但 Autome 不填这个占位符。\
                         能填的是：{}",
                        KNOWN_PLACEHOLDERS.join("、")
                    ),
                ));
            }
        }
    }
    // The one placeholder whose *absence* is a defect: a round with a budget
    // and nowhere to put it is told nothing about `N`, which is how a session
    // came to guess the factor and write `14/14` against the core's 35.
    for file in ["prompts/impl.md", "prompts/audit.md"] {
        if let Some(text) = files.get(file)
            && !text.contains("{budget_line}")
        {
            r.problems.push(Problem::fail(
                L1,
                format!("{file} 里没有 `{{budget_line}}`，这一轮就不会被告知 k 与 N。"),
            ));
        }
    }

    // The brief map points at protocol headings by name. A heading that was
    // renamed leaves the brief missing a section *silently*, and a round that
    // was never shown a rule cannot follow it — which then looks like the
    // model ignoring the protocol.
    match files.get("brief-map.toml").map(crate::brief::parse_map) {
        None => r
            .problems
            .push(Problem::fail(L1, "本版本没有 brief-map.toml")),
        Some(Err(e)) => r.problems.push(Problem::fail(L1, e)),
        Some(Ok(map)) => {
            let dangling = map.dangling(
                files.loop_protocol().unwrap_or_default(),
                files.session_protocol().unwrap_or_default(),
            );
            if dangling.is_empty() {
                r.passed.push("brief-map 指向的协议小节都还在".into());
            }
            for d in dangling {
                r.problems
                    .push(Problem::fail(L1, format!("brief-map.toml {d}")));
            }
        }
    }

    layer_one_cases(files, r);
    layer_one_changelog(files, r);
}

/// The eval cases, as data. Layer 3 runs them; this reads them.
fn layer_one_cases(files: &ProtocolFiles, r: &mut Report) {
    let paths: Vec<String> = files
        .paths()
        .filter(|p| p.ends_with("/case.yaml"))
        .map(str::to_string)
        .collect();
    if paths.is_empty() {
        r.problems.push(Problem::warn(
            L1,
            "本版本一个 eval 用例都没有。behavioral 类改动会因此没法验证。",
        ));
        return;
    }
    let mut ok = 0;
    for path in &paths {
        let dir = path.trim_end_matches("case.yaml");
        let case = match super::case::parse(files.get(path).unwrap_or_default()) {
            Ok(c) => c,
            Err(e) => {
                r.problems.push(Problem::fail(L1, format!("{path}：{e}")));
                continue;
            }
        };
        let mut bodies = Vec::new();
        let mut missing = false;
        for g in &case.graders {
            let full = format!("{dir}{g}");
            match files.get(&full) {
                Some(body) => bodies.push((g.clone(), body.to_string())),
                None => {
                    missing = true;
                    r.problems
                        .push(Problem::fail(L1, format!("{path} 引用了不存在的 {full}")));
                }
            }
        }
        // The scaffold has to be there too, or the case fails at run time —
        // after the money is spent.
        if !files.contains(&format!("{dir}{}", case.scaffold.trim_start_matches("./"))) {
            missing = true;
            r.problems.push(Problem::fail(
                L1,
                format!("{path} 的 scaffold `{}` 不存在", case.scaffold),
            ));
        }
        for problem in super::case::lint(&case, &bodies) {
            r.problems.push(Problem::fail(L1, problem));
        }
        if !missing {
            ok += 1;
        }
    }
    if ok == paths.len() {
        r.passed
            .push(format!("eval 用例 {ok} 个，可读且带阳性对照"));
    }
}

/// Placeholders a template may use.
const KNOWN_PLACEHOLDERS: [&str; 8] = [
    "slug",
    "request",
    "inputs",
    "brief_path",
    "budget_line",
    "design_rounds",
    "task_metrics",
    "metric_vocabulary",
];

/// `{lower_snake}` runs, ignoring braces in code samples and prose.
fn placeholders(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let name = &after[..close];
        if !name.is_empty()
            && name.len() < 40
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
            && !out.contains(&name.to_string())
        {
            out.push(name.to_string());
        }
        rest = &after[close + 1..];
    }
    out
}

fn layer_one_changelog(files: &ProtocolFiles, r: &mut Report) {
    let Some(text) = files.get("CHANGELOG.md") else {
        r.problems
            .push(Problem::fail(L1, "本版本没有 CHANGELOG.md"));
        return;
    };
    let log = match changelog::parse(text) {
        Ok(l) => l,
        Err(e) => {
            r.problems
                .push(Problem::fail(L1, format!("CHANGELOG.md {e}")));
            return;
        }
    };

    let has_path = |p: &str| -> bool {
        // An `eval` entry names a directory; `ProtocolFiles` holds files.
        files.contains(p) || files.paths().any(|f| f.starts_with(p))
    };
    let mut entries = 0;
    for entry in log.entries() {
        entries += 1;
        for problem in changelog::check_entry(entry, &has_path) {
            r.problems.push(Problem::fail(L1, problem.to_string()));
        }
    }
    if entries == 0 {
        r.problems
            .push(Problem::fail(L1, "CHANGELOG.md 里一条改动都没有"));
    } else {
        r.passed
            .push(format!("CHANGELOG 条目 {entries} 条，门槛齐全"));
    }

    clause_coverage(files, &log, r);
}

/// Two directions, and they are not the same check.
///
/// A changelog entry pointing at a clause that does not exist is a **failure**:
/// the entry claims to govern something, the evidence trail is broken, and
/// the `retire` experiment that would later measure it has nothing to remove.
///
/// A clause with imperatives that no entry covers is a **warning**. Most of
/// the protocol was inherited from 1.x, which arrived at it by running the
/// loop for months without writing down why; demanding a changelog entry for
/// every sentence of it retroactively would either fail forever or be filled
/// in with fiction. The warning is a worklist, and the number is supposed to
/// go down.
fn clause_coverage(files: &ProtocolFiles, log: &Changelog, r: &mut Report) {
    let mut dangling = 0;
    for entry in log.entries() {
        let (file, heading) = match entry.clause.split_once('#') {
            Some((f, h)) => (f, h),
            None => (entry.clause.as_str(), ""),
        };
        let Some(text) = files.get(file) else {
            // `check_entry` already reported the missing file.
            continue;
        };
        if heading.is_empty() {
            continue;
        }
        // `实现循环/自审清单` — the last segment is the one to find, and it is
        // matched as a prefix so a heading may be edited for clarity without
        // silently orphaning its entry.
        let leaf = heading.rsplit('/').next().unwrap_or(heading);
        let found = text
            .lines()
            .filter(|l| l.starts_with('#'))
            .any(|l| l.trim_start_matches('#').trim().starts_with(leaf));
        if !found {
            dangling += 1;
            r.problems.push(Problem::fail(
                L1,
                format!(
                    "{} 的 clause 指向 `{}`，但 {file} 里没有这个标题。\
                     条文被改名或删掉时，要一起改 CHANGELOG——不然证据链断在这里。",
                    entry.id, entry.clause
                ),
            ));
        }
    }
    if dangling == 0 {
        r.passed.push("CHANGELOG 的 clause 都能在协议里找到".into());
    }

    let covered: Vec<&str> = log
        .clauses()
        .iter()
        .filter_map(|c| {
            c.split_once('#')
                .map(|(_, h)| h.rsplit('/').next().unwrap_or(h))
        })
        .collect();
    let mut uncovered = 0;
    for file in protocol::SIZED_FILES {
        let Some(text) = files.get(file) else {
            continue;
        };
        let mut heading = String::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix('#') {
                heading = rest.trim_start_matches('#').trim().to_string();
                continue;
            }
            if !IMPERATIVES.iter().any(|w| line.contains(w)) {
                continue;
            }
            if covered.iter().any(|c| heading.starts_with(c)) {
                continue;
            }
            uncovered += 1;
        }
    }
    if uncovered > 0 {
        r.problems.push(Problem::warn(
            L1,
            format!(
                "{uncovered} 行带「必须 / 不得 / 不要」的条文，没有任何 CHANGELOG 条目说明它为什么在。\
                 多数是从 1.x 继承来的——那几个月的运行没有把理由写下来。这个数字应该往下走。"
            ),
        ));
    } else {
        r.passed.push("每条祈使句都有出处".into());
    }
}

const IMPERATIVES: [&str; 3] = ["必须", "不得", "不要"];

// ---------------------------------------------------------------------------
// Layer 2: the examples parse
// ---------------------------------------------------------------------------

const L2: &str = "示例";

fn layer_two(files: &ProtocolFiles, r: &mut Report) {
    let mut checked = 0;
    for file in protocol::SIZED_FILES {
        let Some(text) = files.get(file) else {
            continue;
        };
        for (line, block) in fenced_blocks(text) {
            let Some(doc) = example_document(&block) else {
                continue;
            };
            checked += 1;
            if let Err(e) = status_block::parse(&doc) {
                r.problems.push(Problem::fail(
                    L2,
                    format!(
                        "{file}:{line} 的示例喂给真正的解析器读不出来：{e}。\
                         协议给会话看的格式，必须是内核认得的那个。"
                    ),
                ));
            }
        }
    }
    if checked == 0 {
        r.problems.push(Problem::fail(
            L2,
            "协议里一个状态块或里程碑表的示例都没有。会话要照着写的格式得有个样子。",
        ));
    } else {
        r.passed
            .push(format!("状态块 / 里程碑表示例 {checked} 处可解析"));
    }
}

/// Fenced code blocks, with the 1-based line the fence opened on.
fn fenced_blocks(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut open: Option<(usize, Vec<&str>)> = None;
    for (i, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            match open.take() {
                Some((start, body)) => out.push((start, body.join("\n"))),
                None => open = Some((i + 1, Vec::new())),
            }
            continue;
        }
        if let Some((_, body)) = open.as_mut() {
            body.push(line);
        }
    }
    out
}

/// Turns an example block into something `status_block::parse` can be given.
///
/// The protocol shows the two halves separately — a status block in one fence,
/// a milestone table in another — and the parser wants a whole document. Each
/// half is completed with a minimal version of the other, so what is being
/// checked is the half the protocol actually printed.
fn example_document(block: &str) -> Option<String> {
    let is_status = block.contains("status:") && block.contains("design-round:");
    let is_table = block.contains("| ID | 状态 |");
    if !is_status && !is_table {
        return None;
    }
    let status: String = if is_status {
        block
            .lines()
            .map(concrete_field)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        DEFAULT_STATUS.to_string()
    };
    let table = if is_table {
        // The fence may or may not print the `## 里程碑` heading above the
        // table; the wrapper below supplies exactly one, so drop any the
        // example carried.
        block
            .lines()
            .filter(|l| !l.trim().starts_with("## "))
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        DEFAULT_TABLE.to_string()
    };
    Some(format!(
        "{status}\n\n# 示例\n\n## 里程碑\n\n{table}\n\n## Backlog\n\n## 争议项\n"
    ))
}

/// Turns one line of the status-block *legend* into one line of an example.
///
/// The protocol prints the legend, not a filled-in block: `design-round: d/N`,
/// `status: 设计中 | 实现中 | …`. Feeding that to the parser verbatim would
/// fail on the metavariables and say nothing about the protocol.
///
/// What is worth checking is the **shape** — which fields exist, what they are
/// called, and in what order — because that is what the parser depends on and
/// what a round gets wrong. The 2026-09-16 failure and every other
/// status-block failure so far has been a shape problem, never a value one. So
/// each metavariable is replaced by a concrete value of the right kind and the
/// result is parsed.
fn concrete_field(line: &str) -> String {
    let Some((key, value)) = line.split_once(':') else {
        return line.to_string();
    };
    let mut value = value.trim();
    // `设计中 | 实现中 | …` — a list of the allowed values. Take the first,
    // then normalise it like any other, because the first alternative is
    // itself often a metavariable (`current-milestone: M-xx | 无`).
    if value.contains('|') && !value.starts_with('|') {
        value = value.split('|').next().unwrap_or("").trim();
    }
    format!("{key}: {}", concrete_value(value))
}

fn concrete_value(value: &str) -> String {
    // `<下一实现轮首先完成的具体工作；没有时写"无">` — a description of what to
    // write, in angle brackets.
    if value.starts_with('<') {
        return "无".into();
    }
    // `d/N`, `k/N` — a counter over a limit, written with metavariables.
    if let Some((used, limit)) = value.split_once('/')
        && !used.chars().all(|c| c.is_ascii_digit())
        && !limit.chars().all(|c| c.is_ascii_digit())
    {
        return "1/15".into();
    }
    // `M-xx` — any milestone. The default table this is completed with has
    // M-01 in it, and the parser rightly rejects a pointer to a row that is
    // not there.
    if let Some(suffix) = value.strip_prefix("M-")
        && !suffix.chars().all(|c| c.is_ascii_digit())
    {
        return "M-01".into();
    }
    // `r` — a bare counter.
    if value.chars().count() == 1 && value.chars().all(|c| c.is_ascii_alphabetic()) {
        return "0".into();
    }
    value.to_string()
}

const DEFAULT_STATUS: &str = "status: 实现中\n\
     design-round: 1/15\n\
     implementation-round: 1/5\n\
     current-milestone: M-01\n\
     current-milestone-reopens: 0\n\
     convergence-mode: normal\n\
     next-action: 无";

const DEFAULT_TABLE: &str = "| ID | 状态 | 标题 | reopen | 领域 |\n\
     |---|---|---|---|---|\n\
     | M-01 | 开放 | 示例 | 0 | |";

/// Which eval cases a `--changed` run has to include: the ones the changelog's
/// newest version references, plus the baselines.
pub const BASELINE_CASES: [&str; 3] = [
    "evals/reviewer-independence/",
    "evals/auditor-rerun/",
    "evals/session-does-not-relay/",
];

pub fn changed_cases(files: &ProtocolFiles, tag: &str) -> Vec<String> {
    let mut out: Vec<String> = BASELINE_CASES.iter().map(|s| s.to_string()).collect();
    let Some(text) = files.get("CHANGELOG.md") else {
        return out;
    };
    let Ok(log) = changelog::parse(text) else {
        return out;
    };
    let Some(version) = log.version(tag) else {
        return out;
    };
    for entry in &version.entries {
        // A `retire` names the case it removes, which is not one to run.
        if entry.kind == ChangeKind::Retire {
            continue;
        }
        if let Some(path) = &entry.eval
            && !out.contains(path)
        {
            out.push(path.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::seed;

    fn details(r: &Report) -> String {
        r.problems
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_seed_passes_the_static_and_parse_layers() {
        let r = check(seed());
        assert!(!r.failed(), "{}", details(&r));
    }

    #[test]
    fn the_report_says_what_it_looked_at_not_only_what_it_disliked() {
        let r = check(seed());
        assert!(r.passed.len() >= 5, "{:?}", r.passed);
        assert!(r.render().contains("项通过"), "{}", r.render());
    }

    #[test]
    fn editing_a_contract_region_is_refused() {
        let mut files = seed().clone();
        let text = files.loop_protocol().unwrap().replace(
            "| 实现 impl | 推进最小编号的开放里程碑 | `待审` |",
            "| 实现 impl | 随便 | `已完成` |",
        );
        files.insert(autome_domain::protocol::LOOP_PROTOCOL, text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("roles"), "{}", details(&r));
    }

    #[test]
    fn deleting_a_clause_a_changelog_entry_points_at_is_refused() {
        // The evidence trail is the point of the changelog; a dangling clause
        // breaks it silently.
        let mut files = seed().clone();
        let text = files
            .loop_protocol()
            .unwrap()
            .replace("### 证据不写进设计文档", "### 证据的去处");
        files.insert(autome_domain::protocol::LOOP_PROTOCOL, text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(
            details(&r).contains("证据不写进设计文档"),
            "{}",
            details(&r)
        );
    }

    #[test]
    fn a_behavioral_change_with_no_case_is_refused() {
        let mut files = seed().clone();
        let text = files.get("CHANGELOG.md").unwrap().replace(
            "  eval: evals/impl-writes-evidence-not-design/\n",
            "  eval: null\n",
        );
        files.insert("CHANGELOG.md", text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("eval"), "{}", details(&r));
    }

    #[test]
    fn a_protocol_over_the_size_budget_is_refused() {
        let mut files = seed().clone();
        let text = format!(
            "{}\n{}",
            files.loop_protocol().unwrap(),
            "补".repeat(autome_domain::protocol::SIZE_BUDGET_BYTES)
        );
        files.insert(autome_domain::protocol::LOOP_PROTOCOL, text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("超过"), "{}", details(&r));
    }

    #[test]
    fn a_template_asking_for_something_the_core_does_not_fill_is_refused() {
        let mut files = seed().clone();
        let text = format!("{}\n参考 {{mood}}。\n", files.prompt("impl").unwrap());
        files.insert("prompts/impl.md", text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("mood"), "{}", details(&r));
    }

    #[test]
    fn a_loop_round_template_that_lost_its_budget_placeholder_is_refused() {
        let mut files = seed().clone();
        let text = files.prompt("impl").unwrap().replace("{budget_line}", "");
        files.insert("prompts/impl.md", text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("budget_line"), "{}", details(&r));
    }

    #[test]
    fn an_example_status_block_the_parser_cannot_read_is_refused() {
        // 2026-09-16: a table the document's own reader could not parse cost a
        // whole task. A protocol that *prints* such a format is worse than one
        // that prints none.
        let mut files = seed().clone();
        let text = files.session_protocol().unwrap().replace(
            "current-milestone: M-xx | 无",
            "current-milestone-id: M-xx | 无",
        );
        files.insert(autome_domain::protocol::SESSION_PROTOCOL, text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("解析器"), "{}", details(&r));
    }

    #[test]
    fn an_example_milestone_table_with_a_blank_header_cell_is_refused() {
        let mut files = seed().clone();
        let text = files.loop_protocol().unwrap().replace(
            "| ID | 状态 | 标题 | reopen | 领域 |",
            "|  | 状态 | 标题 | reopen | 领域 |",
        );
        files.insert(autome_domain::protocol::LOOP_PROTOCOL, text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
    }

    #[test]
    fn a_code_sample_that_is_not_a_status_block_is_not_parsed() {
        // The protocol shows shell commands too, and feeding those to the
        // status-block parser would fail for reasons that say nothing.
        let mut files = seed().clone();
        let text = format!(
            "{}\n\n```sh\ncargo test -- --nocapture\n```\n",
            files.loop_protocol().unwrap()
        );
        files.insert(autome_domain::protocol::LOOP_PROTOCOL, text);
        assert!(!check(&files).failed());
    }

    #[test]
    fn uncovered_imperatives_are_a_worklist_rather_than_a_refusal() {
        let r = check(seed());
        assert!(!r.failed(), "{}", details(&r));
        // The inherited 1.x text has plenty; the point is that the number is
        // visible and supposed to fall.
        let warned = r
            .warnings()
            .any(|p| p.detail.contains("没有任何 CHANGELOG 条目"));
        assert!(warned, "{}", r.render());
    }

    #[test]
    fn a_changed_run_covers_the_new_cases_and_the_three_baselines() {
        let files = seed().clone();
        let cases = changed_cases(&files, "protocol/v1");
        for baseline in BASELINE_CASES {
            assert!(cases.contains(&baseline.to_string()), "{cases:?}");
        }
        assert!(
            cases.contains(&"evals/impl-self-check-before-pending/".to_string()),
            "{cases:?}"
        );
        // A `clarify` entry names no case, and nothing is invented for it.
        assert!(!cases.iter().any(|c| c == "null"), "{cases:?}");
    }

    #[test]
    fn the_legend_is_checked_for_its_shape_not_for_its_metavariables() {
        // `design-round: d/N` is not a value the parser could ever accept, and
        // it is also not what a round writes. What matters is that the field
        // is called `design-round` and sits where it sits.
        assert_eq!(concrete_field("design-round: d/N"), "design-round: 1/15");
        assert_eq!(
            concrete_field("status: 设计中 | 实现中 | 已完成"),
            "status: 设计中"
        );
        assert_eq!(
            concrete_field("next-action: <下一轮要做的事>"),
            "next-action: 无"
        );
        assert_eq!(
            concrete_field("current-milestone-reopens: r"),
            "current-milestone-reopens: 0"
        );
        assert_eq!(
            concrete_field("current-milestone: M-xx | 无"),
            "current-milestone: M-01"
        );
        // A filled-in example is left exactly as written.
        assert_eq!(
            concrete_field("implementation-round: 7/35"),
            "implementation-round: 7/35"
        );
    }

    #[test]
    fn renaming_a_status_field_in_the_legend_is_caught() {
        let mut files = seed().clone();
        let text = files
            .session_protocol()
            .unwrap()
            .replace("convergence-mode:", "convergence:");
        files.insert(autome_domain::protocol::SESSION_PROTOCOL, text);
        assert!(check(&files).failed());
    }

    /// Writes a version to a scratch directory, the way a checkout holds one.
    fn on_disk(tag: &str, files: &ProtocolFiles) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("autome-eval-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, content) in files.iter() {
            let full = dir.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(full, content).unwrap();
        }
        dir
    }

    #[test]
    fn running_against_the_seed_on_disk_is_clean() {
        // The same files the unit tests use, but reached the way the command
        // line reaches them: off the filesystem, eval cases and generated
        // contract.toml included.
        let dir = on_disk("good", seed());
        let (report, code) = run(&dir);
        assert_eq!(code, 0, "{report}");
        assert!(report.contains("项通过"), "{report}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_version_exits_one_and_a_missing_directory_exits_two() {
        // The audit round of a meta task reads the exit code, so the two
        // failures have to be distinguishable: one means "this version is not
        // acceptable", the other means "there was nothing to look at".
        let mut files = seed().clone();
        files.remove("CHANGELOG.md");
        let dir = on_disk("bad", &files);
        let (report, code) = run(&dir);
        assert_eq!(code, 1, "{report}");
        let _ = std::fs::remove_dir_all(&dir);

        let (_, code) = run(&std::env::temp_dir().join("autome-eval-nowhere"));
        assert_eq!(code, 2);
    }

    #[test]
    fn a_brief_map_pointing_at_a_renamed_heading_is_refused() {
        let mut files = seed().clone();
        let text = files
            .loop_protocol()
            .unwrap()
            .replace("## 实现循环", "## 实现与审计循环");
        files.insert(autome_domain::protocol::LOOP_PROTOCOL, text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("实现循环"), "{}", details(&r));
    }

    #[test]
    fn a_case_that_could_never_say_anything_is_caught_before_it_is_paid_for() {
        let mut files = seed().clone();
        let path = "evals/auditor-rerun/graders/reopened-the-false-pass.md";
        let text = files.get(path).unwrap().replace("阳性对照", "说明");
        files.insert(path, text);
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("阳性对照"), "{}", details(&r));
    }

    #[test]
    fn a_case_whose_scaffold_is_missing_is_caught_statically() {
        let mut files = seed().clone();
        files.remove("evals/auditor-rerun/scaffold.sh");
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
        assert!(details(&r).contains("scaffold"), "{}", details(&r));
    }

    #[test]
    fn a_version_with_no_changelog_at_all_is_refused() {
        let mut files = seed().clone();
        files.remove("CHANGELOG.md");
        let r = check(&files);
        assert!(r.failed(), "{}", r.render());
    }
}
