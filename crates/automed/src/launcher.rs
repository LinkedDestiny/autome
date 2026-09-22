//! The session launcher. Technical design §7.
//!
//! A session runs in a visible terminal tab, not as a child of `automed`
//! (design §1, change 1). The user can watch it, scroll it, and interrupt it
//! the way they would any other CLI session. The cost is that the core cannot
//! `wait()` on the process, which is what the wrapper script's exit marker and
//! the pid/heartbeat backstop in `session` exist to solve.
//!
//! This module owns three things:
//!
//! 1. **The prompt.** The entry sentence from the task-file protocol, plus the
//!    bound skills, plus whatever the transition asked to inject.
//! 2. **The argv.** Per-runtime flags live in one table (§7.2), because they
//!    change with the CLIs rather than with our logic.
//! 3. **The terminal.** AppleScript into iTerm2, or Terminal when iTerm2 is
//!    missing (requirement E-02).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use autome_domain::config::RoleConfig;
use autome_domain::role::Role;
use autome_domain::role::Runtime;
use autome_domain::session::{SessionKind, SessionPaths};
use autome_domain::task::Inject;

use crate::store::DecisionRecord;

#[derive(Debug)]
pub struct LaunchError {
    pub detail: String,
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "启动会话失败：{}", self.detail)
    }
}

impl std::error::Error for LaunchError {}

pub type Result<T> = std::result::Result<T, LaunchError>;

fn err(detail: impl Into<String>) -> LaunchError {
    LaunchError {
        detail: detail.into(),
    }
}

// ---------------------------------------------------------------------------
// The CLI adapter table (design §7.2)
// ---------------------------------------------------------------------------

/// How to invoke one runtime non-interactively.
///
/// Flags are data, not code, because they track the CLIs' release cadence
/// rather than anything about Autome. When a flag moves, this table is the
/// single place to change — and `env_probe`'s login table is its sibling.
///
/// The prompt is delivered on **stdin**, never as an argv element: a task
/// request can be arbitrarily long and can contain anything the user typed,
/// and neither an argv length limit nor a quoting bug should be able to
/// truncate or reinterpret it.
pub struct RuntimeAdapter {
    pub runtime: Runtime,
    /// The binary name looked up on PATH.
    pub binary: &'static str,
    /// Flags that put the CLI in non-interactive, act-without-asking mode.
    pub autonomous_flags: &'static [&'static str],
    /// The flag that selects a model; the value follows as its own argument.
    pub model_flag: &'static str,
    /// The flag that selects an effort/reasoning level, when the CLI has one.
    pub effort_flag: Option<&'static str>,
    /// The flag that lets the CLI run somewhere that is not itself a Git
    /// repository, for the runtimes that otherwise refuse.
    ///
    /// A workspace task's working directory is exactly that: a plain
    /// directory holding one checkout per member repository. Codex declines —
    /// "Not inside a trusted directory and --skip-git-repo-check was not
    /// specified" — and the round exits 1 before reading anything.
    ///
    /// What the check protects against is "this directory has no version
    /// control, so nothing you do here can be undone". That is not the
    /// situation: every subdirectory is a repository, on its own branch. So
    /// the flag is passed only where the working directory really is outside
    /// one, and the guard keeps its meaning everywhere it has one.
    pub no_repo_flag: Option<&'static str>,
}

impl RuntimeAdapter {
    /// Args that add one more directory to the CLI's own sandbox, for the
    /// runtimes that have one.
    ///
    /// Only Codex does. `claude` is given no OS sandbox by 2.0 (design §17),
    /// so there is nothing to widen and nothing to return.
    fn extra_writable_roots(&self, roots: &[PathBuf]) -> Vec<String> {
        match self.runtime {
            Runtime::Codex if !roots.is_empty() => vec![
                "--config".to_string(),
                format!(
                    "sandbox_workspace_write.writable_roots={}",
                    toml::Value::Array(
                        roots
                            .iter()
                            .map(|r| toml::Value::String(r.to_string_lossy().into_owned()))
                            .collect()
                    )
                ),
            ],
            _ => vec![],
        }
    }
}

/// The repository's **common** Git directory, when it is not inside `cwd`.
///
/// For a worktree it never is: the common directory of
/// `<repo>/.worktree/<slug>/` is `<repo>/.git`, outside the directory the
/// session is allowed to write. Codex's `workspace-write` sandbox therefore
/// refuses to create `index.lock`, and every `git add` in a worktree session
/// exits 128.
///
/// A real run did that on every audit round for thirty rounds. Nothing was
/// lost — the core sweeps the leftover changes into a commit of its own — but
/// each round still spent a doomed commit attempt and a paragraph explaining
/// it, and the branch history ended up narrated by two different voices.
///
/// **Common, not per-worktree.** The first version of this asked for
/// `--absolute-git-dir` and got `<repo>/.git/worktrees/<slug>`. That is enough
/// to create `index.lock` and not enough to finish: `git add` writes the blob
/// into the shared object store at `<repo>/.git/objects`, so it got one step
/// further and failed with `failed to insert into database` instead. Widening
/// to the common directory covers both, and the per-worktree directory is
/// inside it.
///
/// Returns `None` when the Git directory is already inside `cwd` (the
/// onboarding session runs in the repository itself, where nothing needs
/// widening) and when Git cannot answer at all — a guessed path would be worse
/// than the status quo.
fn git_common_dirs(worktrees: &[&Path]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for wt in worktrees {
        if let Some(dir) = git_dir_outside(wt)
            && !out.contains(&dir)
        {
            out.push(dir);
        }
    }
    out
}

fn git_dir_outside(cwd: &Path) -> Option<PathBuf> {
    let out = crate::git::run(
        cwd,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .ok()?;
    if !out.ok() {
        return None;
    }
    let dir = PathBuf::from(out.line());
    if dir.as_os_str().is_empty() || !dir.is_absolute() || dir.starts_with(cwd) {
        return None;
    }
    Some(dir)
}

/// Verified against Claude Code 2.1.261 and Codex 0.153.4 on 2026-09-15 by
/// running each form and checking both the effect and the exit code. The
/// details that cost the most to get wrong:
///
/// - **`claude` needs `-p`.** Without it the CLI starts an *interactive*
///   session and never exits, so the wrapper never writes its marker and the
///   task sits at its node forever. This is not a flag that merely changes
///   output formatting; it is the difference between a session and a hang.
/// - **`codex exec` has no `--full-auto`.** Its approval policy is
///   `--sandbox <read-only|workspace-write|danger-full-access>`;
///   `workspace-write` is what lets a session edit its own worktree.
/// - Both take the prompt on stdin when it is not given as an argument.
pub const ADAPTERS: [RuntimeAdapter; 2] = [
    RuntimeAdapter {
        runtime: Runtime::Claude,
        binary: "claude",
        // `-p` is non-interactive mode.
        //
        // `acceptEdits` alone was wrong, and the comment here used to claim it
        // "lets the session edit files and run commands" — only the first half
        // was true. Bash was denied, so an implementation round could write
        // Swift but never compile it, never run a test, never commit. The
        // first real implementation round declared a protocol failure rather
        // than burn its budget producing no evidence, which was the right call
        // and is how this was found.
        //
        // `--allowedTools Bash` is what unlocks command execution. A pattern
        // like `Bash(swift *)` looks narrower but is not: tested against 2.1.261,
        // an `rm -rf` outside the pattern still ran. So the honest choice is
        // between "no commands" and "all commands", and a tool that cannot run
        // a build cannot implement anything.
        //
        // `--permission-mode auto` is the other candidate and was rejected: it
        // asks a classifier about each command, which adds latency and a new
        // failure mode — during testing it was rate-limited and refused every
        // command, exactly the failure being fixed.
        //
        // 2.0 deliberately adds no sandbox of its own (design §17): the
        // confinement is the working directory and the protocol. Note that
        // Bash is not confined by either — Codex's `workspace-write` is a real
        // OS sandbox, this is not.
        //
        // `--output-format stream-json` (with the `--verbose` it requires) is
        // what makes the session log a record of the work rather than of its
        // closing paragraph. In `text` mode `claude -p` prints only the final
        // summary: a round that edited a dozen files left a 3 KB log, against
        // 300 KB for the same work on Codex, which streams its execution. With
        // sessions headless that log is the only place to see what happened.
        // The wrapper pipes the stream through `automed render-stream` to make
        // it readable and keeps the raw JSONL beside it.
        autonomous_flags: &[
            "-p",
            "--permission-mode",
            "acceptEdits",
            "--allowedTools",
            "Bash",
            "--output-format",
            "stream-json",
            "--verbose",
        ],
        model_flag: "--model",
        effort_flag: Some("--effort"),
        // Claude Code runs anywhere; it has no such check to disable.
        no_repo_flag: None,
    },
    RuntimeAdapter {
        runtime: Runtime::Codex,
        binary: "codex",
        // `--json` is what makes a Codex session measurable. Without it the
        // stream is prose written for a person: no usage, no turn count,
        // nothing the core can record — so every Codex round was invisible to
        // the version page while every Claude round was not, which would have
        // made the two runtimes incomparable in the one direction that
        // matters. The wrapper renders the JSONL back into prose for the log.
        autonomous_flags: &["exec", "--json", "--sandbox", "workspace-write"],
        model_flag: "--model",
        // Codex takes reasoning effort as a config override rather than a
        // dedicated flag.
        effort_flag: Some("--config"),
        no_repo_flag: Some("--skip-git-repo-check"),
    },
];

pub fn adapter(runtime: Runtime) -> &'static RuntimeAdapter {
    ADAPTERS
        .iter()
        .find(|a| a.runtime == runtime)
        .expect("every runtime has an adapter")
}

