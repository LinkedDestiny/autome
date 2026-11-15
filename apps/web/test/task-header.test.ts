import { taskPageData } from './fixtures/task-page-data.ts';
import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter, createRoutesStub } from 'react-router';
import { expect, test } from 'vitest';
import { headerView } from '../app/components/task-header-model.ts';
import { TaskHeader } from '../app/components/task-header.tsx';
import { cardView, type CardContext } from '../app/components/task-card-model.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import TaskPage from '../app/routes/tasks.$id.tsx';
const ctx: CardContext = { now: '2026-11-13T06:20:00Z', fixRounds: 3, cooldowns: [], unread: {} };
const p: TaskPayload = { task: { id: 'T-1', title: '测试标题', kind: 'feature', branch: 'autome/test', state: { state: 'active', node: 'implement' } }, project: { id: 'P-1', display_name: '项目一' }, sessions: [], cost: { usd_total: 1.234, sessions: 3, sessions_without_cost: 1 } };
test('1 header fields, project link and CLI cost accounting rendered on page', async () => {
  const recorded = JSON.parse(readFileSync(new URL('./fixtures/w7/feature.json', import.meta.url), 'utf8'));
  expect(headerView(recorded, ctx).project?.label).toBe(recorded.project.name);
  const v = headerView(p, ctx);
  const html = renderToStaticMarkup(createElement(MemoryRouter, null, createElement(TaskHeader, { view: v })));
  for (const text of ['需求', 'T-1', '测试标题', '项目一', 'href="/projects/P-1"', 'autome/test', '费用 $1.23（3 个会话，其中 1 个无计费数据）']) expect(html).toContain(text);
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  const page = renderToStaticMarkup(createElement(Stub, { initialEntries: ['/tasks/T-1'], hydrationData: { loaderData: { task: await taskPageData(p) } } }));
  expect(page).toContain('task-detail-header'); expect(page).toContain('aria-current="step"'); expect(page).toContain(v.cost);
});
test('2 activity and gate delegate to cardView for running and waiting states', () => {
  for (const state of [{ state: 'active', node: 'implement' }, { state: 'active', node: 'await_design_approval' }, { state: 'queued' }, { state: 'failed', at: 'audit', reason: { kind: 'protocol', detail: '错误' } }]) {
    const data = { ...p, task: { ...p.task, state } };
    const card = cardView({ ...data.task, kind: 'feature', project_id: 'P-1' }, data, ctx);
    expect(headerView(data, ctx).activity).toBe(card.activity); expect(headerView(data, ctx).gate).toBe(card.gate);
  }
});
test('3 no missing-cost annotation when all sessions billed including zero', () => {
  expect(headerView({ ...p, cost: { usd_total: 0, sessions: 0, sessions_without_cost: 0 } }, ctx).cost).toBe('费用 $0.00（0 个会话）');
  expect(headerView({ ...p, cost: { usd_total: 42, sessions: 2, sessions_without_cost: 0 } }, ctx).cost).toBe('费用 $42.00（2 个会话）');
});
