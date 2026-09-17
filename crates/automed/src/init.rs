//! Project initialisation: the `.autome/` scaffold. Technical design §3.1,
//! §10; requirements C-01, C-03, C-11.
//!
//! Initialisation is idempotent by construction. Re-running it must not
//! clobber a rule the user wrote, a config value they changed, or a wrapper
//! script they patched — so every file is written only when absent, except the
//! two Autome owns outright (the wrapper script and the session protocol),
//! which are refreshed when their embedded version marker is older than ours.
//!
//! What lands in the repository, and why it is committed rather than kept in
//! Application Support: the user asked for cross-machine sync (requirement
//! C-11). A second checkout of the same repository gets the same Loop
//! configuration, the same rules and the same launcher without any Autome-side
//! state transfer.

use std::path::{Path, PathBuf};

use autome_domain::protocol::ProtocolFiles;

/// Bumped when `run_session.sh` or `session-protocol.md` change in a way that
/// an existing project must pick up. Files carrying an older marker are
/// rewritten; files with no marker at all are left alone, because the user has
/// clearly taken them over.
///
/// 5: the protocol audit of 2026-09-16, which read three complete runs. Its
/// seven changes are all rule text — evidence leaves the design document,
/// `retro.md` is one line a round, the implementation round self-checks before
/// claiming `待审`, the audit's discriminating checks become regression tests,
/// the budget denominator comes from the core, results only a human can see
/// stop being milestone acceptance, and the audit may again conclude
/// "验证缺口".
///
/// 6: the protocol left the binary. The rule text now lives in
/// `~/.autome/protocol/` and arrives here as an argument, with
/// `<!-- kernel-contract: … -->` markers around the regions the core parses.
///
/// 7: the deterministic guards. Evidence filenames gained a role suffix —
/// without it the audit round and the implementation round of the same `k`
/// wrote to the same path and the second one won — and the clauses the core
/// now checks say so, so that the rules a round has to remember are
/// distinguishable from the ones it does not.
pub const SCAFFOLD_VERSION: u32 = 7;

const VERSION_MARKER: &str = "autome-scaffold-version:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Created,
    /// Present and left exactly as it was.
    Kept,
    /// An Autome-owned file whose version marker was out of date.
    Refreshed,
    /// A section appended to a file that already existed (`.gitignore`,
    /// `AGENTS.md`).
    Appended,
    /// An Autome-owned file this scaffold version no longer produces.
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitStep {
    pub path: String,
    pub action: Action,
}

#[derive(Debug)]
pub struct InitError {
    pub path: String,
    pub detail: String,
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "初始化 {} 失败：{}", self.path, self.detail)
    }
}

impl std::error::Error for InitError {}

pub type Result<T> = std::result::Result<T, InitError>;

/// Everything `init` touched, in the order it happened. Returned so the UI can
/// say what was created versus kept, and so the caller knows which paths to
/// stage for the init commit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitReport {
    pub steps: Vec<InitStep>,
}

impl InitReport {
    /// Repository-relative paths to stage for the init commit (requirement
    /// C-03). `.autome/output/` is never included: it is gitignored.
    pub fn paths_to_commit(&self) -> Vec<String> {
        self.steps
            .iter()
            .filter(|s| s.action != Action::Kept)
            .map(|s| s.path.clone())
            .collect()
    }

    pub fn changed(&self) -> bool {
        self.steps.iter().any(|s| s.action != Action::Kept)
    }
}

