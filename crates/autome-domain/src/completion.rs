//! The completion predicate from plan §7 / §0. This is *the* normative
//! definition of "done" (plan §0: "唯一规范谓词见 §7，并由 Rust 测试锁定").
//! Every field is a fact some other subsystem must establish; this module
//! only conjuncts them. It must never gain a way to short-circuit to `true`
//! (e.g. no `Default` that starts all-true, no partial constructor).

use serde::{Deserialize, Serialize};

macro_rules! completion_gate {
    ($($field:ident : $doc:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub struct CompletionGate {
            $(
                #[doc = $doc]
                pub $field: bool,
            )+
        }

        impl CompletionGate {
            /// All-true only exists for tests that build a passing baseline
            /// and then flip one field at a time; production callers must
            /// set every field from an actual fact.
            #[cfg(test)]
            fn all_true_for_test() -> Self {
                Self { $( $field: true, )+ }
            }

            /// Every gate that is false, by field name, for diagnostics.
            pub fn open_gates(&self) -> Vec<&'static str> {
                let mut open = Vec::new();
                $(
                    if !self.$field {
                        open.push(stringify!($field));
                    }
                )+
                open
            }

            pub fn is_complete(&self) -> bool {
                self.open_gates().is_empty()
            }
        }
    };
}

completion_gate! {
    contract_is_frozen: "TaskContract has been frozen (D5/D11).",
    execution_run_origin_chain_is_valid_and_promoted_outputs_match_contract_graph:
        "Every promoted output traces back through a valid origin chain to the frozen contract/graph.",
    execution_run_spec_is_frozen_and_matches_current_run: "ExecutionRunSpec is frozen and bound to this Run.",
    project_is_active_initialized_and_identity_current: "Project.lifecycle=Active, initialized, identity unchanged since Run start.",
    project_intent_revision_matches_contract_and_has_no_unapproved_conflict:
        "ProjectIntentRevision used by the contract has no unresolved amendment conflict.",
    resolved_project_config_matches_run_snapshot: "ResolvedProjectConfig hash matches the snapshot bound to this Run.",
    every_step_route_matches_qualified_cli_model_effort: "Every StepExecutionRoute resolves to a currently-qualified CLI/model/Effort (D8).",
    every_attempt_matches_its_frozen_permission_profile_and_provider_observation:
        "Every Attempt's AttemptPermissionProfile matches both the frozen profile and observed provider behavior.",
    skill_set_snapshot_is_unchanged_and_projection_verified: "SkillSetSnapshot is unchanged and its per-CLI projection was verified (D15).",
    no_environment_or_skill_install_binding_transaction_affects_run_snapshot:
        "No concurrent Environment/Skill install or binding transaction touches this Run's snapshot.",
    original_source_coverage_is_100_percent: "Every original-request semantic fragment maps to Requirement/Constraint/Non-goal/Question (§7).",
    all_must_requirements_have_checks: "Every must Requirement has at least one mandatory AcceptanceCheck.",
    applicable_project_rule_snapshot_matches_contract_and_base: "ApplicableProjectRuleSnapshot matches both the contract and the base tree.",
    no_unadjudicated_project_rule_change: "No project rule changed without an adjudicated decision.",
    supported_environment_profile_matches: "SupportedEnvironmentProfile matches the qualified profile for this Run (D12).",
    current_readiness_receipt_is_ready: "The current ReadinessReceipt reports Ready.",
    all_required_fact_receipts_are_current: "Every mandatory FactReceipt is within its freshness policy (§4.4).",
    every_must_requirement_is_satisfied_by_all_its_mandatory_checks_on_candidate_tree:
        "Every must Requirement's mandatory checks pass on the final candidate tree.",
    all_receipts_match_current_contract_check_tree_and_environment: "Every EvidenceReceipt matches the current contract/check/environment triple.",
    every_mandatory_process_check_matches_its_executable_oracle_snapshot: "Every mandatory process check matches its ExecutableOracleSnapshot.",
    no_unexpected_skips_or_filtered_tests: "No test was skipped or filtered outside the recorded TestInventorySnapshot.",
    test_inventory_not_silently_reduced: "TestInventorySnapshot was not silently reduced relative to baseline.",
    every_test_inventory_change_is_requirement_mapped_and_independently_accepted:
        "Every test inventory change maps to a Requirement and was independently accepted.",
    all_required_negative_cases_pass: "All required negative/adversarial cases pass.",
    final_regression_policy_satisfied: "Final regression policy (§7.2 historical red lines) is satisfied.",
    final_audit_verdict_is_pass: "Independent final AuditVerdict == pass.",
    producer_evaluator_model_choice_is_distinct_and_qualification_is_current: "Producer/evaluator model_choice_key_hash differ and both are currently qualified (D6).",
    no_open_blocking_question_or_material_assumption: "No open blocking question or unresolved material assumption remains (§6.4).",
    no_open_blocking_finding: "No open blocking finding remains.",
    no_open_human_review_finding: "No open HumanReviewFinding remains unresolved/superseded.",
    no_unexplained_out_of_scope_diff: "No diff outside the contract's scope is unexplained.",
    candidate_working_tree_is_clean: "The candidate working tree is clean at final check.",
    all_required_artifacts_exist_with_matching_hash: "Every required artifact exists with a matching content hash.",
    required_human_decisions_have_receipts: "Every required human decision has a receipt.",
    all_configured_human_reviews_have_current_receipts: "Every configured-required HumanReview has a current HumanReviewReceipt.",
    candidate_certificate_is_valid: "CandidateCertificate is valid for the final candidate tree.",
    delivery_tree_content_equals_candidate_tree: "The delivered tree's content equals the certified candidate tree.",
    every_required_deliverable_is_present_at_approved_destination_with_matching_hash:
        "Every required deliverable exists at its approved destination with a matching hash.",
    delivery_chain_matches_task_kind: "Delivery rehearsal/approval/receipt/tree-check chain matches the task_kind-specific predicate (existing_repo_change or greenfield_product, §7).",
    event_log_integrity_check_passes: "The event log integrity check passes.",
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_true_baseline_is_complete() {
        assert!(CompletionGate::all_true_for_test().is_complete());
        assert!(CompletionGate::all_true_for_test().open_gates().is_empty());
    }

    #[test]
    fn any_single_false_field_blocks_completion() {
        // This is the load-bearing property: there is no field whose
        // falseness the predicate tolerates. Flip each field one at a time.
        let baseline = CompletionGate::all_true_for_test();
        let json = serde_json::to_value(baseline).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.is_empty());
        for key in obj.keys() {
            let mut mutated = json.clone();
            mutated[key] = serde_json::Value::Bool(false);
            let gate: CompletionGate = serde_json::from_value(mutated).unwrap();
            assert!(
                !gate.is_complete(),
                "flipping `{key}` to false must block completion, but is_complete() was still true"
            );
            assert_eq!(gate.open_gates(), vec![key.as_str()]);
        }
    }

    #[test]
    fn final_audit_pass_alone_is_not_sufficient() {
        let mut gate = CompletionGate::all_true_for_test();
        gate.final_audit_verdict_is_pass = true;
        gate.all_required_artifacts_exist_with_matching_hash = false;
        assert!(!gate.is_complete());
    }
}
