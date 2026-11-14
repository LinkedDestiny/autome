import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider, RouterContextProvider } from 'react-router';
import { expect, test, vi } from 'vitest';
import { action as routeAction, loader, SettingsView } from '../app/routes/projects.$id.settings.tsx';
import ProjectBoard from '../app/routes/projects.$id.tsx';
import { servicesContext, type WebServices } from '../app/load-context.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { createGateway } from '../server/gateway.ts';
import { subscribeAdminRevalidate } from '../app/components/admin-live.ts';
import { publishLiveMessage, subscribeLiveMessages } from '../app/lib/live-client.ts';
import type { SettingsData, OnboardingResult } from '../app/components/settings-data.ts';
const base: SettingsData = { project: { id: 'p', path: '/tmp/p', display_name: '测试', onboarding: { onboarding: 'in_progress', step: 3 } }, files: [{ path: 'AGENTS.md', exists: true, content: 'old' }, { path: 'docs/agent-project-profile.md', exists: true, content: '' }], session: { state: 'none' } };
function args(services: WebServices, form: Record<string, string>) { const context = new RouterContextProvider(); context.set(servicesContext, services); return { context, params: { id: 'p' }, request: new Request('http://localhost/projects/p/settings', { method: 'POST', body: new URLSearchParams(form) }), url: new URL('http://localhost/projects/p/settings'), pattern: '/projects/:id/settings' }; }
function html(data = base, result?: OnboardingResult, allowed = true) { const router = createMemoryRouter([{ path: '/', element: createElement(WriteGateProvider, { value: { allowed, reason: '离线原因' } }, createElement(SettingsView, { data, result })) }]); try { return renderToString(createElement(RouterProvider, { router })); } finally { router.dispose(); } }
const ok = (payload: unknown) => ({ status: 200, body: { ok: true, payload } });
test('1 步骤与结束状态', async () => {
  for (const step of [3, 4, 5] as const) { const text = html({ ...base, project: { ...base.project!, onboarding: { onboarding: 'in_progress', step } } }); expect(text).toContain(`第 ${step} / 5 步`); expect(text).toContain(({ 3: '起草项目画像', 4: '确认或编辑', 5: '查看 Loop 配置' })[step]); }
  for (const onboarding of ['completed', 'skipped']) { const text = html({ ...base, project: { ...base.project!, onboarding: { onboarding } } }); expect(text).not.toContain('value="run"'); expect(text).not.toContain('<textarea'); }
  expect(html(base, { init_commit: '1234567890' })).toContain('已生成 init commit 12345678'); expect(html(base, { init_commit: null })).toContain('没有需要提交的改动');
  const watch = vi.fn(); const readSession = vi.fn().mockResolvedValueOnce(ok({ projects: [{ project: base.project }] })).mockResolvedValueOnce(ok({ resolved: {} })).mockResolvedValueOnce(ok({ violations: [] })).mockResolvedValueOnce(ok({ files: base.files }));
  expect((await loader(args({ gateway: { read: readSession, write: vi.fn() }, admin: { configPaths: () => [], onboarding: { status: () => ({ state: 'running', session_id: 'ses-restored' }), watch } } } as unknown as WebServices, {}))).files).toEqual(base.files); expect(watch).toHaveBeenCalledWith('p', '/tmp/p', 'ses-restored');
  const read = vi.fn(async () => ok({ projects: [] })); expect(await loader(args({ gateway: { read, write: vi.fn() } }, {}))).toHaveProperty('error', '项目不存在');
  for (const error of ['拒绝授权', '读盘失败', '请求超时', '内核未连接']) { read.mockResolvedValueOnce({ status: 503, body: { ok: false, error } } as never); expect(await loader(args({ gateway: { read, write: vi.fn() } }, {}))).toHaveProperty('error', error); }
});
test('2 完成推进三次、各步重试与失败停止', async () => {
  for (const step of [3, 4, 5]) { let next = step; const write = vi.fn(async () => ok({ onboarding: ++next <= 5 ? { onboarding: 'in_progress', step: next } : { onboarding: 'completed' }, init_commit: next > 5 ? 'sha' : null })); expect(await action(args({ gateway: { read: vi.fn(), write } }, { intent: 'finish' }))).toEqual({ init_commit: 'sha' }); expect(write).toHaveBeenCalledTimes(6 - step); }
  const write = vi.fn().mockResolvedValueOnce(ok({ onboarding: { onboarding: 'in_progress', step: 4 } })).mockResolvedValueOnce({ status: 409, body: { ok: false, error: '原文拒绝' } });
  expect(await action(args({ gateway: { read: vi.fn(), write } }, { intent: 'finish' }))).toEqual({ error: '原文拒绝' }); expect(write).toHaveBeenCalledTimes(2);
  for (const error of ['拒绝授权', '读盘失败', '请求超时', '内核未连接']) { const fail = vi.fn(async () => ({ status: 503, body: { ok: false, error } })); for (const intent of ['save', 'skip', 'finish']) expect(await action(args({ gateway: { read: vi.fn(), write: fail } }, { intent, path: 'AGENTS.md', content: '' }))).toEqual({ error }); }
});
test('3 保存只收两份路径、空串和最大长度原样送网关', async () => {
  const call = vi.fn(async () => ({})); const gateway = createGateway({ core: { call, restart: vi.fn() }, home: '/tmp' });
  for (const path of ['AGENTS.md', 'docs/agent-project-profile.md']) for (const content of ['', 'a', 'x'.repeat(262144)]) { expect(await action(args({ gateway }, { intent: 'save', path, content }))).toEqual({ saved: true }); expect(call).toHaveBeenLastCalledWith('project.onboarding.save', { project_id: 'p', path, content }); }
  call.mockClear(); for (const path of ['', '../AGENTS.md', 'other']) expect(await action(args({ gateway }, { intent: 'save', path, content: '' }))).toHaveProperty('error'); expect(call).not.toHaveBeenCalled();
  expect(await action(args({ gateway: null }, { intent: 'save' }))).toHaveProperty('error'); expect(await action(args({ gateway }, { intent: 'unknown' }))).toHaveProperty('error');
});
test('4 超长由真实网关拒绝并显示原文', async () => { const call = vi.fn(); const gateway = createGateway({ core: { call, restart: vi.fn() }, home: '/tmp' }); const result = await action(args({ gateway }, { intent: 'save', path: 'AGENTS.md', content: 'x'.repeat(262145) })); expect(result.error).toMatch(/^已拒绝：.*超过.*字符上限/); expect(call).not.toHaveBeenCalled(); expect(html(base, result)).toContain(result.error); });
test('5 会话状态和离线禁用全部按钮', () => { const running = html({ ...base, session: { state: 'running', session_id: 'ses-test' } }); expect(running).toContain('会话运行中'); expect(running).toContain('ses-test'); expect(running).toMatch(/value="run" disabled=""/); expect(html({ ...base, session: { state: 'ended', exit_code: 7 } })).toContain('退出码 7'); const offline = html(base, undefined, false); expect(offline.match(/<button/g)).toHaveLength(6); expect(offline.match(/disabled="" title="离线原因"/g)).toHaveLength(6); });
test('6 项目页设置链接', () => { const router = createMemoryRouter([{ path: '/', Component: ProjectBoard }], { hydrationData: { loaderData: { '0': { project: { id: 'p', display_name: '测试' }, tasks: [], alerts: [], now: new Date().toISOString() } } } }); try { expect(renderToString(createElement(RouterProvider, { router }))).toContain('href="/projects/p/settings"'); } finally { router.dispose(); } });
test('7 扩展消息只刷新 onboarding.ended 且可取消', async () => {
  vi.useFakeTimers();
  const refresh = vi.fn(); const unsubscribe = subscribeAdminRevalidate(subscribeLiveMessages, refresh);
  try {
    publishLiveMessage('hello', { type: 'hello' });
    await vi.advanceTimersByTimeAsync(1000); expect(refresh).not.toHaveBeenCalled();
    publishLiveMessage('onboarding.ended', { type: 'onboarding.ended' });
    await vi.advanceTimersByTimeAsync(999); expect(refresh).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1); expect(refresh).toHaveBeenCalledTimes(1);
    unsubscribe(); publishLiveMessage('onboarding.ended', { type: 'onboarding.ended' });
    await vi.advanceTimersByTimeAsync(1000); expect(refresh).toHaveBeenCalledTimes(1);
  } finally { unsubscribe(); await vi.runOnlyPendingTimersAsync(); vi.useRealTimers(); }
});

async function action(args: Parameters<typeof routeAction>[0]) { const result = await routeAction(args); if (result instanceof Response) throw new Error('Unexpected redirect during onboarding'); return result; }