/// Writes the scaffold into `repo`. Idempotent.
///
/// `protocol` is the version this project resolved to — the caller gets it
/// from `~/.autome/protocol/`, not from a constant here. The two rule files
/// are mirrored into `.autome/skill/` so that a human reading the repository,
/// and the onboarding session which runs outside any task worktree, can see
/// the rules in force. A *task's* authoritative copy is the one frozen into
/// its own `docs/<slug>/protocol/` at creation time.
pub fn init(repo: &Path, protocol: &ProtocolFiles) -> Result<InitReport> {
    let _ = protocol;
    let mut report = InitReport::default();

    for dir in [
        ".autome",
        ".autome/rules",
        ".autome/skill",
        ".autome/output",
        ".autome/output/sessions",
        "docs",
    ] {
        create_dir(repo, dir)?;
    }

    // Owned by Autome: refreshed when the version marker is stale.
    write_owned(
        repo,
        ".autome/skill/run_session.sh",
        &run_session_sh(),
        &mut report,
    )?;
    set_executable(&repo.join(".autome/skill/run_session.sh"))?;
    // The protocol is deliberately *not* mirrored here. A task reads the copy
    // frozen into its own `docs/<slug>/protocol/` at creation time, which is
    // what makes its rules unchangeable mid-run. A third copy in the project
    // scaffold would be a second answer to "what are the rules", and the two
    // would drift the first time someone edited `~/.autome/protocol/`.
    remove_stale(repo, ".autome/skill/session-protocol.md", &mut report);
    remove_stale(repo, ".autome/skill/loop-protocol.md", &mut report);

    // Owned by the user: created once, then never touched again.
    write_if_absent(
        repo,
        ".autome/rules/README.md",
        RULES_README_MD,
        &mut report,
    )?;

    // `.autome/config.toml` is written by config_io, which knows the sparse
    // format. Init only guarantees the directory exists; an absent file
    // already means "inherit everything".

    append_gitignore(repo, &mut report)?;
    append_agents_md(repo, &mut report)?;

    Ok(report)
}

fn create_dir(repo: &Path, rel: &str) -> Result<()> {
    let path = repo.join(rel);
    std::fs::create_dir_all(&path).map_err(|e| InitError {
        path: rel.to_string(),
        detail: e.to_string(),
    })
}

fn write_if_absent(repo: &Path, rel: &str, contents: &str, report: &mut InitReport) -> Result<()> {
    let path = repo.join(rel);
    if path.exists() {
        report.steps.push(InitStep {
            path: rel.to_string(),
            action: Action::Kept,
        });
        return Ok(());
    }
    write(&path, contents, rel)?;
    report.steps.push(InitStep {
        path: rel.to_string(),
        action: Action::Created,
    });
    Ok(())
}

/// Writes a file Autome owns, refreshing it when its version marker is behind.
/// A file the user has stripped the marker from is treated as theirs and left
/// alone — an explicit escape hatch for someone who needs a patched launcher.
fn write_owned(repo: &Path, rel: &str, contents: &str, report: &mut InitReport) -> Result<()> {
    let path = repo.join(rel);
    if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        match embedded_version(&existing) {
            Some(v) if v < SCAFFOLD_VERSION => {
                write(&path, contents, rel)?;
                report.steps.push(InitStep {
                    path: rel.to_string(),
                    action: Action::Refreshed,
                });
            }
            _ => report.steps.push(InitStep {
                path: rel.to_string(),
                action: Action::Kept,
            }),
        }
        return Ok(());
    }
    write(&path, contents, rel)?;
    report.steps.push(InitStep {
        path: rel.to_string(),
        action: Action::Created,
    });
    Ok(())
}

/// Deletes a file an earlier scaffold version wrote and this one does not.
///
/// Only when it still carries Autome's version marker: a file the user has
/// taken over — marker stripped — is theirs, including the decision to keep it
/// after Autome stopped producing it.
fn remove_stale(repo: &Path, rel: &str, report: &mut InitReport) {
    let path = repo.join(rel);
    let Ok(existing) = std::fs::read_to_string(&path) else {
        return;
    };
    if embedded_version(&existing).is_none() {
        report.steps.push(InitStep {
            path: rel.to_string(),
            action: Action::Kept,
        });
        return;
    }
    if std::fs::remove_file(&path).is_ok() {
        report.steps.push(InitStep {
            path: rel.to_string(),
            action: Action::Removed,
        });
    }
}

