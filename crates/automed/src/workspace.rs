//! Disposable clone creation for one Run's execution workspace (plan §8.1,
//! "Core 在 OS 应用数据目录创建 runs/<task-id>/<run-id>/repo，使用 git clone
//! --no-local --no-hardlinks 形成拥有独立 Git metadata/object store 的
//! disposable clone；不使用 linked worktree 充当安全边界。disposable clone
//! 禁用 push URL 和 credential helper.").
//!
//! This module owns exactly that slice: given an already-verified
//! `runs_root` (an `OwnedDirGuard`, matching `store.rs`'s `projects_root`
//! discipline) and a source repository, it produces a real, fully
//! independent clone — not a linked worktree, not a shared-object-store
//! clone — verifies the clone landed exactly where expected, strips its
//! push URL and credential helper, and reports the clone's HEAD commit.
//! Uses the real `git` binary throughout via a blocking `std::process::Command`
//! (matching `store.rs`/`dispatch.rs`'s synchronous style, not the
//! `tokio::process` style of `codex_transport.rs`/`claude_transport.rs` —
//! there is no long-lived interactive protocol here, just one blocking
//! command sequence), following this crate's "confirm empirically against
//! the real tool" discipline rather than guessing `git`'s CLI surface.
//!
//! Deliberately out of scope here, not guessed at:
//! - The sandbox *executor* that actually runs Agent/verifier commands
//!   inside this clone (§8.1's qualified sandbox executor) — this module
//!   only prepares the filesystem substrate it will later run inside.
//! - Process-group isolation, per-Run temp dir/port/test-data leases.
//! - The independent second low-privilege clone §8.1 describes for the
//!   verifier — callers get that by calling `create_disposable_clone`
//!   again with a different `run_id`; this module has no notion of
//!   "primary" vs. "verifier" clone.
//! - Recovery of a Run whose clone was only partially created before a
//!   crash: `create_owned_dir` refuses to reuse an existing `repo`
//!   directory, so a second call for the same `task_id`/`run_id` fails
//!   rather than silently resuming or overwriting.
//!
//! The push URL is disabled by pointing it at a value that is not a valid
//! transport (`disabled-by-autome-sandbox`) rather than removed outright —
//! `git remote` has no "no push URL at all" state distinct from "same as
//! fetch URL", so a push attempt must fail *for a specific, recognizable
//! reason* instead of silently succeeding against the fetch remote.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use thiserror::Error;

use crate::fs_guard::{self, FsGuardError, OwnedDirGuard};

/// Value `remote.origin.pushurl` is set to on every disposable clone. Not a
/// resolvable transport, so any push attempt fails closed instead of
/// silently landing on the fetch remote.
const DISABLED_PUSH_URL: &str = "disabled-by-autome-sandbox";

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("failed to prepare the runs directory tree: {0}")]
    FsGuard(#[from] FsGuardError),
    #[error("failed to spawn git {args:?}: {source}")]
    GitSpawn {
        args: Vec<String>,
        #[source]
        source: std::io::Error,
    },
    #[error("git {args:?} exited with status {status}: {stderr}")]
    GitFailed {
        args: Vec<String>,
        status: i32,
        stderr: String,
    },
    #[error("git rev-parse HEAD produced unexpected output: {0:?}")]
    UnexpectedHead(String),
}

/// A disposable, fully independent clone created for exactly one Run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisposableClone {
    pub repo_path: PathBuf,
    pub head_commit: String,
}

/// Creates or re-verifies `parent/name` as an owner-only directory,
/// mirroring the exact idiom `store.rs::create_from_target` already uses
/// for `projects_root` — reuse if a past run already created it, create
/// fresh otherwise. Unlike `create_owned_dir` alone, this is safe to call
/// repeatedly for the same `task_id` across multiple Runs.
fn ensure_owned_dir(parent: &OwnedDirGuard, name: &str) -> Result<OwnedDirGuard, FsGuardError> {
    let target = parent.canonical_path.join(name);
    if std::fs::symlink_metadata(&target).is_ok() {
        fs_guard::verify_owned_dir(&target, &parent.canonical_path)
    } else {
        fs_guard::create_owned_dir(&parent.canonical_path, name)
    }
}

