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
  for (const text of ['需求', 'T-1', '测试标题', '项目一', 'href="/projects/P-1"', 'autome/test']) expect(html).toContain(text);
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  const page = renderToStaticMarkup(createElement(Stub, { initialEntries: ['/tasks/T-1'], hydrationData: { loaderData: { task: await taskPageData(p) } } }));
  expect(page).toContain('task-detail-header'); expect(page).toContain('aria-current="step"'); expect(page).toContain('$1.23'); expect(page).toContain('1 个会话无计费');
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

import { SummarySteps } from '../app/components/task-summary.tsx';
import { taskSummary } from '../app/components/task-summary-model.ts';
import { availableOps, opsInput } from '../app/components/task-ops.ts';
import { WRITE_OPS } from '../app/components/task-action.ts';
test('已合并头部显示状态胶囊、合并时间和短提交', () => {
  const view = headerView({ ...p, task: { ...p.task, state: { state: 'done' }, completed_at: ctx.now, merge_commit: '12345678abcdef' } }, ctx);
  const html = renderToStaticMarkup(createElement(MemoryRouter, null, createElement(TaskHeader, { view })));
  expect(html).toContain('pill-green pill-fill'); expect(html).toContain('已合并'); expect(html).toContain('12345678'); expect(html).not.toContain('12345678abcdef');
});
test('头部保留全部可用写操作，包括重跑和预算追加', async () => {
  for (const state of [{state:'draft'}, {state:'active',node:'await_design_approval'}, {state:'active',node:'not_a_bug'}, {state:'paused',resume:'implement'}, {state:'failed',at:'gate',reason:{kind:'impl_budget'}}]) {
    const data = { ...p, task: { ...p.task, state } };
    const Stub = createRoutesStub([{ id:'task', path:'/tasks/:id', Component:TaskPage }]);
    const html = renderToStaticMarkup(createElement(Stub,{initialEntries:['/tasks/T-1'],hydrationData:{loaderData:{task:await taskPageData(data)}}}));
    const header = html.split('</header>')[0];
    const expected = availableOps(opsInput(data, null)).filter(o => o.op !== 'decide').map(o => o.op).sort();
    expect([...header.matchAll(/data-op="([^"]+)"/g)].map(m=>m[1]).sort()).toEqual(expected);
    for (const op of expected) expect(WRITE_OPS).toHaveProperty(op);
  }
});
test('你批准步骤使用黄色类且显示等待用时', () => {
  const html = renderToStaticMarkup(createElement(SummarySteps,{summary:taskSummary(p,[])}));
  expect(html).toContain('step-approval'); expect(html).toContain('等了 0 秒');
});
test('失败的单次门禁不能显示一次过', () => {
  const summary=taskSummary(p,[{seq:1,subject:'T-1',at:ctx.now,kind:'gate.result',payload:{node:'merge_gate',ok:false}}]);
  const html=renderToStaticMarkup(createElement(SummarySteps,{summary}));
  expect(html).not.toContain('一次过');expect(html).toContain('1 次');
});
