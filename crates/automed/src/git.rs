//! Git operations. Technical design §6.
//!
//! Every command runs through one place, with a fixed binary and a sanitised
//! environment: no credential helper, no user or system config, no hooks, no
//! remote access. The design's operations table is the whole surface — this
//! module does not offer a general "run git" escape hatch, because the set of
//! things Autome is allowed to do to a user's repository is exactly that table
//! and nothing else.
//!
//! What we never do, and why:
//!
//! - Never touch a remote. 2.0 does not push, fetch or open pull requests
//!   (requirement §3.2). Disabling the credential helper makes an accidental
//!   network operation fail closed rather than prompt.
//! - Never run repository hooks. A task branch is written by an agent; letting
//!   its hooks run during our merge would hand it execution in the user's main
//!   worktree.
//! - Never modify the user's checkout except through the two operations they
//!   asked for: the init commit and the merge.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How long any single Git invocation may take. A rebase on a large repository
/// is the slow case; ten minutes is far beyond it and still bounded.
const GIT_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitError {
    /// The argv we ran, for the diagnostic surface. Never contains a secret:
    /// the environment is sanitised and no remote is contacted.
    pub argv: Vec<String>,
    pub code: Option<i32>,
    pub stderr: String,
    pub detail: String,
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "git {}: {}", self.argv.join(" "), self.detail)
    }
}

impl std::error::Error for GitError {}

pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.code == 0
    }
    /// stdout with the trailing newline removed — the shape almost every
    /// caller wants from a single-value query.
    pub fn line(&self) -> &str {
        self.stdout.trim_end_matches(['\n', '\r'])
    }
}

/// Environment applied to every invocation (design §6: "固定二进制、净化环境").
fn sanitised_env() -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    // Deterministic, English, machine-readable output regardless of the user's
    // locale — several parsers below match on porcelain output.
    env.insert("LC_ALL".into(), "C".into());
    env.insert("LANG".into(), "C".into());
    // No user or system config: a `merge.tool`, `core.autocrlf` or alias in
    // the user's ~/.gitconfig must not change what Autome does.
    env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
    env.insert("GIT_CONFIG_GLOBAL".into(), "/dev/null".into());
    // Fail closed instead of prompting if anything ever reaches for a remote.
    env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    env.insert("GIT_ASKPASS".into(), "/usr/bin/false".into());
    env.insert("SSH_ASKPASS".into(), "/usr/bin/false".into());
    // No interactive editor can ever open: a merge or rebase that wants one
    // must fail rather than hang forever on a terminal nobody is watching.
    env.insert("GIT_EDITOR".into(), "/usr/bin/true".into());
    env.insert("GIT_SEQUENCE_EDITOR".into(), "/usr/bin/true".into());
    env
}

/// The identity Autome commits under. Only ever used for the init commit and
/// the merge commit; agent commits carry the user's own identity because they
/// are made by the CLI inside the worktree, not by us.
const AUTOME_NAME: &str = "Autome";
const AUTOME_EMAIL: &str = "autome@localhost";

/// The Git binary. Overridable for tests and for a user whose git is not on
/// the default path; resolved once per call rather than cached, so a mid-session
/// install of Xcode command line tools takes effect without a restart.
fn git_binary() -> String {
    std::env::var("AUTOMED_GIT_BINARY").unwrap_or_else(|_| "git".to_string())
}

/// Runs git in `cwd`, returning output regardless of exit status. Callers that
/// require success use [`run_ok`].
pub fn run(cwd: &Path, args: &[&str]) -> Result<Output> {
    run_with_extra_env(cwd, args, &[])
}

fn run_with_extra_env(cwd: &Path, args: &[&str], extra: &[(&str, &str)]) -> Result<Output> {
    run_with_binary(&git_binary(), cwd, args, extra)
}

/// The one place a Git process is actually spawned. Takes the binary
/// explicitly so the "cannot start git" path is testable without mutating a
/// process-global environment variable — which would otherwise leak into every
/// other test running in parallel and make them fail at random.
fn run_with_binary(
    binary: &str,
    cwd: &Path,
    args: &[&str],
    extra: &[(&str, &str)],
) -> Result<Output> {
    let argv: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let mut cmd = Command::new(binary);
    cmd.current_dir(cwd)
        .args(args)
        .env_clear()
        // PATH is still needed: git shells out to its own subcommands.
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in sanitised_env() {
        cmd.env(k, v);
    }
    for (k, v) in extra {
        cmd.env(k, v);
    }

    let child = cmd.spawn().map_err(|e| GitError {
        argv: argv.clone(),
        code: None,
        stderr: String::new(),
        detail: format!("无法启动 git：{e}"),
    })?;

    let out = wait_with_timeout(child, GIT_TIMEOUT_SECS).map_err(|detail| GitError {
        argv: argv.clone(),
        code: None,
        stderr: String::new(),
        detail,
    })?;

    Ok(Output {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().unwrap_or(-1),
    })
}