/// Writes a mirror of a file Autome does not own the content of.
///
/// Different from [`write_owned`] in what triggers a rewrite. An owned file —
/// the wrapper script — changes only when Autome's own version does, so the
/// version marker is the right trigger. A mirror of the protocol changes
/// whenever the user edits `~/.autome/protocol/`, and the marker inside it
/// does not move when they do; keying off the marker would leave the mirror
/// showing last release's rules indefinitely.
///
/// The escape hatch is the same: strip the version marker and the file is
/// yours, Autome stops touching it.
fn write_mirrored(repo: &Path, rel: &str, contents: &str, report: &mut InitReport) -> Result<()> {
    let path = repo.join(rel);
    if path.exists() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if embedded_version(&existing).is_none() || existing == contents {
            report.steps.push(InitStep {
                path: rel.to_string(),
                action: Action::Kept,
            });
            return Ok(());
        }
        write(&path, contents, rel)?;
        report.steps.push(InitStep {
            path: rel.to_string(),
            action: Action::Refreshed,
        });
        return Ok(());
    }
    write(&path, contents, rel)?;
    report.steps.push(InitStep {
        path: rel.to_string(),
        action: Action::Created,
    });
    Ok(())
}

fn embedded_version(text: &str) -> Option<u32> {
    text.lines()
        .find_map(|l| l.split_once(VERSION_MARKER))
        .and_then(|(_, v)| v.split_whitespace().next())
        .and_then(|v| v.parse().ok())
}

fn write(path: &Path, contents: &str, rel: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| InitError {
            path: rel.to_string(),
            detail: e.to_string(),
        })?;
    }
    std::fs::write(path, contents).map_err(|e| InitError {
        path: rel.to_string(),
        detail: e.to_string(),
    })
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)
            .map_err(|e| InitError {
                path: path.display().to_string(),
                detail: e.to_string(),
            })?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).map_err(|e| InitError {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// The two entries Autome adds. `.worktree/` holds per-task checkouts and
/// `.autome/output/` holds logs — neither belongs in history, and both would
/// otherwise show up as noise in the user's `git status` on every task.
const GITIGNORE_ENTRIES: [&str; 2] = [".worktree/", ".autome/output/"];
const GITIGNORE_HEADER: &str = "# Autome";

fn append_gitignore(repo: &Path, report: &mut InitReport) -> Result<()> {
    let path = repo.join(".gitignore");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let missing: Vec<&str> = GITIGNORE_ENTRIES
        .into_iter()
        .filter(|e| !gitignore_contains(&existing, e))
        .collect();
    if missing.is_empty() {
        report.steps.push(InitStep {
            path: ".gitignore".into(),
            action: Action::Kept,
        });
        return Ok(());
    }
    let existed = path.exists();
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.contains(GITIGNORE_HEADER) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(GITIGNORE_HEADER);
        out.push('\n');
    }
    for entry in missing {
        out.push_str(entry);
        out.push('\n');
    }
    write(&path, &out, ".gitignore")?;
    report.steps.push(InitStep {
        path: ".gitignore".into(),
        action: if existed {
            Action::Appended
        } else {
            Action::Created
        },
    });
    Ok(())
}

/// Matches an entry allowing for the usual spelling variations, so we do not
/// append `.worktree/` to a file that already says `/.worktree` or `.worktree`.
fn gitignore_contains(text: &str, entry: &str) -> bool {
    let target = entry.trim_matches('/');
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.starts_with('#'))
        .any(|l| l.trim_matches('/') == target)
}

const AGENTS_BEGIN: &str = "<!-- autome:begin -->";
const AGENTS_END: &str = "<!-- autome:end -->";

/// Adds Autome's section to `AGENTS.md`, or creates the file with it. The
/// section is delimited so a later scaffold version can replace exactly it and
/// nothing the user wrote around it.
fn append_agents_md(repo: &Path, report: &mut InitReport) -> Result<()> {
    let path = repo.join("AGENTS.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let section = agents_section();

    if let (Some(start), Some(end)) = (existing.find(AGENTS_BEGIN), existing.find(AGENTS_END)) {
        let current = &existing[start..end + AGENTS_END.len()];
        if current == section {
            report.steps.push(InitStep {
                path: "AGENTS.md".into(),
                action: Action::Kept,
            });
            return Ok(());
        }
        let mut out = String::with_capacity(existing.len());
        out.push_str(&existing[..start]);
        out.push_str(&section);
        out.push_str(&existing[end + AGENTS_END.len()..]);
        write(&path, &out, "AGENTS.md")?;
        report.steps.push(InitStep {
            path: "AGENTS.md".into(),
            action: Action::Refreshed,
        });
        return Ok(());
    }

    let existed = path.exists();
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    } else {
        out.push_str("# AGENTS.md\n\n本仓库对 AI Agent 的项目级规范。\n\n");
    }
    out.push_str(&section);
    out.push('\n');
    write(&path, &out, "AGENTS.md")?;
    report.steps.push(InitStep {
        path: "AGENTS.md".into(),
        action: if existed {
            Action::Appended
        } else {
            Action::Created
        },
    });
    Ok(())
}

