//! Composes `workspace::create_disposable_clone`, a `cwd`-scoped Codex turn,
//! and `AttemptOutcome` classification into the first real Harness-to-Run
//! execution path (plan §5.6/§7.1).
//!
//! This is deliberately the *thinnest* version: it drives exactly one Codex
//! turn against a disposable clone and classifies the terminal outcome. It
//! does not compose planning/repair/evaluation, and it does not persist
//! anything via `store.rs` — the caller does that with the returned
//! `ExecutedAttempt`. This module exists to close the gap documented on
//! `attempt::AttemptOutcome` and `historical_red_light`: "a single boolean
//! fact recorded by whichever automed-side code drives the Harness" — this
//! is that code, for the Codex adapter specifically.

use std::path::Path;
use std::time::Duration;

use thiserror::Error;

use autome_domain::attempt::AttemptOutcome;

use crate::codex_transport::{self, CodexSandboxMode, CodexTransportError};
use crate::fs_guard::OwnedDirGuard;
use crate::workspace::{self, DisposableClone, WorkspaceError};

#[derive(Debug, Error)]
pub enum HarnessExecutorError {
    #[error("failed to prepare the disposable clone: {0}")]
    Workspace(#[from] WorkspaceError),
    #[error("codex transport failed: {0}")]
    CodexTransport(#[from] CodexTransportError),
    #[error("failed to compute the candidate tree hash: {0}")]
    TreeHash(String),
}

/// Result of driving one Codex turn against a fresh disposable clone: the
/// clone itself (so the caller can inspect or independently verify the
/// working tree), the raw turn fields, and the classified `AttemptOutcome`.
#[derive(Debug)]
pub struct ExecutedAttempt {
    pub clone: DisposableClone,
    pub turn_id: Option<String>,
    pub turn_status: String,
    pub error_message: Option<String>,
    pub outcome: AttemptOutcome,
}

/// The one Codex `turn/completed` status value this module treats as having
/// produced a real candidate. Every other observed or unobserved status
/// (e.g. the confirmed `"failed"` unauthenticated case) classifies as
/// `AttemptOutcome::AttemptFailed` — per §7.1, absence of a positive
/// success signal must never default to success.
const SUCCESS_STATUS: &str = "completed";

/// Stages the full working tree and writes it as a tree object, without
/// creating a commit. A disposable clone is single-purpose and fully owned
/// by this one Run, so `git add -A` cannot clobber unrelated work the way
/// it would in a shared checkout.
fn compute_candidate_tree_hash(repo_path: &Path) -> Result<String, HarnessExecutorError> {
    let repo_path_str = repo_path.to_string_lossy().into_owned();

    let add = std::process::Command::new("git")
        .args(["-C", &repo_path_str, "add", "-A"])
        .output()
        .map_err(|e| HarnessExecutorError::TreeHash(e.to_string()))?;
    if !add.status.success() {
        return Err(HarnessExecutorError::TreeHash(format!(
            "git add -A failed: {}",
            String::from_utf8_lossy(&add.stderr)
        )));
    }

    let write_tree = std::process::Command::new("git")
        .args(["-C", &repo_path_str, "write-tree"])
        .output()
        .map_err(|e| HarnessExecutorError::TreeHash(e.to_string()))?;
    if !write_tree.status.success() {
        return Err(HarnessExecutorError::TreeHash(format!(
            "git write-tree failed: {}",
            String::from_utf8_lossy(&write_tree.stderr)
        )));
    }

    let hash = String::from_utf8_lossy(&write_tree.stdout)
        .trim()
        .to_string();
    if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(HarnessExecutorError::TreeHash(format!(
            "git write-tree produced unexpected output: {hash:?}"
        )));
    }
    Ok(hash)
}

/// Bundled inputs for `execute_codex_attempt`. A plain struct rather than a
/// long parameter list — this has no invariant of its own to enforce
/// (`create_disposable_clone`/`probe_turn_to_completion` already validate
/// their own pieces), so it needs no dedicated constructor.
pub struct ExecuteCodexAttemptRequest<'a> {
    pub codex_binary: &'a Path,
    pub codex_home: &'a OwnedDirGuard,
    pub runs_root: &'a OwnedDirGuard,
    pub task_id: &'a str,
    pub run_id: &'a str,
    pub source_repo: &'a Path,
    pub instruction: &'a str,
    pub turn_completion_timeout: Duration,
}

