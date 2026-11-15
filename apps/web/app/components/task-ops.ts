import type { TaskPayload } from './task-detail-data.ts';
export const OP_LABELS = { note: '备注', set_after: '依赖', start: '开始', approve: '批准', reject: '驳回', reply: '补充信息', merge: '合并', merge_cancel: '撤销授权', convert: '改建成需求', pause: '暂停', resume: '恢复', stop: '停止', cancel: '取消', rerun: '重跑', extend: '追加预算', decide: '表态' } as const;
export type Intent = keyof typeof OP_LABELS;
export type OpView = { op: Intent; enabled: boolean; reason?: string; nodes?: string[]; label?: string };
export type OpsInput = { kind: string; state: NonNullable<TaskPayload['task']['state']>; roles: Record<string, boolean> | null; mergeQueue: boolean; merge?: TaskPayload['task']['merge']; openDecisions: boolean; archived?: boolean };
export const RERUN_NODES = { feature: ['intake', 'design', 'review', 'adjudicate', 'implement', 'audit', 'retro', 'rebase'], bug: ['locate', 'fix', 'fix_review', 'rebase'] };
export const EXTEND_REASONS = { feature: ['design_budget', 'impl_budget', 'gate_same_failure'], bug: ['fix_budget', 'gate_same_failure'] };
const NODE_ROLES: Record<string, string> = { design: 'plan', review: 'review', adjudicate: 'adjudicate', implement: 'impl', audit: 'audit', retro: 'retro', locate: 'impl', fix: 'impl', fix_review: 'audit' };
const waits = ['await_design_approval', 'await_merge', 'await_info', 'not_a_bug'];
const terminal = (i: OpsInput) => ['done', 'cancelled'].includes(i.state.state);
const active = (i: OpsInput) => i.state.state === 'active';
const running = (i: OpsInput) => active(i) && !waits.includes(i.state.node ?? '');
const disabledRole = (i: OpsInput, node: string) => i.roles?.[NODE_ROLES[node]] === false;
export const OP_RULES: { op: Intent; when: (i: OpsInput) => boolean }[] = [
  { op: 'start', when: i => i.state.state === 'draft' },
  { op: 'approve', when: i => i.kind !== 'bug' && active(i) && i.state.node === 'await_design_approval' },
  { op: 'reject', when: i => i.kind !== 'bug' && active(i) && i.state.node === 'await_design_approval' },
  { op: 'reply', when: i => i.kind === 'bug' && active(i) && ['await_info', 'not_a_bug'].includes(i.state.node ?? '') },
  { op: 'merge', when: i => i.mergeQueue ? !terminal(i) && i.state.state !== 'failed' && !(active(i) && ['merging', 'main_check', 'cleanup'].includes(i.state.node ?? '')) && (!i.merge?.authorized || i.state.node === 'await_merge') : active(i) && i.state.node === 'await_merge' },
  { op: 'merge_cancel', when: i => i.mergeQueue && !!i.merge?.authorized && !terminal(i) && !(active(i) && ['merging', 'main_check', 'cleanup'].includes(i.state.node ?? '')) },
  { op: 'convert', when: i => i.kind === 'bug' && !i.archived && active(i) && i.state.node === 'not_a_bug' },
  { op: 'pause', when: running },
  { op: 'resume', when: i => ['paused', 'stopped'].includes(i.state.state) },
  { op: 'stop', when: i => running(i) || i.state.state === 'paused' },
  { op: 'cancel', when: i => !terminal(i) },
  { op: 'rerun', when: i => ['failed', 'stopped'].includes(i.state.state) },
  { op: 'extend', when: i => i.state.state === 'failed' && EXTEND_REASONS[i.kind === 'bug' ? 'bug' : 'feature'].includes(String(i.state.reason?.kind)) },
  { op: 'decide', when: i => i.openDecisions },
];
export function availableOps(input: OpsInput): OpView[] {
  return OP_RULES.filter(rule => rule.when(input)).map(({ op }): OpView => {
    if (op === 'start' && input.kind === 'bug' && input.roles?.audit === false) return { op, enabled: false, reason: 'Bug 任务需要独立复核，请启用审计角色后启动' };
    if (op === 'resume' && disabledRole(input, input.state.resume ?? input.state.at ?? '')) return { op, enabled: false, reason: '该角色当前已关闭，无法从这里恢复' };
    if (op === 'rerun') {
      const all = RERUN_NODES[input.kind === 'bug' ? 'bug' : 'feature'];
      const nodes = all.filter(node => !disabledRole(input, node));
      if (input.state.state === 'failed' && input.state.reason?.kind === 'gate_same_failure' && [input.kind === 'bug' ? 'fix_gate' : 'gate', 'merge_gate'].includes(input.state.at ?? '')) nodes.push(input.state.at!);
      return { op, enabled: true, nodes, reason: all.some(node => disabledRole(input, node)) ? '已隐藏当前关闭角色的重跑节点' : undefined };
    }
    return { op, enabled: true, ...(op === 'merge' && input.state.node !== 'await_merge' ? { label: '授权合并' } : {}) };
  });
}
export function opsInput(data: TaskPayload, config: unknown): OpsInput {
  const resolved = config as { resolved?: { roles?: { role: string; config: { enabled: boolean } }[] } } | null;
  return { kind: data.task.kind ?? 'feature', state: data.task.state ?? { state: 'draft' }, roles: resolved?.resolved?.roles ? Object.fromEntries(resolved.resolved.roles.map(r => [r.role, r.config.enabled])) : null,
    archived: data.task.archived, mergeQueue: Object.hasOwn(data.task, 'merge'), merge: data.task.merge, openDecisions: !!data.decisions?.some(d => !d.consumed) };
}
