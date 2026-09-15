//! TaskGraph per plan §5.5. The graph is an implementation strategy, not a
//! requirements authority ("TaskGraph 是可修改的实施策略，不是需求权威"): it
//! must satisfy Requirement coverage, but Requirements never point back into
//! specific graph shapes.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::requirement::{CheckId, RequirementId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Plan §5.5 only names one structural distinction that the freeze gate
/// depends on ("纯基础设施节点必须指向被解锁的业务节点" — a pure
/// infrastructure node must unlock a business node): whether a node itself
/// carries any Requirement. Everything else about "kind" (build/test/repair/
/// etc.) is a label, not a freeze-gate input, so it stays a plain String on
/// GraphNode; this enum captures only the part the gate must reason about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodePurpose {
    Business,
    Infrastructure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub kind: String,
    pub purpose: NodePurpose,
    pub title: String,
    pub requirement_ids: Vec<RequirementId>,
    pub acceptance_check_ids: Vec<CheckId>,
    pub depends_on: Vec<NodeId>,
    pub expected_outputs: Vec<String>,
    pub write_scope: Vec<String>,
    pub risk_level: RiskLevel,
    /// Placeholder unit until the Budget model (turns/tokens/$/wall-clock,
    /// plan §6.7) lands; kept as an opaque non-zero magnitude so freeze
    /// validation can at least reject an unset budget.
    pub estimated_budget: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGraph {
    pub id: String,
    pub version: u32,
    pub graph_hash: String,
    pub contract_ref: String,
    pub nodes: Vec<GraphNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FreezeViolation {
    DanglingDependency {
        node: NodeId,
        missing_dependency: NodeId,
    },
    Cycle {
        involved: Vec<NodeId>,
    },
    UnmappedMustRequirement {
        requirement: RequirementId,
    },
    MustRequirementWithoutMandatoryCheck {
        requirement: RequirementId,
    },
    OverlappingWriteScopeNotSerialized {
        a: NodeId,
        b: NodeId,
        path: String,
    },
    UnanchoredInfrastructureNode {
        node: NodeId,
    },
    ZeroBudgetNode {
        node: NodeId,
    },
}

/// Validates the four freeze-gate conditions from plan §5.5:
/// "图无环；所有 must Requirement 都映射到至少一个节点与 mandatory Check；
/// 节点说明服务的要求；重叠写域必须串行；纯基础设施节点必须指向被解锁的业务
/// 节点。"
///
/// `must_requirements` and `mandatory_checks_by_requirement` come from the
/// frozen TaskContract (plan §5.3/§5.4) — this function does not itself
/// decide necessity or mandatoriness, it only checks the graph against them.
pub fn validate_for_freeze(
    graph: &TaskGraph,
    must_requirements: &[RequirementId],
    mandatory_checks_by_requirement: &HashMap<RequirementId, Vec<CheckId>>,
) -> Vec<FreezeViolation> {
    let mut violations = Vec::new();
    let node_ids: HashSet<&NodeId> = graph.nodes.iter().map(|n| &n.id).collect();

    for node in &graph.nodes {
        for dep in &node.depends_on {
            if !node_ids.contains(dep) {
                violations.push(FreezeViolation::DanglingDependency {
                    node: node.id.clone(),
                    missing_dependency: dep.clone(),
                });
            }
        }
        if node.estimated_budget == 0 {
            violations.push(FreezeViolation::ZeroBudgetNode {
                node: node.id.clone(),
            });
        }
    }

    if let Some(cycle) = find_cycle(graph) {
        violations.push(FreezeViolation::Cycle { involved: cycle });
        // A cyclic graph makes reachability analysis below meaningless;
        // report the cycle alone rather than compounding it with derived
        // false positives.
        return violations;
    }

    let mut requirement_to_nodes: HashMap<&RequirementId, Vec<&NodeId>> = HashMap::new();
    for node in &graph.nodes {
        for req in &node.requirement_ids {
            requirement_to_nodes.entry(req).or_default().push(&node.id);
        }
    }

    for req in must_requirements {
        let covering_nodes = requirement_to_nodes.get(req);
        match covering_nodes {
            None => violations.push(FreezeViolation::UnmappedMustRequirement {
                requirement: req.clone(),
            }),
            Some(nodes) => {
                let mandatory_checks = mandatory_checks_by_requirement
                    .get(req)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                let covered = mandatory_checks.iter().all(|check| {
                    nodes.iter().any(|node_id| {
                        graph
                            .nodes
                            .iter()
                            .find(|n| &n.id == *node_id)
                            .is_some_and(|n| n.acceptance_check_ids.contains(check))
                    })
                });
                if mandatory_checks.is_empty() || !covered {
                    violations.push(FreezeViolation::MustRequirementWithoutMandatoryCheck {
                        requirement: req.clone(),
                    });
                }
            }
        }
    }

    let reachable = transitive_reachability(graph);
    for (a_idx, a) in graph.nodes.iter().enumerate() {
        for b in graph.nodes.iter().skip(a_idx + 1) {
            for path in &a.write_scope {
                if b.write_scope.contains(path) {
                    let ordered = reachable.get(&a.id).is_some_and(|r| r.contains(&b.id))
                        || reachable.get(&b.id).is_some_and(|r| r.contains(&a.id));
                    if !ordered {
                        violations.push(FreezeViolation::OverlappingWriteScopeNotSerialized {
                            a: a.id.clone(),
                            b: b.id.clone(),
                            path: path.clone(),
                        });
                    }
                }
            }
        }
    }

    let dependents = dependents_index(graph);
    for node in &graph.nodes {
        if node.purpose != NodePurpose::Infrastructure {
            continue;
        }
        let unlocks_business = reachable_downstream_includes_business(node, graph, &dependents);
        if !unlocks_business {
            violations.push(FreezeViolation::UnanchoredInfrastructureNode {
                node: node.id.clone(),
            });
        }
    }

    violations
}

fn find_cycle(graph: &TaskGraph) -> Option<Vec<NodeId>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Unvisited,
        InProgress,
        Done,
    }

    let mut marks: HashMap<&NodeId, Mark> = graph
        .nodes
        .iter()
        .map(|n| (&n.id, Mark::Unvisited))
        .collect();
    let by_id: HashMap<&NodeId, &GraphNode> = graph.nodes.iter().map(|n| (&n.id, n)).collect();

    fn visit<'a>(
        node_id: &'a NodeId,
        by_id: &HashMap<&'a NodeId, &'a GraphNode>,
        marks: &mut HashMap<&'a NodeId, Mark>,
        stack: &mut Vec<NodeId>,
    ) -> Option<Vec<NodeId>> {
        match marks.get(node_id) {
            Some(Mark::Done) => return None,
            Some(Mark::InProgress) => {
                let start = stack.iter().position(|n| n == node_id).unwrap_or(0);
                return Some(stack[start..].to_vec());
            }
            _ => {}
        }
        marks.insert(node_id, Mark::InProgress);
        stack.push(node_id.clone());
        if let Some(node) = by_id.get(node_id) {
            for dep in &node.depends_on {
                if by_id.contains_key(dep)
                    && let Some(cycle) = visit(dep, by_id, marks, stack)
                {
                    return Some(cycle);
                }
            }
        }
        stack.pop();
        marks.insert(node_id, Mark::Done);
        None
    }

    let mut stack = Vec::new();
    for node in &graph.nodes {
        if let Some(cycle) = visit(&node.id, &by_id, &mut marks, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

/// For each node, the set of nodes reachable by following `depends_on`
/// (i.e. the node's transitive prerequisites — things that must run before
/// it). Used both directions by the write-scope check.
fn transitive_reachability(graph: &TaskGraph) -> HashMap<NodeId, HashSet<NodeId>> {
    let by_id: HashMap<&NodeId, &GraphNode> = graph.nodes.iter().map(|n| (&n.id, n)).collect();
    let mut memo: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    fn resolve(
        node_id: &NodeId,
        by_id: &HashMap<&NodeId, &GraphNode>,
        memo: &mut HashMap<NodeId, HashSet<NodeId>>,
    ) -> HashSet<NodeId> {
        if let Some(cached) = memo.get(node_id) {
            return cached.clone();
        }
        let mut result = HashSet::new();
        if let Some(node) = by_id.get(node_id) {
            for dep in &node.depends_on {
                result.insert(dep.clone());
                let transitive = resolve(dep, by_id, memo);
                result.extend(transitive);
            }
        }
        memo.insert(node_id.clone(), result.clone());
        result
    }

    for node in &graph.nodes {
        resolve(&node.id, &by_id, &mut memo);
    }
    memo
}

fn dependents_index(graph: &TaskGraph) -> HashMap<NodeId, Vec<NodeId>> {
    let mut dependents: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for node in &graph.nodes {
        for dep in &node.depends_on {
            dependents
                .entry(dep.clone())
                .or_default()
                .push(node.id.clone());
        }
    }
    dependents
}

fn reachable_downstream_includes_business(
    start: &GraphNode,
    graph: &TaskGraph,
    dependents: &HashMap<NodeId, Vec<NodeId>>,
) -> bool {
    let by_id: HashMap<&NodeId, &GraphNode> = graph.nodes.iter().map(|n| (&n.id, n)).collect();
    let mut seen: HashSet<NodeId> = HashSet::new();
    let mut queue: Vec<NodeId> = dependents.get(&start.id).cloned().unwrap_or_default();
    while let Some(next) = queue.pop() {
        if !seen.insert(next.clone()) {
            continue;
        }
        if let Some(node) = by_id.get(&next)
            && node.purpose == NodePurpose::Business
            && !node.requirement_ids.is_empty()
        {
            return true;
        }
        if let Some(more) = dependents.get(&next) {
            queue.extend(more.clone());
        }
    }
    false
}

/// Every event this module's event-sourced TaskGraph aggregate can replay,
/// mirroring `contract::ContractEvent`/`contract::apply`. Unlike
/// `TaskContract`, `TaskGraph` has no `status` field at all — there is no
/// `Draft`/`Frozen` distinction stored on the graph itself; freeze-gate
/// approval is tracked externally as a `review::GraphReviewReceipt` keyed
/// by `graph_hash`; `validate_for_freeze` is a stateless predicate that
/// takes the must-requirement set and mandatory-check map as external
/// inputs it cannot look up from `TaskGraph` alone. So this event enum
/// does not model a `Frozen` transition — there is no such state to
/// transition into. `Created` is the one-shot initial draft, matching
/// every existing test's direct struct-literal construction. `Replaced` is
/// the event-sourced counterpart of a replan: plan §6.6 says "重规划只能提出
/// 新的 TaskGraph，不能改变 TaskContract", so `Replaced` only carries the new
/// `graph_hash`/`nodes` — `id` and `contract_ref` are immutable after
/// `Created` and cannot be changed by a replan event. Validating that a
/// `Replaced` event's new graph is actually legal (DAG, coverage, replan
/// coverage-does-not-shrink, ...) is `replan::evaluate_replan_proposal`'s
/// job, run by the caller *before* emitting this event — `apply` does not
/// re-run that check, exactly as `run::apply` never re-validates an
/// `origin` that the caller already decided was legal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphEvent {
    Created {
        id: String,
        contract_ref: String,
        graph_hash: String,
        nodes: Vec<GraphNode>,
    },
    Replaced {
        graph_hash: String,
        nodes: Vec<GraphNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEventError {
    AlreadyCreated,
    NotYetCreated,
}

/// `state = None` means the aggregate has never been created, the same
/// convention `task::apply`/`contract::apply` use. Matches on `event` first
/// (not on `(state, event)` jointly) so adding a GraphEvent variant without
/// handling it here is still a compile error.
pub fn apply(state: Option<TaskGraph>, event: GraphEvent) -> Result<TaskGraph, GraphEventError> {
    match event {
        GraphEvent::Created {
            id,
            contract_ref,
            graph_hash,
            nodes,
        } => {
            if state.is_some() {
                return Err(GraphEventError::AlreadyCreated);
            }
            Ok(TaskGraph {
                id,
                version: 1,
                graph_hash,
                contract_ref,
                nodes,
            })
        }
        GraphEvent::Replaced { graph_hash, nodes } => {
            let mut graph = state.ok_or(GraphEventError::NotYetCreated)?;
            graph.version += 1;
            graph.graph_hash = graph_hash;
            graph.nodes = nodes;
            Ok(graph)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, purpose: NodePurpose, requirement_ids: Vec<&str>) -> GraphNode {
        GraphNode {
            id: NodeId(id.into()),
            kind: "generic".into(),
            purpose,
            title: id.into(),
            requirement_ids: requirement_ids
                .into_iter()
                .map(|r| RequirementId(r.into()))
                .collect(),
            acceptance_check_ids: vec![],
            depends_on: vec![],
            expected_outputs: vec![],
            write_scope: vec![],
            risk_level: RiskLevel::Low,
            estimated_budget: 1,
        }
    }

    fn graph(nodes: Vec<GraphNode>) -> TaskGraph {
        TaskGraph {
            id: "G-1".into(),
            version: 1,
            graph_hash: "hash".into(),
            contract_ref: "C-1".into(),
            nodes,
        }
    }

    #[test]
    fn empty_graph_with_no_must_requirements_is_clean() {
        let g = graph(vec![]);
        assert!(validate_for_freeze(&g, &[], &HashMap::new()).is_empty());
    }

    #[test]
    fn dangling_dependency_is_reported() {
        let mut a = node("A", NodePurpose::Business, vec![]);
        a.depends_on = vec![NodeId("does-not-exist".into())];
        let g = graph(vec![a]);
        let violations = validate_for_freeze(&g, &[], &HashMap::new());
        assert!(matches!(
            violations[0],
            FreezeViolation::DanglingDependency { .. }
        ));
    }

    #[test]
    fn direct_cycle_is_detected() {
        let mut a = node("A", NodePurpose::Business, vec![]);
        let mut b = node("B", NodePurpose::Business, vec![]);
        a.depends_on = vec![NodeId("B".into())];
        b.depends_on = vec![NodeId("A".into())];
        let g = graph(vec![a, b]);
        let violations = validate_for_freeze(&g, &[], &HashMap::new());
        assert!(
            violations
                .iter()
                .any(|v| matches!(v, FreezeViolation::Cycle { .. }))
        );
    }

    #[test]
    fn must_requirement_with_no_covering_node_is_unmapped() {
        let g = graph(vec![node("A", NodePurpose::Business, vec![])]);
        let must = vec![RequirementId("R-001".into())];
        let violations = validate_for_freeze(&g, &must, &HashMap::new());
        assert_eq!(
            violations,
            vec![FreezeViolation::UnmappedMustRequirement {
                requirement: RequirementId("R-001".into())
            }]
        );
    }

    #[test]
    fn must_requirement_covered_by_node_but_missing_mandatory_check_is_flagged() {
        let g = graph(vec![node("A", NodePurpose::Business, vec!["R-001"])]);
        let must = vec![RequirementId("R-001".into())];
        let mut mandatory = HashMap::new();
        mandatory.insert(RequirementId("R-001".into()), vec![CheckId("C-001".into())]);
        let violations = validate_for_freeze(&g, &must, &mandatory);
        assert_eq!(
            violations,
            vec![FreezeViolation::MustRequirementWithoutMandatoryCheck {
                requirement: RequirementId("R-001".into())
            }]
        );
    }

    #[test]
    fn must_requirement_fully_covered_passes() {
        let mut a = node("A", NodePurpose::Business, vec!["R-001"]);
        a.acceptance_check_ids = vec![CheckId("C-001".into())];
        let g = graph(vec![a]);
        let must = vec![RequirementId("R-001".into())];
        let mut mandatory = HashMap::new();
        mandatory.insert(RequirementId("R-001".into()), vec![CheckId("C-001".into())]);
        assert!(validate_for_freeze(&g, &must, &mandatory).is_empty());
    }

    #[test]
    fn overlapping_write_scope_without_ordering_is_rejected() {
        let mut a = node("A", NodePurpose::Business, vec![]);
        let mut b = node("B", NodePurpose::Business, vec![]);
        a.write_scope = vec!["src/lib.rs".into()];
        b.write_scope = vec!["src/lib.rs".into()];
        let g = graph(vec![a, b]);
        let violations = validate_for_freeze(&g, &[], &HashMap::new());
        assert!(violations.iter().any(|v| matches!(
            v,
            FreezeViolation::OverlappingWriteScopeNotSerialized { .. }
        )));
    }

    #[test]
    fn overlapping_write_scope_with_dependency_ordering_is_allowed() {
        let a = node("A", NodePurpose::Business, vec![]);
        let mut b = node("B", NodePurpose::Business, vec![]);
        let mut a = a;
        a.write_scope = vec!["src/lib.rs".into()];
        b.write_scope = vec!["src/lib.rs".into()];
        b.depends_on = vec![NodeId("A".into())];
        let g = graph(vec![a, b]);
        assert!(validate_for_freeze(&g, &[], &HashMap::new()).is_empty());
    }

    #[test]
    fn infrastructure_node_with_no_business_dependent_is_rejected() {
        let infra = node("infra", NodePurpose::Infrastructure, vec![]);
        let g = graph(vec![infra]);
        let violations = validate_for_freeze(&g, &[], &HashMap::new());
        assert_eq!(
            violations,
            vec![FreezeViolation::UnanchoredInfrastructureNode {
                node: NodeId("infra".into())
            }]
        );
    }

    #[test]
    fn infrastructure_node_unlocking_a_business_node_is_allowed() {
        let infra = node("infra", NodePurpose::Infrastructure, vec![]);
        let mut biz = node("biz", NodePurpose::Business, vec!["R-001"]);
        biz.depends_on = vec![NodeId("infra".into())];
        let g = graph(vec![infra, biz]);
        let must = vec![RequirementId("R-001".into())];
        // biz has no acceptance checks configured, but we only care here
        // about the infrastructure-anchoring violation being absent.
        let violations = validate_for_freeze(&g, &must, &HashMap::new());
        assert!(
            !violations
                .iter()
                .any(|v| matches!(v, FreezeViolation::UnanchoredInfrastructureNode { .. }))
        );
    }

    #[test]
    fn zero_budget_node_is_rejected() {
        let mut a = node("A", NodePurpose::Business, vec![]);
        a.estimated_budget = 0;
        let g = graph(vec![a]);
        let violations = validate_for_freeze(&g, &[], &HashMap::new());
        assert!(
            violations
                .iter()
                .any(|v| matches!(v, FreezeViolation::ZeroBudgetNode { .. }))
        );
    }

    fn created_event(id: &str, nodes: Vec<GraphNode>) -> GraphEvent {
        GraphEvent::Created {
            id: id.into(),
            contract_ref: "C-1".into(),
            graph_hash: "hash-1".into(),
            nodes,
        }
    }

    #[test]
    fn apply_created_on_none_state_constructs_version_one() {
        let a = node("A", NodePurpose::Business, vec![]);
        let result = apply(None, created_event("G-1", vec![a])).unwrap();
        assert_eq!(result.id, "G-1");
        assert_eq!(result.version, 1);
        assert_eq!(result.graph_hash, "hash-1");
        assert_eq!(result.contract_ref, "C-1");
        assert_eq!(result.nodes.len(), 1);
    }

    #[test]
    fn apply_created_on_an_existing_graph_is_rejected() {
        let existing = graph(vec![]);
        let err = apply(Some(existing), created_event("G-1", vec![])).unwrap_err();
        assert_eq!(err, GraphEventError::AlreadyCreated);
    }

    #[test]
    fn apply_replaced_on_none_state_is_rejected() {
        let event = GraphEvent::Replaced {
            graph_hash: "hash-2".into(),
            nodes: vec![],
        };
        let err = apply(None, event).unwrap_err();
        assert_eq!(err, GraphEventError::NotYetCreated);
    }

    #[test]
    fn apply_replaced_bumps_version_and_swaps_nodes_and_hash_but_keeps_id_and_contract_ref() {
        let existing = graph(vec![node("A", NodePurpose::Business, vec![])]);
        let b = node("B", NodePurpose::Business, vec![]);
        let event = GraphEvent::Replaced {
            graph_hash: "hash-2".into(),
            nodes: vec![b],
        };
        let result = apply(Some(existing), event).unwrap();
        assert_eq!(result.id, "G-1");
        assert_eq!(result.contract_ref, "C-1");
        assert_eq!(result.version, 2);
        assert_eq!(result.graph_hash, "hash-2");
        assert_eq!(result.nodes, vec![node("B", NodePurpose::Business, vec![])]);
    }

    #[test]
    fn apply_replaced_twice_advances_version_each_time() {
        let existing = graph(vec![]);
        let first = apply(
            Some(existing),
            GraphEvent::Replaced {
                graph_hash: "hash-2".into(),
                nodes: vec![],
            },
        )
        .unwrap();
        assert_eq!(first.version, 2);
        let second = apply(
            Some(first),
            GraphEvent::Replaced {
                graph_hash: "hash-3".into(),
                nodes: vec![],
            },
        )
        .unwrap();
        assert_eq!(second.version, 3);
        assert_eq!(second.graph_hash, "hash-3");
    }
}