/// Waits for a child, killing it if it outlives `secs`. Written by hand rather
/// than pulled from a crate because it is eight lines and the alternative is a
/// dependency in the trusted path that touches the user's repository.
fn wait_with_timeout(
    mut child: std::process::Child,
    secs: u64,
) -> std::result::Result<std::process::Output, String> {
    use std::io::Read;
    use std::time::{Duration, Instant};

    // Drain the pipes on threads: a child that fills its stderr buffer would
    // otherwise block forever while we poll, and look like a timeout.
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = stdout_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(p) = stderr_pipe.as_mut() {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("git 超过 {secs}s 未结束，已终止"));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(format!("等待 git 失败：{e}")),
        }
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();
    Ok(std::process::Output {
        status,
        stdout,
        stderr,
    })
}

/// Runs git with `input` on its stdin, and fails on a non-zero exit.
///
/// For the plumbing commands that take a list of things to do rather than one
/// argument — `cat-file --batch` above all, which turns "read ninety-two
/// blobs" from ninety-two processes into one.
///
/// stdin is written on its own thread. `wait_with_timeout` already drains
/// stdout and stderr on theirs, and writing inline instead would deadlock the
/// moment git's output filled the pipe before it had finished reading ours.
pub fn run_ok_with_stdin(cwd: &Path, args: &[&str], input: &str) -> Result<Output> {
    use std::io::Write;
    let argv: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let mut cmd = Command::new(git_binary());
    cmd.current_dir(cwd)
        .args(args)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in sanitised_env() {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| GitError {
        argv: argv.clone(),
        code: None,
        stderr: String::new(),
        detail: format!("无法启动 git：{e}"),
    })?;
    if let Some(mut pipe) = child.stdin.take() {
        let owned = input.to_string();
        std::thread::spawn(move || {
            let _ = pipe.write_all(owned.as_bytes());
            // Dropping closes the pipe, which is what tells `--batch` to stop.
        });
    }
    let out = wait_with_timeout(child, GIT_TIMEOUT_SECS).map_err(|detail| GitError {
        argv: argv.clone(),
        code: None,
        stderr: String::new(),
        detail,
    })?;
    let result = Output {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().unwrap_or(-1),
    };
    if result.ok() {
        Ok(result)
    } else {
        Err(GitError {
            argv,
            code: Some(result.code),
            stderr: result.stderr,
            detail: String::new(),
        })
    }
}

/// Runs git and fails on a non-zero exit.
pub fn run_ok(cwd: &Path, args: &[&str]) -> Result<Output> {
    let out = run(cwd, args)?;
    if out.ok() {
        Ok(out)
    } else {
        Err(GitError {
            argv: args.iter().map(|s| (*s).to_string()).collect(),
            code: Some(out.code),
            stderr: out.stderr.clone(),
            detail: first_meaningful_line(&out.stderr)
                .unwrap_or_else(|| format!("退出码 {}", out.code)),
        })
    }
}

/// Picks the line a user should see out of git's stderr: the first line that
/// is not a progress counter or a blank.
pub fn first_meaningful_line(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("remote:") && !l.contains('\r'))
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// Repository shape
// ---------------------------------------------------------------------------

/// Whether `path` is inside a Git working tree, and if so whether it is the
/// root of one.
pub fn is_repo_root(path: &Path) -> bool {
    match run(path, &["rev-parse", "--show-toplevel"]) {
        Ok(out) if out.ok() => {
            let top = PathBuf::from(out.line());
            canonical(&top) == canonical(path)
        }
        _ => false,
    }
}

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// `git init -b <branch>`. Used when the chosen directory is not a repository
/// (requirement P-01).
pub fn init(path: &Path, default_branch: &str) -> Result<()> {
    run_ok(path, &["init", "-b", default_branch])?;
    Ok(())
}

/// Resolves the branch task branches are cut from (design §6, requirement
/// P-03), in the order the table specifies:
///
/// 1. `origin/HEAD`, when the repository has a remote that declares one;
/// 2. the configured `init.defaultBranch`, when it names an existing branch;
/// 3. whatever HEAD currently points at.
///
/// Returns `None` for a repository with no branches at all — a freshly
/// `git init`ed directory with no commit yet has an unborn HEAD, which is
/// still usable and handled by the caller.
pub fn default_branch(repo: &Path) -> Option<String> {
    if let Ok(out) = run(
        repo,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) && out.ok()
    {
        // "origin/main" -> "main"
        if let Some(name) = out.line().strip_prefix("origin/")
            && branch_exists(repo, name)
        {
            return Some(name.to_string());
        }
    }
    if let Ok(out) = run(repo, &["config", "--get", "init.defaultBranch"])
        && out.ok()
    {
        let name = out.line();
        if !name.is_empty() && branch_exists(repo, name) {
            return Some(name.to_string());
        }
    }
    // Current HEAD, including an unborn one (`--short HEAD` still resolves the
    // symbolic name before the first commit).
    if let Ok(out) = run(repo, &["symbolic-ref", "--short", "HEAD"])
        && out.ok()
        && !out.line().is_empty()
    {
        return Some(out.line().to_string());
    }
    None
}