fn agents_section() -> String {
    format!(
        r#"{AGENTS_BEGIN}
## Autome

本仓库由 Autome 驱动 Loop 开发。运行中的会话必须遵守：

- **不要修改 `.autome/` 下的任何内容。** 配置、规则与启动脚本由用户在 Autome 界面或编辑器中维护；任务内的改动会被下一次初始化覆盖，也不会被 Autome 读取。
- 本轮工作只在当前 worktree 内进行，不要切换分支、不要操作 `.worktree/` 下的其它目录、不要推送远端。
- 完成本轮后结束会话即可，**不要自行启动下一个会话**——下一个节点由 Autome 调度。
- 项目级规范见 `.autome/rules/` 下的文件，与本文件同等效力。
- 超过 5 行的命令输出、临时验证产物写入 `.autome/output/`，不要进入任务文档目录。
{AGENTS_END}"#
    )
}

/// The wrapper script. This is the exit-marker protocol (design §2, §7.3).
///
/// Three things make it load-bearing:
///
/// 1. It records the pid *before* exec'ing, so Stop can signal the process
///    group and the liveness backstop has something to check.
/// 2. It writes the exit marker in a `trap`, so an interrupted or killed CLI
///    still produces one — a session that vanishes without a marker is the
///    expensive case the core has to fall back on heuristics for.
/// 3. The marker is written last and ends with `marker_end`, so a partially
///    written file is detectably incomplete rather than parsing as success.
fn run_session_sh() -> String {
    format!(
        r#"#!/bin/sh
# {VERSION_MARKER} {SCAFFOLD_VERSION}
#
# Autome 会话包装脚本。由 Autome 在可见终端中调用，不要手工修改
# （下次初始化会按版本号刷新；删掉上面的版本标记行即可接管本文件）。
#
# 用法：
#   run_session.sh <session-id> <output-dir> <runtime> <renderer> <binary> <prompt-file> [extra-args...]
#
# <renderer> 是 automed 自己的可执行文件路径，用来把 CLI 的 JSONL 输出渲染成
# 人能读的日志；传 `-` 或传一个不可执行的路径即跳过渲染。
#
# 职责：写 pid → 执行 CLI 并把输出 tee 到日志 → 无论如何都写退出标记。

set -u

if [ $# -lt 6 ]; then
  echo "run_session.sh: 参数不足" >&2
  exit 64
fi

session_id="$1"; shift
out_dir="$1"; shift
runtime="$1"; shift
renderer="$1"; shift
binary="$1"; shift
prompt_file="$1"; shift

log="$out_dir/$session_id.log"
pid_file="$out_dir/$session_id.pid"
exit_file="$out_dir/$session_id.exit"

# If the output directory cannot be created there is nowhere to write the log,
# the pid or the marker, and every later line would fail in turn — five
# cascading errors that say nothing about the cause. Stop here instead.
if ! mkdir -p "$out_dir" 2>/dev/null || [ ! -d "$out_dir" ]; then
  echo "run_session.sh: 会话目录不存在且无法创建：$out_dir" >&2
  echo "run_session.sh: 项目可能已被移动或删除。" >&2
  exit 65
fi
if ! : > "$log" 2>/dev/null; then
  echo "run_session.sh: 无法写入日志 $log" >&2
  echo "run_session.sh: 项目可能已被移动或删除。" >&2
  exit 65
fi

# 退出标记：无论正常结束、被 kill 还是脚本出错都要写出。
# 先写到临时文件再 mv，保证读取方不会看到写了一半的标记。
write_marker() {{
  code=$1
  tmp="$exit_file.tmp"
  {{
    echo "exit_code=$code"
    echo "ended_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "marker_end"
  }} > "$tmp" 2>/dev/null
  mv -f "$tmp" "$exit_file" 2>/dev/null
}}

on_signal() {{
  write_marker 130
  exit 130
}}
trap on_signal INT TERM HUP

echo $$ > "$pid_file"

printf '=== autome session %s (%s) started %s ===\n' \
  "$session_id" "$runtime" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$log"

if [ ! -f "$prompt_file" ]; then
  echo "run_session.sh: 找不到 prompt 文件 $prompt_file" >> "$log"
  write_marker 66
  exit 66
fi

# CLI 的 stdout/stderr 同时进日志和终端，用户能实时看到，Autome 事后能读到。
#
# 退出码必须取 CLI 的，不是 tee 的。POSIX sh 没有 bash 的管道状态数组，
# 所以把 CLI 的退出码写进一个临时文件再读回来——这是可移植的写法。
# 这里错了的后果是：崩溃的会话会被当成正常结束，内核会去解析一份不存在或半截的
# 设计文档，然后把协议失败归错到文档头上。
code_file="$out_dir/$session_id.code"
rm -f "$code_file"

# 两个 CLI 都用 JSONL 跑：Claude 是 --output-format stream-json，Codex 是 --json。
#
# Claude 的 text 模式只打印收尾那段总结——会话不开窗口，日志是唯一能看到这一轮
# 干了什么的地方，而 text 模式下它只有几 KB。Codex 的默认输出本来就是给人看的，
# 但它不带用量：费用、turn 数、token 全都拿不到，内核记不下任何东西。
#
# JSONL 是完整的，但一行一个 JSON，人读不了，所以经 automed 自己渲染一遍。
# 原始流另存 .jsonl——内核事后从它读用量，渲染只影响展示、不丢东西。
if [ -n "$renderer" ] && [ "$renderer" != "-" ] && [ -x "$renderer" ]; then
  {{ "$binary" "$@" < "$prompt_file" 2>&1; echo $? > "$code_file"; }} \
    | tee -a "$out_dir/$session_id.jsonl" \
    | "$renderer" render-stream "$runtime" \
    | tee -a "$log"
else
  {{ "$binary" "$@" < "$prompt_file" 2>&1; echo $? > "$code_file"; }} | tee -a "$log"
fi
code=$(cat "$code_file" 2>/dev/null || echo 70)
rm -f "$code_file"

printf '=== autome session %s finished, exit %s ===\n' "$session_id" "$code" >> "$log"
write_marker "$code"
rm -f "$pid_file"
exit "$code"
"#
    )
}

// The Loop protocol and the session protocol used to be two constants right
// here. They now live in `~/.autome/protocol/`, seeded from
// `crate::protocol::seed()`, and arrive as an argument to `init`. See that
// module for why: a rule set nobody can change is a rule set that cannot
// learn, and the retro round had been writing improvement suggestions into a
// dead end for months.

const RULES_README_MD: &str = r#"# 项目规则

本目录下的 Markdown 文件是本项目对 Agent 的规范，与仓库根的 `AGENTS.md` 同等效力。
每个节点的会话启动时都会被要求遵守它们。

建议按主题分文件，例如：

- `architecture.md` — 分层、依赖方向、不允许的耦合
- `testing.md` — 测试命令、覆盖要求、什么算通过
- `conventions.md` — 命名、提交信息、目录约定

规则应当是可检验的约束，而不是风格偏好。写"所有对外接口必须有契约测试"，
而不是"代码要写得优雅"。

本文件本身可以删除。
"#;

/// Whether a repository already has the scaffold, used to decide between
/// "add project" and "reopen project".
pub fn is_initialised(repo: &Path) -> bool {
    repo.join(".autome/skill/run_session.sh").exists()
}

/// Where the wrapper script lives, absolute.
pub fn wrapper_script(repo: &Path) -> PathBuf {
    repo.join(".autome/skill/run_session.sh")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Tests call `init` through this so the protocol argument does not have
    /// to be spelled out thirty times. Production always passes the version
    /// the *project* resolved to, which may not be the seed.
    fn init(repo: &Path) -> Result<InitReport> {
        super::init(repo, &crate::protocol::seed())
    }


    static COUNTER: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("automed-init-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
        fn read(&self, rel: &str) -> String {
            std::fs::read_to_string(self.0.join(rel)).unwrap()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn action_for(report: &InitReport, path: &str) -> Option<Action> {
        report
            .steps
            .iter()
            .find(|s| s.path == path)
            .map(|s| s.action.clone())
    }

    #[test]
    fn init_creates_the_whole_scaffold() {
        let dir = TempDir::new("create");
        let report = init(dir.path()).unwrap();
        for rel in [
            ".autome/skill/run_session.sh",
            ".autome/rules/README.md",
            ".gitignore",
            "AGENTS.md",
        ] {
            assert!(dir.path().join(rel).exists(), "{rel} missing");
        }
        assert!(dir.path().join(".autome/output/sessions").is_dir());
        assert!(dir.path().join("docs").is_dir());
        assert!(report.changed());
        assert!(is_initialised(dir.path()));
    }

    #[test]
    fn init_is_idempotent_and_reports_everything_as_kept() {
        let dir = TempDir::new("idempotent");
        init(dir.path()).unwrap();
        let before = dir.read("AGENTS.md");
        let second = init(dir.path()).unwrap();
        assert!(
            second.steps.iter().all(|s| s.action == Action::Kept),
            "{:#?}",
            second.steps
        );
        assert!(!second.changed());
        assert_eq!(dir.read("AGENTS.md"), before);
    }

    #[test]
    fn a_user_edited_rule_file_is_never_overwritten() {
        let dir = TempDir::new("user-rules");
        init(dir.path()).unwrap();
        std::fs::write(dir.path().join(".autome/rules/README.md"), "我的规则\n").unwrap();
        init(dir.path()).unwrap();
        assert_eq!(dir.read(".autome/rules/README.md"), "我的规则\n");
    }

    #[test]
    fn an_owned_file_with_a_stale_version_marker_is_refreshed() {
        let dir = TempDir::new("refresh");
        init(dir.path()).unwrap();
        std::fs::write(
            dir.path().join(".autome/skill/run_session.sh"),
            "#!/bin/sh\n# autome-scaffold-version: 0\necho old\n",
        )
        .unwrap();
        let report = init(dir.path()).unwrap();
        assert_eq!(
            action_for(&report, ".autome/skill/run_session.sh"),
            Some(Action::Refreshed)
        );
        assert!(
            dir.read(".autome/skill/run_session.sh")
                .contains("write_marker")
        );
    }

    #[test]
    fn an_owned_file_the_user_has_taken_over_is_left_alone() {
        let dir = TempDir::new("adopted");
        init(dir.path()).unwrap();
        // No version marker: the user has claimed the file.
        std::fs::write(
            dir.path().join(".autome/skill/run_session.sh"),
            "#!/bin/sh\necho my own launcher\n",
        )
        .unwrap();
        let report = init(dir.path()).unwrap();
        assert_eq!(
            action_for(&report, ".autome/skill/run_session.sh"),
            Some(Action::Kept)
        );
        assert_eq!(
            dir.read(".autome/skill/run_session.sh"),
            "#!/bin/sh\necho my own launcher\n"
        );
    }

    #[test]
    fn embedded_version_reads_the_marker_and_ignores_everything_else() {
        assert_eq!(embedded_version("# autome-scaffold-version: 7\n"), Some(7));
        // Deliberately not the current SCAFFOLD_VERSION: this is about the
        // parser, and a literal that tracks the constant invites a blanket
        // find-and-replace to "fix" it into meaninglessness.
        assert_eq!(
            embedded_version("<!-- autome-scaffold-version: 41 -->"),
            Some(41)
        );
        assert_eq!(embedded_version("no marker here"), None);
        assert_eq!(embedded_version("# autome-scaffold-version: abc"), None);
    }

    #[test]
    fn gitignore_gets_both_entries_under_a_header() {
        let dir = TempDir::new("gitignore-new");
        init(dir.path()).unwrap();
        let text = dir.read(".gitignore");
        assert!(text.contains(".worktree/"));
        assert!(text.contains(".autome/output/"));
        assert!(text.contains("# Autome"));
    }

    #[test]
    fn an_existing_gitignore_is_appended_to_not_replaced() {
        let dir = TempDir::new("gitignore-existing");
        std::fs::write(dir.path().join(".gitignore"), "node_modules\ndist\n").unwrap();
        let report = init(dir.path()).unwrap();
        assert_eq!(action_for(&report, ".gitignore"), Some(Action::Appended));
        let text = dir.read(".gitignore");
        assert!(text.starts_with("node_modules\ndist\n"), "{text}");
        assert!(text.contains(".worktree/"));
    }

    #[test]
    fn gitignore_entries_are_not_duplicated_across_spelling_variants() {
        let dir = TempDir::new("gitignore-variants");
        std::fs::write(
            dir.path().join(".gitignore"),
            "/.worktree\n.autome/output\n",
        )
        .unwrap();
        let report = init(dir.path()).unwrap();
        assert_eq!(action_for(&report, ".gitignore"), Some(Action::Kept));
        let text = dir.read(".gitignore");
        assert_eq!(text.matches("worktree").count(), 1, "{text}");
    }

    #[test]
    fn a_commented_out_entry_does_not_count_as_present() {
        let dir = TempDir::new("gitignore-comment");
        std::fs::write(dir.path().join(".gitignore"), "# .worktree/\n").unwrap();
        init(dir.path()).unwrap();
        let text = dir.read(".gitignore");
        assert!(
            text.lines().any(|l| l.trim() == ".worktree/"),
            "an uncommented entry should have been added:\n{text}"
        );
    }

    #[test]
    fn a_missing_gitignore_without_changes_is_still_created() {
        let dir = TempDir::new("gitignore-created");
        let report = init(dir.path()).unwrap();
        assert_eq!(action_for(&report, ".gitignore"), Some(Action::Created));
    }

    #[test]
    fn agents_md_is_created_with_the_autome_section() {
        let dir = TempDir::new("agents-new");
        init(dir.path()).unwrap();
        let text = dir.read("AGENTS.md");
        assert!(text.contains(AGENTS_BEGIN));
        assert!(text.contains(AGENTS_END));
        assert!(text.contains("不要自行启动下一个会话"));
        assert!(text.contains("不要修改 `.autome/`"));
    }

    #[test]
    fn an_existing_agents_md_keeps_its_content_and_gains_a_delimited_section() {
        let dir = TempDir::new("agents-existing");
        std::fs::write(dir.path().join("AGENTS.md"), "# 我的项目\n\n请遵守 X。\n").unwrap();
        let report = init(dir.path()).unwrap();
        assert_eq!(action_for(&report, "AGENTS.md"), Some(Action::Appended));
        let text = dir.read("AGENTS.md");
        assert!(text.starts_with("# 我的项目"), "{text}");
        assert!(text.contains("请遵守 X。"));
        assert!(text.contains(AGENTS_BEGIN));
    }

    #[test]
    fn the_autome_section_is_replaced_in_place_leaving_user_text_around_it() {
        let dir = TempDir::new("agents-replace");
        init(dir.path()).unwrap();
        let mut text = dir.read("AGENTS.md");
        text.push_str("\n## 我自己加的一节\n\n内容。\n");
        // Corrupt the section so init has to rewrite it.
        let text = text.replace("不要自行启动下一个会话", "过时的说法");
        std::fs::write(dir.path().join("AGENTS.md"), &text).unwrap();

        let report = init(dir.path()).unwrap();
        assert_eq!(action_for(&report, "AGENTS.md"), Some(Action::Refreshed));
        let after = dir.read("AGENTS.md");
        assert!(after.contains("我自己加的一节"), "user text survived");
        assert!(after.contains("不要自行启动下一个会话"));
        assert!(!after.contains("过时的说法"));
    }

    #[test]
    fn the_wrapper_script_is_executable() {
        let dir = TempDir::new("exec");
        init(dir.path()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(wrapper_script(dir.path()))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "mode {mode:o}");
        }
    }

    #[test]
    fn the_wrapper_script_writes_a_terminated_marker_on_every_path() {
        let script = run_session_sh();
        assert!(script.contains("trap on_signal INT TERM HUP"));
        assert!(script.contains("marker_end"));
        // The marker is moved into place, never written in-place, so a reader
        // cannot observe a half-written file.
        assert!(script.contains("mv -f \"$tmp\" \"$exit_file\""));
        // pid is recorded before the CLI runs.
        let pid_at = script.find("echo $$ > \"$pid_file\"").unwrap();
        let exec_at = script.find("\"$binary\" \"$@\"").unwrap();
        assert!(pid_at < exec_at, "pid must be written before exec");
    }

    #[test]
    fn the_wrapper_script_refuses_a_missing_prompt_file_with_its_own_code() {
        assert!(run_session_sh().contains("write_marker 66"));
    }

    #[test]
    fn the_wrapper_refuses_to_run_when_its_session_directory_is_gone() {
        // Observed for real: a terminal window opened by a test ran after the
        // test had deleted its sandbox, and produced five cascading "No such
        // file or directory" errors — one per later line — none of which named
        // the cause.
        let dir = std::env::temp_dir().join(format!("automed-nodir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        init(&dir).unwrap();
        let wrapper = wrapper_script(&dir);
        let out = std::process::Command::new("sh")
            .arg(&wrapper)
            .arg("s1")
            // A path under a directory that does not exist and cannot be made.
            .arg("/dev/null/nowhere")
            .arg("claude")
            // The renderer slot. `-` means "pipe the stream through
            // unchanged", which is what this test wants: its subject is the
            // wrapper's refusal, not the rendering.
            .arg("-")
            .arg("/bin/echo")
            .arg("/tmp/whatever")
            .output()
            .expect("wrapper runs");
        assert_eq!(out.status.code(), Some(65));
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("会话目录"), "{stderr}");
        assert_eq!(
            stderr.lines().count(),
            2,
            "one cause and one hint, not a cascade:\n{stderr}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_wrapper_script_uses_no_bash_only_constructs() {
        // The shebang is /bin/sh, so anything bash-only is a latent bug on a
        // machine where /bin/sh is dash. PIPESTATUS in particular silently
        // yields tee's exit code instead of the CLI's, which would make every
        // crashed session look successful.
        let script = run_session_sh();
        for bashism in ["PIPESTATUS", "[[", "function ", "local ", "$'"] {
            assert!(
                !script.contains(bashism),
                "wrapper uses the bash-only construct {bashism:?}"
            );
        }
        assert!(script.starts_with("#!/bin/sh"));
    }

    #[test]
    fn paths_to_commit_excludes_kept_files_and_never_includes_output() {
        let dir = TempDir::new("commit-paths");
        let report = init(dir.path()).unwrap();
        let paths = report.paths_to_commit();
        assert!(paths.contains(&".autome/skill/run_session.sh".to_string()));
        assert!(paths.contains(&"AGENTS.md".to_string()));
        assert!(!paths.iter().any(|p| p.contains("output")), "{paths:?}");

        let second = init(dir.path()).unwrap();
        assert!(second.paths_to_commit().is_empty());
    }

    #[test]
    fn is_initialised_is_false_before_and_true_after() {
        let dir = TempDir::new("is-init");
        assert!(!is_initialised(dir.path()));
        init(dir.path()).unwrap();
        assert!(is_initialised(dir.path()));
    }

    #[test]
    fn init_into_a_directory_that_does_not_exist_yet_creates_it() {
        let dir = TempDir::new("nested");
        let nested = dir.path().join("a/b/c");
        init(&nested).unwrap();
        assert!(nested.join(".autome/skill/run_session.sh").exists());
    }
}
