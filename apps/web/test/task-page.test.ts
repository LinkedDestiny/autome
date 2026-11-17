import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createRoutesStub, RouterContextProvider, type LoaderFunctionArgs } from 'react-router';
import { readFileSync } from 'node:fs';
import { expect, it, vi } from 'vitest';
import TaskPage, { loader, shouldRevalidate } from '../app/routes/tasks.$id.tsx';
import { servicesContext, type KernelGateway } from '../app/load-context.ts';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import { TARGETS } from '../app/components/bell-targets.ts';
function load(gateway: KernelGateway | null) {
  const context = new RouterContextProvider(); context.set(servicesContext, { gateway });
  return loader({ context, params: { id: 'T-1' }, request: new Request('http://localhost/tasks/T-1'), url: new URL('http://localhost/tasks/T-1'), pattern: '/tasks/:id' } as LoaderFunctionArgs);
}
function render(data: Awaited<ReturnType<typeof loader>>, query = '') {
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  return renderToString(createElement(Stub, { initialEntries: [`/tasks/T-1${query}`], hydrationData: { loaderData: { task: data } } }));
}
it('任务路由存在，铃铛目标同步开放', () => {
  expect(readFileSync(new URL('../app/routes.ts', import.meta.url), 'utf8')).toMatch(/route\("tasks\/:id", "routes\/tasks\.\$id\.tsx"\)/);
  expect(readFileSync(new URL('../app/components/overview-card.tsx', import.meta.url), 'utf8')).toContain('overviewCardView');
  expect(readFileSync(new URL('../app/lib/overview-labels.ts', import.meta.url), 'utf8')).toContain('taskHref(task.id)');
  expect(TARGETS.find(t => t.pattern === '/tasks/:taskId')?.available).toBe(true);
});
it('离线、503/504 和 not_found/404 分支呈现中文', async () => {
  expect(render(await load(null))).toContain('内核离线，暂时拿不到任务');
  for (const [status, code, expected] of [[503, '', '内核离线，暂时拿不到任务'], [504, '', '内核离线，暂时拿不到任务'], [400, 'not_found', '找不到任务'], [404, '', '找不到任务'], [500, '', '无法读取任务：失败']] as const) {
    expect(render(await load({ read: vi.fn().mockResolvedValue({ status, body: { ok: false, code, error: '失败' } }), write: vi.fn() }))).toContain(expected);
  }
});
it('仅本任务八类事件触发重验', async () => {
  const url = new URL('http://localhost/tasks/T-1');
  for (const kind of ['task.updated', 'session.started', 'gate.result', 'bug.located', 'merge.auto', 'rebase.conflicts', 'guard.warning', 'runtime.cooldown', 'project.updated']) for (const subject of ['T-1', 'T-2']) {
    await revalidateForEvents([{ seq: 1, kind, subject, payload: {} }], () => {
      expect(shouldRevalidate({ currentUrl: url, nextUrl: url, currentParams: { id: 'T-1' }, defaultShouldRevalidate: false } as never)).toBe(subject === 'T-1' && kind !== 'project.updated');
    });
  }
});

it('四个入口共用任务详情地址', () => {
  for (const file of ['task-card.tsx', 'palette-model.ts']) expect(readFileSync(new URL('../app/components/' + file, import.meta.url), 'utf8')).toContain('taskHref');
  expect(TARGETS.find(t => t.pattern === '/tasks/:taskId')?.available).toBe(true);
  expect(readFileSync(new URL('../app/components/nav-links.ts', import.meta.url), 'utf8')).toContain('/tasks/');
});
it('页面定高、两栏与每个内容区内部滚动', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  const rule = (selector: string) => [...css.matchAll(/([^{}]+)\{([^}]+)\}/g)].filter(([,name]) => name.trim() === selector).map(([, ,body])=>body).join(' ');
  expect(rule('.task-page')).toContain('height: 100%'); expect(rule('.task-page')).toContain('overflow: hidden');
  expect(rule('.task-detail-body')).toContain('grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr)');
  for(const selector of ['.task-detail-body','.task-detail-panel'])expect(rule(selector)).toContain('min-height: 0');
  for(const selector of ['.task-timeline','.task-docs-body','.task-changes','.task-decisions','.task-controls']) {expect(rule(selector)).toContain('min-height: 0');expect(rule(selector)).toContain('overflow-y: auto');}
});

it('三个标签接入真实页面且文档链接不会回到默认合并标签', async () => {
  const payload = JSON.parse(readFileSync(new URL('./fixtures/w7/feature.json', import.meta.url), 'utf8'));
  const changes = JSON.parse(readFileSync(new URL('./fixtures/w7/changes.json', import.meta.url), 'utf8'));
  const data = await load({read: vi.fn(async (request: unknown) => { const {method} = request as {method:string}; return {status: 200, body: {ok: true, payload: method === 'task.get' ? payload : method === 'task.changes' ? changes : {}}}; }),write:vi.fn()});
  expect(render(data)).toContain('class="task-changes"');
  expect(render(data,'?tab=decisions')).toContain('下一轮生效');
  const docs = render(data,'?tab=docs');expect(docs).toContain('class="task-docs"');expect(docs).toContain('tab=docs');
  expect(render(data,'?doc=feat.md')).toContain('class="task-docs"');
});

import { taskPageData } from './fixtures/task-page-data.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
const detailFixture = (): TaskPayload => JSON.parse(readFileSync(new URL('./fixtures/w7/feature.json', import.meta.url), 'utf8'));
it('头部、九步骤与六项数字块按阅读顺序排列', async () => {
 const html=render(await taskPageData(detailFixture()));
 expect(html.indexOf('task-detail-header')).toBeLessThan(html.indexOf('summary-steps'));
 expect(html.indexOf('summary-steps')).toBeLessThan(html.indexOf('task-summary-tiles'));
 expect(html.match(/class="tile tile-/g)).toHaveLength(6);
 for(const label of ['花费','总用时','会话数','设计轮','实现轮','门禁'])expect(html).toContain(label);
});
it('Bug 六步骤和空会话仍显示统计与空执行提示', async () => {
 const data=detailFixture();data.task.kind='bug';data.sessions=[];
 const html=render(await taskPageData(data));
 for(const label of ['定位','复现检查','修复','修复门禁','复核','尚未合并'])expect(html).toContain(label);
 expect(html).not.toContain('step-approval');expect(html).toContain('还没有执行记录');
});
it('四页签保留计数和备注送回实现入口', async () => {
 const data=detailFixture();data.task.state={state:'active',node:'await_merge'};
 const html=render(await taskPageData(data),'?tab=notes');
 expect(html).toContain('seg task-detail-tabs');expect(html).toContain('name="reopen"');expect(html).toContain('pill-yellow');
});
