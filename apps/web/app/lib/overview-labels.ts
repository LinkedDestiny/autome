import { taskHref } from '../components/nav-links.ts';
import { AWAIT_PHRASES, failureText, gatePhrase, stageStatusLine } from './labels.ts';
import { formatSpan } from './clock.tsx';
import type { ShellTask } from '../components/shell-data.ts';
import type { OverviewDetail } from '../../server/overview.ts';
export const OVERVIEW = {
  today: '今天', merged: '合并', recent: '最近合并', emptyMerged: '还没有已合并的任务',
  emptyWait: '都处理完了', emptyWaitBody: '设计批准、Backlog 表态、失败的任务会出现在这里。',
  emptyRun: '没有任务在跑', emptyRunBody: '新建一张任务卡，驱动会在 3 秒内接手。',
  progress: '里程碑进度', designRound: '设计轮', designDoc: '设计文档',
};
export const WAIT_BADGES: Record<string, string> = { await_design_approval: '批准设计', await_merge: '批准合并', await_info: '补充信息', not_a_bug: '确认不是 Bug', failed: '处理失败', paused: '已暂停', stopped: '已停止' };
export function overviewCardView(task: ShellTask, detail: OverviewDetail | undefined, now: string) {
  const key = task.state.state === 'active' ? task.state.node! : task.state.state;
  const block = detail?.status_block, session = detail?.session;
  const impl = session && session.kind?.kind === 'role' && ['impl', 'audit', 'retro'].includes(session.kind.role);
  const round = block ? `${(impl ? block.impl_round : block.design_round) ?? '—'} / ${(impl ? block.impl_round_limit : block.design_round_limit) ?? '—'}` : session ? String(session.round ?? '—') : '';
  const summary = detail?.error ?? (key === 'await_design_approval'
    ? block ? `设计已定稿：${block.milestones_total} 个里程碑。评审 ${detail!.reviews} 轮、裁决 ${detail!.adjudications} 轮。${detail!.pending ? `另有 ${detail!.pending} 条 Backlog 等你表态。` : ''}` : '设计摘要暂不可用'
    : key === 'failed' ? failureText((task.state as ShellTask['state'] & { reason?: Record<string, unknown> }).reason) || '任务失败，请查看详情'
    : AWAIT_PHRASES[key as keyof typeof AWAIT_PHRASES] ?? '');
  return { summary, badge: WAIT_BADGES[key] ?? (session ? `${session.label} ${round}` : stageStatusLine(task.kind, task.state)),
    reviewDocuments: key === 'await_design_approval' ? [
      { label: '查看评审', missing: '评审结果摘要不可得', name: detail?.reviewName },
      { label: '查看裁决', missing: '裁决结果摘要不可得', name: detail?.adjudicationName },
    ].map(doc => ({ ...doc, href: doc.name ? `${taskHref(task.id)}?tab=docs&doc=${encodeURIComponent(doc.name)}` : undefined })) : [],
    cost: task.cost_usd == null || (task.cost_usd === 0 && (task.cost_sessions_without_cost ?? 0) > 0) ? '无计费' : `$${task.cost_usd.toFixed(2)}`,
    elapsed: task.created_at ? formatSpan(Date.parse(now) - Date.parse(task.created_at)) : '—',
    round: block ? `${block.design_round ?? '—'} / ${block.design_round_limit ?? '—'}` : '—',
    sessionTime: session ? `本轮已跑 ${formatSpan(Date.parse(now) - Date.parse(session.started_at))}` : '',
    progress: block ? `里程碑 ${block.milestones_done ?? '—'} / ${block.milestones_total ?? '—'}` : '',
    current: block?.current_milestone ? `正在做 ${block.current_milestone}` : '',
    gate: detail?.gate ? gatePhrase(detail.gate) : '',
    queue: task.state.state === 'queued' ? task.blocked_by?.length ? '等 ' + task.blocked_by.map(item => `${item.id} ${item.reason === 'cancelled' ? '已取消，请修改依赖' : item.reason === 'failed' ? '已失败，请处理依赖' : '合并后开始'}`).join('、') : `排第 ${task.queue_position ?? '—'} 位 · 等空闲槽位` : '',
    href: `${taskHref(task.id)}${key === 'await_design_approval' ? `?doc=${encodeURIComponent(detail?.designName ?? 'design')}` : key === 'await_merge' ? '?tab=changes' : ''}`,
    action: key === 'await_design_approval' ? '看设计并批准' : key === 'await_merge' ? '看改动并合并' : '查看并处理',
  };
}
export function endedWaiting(ended: Record<string, number>) { return Object.entries(ended).map(([kind, n]) => `${WAIT_BADGES[kind] ?? kind} ${n} 次`).join(' · '); }
