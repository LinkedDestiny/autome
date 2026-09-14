//! §8.2 Capability broker.
//!
//! This module intentionally does not attempt to model the brokers
//! themselves (Git/Artifact, Project, Environment, Skill) — each already
//! has, or will have, its own real request/receipt shapes elsewhere
//! (`delivery.rs` for Git/Artifact; Environment/Skill transaction types
//! are explicitly deferred system-integration work per `readiness.rs` and
//! `skill.rs`'s module docs). What belongs here, and nowhere else, are the
//! two crate-independent rules from §8.2's opening paragraphs: which
//! capabilities may never be exposed through a generic sandbox shell, and
//! which action kinds may only ever be *proposed* by an Agent, never
//! caused by one directly.

use serde::{Deserialize, Serialize};

/// §8.2: "以下能力不能通过任意 sandbox shell 获得" — the exhaustive list.
/// Nothing in this crate ever grants one of these directly; they only
/// exist as a fact an authorization layer elsewhere can check against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrivilegedCapability {
    MergeOrPushOrForcePush,
    CreateOrArchiveProjectOrChangeConfig,
    InstallUpgradeMigrateOrUninstallSoftwareOrSkill,
    PublishDeployExternalWriteOrDatabaseMigration,
    ReadLongLivedCredential,
    ProducePaidExternalSideEffect,
}

/// §8.2: "首版将能力拆成 Git/Artifact、Project、Environment 和 Skill 四个
/// Rust broker."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityBrokerKind {
    GitArtifact,
    Project,
    Environment,
    Skill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrokerActionOrigin {
    AgentProposal,
    UserInitiatedFromUi,
}

/// §8.2: "Agent 只能提交 proposal；Project 初始化、配置应用、
/// Environment/Skill 事务和交付必须由用户从 UI 发起." Five action kinds,
/// exhaustively — an Agent-originated request for any of them is always
/// illegal, regardless of which broker would ultimately execute it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UserInitiatedOnlyActionKind {
    ProjectInitialization,
    ConfigApplication,
    EnvironmentTransaction,
    SkillTransaction,
    Delivery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrokerActionError {
    RequiresUserInitiationNotAgentProposal(UserInitiatedOnlyActionKind),
}

/// Sole gate for these five action kinds: an `AgentProposal` origin is
/// always rejected, no matter which broker or capability is involved.
pub fn validate_broker_action_origin(
    action: UserInitiatedOnlyActionKind,
    origin: BrokerActionOrigin,
) -> Result<(), BrokerActionError> {
    if matches!(origin, BrokerActionOrigin::AgentProposal) {
        return Err(BrokerActionError::RequiresUserInitiationNotAgentProposal(
            action,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_ACTIONS: [UserInitiatedOnlyActionKind; 5] = [
        UserInitiatedOnlyActionKind::ProjectInitialization,
        UserInitiatedOnlyActionKind::ConfigApplication,
        UserInitiatedOnlyActionKind::EnvironmentTransaction,
        UserInitiatedOnlyActionKind::SkillTransaction,
        UserInitiatedOnlyActionKind::Delivery,
    ];

    #[test]
    fn every_user_initiated_only_action_rejects_agent_proposal_origin() {
        for action in ALL_ACTIONS {
            let err = validate_broker_action_origin(action, BrokerActionOrigin::AgentProposal)
                .unwrap_err();
            assert_eq!(
                err,
                BrokerActionError::RequiresUserInitiationNotAgentProposal(action)
            );
        }
    }

    #[test]
    fn every_user_initiated_only_action_accepts_user_initiated_origin() {
        for action in ALL_ACTIONS {
            assert!(
                validate_broker_action_origin(action, BrokerActionOrigin::UserInitiatedFromUi)
                    .is_ok()
            );
        }
    }
}
