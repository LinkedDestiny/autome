import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, it, vi } from 'vitest';
import App, { ErrorBoundary, links } from '../app/root.tsx';
vi.mock('../app/styles/app.css?url', () => ({ default: '/assets/app.css' }));
function shell() {
  const Stub = createRoutesStub([{ id: 'root', path: '/', loader: () => null, Component: App, children: [{ index: true, Component: () => createElement('p', null, '子路由内容') }] }]);
  return renderToString(createElement(Stub, { hydrationData: { loaderData: { root: { kernel: { ok: true }, projects: [{ id: 'p1', name: '演示项目' }], tasks: [] } } } }));
}
it('项目栏、顶栏铃铛和主内容分区', () => {
  const html = shell(); expect(html).toMatch(/<nav aria-label="项目"[^>]*>[\s\S]*演示项目/);
  expect(html).toMatch(/<header[^>]*>[\s\S]*aria-label="通知，0 条未读"[\s\S]*<\/header>/);
  expect(html).toMatch(/<main[^>]*><p>子路由内容<\/p><\/main>/);
});
it('根样式表接线', () => { expect(links()).toEqual([{ rel: 'stylesheet', href: expect.stringContaining('app.css') }]); });
it('未知任务路径显示未开放和返回总览链接', () => {
  const Stub = createRoutesStub([{ id: 'root', path: '/', Component: App, ErrorBoundary }]);
  const html = renderToString(createElement(Stub, { initialEntries: ['/tasks/T-1'] }));
  expect(html).toContain('这个页面还没开放'); expect(html).toContain('href="/"'); expect(html).toContain('回到总览');
});
it('SSR 新建入口禁用并说明连接中', () => { expect(shell()).toMatch(/<button[^>]*disabled=""[^>]*title="正在连接服务"[^>]*>新建任务卡/); });