pub fn branch_exists(repo: &Path, branch: &str) -> bool {
    run(
        repo,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .map(|o| o.ok())
    .unwrap_or(false)
}

/// Whether the repository has at least one commit.
pub fn has_commits(repo: &Path) -> bool {
    run(repo, &["rev-parse", "--verify", "HEAD"])
        .map(|o| o.ok())
        .unwrap_or(false)
}

/// `git status --porcelain` is empty. The precondition for merging
/// (requirement T-07): Autome must never merge into a dirty checkout, because
/// the merge would mix the user's uncommitted work into the result.
pub fn is_clean(repo: &Path) -> Result<bool> {
    Ok(dirty_paths(repo)?.is_empty())
}

/// The list of dirty paths, for the message shown when a merge is refused.
/// `--untracked-files=all` matters: by default Git collapses a wholly
/// untracked directory to the directory name, so a new `文档/说明.md` reports
/// as `文档/` and would never compare equal to the path the merge is about to
/// write. Listing them individually is what makes the comparison in
/// `conflicting_dirty_paths` sound.
pub fn dirty_paths(repo: &Path) -> Result<Vec<String>> {
    let out = run_ok(repo, &["status", "--porcelain", "--untracked-files=all"])?;
    Ok(out
        .stdout
        .lines()
        .filter_map(|l| l.get(3..).map(str::to_string))
        .collect())
}

pub fn head_sha(repo: &Path) -> Result<String> {
    Ok(run_ok(repo, &["rev-parse", "HEAD"])?.line().to_string())
}

pub fn short_sha(repo: &Path, rev: &str) -> Result<String> {
    Ok(run_ok(repo, &["rev-parse", "--short", rev])?
        .line()
        .to_string())
}

// ---------------------------------------------------------------------------
// Commits
// ---------------------------------------------------------------------------

/// Stages the given paths and commits them under Autome's identity.
///
/// Used for exactly two things: the init commit (requirement C-03) and the
/// task's input commit on its own branch. Both are narrow by construction —
/// the caller names the paths, so a stray file in the user's tree is never
/// swept in.
pub fn commit_paths(repo: &Path, paths: &[&str], message: &str) -> Result<Option<String>> {
    if paths.is_empty() {
        return Ok(None);
    }
    let mut args = vec!["add", "--"];
    args.extend_from_slice(paths);
    run_ok(repo, &args)?;

    // Nothing staged means nothing to do: re-running init on an unchanged
    // repository must not produce an empty commit.
    let staged = run(repo, &["diff", "--cached", "--quiet"])?;
    if staged.ok() {
        return Ok(None);
    }

    let out = run_with_extra_env(
        repo,
        &["commit", "--no-verify", "--no-gpg-sign", "-m", message],
        &[
            ("GIT_AUTHOR_NAME", AUTOME_NAME),
            ("GIT_AUTHOR_EMAIL", AUTOME_EMAIL),
            ("GIT_COMMITTER_NAME", AUTOME_NAME),
            ("GIT_COMMITTER_EMAIL", AUTOME_EMAIL),
        ],
    )?;
    if !out.ok() {
        return Err(GitError {
            argv: vec!["commit".into()],
            code: Some(out.code),
            stderr: out.stderr.clone(),
            detail: first_meaningful_line(&out.stderr).unwrap_or_else(|| "提交失败".to_string()),
        });
    }
    Ok(Some(head_sha(repo)?))
}

// ---------------------------------------------------------------------------
// Worktrees
// ---------------------------------------------------------------------------

/// `git worktree add .worktree/<slug> -b autome/<slug> <base>`
/// (requirement P-05).
pub fn worktree_add(repo: &Path, rel_path: &str, branch: &str, base: &str) -> Result<()> {
    run_ok(repo, &["worktree", "add", rel_path, "-b", branch, base])?;
    Ok(())
}

/// Removes a worktree. `force` discards uncommitted changes inside it, which
/// is correct on cancel (the user asked for the work to go away) and wrong on
/// cleanup after a merge (where a dirty worktree means something unexpected
/// happened and should be surfaced).
pub fn worktree_remove(repo: &Path, rel_path: &str, force: bool) -> Result<()> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(rel_path);
    run_ok(repo, &args)?;
    Ok(())
}

/// Clears registrations whose directories are already gone, so a worktree
/// deleted outside Autome does not block re-creating the same path.
pub fn worktree_prune(repo: &Path) -> Result<()> {
    run_ok(repo, &["worktree", "prune"])?;
    Ok(())
}

/// The worktree paths Git currently knows about, excluding the main one.
pub fn worktree_list(repo: &Path) -> Result<Vec<String>> {
    let out = run_ok(repo, &["worktree", "list", "--porcelain"])?;
    let main = canonical(repo);
    Ok(out
        .stdout
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .filter(|p| canonical(Path::new(p)) != main)
        .map(str::to_string)
        .collect())
}

