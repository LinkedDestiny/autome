import { toOverviewDetail } from '../lib/overview-model.ts';
import type { OverviewDetail } from '../../server/overview.ts';
import type { TaskPayload } from './task-detail-data.ts';
import { queueOrder, type BlockedDependency } from './queue-deps.ts';
export { doneGroup } from '../lib/clock.tsx';
import { currentClock, doneGroup } from '../lib/clock.tsx';
import { NOTIFICATION_RULES } from '../../server/notifier-rules.ts';
import { needsTaskDetail, type TaskDetail, type CardContext, toCardDetail } from './task-card-model.ts';
import type { WebServices } from '../load-context.ts';
import { BOARD_READ_ERROR, COLUMN_LABELS } from '../lib/labels.ts';
import type { ShellTask } from './shell-data.ts';
import { columnOf, type TaskColumn } from './task-columns.ts';

export interface BoardTask extends ShellTask { queue_position?: number | null; after?: string[]; blocked_by?: BlockedDependency[]; completed_at?: string | null; merge_commit?: string | null; state: ShellTask['state'] & { reason?: Record<string, unknown> } }
export type KindFilter = 'all' | 'feature' | 'bug';
export interface BoardColumn { id: TaskColumn; label: string; tasks: BoardTask[]; recent: BoardTask[]; older: BoardTask[]; total: number }

