import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createRoutesStub, RouterContextProvider, type LoaderFunctionArgs } from 'react-router';
import { readFileSync } from 'node:fs';
import { expect, it, vi } from 'vitest';
import TaskPage, { loader, shouldRevalidate } from '../app/routes/tasks.$id.tsx';
import { servicesContext, type KernelGateway } from '../app/load-context.ts';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import { TARGETS } from '../app/components/bell-targets.ts';
const payload = { task: { id: 'T-1', title: '日志任务' }, sessions: [
  { id: 'ses-2', label: '第二会话', round: 2, running: true, started_at: '' },
  { id: 'ses-1', label: '第一会话', round: 1, running: false, started_at: '' },
] };
function load(gateway: KernelGateway | null) {
  const context = new RouterContextProvider(); context.set(servicesContext, { gateway });
  return loader({ context, params: { id: 'T-1' }, request: new Request('http://localhost/tasks/T-1'), url: new URL('http://localhost/tasks/T-1'), pattern: '/tasks/:id' } as LoaderFunctionArgs);
}
function render(data: Awaited<ReturnType<typeof loader>>) {
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  return renderToString(createElement(Stub, { initialEntries: ['/tasks/T-1'], hydrationData: { loaderData: { task: data } } }));
}
it('任务路由存在，铃铛目标同步开放', () => {
  expect(readFileSync(new URL('../app/routes.ts', import.meta.url), 'utf8')).toMatch(/route\("tasks\/:id", "routes\/tasks\.\$id\.tsx"\)/);
  expect(TARGETS.find(t => t.pattern === '/tasks/:taskId')?.available).toBe(true);
});
it('loader 仅通过网关读取一次 task.get 并返回任务会话', async () => {
  const read = vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload } });
  expect(await load({ read, write: vi.fn() })).toEqual({ data: payload });
  expect(read.mock.calls).toEqual([[{ method: 'task.get', params: { task_id: 'T-1' } }]]);
});
it('路由渲染标题 id 和每个会话，保持内核顺序，无阶段条和写按钮', () => {
  const html = render({ data: payload });
  expect(html).toContain('日志任务'); expect(html).toContain('T-1'); expect(html.match(/<details/g)).toHaveLength(2);
  expect(html.indexOf('第二会话')).toBeLessThan(html.indexOf('第一会话'));
  expect(html).not.toContain('<button'); expect(html).not.toContain('task-stage');
  expect(render({ data: { ...payload, sessions: [] } })).not.toContain('<details');
});
it('离线、503/504 和 not_found/404 分支呈现中文', async () => {
  expect(render(await load(null))).toContain('内核离线，暂时拿不到任务');
  for (const [status, code, expected] of [[503, '', '内核离线，暂时拿不到任务'], [504, '', '内核离线，暂时拿不到任务'], [400, 'not_found', '找不到任务'], [404, '', '找不到任务'], [500, '', '读取任务失败：失败']] as const) {
    expect(render(await load({ read: vi.fn().mockResolvedValue({ status, body: { ok: false, code, error: '失败' } }), write: vi.fn() }))).toContain(expected);
  }
});
it('仅本任务 task/session 事件触发重验', async () => {
  const url = new URL('http://localhost/tasks/T-1');
  for (const kind of ['task.updated', 'session.ended', 'project.updated']) for (const subject of ['T-1', 'T-2']) {
    await revalidateForEvents([{ seq: 1, kind, subject, payload: {} }], () => {
      expect(shouldRevalidate({ currentUrl: url, nextUrl: url, currentParams: { id: 'T-1' }, defaultShouldRevalidate: false } as never)).toBe(subject === 'T-1' && kind !== 'project.updated');
    });
  }
});
