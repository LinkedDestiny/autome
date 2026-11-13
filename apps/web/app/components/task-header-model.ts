import { cardView, type CardContext } from './task-card-model.ts';
import type { TaskPayload } from './task-detail-data.ts';
import { projectHref } from './nav-links.ts';

export function headerView(payload: TaskPayload, ctx: CardContext) {
  const task = { ...payload.task, kind: payload.task.kind ?? 'feature', state: payload.task.state ?? { state: 'draft' }, project_id: payload.project?.id ?? '' };
  const card = cardView(task, payload, ctx);
  const cost = payload.cost;
  return {
    id: task.id, title: task.title, kind: card.kind, branch: task.branch,
    project: payload.project ? { label: payload.project.name ?? payload.project.display_name ?? payload.project.id, href: projectHref(payload.project.id) } : null,
    cost: cost ? `费用 $${cost.usd_total.toFixed(2)}（${cost.sessions} 个会话${cost.sessions_without_cost > 0 ? `，其中 ${cost.sessions_without_cost} 个无计费数据` : ''}）` : null,
    activity: card.activity, gate: card.gate,
  };
}
