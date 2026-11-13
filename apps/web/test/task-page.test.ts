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
function render(data: Awaited<ReturnType<typeof loader>>) {
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  return renderToString(createElement(Stub, { initialEntries: ['/tasks/T-1'], hydrationData: { loaderData: { task: data } } }));
}
it('任务路由存在，铃铛目标同步开放', () => {
  expect(readFileSync(new URL('../app/routes.ts', import.meta.url), 'utf8')).toMatch(/route\("tasks\/:id", "routes\/tasks\.\$id\.tsx"\)/);
  expect(TARGETS.find(t => t.pattern === '/tasks/:taskId')?.available).toBe(true);
});
it('离线、503/504 和 not_found/404 分支呈现中文', async () => {
  expect(render(await load(null))).toContain('内核离线，暂时拿不到任务');
  for (const [status, code, expected] of [[503, '', '内核离线，暂时拿不到任务'], [504, '', '内核离线，暂时拿不到任务'], [400, 'not_found', '找不到任务'], [404, '', '找不到任务'], [500, '', '无法读取任务：失败']] as const) {
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
