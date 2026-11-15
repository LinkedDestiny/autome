import { expect, test, vi } from 'vitest';
import { loadShell, overviewTasks } from '../app/components/shell-data.ts';
import { countByColumn } from '../app/components/task-columns.ts';
import { createLoadContext, type KernelGateway } from '../app/load-context.ts';
import { loader } from '../app/root.tsx';
import type { LoaderFunctionArgs } from 'react-router';
const states = [
  { state: 'draft' }, { state: 'queued' }, { state: 'queued', resume: 'gate' },
  ...['implement', 'gate', 'await_design_approval', 'await_merge', 'await_info', 'not_a_bug'].map(node => ({ state: 'active', node })),
  { state: 'paused', resume: 'gate' }, { state: 'stopped', at: 'gate' }, { state: 'failed', at: 'gate' }, { state: 'done' }, { state: 'cancelled' }, { state: 'future' },
];
const tasks = states.map((state, i) => ({ id: `T-${i + 1}`, kind: 'feature', title: `任务 ${i}`, state, request: 'private request', metrics: { secret: true }, ...(i === 0 ? { fix_round: 0 } : {}) }));
function gateway(fail?: string): KernelGateway {
  return { read: vi.fn(async (input: unknown) => {
    const { method, params } = input as { method: string; params: { project_id?: string } };
    if (fail !== undefined && (fail === method || fail === params.project_id)) return { status: 504, body: { ok: false, error: '内核响应超时' } };
    return { status: 200, body: { ok: true, payload: method === 'project.list' ? { projects: ['P-1', 'P-2'].map(id => ({ project: { id, display_name: id }, counts: { awaiting_user: 0 } })) } : { tasks: params.project_id === 'P-1' ? tasks : [] } } };
  }), write: vi.fn() };
}
test('1 reads list once and each detail once through the gateway', async () => {
  const g = gateway(); await loadShell(g);
  expect(vi.mocked(g.read).mock.calls).toEqual([[{ method: 'project.list', params: {} }], ...['P-1', 'P-2'].map(project_id => [{ method: 'project.get', params: { project_id } }]), [{ method: 'dashboard.get', params: {} }]]);
  expect(g.write).not.toHaveBeenCalled();
});
test('2 shared classification drives counts and exact sorted overview membership', async () => {
  const shell = await loadShell(gateway());
  const counts = countByColumn(tasks); expect(shell.projects[0].counts).toEqual({ running: counts.running, needs_you: counts.needs_you });
  expect(shell.projects[1].counts).toEqual({ running: 0, needs_you: 0 });
  expect(overviewTasks(shell.tasks, 'needs_you').map(t => t.id)).toEqual(['T-12', 'T-11', 'T-10', 'T-9', 'T-8', 'T-7', 'T-6']);
  expect(overviewTasks(shell.tasks, 'running').map(t => t.id)).toEqual(['T-5', 'T-4']);
  expect(shell.tasks).toHaveLength(states.length);
  const one = shell.tasks[3]; expect(overviewTasks([{ ...one, id: 'T-02' }, { ...one, id: 'T-10' }], 'running').map(t => t.id)).toEqual(['T-10', 'T-02']);
  expect(overviewTasks([], 'running')).toEqual([]);
});
test('3 detail failures stay local, preserve reasons and recover on the next load', async () => {
  for (const [status, error] of [[400, '已拒绝：禁止读取'], [409, '读盘失败'], [504, '内核响应超时'], [503, '内核未连接：离线']] as const) {
    const g = gateway(); const read = g.read;
    g.read = vi.fn(input => (input as { params: { project_id?: string } }).params.project_id === 'P-2' ? Promise.resolve({ status, body: { ok: false, error } }) : read(input));
    const shell = await loadShell(g); expect(shell.kernel.ok).toBe(true); expect(shell.projects[0].counts?.running).toBe(2);
    expect(shell.projects[1]).toEqual({ id: 'P-2', name: 'P-2', counts: null, error });
    g.read = read; expect((await loadShell(g)).projects[1].counts).toEqual({ running: 0, needs_you: 0 });
  }
});
test('4 list failures and missing gateway return an offline empty shell', async () => {
  expect(await loadShell(gateway('project.list'))).toEqual({ kernel: { ok: false, error: '内核响应超时' }, projects: [], tasks: [] });
  expect((await loadShell(null)).kernel.ok).toBe(false);
  const g = gateway(); vi.mocked(g.read).mockResolvedValue({ status: 200, body: { ok: true, payload: { projects: [] } } });
  expect(await loadShell(g)).toEqual({ kernel: { ok: true }, projects: [], tasks: [] });
});
test('5 only whitelisted task fields leave the loader, including zero fix_round', async () => {
  const shell = await loadShell(gateway());
  shell.tasks.forEach((task, i) => expect(Object.keys(task).sort()).toEqual(['id', 'kind', 'title', 'project_id', 'state', ...(i === 0 ? ['fix_round'] : [])].sort()));
  expect(shell.tasks[0].fix_round).toBe(0); expect(JSON.stringify(shell)).not.toContain('private request');
});
test('6 root loader returns the same shell through RouterContextProvider', async () => {
  const g = gateway(); const args = { context: createLoadContext({ gateway: g })(), request: new Request('http://localhost/'), params: {}, url: new URL('http://localhost/'), pattern: '/' } satisfies LoaderFunctionArgs;
  vi.useFakeTimers();
  const now = '2026-11-14T00:00:00.000Z';
  vi.setSystemTime(new Date(now));
  try {
    expect(await loader(args)).toEqual({ ...await loadShell(g), clock: { now, timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone } });
  } finally { vi.useRealTimers(); }
});

test('7 config_error 标记来自 dashboard 且修复后消失', async () => {
  const g = gateway(), read = g.read; let broken = true;
  g.read = vi.fn(input => (input as { method: string }).method === 'dashboard.get' ? Promise.resolve({ status: 200, body: { ok: true, payload: { projects: [{ project: { id: 'P-1' }, config_error: broken ? 'TOML 原文' : null }] } } }) : read(input));
  expect((await loadShell(g)).projects[0].configError).toBe('TOML 原文'); broken = false; expect((await loadShell(g)).projects[0].configError).toBeUndefined();
});
test('8 一个坏配置不影响其它项目与任务', async () => {
  const g = gateway(), read = g.read;
  g.read = vi.fn(input => (input as { method: string }).method === 'dashboard.get' ? Promise.resolve({ status: 200, body: { ok: true, payload: { projects: [{ project: { id: 'P-2' }, config_error: '坏配置' }] } } }) : read(input));
  const result = await loadShell(g); expect(result.projects[0].counts?.running).toBe(2); expect(result.tasks).toHaveLength(tasks.length); expect(result.projects[0].configError).toBeUndefined(); expect(result.projects[1].configError).toBe('坏配置'); expect(result.projects[1].alerts).toBeUndefined();
});
