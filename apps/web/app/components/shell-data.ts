import type { KernelGateway } from '../load-context.ts';
import type { TaskState } from '../lib/labels.ts';
import { columnOf, countByColumn } from './task-columns.ts';

export interface ShellTask { id: string; kind: string; title: string; project_id: string; state: TaskState; fix_round?: number }
export interface ShellProject { id: string; name: string; counts: { running: number; needs_you: number } | null; error?: string }
export interface ShellData { kernel: { ok: boolean; error?: string }; projects: ShellProject[]; tasks: ShellTask[] }
export async function loadShell(gateway: KernelGateway | null): Promise<ShellData> {
  if (!gateway) return { kernel: { ok: false, error: '内核未连接：服务未接入网关' }, projects: [], tasks: [] };
  const result = await gateway.read({ method: 'project.list', params: {} });
  if (!result.body.ok) return { kernel: { ok: false, error: result.body.error }, projects: [], tasks: [] };
  const payload = result.body.payload as { projects: { project: { id: string; display_name: string } }[] };
  const loaded = await Promise.all(payload.projects.map(async ({ project }) => {
    const base = { id: project.id, name: project.display_name };
    const detail = await gateway.read({ method: 'project.get', params: { project_id: project.id } });
    if (!detail.body.ok) return { project: { ...base, counts: null, error: detail.body.error }, tasks: [] };
    const tasks = (detail.body.payload as { tasks: ShellTask[] }).tasks.map(({ id, kind, title, state, fix_round }) => ({
      id, kind, title, project_id: project.id, state, ...(fix_round === undefined ? {} : { fix_round }),
    }));
    const { running, needs_you } = countByColumn(tasks);
    return { project: { ...base, counts: { running, needs_you } }, tasks };
  }));
  return { kernel: { ok: true }, projects: loaded.map(item => item.project), tasks: loaded.flatMap(item => item.tasks) };
}
export function overviewTasks(tasks: ShellTask[], column: 'needs_you' | 'running'): ShellTask[] {
  return tasks.filter(task => columnOf(task.state) === column).sort((a, b) => Number(b.id.match(/\d+/)?.[0] ?? 0) - Number(a.id.match(/\d+/)?.[0] ?? 0));
}
