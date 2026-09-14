//! Owner-only filesystem discipline per §8.3:1362 — the data root and every
//! ProjectHome must be created `0700` (files inside them `0600`), and both
//! creation and every startup re-verification must reject: a symlinked
//! entry, wrong owner, a group/world-writable directory, a hard-linked
//! file, and a real (canonicalized) path that escapes its expected root.
//! One implementation, reused by both call sites — `store::create_project`
//! at creation time and `store::EventStore::open` at startup — so the two
//! checks can never drift apart.
//!
//! Known gap, in the same spirit as `harness_probe.rs`'s honest scoping:
//! this uses `symlink_metadata` on the leaf component plus a
//! `canonicalize()`-based boundary check, not `openat2(RESOLVE_NO_SYMLINKS)`
//! or per-component no-follow directory-fd walking (neither is available in
//! std, and the former is Linux-only). A symlink swapped into an
//! *intermediate* path component between this check and its use (TOCTOU),
//! or one that happens to resolve back inside the root, is not caught here.

use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Owner read/write/execute only.
const DIR_MODE: u32 = 0o700;
/// Owner read/write only.
const FILE_MODE: u32 = 0o600;
/// Group-write or other-write set on the raw permission bits.
const UNSAFE_WRITABLE_BITS: u32 = 0o022;

/// A directory that has just passed the full owner-only check. Deliberately
/// not `Copy` and not cached anywhere long-lived by this module — holding
/// one represents "verified just now", not a fact assumed to stay true
/// forever (callers that need a long-lived guarantee must re-verify).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedDirGuard {
    pub canonical_path: PathBuf,
    pub dev: u64,
    pub ino: u64,
    pub uid: u32,
    pub mode: u32,
}

