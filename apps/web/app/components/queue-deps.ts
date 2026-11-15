import type { ShellTask } from './shell-data.ts';
import type { TaskPayload } from './task-detail-data.ts';
import type { WebServices } from '../load-context.ts';
import { MAX_AFTER_IDS } from '../../server/gates/write-gate.ts';

export type BlockedDependency = { id: string; state: { state: string; node?: string }; reason?: string | null };
export const isQueued = (task: Pick<ShellTask, 'state'>) => task.state.state === 'queued';
export function queueOrder(a: { state: ShellTask['state']; queue_position?: number | null }, b: { state: ShellTask['state']; queue_position?: number | null }) {
  return Number(isQueued(b)) - Number(isQueued(a)) || (isQueued(a) && isQueued(b) ? (a.queue_position ?? Infinity) - (b.queue_position ?? Infinity) : 0);
}
export function dependencyCandidates(tasks: readonly ShellTask[], projectId: string, self?: string) {
  return tasks.filter(task => task.project_id === projectId && task.id !== self && !['done', 'cancelled'].includes(task.state.state))
    .sort((a, b) => Number(b.id.slice(2)) - Number(a.id.slice(2)));
}
export function dependencyReadOnly(data: TaskPayload): string | null {
  const state = data.task.state;
  if (!state) return '任务状态未知';
  if (['done', 'cancelled'].includes(state.state)) return '任务已结束';
  if (data.task.kind !== 'bug' && data.sessions.some(s => s.kind?.kind === 'role' && s.kind.role === 'impl')) return '依赖只能在设计批准之前修改';
  if (['draft', 'queued'].includes(state.state) && !state.resume) return null;
  if (data.task.kind === 'bug') return 'Bug 任务开始后不能修改依赖';
  return ['intake', 'design', 'review', 'adjudicate', 'await_design_approval'].includes(state.node ?? state.resume ?? state.at ?? '') ? null : '依赖只能在设计批准之前修改';
}
export function afterInput(form: FormData): { after: string[]; error?: never } | { error: string; after?: never } {
  const after = form.get('clear') === 'yes' ? [] : form.getAll('after');
  if (after.length > MAX_AFTER_IDS || !after.every(id => typeof id === 'string' && /^T-\d+$/.test(id))) return { error: `after 必须是任务编号列表，每项格式为 T-N，最多 ${MAX_AFTER_IDS} 项` };
  return { after: after as string[] };
}
export async function moveTask(form: FormData, services: WebServices) {
  const placements = ['top', 'before', 'after'].filter(key => form.has(key));
  if (placements.length !== 1 || (placements[0] === 'top' && form.get('top') !== 'yes')) return { error: 'before、after、top 必须且只能给一个' };
  const key = placements[0];
  const params = { task_id: form.get('task_id'), [key]: key === 'top' ? true : form.get(key) };
  if (!services.gateway) return { error: '内核未连接：服务未接入网关' };
  return (await services.gateway.write({ op: 'task.move', params })).body;
}
