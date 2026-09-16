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
        // `-p` is non-interactive mode; `acceptEdits` lets the session edit
        // files and run commands inside its worktree without prompting, which
        // it must be able to do since nobody is watching for a prompt.
        //
        // 2.0 deliberately adds no sandbox of its own (design §17): the
        // confinement is the working directory and the protocol.
        autonomous_flags: &["-p", "--permission-mode", "acceptEdits"],
        model_flag: "--model",
        effort_flag: Some("--effort"),
    },
    RuntimeAdapter {
        runtime: Runtime::Codex,
        binary: "codex",
        autonomous_flags: &["exec", "--sandbox", "workspace-write"],
        model_flag: "--model",
        // Codex takes reasoning effort as a config override rather than a
        // dedicated flag.
        effort_flag: Some("--config"),
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
pub fn build_args(config: &RoleConfig) -> Vec<String> {
    let a = adapter(config.runtime);
    let mut args: Vec<String> = a
        .autonomous_flags
        .iter()
        .map(|s| (*s).to_string())
        .collect();
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

/// Everything needed to write a session's prompt.
pub struct PromptSpec<'a> {
    pub kind: SessionKind,
    pub slug: &'a str,
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
pub fn build_prompt(spec: &PromptSpec<'_>) -> String {
    let mut p = String::new();

    match spec.kind {
        SessionKind::Intake => {
            p.push_str(&intake_prompt(spec));
            return p;
        }
        SessionKind::Onboarding => {
            p.push_str(&onboarding_prompt());
            return p;
        }
        SessionKind::Role { role } => {
            p.push_str(&role_prompt(role, spec.slug));
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

    p
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
/// Each prompt therefore states three things and nothing else: which round
/// this is, which files to read and write, and where the rules are. The rules
/// themselves stay in the task file, which the intake round embedded them into.
fn role_prompt(role: Role, slug: &str) -> String {
    let task_file = format!("docs/{slug}/{slug}-task.md");
    let design = format!("docs/{slug}/{slug}.md");
    let review = format!("docs/{slug}/{slug}-review.md");
    let adjudication = format!("docs/{slug}/{slug}-adjudication.md");
    let audit = format!("docs/{slug}/{slug}-audit.md");
    let retro = format!("docs/{slug}/retro.md");

    let body = match role {
        Role::Plan => format!(
            "先读 `{task_file}`（任务目标、范围、约束和 Loop 协议全文都在里面），\
             再读 `{design}`。如果存在 `{review}` 与 `{adjudication}`，也要读——\
             它们是上一轮评审提出的问题和对这些问题的裁决，本轮必须按裁决修改设计。\n\n\
             本轮产出：更新 `{design}`。写清背景、目标与非目标、方案、风险与验证安排，\
             并把工作拆成里程碑表（格式见协议「里程碑」一节，Autome 按列读取）。\n\n\
             不要写实现代码，不要改 `{review}` 或 `{adjudication}`。"
        ),
        Role::Review => format!(
            "先读 `{task_file}`，再独立复核 `{design}`。\n\n\
             本轮产出：覆盖写 `{review}`，逐条列出问题。\
             **只有协议「可以要求再评审一轮的六类问题」里的六类才能提**，\
             每条必须写明它违反的任务要求编号、设计条款或里程碑验收命令；\
             追溯不了的一律写进「非阻塞建议」，由裁决轮决定是否进 Backlog。\
             没有问题时也要写出这个结论。\n\n\
             不要修改 `{design}`，不要实现代码。"
        ),
        Role::Adjudicate => format!(
            "先读 `{task_file}`、`{design}` 和本轮的 `{review}`；\
             如果 `{adjudication}` 已存在，读它了解此前的裁决与复提计数。\n\n\
             本轮产出三件事：\n\n\
             1. **追加**（不是覆盖）到 `{adjudication}`：本轮轮次、评审结论，\
                以及逐条裁决记录（稳定 ID、主张摘要、设计位置、裁决、证据或理由、\
                修改落点、复提计数）。复提计数达到 2 的主张冻结为争议项，\
                写进 `{design}` 的「## 争议项」小节。\n\
             2. 按采纳的裁决修改 `{design}`，并把 `design-round` 加 1。\n\
             3. **判断设计是否定稿。** 若已没有剩余的六类问题：把 `{design}` 状态块的 \
                `status` 从 `设计中` 改为 `实现中`，并填好完整的里程碑表——\
                这是 Autome 判断「可以停下来等用户批准」的唯一信号。\
                若仍有问题，`status` 保持 `设计中`。\n\n\
             注意：状态块里的 `design-round` 只由本轮增加，评审轮不增加。"
        ),
        Role::Impl => format!(
            "先读 `{task_file}` 和 `{design}`；如果 `{audit}` 存在，读它——\
             上一轮审计退回的里程碑和原因在里面，本轮要先处理。\n\n\
             本轮产出：推进**编号最小的「开放」里程碑**，取得该里程碑验收命令的通过证据，\
             把它在里程碑表里标成 `待审`，并更新状态块的 `implementation-round`（加 1）\
             与 `next-action`。一轮只推进一个里程碑。\
             在 `{retro}` 追加一行本轮记录。\n\n\
             **不得把里程碑标成 `已完成`**——只有审计轮独立复验通过才能关闭它。\
             超过 5 行的命令输出写进 `.autome/output/`，不要进任务目录。"
        ),
        Role::Audit => format!(
            "先读 `{task_file}` 和 `{design}`，找出状态为 `待审` 的里程碑。\n\n\
             本轮产出：**独立复验**——自己跑该里程碑的验收命令，\
             自己构造能区分错误实现的检查，不要以实现轮的说法为准。\
             结论覆盖写进 `{audit}`，并在 `{retro}` 追加一行。\n\n\
             结论二选一：\n\n\
             - **通过** → 在里程碑表里标成 `已完成`。\n\
             - **有实现缺陷** → 退回 `开放`，`reopen` 加 1，在「领域」列按稳定的行为领域名归组，\
               并按协议「收敛模式」更新 `convergence-mode`。\n\n\
             只以「产品行为不符合设计或任务」为缺陷。代码风格、超出验收范围的健壮性、\
             性能微优化、测试还可以更多等属于改进建议，写进 `{design}` 的「## Backlog」，\
             不得据此退回实现轮。"
        ),
    };

    format!(
        "你是本任务的**{round}**。本轮在 worktree 内独立完成，完成后结束会话——\
         **不要启动下一个会话**，下一个节点由 Autome 调度。\n\n{body}\n\n\
         结束前把本轮的改动提交到当前分支（`git add` + `git commit`）。\
         任务文档和代码改动都要提交——没有提交的东西不会进入最终的合并。\n\n\
         状态块格式必须严格符合 `{task_file}` 中「Loop 协议」一节与 \
         `.autome/skill/session-protocol.md` 的规定；格式错一次即判协议失败，任务会停下等人。\n",
        round = role.round_name()
    )
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

/// The intake session's prompt. This is the one place 2.0 still leans on a
/// 1.x asset: the `loop-task` skill knows how to turn a one-line request into
/// a task file. Rather than duplicate that knowledge, the prompt asks for the
/// same artefacts and states the 2.0-specific constraints the skill predates.
fn intake_prompt(spec: &PromptSpec<'_>) -> String {
    let mut p = format!(
        "把下面这句需求整理成一个可执行的循环任务。\n\n需求原文（不要改写，不要扩大范围）：\n\n{}\n\n",
        spec.request
    );
    if !spec.attachments.is_empty() {
        p.push_str(&format!(
            "用户提供的附件（已复制到任务目录）：\n{}\n\n",
            spec.attachments
                .iter()
                .map(|a| format!("- docs/{}/attachments/{a}", spec.slug))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if !spec.doc_refs.is_empty() {
        p.push_str(&format!(
            "用户指定的仓库内参考文档：\n{}\n\n",
            spec.doc_refs
                .iter()
                .map(|d| format!("- {d}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    p.push_str(&format!(
        r#"请完成三件事：

1. 调研本仓库，读 AGENTS.md、docs/agent-project-profile.md（若存在）
   与 .autome/rules/ 下的规则。
2. 生成任务文件 docs/{slug}/{slug}-task.md。它必须是自包含的——执行后续各轮的
   会话不会读到别的说明文件，所以任务文件里要有：
   - 本任务的目标、范围与硬性约束（从上面那句需求和你的调研中得出）；
   - 生成时的项目背景摘要（技术栈、布局、测试命令、权威规则文件）；
   - **`.autome/skill/loop-protocol.md` 的全文，逐字复制**，不要只写路径、
     不要概括、不要改写。任务目录归档多年后仍要能凭它复现当时的规则。
   另见 .autome/skill/session-protocol.md 的会话边界，同样逐条遵守。
3. 生成设计文档 docs/{slug}/{slug}.md 的骨架，头部写完整的状态块：

```text
status: 设计中
design-round: 0/{design_rounds}
implementation-round: 0/0
current-milestone: 无
current-milestone-reopens: 0
convergence-mode: normal
next-action: 无
```

同时为这个任务起一个简短准确的标题，写在设计文档的一级标题里。

注意：本轮只做整理，不要开始设计、不要写代码、不要启动别的会话。完成后结束会话。
"#,
        slug = spec.slug,
        design_rounds = 15
    ));
    p
}

fn onboarding_prompt() -> String {
    r#"这是一个刚被 Autome 接管的项目，请为它建立 Agent 可用的项目背景。

请完成两件事：

1. 生成 docs/agent-project-profile.md —— 项目画像。内容应当是可核对的事实，
   不是评价：技术栈与版本、源码布局、构建与测试命令、运行环境、已有的权威规则文件、
   以及调研入口（从哪里开始读代码）。不要写流程协议，不要写里程碑状态机。
2. 补充 AGENTS.md 中 <!-- autome:begin --> 标记之外的部分，写这个项目对 Agent 的
   硬性约束。约束要可检验，例如「所有对外接口必须有契约测试」，而不是「代码要优雅」。

如果这是一个空目录、没有代码可读，请直接问用户三个问题：这个项目要做什么、
给谁用、技术栈倾向。拿到回答后再写这两个文件。

不要修改 .autome/ 下的任何内容。完成后结束会话。
"#
    .to_string()
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
    /// Hand the command to iTerm2 or Terminal, where the user can watch it.
    #[default]
    Terminal,
    /// Run the wrapper directly, with no window. The end-to-end suites use
    /// this: everything up to and including the wrapper is real.
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
    pub title: String,
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
    ITerm2,
    Terminal,
    /// The wrapper was executed directly, with no window.
    Headless,
    /// Nothing was started.
    Dry,
}

impl Terminal {
    pub fn as_str(self) -> &'static str {
        match self {
            Terminal::ITerm2 => "iTerm2",
            Terminal::Terminal => "Terminal",
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
        resolved_binary,
        prompt_path.to_string_lossy().into_owned(),
    ];
    argv.extend(spec.args.iter().cloned());

    let terminal = run_in_terminal(spec.mode, &argv, spec.cwd, &spec.title)?;

    Ok(Launched {
        log_path: log_path.to_string_lossy().into_owned(),
        prompt_path: prompt_path.to_string_lossy().into_owned(),
        terminal,
    })
}

/// Which terminal to use. iTerm2 when present, Terminal otherwise
/// (requirement E-02).
fn run_in_terminal(mode: LaunchMode, argv: &[String], cwd: &Path, title: &str) -> Result<Terminal> {
    if mode == LaunchMode::Headless || cfg!(test) {
        // `cfg!(test)` is a backstop, not the mechanism: a unit test inside
        // this crate that forgets to pass `Dry` still must not open a window
        // on the developer's machine.
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd.spawn()
            .map_err(|e| err(format!("无法启动包装脚本：{e}")))?;
        return Ok(Terminal::Headless);
    }

    let script = shell_command(argv, cwd);
    if Path::new("/Applications/iTerm.app").exists() {
        run_osascript(&iterm_applescript(&script, title))?;
        Ok(Terminal::ITerm2)
    } else {
        run_osascript(&terminal_applescript(&script))?;
        Ok(Terminal::Terminal)
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

fn iterm_applescript(command: &str, title: &str) -> String {
    format!(
        r#"tell application "iTerm"
    activate
    if (count of windows) = 0 then
        set newWindow to (create window with default profile)
        set targetSession to current session of newWindow
    else
        tell current window
            set newTab to (create tab with default profile)
            set targetSession to current session of newTab
        end tell
    end if
    tell targetSession
        set name to {title}
        write text {command}
    end tell
end tell"#,
        title = as_quote(title),
        command = as_quote(command)
    )
}

fn terminal_applescript(command: &str) -> String {
    format!(
        r#"tell application "Terminal"
    activate
    do script {command}
end tell"#,
        command = as_quote(command)
    )
}

fn run_osascript(script: &str) -> Result<()> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| err(format!("无法运行 osascript：{e}")))?;
    if !output.status.success() {
        return Err(err(format!(
            "终端拒绝启动会话：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
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

/// Builds the terminal tab title, e.g. `autome · T-15 · 实现 #4`.
pub fn tab_title(task_id: &str, kind: SessionKind, round: u32) -> String {
    match kind {
        SessionKind::Role { .. } => format!("autome · {task_id} · {} #{round}", kind.label()),
        _ => format!("autome · {task_id} · {}", kind.label()),
    }
}

/// The role whose session this is, for the store record. Intake and onboarding
/// always run Claude Code on a fixed prompt (design §7.1).
pub fn runtime_for(kind: SessionKind, config: Option<&RoleConfig>) -> Runtime {
    match (kind, config) {
        (SessionKind::Role { .. }, Some(c)) => c.runtime,
        _ => Runtime::Claude,
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

    fn spec<'a>(
        kind: SessionKind,
        skills: &'a [String],
        inject: Option<&'a Inject>,
    ) -> PromptSpec<'a> {
        PromptSpec {
            kind,
            slug: "checkout-flow",
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

    #[test]
    fn an_absent_effort_adds_no_flag_on_either_runtime() {
        assert!(
            !build_args(&role_config(Runtime::Codex, "gpt-5.6-sol", None))
                .join(" ")
                .contains("reasoning_effort")
        );
        assert!(
            !build_args(&role_config(Runtime::Claude, "opus", None))
                .join(" ")
                .contains("--effort")
        );
    }

    #[test]
    fn an_empty_model_adds_no_model_flag() {
        let args = build_args(&system_role_config());
        assert!(!args.contains(&"--model".to_string()), "{args:?}");
    }

    // ---- prompts ---------------------------------------------------------

    #[test]
    fn a_role_prompt_names_the_round_it_is() {
        // The bug this replaced: every round was handed the same sentence
        // pointing at a task-file section that did not exist, so four real
        // sessions ran and produced nothing.
        for role in Role::ALL {
            let p = build_prompt(&spec(SessionKind::Role { role }, &[], None));
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
            let p = build_prompt(&spec(SessionKind::Role { role }, &[], None));
            // Every round reads the task file and the design document.
            assert!(p.contains("checkout-flow-task.md"), "{role}: {p}");
            assert!(
                p.contains("docs/checkout-flow/checkout-flow.md"),
                "{role}: {p}"
            );
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
    fn every_role_prompt_asks_for_a_commit() {
        // A real run did all the work correctly and committed none of it, so
        // the branch was identical to its base and the merge would have
        // brought nothing. The core sweeps up afterwards, but a round that
        // commits its own work produces a legible history.
        for role in Role::ALL {
            let p = build_prompt(&spec(SessionKind::Role { role }, &[], None));
            assert!(p.contains("git commit"), "{role}: {p}");
            assert!(p.contains("不会进入最终的合并"), "{role}: {p}");
        }
    }

    #[test]
    fn every_role_prompt_forbids_starting_the_next_session() {
        // The core schedules; a round that relays would bypass the parallel
        // limit, pause, the role toggles and the budgets.
        for role in Role::ALL {
            let p = build_prompt(&spec(SessionKind::Role { role }, &[], None));
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
        ));
        assert!(p.contains("实现中"), "{p}");
        assert!(p.contains("里程碑表"), "{p}");
        assert!(p.contains("design-round"), "{p}");
    }

    #[test]
    fn the_implement_round_is_forbidden_from_closing_a_milestone() {
        let p = build_prompt(&spec(SessionKind::Role { role: Role::Impl }, &[], None));
        assert!(p.contains("不得把里程碑标成 `已完成`"), "{p}");
    }

    #[test]
    fn the_audit_round_is_told_to_verify_independently() {
        let p = build_prompt(&spec(SessionKind::Role { role: Role::Audit }, &[], None));
        assert!(p.contains("独立复验"), "{p}");
        assert!(p.contains("不要以实现轮的说法为准"), "{p}");
    }

    #[test]
    fn bound_skills_are_stated_as_mandatory() {
        let skills = vec!["conventions".to_string(), "vitest".to_string()];
        let p = build_prompt(&spec(SessionKind::Role { role: Role::Impl }, &skills, None));
        assert!(p.contains("必须使用"), "{p}");
        assert!(p.contains("conventions、vitest"), "{p}");
    }

    #[test]
    fn no_skills_means_no_skill_sentence() {
        let p = build_prompt(&spec(SessionKind::Role { role: Role::Impl }, &[], None));
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
        ));
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
        ));
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
        let mut s = spec(
            SessionKind::Role { role: Role::Impl },
            &[],
            Some(&Inject::Decisions),
        );
        s.decisions = &decisions;
        let p = build_prompt(&s);
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
        let mut s = spec(
            SessionKind::Role { role: Role::Impl },
            &[],
            Some(&Inject::Decisions),
        );
        s.decisions = &decisions;
        assert!(!build_prompt(&s).contains("B-09"));
    }

    #[test]
    fn the_intake_prompt_carries_the_request_verbatim_and_the_status_block_shape() {
        let p = build_prompt(&spec(SessionKind::Intake, &[], None));
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
    fn the_intake_prompt_demands_the_loop_protocol_be_embedded_not_referenced() {
        // A task file that only *points* at the protocol stops being
        // self-contained the moment the scaffold is refreshed — and the
        // archived task directory would no longer explain its own history.
        let p = build_prompt(&spec(SessionKind::Intake, &[], None));
        assert!(p.contains("loop-protocol.md"), "{p}");
        assert!(p.contains("逐字复制"), "{p}");
        assert!(p.contains("不要只写路径"), "{p}");
        assert!(p.contains("自包含"), "{p}");
    }

    #[test]
    fn the_intake_prompt_lists_attachments_and_doc_refs_when_present() {
        let mut s = spec(SessionKind::Intake, &[], None);
        let attachments = vec!["promo.csv".to_string()];
        let refs = vec!["docs/notes.md".to_string()];
        s.attachments = &attachments;
        s.doc_refs = &refs;
        let p = build_prompt(&s);
        assert!(
            p.contains("docs/checkout-flow/attachments/promo.csv"),
            "{p}"
        );
        assert!(p.contains("- docs/notes.md"), "{p}");
    }

    #[test]
    fn the_onboarding_prompt_asks_for_both_artefacts_and_forbids_touching_autome() {
        let p = build_prompt(&spec(SessionKind::Onboarding, &[], None));
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

    #[test]
    fn the_applescript_embeds_a_shell_command_without_breaking_out() {
        let cmd = shell_command(
            &["/bin/echo".to_string(), "he said \"hi\"".to_string()],
            Path::new("/tmp"),
        );
        let script = iterm_applescript(&cmd, "autome · T-1 · 实现 #1");
        // The embedded double quote must be escaped for AppleScript.
        assert!(script.contains(r#"\""#), "{script}");
        // And the title is quoted too.
        assert!(
            script.contains(r#"set name to "autome · T-1 · 实现 #1""#),
            "{script}"
        );
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
    fn tab_titles_include_the_round_only_for_role_sessions() {
        assert_eq!(
            tab_title("T-15", SessionKind::Role { role: Role::Impl }, 4),
            "autome · T-15 · 实现 #4"
        );
        assert_eq!(
            tab_title("T-15", SessionKind::Intake, 1),
            "autome · T-15 · 任务整理"
        );
    }

    #[test]
    fn system_sessions_always_run_claude_regardless_of_config() {
        let codex = role_config(Runtime::Codex, "gpt-5.4", None);
        assert_eq!(
            runtime_for(SessionKind::Intake, Some(&codex)),
            Runtime::Claude
        );
        assert_eq!(
            runtime_for(SessionKind::Onboarding, Some(&codex)),
            Runtime::Claude
        );
        assert_eq!(
            runtime_for(SessionKind::Role { role: Role::Audit }, Some(&codex)),
            Runtime::Codex
        );
    }

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
        crate::init::init(&dir).unwrap();
        let launched = launch(&LaunchSpec {
            session_id: "s-dry",
            task_id: "T-1",
            cwd: &dir,
            repo: &dir,
            runtime: Runtime::Claude,
            args: vec![],
            prompt: "the prompt".into(),
            title: "t".into(),
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
        assert_eq!(LaunchMode::default(), LaunchMode::Terminal);
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
            title: "t".into(),
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
        crate::init::init(&dir).unwrap();
        let spec = LaunchSpec {
            session_id: "s1",
            task_id: "T-1",
            cwd: &dir,
            repo: &dir,
            runtime: Runtime::Claude,
            args: vec![],
            prompt: "hello prompt".into(),
            title: "t".into(),
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