#[derive(Debug, Error)]
pub enum FsGuardError {
    #[error("{0} is a symlink, refusing to follow it")]
    Symlink(PathBuf),
    #[error("{path} is owned by uid {actual}, expected the current process's uid {expected}")]
    WrongOwner {
        path: PathBuf,
        expected: u32,
        actual: u32,
    },
    #[error("{path} is group- or world-writable (mode {mode:03o})")]
    GroupOrWorldWritable { path: PathBuf, mode: u32 },
    #[error("{0} has more than one hard link, refusing to trust it")]
    HardLinkAnomaly(PathBuf),
    #[error("{path} resolves to {resolved}, which escapes the expected root {root}")]
    OutsideRoot {
        path: PathBuf,
        resolved: PathBuf,
        root: PathBuf,
    },
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

fn current_uid() -> u32 {
    // SAFETY: getuid(2) takes no arguments, dereferences no pointers, and
    // cannot fail.
    unsafe { libc::getuid() }
}

fn io_err(path: &Path, source: io::Error) -> FsGuardError {
    FsGuardError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Reads metadata for exactly `path`'s final component without following it
/// if it is itself a symlink. `fs::metadata` would silently follow;
/// `fs::symlink_metadata` does not — that difference is the whole reason
/// this helper exists instead of calling `fs::metadata` directly.
fn inspect_leaf(path: &Path) -> Result<fs::Metadata, FsGuardError> {
    let meta = fs::symlink_metadata(path).map_err(|source| io_err(path, source))?;
    if meta.file_type().is_symlink() {
        return Err(FsGuardError::Symlink(path.to_path_buf()));
    }
    Ok(meta)
}

fn check_owner(path: &Path, meta: &fs::Metadata) -> Result<(), FsGuardError> {
    let expected = current_uid();
    let actual = meta.uid();
    if actual != expected {
        return Err(FsGuardError::WrongOwner {
            path: path.to_path_buf(),
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_not_group_or_world_writable(path: &Path, meta: &fs::Metadata) -> Result<(), FsGuardError> {
    let mode = meta.mode() & 0o777;
    if mode & UNSAFE_WRITABLE_BITS != 0 {
        return Err(FsGuardError::GroupOrWorldWritable {
            path: path.to_path_buf(),
            mode,
        });
    }
    Ok(())
}

/// Only meaningful for files: directories legitimately carry `nlink > 1`
/// (their own `.` entry, plus one per subdirectory's `..`), so this is
/// never called on a directory.
fn check_single_hard_link(path: &Path, meta: &fs::Metadata) -> Result<(), FsGuardError> {
    if meta.nlink() > 1 {
        return Err(FsGuardError::HardLinkAnomaly(path.to_path_buf()));
    }
    Ok(())
}

/// Refuses to reuse whatever is already at `path` — including a dangling
/// symlink, which `symlink_metadata` still reports `Ok(..)` for even though
/// its target does not exist.
fn refuse_existing_entry(path: &Path) -> Result<(), FsGuardError> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(io_err(
            path,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "refusing to reuse an existing filesystem entry",
            ),
        ));
    }
    Ok(())
}

/// Creates `parent/name` as a fresh, owner-only (`0700`) directory and
/// returns its verified guard. `parent` itself must already pass the same
/// discipline (not a symlink, owned by us, not group/world-writable) — that
/// is what makes it safe to trust `parent.join(name)` afterwards.
pub fn create_owned_dir(parent: &Path, name: &str) -> Result<OwnedDirGuard, FsGuardError> {
    let parent_meta = inspect_leaf(parent)?;
    check_owner(parent, &parent_meta)?;
    check_not_group_or_world_writable(parent, &parent_meta)?;

    let target = parent.join(name);
    refuse_existing_entry(&target)?;

    let mut builder = DirBuilder::new();
    builder.mode(DIR_MODE);
    builder
        .create(&target)
        .map_err(|source| io_err(&target, source))?;

    verify_owned_dir(&target, parent)
}

/// Re-verifies an existing directory: not a symlink, owned by the current
/// process's uid, not group/world-writable, and its canonicalized real path
/// falls under `expect_root`'s canonicalized real path. Used both right
/// after `create_owned_dir` builds a fresh directory and, independently, at
/// every `EventStore::open` to re-check directories created in a past run.
pub fn verify_owned_dir(path: &Path, expect_root: &Path) -> Result<OwnedDirGuard, FsGuardError> {
    // Leaf check first, on the exact path given — `canonicalize()` below
    // would silently resolve straight through a symlink planted at `path`
    // itself, so the no-follow check has to happen before that call.
    let leaf_meta = inspect_leaf(path)?;
    if !leaf_meta.file_type().is_dir() {
        return Err(io_err(
            path,
            io::Error::new(io::ErrorKind::InvalidInput, "expected a directory"),
        ));
    }
    check_owner(path, &leaf_meta)?;
    check_not_group_or_world_writable(path, &leaf_meta)?;

    let canonical_path = path.canonicalize().map_err(|source| io_err(path, source))?;
    let canonical_root = expect_root
        .canonicalize()
        .map_err(|source| io_err(expect_root, source))?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(FsGuardError::OutsideRoot {
            path: path.to_path_buf(),
            resolved: canonical_path,
            root: canonical_root,
        });
    }

    Ok(OwnedDirGuard {
        dev: leaf_meta.dev(),
        ino: leaf_meta.ino(),
        uid: leaf_meta.uid(),
        mode: leaf_meta.mode() & 0o777,
        canonical_path,
    })
}

/// Writes `bytes` to a fresh, owner-only (`0600`) file named `name` inside
/// an already-verified directory. Refuses to overwrite an existing entry
/// (`create_new`), and re-checks the freshly-written file for the same
/// owner/writability/hard-link discipline the directory checks apply.
pub fn write_owned_file(dir: &OwnedDirGuard, name: &str, bytes: &[u8]) -> Result<(), FsGuardError> {
    let target = dir.canonical_path.join(name);
    refuse_existing_entry(&target)?;

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .open(&target)
        .map_err(|source| io_err(&target, source))?;
    file.write_all(bytes)
        .map_err(|source| io_err(&target, source))?;
    file.sync_all().map_err(|source| io_err(&target, source))?;
    drop(file);

    let meta = inspect_leaf(&target)?;
    check_owner(&target, &meta)?;
    check_not_group_or_world_writable(&target, &meta)?;
    check_single_hard_link(&target, &meta)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn temp_test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "automed-fs-guard-test-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        let mut builder = DirBuilder::new();
        builder.mode(DIR_MODE);
        builder.create(&root).unwrap();
        root
    }

    #[test]
    fn create_owned_dir_produces_an_owner_only_0700_directory() {
        let root = temp_test_root("create-mode");
        let guard = create_owned_dir(&root, "project-home").unwrap();
        assert_eq!(guard.mode, 0o700);
        assert!(guard.canonical_path.is_dir());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn write_owned_file_produces_an_owner_only_0600_file() {
        let root = temp_test_root("write-mode");
        let guard = create_owned_dir(&root, "project-home").unwrap();
        write_owned_file(&guard, "manifest.json", b"{}").unwrap();
        let file_path = guard.canonical_path.join("manifest.json");
        let mode = fs::symlink_metadata(&file_path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(fs::read(&file_path).unwrap(), b"{}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn verify_owned_dir_rejects_a_symlink_even_to_a_legitimate_directory() {
        let root = temp_test_root("reject-symlink");
        let real = root.join("real-project-home");
        fs::create_dir(&real).unwrap();
        let link = root.join("linked-project-home");
        symlink(&real, &link).unwrap();

        let err = verify_owned_dir(&link, &root).unwrap_err();
        assert!(matches!(err, FsGuardError::Symlink(p) if p == link));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn verify_owned_dir_rejects_a_directory_not_owned_by_the_current_process() {
        // Simulating "wrong owner" needs a real filesystem entry owned by a
        // different uid; the test process has no privilege to `chown` a
        // tempdir fixture to another user, so this deliberately uses a real
        // system path instead of a fresh tempdir (unlike the other four
        // fs_guard tests). "/" is root-owned (uid 0) on every macOS install,
        // which this workspace already targets exclusively.
        let root = Path::new("/");
        let err = verify_owned_dir(root, root).unwrap_err();
        match err {
            FsGuardError::WrongOwner {
                actual, expected, ..
            } => {
                assert_eq!(actual, 0);
                assert_ne!(expected, 0, "test process must not itself be root");
            }
            other => panic!("expected WrongOwner, got {other:?}"),
        }
    }

    #[test]
    fn verify_owned_dir_rejects_a_group_or_world_writable_directory() {
        let root = temp_test_root("reject-writable");
        let target = root.join("world-writable-home");
        fs::create_dir(&target).unwrap();
        let mut perms = fs::metadata(&target).unwrap().permissions();
        perms.set_mode(0o777);
        fs::set_permissions(&target, perms).unwrap();

        let err = verify_owned_dir(&target, &root).unwrap_err();
        assert!(matches!(
            err,
            FsGuardError::GroupOrWorldWritable { mode: 0o777, .. }
        ));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn verify_owned_dir_rejects_a_real_path_that_escapes_the_expected_root() {
        let root = temp_test_root("reject-escape");
        let inside_root = root.join("boundary");
        fs::create_dir(&inside_root).unwrap();
        let outside = temp_test_root("reject-escape-outside");

        let err = verify_owned_dir(&outside, &inside_root).unwrap_err();
        assert!(matches!(err, FsGuardError::OutsideRoot { .. }));
        fs::remove_dir_all(&root).ok();
        fs::remove_dir_all(&outside).ok();
    }

    #[test]
    fn write_owned_file_rejects_overwriting_an_existing_entry() {
        let root = temp_test_root("no-overwrite");
        let guard = create_owned_dir(&root, "project-home").unwrap();
        write_owned_file(&guard, "manifest.json", b"first").unwrap();
        let err = write_owned_file(&guard, "manifest.json", b"second").unwrap_err();
        assert!(matches!(err, FsGuardError::Io { .. }));
        assert_eq!(
            fs::read(guard.canonical_path.join("manifest.json")).unwrap(),
            b"first"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn create_owned_dir_refuses_a_second_call_with_the_same_name() {
        let root = temp_test_root("no-reuse-dir");
        create_owned_dir(&root, "project-home").unwrap();
        let err = create_owned_dir(&root, "project-home").unwrap_err();
        assert!(matches!(err, FsGuardError::Io { .. }));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn check_single_hard_link_rejects_a_file_with_more_than_one_link() {
        // `write_owned_file` creates and checks a file atomically in one
        // call, leaving no window within a single test to hard-link it
        // between creation and the check — so this exercises the private
        // helper directly against a real file and a real second hard link,
        // rather than going through the public `write_owned_file` API.
        let root = temp_test_root("hard-link");
        let original = root.join("original");
        fs::write(&original, b"data").unwrap();
        let meta = fs::symlink_metadata(&original).unwrap();
        check_single_hard_link(&original, &meta)
            .expect("a freshly-written file has exactly one link");

        let linked = root.join("linked");
        fs::hard_link(&original, &linked).unwrap();
        let meta = fs::symlink_metadata(&original).unwrap();
        let err = check_single_hard_link(&original, &meta).unwrap_err();
        assert!(matches!(err, FsGuardError::HardLinkAnomaly(p) if p == original));
        fs::remove_dir_all(&root).ok();
    }
}
