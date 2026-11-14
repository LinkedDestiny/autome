import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, test, vi } from 'vitest';
import { browserSystemNotifier } from '../app/components/bell-system.ts';
import { nativePicker } from '../app/components/native-picker.ts';
import { DirectoryPicker } from '../app/routes/projects.new.tsx';
import { createGateway } from '../server/gateway.ts';
test('原生通知优先且权限视为已授予', async () => {
  const notify = vi.fn(async () => {}), assign = vi.fn();
  const notifier = browserSystemNotifier({ autome: { notify }, focus: vi.fn(), location: { assign } });
  expect(notifier.permission()).toBe('granted'); expect(await notifier.request()).toBe('granted');
  notifier.show({ id: 'n', title: '标题', href: '/tasks/T-1' });
  expect(notify).toHaveBeenCalledExactlyOnceWith({ title: '标题', body: '', href: '/tasks/T-1' }); expect(assign).not.toHaveBeenCalled();
});
test('原生目录选择按钮仅在能力存在时显示', () => {
  for (const present of [false, true]) {
    const picker = nativePicker(present ? { autome: { pickDirectory: async () => '/tmp' } } : {});
    const html = renderToStaticMarkup(createElement(DirectoryPicker, { picker, disabled: false, onPick: vi.fn(), onError: vi.fn() }));
    expect(html.includes('data-testid="pick-directory"')).toBe(present);
  }
});
test('project.pick 继续返回 501，不转发内核', async () => {
  const core = { call: vi.fn(), restart: vi.fn() };
  const gateway = createGateway({ core, home: '/tmp/native-shell', uploads: { resolve: () => null } });
  expect((await gateway.write({ op: 'project.pick', params: {} })).status).toBe(501); expect(core.call).not.toHaveBeenCalled();
});