/// Builds the argv after the binary. Returned separately from the binary so
/// the wrapper script receives them as distinct arguments.
pub fn build_args(config: &RoleConfig, cwd: &Path, worktrees: &[&Path]) -> Vec<String> {
    let a = adapter(config.runtime);
    let mut args: Vec<String> = a
        .autonomous_flags
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    // Only where the working directory really is outside a repository — a
    // workspace task's, which holds the checkouts rather than being one.
    if let Some(flag) = a.no_repo_flag
        && !crate::git::is_inside_work_tree(cwd)
    {
        args.push(flag.to_string());
    }
    // Only the runtime that has a sandbox pays for the probe: `git rev-parse`
    // is a process, and `extra_writable_roots` returns nothing for Claude.
    //
    // One root per checkout, asked of each checkout rather than of the
    // directory they sit in. A workspace task's working directory is not
    // inside any repository, so asking there answers either nothing — and
    // every `git add` in every member fails — or, worse, names a stray
    // repository at the workspace root that must not be written to at all.
    if a.runtime == Runtime::Codex {
        args.extend(a.extra_writable_roots(&git_common_dirs(worktrees)));
    }
    if !config.model.trim().is_empty() {
        args.push(a.model_flag.to_string());
        args.push(config.model.clone());
    }
    if let (Some(flag), Some(effort)) = (a.effort_flag, config.effort.as_ref()) {
        args.push(flag.to_string());
        // Codex takes effort as a config override rather than a bare flag.
        args.push(match config.runtime {
            Runtime::Codex => format!("model_reasoning_effort={effort}"),
            Runtime::Claude => effort.clone(),
        });
    }
    args
}

// ---------------------------------------------------------------------------
// Prompt construction
// ---------------------------------------------------------------------------

/// What the core knows about the implementation budget as it starts a round.
///
/// Neither number is reliably derivable inside a session, and a real run
/// proved it: the protocol said "N is the milestone count times a factor" but
/// not which factor, so the round guessed 2 and wrote `implementation-round:
/// 14/14` while the core had computed 35. The session then declared the budget
/// spent, the core scheduled another round anyway, and the next session
/// invented "the user released one more round" to explain the contradiction.
///
/// So the core states both numbers and the protocol says the denominator comes
/// from here.
pub struct BudgetLine {
    /// The round number this session is about to run, counted per role.
    pub round: u32,
    /// `N`, the shared implementation budget.
    pub limit: u32,
}

/// Everything needed to write a session's prompt.
pub struct PromptSpec<'a> {
    pub kind: SessionKind,
    /// The prompt templates of the protocol version this task is pinned to.
    /// Passed in rather than read here: two tasks in the same project can be
    /// running under different versions, and the one that started first keeps
    /// the rules it started under.
    pub templates: &'a autome_domain::protocol::ProtocolFiles,
    pub slug: &'a str,
    /// The task's document directory as *this round* sees it: relative to the
    /// directory the session starts in.
    ///
    /// The templates used to spell `docs/{slug}` out, 54 times across seven
    /// files. That is right for a single repository and wrong for a workspace,
    /// where the session starts beside the member checkouts rather than inside
    /// one and the same directory is `<docs-member>/<doc_root>/<slug>`. A
    /// round told to write somewhere it cannot reach produces no document, and
    /// the loop reads that as a round that did nothing.
    pub doc_dir: &'a str,
    /// The project's design-round limit, for the skeleton status block the
    /// intake round writes.
    pub design_rounds: u32,
    /// The task's measured numbers, for the retro round. `None` everywhere
    /// else, and for a task whose metrics were never recorded.
    pub task_metrics: Option<&'a autome_domain::metrics::TaskMetrics>,
    /// Where the core wrote this round's brief, worktree-relative.
    pub brief_path: &'a str,
    /// Present for the two rounds of the implementation loop, which are the
    /// only ones that reason about `k` and `N`.
    pub budget: Option<BudgetLine>,
    /// The one-line request, for the intake session.
    pub request: &'a str,
    pub skills: &'a [String],
    pub inject: Option<&'a Inject>,
    /// Decided-but-unconsumed Backlog items and disputes, when the transition
    /// asked for `Inject::Decisions`.
    pub decisions: &'a [DecisionRecord],
    pub attachments: &'a [String],
    pub doc_refs: &'a [String],
}

