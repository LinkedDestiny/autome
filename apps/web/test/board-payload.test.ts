import { ClockProvider } from '../app/lib/clock.tsx';
import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { boardColumns, loadBoard, type BoardData, type BoardTask } from '../app/components/board-data.ts';
import { loadShell } from '../app/components/shell-data.ts';
import { cardView, toCardDetail, type TaskDetail } from '../app/components/task-card-model.ts';
import type { KernelGateway } from '../app/load-context.ts';
import ProjectBoard from '../app/routes/projects.$id.tsx';
const now = '2026-11-14T12:00:00Z';
const project = JSON.parse(readFileSync(new URL('./fixtures/w5-project-get.json', import.meta.url), 'utf8'));
const detail = JSON.parse(readFileSync(new URL('./fixtures/w6-task-get.json', import.meta.url), 'utf8'));
beforeEach(() => { vi.useFakeTimers(); vi.setSystemTime(new Date(now)); });
afterEach(() => vi.useRealTimers());
function fixture() {
  const tasks: BoardTask[] = Array.from({ length: 100 }, (_, i) => ({
    ...project.tasks[i % project.tasks.length], id: `T-${i + 1}`, title: `任务 ${i + 1}`, kind: i % 2 ? 'bug' : 'feature',
    state: i < 10 ? { state: 'draft' } : i < 20 ? { state: 'queued' } : i < 30 ? { state: 'active', node: 'implement' } : i < 40 ? { state: 'active', node: 'await_merge' } : { state: 'done' },
    completed_at: i < 80 ? new Date(Date.parse(now) - (i - 40) * 60_000).toISOString() : '2020-01-01T00:00:00Z',
  }));
  const read = vi.fn<KernelGateway['read']>(async request => {
    const { method } = request as { method: string };
    return { status: 200, body: { ok: true, payload: method === 'project.get' ? { ...project, tasks }
      : method === 'project.list' ? { projects: [{ project: project.project }] }
      : method === 'dashboard.get' ? { cooldowns: [] } : detail } };
  });
  return { tasks, read, gateway: { read, write: vi.fn() } satisfies KernelGateway };
}
function render(board: BoardData) {
  const Stub = createRoutesStub([{ id: 'board', path: '/projects/:id', Component: () => createElement(ClockProvider, { initial: { now, timeZone: 'UTC' } }, createElement(ProjectBoard)) }]);
  return renderToStaticMarkup(createElement(Stub, { initialEntries: ['/projects/P-1'], hydrationData: { loaderData: { board: { ...board, alerts: [] } } } }));
}
async function measure(showAll: boolean) {
  const { gateway } = fixture();
  const board = await loadBoard({ gateway }, 'P-1', showAll);
  const shell = await loadShell(gateway);
  const bytes = Buffer.byteLength(JSON.stringify(board)) + Buffer.byteLength(JSON.stringify(shell));
  const document = bytes + Buffer.byteLength(render(board));
  console.info(`board payload ${showAll ? 'all' : 'default'}: data=${bytes}, document=${document} bytes`);
  expect(bytes).toBeLessThanOrEqual(200 * 1024);
  expect(document).toBeLessThanOrEqual(200 * 1024);
}
test('1 default document and revalidation stay under 200 KB with 100 tasks', () => measure(false));
test('2 all completed cards also stay under 200 KB', () => measure(true));
test('3 default completion cap, descending order, full count and reversible link', async () => {
  const { gateway } = fixture();
  for (const showAll of [false, true, false]) {
    const board = await loadBoard({ gateway }, 'P-1', showAll);
    if ('error' in board) throw Error(board.error);
    const done = boardColumns(board.tasks, now)[3];
    expect(done.total).toBe(60);
    expect(done.recent.map(t => t.id)).toEqual(Array.from({ length: 40 }, (_, i) => `T-${41 + i}`));
    const html = render(board);
    expect(html).toContain('已完成（60）');
    expect((html.match(/data-testid="task-card"/g) ?? []).length).toBe(showAll ? 100 : 80);
    expect(html).toContain(showAll ? '收起' : '更早 · 20 个');
  }
  for (const count of [0, 1, 19, 20, 21]) {
    const rows = fixture().tasks.slice(40, 40 + count);
    expect(boardColumns(rows, now)[3].recent).toHaveLength(count);
  }
});
test('4 completed tasks never fetch details; card projection preserves only used fields and running session', async () => {
  const { gateway, read } = fixture();
  for (const showAll of [false, true]) {
    read.mockClear();
    const board = await loadBoard({ gateway }, 'P-1', showAll);
    if ('error' in board) throw Error(board.error);
    const ids = read.mock.calls.map(([r]) => r as { method: string; params: { task_id: string } }).filter(r => r.method === 'task.get').map(r => r.params.task_id);
    expect(ids).toHaveLength(30);
    expect(ids.every(id => Number(id.slice(2)) >= 11 && Number(id.slice(2)) <= 40)).toBe(true);
    expect(Object.keys(board.details!)).toEqual(ids);
    for (const value of Object.values(board.details!)) {
      expect(JSON.parse(JSON.stringify(value))).toEqual({ gate: { ok: true, node: 'main_check', exit_code: 0 }, gate_queue_position: null, queue_position: null, retry: null, status_block: null });
    }
    expect(Object.keys(board.tasks[0]).sort()).toEqual(['id', 'title', 'kind', 'state', 'fix_round', 'completed_at', 'merge_commit', 'project_id', 'queue_position', 'after', 'blocked_by', 'created_at', 'cost_usd', 'metrics'].sort());
  }
  const input = { ...detail, sessions: [...detail.sessions, { ...detail.sessions[0], running: true }], retry: { attempts: 2, not_before: now, secret: 'omit' }, status_block: { milestones_done: 1, milestones_total: 2, current_milestone: 'M-01', secret: 'omit' } };
  const before = JSON.stringify(input), projected = toCardDetail(input);
  expect(projected.session).toEqual({ label: input.sessions.at(-1).label, round: 1, started_at: input.sessions.at(-1).started_at });
  expect(JSON.stringify(projected)).not.toMatch(/secret|log_path|sessions|worktree/);
  const context = { now, fixRounds: 3, cooldowns: [], unread: {} };
  for (const task of fixture().tasks.slice(10, 40)) expect(cardView(task, projected, context)).toEqual(cardView(task, input as TaskDetail, context));
  expect(JSON.stringify(input)).toBe(before);
  expect(toCardDetail({}).session).toBeUndefined();
});
test('5 todo cards omit gate rows while running cards keep them', async () => {
  const { gateway } = fixture();
  const board = await loadBoard({ gateway }, 'P-1');
  const html = render(board);
  const todo = html.split('<h2 id="board-todo">')[1].split('id="board-running"')[0];
  expect(todo).not.toContain('task-card-gate');
  expect(todo).not.toContain('门禁未跑过');
  expect(html.split('<h2 id="board-running">')[1]).toContain('task-card-gate');
});

test('6 100 tasks plus 40 expanded cancellations stay below 200 KB', async () => {
  const { tasks, gateway } = fixture();
  tasks.push(...Array.from({ length: 40 }, (_, i) => ({ id: 'T-' + (101 + i), title: '已取消任务', kind: 'feature', project_id: 'P-1', state: { state: 'cancelled' } })));
  const board = await loadBoard({ gateway }, 'P-1', true, true);
  const shell = await loadShell(gateway);
  const bytes = Buffer.byteLength(JSON.stringify(board)) + Buffer.byteLength(JSON.stringify(shell));
  expect(bytes + Buffer.byteLength(render(board))).toBeLessThanOrEqual(200 * 1024);
});
