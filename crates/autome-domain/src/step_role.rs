//! §10.1 逻辑角色 and the nine-entry `LoopStepId` schema it defines.
//!
//! `LoopStepId` itself already lives in `attempt.rs` as a `String`
//! newtype, routed through `config.rs`/`model_selection.rs`/`review.rs`/
//! `skill.rs` — this module does not introduce a competing typed identity
//! for it, only the fixed canonical schema §10.1 requires: which of the
//! five logical roles owns each step, and the schema-completeness check
//! that `model_selection.rs`'s `validate_model_separation` doc comment
//! already names and defers: "a separate schema-completeness check owns
//! 'all nine LoopStepIds must be routed before the Scheduler starts'."
//! This is that check.
//!
//! §10.1 names Verifier and "测试作者" as explicitly NOT logical roles
//! ("Verifier 不是 LLM 角色，而是 Rust 控制的事实执行器"; "测试作者也不是
//! auditor") — so `LogicalRole` has five variants, not seven.
//!
//! Deliberately out of scope here: the pass/reject successor routing in
//! §10.1's step table (`DraftingContract`→`ContractReview` etc.) and all of
//! §10.2's `StepExecutionRoute`/`AttemptPermissionProfile` freezing
//! mechanism. Several of those successor cells are conditional prose
//! ("初始规划→CheckingReadiness；重规划→待新 Run 批准"), not a single fixed
//! next state, and modeling them needs `StepExecutionRoute` and a
//! Run/Node-phase type this crate does not have yet — inventing that
//! mapping now would be guessing, not modeling.

use std::collections::HashSet;