/// Builds the prompt text a session is started with.
///
/// The entry sentence is quoted verbatim from the task-file protocol (§5.2),
/// because the task file dispatches on it. Everything else is appended after
/// it, so a protocol change in the file cannot be broken by prose we add here.
pub fn build_prompt(spec: &PromptSpec<'_>) -> Result<String> {
    let mut p = String::new();

    match spec.kind {
        SessionKind::Intake => return intake_prompt(spec),
        SessionKind::Onboarding => return onboarding_prompt(spec),
        SessionKind::Role { role } => {
            p.push_str(&role_prompt(role, spec)?);
        }
    }

    if !spec.skills.is_empty() {
        // "Must use" is the whole semantics of a binding (requirement S-03);
        // a softer wording would make the setting decorative.
        p.push_str(&format!(
            "\n本轮必须使用以下技能：{}。\n",
            spec.skills.join("、")
        ));
    }

    match spec.inject {
        Some(Inject::DesignFeedback { feedback }) => {
            p.push_str(&format!(
                "\n用户驳回了上一版设计，意见如下（原文，不要改写）：\n\n{feedback}\n\n\
                 请按这条意见修改设计文档，然后照常结束会话。\n"
            ));
        }
        Some(Inject::RebaseConflict { files }) => {
            p.push_str(&format!(
                "\n本分支 rebase 到最新默认分支时发生冲突，涉及文件：\n{}\n\n\
                 请解决这些冲突并提交，保持里程碑状态与设计文档一致，然后照常结束会话。\n",
                files
                    .iter()
                    .map(|f| format!("- {f}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        Some(Inject::Decisions) => {
            p.push_str(&render_decisions(spec.decisions));
        }
        None => {}
    }

    Ok(p)
}

/// What a role session is told to do.
///
/// 1.x dispatched each round with an entry sentence — "execute the task file,
/// additional task 3" — and the task file carried a numbered section per
/// round. That indirection existed because the *agent* had to work out which
/// round it was and hand off to the next one.
///
/// 2.0 does not work that way: the core knows which round it is dispatching,
/// so it says so. Carrying the indirection across cost a real run — the
/// generated task file had no section called "Task 1", every round opened it,
/// found nothing addressed to itself, and did nothing. Four sessions ran and
/// the design document was untouched.
///
/// The text itself is no longer here. It is a template in the protocol
/// version this task is pinned to (`prompts/<role>.md`), so that improving a
/// prompt is something a meta task can do and something the user can read a
/// diff of, rather than something that needs a release. What stays in code is
/// the *substitution*: which placeholders exist and what the core puts in
/// them. A template asking for a placeholder the core does not fill would
/// leave a literal `{…}` in front of the model, so every placeholder is
/// replaced and any leftover is an error.
fn role_prompt(role: Role, spec: &PromptSpec<'_>) -> Result<String> {
    let template = spec.templates.prompt(role.as_str()).ok_or_else(|| {
        err(format!(
            "协议版本里没有 prompts/{}.md，无法启动{}",
            role.as_str(),
            role.round_name()
        ))
    })?;

    // The denominator comes from here or it comes from a guess. See BudgetLine.
    let budget_line = match (role, spec.budget.as_ref()) {
        (Role::Impl, Some(b)) => format!(
            "本轮是**实现轮第 {} 轮**，实现预算 N = {}。\
             状态块的 `implementation-round` 写 `{}/{}`——\
             这两个数由 Autome 计算，不要自己按里程碑数推算、不要改写分母。\n\n",
            b.round, b.limit, b.round, b.limit
        ),
        (Role::Audit, Some(b)) => format!(
            "实现预算 N = {}，由 Autome 计算。预算是否用尽由 Autome 判断并停下等用户，\
             你不需要据此终止或改写分母。\n\n",
            b.limit
        ),
        _ => String::new(),
    };

    let rendered = template
        .replace("{doc_dir}", spec.doc_dir)
        .replace("{slug}", spec.slug)
        .replace("{budget_line}", &budget_line)
        .replace("{brief_path}", spec.brief_path)
        .replace("{task_metrics}", &render_task_metrics(spec.task_metrics))
        .replace(
            "{metric_vocabulary}",
            &autome_domain::metrics::TaskMetrics::METRIC_NAMES.join(" / "),
        );
    check_no_placeholders_left(&rendered, &format!("prompts/{}.md", role.as_str()))?;
    Ok(rendered)
}

/// The retro round is handed the task's measured numbers so it writes against
/// them rather than against its recollection of the run.
fn render_task_metrics(metrics: Option<&autome_domain::metrics::TaskMetrics>) -> String {
    let Some(m) = metrics else {
        return "（本任务没有记录到指标。照常复盘，但不要编造数字。）".to_string();
    };
    let mut s = String::from("| 指标 | 值 |\n|---|---|\n");
    let mut row = |name: &str, value: String| s.push_str(&format!("| {name} | {value} |\n"));
    row(
        "设计轮",
        format!("{}/{}", m.design_rounds_used, m.design_rounds_limit),
    );
    row("实现轮", format!("{}/{}", m.impl_rounds_used, m.budget_n));
    row("里程碑数", m.milestones.to_string());
    row("reopen 合计", m.reopen_total.to_string());
    if !m.reopen_by_domain.is_empty() {
        row(
            "reopen 按领域",
            m.reopen_by_domain
                .iter()
                .map(|(d, n)| format!("{d} × {n}"))
                .collect::<Vec<_>>()
                .join("、"),
        );
    }
    row("实现缺陷", m.impl_defects.to_string());
    row("验证缺口", m.verification_gaps.to_string());
    row("协议失败", m.protocol_failures.to_string());
    row("关闭后被推翻", m.closed_then_contradicted.to_string());
    row("人工验收未确认", m.manual_items_open.to_string());
    row("总 tokens", m.total_tokens.to_string());
    row("总 turns", m.total_turns.to_string());
    if let Some(cost) = m.total_cost_usd {
        row("费用 USD（仅 Claude 会话）", format!("{cost:.4}"));
    }
    s
}

/// A template placeholder the core does not know how to fill would reach the
/// model as a literal `{task_metrics}`. That is not a cosmetic problem: the
/// round would be reading an instruction about data it was never given.
fn check_no_placeholders_left(rendered: &str, file: &str) -> Result<()> {
    // Only single-word `{lower_snake}` runs count. Protocol text contains
    // braces in code samples and in prose, and rejecting those would make the
    // check unusable.
    let mut rest = rendered;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let name = &after[..close];
        if !name.is_empty()
            && name.len() < 40
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        {
            return Err(err(format!(
                "{file} 里有 Autome 不认识的占位符 `{{{name}}}`"
            )));
        }
        rest = &after[close + 1..];
    }
    Ok(())
}

fn render_decisions(decisions: &[DecisionRecord]) -> String {
    use autome_domain::task::Disposition;
    if decisions.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n用户已对以下待决条目作出决定，请在本轮一并处理：\n\n");
    for d in decisions {
        let verdict = match (d.kind.as_str(), d.disposition) {
            ("backlog", Disposition::Include) => "纳入：作为新的里程碑实现".to_string(),
            ("backlog", Disposition::Ignore) => "忽略：记入 retro，不做".to_string(),
            ("dispute", Disposition::Ruled) => match &d.ruling {
                Some(r) if !r.trim().is_empty() => format!("裁定：{r}"),
                _ => "裁定：按用户在界面上的选择处理".to_string(),
            },
            _ => continue,
        };
        out.push_str(&format!("- {} {} → {verdict}\n", d.item_id, d.text));
    }
    out.push_str("\n纳入的条目要写进设计文档的里程碑表并实现；忽略与裁定要写进 retro.md。\n");
    out
}

/// The intake session's prompt.
///
/// Like the role prompts, the text lives in the protocol version
/// (`prompts/intake.md`). The three placeholders the core fills are the
/// request verbatim, the slug, and the design-round limit — which used to be
/// the literal `15` here regardless of what the project had configured, so a
/// project that lowered `design_rounds` got a skeleton status block claiming a
/// limit it did not have.
fn intake_prompt(spec: &PromptSpec<'_>) -> Result<String> {
    let template = spec
        .templates
        .prompt("intake")
        .ok_or_else(|| err("协议版本里没有 prompts/intake.md，无法启动任务整理轮"))?;

    let mut inputs = String::new();
    if !spec.attachments.is_empty() {
        inputs.push_str(&format!(
            "用户提供的附件（已复制到任务目录）：\n{}\n\n",
            spec.attachments
                .iter()
                .map(|a| format!("- {}/attachments/{a}", spec.doc_dir))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if !spec.doc_refs.is_empty() {
        inputs.push_str(&format!(
            "用户指定的仓库内参考文档：\n{}\n\n",
            spec.doc_refs
                .iter()
                .map(|d| format!("- {d}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    let rendered = template
        .replace("{doc_dir}", spec.doc_dir)
        .replace("{request}", spec.request)
        .replace("{inputs}", &inputs)
        .replace("{slug}", spec.slug)
        .replace("{design_rounds}", &spec.design_rounds.to_string());
    check_no_placeholders_left(&rendered, "prompts/intake.md")?;
    Ok(rendered)
}

fn onboarding_prompt(spec: &PromptSpec<'_>) -> Result<String> {
    let template = spec
        .templates
        .prompt("onboarding")
        .ok_or_else(|| err("协议版本里没有 prompts/onboarding.md，无法启动项目上手轮"))?;
    check_no_placeholders_left(template, "prompts/onboarding.md")?;
    Ok(template.to_string())
}

// ---------------------------------------------------------------------------
// Launching
// ---------------------------------------------------------------------------

/// Where a session is started.
///
/// This is a parameter rather than an environment variable, and that is not a
/// style preference. It used to be `AUTOMED_HEADLESS`, which meant the default
/// was "open a real terminal window" for anything that did not set it — and
/// the scheduler's own unit tests did not set it. On a machine with the CLIs
/// installed, `cargo test` opened a Terminal window per test, each running a
/// wrapper script against a sandbox the test had already deleted.
///
/// A process-global switch also cannot be right for two things at once, and
/// this crate already fixed that same defect twice (the config root, the git
/// binary). Third time: make it an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchMode {
    /// Run the wrapper directly, with no window at all. **The default, and
    /// what the desktop uses.**
    ///
    /// A session used to open a visible terminal so the user could watch it.
    /// In practice a Loop opens six or more of them and every one steals
    /// focus, which interrupts whatever the user was doing — the opposite of
    /// what a background worker should do. The wrapper tees the CLI's stdout
    /// and stderr to the session log either way, so nothing is lost: the task
    /// panel opens that log, and `tail -f` follows it live.
    #[default]
    Headless,
    /// Write the prompt and record the launch, but start nothing. Unit tests
    /// that only care about the state transition use this.
    Dry,
}

/// Everything needed to start one session.
pub struct LaunchSpec<'a> {
    pub session_id: &'a str,
    pub task_id: &'a str,
    /// Absolute path to the worktree (or the repository root, for onboarding).
    pub cwd: &'a Path,
    /// Absolute path to the repository root — where `.autome/` lives, which is
    /// not the same directory for a worktree session.
    pub repo: &'a Path,
    pub runtime: Runtime,
    pub args: Vec<String>,
    pub prompt: String,
    pub mode: LaunchMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launched {
    pub log_path: String,
    pub prompt_path: String,
    /// Which terminal actually took the session, for the message that tells
    /// the user where to look.
    pub terminal: Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminal {
    /// The wrapper was executed directly, with no window.
    Headless,
    /// Nothing was started.
    Dry,
}

impl Terminal {
    pub fn as_str(self) -> &'static str {
        match self {
            Terminal::Headless => "headless",
            Terminal::Dry => "dry",
        }
    }
}

/// Writes the prompt file and starts the wrapper script in a terminal.
///
/// The session directory is per task (`.autome/output/sessions/<task-id>/`),
/// so parallel tasks cannot collide on a log or a pid file (design §17).
pub fn launch(spec: &LaunchSpec<'_>) -> Result<Launched> {
    let session_dir = spec.repo.join(SessionPaths::dir(spec.task_id));
    std::fs::create_dir_all(&session_dir)
        .map_err(|e| err(format!("无法创建会话目录 {}：{e}", session_dir.display())))?;

    let prompt_path = session_dir.join(format!("{}.prompt", spec.session_id));
    std::fs::write(&prompt_path, &spec.prompt)
        .map_err(|e| err(format!("无法写入 prompt 文件：{e}")))?;

    let log_path = spec
        .repo
        .join(SessionPaths::log(spec.task_id, spec.session_id));
    let wrapper = crate::init::wrapper_script(spec.repo);
    if !wrapper.exists() {
        return Err(err(format!(
            "缺少会话包装脚本 {}，请重新初始化项目",
            wrapper.display()
        )));
    }

    // A dry launch stops here: the prompt is on disk, which is what a caller
    // inspecting the launch wants, and nothing has been started.
    if spec.mode == LaunchMode::Dry {
        return Ok(Launched {
            log_path: log_path.to_string_lossy().into_owned(),
            prompt_path: prompt_path.to_string_lossy().into_owned(),
            terminal: Terminal::Dry,
        });
    }

    let binary = adapter(spec.runtime).binary;
    let resolved_binary = resolve_binary(spec.runtime)
        .ok_or_else(|| err(format!("PATH 中找不到 {binary}，请先在本地环境页安装")))?;

    let mut argv = vec![
        wrapper.to_string_lossy().into_owned(),
        spec.session_id.to_string(),
        session_dir.to_string_lossy().into_owned(),
        spec.runtime.as_str().to_string(),
        renderer_path(),
        resolved_binary,
        prompt_path.to_string_lossy().into_owned(),
    ];
    argv.extend(spec.args.iter().cloned());

    let terminal = run_in_terminal(&argv, spec.cwd)?;

    Ok(Launched {
        log_path: log_path.to_string_lossy().into_owned(),
        prompt_path: prompt_path.to_string_lossy().into_owned(),
        terminal,
    })
}

/// This executable's own path, which the wrapper invokes as
/// `automed render-stream` to turn Claude's `stream-json` into a readable log.
///
/// `-` when the path cannot be determined: the wrapper then pipes the stream
/// through unchanged. A raw JSONL log is ugly but complete, and that is a far
/// better failure than a session that will not start.
fn renderer_path() -> String {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "-".to_string())
}

/// Starts the wrapper detached, with no window at all — the only non-dry mode
/// there is. The wrapper tees the CLI's output to the session log, which is
/// where the task panel looks.
fn run_in_terminal(argv: &[String], cwd: &Path) -> Result<Terminal> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // A session runs `git` itself — that is how a round's work gets committed
    // — and `GIT_DIR`, `GIT_INDEX_FILE` and their relatives override the
    // directory it is standing in. Whatever set them, it was not the user
    // asking for this round to commit somewhere else.
    //
    // This is not hypothetical: a Git hook exports them, so a core started
    // from one hands every session a git pointed at the hook's repository.
    // Found because the end-to-end suite runs from a post-commit hook, where
    // the one test that did not swallow git's exit code reported 128 — and
    // the ones that did swallow it had been passing for the wrong reason.
    //
    // The core's own git calls were never exposed: `git::run` clears the
    // environment outright. This is the other half of the same rule.
    strip_git_env(&mut cmd);
    cmd.spawn()
        .map_err(|e| err(format!("无法启动包装脚本：{e}")))?;
    Ok(Terminal::Headless)
}

/// Removes every `GIT_*` variable from a child's environment.
///
/// All of them rather than a list of the dangerous ones: a session is meant to
/// behave like a shell the user opened in that checkout, and every `GIT_*` it
/// would inherit came from whatever started Autome rather than from the user.
/// Sessions never push, so nothing here is load-bearing for authentication.
fn strip_git_env(cmd: &mut Command) {
    for (key, _) in std::env::vars() {
        if key.starts_with("GIT_") {
            cmd.env_remove(key);
        }
    }
}

/// A single `sh -c`-safe command line: `cd <dir> && <wrapper> <args...>`.
/// Every element is single-quoted, so a path or a model name containing a
/// space or a quote cannot break out.
pub fn shell_command(argv: &[String], cwd: &Path) -> String {
    let mut parts = vec![format!("cd {}", sh_quote(&cwd.to_string_lossy()))];
    parts.push(
        argv.iter()
            .map(|a| sh_quote(a))
            .collect::<Vec<_>>()
            .join(" "),
    );
    // The wrapper exiting is not enough: the interactive shell the terminal
    // started is still sitting there at a prompt, so the window stays open and
    // the next session opens another one. A day's work left a dozen dead tabs
    // behind, and each new one stole focus from the app.
    //
    // `;` rather than `&&` — the shell must exit whether the session succeeded
    // or not. Nothing is lost by closing: the wrapper tees the CLI's stdout and
    // stderr to the session log, which the task panel opens.
    format!("{}; exit $?", parts.join(" && "))
}

/// POSIX single-quoting: wrap in `'`, and replace each embedded `'` with
/// `'\''`. Total — there is no input this cannot quote.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// AppleScript string literal quoting: escape backslashes and double quotes.
/// Applied *after* shell quoting, because the shell command is embedded in an
/// AppleScript string.
pub fn as_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', r"\\").replace('"', "\\\""))
}

/// The binary for a runtime: an explicit override if the user set one, else
/// the first match on PATH.
///
/// The override exists for two reasons: a user whose CLI lives outside the
/// PATH the app inherits from `launchd`, and the end-to-end test suite, which
/// points both runtimes at a stand-in so a whole task can be driven without a
/// real model. Same shape as `git`'s `AUTOMED_GIT_BINARY`.
pub fn resolve_binary(runtime: Runtime) -> Option<String> {
    let var = match runtime {
        Runtime::Claude => "AUTOMED_CLAUDE_BINARY",
        Runtime::Codex => "AUTOMED_CODEX_BINARY",
    };
    if let Ok(path) = std::env::var(var)
        && !path.is_empty()
    {
        return Some(path);
    }
    which(adapter(runtime).binary)
}

/// First executable match on PATH. Written here rather than shelling out to
/// `which`, which is itself a PATH lookup and one more process to fail.
pub fn which(binary: &str) -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    which_in(binary, &path, is_executable)
}

/// The testable core of `which`: PATH scanning against an injected
/// executability predicate.
pub fn which_in(binary: &str, path_var: &str, is_exec: impl Fn(&Path) -> bool) -> Option<String> {
    for dir in path_var.split(':') {
        // An empty PATH segment means the current directory in POSIX. For a
        // daemon that is a foot-gun, so it is skipped rather than honoured.
        if dir.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(dir).join(binary);
        if is_exec(&candidate) {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// Sends SIGTERM to the session's process group, then SIGKILL after a grace
/// period (requirement T-08's "立刻杀掉当前会话").
///
/// The *group* rather than the pid: the wrapper runs the CLI in a pipeline, so
/// killing only the wrapper would orphan the CLI and leave it writing to a log
/// nobody reads.
pub fn stop_session(pid: i32) -> Result<()> {
    if pid <= 1 {
        return Err(err(format!("拒绝对 pid {pid} 发信号")));
    }
    #[cfg(unix)]
    unsafe {
        // Negative pid targets the process group. The wrapper is the group
        // leader because the terminal starts it as its own job.
        libc::kill(-pid, libc::SIGTERM);
        std::thread::sleep(std::time::Duration::from_millis(3000));
        if libc::kill(-pid, 0) == 0 {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    Ok(())
}

/// Whether a pid is still alive, for the liveness backstop (design §17).
pub fn pid_alive(pid: i32) -> bool {
    if pid <= 1 {
        return false;
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(pid, 0) == 0
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// The fixed profile the two system sessions run under.
pub fn system_role_config() -> RoleConfig {
    RoleConfig {
        enabled: true,
        runtime: Runtime::Claude,
        // Empty means "whatever the CLI defaults to", which is right for a
        // step that is not part of the configurable Loop (requirement C-04).
        model: String::new(),
        effort: None,
        skills: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autome_domain::task::Disposition;

    fn role_config(runtime: Runtime, model: &str, effort: Option<&str>) -> RoleConfig {
        RoleConfig {
            enabled: true,
            runtime,
            model: model.into(),
            effort: effort.map(str::to_string),
            skills: vec![],
        }
    }

    /// The seed's templates. Tests here are about *rendering* — which
    /// placeholder gets what, which round is handed a budget line — not about
    /// the wording, which lives in the protocol repository and is checked by
    /// `protocol::phrases`.
    fn templates() -> autome_domain::protocol::ProtocolFiles {
        crate::protocol::seed().clone()
    }

    fn spec<'a>(
        kind: SessionKind,
        skills: &'a [String],
        inject: Option<&'a Inject>,
        templates: &'a autome_domain::protocol::ProtocolFiles,
    ) -> PromptSpec<'a> {
        PromptSpec {
            kind,
            templates,
            brief_path: "docs/checkout-flow/brief/impl-1.md",
            slug: "checkout-flow",
            doc_dir: "docs/checkout-flow",
            design_rounds: 15,
            task_metrics: None,
            budget: None,
            request: "加购物车结算",
            skills,
            inject,
            decisions: &[],
            attachments: &[],
            doc_refs: &[],
        }
    }

    // ---- adapter table ---------------------------------------------------

    #[test]
    fn every_runtime_has_exactly_one_adapter() {
        for runtime in Runtime::ALL {
            assert_eq!(adapter(runtime).runtime, runtime);
        }
        assert_eq!(ADAPTERS.len(), Runtime::ALL.len());
    }

    /// A directory Git cannot answer about, so `build_args` adds no writable
    /// root and these tests see only the flags they are about.
    fn no_repo() -> &'static [&'static Path] {
        &[]
    }

    /// A directory git cannot answer about, for the tests that only care
    /// about flags.
    fn nowhere() -> &'static Path {
        Path::new("/nonexistent-so-git-cannot-answer")
    }

    /// One checkout, as every single-repository task has.
    fn one(worktree: &Path) -> Vec<&Path> {
        vec![worktree]
    }

    #[test]
    fn an_absent_effort_adds_no_flag_on_either_runtime() {
        assert!(
            !build_args(
                &role_config(Runtime::Codex, "gpt-5.6-sol", None),
                nowhere(),
                no_repo()
            )
            .join(" ")
            .contains("reasoning_effort")
        );
        assert!(
            !build_args(
                &role_config(Runtime::Claude, "opus", None),
                nowhere(),
                no_repo()
            )
            .join(" ")
            .contains("--effort")
        );
    }

    #[test]
    fn an_empty_model_adds_no_model_flag() {
        let args = build_args(&system_role_config(), nowhere(), no_repo());
        assert!(!args.contains(&"--model".to_string()), "{args:?}");
    }

    #[test]
    fn a_worktree_session_on_codex_may_write_the_repositorys_git_directory() {
        // Without this, `git add` inside a worktree exits 128: the real Git
        // directory is `<repo>/.git/worktrees/<slug>`, outside the worktree
        // that `workspace-write` allows.
        let repo =
            std::env::temp_dir().join(format!("automed-writable-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(&repo).unwrap();
        let repo = repo.canonicalize().unwrap();
        let repo = repo.as_path();
        crate::git::run(repo, &["init", "-q"]).unwrap();
        let worktree = repo.join("wt");
        crate::git::run(repo, &["commit", "-q", "--allow-empty", "-m", "base"]).unwrap();
        crate::git::run(
            repo,
            &[
                "worktree",
                "add",
                "-q",
                worktree.to_str().unwrap(),
                "-b",
                "t",
            ],
        )
        .unwrap();

        let args = build_args(
            &role_config(Runtime::Codex, "gpt-5.6-sol", None),
            &worktree,
            &one(&worktree),
        )
        .join(" ");
        assert!(
            args.contains("sandbox_workspace_write.writable_roots"),
            "{args}"
        );
        // The *common* directory, not `<repo>/.git/worktrees/<slug>`. The
        // narrower one lets `git add` take the lock and then fail on the
        // shared object store with `failed to insert into database`, which is
        // the same outcome one step later. Asserting only that some path is
        // there is what let that ship.
        let common = repo.join(".git");
        assert!(args.contains(common.to_str().unwrap()), "{args}");
        assert!(
            !args.contains("worktrees"),
            "asked for the per-worktree directory, which cannot hold the objects: {args}"
        );

        // In the repository itself the Git directory is already inside the
        // sandbox, so nothing is widened.
        let at_repo = build_args(
            &role_config(Runtime::Codex, "gpt-5.6-sol", None),
            repo,
            &one(repo),
        )
        .join(" ");
        assert!(!at_repo.contains("writable_roots"), "{at_repo}");

        // Claude has no sandbox of its own to widen.
        let claude = build_args(
            &role_config(Runtime::Claude, "opus", None),
            &worktree,
            &one(&worktree),
        )
        .join(" ");
        assert!(!claude.contains("writable_roots"), "{claude}");

        let _ = std::fs::remove_dir_all(repo);
    }

    #[test]
    fn a_working_directory_that_is_not_a_repository_gets_the_flag_that_allows_it() {
        // A workspace task's working directory holds one checkout per member
        // repository and is not one itself. Codex refuses to start there —
        // "Not inside a trusted directory" — and the round exits 1 before it
        // has read anything. A real review round died exactly that way.
        let ws = std::env::temp_dir().join(format!("automed-norepo-{}", std::process::id()));
        std::fs::create_dir_all(&ws).unwrap();
        let args =
            build_args(&role_config(Runtime::Codex, "gpt-5.6-sol", None), &ws, &[]).join(" ");
        assert!(args.contains("--skip-git-repo-check"), "{args}");
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn inside_a_repository_the_check_is_left_alone() {
        // The guard means something there — "this directory has no version
        // control" — so it keeps it. Passing the flag unconditionally would
        // have been one line shorter and would have turned it off everywhere.
        let repo = std::env::temp_dir().join(format!("automed-isrepo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(&repo).unwrap();
        crate::git::init(&repo, "main").unwrap();
        let args = build_args(
            &role_config(Runtime::Codex, "gpt-5.6-sol", None),
            &repo,
            &[],
        )
        .join(" ");
        assert!(!args.contains("--skip-git-repo-check"), "{args}");
        let _ = std::fs::remove_dir_all(&repo);
    }

    #[test]
    fn a_session_does_not_inherit_a_git_that_points_somewhere_else() {
        // `GIT_DIR` and friends override the directory git is standing in. A
        // session runs git itself, so inheriting them means a round's work is
        // committed into whatever repository set them — a Git hook, most
        // likely, since hooks export them and a core can be started from one.
        //
        // Asserted on the spawn rather than on a launch, because the effect is
        // the child's environment and nothing else observes it.
        let mut cmd = std::process::Command::new("/bin/sh");
        let before = ["GIT_DIR", "GIT_INDEX_FILE", "GIT_WORK_TREE"];
        for key in before {
            unsafe { std::env::set_var(key, "/somewhere/else") };
        }
        strip_git_env(&mut cmd);
        let removed: Vec<&str> = cmd
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .filter_map(|(k, _)| k.to_str())
            .collect();
        for key in before {
            assert!(removed.contains(&key), "{key} still reaches the session");
            unsafe { std::env::remove_var(key) };
        }
    }

    #[test]
    fn a_path_with_a_quote_in_it_does_not_break_the_toml() {
        // Parsed the way Codex parses it: `-c` takes one `key=value` line and
        // reads the value as TOML. Delegating to `toml` also covers the
        // control characters a two-`replace` escaper emitted raw — here a
        // newline, which is legal in a POSIX path.
        let root = std::path::PathBuf::from("/a\"b\\c\nd");
        let args = adapter(Runtime::Codex).extra_writable_roots(std::slice::from_ref(&root));
        let doc: toml::Value = args[1].parse().expect("`-c` line must be valid TOML");
        assert_eq!(
            doc["sandbox_workspace_write"]["writable_roots"][0]
                .as_str()
                .unwrap(),
            root.to_string_lossy(),
            "路径要原样还原回来：{}",
            args[1]
        );
    }

    // ---- prompts ---------------------------------------------------------

    #[test]
    fn a_role_prompt_names_the_round_it_is() {
        // The bug this replaced: every round was handed the same sentence
        // pointing at a task-file section that did not exist, so four real
        // sessions ran and produced nothing.
        for role in Role::ALL {
            let p =
                build_prompt(&spec(SessionKind::Role { role }, &[], None, &templates())).unwrap();
            assert!(
                p.contains(role.round_name()),
                "{role} prompt does not say which round it is:\n{p}"
            );
            assert!(
                !p.contains("additional task"),
                "{role} prompt still uses the old indirection:\n{p}"
            );
        }
    }

    #[test]
    fn every_role_prompt_names_the_files_it_reads_and_writes() {
        for role in Role::ALL {
            let p =
                build_prompt(&spec(SessionKind::Role { role }, &[], None, &templates())).unwrap();
            // Every round reads the task file.
            assert!(p.contains("checkout-flow-task.md"), "{role}: {p}");
            // Every round is pointed at its brief, which is the index into
            // everything else.
            assert!(p.contains("brief/"), "{role}: {p}");
            // Every round of the loop itself names the design document. The
            // retro round deliberately does not: it reads the evidence and the
            // numbers the core hands it, and adding the design document back
            // would put the largest file in the task directory in front of the
            // one round that has no use for it.
            if role != Role::Retro {
                assert!(
                    p.contains("docs/checkout-flow/checkout-flow.md"),
                    "{role}: {p}"
                );
            }
            // The three reviewing rounds each own an output document.
            if let Some(kind) = role.output_document() {
                assert!(
                    p.contains(&format!("checkout-flow-{kind}.md")),
                    "{role} does not name its own output document:\n{p}"
                );
            }
        }
    }

    #[test]
    fn a_refused_commit_is_explicitly_not_a_protocol_failure() {
        // An audit round did its job — found a real defect, reopened the
        // milestone, updated the tests — and then wrote `status: 协议失败`
        // because Codex's `workspace-write` sandbox would not let it write
        // `.git/index.lock`. The prompt had told it "没有提交的东西不会进入
        // 最终的合并", which is false: the core sweeps up what a session
        // leaves behind. A sentence that is not true about the system will be
        // acted on as if it were.
        for role in Role::ALL {
            let prompt =
                build_prompt(&spec(SessionKind::Role { role }, &[], None, &templates())).unwrap();
            assert!(
                prompt.contains("不是协议失败"),
                "{role:?} is not told that a refused commit is survivable"
            );
            assert!(
                !prompt.contains("没有提交的东西不会进入最终的合并"),
                "{role:?} still carries the claim that made an audit stop the task"
            );
        }
    }

    #[test]
    fn every_role_prompt_says_what_protocol_failure_is_for() {
        for role in Role::ALL {
            let prompt =
                build_prompt(&spec(SessionKind::Role { role }, &[], None, &templates())).unwrap();
            assert!(
                prompt.contains("`协议失败` 只用于一种情况"),
                "{role:?} does not narrow what 协议失败 means"
            );
        }
    }

    #[test]
    fn every_role_prompt_asks_for_a_commit() {
        // A real run did all the work correctly and committed none of it, so
        // the branch was identical to its base and the merge would have
        // brought nothing. The core sweeps up afterwards, but a round that
        // commits its own work produces a legible history.
        for role in Role::ALL {
            let p =
                build_prompt(&spec(SessionKind::Role { role }, &[], None, &templates())).unwrap();
            assert!(p.contains("git commit"), "{role}: {p}");
        }
    }

    // ---- the 2026-09-16 protocol audit ------------------------------------

    fn budgeted(role: Role, round: u32, limit: u32) -> String {
        let tpl = templates();
        let mut s = spec(SessionKind::Role { role }, &[], None, &tpl);
        s.budget = Some(BudgetLine { round, limit });
        build_prompt(&s).unwrap()
    }

    #[test]
    fn the_implementation_round_is_told_which_round_it_is_and_what_n_is() {
        // S5. The run of 2026-09-16: the protocol said N was "the milestone
        // count times a factor" without naming the factor, the round guessed
        // 2, wrote `implementation-round: 14/14` against the core's 35,
        // declared the budget spent — and the core, which disagreed,
        // scheduled another round. The next session invented "the user
        // released one more round" to reconcile the two.
        let p = budgeted(Role::Impl, 7, 35);
        assert!(p.contains("实现轮第 7 轮"), "{p}");
        assert!(p.contains("N = 35"), "{p}");
        assert!(p.contains("`7/35`"), "the round is told what to write: {p}");
        assert!(p.contains("不要自己按里程碑数推算"), "{p}");
    }

    #[test]
    fn the_audit_round_is_given_n_but_not_asked_to_enforce_it() {
        // The same run: an audit round stopped "on budget" on its own
        // initiative. Deciding that is the core's job, and the core did not
        // agree with the number the audit was reading.
        let p = budgeted(Role::Audit, 4, 35);
        assert!(p.contains("N = 35"), "{p}");
        assert!(p.contains("你不需要据此终止"), "{p}");
        assert!(
            !p.contains("实现轮第 4 轮"),
            "the audit does not increment k: {p}"
        );
    }

    #[test]
    fn the_three_rounds_outside_the_implementation_loop_get_no_budget_line() {
        for role in [Role::Plan, Role::Review, Role::Adjudicate] {
            let p = budgeted(role, 3, 35);
            assert!(!p.contains("N = 35"), "{role} has no business with N:\n{p}");
        }
    }

    #[test]
    fn a_round_with_no_budget_yet_is_told_nothing_about_it() {
        // Before the design is approved there is no N. Saying "N = 0" would
        // be worse than saying nothing.
        for role in Role::ALL {
            let p =
                build_prompt(&spec(SessionKind::Role { role }, &[], None, &templates())).unwrap();
            assert!(!p.contains("实现预算 N"), "{role}: {p}");
        }
    }

    #[test]
    fn the_implementation_round_self_checks_before_claiming_pending() {
        // S3. All nine implementation defects of the last run were of a kind
        // the design already listed: state transitions, named failure
        // branches, input-domain edges. Each cost an implementation round and
        // an audit round to find.
        let p = budgeted(Role::Impl, 1, 10);
        assert!(p.contains("自审清单"), "{p}");
        for item in ["情形表逐行", "失败分支", "状态迁移", "输入域边界"] {
            assert!(p.contains(item), "self-check item `{item}` missing:\n{p}");
        }
        assert!(p.contains("用例总数"), "S7a: {p}");
    }

    #[test]
    fn the_audit_round_has_three_verdicts_including_a_verification_gap() {
        // S7c. 2.0 dropped the verification-gap verdict that 1.x had, and the
        // audits kept doing it anyway — 30 gaps against 14 defects in one run
        // — with no rule to do it under.
        let p = budgeted(Role::Audit, 1, 10);
        assert!(p.contains("结论三选一"), "{p}");
        assert!(p.contains("验证缺口"), "{p}");
        assert!(p.contains("不计 reopen、不退回实现轮"), "{p}");
        // S7b.
        assert!(p.contains("复现不了不等于不存在"), "{p}");
    }

    #[test]
    fn the_audit_does_not_re_run_earlier_temporary_checks() {
        // S4. One audit re-ran 346 + 76 + 72 checks left behind by earlier
        // rounds. They live in `.autome/output/`, which is gitignored, so
        // none of them survives the merge either.
        let p = budgeted(Role::Audit, 5, 20);
        assert!(p.contains("不要重跑前几轮"), "{p}");
        assert!(p.contains("搬进项目测试体系"), "{p}");
        // And the other half of the handshake: the next implementation round
        // is the one that promotes the check.
        let impl_p = budgeted(Role::Impl, 6, 20);
        assert!(impl_p.contains("逐字搬进项目测试体系"), "{impl_p}");
        assert!(impl_p.contains("回归用例"), "{impl_p}");
    }

    #[test]
    fn both_loop_rounds_write_evidence_to_a_file_and_one_line_to_retro() {
        // S1 and S2.
        for role in [Role::Impl, Role::Audit] {
            let p = budgeted(role, 2, 10);
            assert!(
                p.contains("docs/checkout-flow/evidence/"),
                "{role} is not told where evidence goes:\n{p}"
            );
            assert!(
                p.contains("轮次 | 里程碑 | 结果 | 证据 | 阻塞"),
                "{role} is not given the one-line retro format:\n{p}"
            );
        }
        // The design document takes a pointer, never the evidence itself.
        let impl_p = budgeted(Role::Impl, 2, 10);
        assert!(impl_p.contains("最新证据："), "{impl_p}");
        assert!(impl_p.contains("不要把证据正文"), "{impl_p}");
        let audit_p = budgeted(Role::Audit, 2, 10);
        assert!(audit_p.contains("不要把审计结论抄进"), "{audit_p}");
    }

    #[test]
    fn the_design_round_keeps_human_only_acceptance_out_of_the_milestones() {
        // S6. "End-to-end acceptance on real hardware" was made the last
        // milestone of a real run. It needs a real mouse and a real
        // microphone, so no session could ever close it.
        let p = build_prompt(&spec(
            SessionKind::Role { role: Role::Plan },
            &[],
            None,
            &templates(),
        ))
        .unwrap();
        assert!(p.contains("## 人工验收清单"), "{p}");
        assert!(p.contains("不做里程碑验收条件"), "{p}");
        // And the parser's lesson, stated where the table is written.
        assert!(p.contains("只放里程碑表那一张表格"), "{p}");
    }

    #[test]
    fn every_role_prompt_forbids_starting_the_next_session() {
        // The core schedules; a round that relays would bypass the parallel
        // limit, pause, the role toggles and the budgets.
        for role in Role::ALL {
            let p =
                build_prompt(&spec(SessionKind::Role { role }, &[], None, &templates())).unwrap();
            assert!(p.contains("不要启动下一个会话"), "{role}: {p}");
        }
    }

    #[test]
    fn the_adjudication_round_is_told_it_owns_the_design_final_signal() {
        // Nothing else flips `status` off 设计中, and that flip is the only
        // thing that moves the task to the approval stop.
        let p = build_prompt(&spec(
            SessionKind::Role {
                role: Role::Adjudicate,
            },
            &[],
            None,
            &templates(),
        ))
        .unwrap();
        assert!(p.contains("实现中"), "{p}");
        assert!(p.contains("里程碑表"), "{p}");
        assert!(p.contains("design-round"), "{p}");
    }

    #[test]
    fn the_implement_round_is_forbidden_from_closing_a_milestone() {
        let p = build_prompt(&spec(
            SessionKind::Role { role: Role::Impl },
            &[],
            None,
            &templates(),
        ))
        .unwrap();
        assert!(p.contains("不得把里程碑标成 `已完成`"), "{p}");
    }

    #[test]
    fn the_audit_round_is_told_to_verify_independently() {
        let p = build_prompt(&spec(
            SessionKind::Role { role: Role::Audit },
            &[],
            None,
            &templates(),
        ))
        .unwrap();
        assert!(p.contains("独立复验"), "{p}");
        assert!(p.contains("不要以实现轮的说法为准"), "{p}");
    }

    #[test]
    fn bound_skills_are_stated_as_mandatory() {
        let skills = vec!["conventions".to_string(), "vitest".to_string()];
        let p = build_prompt(&spec(
            SessionKind::Role { role: Role::Impl },
            &skills,
            None,
            &templates(),
        ))
        .unwrap();
        assert!(p.contains("必须使用"), "{p}");
        assert!(p.contains("conventions、vitest"), "{p}");
    }

    #[test]
    fn no_skills_means_no_skill_sentence() {
        let p = build_prompt(&spec(
            SessionKind::Role { role: Role::Impl },
            &[],
            None,
            &templates(),
        ))
        .unwrap();
        assert!(!p.contains("必须使用"), "{p}");
    }

    #[test]
    fn design_feedback_is_injected_verbatim() {
        let inject = Inject::DesignFeedback {
            feedback: "确认邮件只发登录用户\n不要发给游客".into(),
        };
        let p = build_prompt(&spec(
            SessionKind::Role { role: Role::Plan },
            &[],
            Some(&inject),
            &templates(),
        ))
        .unwrap();
        assert!(p.contains("确认邮件只发登录用户\n不要发给游客"), "{p}");
        assert!(p.contains("不要改写"), "{p}");
    }

    #[test]
    fn a_rebase_conflict_lists_the_files() {
        let inject = Inject::RebaseConflict {
            files: vec!["src/a.ts".into(), "src/b.ts".into()],
        };
        let p = build_prompt(&spec(
            SessionKind::Role { role: Role::Impl },
            &[],
            Some(&inject),
            &templates(),
        ))
        .unwrap();
        assert!(p.contains("- src/a.ts"), "{p}");
        assert!(p.contains("- src/b.ts"), "{p}");
        assert!(p.contains("冲突"), "{p}");
    }

    #[test]
    fn decisions_render_their_verdicts_and_the_dispute_ruling() {
        let decisions = vec![
            DecisionRecord {
                task_id: "T-1".into(),
                kind: "backlog".into(),
                item_id: "B-01".into(),
                text: "优惠码次数上限".into(),
                disposition: Disposition::Include,
                ruling: None,
                consumed_at: None,
            },
            DecisionRecord {
                task_id: "T-1".into(),
                kind: "backlog".into(),
                item_id: "B-02".into(),
                text: "日志脱敏".into(),
                disposition: Disposition::Ignore,
                ruling: None,
                consumed_at: None,
            },
            DecisionRecord {
                task_id: "T-1".into(),
                kind: "dispute".into(),
                item_id: "D3-P02".into(),
                text: "大小写敏感".into(),
                disposition: Disposition::Ruled,
                ruling: Some("按评审方，不敏感".into()),
                consumed_at: None,
            },
        ];
        let tpl = templates();
        let mut s = spec(
            SessionKind::Role { role: Role::Impl },
            &[],
            Some(&Inject::Decisions),
            &tpl,
        );
        s.decisions = &decisions;
        let p = build_prompt(&s).unwrap();
        assert!(p.contains("B-01 优惠码次数上限 → 纳入"), "{p}");
        assert!(p.contains("B-02 日志脱敏 → 忽略"), "{p}");
        assert!(p.contains("按评审方，不敏感"), "{p}");
    }

    #[test]
    fn an_undecided_item_is_not_rendered() {
        let decisions = vec![DecisionRecord {
            task_id: "T-1".into(),
            kind: "backlog".into(),
            item_id: "B-09".into(),
            text: "没决定的".into(),
            disposition: Disposition::None,
            ruling: None,
            consumed_at: None,
        }];
        let tpl = templates();
        let mut s = spec(
            SessionKind::Role { role: Role::Impl },
            &[],
            Some(&Inject::Decisions),
            &tpl,
        );
        s.decisions = &decisions;
        assert!(!build_prompt(&s).unwrap().contains("B-09"));
    }

    #[test]
    fn the_intake_prompt_carries_the_request_verbatim_and_the_status_block_shape() {
        let p = build_prompt(&spec(SessionKind::Intake, &[], None, &templates())).unwrap();
        assert!(p.contains("加购物车结算"), "{p}");
        assert!(p.contains("不要改写，不要扩大范围"), "{p}");
        assert!(p.contains("status: 设计中"), "{p}");
        assert!(
            p.contains("docs/checkout-flow/checkout-flow-task.md"),
            "{p}"
        );
        assert!(p.contains("不要启动别的会话"), "{p}");
    }

    #[test]
    fn the_intake_prompt_points_at_the_tasks_own_copy_rather_than_embedding_the_protocol() {
        // It used to demand the whole protocol be pasted into the task file,
        // for a real reason: a task file that only *pointed* at the scaffold
        // stopped being self-contained the moment the scaffold was refreshed,
        // and an archived task directory could no longer explain its own
        // history.
        //
        // The copy under `docs/<slug>/protocol/` answers that better than the
        // paste did. It is frozen at creation, it travels with the branch, it
        // survives archival — and there is exactly one of it, so the task file
        // and the protocol can no longer disagree.
        let p = build_prompt(&spec(SessionKind::Intake, &[], None, &templates())).unwrap();
        assert!(
            p.contains("docs/checkout-flow/protocol/loop-protocol.md"),
            "{p}"
        );
        assert!(p.contains("不要把协议抄进任务文件"), "{p}");
        assert!(!p.contains("逐字复制"), "{p}");
    }

    #[test]
    fn the_intake_prompt_lists_attachments_and_doc_refs_when_present() {
        let tpl = templates();
        let mut s = spec(SessionKind::Intake, &[], None, &tpl);
        let attachments = vec!["promo.csv".to_string()];
        let refs = vec!["docs/notes.md".to_string()];
        s.attachments = &attachments;
        s.doc_refs = &refs;
        let p = build_prompt(&s).unwrap();
        assert!(
            p.contains("docs/checkout-flow/attachments/promo.csv"),
            "{p}"
        );
        assert!(p.contains("- docs/notes.md"), "{p}");
    }

    #[test]
    fn the_onboarding_prompt_asks_for_both_artefacts_and_forbids_touching_autome() {
        let p = build_prompt(&spec(SessionKind::Onboarding, &[], None, &templates())).unwrap();
        assert!(p.contains("docs/agent-project-profile.md"), "{p}");
        assert!(p.contains("AGENTS.md"), "{p}");
        assert!(p.contains("不要修改 .autome/"), "{p}");
        assert!(p.contains("空目录"), "empty-directory branch: {p}");
    }

    // ---- quoting ---------------------------------------------------------

    #[test]
    fn sh_quote_survives_embedded_single_quotes() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert_eq!(sh_quote("a b"), "'a b'");
    }

    /// The real property, checked by an actual shell rather than by guessing
    /// at substrings: whatever goes in comes back out as exactly one argument.
    #[test]
    fn sh_quote_round_trips_through_a_real_shell() {
        let inputs = [
            "plain",
            "it's",
            "a b",
            "'; rm -rf /; echo '",
            "$(whoami)",
            "`id`",
            "back\\slash",
            "new\nline",
            "中文 路径/带空格",
            "\"double\"",
        ];
        for input in inputs {
            let out = Command::new("/bin/sh")
                .arg("-c")
                .arg(format!("printf %s {}", sh_quote(input)))
                .output()
                .expect("sh");
            assert!(out.status.success(), "{input:?}");
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                input,
                "quoting changed the value of {input:?}"
            );
        }
    }

    /// And the same for a whole command line: the wrapper must receive each
    /// element as one argument, whatever is in it.
    #[test]
    fn a_quoted_command_line_reaches_the_program_as_distinct_arguments() {
        let argv = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "printf '%s\\n' \"$@\"".to_string(),
            "--".to_string(),
            "first arg".to_string(),
            "it's second".to_string(),
            "$(echo pwned)".to_string(),
        ];
        let line = shell_command(&argv, Path::new("/tmp"));
        let out = Command::new("/bin/sh")
            .arg("-c")
            .arg(&line)
            .output()
            .expect("sh");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(
            lines,
            vec!["first arg", "it's second", "$(echo pwned)"],
            "command line was: {line}"
        );
    }

    #[test]
    fn as_quote_escapes_backslashes_and_double_quotes() {
        assert_eq!(as_quote("plain"), "\"plain\"");
        assert_eq!(as_quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(as_quote(r"a\b"), r#""a\\b""#);
    }

    #[test]
    fn the_shell_exits_so_the_terminal_window_closes() {
        // Without this the wrapper finishes and the interactive shell sits at
        // a prompt forever: every session left a window open, and each new one
        // stole focus from the app. `;` not `&&` — a failed session must close
        // too, and its output is in the session log either way.
        let argv = vec!["/repo/.autome/skill/run_session.sh".to_string()];
        let line = shell_command(&argv, Path::new("/repo/.worktree/x"));
        assert!(line.ends_with("; exit $?"), "{line}");
        assert!(
            !line.contains("&& exit"),
            "a crashed session must still close its window: {line}"
        );
    }

    #[test]
    fn a_shell_command_quotes_every_element_including_the_directory() {
        let argv = vec![
            "/path/with space/run.sh".to_string(),
            "arg with 'quote'".to_string(),
        ];
        let cmd = shell_command(&argv, Path::new("/dir with space"));
        assert!(cmd.starts_with("cd '/dir with space' && "), "{cmd}");
        assert!(cmd.contains(r"'/path/with space/run.sh'"), "{cmd}");
        assert!(cmd.contains(r"'arg with '\''quote'\'''"), "{cmd}");
    }

    // ---- which -----------------------------------------------------------

    #[test]
    fn which_in_returns_the_first_executable_match() {
        let found = which_in("claude", "/a:/b:/c", |p| {
            p == Path::new("/b/claude") || p == Path::new("/c/claude")
        });
        assert_eq!(found.as_deref(), Some("/b/claude"));
    }

    #[test]
    fn which_in_skips_empty_path_segments() {
        use std::cell::RefCell;
        let seen = RefCell::new(Vec::new());
        let found = which_in("x", "::/a", |p| {
            seen.borrow_mut().push(p.to_string_lossy().into_owned());
            false
        });
        assert_eq!(found, None);
        assert_eq!(seen.into_inner(), vec!["/a/x".to_string()]);
    }

    #[test]
    fn which_in_returns_none_when_nothing_matches() {
        assert_eq!(which_in("ghost", "/a:/b", |_| false), None);
    }

    // ---- misc ------------------------------------------------------------

    #[test]
    fn stopping_an_implausible_pid_is_refused_rather_than_signalling_everything() {
        // -1 to `kill` means "every process we may signal". Guarding this is
        // the difference between stopping a session and stopping the machine.
        for pid in [-1, 0, 1] {
            assert!(stop_session(pid).is_err(), "pid {pid} must be refused");
        }
    }

    #[test]
    fn pid_alive_is_false_for_implausible_pids_and_true_for_our_own() {
        assert!(!pid_alive(0));
        assert!(!pid_alive(-1));
        assert!(pid_alive(std::process::id() as i32));
    }

    #[test]
    fn a_dry_launch_writes_the_prompt_and_starts_nothing() {
        // What this guards: the scheduler's unit tests reach the launcher, and
        // with the old environment-variable switch they opened a real Terminal
        // window each — on the developer's machine, running a wrapper against
        // a sandbox the test had already deleted.
        let dir = std::env::temp_dir().join(format!("automed-dry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::init::init(&dir, crate::protocol::seed()).unwrap();
        let launched = launch(&LaunchSpec {
            session_id: "s-dry",
            task_id: "T-1",
            cwd: &dir,
            repo: &dir,
            runtime: Runtime::Claude,
            args: vec![],
            prompt: "the prompt".into(),
            mode: LaunchMode::Dry,
        })
        .unwrap();
        assert_eq!(launched.terminal, Terminal::Dry);
        assert_eq!(
            std::fs::read_to_string(&launched.prompt_path).unwrap(),
            "the prompt",
            "the prompt is still written, so a caller can inspect the launch"
        );
        assert!(
            !std::fs::exists(dir.join(".autome/output/sessions/T-1/s-dry.pid")).unwrap_or(false),
            "nothing was started"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_default_launch_mode_is_a_terminal() {
        // Production shows the user what is happening (design §1). The tests
        // opt out explicitly; nothing opts in by forgetting.
        assert_eq!(
            LaunchMode::default(),
            LaunchMode::Headless,
            "a session must not open a window unless someone asked for one"
        );
    }

    #[test]
    fn launching_without_a_wrapper_script_fails_with_a_useful_message() {
        let dir = std::env::temp_dir().join(format!("automed-launch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let spec = LaunchSpec {
            session_id: "s1",
            task_id: "T-1",
            cwd: &dir,
            repo: &dir,
            runtime: Runtime::Claude,
            args: vec![],
            prompt: "hello".into(),
            mode: LaunchMode::Dry,
        };
        let err = launch(&spec).unwrap_err();
        assert!(err.detail.contains("包装脚本"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn launching_writes_the_prompt_file_before_it_needs_the_terminal() {
        let dir = std::env::temp_dir().join(format!("automed-launch2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::init::init(&dir, crate::protocol::seed()).unwrap();
        let spec = LaunchSpec {
            session_id: "s1",
            task_id: "T-1",
            cwd: &dir,
            repo: &dir,
            runtime: Runtime::Claude,
            args: vec![],
            prompt: "hello prompt".into(),
            mode: LaunchMode::Dry,
        };
        // The launch itself may fail (no `claude` on PATH in CI), but the
        // prompt must already be on disk by then.
        let _ = launch(&spec);
        let prompt = dir.join(".autome/output/sessions/T-1/s1.prompt");
        if prompt.exists() {
            assert_eq!(std::fs::read_to_string(&prompt).unwrap(), "hello prompt");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