export function boardColumns(tasks: readonly BoardTask[], now: string, timeZone = currentClock().timeZone): BoardColumn[] {
  return (Object.keys(COLUMN_LABELS) as TaskColumn[]).map(id => {
    const rows = tasks.filter(task => columnOf(task.state) === id).sort((a, b) => id === 'done'
      ? completed(b) - completed(a)
      : (id === 'todo' ? queueOrder(a, b) : 0) || Number(b.id.match(/\d+/)?.[0] ?? 0) - Number(a.id.match(/\d+/)?.[0] ?? 0));
    const recent = id === 'done' ? rows.filter(task => doneGroup(task.completed_at, now, timeZone) !== '更早') : rows;
    const older = id === 'done' ? rows.filter(task => !recent.includes(task)) : [];
    return { id, label: COLUMN_LABELS[id], tasks: rows, recent, older, total: rows.length };
  });
}
function completed(task: BoardTask): number { const time = Date.parse(task.completed_at ?? ''); return Number.isFinite(time) ? time : -Infinity; }
export function filterBoard(columns: readonly BoardColumn[], kind: KindFilter): BoardColumn[] {
  const matches = (task: BoardTask) => kind === 'all' || task.kind === kind;
  return columns.map(column => ({ ...column, tasks: column.tasks.filter(matches), recent: column.recent.filter(matches), older: column.older.filter(matches) }));
}
export function boardHeading(column: BoardColumn, kind: KindFilter): string {
  return `${column.label}（${kind === 'all' ? column.total : `${column.tasks.length} / ${column.total}`}）`;
}
export type CancelledTask = Pick<BoardTask, 'id' | 'title' | 'kind' | 'state'>;
export type BoardData = { error: string } | { project: { id: string; display_name: string }; configError?: string; tasks: BoardTask[]; now: string; showAll: boolean; showCancelled?: boolean; cancelledCounts?: Record<KindFilter, number>; cancelled?: CancelledTask[]; details?: Record<string, TaskDetail>; overviewDetails?: Record<string, OverviewDetail>; cardContext?: CardContext };
export async function loadBoard({ gateway, unread }: WebServices, id: string, showAll = false, showCancelled = false): Promise<BoardData> {
  if (!gateway) return { error: '内核未连接：服务未接入网关' };
  const result = await gateway.read({ method: 'project.get', params: { project_id: id } });
  if (!result.body.ok) return { error: result.body.error ?? BOARD_READ_ERROR };
  const payload = result.body.payload as { project: { id: string; display_name: string }; config_error?: string; tasks: BoardTask[]; config?: { bug?: { fix_rounds?: number } } };
  const now = currentClock().now;
  const cancelledRows = payload.tasks.filter(task => task.state.state === 'cancelled');
  const cancelledCounts = { all: cancelledRows.length, feature: cancelledRows.filter(task => task.kind === 'feature').length, bug: cancelledRows.filter(task => task.kind === 'bug').length };
  const cancelled = showCancelled ? cancelledRows.map(({ id, title, kind, state }) => ({ id, title, kind, state })).sort((a, b) => Number(b.id.match(/\d+/)?.[0] ?? 0) - Number(a.id.match(/\d+/)?.[0] ?? 0)) : undefined;
  const tasks = payload.tasks.filter(task => task.state.state !== 'cancelled').map(({ id: taskId, title, kind, state, fix_round, completed_at, merge_commit, queue_position, after, blocked_by, created_at, cost_usd, metrics }) => ({ id: taskId, title, kind, state, fix_round, completed_at, merge_commit, queue_position, after, blocked_by, created_at, cost_usd, metrics: metrics && { impl_rounds_used: metrics.impl_rounds_used, reopen_total: metrics.reopen_total }, project_id: id }));
  const visible = boardColumns(tasks, now).flatMap(column => showAll ? column.tasks : column.recent);
  const results = await Promise.all(visible.filter(needsTaskDetail).map(async task => ({ id: task.id, result: await gateway.read({ method: 'task.get', params: { task_id: task.id } }) })));
  const details = Object.fromEntries(results.filter(item => item.result.body.ok).map(item => [item.id, toCardDetail(item.result.body.payload as TaskDetail)]));
  const overviewDetails = Object.fromEntries(results.map(item => [item.id, item.result.body.ok ? toOverviewDetail(item.result.body.payload as TaskPayload) : { reviews: 0, adjudications: 0, pending: 0, error: item.result.body.error ?? '详情暂不可用' }]));
  let cooldowns: CardContext['cooldowns'] = [];
  if (Object.values(details).some(detail => detail.retry)) {
    const dashboard = await gateway.read({ method: 'dashboard.get', params: {} });
    if (!dashboard.body.ok) return { error: dashboard.body.error ?? BOARD_READ_ERROR };
    cooldowns = (dashboard.body.payload as { cooldowns: CardContext['cooldowns'] }).cooldowns;
  }
  const counts = Object.fromEntries((unread?.unreadByTask(needYouKinds()) ?? []).map(row => [row.task_id, row.count]));
  return { configError: payload.config_error, project: { id: payload.project.id, display_name: payload.project.display_name }, tasks, now, showAll, showCancelled, cancelledCounts, ...(cancelled ? { cancelled } : {}), details, overviewDetails, cardContext: { now, fixRounds: payload.config?.bug?.fix_rounds ?? 0, cooldowns, unread: counts } };
}

export function needYouKinds() { return NOTIFICATION_RULES.filter(rule => rule.level === 'need_you').map(rule => rule.kind); }

export const DONE_GROUPS = ['今天', '昨天', '本周更早', '更早'] as const;
export function doneRow(task: BoardTask) {
  return { id: task.id, title: task.title, completed_at: task.completed_at,
    rounds: task.kind === 'feature' ? task.metrics?.impl_rounds_used ?? '—' : '—',
    reopens: task.metrics?.reopen_total ?? '—', cost: task.cost_usd == null ? '无计费' : '$' + task.cost_usd.toFixed(2) };
}
export function doneTotals(tasks: readonly BoardTask[]) {
  const done = tasks.filter(t => columnOf(t.state) === 'done');
  const rounds = done.filter(t => t.kind === 'feature' && t.metrics?.impl_rounds_used != null);
  return { cost: '$' + done.reduce((sum, t) => sum + (t.cost_usd ?? 0), 0).toFixed(2),
    average: rounds.length ? (rounds.reduce((sum, t) => sum + t.metrics!.impl_rounds_used!, 0) / rounds.length).toFixed(1) : '—' };
}
