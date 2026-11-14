import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test, vi } from 'vitest';
import { loadBoard, boardColumns, type BoardTask } from '../app/components/board-data.ts';
import { countByColumn } from '../app/components/task-columns.ts';
import type { KernelGateway } from '../app/load-context.ts';
import { ClockProvider } from '../app/lib/clock.tsx';
import Board from '../app/routes/projects.$id.tsx';
const filter = vi.hoisted(() => ({ kind: 'all' }));
vi.mock('react', async importOriginal => {
  const actual = await importOriginal<typeof import('react')>();
  return { ...actual, useState: (value: unknown) => actual.useState(value === 'all' ? filter.kind : value) };
});
const rows: BoardTask[] = ['T-2', 'T-010', 'T-100'].map((id, i) => ({ id, title: '取消 ' + id, kind: i === 1 ? 'bug' : 'feature', project_id: 'P-1', state: { state: 'cancelled' } }));
async function fixture(showCancelled = false, tasks = rows, search = '', kind = 'all') {
  filter.kind = kind;
  const read = vi.fn<KernelGateway['read']>(async () => ({ status: 200, body: { ok: true, payload: { project: { id: 'P-1', display_name: '项目' }, tasks } } }));
  const data = await loadBoard({ gateway: { read, write: vi.fn() } }, 'P-1', search.includes('done=all'), showCancelled);
  if ('error' in data) throw Error(data.error);
  const Stub = createRoutesStub([{ id: 'board', path: '/projects/:id', Component: () => createElement(ClockProvider, { initial: { now: data.now, timeZone: 'UTC' } }, createElement(Board)) }]);
  const html = renderToStaticMarkup(createElement(Stub, { initialEntries: ['/projects/P-1' + search], hydrationData: { loaderData: { board: { ...data, alerts: [] } } } }));
  return { data, html, read };
}
test('1 default only carries counts, with no cancelled objects or cards', async () => {
  const { data, html } = await fixture();
  expect(data.cancelledCounts).toEqual({ all: 3, feature: 2, bug: 1 });
  expect(data.tasks).toEqual([]); expect(data).not.toHaveProperty('cancelled');
  expect(html).toContain('已取消（3）'); expect(html).not.toContain('data-task-id=');
});
test('2 expanded numeric order, small projection and no detail reads', async () => {
  const { data, html, read } = await fixture(true);
  expect(data.cancelled?.map(t => t.id)).toEqual(['T-100', 'T-010', 'T-2']);
  for (const row of data.cancelled!) expect(Object.keys(row).sort()).toEqual(['id', 'kind', 'state', 'title']);
  expect(html).toContain('data-testid="board-cancelled"');
  for (const row of rows) expect(html).toContain('href="/tasks/' + row.id + '"');
  expect(html.indexOf('data-task-id="T-100"')).toBeLessThan(html.indexOf('data-task-id="T-010"'));
  expect(read).toHaveBeenCalledTimes(1);
  const single = await fixture(true, rows.slice(0, 1)); expect(single.data.cancelled).toHaveLength(1);
});
test('3 Bug filter changes both count and rendered list', async () => {
  expect((await fixture(false, rows, '', 'bug')).html).toContain('已取消（1）');
  const { html } = await fixture(true, rows, '', 'bug');
  expect(html).toContain('data-task-id="T-010"');
  expect(html).not.toContain('data-task-id="T-2"'); expect(html).not.toContain('data-task-id="T-100"');
});
test('4 both toggles preserve the other flag across all four combinations', async () => {
  const tasks = [...rows, { ...rows[0], id: 'T-200', state: { state: 'done' }, completed_at: '2000-01-01' }];
  for (const done of [false, true]) for (const cancelled of [false, true]) {
    const query = new URLSearchParams(); if (done) query.set('done', 'all'); if (cancelled) query.set('cancelled', 'all');
    const { html } = await fixture(cancelled, tasks, '?' + query);
    for (const key of ['done', 'cancelled']) {
      const next = new URLSearchParams(query); if (next.has(key)) next.delete(key); else next.set(key, 'all');
      expect(html).toContain('href="/projects/P-1' + (next.size ? '?' + next.toString().replaceAll('&', '&amp;') : '') + '"');
    }
  }
});
test('5 zero cancellations hide controls and never affect column counts', async () => {
  for (const expanded of [false, true]) {
    const { html } = await fixture(expanded, []);
    expect(html).not.toContain('data-testid="board-cancelled');
  }
  expect(boardColumns(rows, '2026-11-14').every(column => column.total === 0)).toBe(true);
  expect(Object.values(countByColumn(rows)).every(count => count === 0)).toBe(true);
});
