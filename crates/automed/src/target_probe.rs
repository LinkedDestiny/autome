//! Narrow, mechanically-defined slice of a candidate project target's git
//! identity, probed by invoking `git` itself (no git library dependency),
//! under the exact hardening discipline §4.1:306 requires for every git
//! invocation in this codebase: argv arrays (never a shell string), a
//! cleared environment with only `PATH` re-added, `GIT_CONFIG_NOSYSTEM=1`,
//! `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`,
//! `GIT_OPTIONAL_LOCKS=0`, `GIT_ASKPASS=`, `-c core.hooksPath=/dev/null -c
//! credential.helper=`, and exclusively machine-readable flags
//! (`--porcelain`/`-z`/`--show-object-format`) — never parsing git's
//! colored, human-facing output.
//!
//! Named `TargetIdentityProbe`, deliberately not `RepositoryIdentity`, for
//! the same reason `harness_probe.rs` calls its result `HarnessBinaryProbe`
//! rather than `HarnessCapabilitySnapshot`: this binds only what four local,
//! offline `git` invocations plus one local file read can mechanically
//! answer — canonical no-follow path, `st_dev`/`st_ino`, the repository's
//! common dir, HEAD's resolved commit (if any), a worktree-cleanliness
//! judgment from `status --porcelain -z`, the object format, and a digest
//! of the on-disk `<common-dir>/config` bytes (a fifth git subprocess is
//! not needed for that last one — the common dir is already resolved by
//! the first call, so this is a plain file read and hash).
//!
//! Known, deliberate gaps (documented here rather than guessed at):
//! - **Volume UUID.** Needs IOKit/`statfs`, neither reachable from std.
//!   `st_dev` is used as an in-process proxy — valid only for the lifetime
//!   of a single OS boot, not a stable cross-reboot identity.
//! - **A long-held, protected directory handle** per §5.9:858, meant to
//!   support pre/post-delivery re-checks around M2's CAS/rename machinery.
//!   This module only ever opens paths transiently to probe them; it holds
//!   nothing open past the end of `probe_target`.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetIdentityProbe {
    pub canonical_path: PathBuf,
    pub dev: u64,
    pub ino: u64,
    pub is_git_repo: bool,
    pub git_common_dir: Option<PathBuf>,
    pub object_format: Option<String>,
    pub config_digest_sha256_hex: Option<String>,
    pub head_commit: Option<String>,
    pub head_resolvable: bool,
    pub worktree_clean: bool,
}

impl TargetIdentityProbe {
    /// Builds the pure `TargetInspection` judgment input `locator_for`
    /// (`autome-domain::project`) consumes. `destination_absent` is
    /// deliberately not part of this probe — it must be re-checked fresh
    /// at creation time, not cached from registration time.
    pub fn to_inspection(
        &self,
        destination_absent: bool,
    ) -> autome_domain::project::TargetInspection {
        autome_domain::project::TargetInspection {
            is_git_repo: self.is_git_repo,
            head_resolvable: self.head_resolvable,
            worktree_clean: self.worktree_clean,
            destination_absent,
        }
    }
}

#[derive(Debug, Error)]
pub enum TargetProbeError {
    #[error("{0} is a symlink, refusing to follow it")]
    Symlink(PathBuf),
    #[error("{0} does not exist or is not readable")]
    NotFound(PathBuf),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to spawn git -C {path}: {source}")]
    Spawn {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("git -C {path} {args} produced non-UTF-8 output")]
    NonUtf8Output { path: PathBuf, args: String },
}

/// A `git` invocation hardened per §4.1:306: no shell, a cleared
/// environment with only `PATH` preserved, every credential/prompt/lock
/// side channel disabled, and hooks/credential-helper neutralized via `-c`.
fn git_command(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.env_clear();
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd.env("GIT_ASKPASS", "");
    cmd.arg("-c").arg("core.hooksPath=/dev/null");
    cmd.arg("-c").arg("credential.helper=");
    cmd.arg("-C").arg(dir);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd
}

fn run_git(dir: &Path, args: &[&str]) -> Result<Output, TargetProbeError> {
    git_command(dir)
        .args(args)
        .output()
        .map_err(|source| TargetProbeError::Spawn {
            path: dir.to_path_buf(),
            source,
        })
}

