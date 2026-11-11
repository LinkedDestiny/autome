import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createRoutesStub, MemoryRouter } from 'react-router';
import { readFileSync, readdirSync } from 'node:fs';
import { expect, it, vi } from 'vitest';
import { BellView } from '../app/components/bell.tsx';
import { TARGETS, resolveTarget } from '../app/components/bell-targets.ts';
import type { BellSnapshot, NotificationItem } from '../app/components/bell-client.ts';
import App from '../app/root.tsx';
const item: NotificationItem = { id: 'ntf-1', kind: 'await_merge', level: 'need_you', task_id: 'T-3', project_id: 'prj-1', title: 'T-3 可以合并了', body: '实现通知器', created_at: new Date(2026, 0, 2, 3, 4).toISOString(), read_at: null };
function view(snapshot: BellSnapshot, targets = TARGETS) {
  return renderToString(createElement(MemoryRouter, null, createElement(BellView, { snapshot, open: true, targets, onToggle: vi.fn(), onRead: async () => {}, onAll: async () => {} })));
}
it('徽标 0 隐藏，1、99、100 边界与中文 aria-label', () => {
  for (const unread of [0, 1, 99, 100]) {
    const html = view({ unread, items: [] }); expect(html).toContain(`aria-label="通知，${unread} 条未读"`);
    if (unread === 0) expect(html).not.toContain('font-weight:600');
    else expect(html).toContain(`>${unread >= 100 ? '99+' : unread}</span>`);
  }
});
it('列表标题、正文、本地时间、未读圆点与操作；全已读时禁用', () => {
  const html = view({ unread: 1, items: [item] });
  for (const text of [item.title, item.body!, '01-02 03:04', 'aria-label="未读"', '标为已读', '全部已读']) expect(html).toContain(text);
  const read = view({ unread: 0, items: [{ ...item, body: null, read_at: item.created_at }] });
  expect(read).not.toContain(item.body!);
  expect(read).not.toContain('标为已读'); expect(read).not.toContain('aria-label="未读"'); expect(read).toMatch(/disabled="">全部已读/);
});
it('默认任务与项目无链接；开放后正确目标；无归属不链接；ID 编码', () => {
  for (const entry of [item, { ...item, task_id: null }]) {
    const html = view({ unread: 1, items: [entry] }); expect(html).not.toContain('href='); expect(html).toContain('详情页尚未开放');
  }
  const available = TARGETS.map(target => ({ ...target, available: true }));
  expect(view({ unread: 1, items: [item] }, available)).toContain('href="/tasks/T-3"');
  expect(view({ unread: 1, items: [{ ...item, task_id: null }] }, available)).toContain('href="/projects/prj-1"');
  const none = view({ unread: 1, items: [{ ...item, task_id: null, project_id: null }] }, available);
  expect(none).not.toContain('href='); expect(none).not.toContain('详情页尚未开放');
  expect(resolveTarget({ task_id: 'a/b ?中', project_id: null }, available).href).toBe('/tasks/a%2Fb%20%3F%E4%B8%AD');
});
it('空列表及加载失败文案', () => {
  expect(view({ unread: 0, items: [] })).toContain('还没有通知');
  expect(view({ unread: 0, items: [], error: '通知加载失败：网络不可达' })).toContain('通知加载失败：网络不可达');
});
it('固定定位与内部 60vh 滚动', () => {
  const html = view({ unread: 1, items: [item] });
  const closed = renderToString(createElement(MemoryRouter, null, createElement(BellView, { snapshot: { unread: 1, items: [item] }, open: false, onToggle: vi.fn(), onRead: async () => {}, onAll: async () => {} })));
  expect(closed).toContain('aria-expanded="false"'); expect(closed).not.toContain('通知列表');
  expect(html).toContain('aria-expanded="true"');
  expect(html).toContain('position:fixed'); expect(html).toContain('max-height:60vh'); expect(html).toContain('overflow-y:auto');
});
it('root 同时渲染铃铛与子路由，仍只由 useLiveEvents 建连接', () => {
  const Stub = createRoutesStub([{ path: '/', Component: App, children: [{ index: true, Component: () => createElement('p', null, '子路由内容') }] }]);
  const html = renderToString(createElement(Stub));
  expect(html).toContain('aria-label="通知，0 条未读"'); expect(html).toContain('子路由内容');
  const root = readFileSync(new URL('../app/root.tsx', import.meta.url), 'utf8'); expect(root.match(/useLiveEvents\(\)/g)).toHaveLength(1);
  for (const name of readdirSync(new URL('../app/components/', import.meta.url)).filter(name => name.startsWith('bell'))) {
    const source = readFileSync(new URL(`../app/components/${name}`, import.meta.url), 'utf8'); expect(source).not.toMatch(/createLiveClient\(|WebSocket/);
  }
});
it('目标可用性与真实路由表一致', () => {
  const source = readFileSync(new URL('../app/routes.ts', import.meta.url), 'utf8');
  for (const target of TARGETS) {
    const prefix = target.pattern.split('/')[1]; expect(target.available).toBe(new RegExp(`['"\x60]/?${prefix}(?:/|['"\x60])`).test(source));
  }
});
it('权限 default 显示开启按钮，denied 提示，granted 与 unsupported 隐藏', () => {
  for (const permission of ['default', 'denied', 'granted', 'unsupported'] as const) {
    const html = renderToString(createElement(MemoryRouter, null, createElement(BellView, { snapshot: { unread: 0, items: [] }, open: true, permission, onRequest: vi.fn(), onToggle: vi.fn(), onRead: async () => {}, onAll: async () => {} })));
    expect(html.includes('开启系统通知')).toBe(permission === 'default');
    expect(html.includes('系统通知已被浏览器拒绝，铃铛照常提醒')).toBe(permission === 'denied');
  }
});
