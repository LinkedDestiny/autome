//! §8.3 凭证与日志: CredentialRecord/CredentialReceipt.
//!
//! Deliberately does not model real OS Keychain access, secret material,
//! or auth-file contents — this crate has no I/O. What §8.3 requires of
//! the *record about* a credential (never the credential itself) is
//! captured here: `CredentialRecord` can structurally never hold secret
//! material (no field exists for it, the same technique as `AgentClaim`
//! in `attempt.rs`); its `storage_kind` and `storage_location` must
//! agree, mechanizing "Codex OAuth...不与 Autome 自建 API-key item
//! 混称" as a shape check rather than a provider-name guess; and its
//! `status` must be consistent with which timestamps are actually
//! present. `CredentialReceipt` is the append-only audit trail for every
//! lifecycle event §8.3 names.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthMode {
    ApiKey,
    OAuth,
}

/// §8.3: "Codex OAuth 由其 owner-only 独立 CODEX_HOME/auth.json 与官方
/// 凭证机制管理...不与 Autome 自建 API-key item 混称" — two storage
/// mechanisms that must never be conflated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageKind {
    AutomeManagedKeychainItem,
    ProviderOwnedAuthFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeychainItemIdentity {
    pub service: String,
    pub account: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthFileIdentity {
    pub path: String,
    pub file_identity_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageLocation {
    Keychain(KeychainItemIdentity),
    AuthFile(AuthFileIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialStatus {
    Active,
    Rotated,
    Revoked,
}

/// §8.3: "CredentialRecord 只保存 provider、auth mode、storage kind、
/// Keychain service/account 或专用 auth file 的不可逆标识、创建/轮换/
/// 撤销时间和状态，不保存秘密." There is no field here that could hold
/// secret material — that is the enforcement, not a runtime check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRecord {
    pub provider: String,
    pub auth_mode: AuthMode,
    pub storage_kind: StorageKind,
    pub storage_location: StorageLocation,
    pub created_at: String,
    pub rotated_at: Option<String>,
    pub revoked_at: Option<String>,
    pub status: CredentialStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialShapeError {
    StorageKindLocationMismatch,
    RevokedStatusRequiresRevokedAt,
    RotatedStatusRequiresRotatedAt,
}

impl CredentialRecord {
    pub fn validate_shape(&self) -> Vec<CredentialShapeError> {
        let mut errors = Vec::new();

        let location_is_keychain = matches!(self.storage_location, StorageLocation::Keychain(_));
        let kind_is_keychain = matches!(self.storage_kind, StorageKind::AutomeManagedKeychainItem);
        if location_is_keychain != kind_is_keychain {
            errors.push(CredentialShapeError::StorageKindLocationMismatch);
        }

        if matches!(self.status, CredentialStatus::Revoked) && self.revoked_at.is_none() {
            errors.push(CredentialShapeError::RevokedStatusRequiresRevokedAt);
        }
        if matches!(self.status, CredentialStatus::Rotated) && self.rotated_at.is_none() {
            errors.push(CredentialShapeError::RotatedStatusRequiresRotatedAt);
        }

        errors
    }
}

/// §8.3: "创建、替换、撤销、账号漂移、Keychain 锁定以及卸载时'保留/删除
/// Autome 自建 item'的用户决定都生成 CredentialReceipt."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialEvent {
    Created,
    Rotated,
    Revoked,
    AccountDrifted,
    KeychainLocked,
    UninstallRetentionDecision { retained: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialReceipt {
    pub credential_ref: String,
    pub event: CredentialEvent,
    pub occurred_at: String,
    pub operator: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialReceiptError {
    UninstallRetentionDecisionRequiresOperator,
}

/// Sole constructor for `CredentialReceipt`. §8.3 calls
/// `UninstallRetentionDecision` a "用户决定" (a user decision) — that one
/// event must always carry who made it. `AccountDrifted`/`KeychainLocked`
/// are Core-observed facts, not decisions, so they do not require an
/// operator.
pub fn issue_credential_receipt(
    credential_ref: &str,
    event: CredentialEvent,
    occurred_at: &str,
    operator: Option<&str>,
) -> Result<CredentialReceipt, CredentialReceiptError> {
    if matches!(event, CredentialEvent::UninstallRetentionDecision { .. }) && operator.is_none() {
        return Err(CredentialReceiptError::UninstallRetentionDecisionRequiresOperator);
    }
    Ok(CredentialReceipt {
        credential_ref: credential_ref.to_string(),
        event,
        occurred_at: occurred_at.to_string(),
        operator: operator.map(|s| s.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_keychain_record() -> CredentialRecord {
        CredentialRecord {
            provider: "anthropic".into(),
            auth_mode: AuthMode::ApiKey,
            storage_kind: StorageKind::AutomeManagedKeychainItem,
            storage_location: StorageLocation::Keychain(KeychainItemIdentity {
                service: "com.autome.credentials".into(),
                account: "anthropic-default".into(),
            }),
            created_at: "2026-09-14T00:00:00Z".into(),
            rotated_at: None,
            revoked_at: None,
            status: CredentialStatus::Active,
        }
    }

    #[test]
    fn active_keychain_record_has_no_shape_errors() {
        assert!(active_keychain_record().validate_shape().is_empty());
    }

    #[test]
    fn keychain_kind_with_auth_file_location_is_a_mismatch() {
        let mut record = active_keychain_record();
        record.storage_location = StorageLocation::AuthFile(AuthFileIdentity {
            path: "/codex/auth.json".into(),
            file_identity_hash: "hash-1".into(),
        });
        assert_eq!(
            record.validate_shape(),
            vec![CredentialShapeError::StorageKindLocationMismatch]
        );
    }

    #[test]
    fn auth_file_kind_with_keychain_location_is_a_mismatch() {
        let mut record = active_keychain_record();
        record.storage_kind = StorageKind::ProviderOwnedAuthFile;
        assert_eq!(
            record.validate_shape(),
            vec![CredentialShapeError::StorageKindLocationMismatch]
        );
    }

    #[test]
    fn provider_owned_auth_file_matching_location_has_no_shape_errors() {
        let record = CredentialRecord {
            auth_mode: AuthMode::OAuth,
            storage_kind: StorageKind::ProviderOwnedAuthFile,
            storage_location: StorageLocation::AuthFile(AuthFileIdentity {
                path: "/codex/auth.json".into(),
                file_identity_hash: "hash-1".into(),
            }),
            ..active_keychain_record()
        };
        assert!(record.validate_shape().is_empty());
    }

    #[test]
    fn revoked_status_without_revoked_at_is_rejected() {
        let mut record = active_keychain_record();
        record.status = CredentialStatus::Revoked;
        assert_eq!(
            record.validate_shape(),
            vec![CredentialShapeError::RevokedStatusRequiresRevokedAt]
        );
    }

    #[test]
    fn revoked_status_with_revoked_at_is_accepted() {
        let mut record = active_keychain_record();
        record.status = CredentialStatus::Revoked;
        record.revoked_at = Some("2026-09-14T01:00:00Z".into());
        assert!(record.validate_shape().is_empty());
    }

    #[test]
    fn rotated_status_without_rotated_at_is_rejected() {
        let mut record = active_keychain_record();
        record.status = CredentialStatus::Rotated;
        assert_eq!(
            record.validate_shape(),
            vec![CredentialShapeError::RotatedStatusRequiresRotatedAt]
        );
    }

    #[test]
    fn multiple_shape_errors_are_all_collected() {
        let mut record = active_keychain_record();
        record.storage_kind = StorageKind::ProviderOwnedAuthFile;
        record.status = CredentialStatus::Revoked;
        assert_eq!(
            record.validate_shape(),
            vec![
                CredentialShapeError::StorageKindLocationMismatch,
                CredentialShapeError::RevokedStatusRequiresRevokedAt,
            ]
        );
    }

    #[test]
    fn created_event_does_not_require_an_operator() {
        assert!(
            issue_credential_receipt(
                "cred-1",
                CredentialEvent::Created,
                "2026-09-14T00:00:00Z",
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn account_drifted_and_keychain_locked_do_not_require_an_operator() {
        assert!(
            issue_credential_receipt(
                "cred-1",
                CredentialEvent::AccountDrifted,
                "2026-09-14T00:00:00Z",
                None
            )
            .is_ok()
        );
        assert!(
            issue_credential_receipt(
                "cred-1",
                CredentialEvent::KeychainLocked,
                "2026-09-14T00:00:00Z",
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn uninstall_retention_decision_without_operator_is_rejected() {
        let err = issue_credential_receipt(
            "cred-1",
            CredentialEvent::UninstallRetentionDecision { retained: true },
            "2026-09-14T00:00:00Z",
            None,
        )
        .unwrap_err();
        assert_eq!(
            err,
            CredentialReceiptError::UninstallRetentionDecisionRequiresOperator
        );
    }

    #[test]
    fn uninstall_retention_decision_with_operator_is_accepted() {
        let receipt = issue_credential_receipt(
            "cred-1",
            CredentialEvent::UninstallRetentionDecision { retained: false },
            "2026-09-14T00:00:00Z",
            Some("user-1"),
        )
        .unwrap();
        assert_eq!(receipt.operator, Some("user-1".to_string()));
    }
}
