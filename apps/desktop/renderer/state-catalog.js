'use strict';

// plan.md §9.3's required first-version interface state catalog, expanded
// from its run-on enumeration sentence into one entry per distinct state
// (each "/"-separated alternative in the source sentence is its own
// state, since each needs its own microcopy — see plan Context: "'未验证'
// 不能渲染成'通过'"). 49 entries; every one carries non-empty Chinese
// microcopy so the UI never has to fall back to a blank or English string
// for a state §9.3 explicitly requires covering in the first version.
(function (factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.AutomeStateCatalog = api;
  }
})(function () {
  const STATES = Object.freeze([
    { id: 'first_launch', zh: '首次启动', microcopy: '应用刚启动，尚未连接到任何 Project。' },
    { id: 'no_project', zh: '无 Project', microcopy: '当前没有已注册的 Project，需要创建或导入。' },
    { id: 'project_uninitialized', zh: 'Project 未初始化', microcopy: 'Project 已注册但初始化流程尚未完成。' },
    { id: 'project_identity_changed', zh: 'Project 身份变化', microcopy: 'Project 的本地路径或远程身份与注册时不一致，需人工确认。' },
    { id: 'project_archived', zh: 'Project 已归档', microcopy: 'Project 处于 Archived 生命周期，只读，不可发起新 Task。' },
    { id: 'component_missing_codex', zh: 'Codex 缺失', microcopy: 'Codex CLI 未在本机检测到。' },
    { id: 'component_missing_claude', zh: 'Claude 缺失', microcopy: 'Claude Code CLI 未在本机检测到。' },
    { id: 'component_missing_iterm2', zh: 'iTerm2 缺失', microcopy: 'iTerm2 未安装，属于可选便利项，不阻塞任务。' },
    { id: 'component_missing_homebrew', zh: 'Homebrew 缺失', microcopy: 'Homebrew 未安装，属于配置供给项。' },
    { id: 'component_missing_node', zh: 'Node 缺失', microcopy: 'Node/npm 未安装，属于技能管理依赖。' },
    { id: 'duplicate_cli', zh: '重复 CLI', microcopy: '同一 CLI 检测到多个 installation，需要人工选择使用哪一个。' },
    { id: 'source_or_signature_anomaly', zh: '来源或签名异常', microcopy: '组件来源或签名校验未通过，完整性标记为不可信。' },
    { id: 'signing_in', zh: '登录中', microcopy: '认证流程进行中，尚未得到最终结果。' },
    { id: 'login_expired', zh: '登录已失效', microcopy: '此前登录的凭证已过期，需要重新认证。' },
    { id: 'shared_account_pending_confirmation', zh: '共享账号待确认', microcopy: '检测到共享 macOS 钥匙串账号，需人工确认是否使用。' },
    { id: 'qualification_expired', zh: '资格过期', microcopy: '此前的资格认证已过时，需要重新核验。' },
    { id: 'environment_warning', zh: '环境 warning', microcopy: '环境存在非阻塞性问题，Task 仍可继续。' },
    { id: 'environment_blocker', zh: '环境 blocker', microcopy: '环境存在阻塞性问题，Task 无法继续。' },
    { id: 'install_plan', zh: '安装计划', microcopy: '已生成一键补齐安装计划，等待人工确认。' },
    { id: 'install_partial_success', zh: '安装部分成功', microcopy: '安装计划部分步骤成功，其余需要人工处理。' },
    { id: 'install_needs_user_action', zh: '安装 NeedsUserAction', microcopy: '安装过程需要用户在系统层面完成一步操作。' },
    { id: 'install_failed', zh: '安装失败', microcopy: '安装步骤失败。' },
    { id: 'install_unknown_outcome', zh: '安装结果未知', microcopy: '安装步骤的结果无法确认，需要人工核对。' },
    { id: 'skill_search_offline', zh: 'Skill 搜索离线', microcopy: '技能市场搜索不可用，当前处于离线状态。' },
    { id: 'skill_audit_failed', zh: 'Skill 审计失败', microcopy: '技能安装前的安全审计未通过。' },
    { id: 'skill_name_collision', zh: 'Skill 重名', microcopy: '待安装技能与已安装技能重名，需人工裁决。' },
    { id: 'skill_update_available', zh: 'Skill 更新可用', microcopy: '已安装技能存在可用更新。' },
    { id: 'skill_projection_drift', zh: 'Skill 投影漂移', microcopy: '技能在某个 CLI 上的实际投影与期望不一致。' },
    { id: 'config_inherited', zh: '配置继承', microcopy: '此项配置继承自更高层级，未被本层覆盖。' },
    { id: 'config_overridden', zh: '配置覆盖', microcopy: '此项配置被本层显式覆盖。' },
    { id: 'config_invalid', zh: '配置无效', microcopy: '此项配置未通过校验。' },
    { id: 'missing_authoritative_source', zh: '权威来源缺失', microcopy: '该配置项缺少受信来源，无法自动补齐。' },
    { id: 'no_task', zh: '无 Task', microcopy: '此 Project 下尚无 Task。' },
    { id: 'no_pending_human_decision', zh: '无需人工决定', microcopy: '当前没有等待人工处理的决定。' },
    { id: 'no_evidence_yet', zh: '尚无证据', microcopy: '此 Requirement 尚无 EvidenceReceipt。' },
    { id: 'core_unreachable', zh: 'Core 不可达', microcopy: 'Renderer 无法连接到 automed sidecar。' },
    { id: 'protocol_incompatible', zh: '协议不兼容', microcopy: 'Core 汇报的协议版本与 Renderer 期望的不兼容。' },
    { id: 'core_recovering', zh: 'Core 恢复中', microcopy: 'Core 正在从上次异常退出恢复。' },
    { id: 'event_sequence_gap', zh: '事件序号缺口', microcopy: '事件日志中检测到序号缺口，完整性存疑。' },
    { id: 'command_outcome_unknown', zh: '命令结果未知', microcopy: '上一条命令的结果因中断而无法确认。' },
    { id: 'db_migration_failed', zh: 'DB 迁移失败', microcopy: '本地数据库迁移未成功完成。' },
    { id: 'renderer_reloaded', zh: 'Renderer 重载', microcopy: '界面刚被重新加载，正在重新同步状态。' },
    { id: 'task_blocked', zh: 'Task 阻塞', microcopy: 'Task 处于 Blocked hold，等待具体阻塞原因解除。' },
    { id: 'budget_exhausted', zh: '预算耗尽', microcopy: 'Run 的预算已耗尽，等待人工决定。' },
    { id: 'acceptance_failed', zh: '验收失败', microcopy: '人工验收未通过。' },
    { id: 'candidate_ready', zh: '候选已就绪', microcopy: '候选已通过认证，等待下一步。' },
    { id: 'delivery_pending_approval', zh: '交付待批准', microcopy: '交付已准备好，等待人工批准。' },
    { id: 'delivery_integrity_mismatch', zh: '交付完整性失败', microcopy: '交付树内容与已认证候选树不一致。' },
    { id: 'completed', zh: '已完成', microcopy: 'Run 已成功完成。' },
  ]);

  const BY_ID = new Map(STATES.map((s) => [s.id, s]));

  function stateById(id) {
    const found = BY_ID.get(id);
    if (!found) {
      throw new RangeError(`unknown state id "${id}"`);
    }
    return found;
  }

  return {
    STATES,
    stateById,
  };
});