pub fn branch_delete(repo: &Path, branch: &str, force: bool) -> Result<()> {
    run_ok(repo, &["branch", if force { "-D" } else { "-d" }, branch])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Rebase and merge
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseOutcome {
    /// Already on top of the base, or replayed cleanly.
    Clean,
    /// Conflicted; the rebase has been aborted, so the worktree is back to its
    /// pre-rebase state and the implement role can be handed the file list
    /// (design §6, requirement T-06).
    Conflict { files: Vec<String>, detail: String },
}

/// Rebases the task branch onto the latest default branch, inside the task's
/// own worktree. Aborts on conflict rather than leaving the worktree in a
/// detached mid-rebase state that the implement session would then have to
/// understand.
pub fn rebase(worktree: &Path, onto: &str) -> Result<RebaseOutcome> {
    let out = run(worktree, &["rebase", onto])?;
    if out.ok() {
        return Ok(RebaseOutcome::Clean);
    }
    let files = conflicted_files(worktree).unwrap_or_default();
    let detail = first_meaningful_line(&out.stderr)
        .or_else(|| first_meaningful_line(&out.stdout))
        .unwrap_or_else(|| "rebase 失败".to_string());
    // Abort regardless of why it failed: a half-finished rebase is not a state
    // any later step in the design knows how to handle.
    let _ = run(worktree, &["rebase", "--abort"]);
    Ok(RebaseOutcome::Conflict { files, detail })
}

/// Paths with conflict markers, via `--diff-filter=U`.
pub fn conflicted_files(worktree: &Path) -> Result<Vec<String>> {
    let out = run(worktree, &["diff", "--name-only", "--diff-filter=U"])?;
    Ok(out
        .stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect())
}

/// Whether `ancestor` is already contained in `descendant` — the "branch is on
/// top of the latest default branch" precondition for merging.
pub fn is_ancestor(repo: &Path, ancestor: &str, descendant: &str) -> Result<bool> {
    let out = run(repo, &["merge-base", "--is-ancestor", ancestor, descendant])?;
    match out.code {
        0 => Ok(true),
        1 => Ok(false),
        _ => Err(GitError {
            argv: vec!["merge-base".into()],
            code: Some(out.code),
            stderr: out.stderr.clone(),
            detail: first_meaningful_line(&out.stderr).unwrap_or_else(|| "merge-base 失败".into()),
        }),
    }
}

/// Uncommitted paths in the main worktree that this merge would also change.
///
/// The precondition is *not* "the worktree is clean". It is "the merge will
/// not disturb work in progress", and those are different: Git itself only
/// refuses a merge that needs to overwrite a locally-modified file.
///
/// Insisting on a wholly clean tree looked equivalent and was not. Changing a
/// Loop setting in the UI writes `.autome/config.toml` and deliberately does
/// not commit it — the user owns that commit (requirement C-11). Under the
/// stricter rule, editing one model in the routing graph would have blocked
/// *every* subsequent merge, in every task, with a message about a file that
/// has nothing to do with the work being merged. A real run hit exactly that.
pub fn conflicting_dirty_paths(repo: &Path, base: &str, branch: &str) -> Result<Vec<String>> {
    let dirty = dirty_paths(repo)?;
    if dirty.is_empty() {
        return Ok(Vec::new());
    }
    let range = format!("{base}...{branch}");
    let changed = run_ok(repo, &["diff", "--name-only", &range])?;
    let changed: Vec<String> = changed
        .stdout
        .lines()
        .map(|l| unquote_path(l.trim()))
        .filter(|l| !l.is_empty())
        .collect();
    Ok(dirty
        .into_iter()
        .map(|p| unquote_path(&p))
        .filter(|p| changed.iter().any(|c| c == p))
        .collect())
}

/// Git quotes a path containing non-ASCII bytes in its porcelain and
/// `--name-only` output. Both sides of the comparison above must be unquoted
/// the same way, or a Chinese path would never match itself.
fn unquote_path(path: &str) -> String {
    let trimmed = path.trim();
    if !(trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2) {
        return trimmed.to_string();
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let mut bytes = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            continue;
        }
        match chars.next() {
            // An octal escape, which is how Git writes a non-ASCII byte.
            Some(d) if d.is_digit(8) => {
                let mut octal = String::from(d);
                for _ in 0..2 {
                    match chars.clone().next() {
                        Some(n) if n.is_digit(8) => {
                            octal.push(n);
                            chars.next();
                        }
                        _ => break,
                    }
                }
                if let Ok(b) = u8::from_str_radix(&octal, 8) {
                    bytes.push(b);
                }
            }
            Some('n') => bytes.push(b'\n'),
            Some('t') => bytes.push(b'\t'),
            Some('"') => bytes.push(b'"'),
            Some('\\') => bytes.push(b'\\'),
            Some(other) => {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut buf).as_bytes());
            }
            None => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    Merged {
        commit: String,
    },
    /// A precondition failed. Distinct from an error: nothing was attempted,
    /// the user's tree is untouched, and the fix is theirs to make.
    Blocked {
        detail: String,
    },
}

