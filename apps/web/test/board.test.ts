import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test } from 'vitest';
import { boardColumns, boardHeading, filterBoard, loadBoard, type BoardData, type BoardTask } from '../app/components/board-data.ts';
import { columnOf, countByColumn } from '../app/components/task-columns.ts';
import { COLUMN_LABELS, type TaskState } from '../app/lib/labels.ts';
import type { KernelGateway } from '../app/load-context.ts';
import ProjectBoard from '../app/routes/projects.$id.tsx';
const now = '2026-11-13T00:00:00Z';
const task = (state: TaskState, id = 'T-1', kind = 'feature'): BoardTask => ({ id, kind, title: `任务 ${id}`, project_id: 'P-1', state });
const states: TaskState[] = [
  { state: 'draft' }, { state: 'queued' }, { state: 'queued', resume: 'gate' },
  ...['implement', 'gate', 'await_design_approval', 'await_merge', 'await_info', 'not_a_bug'].map(node => ({ state: 'active', node })),
  ...['paused', 'stopped', 'failed', 'done', 'cancelled'].map(state => ({ state })),
];
const tasks = states.map((state, i) => task(state, `T-${i + 1}`, i % 2 ? 'bug' : 'feature'));
function render(data: BoardData, url = '/projects/P-1') {
  const Stub = createRoutesStub([{ id: 'board', path: '/projects/:id', Component: ProjectBoard }]);
  return renderToStaticMarkup(createElement(Stub, { initialEntries: [url], hydrationData: { loaderData: { board: { ...data, alerts: [] } } } }));
}
const data = (rows = tasks, showAll = false): BoardData => ({ project: { id: 'P-1', display_name: '项目甲' }, tasks: rows, now, showAll });
test('1 fourteen state shapes reuse columnOf and omit unknown states', () => {
  expect(states).toHaveLength(14);
  for (const state of [...states, { state: 'future' }, { state: '' }]) {
    const columns = boardColumns([task(state)], now);
    expect(columns.filter(c => c.tasks.length).map(c => c.id)).toEqual(columnOf(state) ? [columnOf(state)] : []);
  }
  expect(boardColumns([], now).every(c => c.total === 0)).toBe(true);
  const sorted = boardColumns(['T-2', 'T-010', 'T-100'].map(id => task({ state: 'draft' }, id)), now);
  expect(sorted[0].tasks.map(t => t.id)).toEqual(['T-100', 'T-010', 'T-2']);
});
test('2 column order and labels match the shared vocabulary', () => {
  expect(boardColumns(tasks, now).map(c => [c.id, c.label])).toEqual(Object.entries(COLUMN_LABELS));
});
test('3 fourteen-day inclusive boundary and missing completion dates', () => {
  const rows = ['2026-10-30T00:00:00Z', '2026-10-29T23:59:59Z', null, '', '2026-11-12T00:00:00Z'].map((completed_at, i) => ({ ...task({ state: 'done' }, `T-${i}`), completed_at }));
  const done = boardColumns(rows, now)[3];
  expect(done.recent.map(t => t.id)).toEqual(['T-4', 'T-0']);
  expect(done.older.map(t => t.id)).toEqual(['T-1', 'T-2', 'T-3']);
  const boundary = '2027-01-01T00:00:00Z';
  expect(boardColumns([{ ...rows[0], completed_at: '2026-12-18T00:00:00Z' }], boundary)[3].recent).toHaveLength(1);
  expect(boardColumns([{ ...rows[0], completed_at: '2024-02-29T00:00:00Z' }], '2024-03-14T00:00:00Z')[3].recent).toHaveLength(1);
});
test('4 all filter transitions preserve columns, counts and source data', () => {
  const columns = boardColumns(tasks, now); const before = JSON.stringify(columns); const counts = countByColumn(tasks);
  for (const from of ['all', 'feature', 'bug'] as const) for (const kind of ['all', 'feature', 'bug'] as const) {
    filterBoard(columns, from);
    for (const c of filterBoard(columns, kind)) {
      expect(c.tasks).toEqual(columns.find(original => original.id === c.id)!.tasks.filter(t => kind === 'all' || t.kind === kind));
      expect(c.total).toBe(counts[c.id]);
      expect(boardHeading(c, kind)).toBe(`${COLUMN_LABELS[c.id]}（${kind === 'all' ? counts[c.id] : `${c.tasks.length} / ${counts[c.id]}`}）`);
      expect(c.tasks.every(t => columnOf(t.state) === c.id)).toBe(true);
    }
  }
  expect(JSON.stringify(columns)).toBe(before);
});
test('5 recorded project counts agree with shared classification', async () => {
  const fixture = JSON.parse(readFileSync(new URL('./fixtures/w5-project-get.json', import.meta.url), 'utf8'));
  const calls: { method: string; params?: unknown }[] = [];
  const gateway: KernelGateway = {
    read: async request => { const call = request as { method: string; params?: unknown }; calls.push(call); return { status: 200, body: { ok: true, payload: call.method === 'project.get' ? fixture : {} } }; },
    write: async () => { throw new Error('只读看板不应调用 write'); },
  };
  const loaded = await loadBoard({ gateway }, 'P-1');
  expect(calls.filter(call => call.method === 'project.get')).toEqual([{ method: 'project.get', params: { project_id: 'P-1' } }]);
  if ('error' in loaded) throw new Error(loaded.error);
  expect(Object.fromEntries(boardColumns(loaded.tasks, loaded.now).map(c => [c.id, c.total]))).toEqual(countByColumn(fixture.tasks));
});
test('6 SSR shows headings, filters, new link and reversible history folding', () => {
  const html = render(data());
  for (const c of boardColumns(tasks, now)) expect(html).toContain(boardHeading(c, 'all'));
  for (const label of ['全部', '需求', 'Bug']) expect(html).toContain(`>${label}</button>`);
  expect(html).toContain('href="/projects/P-1/tasks/new"');
  expect(html).toContain('href="/projects/P-1?done=all"'); expect(html).toContain('查看全部（1）');
  expect(html).not.toContain('href="/tasks/T-13"');
  const expanded = render(data(tasks, true), '/projects/P-1?done=all');
  expect(expanded).toContain('href="/tasks/T-13"'); expect(expanded).toMatch(/href="\/projects\/P-1"[^>]*>收起/);
  expect(render(data())).not.toContain('href="/tasks/T-13"');
  expect(render(data([]))).toContain('暂无任务');
});
test('7 gateway 503 and 409 remain inline errors without throwing', async () => {
  for (const [status, error] of [[503, '内核离线'], [409, '项目读取被拒绝']] as const) {
    const gateway: KernelGateway = {
      read: async () => ({ status, body: { ok: false, error } }),
      write: async () => { throw new Error('只读看板不应调用 write'); },
    };
    const result = await loadBoard({ gateway }, 'P-1'); expect(result).toEqual({ error }); expect(render(result)).toContain(error);
  }
  expect(render(await loadBoard({ gateway: null }, ''))).toContain('内核未连接');
  const gateway: KernelGateway = {
    read: async () => ({ status: 503, body: { ok: false } }),
    write: async () => { throw new Error('只读看板不应调用 write'); },
  };
  expect(render(await loadBoard({ gateway }, 'P-1'))).toContain('项目读取失败');
});
test('8 board source cannot duplicate state classification', () => {
  for (const path of ['components/board-data.ts', 'routes/projects.$id.tsx']) {
    expect(readFileSync(new URL(`../app/${path}`, import.meta.url), 'utf8')).not.toMatch(/['"](?:draft|queued|paused|stopped|failed)['"]/);
  }
});
