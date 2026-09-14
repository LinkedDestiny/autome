'use strict';

// Domain vocabulary mirrored from the Rust types that own it, so the
// renderer can render an honest state catalog with zero IPC reads (see
// Context in the plan this file implements). Every table here is checked
// against its source by test/vocabulary.test.js's count assertions —
// keep the counts in the trailing comment on each export in sync with the
// Rust source if that source ever changes.
//
// Sources:
//   crates/autome-domain/src/run.rs        (RunPhase, RunHold, RunTerminal, BlockedReason)
//   crates/autome-domain/src/project.rs    (ProjectPhase, ProjectHold)
//   crates/autome-domain/src/completion.rs (CompletionGate, 40 named boolean fields)
//   crates/autome-domain/src/skill.rs      (SkillEvidenceLadder, 6-level ladder)
//   docs/development/plan.md §5.9          (EnvironmentSnapshot component axes)
(function (factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.AutomeVocabulary = api;
  }
})(function () {
  // RunPhase — 19 variants (run.rs RUN_NOMINAL_PATH order). `linear` marks
  // the 17 that participate in RUN_LINEAR_NOMINAL_PATH; Repairing and
  // Replanning are branch-only per §6.2's "跳过 Repairing/Replanning".
  const RUN_PHASES = Object.freeze([
    { id: 'Received', zh: '已接收', linear: true },
    { id: 'ResolvingProjectContext', zh: '解析项目上下文', linear: true },
    { id: 'DiscoveringFacts', zh: '事实发现', linear: true },
    { id: 'DraftingContract', zh: '起草契约', linear: true },
    { id: 'ContractReview', zh: '契约评审', linear: true },
    { id: 'PlanningGraph', zh: '规划任务图', linear: true },
    { id: 'GraphReview', zh: '任务图评审', linear: true },
    { id: 'CheckingReadiness', zh: '就绪检查', linear: true },
    { id: 'ContractFrozen', zh: '契约已冻结', linear: true },
    { id: 'Ready', zh: '就绪', linear: true },
    { id: 'Executing', zh: '执行中', linear: true },
    { id: 'Repairing', zh: '修复中', linear: false },
    { id: 'Replanning', zh: '重新规划', linear: false },
    { id: 'Integrating', zh: '集成中', linear: true },
    { id: 'FinalVerifying', zh: '最终验证', linear: true },
    { id: 'FinalAuditing', zh: '最终审计', linear: true },
    { id: 'DeliveryRehearsing', zh: '交付演练', linear: true },
    { id: 'Delivering', zh: '交付中', linear: true },
    { id: 'DeliveredTreeChecking', zh: '交付树核对', linear: true },
  ]);

  const RUN_LINEAR_NOMINAL_PATH = Object.freeze(
    RUN_PHASES.filter((p) => p.linear).map((p) => p.id)
  );

  // RunHold — 14 variants. Blocked carries a BlockedReason (5 sub-reasons).
  const RUN_HOLDS = Object.freeze([
    { id: 'None', zh: '无' },
    { id: 'AwaitingClarification', zh: '等待澄清' },
    { id: 'AwaitingPlanApproval', zh: '等待方案批准' },
    { id: 'AwaitingHumanAcceptance', zh: '等待人工验收' },
    { id: 'AwaitingDeliveryApproval', zh: '等待交付批准' },
    { id: 'AwaitingContractAmendment', zh: '等待契约修订' },
    { id: 'AwaitingCorrectionClassification', zh: '等待纠偏分类' },
    { id: 'AwaitingConfiguredHumanReview', zh: '等待配置的人工评审' },
    { id: 'ConfigurationInvalidated', zh: '配置已失效' },
    { id: 'Paused', zh: '已暂停' },
    { id: 'Blocked', zh: '已阻塞' },
    { id: 'BudgetExhausted', zh: '预算已耗尽' },
    { id: 'Stalled', zh: '停滞' },
    { id: 'UnknownOutcome', zh: '结果未知' },
  ]);

  const BLOCKED_REASONS = Object.freeze([
    { id: 'TargetChanged', zh: '目标已变化' },
    { id: 'DeliveryIntegrityMismatch', zh: '交付完整性不一致' },
    { id: 'VerificationInconclusive', zh: '验证无法定论' },
    { id: 'EnvironmentNotReady', zh: '环境未就绪' },
    { id: 'MissingAuthoritativeSource', zh: '缺少权威来源' },
  ]);

  // RunTerminal — 6 variants.
  const RUN_TERMINALS = Object.freeze([
    { id: 'None', zh: '未终止' },
    { id: 'Completed', zh: '已完成' },
    { id: 'Superseded', zh: '已被取代' },
    { id: 'ProtocolFailed', zh: '协议失败' },
    { id: 'Infeasible', zh: '不可行' },
    { id: 'Cancelled', zh: '已取消' },
  ]);

  // ProjectPhase — 8 variants (project.rs PROJECT_NOMINAL_PATH, all linear).
  const PROJECT_PHASES = Object.freeze([
    { id: 'Registered', zh: '已注册' },
    { id: 'Inspecting', zh: '检查中' },
    { id: 'AwaitingTrust', zh: '等待信任确认' },
    { id: 'Initializing', zh: '初始化中' },
    { id: 'ResolvingIntent', zh: '解析产品意图' },
    { id: 'ResolvingConfig', zh: '解析配置' },
    { id: 'CheckingEnvironmentAndSkills', zh: '检查环境与技能' },
    { id: 'Ready', zh: '就绪' },
  ]);

  // ProjectHold — 7 variants.
  const PROJECT_HOLDS = Object.freeze([
    { id: 'None', zh: '无' },
    { id: 'IdentityChanged', zh: '身份已变化' },
    { id: 'IntentUnresolved', zh: '产品意图未解析' },
    { id: 'ConfigInvalid', zh: '配置无效' },
    { id: 'EnvironmentBlocked', zh: '环境被阻塞' },
    { id: 'SkillsBlocked', zh: '技能被阻塞' },
    { id: 'InitializationFailed', zh: '初始化失败' },
  ]);

  // CompletionGate — 40 named boolean fields (completion.rs). id matches
  // the Rust field name exactly (used by open_gates() diagnostics); zh is
  // a UI gloss of the field's doc comment, not a re-definition of it.
  const COMPLETION_GATES = Object.freeze([
    { id: 'contract_is_frozen', zh: 'TaskContract 已冻结' },
    {
      id: 'execution_run_origin_chain_is_valid_and_promoted_outputs_match_contract_graph',
      zh: '每个已晋升产出的溯源链有效且匹配冻结契约/任务图',
    },
    { id: 'execution_run_spec_is_frozen_and_matches_current_run', zh: 'ExecutionRunSpec 已冻结且绑定当前 Run' },
    { id: 'project_is_active_initialized_and_identity_current', zh: 'Project 处于 Active、已初始化、身份未变化' },
    {
      id: 'project_intent_revision_matches_contract_and_has_no_unapproved_conflict',
      zh: 'ProjectIntentRevision 与契约一致且无未批准冲突',
    },
    { id: 'resolved_project_config_matches_run_snapshot', zh: 'ResolvedProjectConfig 哈希匹配本 Run 快照' },
    { id: 'every_step_route_matches_qualified_cli_model_effort', zh: '每个步骤路由匹配已资格认证的 CLI/模型/Effort' },
    {
      id: 'every_attempt_matches_its_frozen_permission_profile_and_provider_observation',
      zh: '每个 Attempt 匹配其冻结权限画像与 provider 实际观察',
    },
    { id: 'skill_set_snapshot_is_unchanged_and_projection_verified', zh: 'SkillSetSnapshot 未变化且各 CLI 投影已验证' },
    {
      id: 'no_environment_or_skill_install_binding_transaction_affects_run_snapshot',
      zh: '无并发的环境/技能安装或绑定事务影响本 Run 快照',
    },
    { id: 'original_source_coverage_is_100_percent', zh: '原始需求语义片段 100% 映射到 Requirement/Constraint/Non-goal/Question' },
    { id: 'all_must_requirements_have_checks', zh: '每个 must Requirement 至少有一个强制验收检查' },
    { id: 'applicable_project_rule_snapshot_matches_contract_and_base', zh: 'ApplicableProjectRuleSnapshot 同时匹配契约与基线树' },
    { id: 'no_unadjudicated_project_rule_change', zh: '无未经裁决的项目规则变更' },
    { id: 'supported_environment_profile_matches', zh: 'SupportedEnvironmentProfile 匹配本 Run 已资格认证的 profile' },
    { id: 'current_readiness_receipt_is_ready', zh: '当前 ReadinessReceipt 报告 Ready' },
    { id: 'all_required_fact_receipts_are_current', zh: '每个必需 FactReceipt 都在新鲜度策略内' },
    {
      id: 'every_must_requirement_is_satisfied_by_all_its_mandatory_checks_on_candidate_tree',
      zh: '每个 must Requirement 的全部强制检查在最终候选树上通过',
    },
    { id: 'all_receipts_match_current_contract_check_tree_and_environment', zh: '每个 EvidenceReceipt 匹配当前契约/检查/环境三元组' },
    { id: 'every_mandatory_process_check_matches_its_executable_oracle_snapshot', zh: '每个强制过程检查匹配其可执行 oracle 快照' },
    { id: 'no_unexpected_skips_or_filtered_tests', zh: '无测试在记录的 TestInventorySnapshot 之外被跳过或过滤' },
    { id: 'test_inventory_not_silently_reduced', zh: 'TestInventorySnapshot 未相对基线被静默削减' },
    {
      id: 'every_test_inventory_change_is_requirement_mapped_and_independently_accepted',
      zh: '每次测试清单变更都映射到 Requirement 并被独立验收',
    },
    { id: 'all_required_negative_cases_pass', zh: '全部必需负例/对抗用例通过' },
    { id: 'final_regression_policy_satisfied', zh: '最终回归策略（§7.2 历史红线）已满足' },
    { id: 'final_audit_verdict_is_pass', zh: '独立最终 AuditVerdict == pass' },
    {
      id: 'producer_evaluator_model_choice_is_distinct_and_qualification_is_current',
      zh: '生产者/评审者模型选择不同且均为当前已资格认证',
    },
    { id: 'no_open_blocking_question_or_material_assumption', zh: '无开放的阻塞性问题或未解决的重大假设（§6.4）' },
    { id: 'no_open_blocking_finding', zh: '无开放的阻塞性发现' },
    { id: 'no_open_human_review_finding', zh: '无未解决/未被取代的开放 HumanReviewFinding' },
    { id: 'no_unexplained_out_of_scope_diff', zh: '无未说明的契约范围外差异' },
    { id: 'candidate_working_tree_is_clean', zh: '最终检查时候选工作树干净' },
    { id: 'all_required_artifacts_exist_with_matching_hash', zh: '每个必需制品存在且哈希匹配' },
    { id: 'required_human_decisions_have_receipts', zh: '每个必需人工决定都有收据' },
    { id: 'all_configured_human_reviews_have_current_receipts', zh: '每个配置要求的 HumanReview 都有当前 HumanReviewReceipt' },
    { id: 'candidate_certificate_is_valid', zh: '最终候选树的 CandidateCertificate 有效' },
    { id: 'delivery_tree_content_equals_candidate_tree', zh: '交付树内容等于已认证候选树' },
    {
      id: 'every_required_deliverable_is_present_at_approved_destination_with_matching_hash',
      zh: '每个必需交付物存在于已批准目的地且哈希匹配',
    },
    { id: 'delivery_chain_matches_task_kind', zh: '交付链匹配 task_kind 对应的判据（§7）' },
    { id: 'event_log_integrity_check_passes', zh: '事件日志完整性检查通过' },
  ]);

  // SkillEvidenceLadder — 6-level monotonic ladder (skill.rs). `effective`
  // is Option<bool> in Rust (unknown/true/false), not a plain bool like
  // the other five — see status.js for how the UI encodes that.
  const SKILL_EVIDENCE_LEVELS = Object.freeze([
    { id: 'installed', zh: '已安装', en: 'Installed' },
    { id: 'bound', zh: '已绑定', en: 'Bound' },
    { id: 'discoverable', zh: '可被发现', en: 'Discoverable' },
    { id: 'available_to_attempt', zh: '可供尝试', en: 'AvailableToAttempt' },
    { id: 'invoked', zh: '已调用', en: 'Invoked' },
    { id: 'effective', zh: '确认有效', en: 'Effective' },
  ]);

  // Environment component axes — plan §5.9 EnvironmentSnapshot.components[].
  // Five orthogonal facts feed a Core-computed `readiness`; none of them
  // is itself readiness (§5.9: "不能把'命令存在'显示成'可运行'").
  const ENVIRONMENT_AXES = Object.freeze([
    {
      id: 'presence',
      zh: '存在性',
      values: [
        { id: 'missing', zh: '缺失' },
        { id: 'present', zh: '存在' },
        { id: 'duplicate', zh: '重复' },
      ],
    },
    {
      id: 'integrity',
      zh: '完整性',
      values: [
        { id: 'trusted', zh: '可信' },
        { id: 'untrusted', zh: '不可信' },
        { id: 'unknown', zh: '未知' },
      ],
    },
    {
      id: 'auth_mode',
      zh: '认证方式',
      values: [
        { id: 'not_applicable', zh: '不适用' },
        { id: 'dedicated', zh: '独立凭证' },
        { id: 'shared_macos_keychain', zh: '共享 macOS 钥匙串' },
        { id: 'dedicated_api_key', zh: '独立 API Key' },
      ],
    },
    {
      id: 'auth_state',
      zh: '认证状态',
      values: [
        { id: 'not_applicable', zh: '不适用' },
        { id: 'signed_out', zh: '已登出' },
        { id: 'signed_in', zh: '已登录' },
        { id: 'expired', zh: '已过期' },
        { id: 'unknown', zh: '未知' },
      ],
    },
    {
      id: 'qualification',
      zh: '资格认证',
      values: [
        { id: 'qualified', zh: '已认证' },
        { id: 'stale', zh: '已过时' },
        { id: 'failed', zh: '未通过' },
        { id: 'not_required', zh: '无需认证' },
      ],
    },
  ]);

  // readiness is Core-computed from the five axes above, never a sixth
  // orthogonal axis (§5.9): "presence...与 qualification 是正交事实，再由
  // Core 计算 readiness".
  const READINESS_VALUES = Object.freeze([
    { id: 'ready', zh: '就绪' },
    { id: 'warning', zh: '警示' },
    { id: 'blocked', zh: '阻塞' },
  ]);

  // Requirement class — plan §5.9's table preventing "监测项缺失=所有 Task
  // 阻塞": a missing optional_convenience component is never more than a
  // warning; only core_task_required blocks.
  const REQUIREMENT_CLASSES = Object.freeze([
    { id: 'optional_convenience', zh: '可选便利项', missingSeverity: 'warning' },
    { id: 'provisioner', zh: '配置供给项', missingSeverity: 'warning' },
    { id: 'skill_manager', zh: '技能管理依赖', missingSeverity: 'warning' },
    { id: 'route_required', zh: '按路由才需要', missingSeverity: 'warning' },
    { id: 'core_task_required', zh: '核心任务必需', missingSeverity: 'critical' },
  ]);

  return {
    RUN_PHASES,
    RUN_LINEAR_NOMINAL_PATH,
    RUN_HOLDS,
    BLOCKED_REASONS,
    RUN_TERMINALS,
    PROJECT_PHASES,
    PROJECT_HOLDS,
    COMPLETION_GATES,
    SKILL_EVIDENCE_LEVELS,
    ENVIRONMENT_AXES,
    READINESS_VALUES,
    REQUIREMENT_CLASSES,
  };
});
