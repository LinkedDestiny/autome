import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, it, vi } from 'vitest';
import App, { ErrorBoundary, links } from '../app/root.tsx';
vi.mock('../app/styles/app.css?url', () => ({ default: '/assets/app.css' }));
function shell() {
  const Stub = createRoutesStub([{ id: 'root', path: '/', loader: () => null, Component: App, children: [{ index: true, Component: () => createElement('p', null, '子路由内容') }] }]);
  return renderToString(createElement(Stub, { hydrationData: { loaderData: { root: { clock: { now: '2026-11-17T00:00:00Z', timeZone: 'UTC' }, kernel: { ok: true }, projects: [{ id: 'p1', name: '演示项目' }], tasks: [] } } } }));
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

import { MemoryRouter } from 'react-router';
import { Shell } from '../app/components/shell.tsx';
import { driverCardModel } from '../app/components/driver-model.ts';
const now = '2026-11-17T01:00:00Z';
const driver = { pid: 1, command: 'web', self: true, driving: false, started_at: '2026-11-17T00:00:00Z', last_tick_at: '2026-11-17T00:59:57Z' };
const renderShell = (props: Partial<Parameters<typeof Shell>[0]> = {}) => renderToString(createElement(MemoryRouter, null, createElement(Shell, { projects: [], banner: null, children: null, ...props })));
it('导航顺序与面包屑，技能零与未知不同', () => {
  const html = renderShell({ skillCount: 0, crumb: '项目看板' });
  const side = html.slice(0, html.indexOf('</nav>'));
  const positions = ['href="/"', 'href="/loop"', 'href="/skills"', 'href="/projects/new"', '驱动状态', 'href="/env"'].map(value => side.indexOf(value)); expect(positions.every(n => n >= 0)).toBe(true); expect(positions).toEqual([...positions].sort((a,b) => a-b));
  expect(html).toContain('aria-label="当前位置">项目看板'); expect(html).toContain('class="nav-count">0');
  expect(renderShell()).not.toContain('class="nav-count"');
});
it('项目行有任务显示双计数，空闲显示完成数量或没有任务', () => {
  const projects = [{ id: 'a', name: 'a', counts: { running: 1, needs_you: 2 } }, { id: 'b', name: 'b', counts: { running: 0, needs_you: 0 }, done: 70 }, { id: 'c', name: 'c', counts: { running: 0, needs_you: 0 }, done: 0 }];
  const html = renderShell({ projects });
  for (const value of ['进行中 1', '等你处理 2', '70 个已完成', '没有任务在跑', 'project-square']) expect(html).toContain(value);
});
it('驱动卡四种口径及旧内核推断', () => {
  expect(driverCardModel(undefined, now).title).toBe('驱动状态未知');
  expect(driverCardModel({ available: false, list: [], other_driver: { skipped_last_minute: 1, last_skipped_at: null } }, now).detail).toBe('另一个驱动可能在推进');
  expect(driverCardModel({ available: true, list: [] }, now).title).toBe('没有驱动在运行');
  expect(renderShell({ drivers: { available: true, list: [] } })).toContain('查看环境');
  expect(driverCardModel({ available: true, list: [driver] }, now).title).toBe('驱动空闲');
  expect(driverCardModel({ available: true, list: [{ ...driver, driving: true }] }, now)).toMatchObject({ title: '驱动在推进', detail: '3 秒前检查过 · 已运行 1 小时', pid: 1 });
});
it('两个驱动只显示在推进的那个，多推进取最新 tick，self 仅推进时选中', () => {
  const other = { ...driver, pid: 2, self: false, driving: true };
  expect(driverCardModel({ available: true, list: [driver, other] }, now).pid).toBe(2);
  expect(driverCardModel({ available: true, list: [other, { ...driver, driving: true, last_tick_at: now }] }, now).pid).toBe(1);
});
