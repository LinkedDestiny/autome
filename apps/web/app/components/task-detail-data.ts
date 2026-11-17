import { taskSummary } from './task-summary-model.ts';
import { dependencyReadOnly } from './queue-deps.ts';
import { currentClock } from '../lib/clock.tsx';
import { headerView } from './task-header-model.ts';
import { noticeView } from './task-notice-model.ts';
import { stageBarView, type TaskConfig } from './task-stage-model.ts';
import { availableOps, opsInput } from './task-ops.ts';
import { docsView } from './task-docs-model.ts';
import { timelineEntries, timelineGroups } from './task-timeline-model.ts';
import type { CardContext } from './task-card-model.ts';
import type { BlockedDependency } from './queue-deps.ts';
import { detailTab } from './task-changes-model.ts';
import { parseMarkdown } from '../lib/markdown.ts';
import type { WebServices } from '../load-context.ts';
import type { TimelineSession } from './task-timeline-model.ts';
import type { TaskEvent } from '../../server/task-history.ts';
import { DOC_LIMIT_BYTES, pickDocument, type TaskDocument } from '../../server/task-docs.ts';
export type TaskPayload = {
  cost?: { usd_total: number; sessions: number; sessions_without_cost: number };
  gate?: { ok: boolean; node: string; exit_code: number } | null;
  gate_queue_position?: number | null;
  queue_position?: number | null;
  status_error?: boolean;
  status_error_detail?: string | null;
  status_block?: {
    milestones_done: number; milestones_total: number; current_milestone?: string | null;
    milestones?: { id: string; state: string; title?: string; reopen_count?: number }[];
    design_round: number; design_round_limit: number; impl_round: number; impl_round_limit: number;
  } | null;
  task: { merge_commit?: string | null; created_at?: string; completed_at?: string | null; metrics?: { reopen_total?: number; design_rounds_used?: number; design_rounds_limit?: number; impl_rounds_used?: number } | null; after?: string[]; blocked_by?: BlockedDependency[]; archived?: boolean; id: string; title: string; slug?: string; request?: string; kind?: string; branch?: string; fix_round?: number; budget_n?: number | null;
    state?: { state: string; node?: string; at?: string; resume?: string; reason?: Record<string, unknown> };
    merge?: { authorized?: boolean; stale?: { behind: number } | null; waiting_for?: string | null } | null };
  project?: { id: string; name?: string; display_name?: string };
  sessions: TimelineSession[];
  notes?: { n: number; text: string; at: string; delivered?: { role: string; round: number; at: string } | null }[];
  decisions?: { kind: string; item_id: string; consumed: boolean; disposition: string; text: string; ruling?: string; carried?: boolean }[];
  documents?: TaskDocument[];
  brief?: string | null;
  retry?: { attempts: number; not_before: string } | null;
};
export async function loadTaskDetail(services: WebServices, id: string, url: URL) {
  const offline = '内核离线，暂时拿不到任务';
  const { gateway } = services;
  if (!gateway) return { error: offline };
  const result = await gateway.read({ method: 'task.get', params: { task_id: id } });
  if (!result.body.ok) return { error: result.status === 503 || result.status === 504 ? offline
    : result.status === 404 || result.body.code === 'not_found' ? `找不到任务 ${id}` : `无法读取任务：${result.body.error}` };
  const data = result.body.payload as TaskPayload;
  async function read(method: string, params: Record<string, unknown>) {
    try {
      const response = await gateway!.read({ method, params });
      return response.body.ok ? { payload: response.body.payload } : { error: response.body.error };
    } catch (error) { return { error: String(error) }; }
  }
  const selected = detailTab(data, url.searchParams);
  const [config, history, dashboard, changes] = await Promise.all([
    data.project ? read('config.get', { project_id: data.project.id }) : Promise.resolve({ error: '角色状态未知' }),
    services.taskEvents ? services.taskEvents.forTask(id).catch(error => ({ events: [] as TaskEvent[], truncated: false, error: `内核事件暂时读不到：${String(error)}` }))
      : Promise.resolve({ events: [] as TaskEvent[], truncated: false, error: '内核事件暂时读不到：服务不可用' }),
    data.retry ? read('dashboard.get', {}) : undefined,
    selected === 'changes' ? read('task.changes', { task_id: id }) : undefined,
  ]);
  const name = url.searchParams.get('doc');
  let document: { name: string | null; text?: string; error?: string; size?: number; truncated?: boolean; missingBrief?: boolean };
  if (selected !== 'docs') document = { name };
  else if (name !== null) {
    const picked = pickDocument(data.documents ?? [], name);
    document = !picked.ok ? { name, error: picked.message } : services.taskDocs
      ? { name, ...await services.taskDocs.read(picked.entry.absolute, DOC_LIMIT_BYTES) }
      : { name, error: '文档读取服务不可用' };
  } else document = { name: null, text: data.brief ?? data.task.request ?? '', missingBrief: data.brief == null, truncated: false };
  const settings = config && 'payload' in config ? config.payload as TaskConfig : {};
  const clock = currentClock();
  const context: CardContext = { ...clock, fixRounds: data.task.budget_n ?? settings.resolved?.bug?.fix_rounds ?? 0,
    cooldowns: dashboard && 'payload' in dashboard ? (dashboard.payload as { cooldowns?: CardContext['cooldowns'] }).cooldowns ?? [] : [], unread: {} };
  const sessions = data.sessions.toSorted((a, b) => a.started_at.localeCompare(b.started_at));
  const pinned = sessions.findIndex(session => session.id === url.searchParams.get('from'));
  const start = url.searchParams.get('timeline') === 'all' ? 0 : pinned >= 0 ? pinned : Math.max(0, sessions.length - 20);
  const visible = new Set(sessions.slice(start).map(session => session.id));
  const entries = timelineEntries(data.sessions, history.events, clock.now, clock.timeZone)
    .filter(entry => entry.session ? visible.has(entry.session.id) : start === 0 || entry.at >= sessions[start].started_at)
    .map(entry => ({ ...entry, session: entry.session ? projectSession(entry.session) : undefined }));
  const { task } = data;
  const projected: TaskPayload = {
    task: { id: task.id, title: task.title, slug: task.slug, kind: task.kind, state: task.state,
      ...(Object.hasOwn(task, 'merge') ? { merge: task.merge } : {}), archived: task.archived, after: task.after, blocked_by: task.blocked_by },
    project: data.project ? { id: data.project.id } : undefined, sessions: [], status_block: data.status_block, notes: data.notes, decisions: data.decisions?.filter(d => !d.consumed),
  };
  return { summary: taskSummary(data, history.events, history.truncated || 'error' in history), data: projected, header: headerView(data, context), notice: noticeView(data, context),
    stage: stageBarView(data, history.events, settings), ops: availableOps(opsInput(data, 'payload' in config ? config.payload : null)),
    dependencyReason: dependencyReadOnly(data), docs: docsView(data), timeline: { entries, groups: timelineGroups(entries, url.searchParams.get('show')).map(group => ({ ...group, entries: group.entries.map(entry => entry.key) })), truncated: history.truncated, error: 'error' in history ? history.error : undefined },
    window: { from: sessions[start]?.id ?? null, earlier: start }, changes, document: { ...document, nodes: 'text' in document ? parseMarkdown(document.text ?? '') : [] } };
}

function projectSession(session: TimelineSession): TimelineSession {
  const { id, label, round, kind, runtime, model, started_at, ended_at, running, lifecycle, metrics } = session;
  return { id, label, round, kind, runtime, model, started_at, ended_at, running,
    lifecycle: lifecycle ? { state: lifecycle.state } : undefined, metrics: metrics ? { cost_usd: metrics.cost_usd } : metrics };
}
