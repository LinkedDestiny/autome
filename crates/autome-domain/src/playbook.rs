//! §10.3 Prompt 与 Playbook.
//!
//! Three mechanically checkable rules from the prose:
//!
//! 1. "首版只有 `greenfield-product` 和 `existing-repo-change` 两个
//!    playbook" — `PlaybookId` is a closed two-variant enum, not a free
//!    string, matching the `ExistingRepo`/`Greenfield` split vocabulary
//!    already used by `delivery.rs`/`readiness.rs`.
//! 2. "Playbook 由 manifest + Markdown role prompts + references 组成，
//!    运行时固定内容哈希" — once a `FrozenPlaybook` is bound to a Run its
//!    content hash must never drift; `is_current_against` mirrors the same
//!    staleness pattern already used by `evidence.rs`/`readiness.rs`/
//!    `review.rs`/`delivery.rs`.
//! 3. "所有 role 的状态性输出使用 JSON Schema 或 Autome dynamic tools；
//!    自由文本只能作为叙述，不能决定状态" — `RoleOutput` is a closed enum
//!    of `Structured` vs `Narrative`, and only `Structured` exposes a
//!    state-decision payload; `Narrative` has no accessor that could ever
//!    produce one. Same "no escape-hatch accessor" technique already used
//!    for `AgentClaim` (`attempt.rs`) and `AttemptOutcome::AttemptFailed`.
//!
//! Deliberately out of scope: rendering manifest+markdown+references into
//! an actual prompt, and validating a `Structured` payload's JSON against
//! its declared JSON Schema — both need real file I/O / a JSON Schema
//! validator this crate does not have.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybookId {
    GreenfieldProduct,
    ExistingRepoChange,
}

impl PlaybookId {
    pub const ALL: [PlaybookId; 2] = [
        PlaybookId::GreenfieldProduct,
        PlaybookId::ExistingRepoChange,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenPlaybook {
    pub playbook_id: PlaybookId,
    pub manifest_content_hash: String,
}

impl FrozenPlaybook {
    /// §10.3: "运行时固定内容哈希" — a playbook bound to a Run is only
    /// still current if both its identity and its content hash are
    /// unchanged from what was frozen at bind time.
    pub fn is_current_against(&self, current: &FrozenPlaybook) -> bool {
        self.playbook_id == current.playbook_id
            && self.manifest_content_hash == current.manifest_content_hash
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RoleOutput {
    Structured {
        schema_id: String,
        payload: serde_json::Value,
    },
    Narrative(String),
}

impl RoleOutput {
    /// §10.3: "自由文本只能作为叙述，不能决定状态" — `Narrative` has no
    /// path to a decision payload; only `Structured` does.
    pub fn structured_payload(&self) -> Option<&serde_json::Value> {
        match self {
            RoleOutput::Structured { payload, .. } => Some(payload),
            RoleOutput::Narrative(_) => None,
        }
    }

    pub fn may_decide_state(&self) -> bool {
        self.structured_payload().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_two_playbooks_exist_in_the_first_version() {
        assert_eq!(PlaybookId::ALL.len(), 2);
        assert!(PlaybookId::ALL.contains(&PlaybookId::GreenfieldProduct));
        assert!(PlaybookId::ALL.contains(&PlaybookId::ExistingRepoChange));
    }

    fn playbook(id: PlaybookId, hash: &str) -> FrozenPlaybook {
        FrozenPlaybook {
            playbook_id: id,
            manifest_content_hash: hash.to_string(),
        }
    }

    #[test]
    fn identical_playbook_is_current() {
        let frozen = playbook(PlaybookId::ExistingRepoChange, "hash-1");
        assert!(frozen.is_current_against(&playbook(PlaybookId::ExistingRepoChange, "hash-1")));
    }

    #[test]
    fn drifted_content_hash_is_not_current() {
        let frozen = playbook(PlaybookId::ExistingRepoChange, "hash-1");
        assert!(!frozen.is_current_against(&playbook(PlaybookId::ExistingRepoChange, "hash-2")));
    }

    #[test]
    fn different_playbook_id_is_not_current() {
        let frozen = playbook(PlaybookId::ExistingRepoChange, "hash-1");
        assert!(!frozen.is_current_against(&playbook(PlaybookId::GreenfieldProduct, "hash-1")));
    }

    #[test]
    fn structured_output_exposes_its_payload_and_may_decide_state() {
        let output = RoleOutput::Structured {
            schema_id: "contract_drafting.v1".into(),
            payload: serde_json::json!({ "requirements": [] }),
        };
        assert!(output.may_decide_state());
        assert_eq!(
            output.structured_payload(),
            Some(&serde_json::json!({ "requirements": [] }))
        );
    }

    #[test]
    fn narrative_output_has_no_payload_and_may_not_decide_state() {
        let output = RoleOutput::Narrative("I think this looks about right.".into());
        assert!(!output.may_decide_state());
        assert_eq!(output.structured_payload(), None);
    }
}
