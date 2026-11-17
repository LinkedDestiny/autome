import type { OverviewDetail } from '../../server/overview.ts';
import type { TaskPayload } from '../components/task-detail-data.ts';
import type { PillColor } from '../components/ui/pill.tsx';
export type TodayWaiting = { closedMs: number; openStarts: number[]; dayStart: number; dayEnd: number };
export function waitingMilliseconds(slice: TodayWaiting, now: string): number {
  const end = Math.min(Date.parse(now), slice.dayEnd);
  return slice.closedMs + slice.openStarts.reduce((sum, start) => sum + Math.max(0, end - Math.max(start, slice.dayStart)), 0);
}
export function needsNewDay(slice: Pick<TodayWaiting, 'dayEnd'>, now: string) { return Date.parse(now) >= slice.dayEnd; }
export function overviewConclusion(waiting: number, running: number, queued: number, driving = false): string {
  if (!waiting && !running && !queued) return `现在没有事情等你 · 没有任务在跑，驱动${driving ? '在推进' : '空闲'}`;
  const rest = [running ? `${running} 个任务在跑` : '', queued ? `${queued} 个在排队` : ''].filter(Boolean);
  return `${waiting ? `有 ${waiting} 件事等你决定` : '没有事情等你决定'}${rest.length ? ` · 另有 ${rest.join('，')}` : ''}`;
}
export type MergeMetrics = { impl_rounds_used?: number; merge_gate_runs?: number | null; reopen_total?: number; cost_usd?: number | null };
export function mergeResult(metrics?: MergeMetrics | null): { text: string; color: PillColor; filled?: boolean } {
  if (!metrics) return { text: '—', color: 'brown' };
  if ((metrics.merge_gate_runs ?? 0) > 1) return { text: `门禁 ${metrics.merge_gate_runs} 次`, color: 'orange', filled: true };
  if ((metrics.reopen_total ?? 0) >= 5) return { text: `退回 ${metrics.reopen_total} 次`, color: 'orange', filled: true };
  if ((metrics.reopen_total ?? 0) >= 1) return { text: `退回 ${metrics.reopen_total} 次`, color: 'brown' };
  if (metrics.merge_gate_runs === 1) return { text: '门禁一次过', color: 'green', filled: true };
  if (metrics.reopen_total === 0) return { text: '无退回', color: 'green', filled: true };
  return { text: '—', color: 'brown' };
}

export function toOverviewDetail(detail: TaskPayload): OverviewDetail {
  const session = detail.sessions?.find(s => s.running);
  return { status_block: detail.status_block, gate: detail.gate,
    session: session && { label: session.label, kind: session.kind, round: session.round, started_at: session.started_at },
    reviews: detail.sessions?.filter(s => s.kind?.kind === 'role' && s.kind.role === 'review').length ?? 0,
    adjudications: detail.sessions?.filter(s => s.kind?.kind === 'role' && s.kind.role === 'adjudicate').length ?? 0,
    pending: detail.decisions?.filter(d => !d.consumed && d.disposition === 'none').length ?? 0,
    designBytes: detail.documents?.find(d => d.name === `${detail.task?.slug}.md`)?.size,
    designName: detail.documents?.find(d => d.name === `${detail.task?.slug}.md`)?.name,
  };
}
