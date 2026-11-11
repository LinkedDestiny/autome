import type { LedgerEvent } from './eventbus-protocol.ts';

type State = { state?: string; node?: string; reason?: Record<string, unknown> };
export type EventPayload = { state?: State; kind?: string; task_id?: string };
type Rule = { event: string; state?: string; node?: string; kind: string; level: 'need_you' | 'fyi'; title: string; enrich: 'task' | 'project' };
export const NOTIFICATION_RULES: Rule[] = [
  { event: 'task.updated', state: 'active', node: 'await_design_approval', kind: 'await_design_approval', level: 'need_you', title: '{task} 设计已定稿，等你批准', enrich: 'task' },
  { event: 'task.updated', state: 'active', node: 'await_merge', kind: 'await_merge', level: 'need_you', title: '{task} 可以合并了', enrich: 'task' },
  { event: 'task.updated', state: 'active', node: 'await_info', kind: 'await_info', level: 'need_you', title: '{task} 无法复现，需要更多信息', enrich: 'task' },
  { event: 'task.updated', state: 'failed', kind: 'failed', level: 'need_you', title: '{task} 停在失败上：{reason}', enrich: 'task' },
  { event: 'project.alert', kind: 'project_alert', level: 'need_you', title: '{alert}', enrich: 'project' },
  { event: 'task.updated', state: 'done', kind: 'done', level: 'fyi', title: '{task} 已合并', enrich: 'task' },
  { event: 'task.updated', state: 'active', node: 'not_a_bug', kind: 'not_a_bug', level: 'need_you', title: '{task} 被判定不是 Bug，等你决定', enrich: 'task' },
];
export function matchEvent(event: LedgerEvent) {
  const payload = event.payload as EventPayload;
  return NOTIFICATION_RULES.find(rule => rule.event === event.kind && (!rule.state || rule.state === payload.state?.state) && (!rule.node || rule.node === payload.state?.node));
}
export function levelOf(kind: string) { return NOTIFICATION_RULES.find(rule => rule.kind === kind)?.level; }
export function renderReason(reason: Record<string, unknown> = {}) {
  const text = String(reason.detail ?? '').replace(/\s+/g, ' ').trim();
  const detail = Array.from(text).length > 60 ? Array.from(text).slice(0, 60).join('') + '…' : text;
  const titles: Record<string, string> = {
    design_budget: `设计轮次用完（上限 ${reason.limit}）`, impl_budget: `实现轮次用完（上限 ${reason.limit}）`, fix_budget: `修复轮次用完（上限 ${reason.limit}）`,
    infeasible: '设计判定不可实现', protocol: `协议失败（${detail}）`, session_crashed: `会话异常退出（${detail}）`, rebase_conflict: `rebase 冲突（${detail}）`, config: `配置问题（${detail}）`, core_step: `内核步骤 ${reason.node} 失败（${detail}）`,
  };
  return titles[String(reason.kind)] ?? `未知原因（${reason.kind}）`;
}
export function renderTitle(rule: Rule, event: LedgerEvent, branch = 'main') {
  const payload = event.payload as EventPayload;
  const values: Record<string, string> = { task: event.subject, reason: renderReason(payload.state?.reason), alert: payload.kind === 'main_gate_failed' ? `${branch} 门禁失败` : `项目告警：${payload.kind}` };
  return rule.title.replace(/\{(task|reason|alert)\}/g, (_, key: string) => values[key]);
}