fn utf8_stdout(dir: &Path, args: &[&str], output: &Output) -> Result<String, TargetProbeError> {
    std::str::from_utf8(&output.stdout)
        .map(|s| s.trim().to_string())
        .map_err(|_| TargetProbeError::NonUtf8Output {
            path: dir.to_path_buf(),
            args: args.join(" "),
        })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Probes `path` for its git identity. Rejects a symlinked `path` itself
/// (no-follow, matching `fs_guard`'s discipline) before doing anything
/// else; a path that is not a git repository at all is not an error here —
/// `is_git_repo: false` with every other git-derived field left at its
/// default is a legitimate, common result (the greenfield "new product"
/// flow probes an ordinary directory for exactly this reason).
pub fn probe_target(path: &Path) -> Result<TargetIdentityProbe, TargetProbeError> {
    let leaf_meta = std::fs::symlink_metadata(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            TargetProbeError::NotFound(path.to_path_buf())
        } else {
            TargetProbeError::Io {
                path: path.to_path_buf(),
                source,
            }
        }
    })?;
    if leaf_meta.file_type().is_symlink() {
        return Err(TargetProbeError::Symlink(path.to_path_buf()));
    }
    let canonical_path = path.canonicalize().map_err(|source| TargetProbeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let dev = leaf_meta.dev();
    let ino = leaf_meta.ino();

    let common_dir_args = ["rev-parse", "--git-common-dir"];
    let common_dir_output = run_git(&canonical_path, &common_dir_args)?;
    let is_git_repo = common_dir_output.status.success();

    let mut git_common_dir = None;
    let mut object_format = None;
    let mut config_digest_sha256_hex = None;
    let mut head_commit = None;
    let mut head_resolvable = false;
    let mut worktree_clean = false;

    if is_git_repo {
        let common_dir_text = utf8_stdout(&canonical_path, &common_dir_args, &common_dir_output)?;
        let resolved_common_dir = {
            let candidate = PathBuf::from(&common_dir_text);
            if candidate.is_absolute() {
                candidate
            } else {
                canonical_path.join(candidate)
            }
        };

        // Best-effort: unreadable/unusual layouts leave the digest `None`
        // rather than failing the whole probe — the config digest is an
        // auxiliary tamper-evidence fact, not a precondition for judging
        // whether `path` is usable as a project target.
        config_digest_sha256_hex = std::fs::read(resolved_common_dir.join("config"))
            .ok()
            .map(|bytes| sha256_hex(&bytes));
        git_common_dir = Some(resolved_common_dir);

        let head_args = ["rev-parse", "HEAD"];
        let head_output = run_git(&canonical_path, &head_args)?;
        if head_output.status.success() {
            head_commit = Some(utf8_stdout(&canonical_path, &head_args, &head_output)?);
            head_resolvable = true;
        }

        let status_args = ["status", "--porcelain", "-z"];
        let status_output = run_git(&canonical_path, &status_args)?;
        // `-z` output is NUL-separated, not newline-terminated text — do
        // not trim/parse it as a string. A clean worktree is exactly empty
        // stdout on success.
        worktree_clean = status_output.status.success() && status_output.stdout.is_empty();

        let object_format_args = ["rev-parse", "--show-object-format"];
        let object_format_output = run_git(&canonical_path, &object_format_args)?;
        if object_format_output.status.success() {
            object_format = Some(utf8_stdout(
                &canonical_path,
                &object_format_args,
                &object_format_output,
            )?);
        }
    }

    Ok(TargetIdentityProbe {
        canonical_path,
        dev,
        ino,
        is_git_repo,
        git_common_dir,
        object_format,
        config_digest_sha256_hex,
        head_commit,
        head_resolvable,
        worktree_clean,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn temp_test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "automed-target-probe-test-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }

    /// Runs a `git` command against `dir` with the same hardened env
    /// `git_command` uses, for building real fixtures (not the code under
    /// test itself).
    fn fixture_git(dir: &Path, args: &[&str]) {
        let status = git_command(dir)
            .args(args)
            .status()
            .expect("fixture git command should spawn");
        assert!(status.success(), "fixture git {args:?} failed in {dir:?}");
    }

    fn init_repo_with_one_commit(dir: &Path) {
        fixture_git(dir, &["init", "--quiet"]);
        fixture_git(
            dir,
            &[
                "-c",
                "user.email=test@example.com",
                "-c",
                "user.name=test",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "initial",
            ],
        );
    }

    #[test]
    fn a_real_clean_git_repository_is_identified_in_full() {
        let root = temp_test_root("clean-repo");
        init_repo_with_one_commit(&root);

        let probe = probe_target(&root).unwrap();
        assert!(probe.is_git_repo);
        assert!(probe.head_resolvable);
        assert!(probe.head_commit.is_some());
        assert!(probe.worktree_clean);
        assert!(probe.git_common_dir.is_some());
        assert_eq!(probe.object_format.as_deref(), Some("sha1"));
        assert!(probe.config_digest_sha256_hex.is_some());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_plain_directory_with_no_git_is_reported_as_not_a_repo() {
        let root = temp_test_root("plain-dir");
        let probe = probe_target(&root).unwrap();
        assert!(!probe.is_git_repo);
        assert!(!probe.head_resolvable);
        assert!(!probe.worktree_clean);
        assert!(probe.git_common_dir.is_none());
        assert!(probe.object_format.is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_nonexistent_path_is_reported_as_not_found() {
        let root = temp_test_root("missing-parent");
        let missing = root.join("does-not-exist");
        let err = probe_target(&missing).unwrap_err();
        assert!(matches!(err, TargetProbeError::NotFound(p) if p == missing));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_symlink_pointing_at_a_repository_is_rejected_without_following_it() {
        let root = temp_test_root("symlink-to-repo");
        let real = root.join("real-repo");
        std::fs::create_dir(&real).unwrap();
        init_repo_with_one_commit(&real);
        let link = root.join("linked-repo");
        symlink(&real, &link).unwrap();

        let err = probe_target(&link).unwrap_err();
        assert!(matches!(err, TargetProbeError::Symlink(p) if p == link));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_dirty_worktree_is_reported_as_not_clean() {
        let root = temp_test_root("dirty-worktree");
        init_repo_with_one_commit(&root);
        std::fs::write(root.join("untracked.txt"), b"dirty").unwrap();

        let probe = probe_target(&root).unwrap();
        assert!(probe.is_git_repo);
        assert!(!probe.worktree_clean);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_unborn_head_is_reported_as_unresolvable_but_still_a_repo() {
        let root = temp_test_root("unborn-head");
        fixture_git(&root, &["init", "--quiet"]);

        let probe = probe_target(&root).unwrap();
        assert!(probe.is_git_repo);
        assert!(!probe.head_resolvable);
        assert!(probe.head_commit.is_none());
        std::fs::remove_dir_all(&root).ok();
    }
}
