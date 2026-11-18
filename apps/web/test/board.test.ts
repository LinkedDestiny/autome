import { ClockProvider } from '../app/lib/clock.tsx';
import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test } from 'vitest';
import { boardColumns, boardHeading, doneGroup, doneRow, doneTotals, filterBoard, loadBoard, type BoardData, type BoardTask } from '../app/components/board-data.ts';
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
  const Stub = createRoutesStub([{ id: 'board', path: '/projects/:id', Component: () => createElement(ClockProvider, { initial: { now, timeZone: 'UTC' } }, createElement(ProjectBoard)) }]);
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
test('3 date groups and missing completion dates', () => {
  const rows = ['2026-10-30T00:00:00Z', '2026-10-29T23:59:59Z', null, '', '2026-11-12T00:00:00Z'].map((completed_at, i) => ({ ...task({ state: 'done' }, `T-${i}`), completed_at }));
  const done = boardColumns(rows, now)[3];
  expect(done.recent.map(t => t.id)).toEqual(['T-4']);
  expect(done.older.map(t => t.id)).toEqual(['T-0', 'T-1', 'T-2', 'T-3']);
  const boundary = '2027-01-01T00:00:00Z';
  expect(boardColumns([{ ...rows[0], completed_at: '2026-12-31T00:00:00Z' }], boundary)[3].recent).toHaveLength(1);
  expect(boardColumns([{ ...rows[0], completed_at: '2024-02-29T00:00:00Z' }], '2024-03-01T00:00:00Z')[3].recent).toHaveLength(1);
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
  expect(html).toContain('href="/projects/P-1?done=all"'); expect(html).toContain('更早 · 1 个');
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

test('9 坏配置下卡片仍渲染且不带详情', async () => {
  const gateway: KernelGateway = { read: async input => (input as { method: string }).method === 'project.get' ? { status: 200, body: { ok: true, payload: { project: { id: 'P-1', display_name: '坏项目' }, tasks: [task({ state: 'active', node: 'implement' })], config_error: 'TOML 原文' } } } : { status: 409, body: { ok: false, error: 'TOML 原文' } }, write: async () => { throw new Error('只读'); } };
  const result = await loadBoard({ gateway }, 'P-1'); expect(result).not.toHaveProperty('error'); expect(result).toHaveProperty('details', {}); expect(render(result)).toContain('任务 T-1');
});
test('10 页头显示配置校验原文', () => { const html = render({ ...data([]), configError: 'TOML 原文 <bad>' }); expect(html).toContain('项目配置有误，以下是内核的校验原文'); expect(html).toContain('TOML 原文 &lt;bad&gt;'); });

test('11 排队按内核位置而不是编号排序并透传依赖字段', async () => {
  const rows = [3, 1, 2].map((n, i) => ({ ...task({ state: 'queued' }, `T-${n}`), queue_position: i + 1, after: ['T-9'], blocked_by: [{ id: 'T-9', state: { state: 'draft' }, reason: null }] }));
  const gateway: KernelGateway = { read: async () => ({ status: 200, body: { ok: true, payload: { project: { id: 'P-1', display_name: '甲' }, tasks: rows } } }), write: async () => { throw Error('只读'); } };
  const loaded = await loadBoard({ gateway }, 'P-1');
  if ('error' in loaded) throw Error(loaded.error);
  expect(loaded.tasks[0]).toMatchObject({ queue_position: 1, after: ['T-9'], blocked_by: rows[0].blocked_by });
  expect(boardColumns(loaded.tasks, now)[0].tasks.map(t => t.id)).toEqual(['T-3', 'T-1', 'T-2']);
});
test('12 排队在前草稿编号倒序在后含空队列和单元素', () => {
  const rows = [task({ state: 'draft' }, 'T-100'), { ...task({ state: 'queued' }, 'T-1'), queue_position: 1 }, task({ state: 'draft' }, 'T-010')];
  expect(boardColumns(rows, now)[0].tasks.map(t => t.id)).toEqual(['T-1', 'T-100', 'T-010']);
  expect(boardColumns([rows[1]], now)[0].tasks).toEqual([rows[1]]);
});
test('13 只有排队且队列多于一张才给调序菜单', () => {
  const q = task({ state: 'queued' });
  expect(render(data([q]))).not.toContain('<summary class="btn btn-sm">调整顺序');
  const html = render(data([q, task({ state: 'queued' }, 'T-2'), task({ state: 'draft' }, 'T-3')]));
  expect(html.match(/<summary class="btn btn-sm">调整顺序/g)).toHaveLength(2);
});

test('14 四个列头使用小号彩色丝带及按钮提示', () => {
  const html = render(data([])); for (const color of ['brown', 'teal', 'yellow', 'green']) expect(html).toContain('ribbon-' + color + ' ribbon-sm');
  expect(html).toContain('用卡片上的按钮调整顺序');
});
test('15 今天昨天本周更早与更早按服务端时区', () => {
  const now = '2026-11-18T16:00:00Z';
  expect(['2026-11-18T16:00:00Z','2026-11-18T15:59:59Z','2026-11-15T16:00:00Z','2026-11-15T15:59:59Z'].map(at => doneGroup(at, now, 'Asia/Shanghai'))).toEqual(['今天','昨天','本周更早','更早']);
});
test('16 周一零点与跨年闰日空日期边界', () => {
  expect(doneGroup('2026-11-14T12:00:00Z', '2026-11-16T00:00:00Z', 'UTC')).toBe('更早');
  expect(doneGroup('2026-11-15T12:00:00Z', '2026-11-16T00:00:00Z', 'UTC')).toBe('昨天');
  expect(doneGroup('2025-12-29T00:00:00Z', '2026-01-01T00:00:00Z', 'UTC')).toBe('本周更早');
  expect(doneGroup('2024-02-29T00:00:00Z', '2024-03-01T00:00:00Z', 'UTC')).toBe('昨天');
  for (const at of ['', null, undefined, 'invalid']) expect(doneGroup(at, now, 'UTC')).toBe('更早');
});
test('17 紧凑条目仅六字段 Bug 与缺失指标显示破折号', () => {
  const t = { ...task({ state: 'done' }), cost_usd: 2, metrics: { impl_rounds_used: 3, reopen_total: 1 } };
  expect(doneRow(t)).toEqual({ id: 'T-1', title: '任务 T-1', completed_at: undefined, rounds: 3, reopens: 1, cost: '$2.00' });
  expect(doneRow({ ...t, kind: 'bug' }).rounds).toBe('—');
  expect(doneRow(task({ state: 'done' }))).toMatchObject({ rounds: '—', reopens: '—', cost: '无计费' });
});
test('18 累计包含 Bug 不含取消 平均只含有指标需求', () => {
  const t = { ...task({ state: 'done' }), cost_usd: 2, metrics: { impl_rounds_used: 4 } };
  expect(doneTotals([t, { ...t, kind: 'bug' }, { ...t, metrics: null }, { ...t, state: { state: 'cancelled' }, cost_usd: 100 }])).toEqual({ cost: '$6.00', average: '4.0' });
  expect(doneTotals([])).toEqual({ cost: '$0.00', average: '—' }); expect(doneTotals([t]).average).toBe('4.0');
});
test('19 展开紧凑条目无阶段条且保留筛选查询', () => {
  const t = { ...task({ state: 'done' }), metrics: { impl_rounds_used: 7, reopen_total: 2 }, cost_usd: 4 };
  const html = render(data([t], true), '/projects/P-1?done=all&cancelled=all');
  expect(html).toContain('实现 7 轮 · 退回 2 · $4.00'); expect(html).not.toContain('task-card-stages');
  expect(html).toContain('/projects/P-1?cancelled=all'); expect(html).not.toContain('门禁未跑过');
});
