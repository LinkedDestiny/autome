//! Pure preparation identity and rerun decisions; the host owns file I/O.
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checkout {
    Worktree,
    Verify,
    Main,
}

impl Checkout {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Worktree => "worktree",
            Self::Verify => "verify",
            Self::Main => "main",
        }
    }
}

pub fn valid_watch_path(path: &str) -> bool {
    let path = path.trim();
    !path.is_empty() && !path.starts_with('/') && !path.split('/').any(|part| part == "..")
}

pub fn fingerprint(command: &str, files: &[(&str, Option<&[u8]>)]) -> String {
    let mut hash = Sha256::new();
    hash.update(format!("command\0{command}\n"));
    for (path, content) in files.iter().copied().collect::<BTreeMap<_, _>>() {
        let digest = content.map_or_else(
            || "absent".to_string(),
            |bytes| format!("{:x}", Sha256::digest(bytes)),
        );
        hash.update(format!("{path}\0{digest}\n"));
    }
    format!("{:x}", hash.finalize())
}

pub fn needs_run(record: Option<&str>, current: &str) -> bool {
    record != Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_fingerprint_changes_when_a_watched_file_changes() {
        assert_ne!(
            fingerprint("install", &[("lock", Some(b"a"))]),
            fingerprint("install", &[("lock", Some(b"b"))])
        );
        assert_ne!(
            fingerprint("install", &[("lock", Some(b"a"))]),
            fingerprint("install", &[("other", Some(b"a"))])
        );
        assert_eq!(fingerprint("", &[]).len(), 64);
    }
    #[test]
    fn setup_fingerprint_changes_when_the_command_changes() {
        assert_ne!(fingerprint("a", &[]), fingerprint("b", &[]));
    }
    #[test]
    fn setup_fingerprint_tells_a_missing_file_from_an_empty_one() {
        assert_ne!(
            fingerprint("a", &[("lock", None)]),
            fingerprint("a", &[("lock", Some(b""))])
        );
    }
    #[test]
    fn setup_fingerprint_ignores_watch_order_and_duplicates() {
        assert_eq!(
            fingerprint("a", &[("x", None), ("y", Some(b"1"))]),
            fingerprint("a", &[("y", Some(b"1")), ("x", None), ("x", None)])
        );
    }
    #[test]
    fn setup_is_needed_without_a_record_or_with_a_different_one_and_not_with_the_same() {
        assert!(needs_run(None, "a"));
        assert!(needs_run(Some("b"), "a"));
        assert!(!needs_run(Some("a"), "a"));
        assert!(!needs_run(Some(""), ""));
        assert_eq!(
            [Checkout::Worktree, Checkout::Verify, Checkout::Main].map(Checkout::as_str),
            ["worktree", "verify", "main"]
        );
    }
}
