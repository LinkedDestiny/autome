//! Local replanning and contract amendment per plan §6.6.
//!
//! "重规划只能提出新的 TaskGraph，不能改变 TaskContract" — `ReplanProposal`
//! therefore carries no contract-mutating field at all. Acceptance reuses
//! `graph::validate_for_freeze` for the DAG/coverage/write-scope/anchoring
//! checks it already performs on the new graph in isolation, and adds only
//! the two checks that are specific to comparing against the *old* graph:
//! requirement coverage must not regress, and no acceptance check present
//! in the old graph may vanish from the new one (a check swapped for an
//! easier, differently-ID'd one shows up here as the old id disappearing).
//!
//! "相同节点 ID 只用于可读 diff，不授权复用旧输出" is not something to
//! validate here — `GraphNode` (see `graph.rs`) has no field through which
//! a new node could claim to reuse an old node's output, so there is no
//! code path that could grant that authority in the first place.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::graph::{FreezeViolation, TaskGraph, validate_for_freeze};
use crate::requirement::{CheckId, RequirementId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplanProposal {
    pub run_id: String,
    pub trigger_evidence_refs: Vec<String>,
    pub affected_requirement_ids: Vec<RequirementId>,
    pub affected_node_ids: Vec<String>,
    pub semantically_unchanged_node_ids: Vec<String>,
    pub invalidated_attempt_ids: Vec<String>,
    pub invalidated_candidate_ids: Vec<String>,
    pub invalidated_receipt_ids: Vec<String>,
    pub old_graph_hash: String,
    pub new_graph: TaskGraph,
    pub budget_delta_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplanRejection {
    ProposalTargetsWrongOldGraph,
    RequirementCoverageDecreased { requirement: RequirementId },
    AcceptanceCheckDropped { check: CheckId },
    GraphFreezeViolation(FreezeViolation),
}

fn covered_requirement_ids(graph: &TaskGraph) -> HashSet<RequirementId> {
    graph
        .nodes
        .iter()
        .flat_map(|n| n.requirement_ids.iter().cloned())
        .collect()
}

fn covered_check_ids(graph: &TaskGraph) -> HashSet<CheckId> {
    graph
        .nodes
        .iter()
        .flat_map(|n| n.acceptance_check_ids.iter().cloned())
        .collect()
}

/// "要求覆盖率不下降；验收没有被删除或放宽；新图仍为 DAG；节点输入、依赖、
/// 写域和要求合法；失败检查没有被替换成更容易通过的检查." Collects every
/// violation rather than stopping at the first.
pub fn evaluate_replan_proposal(
    old_graph: &TaskGraph,
    proposal: &ReplanProposal,
    must_requirements: &[RequirementId],
    mandatory_checks_by_requirement: &HashMap<RequirementId, Vec<CheckId>>,
) -> Vec<ReplanRejection> {
    if proposal.old_graph_hash != old_graph.graph_hash {
        return vec![ReplanRejection::ProposalTargetsWrongOldGraph];
    }

    let mut rejections = Vec::new();

    let old_covered = covered_requirement_ids(old_graph);
    let new_covered = covered_requirement_ids(&proposal.new_graph);
    for requirement in old_covered.difference(&new_covered) {
        rejections.push(ReplanRejection::RequirementCoverageDecreased {
            requirement: requirement.clone(),
        });
    }

    let old_checks = covered_check_ids(old_graph);
    let new_checks = covered_check_ids(&proposal.new_graph);
    for check in old_checks.difference(&new_checks) {
        rejections.push(ReplanRejection::AcceptanceCheckDropped {
            check: check.clone(),
        });
    }

    for violation in validate_for_freeze(
        &proposal.new_graph,
        must_requirements,
        mandatory_checks_by_requirement,
    ) {
        rejections.push(ReplanRejection::GraphFreezeViolation(violation));
    }

    rejections
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplanAuthorizationError {
    GraphRejected(ReplanRejection),
    MissingGraphReviewReceipt,
    MissingUserApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplanAuthorization {
    pub run_id: String,
    pub new_graph_hash: String,
    pub graph_review_receipt_ref: String,
    pub user_approval_ref: String,
}

/// Sole path from a `ReplanProposal` to an authorization to actually
/// supersede the old Run. "独立 graph review 与用户批准后" — both refs are
/// required non-empty in addition to the proposal itself being accepted;
/// any missing piece fails the whole authorization (nothing partial is
/// returned), matching "缺少任一项整体回滚" from the contract-amendment
/// path this mirrors.
pub fn authorize_replan(
    old_graph: &TaskGraph,
    proposal: &ReplanProposal,
    must_requirements: &[RequirementId],
    mandatory_checks_by_requirement: &HashMap<RequirementId, Vec<CheckId>>,
    graph_review_receipt_ref: Option<&str>,
    user_approval_ref: Option<&str>,
) -> Result<ReplanAuthorization, Vec<ReplanAuthorizationError>> {
    let mut errors: Vec<ReplanAuthorizationError> = evaluate_replan_proposal(
        old_graph,
        proposal,
        must_requirements,
        mandatory_checks_by_requirement,
    )
    .into_iter()
    .map(ReplanAuthorizationError::GraphRejected)
    .collect();

    let graph_review_receipt_ref = match graph_review_receipt_ref {
        Some(r) if !r.is_empty() => Some(r.to_string()),
        _ => {
            errors.push(ReplanAuthorizationError::MissingGraphReviewReceipt);
            None
        }
    };
    let user_approval_ref = match user_approval_ref {
        Some(r) if !r.is_empty() => Some(r.to_string()),
        _ => {
            errors.push(ReplanAuthorizationError::MissingUserApproval);
            None
        }
    };

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ReplanAuthorization {
        run_id: proposal.run_id.clone(),
        new_graph_hash: proposal.new_graph.graph_hash.clone(),
        graph_review_receipt_ref: graph_review_receipt_ref.unwrap(),
        user_approval_ref: user_approval_ref.unwrap(),
    })
}

/// "如果必须改变已冻结 TaskContract，则不能走 Replanning" — a distinct
/// kind from `ReplanProposal`, carrying a replacement contract instead of
/// only a replacement graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmendmentReviewKind {
    Planning,
    Contract,
    Graph,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractAmendmentProposal {
    pub triggering_run_id: String,
    pub preallocated_new_run_id: String,
    pub base_contract_version: u32,
    pub new_contract_ref: String,
    pub new_graph_ref: String,
    pub new_execution_run_spec_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractAmendmentAuthorizationError {
    EmptyContractRef,
    EmptyGraphRef,
    EmptyExecutionRunSpecRef,
    MissingRequiredReviewReceipt(AmendmentReviewKind),
    MissingUserApproval,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractAmendmentAuthorization {
    pub triggering_run_id: String,
    pub new_run_id: String,
    pub review_receipt_refs: HashMap<AmendmentReviewKind, String>,
    pub user_approval_ref: String,
}

/// Sole constructor. "批准事务同时为预先分配的新 run ID 写入当前配置要求
/// 的 planning/contract/graph HumanReviewReceipt、旧 Run
/// terminal=Superseded、新 contract/graph/ExecutionRunSpec 与新
/// Run；缺少任一项整体回滚" — `required_review_kinds` is whatever the
/// caller has already resolved from the current `ResolvedProjectConfig`
/// (plan §5.1); this function does not re-derive which kinds are required,
/// it only enforces that every required kind has a non-empty receipt
/// before returning anything at all.
pub fn authorize_contract_amendment(
    proposal: &ContractAmendmentProposal,
    required_review_kinds: &[AmendmentReviewKind],
    provided_review_receipts: &HashMap<AmendmentReviewKind, String>,
    user_approval_ref: Option<&str>,
) -> Result<ContractAmendmentAuthorization, Vec<ContractAmendmentAuthorizationError>> {
    let mut errors = Vec::new();

    if proposal.new_contract_ref.is_empty() {
        errors.push(ContractAmendmentAuthorizationError::EmptyContractRef);
    }
    if proposal.new_graph_ref.is_empty() {
        errors.push(ContractAmendmentAuthorizationError::EmptyGraphRef);
    }
    if proposal.new_execution_run_spec_ref.is_empty() {
        errors.push(ContractAmendmentAuthorizationError::EmptyExecutionRunSpecRef);
    }

    for kind in required_review_kinds {
        match provided_review_receipts.get(kind) {
            Some(r) if !r.is_empty() => {}
            _ => errors
                .push(ContractAmendmentAuthorizationError::MissingRequiredReviewReceipt(*kind)),
        }
    }

    let user_approval_ref = match user_approval_ref {
        Some(r) if !r.is_empty() => Some(r.to_string()),
        _ => {
            errors.push(ContractAmendmentAuthorizationError::MissingUserApproval);
            None
        }
    };

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(ContractAmendmentAuthorization {
        triggering_run_id: proposal.triggering_run_id.clone(),
        new_run_id: proposal.preallocated_new_run_id.clone(),
        review_receipt_refs: provided_review_receipts.clone(),
        user_approval_ref: user_approval_ref.unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphNode, NodeId, NodePurpose, RiskLevel};

    fn node(id: &str, requirement_ids: Vec<&str>, check_ids: Vec<&str>) -> GraphNode {
        GraphNode {
            id: NodeId(id.into()),
            kind: "generic".into(),
            purpose: NodePurpose::Business,
            title: id.into(),
            requirement_ids: requirement_ids
                .into_iter()
                .map(|r| RequirementId(r.into()))
                .collect(),
            acceptance_check_ids: check_ids.into_iter().map(|c| CheckId(c.into())).collect(),
            depends_on: vec![],
            expected_outputs: vec![],
            write_scope: vec![],
            risk_level: RiskLevel::Low,
            estimated_budget: 1,
        }
    }

    fn graph(hash: &str, nodes: Vec<GraphNode>) -> TaskGraph {
        TaskGraph {
            id: "G-1".into(),
            version: 1,
            graph_hash: hash.into(),
            contract_ref: "C-1".into(),
            nodes,
        }
    }

    fn base_proposal(old_hash: &str, new_graph: TaskGraph) -> ReplanProposal {
        ReplanProposal {
            run_id: "run-1".into(),
            trigger_evidence_refs: vec!["evidence-1".into()],
            affected_requirement_ids: vec![],
            affected_node_ids: vec![],
            semantically_unchanged_node_ids: vec![],
            invalidated_attempt_ids: vec!["attempt-1".into()],
            invalidated_candidate_ids: vec![],
            invalidated_receipt_ids: vec![],
            old_graph_hash: old_hash.into(),
            new_graph,
            budget_delta_ref: "budget-delta-1".into(),
        }
    }

    #[test]
    fn a_replan_with_equal_coverage_and_no_freeze_violations_is_accepted() {
        let old = graph("hash-old", vec![node("A", vec!["R1"], vec!["chk-1"])]);
        let new_graph = graph("hash-new", vec![node("A", vec!["R1"], vec!["chk-1"])]);
        let proposal = base_proposal("hash-old", new_graph);
        let must = vec![RequirementId("R1".into())];
        let mut checks = HashMap::new();
        checks.insert(RequirementId("R1".into()), vec![CheckId("chk-1".into())]);
        assert!(evaluate_replan_proposal(&old, &proposal, &must, &checks).is_empty());
    }

    #[test]
    fn a_proposal_targeting_a_stale_old_graph_hash_is_rejected_immediately() {
        let old = graph("hash-old", vec![]);
        let new_graph = graph("hash-new", vec![]);
        let proposal = base_proposal("hash-stale", new_graph);
        let rejections = evaluate_replan_proposal(&old, &proposal, &[], &HashMap::new());
        assert_eq!(
            rejections,
            vec![ReplanRejection::ProposalTargetsWrongOldGraph]
        );
    }

    #[test]
    fn dropping_requirement_coverage_is_rejected() {
        let old = graph("hash-old", vec![node("A", vec!["R1", "R2"], vec![])]);
        let new_graph = graph("hash-new", vec![node("A", vec!["R1"], vec![])]);
        let proposal = base_proposal("hash-old", new_graph);
        let rejections = evaluate_replan_proposal(&old, &proposal, &[], &HashMap::new());
        assert!(
            rejections.contains(&ReplanRejection::RequirementCoverageDecreased {
                requirement: RequirementId("R2".into())
            })
        );
    }

    #[test]
    fn dropping_an_acceptance_check_is_rejected() {
        let old = graph("hash-old", vec![node("A", vec![], vec!["chk-1", "chk-2"])]);
        let new_graph = graph("hash-new", vec![node("A", vec![], vec!["chk-1"])]);
        let proposal = base_proposal("hash-old", new_graph);
        let rejections = evaluate_replan_proposal(&old, &proposal, &[], &HashMap::new());
        assert!(
            rejections.contains(&ReplanRejection::AcceptanceCheckDropped {
                check: CheckId("chk-2".into())
            })
        );
    }

    #[test]
    fn a_cyclic_new_graph_surfaces_the_underlying_freeze_violation() {
        let old = graph("hash-old", vec![]);
        let mut a = node("A", vec![], vec![]);
        let mut b = node("B", vec![], vec![]);
        a.depends_on = vec![NodeId("B".into())];
        b.depends_on = vec![NodeId("A".into())];
        let new_graph = graph("hash-new", vec![a, b]);
        let proposal = base_proposal("hash-old", new_graph);
        let rejections = evaluate_replan_proposal(&old, &proposal, &[], &HashMap::new());
        assert!(rejections.iter().any(|r| matches!(
            r,
            ReplanRejection::GraphFreezeViolation(FreezeViolation::Cycle { .. })
        )));
    }

    #[test]
    fn authorize_replan_succeeds_with_both_receipts_present() {
        let old = graph("hash-old", vec![node("A", vec!["R1"], vec![])]);
        let new_graph = graph("hash-new", vec![node("A", vec!["R1"], vec![])]);
        let proposal = base_proposal("hash-old", new_graph);
        let authorization = authorize_replan(
            &old,
            &proposal,
            &[],
            &HashMap::new(),
            Some("graph-review-1"),
            Some("user-approval-1"),
        )
        .unwrap();
        assert_eq!(authorization.new_graph_hash, "hash-new");
    }

    #[test]
    fn authorize_replan_fails_without_graph_review_receipt() {
        let old = graph("hash-old", vec![]);
        let new_graph = graph("hash-new", vec![]);
        let proposal = base_proposal("hash-old", new_graph);
        let errors = authorize_replan(
            &old,
            &proposal,
            &[],
            &HashMap::new(),
            None,
            Some("user-approval-1"),
        )
        .unwrap_err();
        assert!(errors.contains(&ReplanAuthorizationError::MissingGraphReviewReceipt));
    }

    #[test]
    fn authorize_replan_collects_rejected_graph_and_missing_approvals_together() {
        let old = graph("hash-old", vec![node("A", vec!["R1"], vec![])]);
        let new_graph = graph("hash-new", vec![node("A", vec![], vec![])]);
        let proposal = base_proposal("hash-old", new_graph);
        let errors =
            authorize_replan(&old, &proposal, &[], &HashMap::new(), None, None).unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ReplanAuthorizationError::GraphRejected(
                ReplanRejection::RequirementCoverageDecreased { .. }
            )
        )));
        assert!(errors.contains(&ReplanAuthorizationError::MissingGraphReviewReceipt));
        assert!(errors.contains(&ReplanAuthorizationError::MissingUserApproval));
    }

    fn amendment_proposal() -> ContractAmendmentProposal {
        ContractAmendmentProposal {
            triggering_run_id: "run-1".into(),
            preallocated_new_run_id: "run-2".into(),
            base_contract_version: 1,
            new_contract_ref: "contract-2".into(),
            new_graph_ref: "graph-2".into(),
            new_execution_run_spec_ref: "spec-2".into(),
        }
    }

    #[test]
    fn contract_amendment_is_authorized_when_every_required_receipt_and_approval_is_present() {
        let mut receipts = HashMap::new();
        receipts.insert(
            AmendmentReviewKind::Planning,
            "planning-receipt".to_string(),
        );
        receipts.insert(
            AmendmentReviewKind::Contract,
            "contract-receipt".to_string(),
        );
        receipts.insert(AmendmentReviewKind::Graph, "graph-receipt".to_string());
        let required = [
            AmendmentReviewKind::Planning,
            AmendmentReviewKind::Contract,
            AmendmentReviewKind::Graph,
        ];
        let authorization = authorize_contract_amendment(
            &amendment_proposal(),
            &required,
            &receipts,
            Some("user-approval-1"),
        )
        .unwrap();
        assert_eq!(authorization.new_run_id, "run-2");
    }

    #[test]
    fn contract_amendment_is_rejected_wholesale_when_one_required_receipt_is_missing() {
        let mut receipts = HashMap::new();
        receipts.insert(
            AmendmentReviewKind::Planning,
            "planning-receipt".to_string(),
        );
        receipts.insert(
            AmendmentReviewKind::Contract,
            "contract-receipt".to_string(),
        );
        let required = [
            AmendmentReviewKind::Planning,
            AmendmentReviewKind::Contract,
            AmendmentReviewKind::Graph,
        ];
        let errors = authorize_contract_amendment(
            &amendment_proposal(),
            &required,
            &receipts,
            Some("user-approval-1"),
        )
        .unwrap_err();
        assert_eq!(
            errors,
            vec![
                ContractAmendmentAuthorizationError::MissingRequiredReviewReceipt(
                    AmendmentReviewKind::Graph
                )
            ]
        );
    }

    #[test]
    fn contract_amendment_collects_empty_ref_and_missing_approval_errors_together() {
        let mut proposal = amendment_proposal();
        proposal.new_contract_ref = String::new();
        let errors =
            authorize_contract_amendment(&proposal, &[], &HashMap::new(), None).unwrap_err();
        assert!(errors.contains(&ContractAmendmentAuthorizationError::EmptyContractRef));
        assert!(errors.contains(&ContractAmendmentAuthorizationError::MissingUserApproval));
    }
}