/// Merges a task branch into the default branch in the project's main
/// worktree (design §6, requirement T-07).
///
/// Preconditions are checked here rather than trusted from the caller, because
/// the user may have dirtied their tree between the panel rendering and the
/// button press.
pub fn merge_task_branch(
    repo: &Path,
    default_branch: &str,
    task_branch: &str,
    message: &str,
) -> Result<MergeOutcome> {
    let conflicting = conflicting_dirty_paths(repo, default_branch, task_branch)?;
    if !conflicting.is_empty() {
        let shown: Vec<&str> = conflicting.iter().take(5).map(String::as_str).collect();
        let suffix = if conflicting.len() > shown.len() {
            format!(" 等 {} 个文件", conflicting.len())
        } else {
            String::new()
        };
        return Ok(MergeOutcome::Blocked {
            detail: format!(
                "主工作树里这些文件有未提交改动，而本次合并要改动它们：{}{suffix}",
                shown.join("、")
            ),
        });
    }

    let current = run_ok(repo, &["symbolic-ref", "--short", "HEAD"])?
        .line()
        .to_string();
    if current != default_branch {
        return Ok(MergeOutcome::Blocked {
            detail: format!("主工作树当前在 {current}，不是默认分支 {default_branch}"),
        });
    }

    if !is_ancestor(repo, default_branch, task_branch)? {
        return Ok(MergeOutcome::Blocked {
            detail: format!("{task_branch} 不在最新的 {default_branch} 之上，需要先 rebase"),
        });
    }

    let out = run_with_extra_env(
        repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "merge",
            "--no-ff",
            "--no-verify",
            "--no-gpg-sign",
            "-m",
            message,
            task_branch,
        ],
        &[
            ("GIT_AUTHOR_NAME", AUTOME_NAME),
            ("GIT_AUTHOR_EMAIL", AUTOME_EMAIL),
            ("GIT_COMMITTER_NAME", AUTOME_NAME),
            ("GIT_COMMITTER_EMAIL", AUTOME_EMAIL),
        ],
    )?;
    if !out.ok() {
        // Undo any partial merge state so the user's tree is exactly as it was.
        let _ = run(repo, &["merge", "--abort"]);
        return Ok(MergeOutcome::Blocked {
            detail: first_meaningful_line(&out.stderr)
                .or_else(|| first_meaningful_line(&out.stdout))
                .unwrap_or_else(|| "合并失败".into()),
        });
    }
    Ok(MergeOutcome::Merged {
        commit: head_sha(repo)?,
    })
}

// ---------------------------------------------------------------------------
// Diff summary, for the merge panel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub added: u32,
    pub deleted: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSummary {
    pub commits: u32,
    pub files: Vec<FileChange>,
    pub total_added: u32,
    pub total_deleted: u32,
}

/// What the merge panel shows (requirement T-05): commit count, file list and
/// per-file line counts. Deliberately not a line-level diff — the design sends
/// the user to their editor for that.
pub fn change_summary(repo: &Path, base: &str, branch: &str) -> Result<ChangeSummary> {
    let range = format!("{base}...{branch}");
    let commits = run_ok(repo, &["rev-list", "--count", &range])?
        .line()
        .parse::<u32>()
        .unwrap_or(0);
    let numstat = run_ok(repo, &["diff", "--numstat", &range])?;
    let mut files = Vec::new();
    let (mut total_added, mut total_deleted) = (0u32, 0u32);
    for line in numstat.stdout.lines() {
        let mut parts = line.split('\t');
        let (a, d, p) = match (parts.next(), parts.next(), parts.next()) {
            (Some(a), Some(d), Some(p)) => (a, d, p),
            _ => continue,
        };
        // Binary files report "-" for both counts.
        let added = a.parse::<u32>().unwrap_or(0);
        let deleted = d.parse::<u32>().unwrap_or(0);
        total_added += added;
        total_deleted += deleted;
        files.push(FileChange {
            path: p.to_string(),
            added,
            deleted,
        });
    }
    Ok(ChangeSummary {
        commits,
        files,
        total_added,
        total_deleted,
    })
}

