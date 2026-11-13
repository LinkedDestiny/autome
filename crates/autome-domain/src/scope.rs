//! Durable intent and result of a path-scoped restoration.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeRestored {
    pub task: String,
    pub session_id: String,
    pub role: String,
    pub label: String,
    pub round: u32,
    pub repos: Vec<RestoredRepo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoredRepo {
    pub repo: String,
    pub worktree: PathBuf,
    pub base: String,
    pub pre_head: String,
    pub commit: Option<String>,
    pub paths: Vec<String>,
    pub patch: PathBuf,
    pub stat: String,
}

pub const BREAKER_NOTE: &str = "上一次已自动还原";

/// Preserve ledger order, including intents interrupted before restoration.
/// A multi-repository intent remains pending until every patch is handled.
pub fn pending<'a>(
    begun: &'a [ScopeRestored],
    failed: &[String],
    handled: &[PathBuf],
) -> Vec<&'a ScopeRestored> {
    begun
        .iter()
        .filter(|entry| {
            !failed.contains(&entry.session_id)
                && entry
                    .repos
                    .iter()
                    .any(|repo| !handled.contains(&repo.patch))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(id: &str, patches: &[&str]) -> ScopeRestored {
        ScopeRestored {
            task: "T-69".into(),
            session_id: id.into(),
            role: "audit".into(),
            label: "审计".into(),
            round: u32::MAX,
            repos: patches
                .iter()
                .map(|patch| RestoredRepo {
                    repo: "成员".into(),
                    worktree: "/tmp/工作 树".into(),
                    base: "base".into(),
                    pre_head: "head".into(),
                    commit: None,
                    paths: vec!["a [1]*.rs".into()],
                    patch: patch.into(),
                    stat: "1 file changed".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn scope_patch_pending_excludes_failed_and_handled_in_ledger_order() {
        let begun = vec![
            intent("z", &["z.patch"]),
            intent("failed", &["f.patch"]),
            intent("handled", &["h.patch"]),
            intent("a", &["a1.patch", "a2.patch"]),
        ];
        let result = pending(
            &begun,
            &["failed".into()],
            &["h.patch".into(), "a1.patch".into()],
        );
        assert_eq!(result, vec![&begun[0], &begun[3]]);
        assert!(result[0].repos[0].commit.is_none());
    }

    #[test]
    fn scope_patch_pending_empty_after_all_handled_or_failed() {
        assert!(pending(&[], &[], &[]).is_empty());
        let begun = vec![intent("s", &["one", "two"])];
        assert_eq!(pending(&begun, &[], &[]), vec![&begun[0]]);
        assert!(pending(&begun, &["s".into()], &[]).is_empty());
        assert!(pending(&begun, &[], &["one".into(), "two".into()]).is_empty());
        assert!(pending(&[intent("empty", &[])], &[], &[]).is_empty());
    }

    #[test]
    fn scope_patch_payload_json_round_trip() {
        for commit in [None, Some("restore-sha".to_owned())] {
            let mut value = intent("s", &["/tmp/补丁 空格.patch"]);
            value.repos[0].commit = commit;
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(serde_json::from_str::<ScopeRestored>(&json).unwrap(), value);
        }
    }
}
