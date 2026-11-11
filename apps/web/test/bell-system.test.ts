import { readFileSync } from 'node:fs';
import { expect, it, vi } from 'vitest';
import { browserSystemNotifier, showSystemNotification, type SystemPermission, type SystemNotifier } from '../app/components/bell-system.ts';
import { createBellClient, type NotificationItem } from '../app/components/bell-client.ts';
const message = { type: 'notification', id: 'ntf-1', kind: 'await_merge', task_id: 'T-3', title: 'T-3 可以合并了' };
const item: NotificationItem = { ...message, level: 'need_you', project_id: 'p1', body: '任务正文', created_at: '', read_at: null };
const targets = [{ pattern: '/tasks/:taskId', available: true }, { pattern: '/projects/:projectId', available: true }];
function fake(permission: SystemPermission = 'granted'): SystemNotifier {
  return { permission: () => permission, request: vi.fn(async () => permission), show: vi.fn() };
}
async function deliver(notifier: SystemNotifier, hidden = true, incoming = message) {
  const received = vi.fn();
  const client = createBellClient({ fetch: vi.fn(async () => new Response(JSON.stringify({ ok: true, payload: { unread: 1, items: [item] } }))), onChange: vi.fn(),
    onNotification: (msg, items) => { showSystemNotification(notifier, hidden, msg, items, targets); received(); } });
  client.handleMessage('notification', incoming);
  await vi.waitFor(() => expect(received).toHaveBeenCalledTimes(1));
  client.stop();
}
function browser() {
  const notices: { title: string; options: NotificationOptions; onclick?: () => void }[] = [];
  class Notice {
    static permission: NotificationPermission = 'granted';
    static requestPermission = vi.fn(async () => Notice.permission);
    onclick?: () => void;
    title: string; options: NotificationOptions;
    constructor(title: string, options: NotificationOptions) { this.title = title; this.options = options; notices.push(this); }
  }
  const window = { Notification: Notice as unknown as typeof Notification, focus: vi.fn(), location: { assign: vi.fn() } };
  return { window, Notice, notices, notifier: browserSystemNotifier(window) };
}
it('后台需要你在列表刷新后立即 show，消息标题与列表正文、目标', async () => {
  const notifier = fake(); await deliver(notifier);
  expect(notifier.show).toHaveBeenCalledWith({ id: message.id, title: message.title, body: item.body, href: '/tasks/T-3' });
});
it('知会与未知级别不弹', () => {
  const notifier = fake();
  for (const kind of ['done', 'unknown']) showSystemNotification(notifier, true, { ...message, kind }, [item]);
  expect(notifier.show).not.toHaveBeenCalled();
});
it('前台不弹；转后台弹；回前台不弹', () => {
  const notifier = fake();
  for (const hidden of [false, true, false]) showSystemNotification(notifier, hidden, message, [item]);
  expect(notifier.show).toHaveBeenCalledTimes(1);
});
it('四种权限只有 granted 弹；权限请求只显式发生，支持拒绝和不支持', async () => {
  for (const permission of ['default', 'denied', 'unsupported', 'granted'] as const) {
    for (const hidden of [false, true]) {
      for (const kind of ['await_design_approval', 'await_merge', 'await_info', 'failed', 'project_alert', 'not_a_bug', 'done']) {
        const notifier = fake(permission);
        showSystemNotification(notifier, hidden, { ...message, kind }, [item]);
        expect(notifier.show).toHaveBeenCalledTimes(permission === 'granted' && hidden && kind !== 'done' ? 1 : 0);
        expect(notifier.request).not.toHaveBeenCalled();
      }
    }
  }
  const b = browser();
  for (const permission of ['default', 'denied', 'granted'] as const) {
    b.Notice.permission = permission;
    expect(b.notifier.permission()).toBe(permission);
    expect(await b.notifier.request()).toBe(permission);
  }
  expect(b.Notice.requestPermission).toHaveBeenCalledTimes(3);
  const unsupported = browserSystemNotifier({ ...b.window, Notification: undefined });
  expect(unsupported.permission()).toBe('unsupported'); expect(await unsupported.request()).toBe('unsupported');
});
it('两个后台控制器同 id 同 tag，不同通知使用自己的 id', async () => {
  const a = browser(), b = browser();
  await Promise.all([deliver(a.notifier), deliver(b.notifier)]);
  await deliver(a.notifier, true, { ...message, id: 'ntf-2' });
  expect(a.notices.map(n => n.options.tag)).toEqual(['ntf-1', 'ntf-2']);
  expect(b.notices.map(n => n.options.tag)).toEqual(['ntf-1']);
});
it('列表缺失、刷新失败、空正文均降级；无任务不跳转，项目路由可解析', () => {
  const notifier = fake();
  for (const items of [null, [], [{ ...item, body: '' }], [{ ...item, body: null }]]) {
    showSystemNotification(notifier, true, message, items, targets);
    expect(notifier.show).toHaveBeenLastCalledWith({ id: message.id, title: message.title, href: '/tasks/T-3' });
  }
  showSystemNotification(notifier, true, { ...message, task_id: null }, null, targets);
  expect(notifier.show).toHaveBeenLastCalledWith({ id: message.id, title: message.title, href: null });
  showSystemNotification(notifier, true, message, [{ ...item, task_id: null }], targets);
  expect(notifier.show).toHaveBeenLastCalledWith(expect.objectContaining({ href: '/projects/p1' }));
  showSystemNotification(notifier, true, message, [item]);
  expect(notifier.show).toHaveBeenLastCalledWith(expect.objectContaining({ href: null }));
});
it('点击 focus；可用路由跳转，null 仅 focus', () => {
  const b = browser();
  for (const href of ['/tasks/T-3', null]) { b.notifier.show({ id: message.id, title: message.title, href }); b.notices.at(-1)!.onclick!(); }
  expect(b.window.focus).toHaveBeenCalledTimes(2); expect(b.window.location.assign).toHaveBeenCalledExactlyOnceWith('/tasks/T-3');
});
it('浏览器构造参数精确；组件使用注入接口且无跨页协调与自动授权', () => {
  const b = browser(); b.notifier.show({ id: message.id, title: message.title, body: '正文', href: null });
  expect(b.notices[0].title).toBe(message.title); expect(b.notices[0].options).toEqual({ body: '正文', tag: message.id });
  const source = readFileSync(new URL('../app/components/bell-system.ts', import.meta.url), 'utf8');
  expect(source).not.toMatch(/navigator\.locks|localStorage/);
  const component = readFileSync(new URL('../app/components/bell.tsx', import.meta.url), 'utf8');
  expect(component).toContain('systemNotifier ?? browserSystemNotifier(window)');
  expect(component).toContain('showSystemNotification(system, document.hidden, message, items)');
  expect(component).not.toMatch(/new Notification|Notification\.requestPermission/);
  expect(component.slice(component.indexOf('useEffect(() =>'), component.indexOf('}, [systemNotifier])'))).not.toContain('.request(');
  expect(component).toMatch(/onRequest=\{\(\) => \{\s*void notifier.current\?\.request\(\).then\(setPermission\)/);
});