use crate::attempt::LoopStepId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogicalRole {
    Analyst,
    Planner,
    ContractReviewer,
    Implementer,
    Auditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteAccess {
    ReadOnly,
    CandidateWrite,
}

impl LogicalRole {
    /// §10.1's role table: analyst/planner/contract-reviewer/auditor are
    /// all "read-only"; implementer alone is "workspace-write".
    pub fn write_access(self) -> WriteAccess {
        match self {
            LogicalRole::Implementer => WriteAccess::CandidateWrite,
            _ => WriteAccess::ReadOnly,
        }
    }

    /// §10.1: contract-reviewer and auditor rows both carry "独立模型".
    pub fn requires_independent_model(self) -> bool {
        matches!(self, LogicalRole::ContractReviewer | LogicalRole::Auditor)
    }
}

/// The nine `LoopStepId` values named in §10.1's step table, each bound to
/// exactly one `LogicalRole`, in the table's own order.
pub fn canonical_step_role_bindings() -> [(LoopStepId, LogicalRole); 9] {
    [
        (LoopStepId("fact_analysis".into()), LogicalRole::Analyst),
        (LoopStepId("contract_drafting".into()), LogicalRole::Planner),
        (
            LoopStepId("contract_review".into()),
            LogicalRole::ContractReviewer,
        ),
        (
            LoopStepId("task_graph_planning".into()),
            LogicalRole::Planner,
        ),
        (
            LoopStepId("graph_review".into()),
            LogicalRole::ContractReviewer,
        ),
        (
            LoopStepId("implementation".into()),
            LogicalRole::Implementer,
        ),
        (LoopStepId("repair".into()), LogicalRole::Implementer),
        (LoopStepId("node_evaluation".into()), LogicalRole::Auditor),
        (LoopStepId("final_audit".into()), LogicalRole::Auditor),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepScheduleError {
    MissingStep(LoopStepId),
    UnknownStep(LoopStepId),
    DuplicateStep(LoopStepId),
    RoleMismatch {
        step: LoopStepId,
        expected: LogicalRole,
        actual: LogicalRole,
    },
}

/// §10.1: "表中包含全部九个 LoopStepId，Rust schema test 必须逐字比对该
/// 全集，任何缺项都不能启动 Scheduler." Collects every violation instead of
/// stopping at the first, in keeping with the rest of this crate's
/// validation functions.
pub fn validate_step_schema(candidate: &[(LoopStepId, LogicalRole)]) -> Vec<StepScheduleError> {
    let mut errors = Vec::new();
    let canonical = canonical_step_role_bindings();

    let mut seen: HashSet<LoopStepId> = HashSet::new();
    for (step, role) in candidate {
        if !seen.insert(step.clone()) {
            errors.push(StepScheduleError::DuplicateStep(step.clone()));
            continue;
        }
        match canonical
            .iter()
            .find(|(canonical_step, _)| canonical_step == step)
        {
            None => errors.push(StepScheduleError::UnknownStep(step.clone())),
            Some((_, expected_role)) if expected_role != role => {
                errors.push(StepScheduleError::RoleMismatch {
                    step: step.clone(),
                    expected: *expected_role,
                    actual: *role,
                });
            }
            Some(_) => {}
        }
    }
    for (canonical_step, _) in &canonical {
        if !seen.contains(canonical_step) {
            errors.push(StepScheduleError::MissingStep(canonical_step.clone()));
        }
    }
    errors
}

/// §10.1: "任何缺项都不能启动 Scheduler" — the sole gate a Scheduler
/// bootstrap should consult before accepting a step schema.
pub fn may_start_scheduler(candidate: &[(LoopStepId, LogicalRole)]) -> bool {
    validate_step_schema(candidate).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_bindings_cover_all_nine_steps_with_no_duplicates() {
        let bindings = canonical_step_role_bindings();
        assert_eq!(bindings.len(), 9);
        let unique: HashSet<&LoopStepId> = bindings.iter().map(|(step, _)| step).collect();
        assert_eq!(unique.len(), 9);
    }

    #[test]
    fn implementer_is_the_only_candidate_write_role() {
        for role in [
            LogicalRole::Analyst,
            LogicalRole::Planner,
            LogicalRole::ContractReviewer,
            LogicalRole::Auditor,
        ] {
            assert_eq!(role.write_access(), WriteAccess::ReadOnly);
        }
        assert_eq!(
            LogicalRole::Implementer.write_access(),
            WriteAccess::CandidateWrite
        );
    }

    #[test]
    fn only_contract_reviewer_and_auditor_require_an_independent_model() {
        assert!(LogicalRole::ContractReviewer.requires_independent_model());
        assert!(LogicalRole::Auditor.requires_independent_model());
        assert!(!LogicalRole::Analyst.requires_independent_model());
        assert!(!LogicalRole::Planner.requires_independent_model());
        assert!(!LogicalRole::Implementer.requires_independent_model());
    }

    #[test]
    fn canonical_schema_may_start_scheduler() {
        let candidate: Vec<(LoopStepId, LogicalRole)> = canonical_step_role_bindings().to_vec();
        assert!(may_start_scheduler(&candidate));
        assert!(validate_step_schema(&candidate).is_empty());
    }

    #[test]
    fn missing_step_blocks_scheduler_start() {
        let mut candidate: Vec<(LoopStepId, LogicalRole)> = canonical_step_role_bindings().to_vec();
        candidate.retain(|(step, _)| step != &LoopStepId("repair".into()));
        let errors = validate_step_schema(&candidate);
        assert_eq!(
            errors,
            vec![StepScheduleError::MissingStep(LoopStepId("repair".into()))]
        );
        assert!(!may_start_scheduler(&candidate));
    }

    #[test]
    fn unknown_step_is_reported_and_the_real_gap_it_left_behind_is_also_reported() {
        let mut candidate: Vec<(LoopStepId, LogicalRole)> = canonical_step_role_bindings().to_vec();
        candidate.retain(|(step, _)| step != &LoopStepId("final_audit".into()));
        candidate.push((LoopStepId("made_up_step".into()), LogicalRole::Auditor));
        let errors = validate_step_schema(&candidate);
        assert_eq!(
            errors,
            vec![
                StepScheduleError::UnknownStep(LoopStepId("made_up_step".into())),
                StepScheduleError::MissingStep(LoopStepId("final_audit".into())),
            ]
        );
    }

    #[test]
    fn duplicate_step_is_reported() {
        let mut candidate: Vec<(LoopStepId, LogicalRole)> = canonical_step_role_bindings().to_vec();
        candidate.push((LoopStepId("fact_analysis".into()), LogicalRole::Analyst));
        let errors = validate_step_schema(&candidate);
        assert_eq!(
            errors,
            vec![StepScheduleError::DuplicateStep(LoopStepId(
                "fact_analysis".into()
            ))]
        );
    }

    #[test]
    fn role_mismatch_on_a_known_step_is_reported() {
        let mut candidate: Vec<(LoopStepId, LogicalRole)> = canonical_step_role_bindings().to_vec();
        let entry = candidate
            .iter_mut()
            .find(|(step, _)| step == &LoopStepId("implementation".into()))
            .unwrap();
        entry.1 = LogicalRole::Auditor;
        let errors = validate_step_schema(&candidate);
        assert_eq!(
            errors,
            vec![StepScheduleError::RoleMismatch {
                step: LoopStepId("implementation".into()),
                expected: LogicalRole::Implementer,
                actual: LogicalRole::Auditor,
            }]
        );
    }
}
