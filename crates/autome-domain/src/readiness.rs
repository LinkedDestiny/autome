//! ReadinessReceipt / SupportedEnvironmentProfile per plan §5.9.
//!
//! §5.9 also defines SignedDependencyCatalog, EnvironmentSnapshot,
//! EnvironmentRemediationPlan, EnvironmentInstallationTransaction,
//! UserActionChallenge and EnvironmentChangeReceipt — real macOS
//! system-integration surface (Homebrew probing, code-signature
//! verification, Keychain state, guided installs) rather than pure domain
//! logic. Those belong in `automed` against real system APIs and are
//! deliberately out of scope for this module; only the receipt/profile
//! shapes that gate other pure domain decisions (readiness-before-write,
//! staleness-invalidates-certificates) are modeled here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadinessScope {
    Planning,
    Execution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExistingRepoSubject {
    pub repository_identity_hash: String,
    pub base_commit: String,
    pub target_head: String,
    pub worktree_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GreenfieldSubject {
    pub parent_directory_identity_hash: String,
    pub destination: String,
    pub destination_absent: bool,
    pub template_hash: String,
}

/// §5.9: "`RepositoryIdentity` 绑定 canonical no-follow path、volume UUID、
/// device/inode/file ID... `DirectoryIdentity` 绑定绿地目标父目录的相同文件
/// 系统身份... 路径替换、mount 变化或 symlink 链变化会使所有审批失效." The
/// two subject kinds carry that identity as an opaque hash computed by
/// `automed`; this module only compares it, it never computes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadinessSubject {
    ExistingRepo(ExistingRepoSubject),
    Greenfield(GreenfieldSubject),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadinessResult {
    Ready,
    NotReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramObservation {
    pub canonical_path: String,
    pub version: String,
    pub binary_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessReceipt {
    pub revision: u32,
    pub scope: ReadinessScope,
    pub profile_hash: String,
    pub environment_relevant_inputs_digest: String,
    pub observed_at: String,
    pub valid_until: String,
    pub subject: ReadinessSubject,
    pub programs: Vec<ProgramObservation>,
    pub lockfile_hashes: Vec<String>,
    pub result: ReadinessResult,
    pub missing: Vec<String>,
    pub receipt_digest: String,
}

/// What a caller must recompute right now and compare against a receipt
/// before trusting it — mirrors `evidence::EvidenceFingerprint`'s staleness
/// rule. §5.9: "环境相关输入、profile 或模型资格变化会追加新的
/// ReadinessReceipt revision，并使相关 EvidenceReceipt、AuditVerdict 与
/// CandidateCertificate 失效."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessFingerprint {
    pub profile_hash: String,
    pub environment_relevant_inputs_digest: String,
    pub subject: ReadinessSubject,
}

impl ReadinessReceipt {
    pub fn is_current_against(&self, current: &ReadinessFingerprint) -> bool {
        self.profile_hash == current.profile_hash
            && self.environment_relevant_inputs_digest == current.environment_relevant_inputs_digest
            && self.subject == current.subject
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.result, ReadinessResult::Ready) && self.missing.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredProgram {
    pub name: String,
    pub version_range: String,
    pub source: String,
    pub digest_policy: String,
}

/// A frozen profile a Task's environment must match. §5.9: "现有仓库也必须
/// 显式匹配一个已资格认证的 host profile." No amendment story is defined
/// for this type yet (unlike TaskContract) — a profile change means a new
/// id/version, not an in-place mutation, so this struct has no setters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportedEnvironmentProfile {
    pub id: String,
    pub version: u32,
    pub profile_hash: String,
    pub task_kind: String,
    pub os_arch: String,
    pub required_programs: Vec<RequiredProgram>,
    pub lockfile_rules: Vec<String>,
    pub dependency_mode: String,
    pub sandbox_capabilities: Vec<String>,
    pub network_policy: String,
    pub qualified_check_runners: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> ReadinessSubject {
        ReadinessSubject::ExistingRepo(ExistingRepoSubject {
            repository_identity_hash: "repo-hash".into(),
            base_commit: "base".into(),
            target_head: "head".into(),
            worktree_fingerprint: "wt-1".into(),
        })
    }

    fn receipt(result: ReadinessResult, missing: Vec<String>) -> ReadinessReceipt {
        ReadinessReceipt {
            revision: 1,
            scope: ReadinessScope::Execution,
            profile_hash: "profile-1".into(),
            environment_relevant_inputs_digest: "env-digest-1".into(),
            observed_at: "2026-09-14T00:00:00Z".into(),
            valid_until: "2026-09-15T00:00:00Z".into(),
            subject: subject(),
            programs: vec![],
            lockfile_hashes: vec![],
            result,
            missing,
            receipt_digest: "receipt-digest-1".into(),
        }
    }

    fn matching_fingerprint() -> ReadinessFingerprint {
        ReadinessFingerprint {
            profile_hash: "profile-1".into(),
            environment_relevant_inputs_digest: "env-digest-1".into(),
            subject: subject(),
        }
    }

    #[test]
    fn ready_with_no_missing_is_ready() {
        assert!(receipt(ReadinessResult::Ready, vec![]).is_ready());
    }

    #[test]
    fn ready_result_with_missing_entries_is_not_ready() {
        assert!(!receipt(ReadinessResult::Ready, vec!["node".into()]).is_ready());
    }

    #[test]
    fn not_ready_result_is_not_ready() {
        assert!(!receipt(ReadinessResult::NotReady, vec![]).is_ready());
    }

    #[test]
    fn matching_fingerprint_is_current() {
        let r = receipt(ReadinessResult::Ready, vec![]);
        assert!(r.is_current_against(&matching_fingerprint()));
    }

    #[test]
    fn changed_environment_digest_makes_receipt_stale() {
        let r = receipt(ReadinessResult::Ready, vec![]);
        let mut fp = matching_fingerprint();
        fp.environment_relevant_inputs_digest = "env-digest-2".into();
        assert!(!r.is_current_against(&fp));
    }

    #[test]
    fn changed_profile_hash_makes_receipt_stale() {
        let r = receipt(ReadinessResult::Ready, vec![]);
        let mut fp = matching_fingerprint();
        fp.profile_hash = "profile-2".into();
        assert!(!r.is_current_against(&fp));
    }

    #[test]
    fn changed_subject_makes_receipt_stale() {
        let r = receipt(ReadinessResult::Ready, vec![]);
        let mut fp = matching_fingerprint();
        fp.subject = ReadinessSubject::Greenfield(GreenfieldSubject {
            parent_directory_identity_hash: "parent".into(),
            destination: "dest".into(),
            destination_absent: true,
            template_hash: "tmpl".into(),
        });
        assert!(!r.is_current_against(&fp));
    }
}