/// Subject lines of the commits a merge would bring in, newest first.
pub fn commit_subjects(repo: &Path, base: &str, branch: &str) -> Result<Vec<String>> {
    let range = format!("{base}..{branch}");
    let out = run_ok(repo, &["log", "--format=%h %s", &range])?;
    Ok(out.stdout.lines().map(str::to_string).collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A throwaway directory. No `tempfile` dependency: this is four lines and
    /// the crate is in the path that touches user repositories.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("automed-git-{tag}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp dir");
            TempDir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A repository with one commit on `main`.
    fn repo_with_commit(tag: &str) -> TempDir {
        let dir = TempDir::new(tag);
        init(dir.path(), "main").unwrap();
        write(dir.path(), "README.md", "hello\n");
        // Every real project gets these two entries from `init::init`, and the
        // merge preconditions depend on them: an unignored `.worktree/` makes
        // the main worktree permanently dirty and blocks every merge.
        write(dir.path(), ".gitignore", ".worktree/\n.autome/output/\n");
        commit_paths(dir.path(), &["README.md", ".gitignore"], "initial").unwrap();
        dir
    }

    fn write(root: &Path, rel: &str, contents: &str) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, contents).unwrap();
    }

    fn git_available() -> bool {
        Command::new(git_binary())
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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

    #[test]
    fn sanitised_env_disables_config_prompts_and_editors() {
        let env = sanitised_env();
        assert_eq!(
            env.get("GIT_CONFIG_NOSYSTEM").map(String::as_str),
            Some("1")
        );
        assert_eq!(
            env.get("GIT_CONFIG_GLOBAL").map(String::as_str),
            Some("/dev/null")
        );
        assert_eq!(
            env.get("GIT_TERMINAL_PROMPT").map(String::as_str),
            Some("0")
        );
        assert!(env.contains_key("GIT_EDITOR"));
        assert!(env.contains_key("GIT_SEQUENCE_EDITOR"));
        assert_eq!(env.get("LC_ALL").map(String::as_str), Some("C"));
    }

    #[test]
    fn first_meaningful_line_skips_blanks_and_progress() {
        assert_eq!(
            first_meaningful_line("\n\nfatal: not a repository\n").as_deref(),
            Some("fatal: not a repository")
        );
        assert_eq!(first_meaningful_line("   \n\n"), None);
        assert_eq!(
            first_meaningful_line("remote: Counting\nfatal: nope").as_deref(),
            Some("fatal: nope")
        );
    }

    #[test]
    fn output_line_strips_the_trailing_newline() {
        let out = Output {
            stdout: "main\n".into(),
            stderr: String::new(),
            code: 0,
        };
        assert_eq!(out.line(), "main");
        assert!(out.ok());
    }

    #[test]
    fn init_creates_a_repo_with_the_requested_default_branch() {
        needs_git!();
        let dir = TempDir::new("init");
        init(dir.path(), "trunk").unwrap();
        assert!(is_repo_root(dir.path()));
        assert_eq!(default_branch(dir.path()).as_deref(), Some("trunk"));
        assert!(!has_commits(dir.path()), "no commit yet");
    }

    #[test]
    fn is_repo_root_is_false_for_a_plain_directory_and_for_a_subdirectory() {
        needs_git!();
        let plain = TempDir::new("plain");
        assert!(!is_repo_root(plain.path()));

        let repo = repo_with_commit("sub");
        fs::create_dir_all(repo.path().join("nested")).unwrap();
        assert!(is_repo_root(repo.path()));
        assert!(
            !is_repo_root(&repo.path().join("nested")),
            "a subdirectory is in the repo but is not its root"
        );
    }

    #[test]
    fn default_branch_falls_back_to_head_when_there_is_no_remote() {
        needs_git!();
        let repo = repo_with_commit("head-branch");
        assert_eq!(default_branch(repo.path()).as_deref(), Some("main"));
    }

    #[test]
    fn a_fresh_repo_is_clean_and_a_new_file_makes_it_dirty() {
        needs_git!();
        let repo = repo_with_commit("clean");
        assert!(is_clean(repo.path()).unwrap());
        write(repo.path(), "new.txt", "x");
        assert!(!is_clean(repo.path()).unwrap());
        assert_eq!(
            dirty_paths(repo.path()).unwrap(),
            vec!["new.txt".to_string()]
        );
    }

    #[test]
    fn commit_paths_stages_only_what_it_is_given() {
        needs_git!();
        let repo = repo_with_commit("narrow");
        write(repo.path(), "wanted.txt", "a");
        write(repo.path(), "unwanted.txt", "b");
        commit_paths(repo.path(), &["wanted.txt"], "add wanted").unwrap();
        // The other file is still uncommitted.
        assert_eq!(
            dirty_paths(repo.path()).unwrap(),
            vec!["unwanted.txt".to_string()]
        );
    }

    #[test]
    fn commit_paths_is_a_no_op_when_nothing_changed() {
        needs_git!();
        let repo = repo_with_commit("noop");
        let before = head_sha(repo.path()).unwrap();
        let result = commit_paths(repo.path(), &["README.md"], "again").unwrap();
        assert_eq!(result, None, "no empty commit");
        assert_eq!(head_sha(repo.path()).unwrap(), before);
    }

    #[test]
    fn commit_paths_with_no_paths_does_nothing() {
        needs_git!();
        let repo = repo_with_commit("empty-paths");
        assert_eq!(commit_paths(repo.path(), &[], "x").unwrap(), None);
    }

    #[test]
    fn a_worktree_can_be_added_listed_and_removed() {
        needs_git!();
        let repo = repo_with_commit("wt");
        worktree_add(repo.path(), ".worktree/feat", "autome/feat", "main").unwrap();
        assert!(repo.path().join(".worktree/feat/README.md").exists());
        let list = worktree_list(repo.path()).unwrap();
        assert_eq!(list.len(), 1, "main worktree is excluded: {list:?}");
        assert!(branch_exists(repo.path(), "autome/feat"));

        worktree_remove(repo.path(), ".worktree/feat", false).unwrap();
        assert!(worktree_list(repo.path()).unwrap().is_empty());
        branch_delete(repo.path(), "autome/feat", true).unwrap();
        assert!(!branch_exists(repo.path(), "autome/feat"));
    }

    #[test]
    fn worktree_prune_clears_a_directory_deleted_behind_gits_back() {
        needs_git!();
        let repo = repo_with_commit("prune");
        worktree_add(repo.path(), ".worktree/gone", "autome/gone", "main").unwrap();
        fs::remove_dir_all(repo.path().join(".worktree/gone")).unwrap();
        worktree_prune(repo.path()).unwrap();
        assert!(worktree_list(repo.path()).unwrap().is_empty());
    }

    #[test]
    fn a_clean_rebase_reports_clean() {
        needs_git!();
        let repo = repo_with_commit("rebase-clean");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "feature.txt", "feature\n");
        commit_paths(&wt, &["feature.txt"], "feature").unwrap();
        // Move main forward on a different file.
        write(repo.path(), "other.txt", "other\n");
        commit_paths(repo.path(), &["other.txt"], "other").unwrap();

        assert_eq!(rebase(&wt, "main").unwrap(), RebaseOutcome::Clean);
        assert!(is_ancestor(repo.path(), "main", "autome/f").unwrap());
    }

    #[test]
    fn a_conflicting_rebase_reports_the_files_and_leaves_no_rebase_in_progress() {
        needs_git!();
        let repo = repo_with_commit("rebase-conflict");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "README.md", "branch version\n");
        commit_paths(&wt, &["README.md"], "branch edit").unwrap();
        write(repo.path(), "README.md", "main version\n");
        commit_paths(repo.path(), &["README.md"], "main edit").unwrap();

        match rebase(&wt, "main").unwrap() {
            RebaseOutcome::Conflict { files, .. } => {
                assert_eq!(files, vec!["README.md".to_string()]);
            }
            other => panic!("expected a conflict, got {other:?}"),
        }
        // The abort must have run: no rebase directory left behind.
        assert!(!wt.join(".git").join("rebase-merge").exists());
        assert!(
            run(&wt, &["rev-parse", "--abbrev-ref", "HEAD"])
                .unwrap()
                .line()
                .contains("autome/f"),
            "still on the task branch after the abort"
        );
    }

    #[test]
    fn merge_writes_a_merge_commit_and_reports_it() {
        needs_git!();
        let repo = repo_with_commit("merge-ok");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "feature.txt", "x\n");
        commit_paths(&wt, &["feature.txt"], "feature").unwrap();

        let outcome =
            merge_task_branch(repo.path(), "main", "autome/f", "merge(autome): T-1").unwrap();
        match outcome {
            MergeOutcome::Merged { commit } => {
                assert_eq!(commit, head_sha(repo.path()).unwrap());
            }
            other => panic!("expected a merge, got {other:?}"),
        }
        assert!(repo.path().join("feature.txt").exists());
    }

    #[test]
    fn merge_is_blocked_by_a_dirty_main_worktree_and_changes_nothing() {
        needs_git!();
        let repo = repo_with_commit("merge-dirty");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "feature.txt", "x\n");
        commit_paths(&wt, &["feature.txt"], "feature").unwrap();

        // Uncommitted work in a file this merge would also write.
        write(repo.path(), "feature.txt", "user work in progress\n");
        let before = head_sha(repo.path()).unwrap();

        match merge_task_branch(repo.path(), "main", "autome/f", "m").unwrap() {
            MergeOutcome::Blocked { detail } => assert!(detail.contains("feature.txt"), "{detail}"),
            other => panic!("expected blocked, got {other:?}"),
        }
        assert_eq!(head_sha(repo.path()).unwrap(), before, "main is untouched");
        assert_eq!(
            fs::read_to_string(repo.path().join("feature.txt")).unwrap(),
            "user work in progress\n",
            "the user's file is untouched"
        );
    }

    #[test]
    fn unrelated_uncommitted_work_does_not_block_a_merge() {
        // The trap this closes: changing a Loop setting writes
        // `.autome/config.toml` and deliberately leaves it uncommitted, so a
        // "the tree must be clean" rule would block every merge in every task
        // from then on, citing a file the merge never touches.
        needs_git!();
        let repo = repo_with_commit("merge-unrelated-dirty");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "feature.txt", "x\n");
        commit_paths(&wt, &["feature.txt"], "feature").unwrap();

        // Something the user has not committed, which this merge will not
        // touch.
        write(
            repo.path(),
            ".autome/config.toml",
            "[roles.impl]\nmodel = \"x\"\n",
        );
        assert!(!is_clean(repo.path()).unwrap(), "the tree really is dirty");

        assert!(
            conflicting_dirty_paths(repo.path(), "main", "autome/f")
                .unwrap()
                .is_empty()
        );
        match merge_task_branch(repo.path(), "main", "autome/f", "m").unwrap() {
            MergeOutcome::Merged { .. } => {}
            other => panic!("expected the merge to proceed, got {other:?}"),
        }
        // And the user's uncommitted file is exactly as they left it.
        assert_eq!(
            fs::read_to_string(repo.path().join(".autome/config.toml")).unwrap(),
            "[roles.impl]\nmodel = \"x\"\n"
        );
    }

    #[test]
    fn uncommitted_work_in_a_file_the_merge_touches_still_blocks_it() {
        needs_git!();
        let repo = repo_with_commit("merge-overlapping-dirty");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "shared.txt", "from the branch\n");
        commit_paths(&wt, &["shared.txt"], "branch edit").unwrap();

        // The user is mid-edit on the same file.
        write(repo.path(), "shared.txt", "the user was here\n");

        let conflicting = conflicting_dirty_paths(repo.path(), "main", "autome/f").unwrap();
        assert_eq!(conflicting, vec!["shared.txt".to_string()]);
        match merge_task_branch(repo.path(), "main", "autome/f", "m").unwrap() {
            MergeOutcome::Blocked { detail } => assert!(detail.contains("shared.txt"), "{detail}"),
            other => panic!("expected blocked, got {other:?}"),
        }
        assert_eq!(
            fs::read_to_string(repo.path().join("shared.txt")).unwrap(),
            "the user was here\n",
            "their work is untouched"
        );
    }

    #[test]
    fn a_non_ascii_path_matches_itself_across_gits_two_output_formats() {
        // `git status --porcelain` and `git diff --name-only` both quote a
        // path containing non-ASCII bytes, and a mismatch here would mean a
        // Chinese filename never compares equal to itself — so an overlapping
        // edit would silently stop blocking the merge.
        needs_git!();
        let repo = repo_with_commit("merge-cjk");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "文档/说明.md", "branch\n");
        commit_paths(&wt, &["文档/说明.md"], "branch edit").unwrap();
        write(repo.path(), "文档/说明.md", "user\n");

        let conflicting = conflicting_dirty_paths(repo.path(), "main", "autome/f").unwrap();
        assert_eq!(
            conflicting,
            vec!["文档/说明.md".to_string()],
            "the quoted and unquoted forms must compare equal"
        );
    }

    #[test]
    fn unquote_path_decodes_gits_octal_escapes() {
        assert_eq!(unquote_path("plain.txt"), "plain.txt");
        assert_eq!(unquote_path("\"a b.txt\""), "a b.txt");
        // "中" is e4 b8 ad, which Git writes as \344\270\255.
        assert_eq!(unquote_path("\"\\344\\270\\255.md\""), "中.md");
        assert_eq!(unquote_path("\"a\\\"b\""), "a\"b");
    }

    #[test]
    fn merge_is_blocked_when_the_branch_is_behind_the_default_branch() {
        needs_git!();
        let repo = repo_with_commit("merge-stale");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "feature.txt", "x\n");
        commit_paths(&wt, &["feature.txt"], "feature").unwrap();
        // main moves on; the branch was never rebased.
        write(repo.path(), "other.txt", "y\n");
        commit_paths(repo.path(), &["other.txt"], "other").unwrap();

        match merge_task_branch(repo.path(), "main", "autome/f", "m").unwrap() {
            MergeOutcome::Blocked { detail } => assert!(detail.contains("rebase"), "{detail}"),
            other => panic!("expected blocked, got {other:?}"),
        }
    }

    #[test]
    fn merge_is_blocked_when_the_main_worktree_is_on_another_branch() {
        needs_git!();
        let repo = repo_with_commit("merge-wrong-branch");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "feature.txt", "x\n");
        commit_paths(&wt, &["feature.txt"], "feature").unwrap();
        run_ok(repo.path(), &["checkout", "-b", "side"]).unwrap();

        match merge_task_branch(repo.path(), "main", "autome/f", "m").unwrap() {
            MergeOutcome::Blocked { detail } => assert!(detail.contains("side"), "{detail}"),
            other => panic!("expected blocked, got {other:?}"),
        }
    }

    #[test]
    fn is_ancestor_distinguishes_contained_from_diverged() {
        needs_git!();
        let repo = repo_with_commit("ancestor");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        assert!(
            is_ancestor(repo.path(), "main", "autome/f").unwrap(),
            "a fresh branch contains main"
        );
        write(repo.path(), "other.txt", "y\n");
        commit_paths(repo.path(), &["other.txt"], "other").unwrap();
        assert!(
            !is_ancestor(repo.path(), "main", "autome/f").unwrap(),
            "main moved on"
        );
    }

    #[test]
    fn change_summary_counts_commits_files_and_lines() {
        needs_git!();
        let repo = repo_with_commit("summary");
        worktree_add(repo.path(), ".worktree/f", "autome/f", "main").unwrap();
        let wt = repo.path().join(".worktree/f");
        write(&wt, "a.txt", "1\n2\n3\n");
        commit_paths(&wt, &["a.txt"], "add a").unwrap();
        write(&wt, "b.txt", "1\n");
        commit_paths(&wt, &["b.txt"], "add b").unwrap();

        let s = change_summary(repo.path(), "main", "autome/f").unwrap();
        assert_eq!(s.commits, 2);
        assert_eq!(s.files.len(), 2);
        assert_eq!(s.total_added, 4);
        assert_eq!(s.total_deleted, 0);
        let subjects = commit_subjects(repo.path(), "main", "autome/f").unwrap();
        assert_eq!(subjects.len(), 2);
        assert!(subjects[0].contains("add b"), "newest first: {subjects:?}");
    }

    #[test]
    fn run_ok_surfaces_gits_own_message_on_failure() {
        needs_git!();
        let dir = TempDir::new("not-a-repo");
        let err = run_ok(dir.path(), &["status"]).unwrap_err();
        assert!(err.detail.contains("repository"), "{err:?}");
        assert_eq!(err.code, Some(128));
    }

    #[test]
    fn a_nonexistent_git_binary_is_an_error_not_a_panic() {
        // The binary is a parameter rather than an environment variable
        // precisely so this test cannot affect any other: an earlier version
        // set AUTOMED_GIT_BINARY and unset it, and every test that happened to
        // spawn git in that window failed with a spurious "cannot start git".
        let dir = TempDir::new("no-binary");
        let err = run_with_binary("/nonexistent/git", dir.path(), &["status"], &[]).unwrap_err();
        assert!(err.detail.contains("无法启动"), "{err:?}");
    }
}