/// Drives the thinnest real Harness-to-Run execution path for the Codex
/// adapter: clones `source_repo` into a disposable, single-purpose
/// workspace; runs one Codex turn against it under `workspace-write` (Codex's
/// own native sandbox disables network access by default under this mode,
/// confirmed empirically — see `codex_transport` module doc); classifies the
/// terminal outcome per §7.1.
pub async fn execute_codex_attempt(
    request: ExecuteCodexAttemptRequest<'_>,
) -> Result<ExecutedAttempt, HarnessExecutorError> {
    let clone = workspace::create_disposable_clone(
        request.runs_root,
        request.task_id,
        request.run_id,
        request.source_repo,
    )?;

    let turn_outcome = codex_transport::probe_turn_to_completion(
        request.codex_binary,
        request.codex_home,
        &clone.repo_path,
        CodexSandboxMode::WorkspaceWrite,
        request.instruction,
        request.turn_completion_timeout,
    )
    .await?;

    let outcome = if turn_outcome.status == SUCCESS_STATUS {
        let candidate_tree_hash = compute_candidate_tree_hash(&clone.repo_path)?;
        AttemptOutcome::ProducedCandidate {
            candidate_tree_hash,
        }
    } else {
        AttemptOutcome::AttemptFailed
    };

    Ok(ExecutedAttempt {
        clone,
        turn_id: turn_outcome.turn_id,
        turn_status: turn_outcome.status,
        error_message: turn_outcome.error_message,
        outcome,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_guard;

    fn scratch_dir(label: &str) -> OwnedDirGuard {
        let root = std::env::temp_dir().join(format!(
            "automed-harness-executor-test-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut perms = std::fs::metadata(&root).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        std::fs::set_permissions(&root, perms).unwrap();
        fs_guard::verify_owned_dir(&root, root.parent().unwrap()).unwrap()
    }

    fn run_git_checked(args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn fixture_source_repo() -> OwnedDirGuard {
        let dir = scratch_dir("source");
        let path_str = dir.canonical_path.to_string_lossy().into_owned();
        run_git_checked(&["-C", &path_str, "init", "--quiet"]);
        run_git_checked(&["-C", &path_str, "config", "user.email", "test@example.com"]);
        run_git_checked(&["-C", &path_str, "config", "user.name", "Test"]);
        std::fs::write(dir.canonical_path.join("README.md"), b"hello\n").unwrap();
        run_git_checked(&["-C", &path_str, "add", "README.md"]);
        run_git_checked(&["-C", &path_str, "commit", "--quiet", "-m", "initial"]);
        dir
    }

    /// Real-binary integration test, driving the full chain against an
    /// unauthenticated `codex` binary: the disposable clone is created, the
    /// turn round-trips to its confirmed `"failed"` terminal status (401,
    /// see `codex_transport` module doc), and this module classifies that
    /// as `AttemptOutcome::AttemptFailed` — never fabricating a
    /// `candidate_tree_hash` for a Harness that produced no real candidate.
    #[tokio::test]
    async fn an_unauthenticated_turn_classifies_as_attempt_failed_with_no_tree_hash() {
        let source = fixture_source_repo();
        let runs_root = scratch_dir("runs-root");
        let codex_home_root = scratch_dir("codex-home-root");
        let codex_home =
            fs_guard::create_owned_dir(&codex_home_root.canonical_path, "codex-home").unwrap();

        let executed = execute_codex_attempt(ExecuteCodexAttemptRequest {
            codex_binary: Path::new("/opt/homebrew/bin/codex"),
            codex_home: &codex_home,
            runs_root: &runs_root,
            task_id: "task-1",
            run_id: "run-1",
            source_repo: &source.canonical_path,
            instruction: "say hi",
            turn_completion_timeout: Duration::from_secs(90),
        })
        .await
        .unwrap();

        assert_eq!(executed.turn_status, "failed");
        assert!(executed.turn_id.is_some_and(|id| !id.is_empty()));
        assert!(
            executed
                .error_message
                .is_some_and(|msg| msg.contains("401"))
        );
        assert!(matches!(executed.outcome, AttemptOutcome::AttemptFailed));
        assert!(!executed.outcome.may_proceed_to_evaluation());
        assert!(executed.clone.repo_path.join("README.md").is_file());
    }
}