/// Creates `data_root/runs` (or re-verifies it if a past run already
/// created it) as the owner-only root every disposable clone lives under.
/// Callers pass the result to `create_disposable_clone` as `runs_root`.
pub fn ensure_runs_root(data_root: &OwnedDirGuard) -> Result<OwnedDirGuard, WorkspaceError> {
    ensure_owned_dir(data_root, "runs").map_err(WorkspaceError::from)
}

/// Creates `data_root/codex-home` (or re-verifies it if a past run already
/// created it) as the owner-only `CODEX_HOME` every Codex-adapter turn runs
/// under (see `codex_transport`'s `CODEX_HOME`/`PATH`-only isolation
/// discipline). Shared and reused across Runs rather than one fresh
/// directory per attempt -- this is also where authentication state a
/// human operator sets up out of band will persist once real credentials
/// are provisioned.
pub fn ensure_codex_home(data_root: &OwnedDirGuard) -> Result<OwnedDirGuard, WorkspaceError> {
    ensure_owned_dir(data_root, "codex-home").map_err(WorkspaceError::from)
}

fn run_git(args: &[&str]) -> Result<Output, WorkspaceError> {
    Command::new("git")
        .args(args)
        .output()
        .map_err(|source| WorkspaceError::GitSpawn {
            args: args.iter().map(|s| s.to_string()).collect(),
            source,
        })
}

