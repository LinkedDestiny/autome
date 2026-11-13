import { parseMarkdown } from '../lib/markdown.ts';
import type { WebServices } from '../load-context.ts';
import type { TimelineSession } from './task-timeline-model.ts';
import type { TaskEvent } from '../../server/task-events.ts';
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
    milestones?: { id: string; state: string; title?: string }[];
    design_round: number; design_round_limit: number; impl_round: number; impl_round_limit: number;
  } | null;
  task: { archived?: boolean; id: string; title: string; slug?: string; request?: string; kind?: string; branch?: string; fix_round?: number; budget_n?: number | null;
    state?: { state: string; node?: string; at?: string; resume?: string; reason?: Record<string, unknown> };
    merge?: { authorized?: boolean; stale?: { behind: number } | null; waiting_for?: string | null } | null };
  project?: { id: string; name?: string; display_name?: string };
  sessions: TimelineSession[];
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
  const mergeSegment = data.task.merge?.authorized || (data.task.state?.state === 'active'
    && ['await_merge', 'rebase', 'merge_gate', 'merging', 'main_check', 'cleanup'].includes(data.task.state.node ?? ''));
  const [config, history, dashboard, changes] = await Promise.all([
    data.project ? read('config.get', { project_id: data.project.id }) : Promise.resolve({ error: '角色状态未知' }),
    services.taskEvents ? services.taskEvents.forTask(id).catch(error => ({ events: [] as TaskEvent[], truncated: false, error: `内核事件暂时读不到：${String(error)}` }))
      : Promise.resolve({ events: [] as TaskEvent[], truncated: false, error: '内核事件暂时读不到：服务不可用' }),
    data.retry ? read('dashboard.get', {}) : undefined,
    mergeSegment ? read('task.changes', { task_id: id }) : undefined,
  ]);
  const name = url.searchParams.get('doc');
  let document: { name: string | null; text?: string; error?: string; size?: number; truncated?: boolean; missingBrief?: boolean };
  if (name !== null) {
    const picked = pickDocument(data.documents ?? [], name);
    document = !picked.ok ? { name, error: picked.message } : services.taskDocs
      ? { name, ...await services.taskDocs.read(picked.entry.absolute, DOC_LIMIT_BYTES) }
      : { name, error: '文档读取服务不可用' };
  } else document = { name: null, text: data.brief ?? data.task.request ?? '', missingBrief: data.brief == null, truncated: false };
  return { data, config, history, dashboard, changes, document: { ...document, nodes: 'text' in document ? parseMarkdown(document.text ?? '') : [] } };
}