fn run_git_checked(args: &[&str]) -> Result<Output, WorkspaceError> {
    let output = run_git(args)?;
    if !output.status.success() {
        return Err(WorkspaceError::GitFailed {
            args: args.iter().map(|s| s.to_string()).collect(),
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output)
}

/// Builds `runs_root/<task_id>/<run_id>/repo` as a disposable, fully
/// independent clone of `source_repo`: `git clone --no-local --no-hardlinks`
/// into a pre-created owner-only directory (so the clone inherits `0700` on
/// its top-level dir regardless of the process umask git itself would
/// otherwise apply), then strips the clone's push URL and credential
/// helper, then resolves and returns its HEAD commit.
///
/// `runs_root` must already be an `OwnedDirGuard` (see `ensure_runs_root`).
/// The `<task_id>` directory is reused across Runs (`ensure_owned_dir`);
/// the `<run_id>/repo` directory is not — a second call with the same
/// `task_id`/`run_id` fails rather than reusing or overwriting a prior
/// clone.
pub fn create_disposable_clone(
    runs_root: &OwnedDirGuard,
    task_id: &str,
    run_id: &str,
    source_repo: &Path,
) -> Result<DisposableClone, WorkspaceError> {
    let task_dir = ensure_owned_dir(runs_root, task_id)?;
    let run_dir = fs_guard::create_owned_dir(&task_dir.canonical_path, run_id)?;
    let repo_dir = fs_guard::create_owned_dir(&run_dir.canonical_path, "repo")?;
    let repo_path = repo_dir.canonical_path;

    let source_repo_str = source_repo.to_string_lossy().into_owned();
    let repo_path_str = repo_path.to_string_lossy().into_owned();
    run_git_checked(&[
        "clone",
        "--no-local",
        "--no-hardlinks",
        "--",
        &source_repo_str,
        &repo_path_str,
    ])?;

    // Re-verify: `git clone` just populated this directory, so re-check the
    // no-symlink/owner/writability/boundary discipline on the now-populated
    // tree's top-level entry before trusting it further (TOCTOU-in-spirit
    // with the rest of this crate's fs_guard call sites).
    fs_guard::verify_owned_dir(&repo_path, &run_dir.canonical_path)?;

    run_git_checked(&[
        "-C",
        &repo_path_str,
        "config",
        "--local",
        "credential.helper",
        "",
    ])?;
    run_git_checked(&[
        "-C",
        &repo_path_str,
        "config",
        "--local",
        "remote.origin.pushurl",
        DISABLED_PUSH_URL,
    ])?;

    let head_output = run_git_checked(&["-C", &repo_path_str, "rev-parse", "HEAD"])?;
    let head_commit = String::from_utf8_lossy(&head_output.stdout)
        .trim()
        .to_string();
    if head_commit.len() != 40 || !head_commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(WorkspaceError::UnexpectedHead(
            String::from_utf8_lossy(&head_output.stdout).into_owned(),
        ));
    }

    Ok(DisposableClone {
        repo_path,
        head_commit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(label: &str) -> OwnedDirGuard {
        let root = std::env::temp_dir().join(format!(
            "automed-workspace-test-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut perms = std::fs::metadata(&root).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        std::fs::set_permissions(&root, perms).unwrap();
        fs_guard::verify_owned_dir(&root, root.parent().unwrap()).unwrap()
    }

    /// Builds a throwaway real Git repository (via the real `git` binary,
    /// no libgit2/gix) with one commit, to act as `source_repo` in tests.
    fn fixture_source_repo() -> (OwnedDirGuard, String) {
        let dir = scratch_dir("source");
        let path_str = dir.canonical_path.to_string_lossy().into_owned();
        run_git_checked(&["-C", &path_str, "init", "--quiet"]).unwrap();
        run_git_checked(&["-C", &path_str, "config", "user.email", "test@example.com"]).unwrap();
        run_git_checked(&["-C", &path_str, "config", "user.name", "Test"]).unwrap();
        std::fs::write(dir.canonical_path.join("README.md"), b"hello\n").unwrap();
        run_git_checked(&["-C", &path_str, "add", "README.md"]).unwrap();
        run_git_checked(&["-C", &path_str, "commit", "--quiet", "-m", "initial"]).unwrap();
        let head = run_git_checked(&["-C", &path_str, "rev-parse", "HEAD"]).unwrap();
        let head_commit = String::from_utf8_lossy(&head.stdout).trim().to_string();
        (dir, head_commit)
    }

    #[test]
    fn creates_an_independent_clone_with_matching_head_and_disabled_push_and_credentials() {
        let (source_dir, expected_head) = fixture_source_repo();
        let runs_root = scratch_dir("runs-root");

        let clone = create_disposable_clone(
            &runs_root,
            "task-1",
            "run-1",
            &source_dir.canonical_path,
        )
        .unwrap();

        assert_eq!(clone.head_commit, expected_head);
        assert!(clone.repo_path.starts_with(&runs_root.canonical_path));
        assert!(clone.repo_path.join("README.md").is_file());

        // Independent object store: it is a real clone, not a linked
        // worktree sharing the source's `.git` — the source repo staying
        // fully intact after the clone (still has its own commit history)
        // is the observable proxy for that, since std has no portable way
        // to assert "two dirs do not share inode-linked git objects" more
        // directly than `--no-hardlinks` + `--no-local` already guarantee.
        assert!(source_dir.canonical_path.join(".git").is_dir());

        let repo_path_str = clone.repo_path.to_string_lossy().into_owned();
        let pushurl = run_git_checked(&[
            "-C",
            &repo_path_str,
            "config",
            "--local",
            "--get",
            "remote.origin.pushurl",
        ])
        .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&pushurl.stdout).trim(),
            DISABLED_PUSH_URL
        );

        let helper = run_git(&[
            "-C",
            &repo_path_str,
            "config",
            "--local",
            "--get",
            "credential.helper",
        ])
        .unwrap();
        // An empty configured value round-trips as an empty (not missing)
        // line from `git config --get` — this is what "cleared, not just
        // absent" looks like, distinguishing it from never having been set.
        assert!(helper.status.success());
        assert_eq!(String::from_utf8_lossy(&helper.stdout).trim(), "");
    }

    #[test]
    fn a_second_run_for_the_same_task_gets_its_own_independent_clone() {
        let (source_dir, expected_head) = fixture_source_repo();
        let runs_root = scratch_dir("runs-root");

        let first = create_disposable_clone(&runs_root, "task-1", "run-1", &source_dir.canonical_path)
            .unwrap();
        let second =
            create_disposable_clone(&runs_root, "task-1", "run-2", &source_dir.canonical_path)
                .unwrap();

        assert_ne!(first.repo_path, second.repo_path);
        assert_eq!(first.head_commit, expected_head);
        assert_eq!(second.head_commit, expected_head);
    }

    #[test]
    fn refuses_to_reuse_an_existing_run_directory() {
        let (source_dir, _) = fixture_source_repo();
        let runs_root = scratch_dir("runs-root");

        create_disposable_clone(&runs_root, "task-1", "run-1", &source_dir.canonical_path).unwrap();
        let err = create_disposable_clone(&runs_root, "task-1", "run-1", &source_dir.canonical_path)
            .unwrap_err();
        assert!(matches!(err, WorkspaceError::FsGuard(_)));
    }
}
