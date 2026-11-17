import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider, RouterContextProvider } from 'react-router';
import { expect, test, vi } from 'vitest';
import { settingsAction as routeAction, settingsLoader as loader, SettingsView } from '../app/routes/loop.tsx';
import ProjectBoard from '../app/routes/projects.$id.tsx';
import { servicesContext, type WebServices } from '../app/load-context.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { createGateway } from '../server/gateway.ts';
import { onboardingState, type SettingsData, type OnboardingResult } from '../app/components/settings-data.ts';
const base: SettingsData = { project: { id: 'p', path: '/tmp/p', display_name: '测试', onboarding: { onboarding: 'in_progress', step: 3 } }, files: [{ path: 'AGENTS.md', exists: true, content: 'old' }, { path: 'docs/agent-project-profile.md', exists: true, content: '' }], session: { state: 'none' } };
function args(services: WebServices, form: Record<string, string>) { const context = new RouterContextProvider(); context.set(servicesContext, services); return { context, params: { id: 'p' }, request: new Request('http://localhost/loop?project=p&tab=project', { method: 'POST', body: new URLSearchParams(form) }), url: new URL('http://localhost/loop?project=p&tab=project'), pattern: '/projects/:id/settings' }; }
function html(data = base, result?: OnboardingResult, allowed = true) { const router = createMemoryRouter([{ path: '/', element: createElement(WriteGateProvider, { value: { allowed, reason: '离线原因' } }, createElement(SettingsView, { data, result })) }]); try { return renderToString(createElement(RouterProvider, { router })); } finally { router.dispose(); } }
const ok = (payload: unknown) => ({ status: 200, body: { ok: true, payload } });
test('1 步骤与结束状态', async () => {
  for (const step of [3, 4, 5] as const) { const text = html({ ...base, project: { ...base.project!, onboarding: { onboarding: 'in_progress', step } } }); expect(text).toContain(`第 ${step} / 5 步`); expect(text).toContain(({ 3: '起草项目画像', 4: '确认或编辑', 5: '查看 Loop 配置' })[step]); }
  for (const onboarding of ['completed', 'skipped']) { const text = html({ ...base, project: { ...base.project!, onboarding: { onboarding } } }); expect(text).not.toContain('value="run"'); expect(text).not.toContain('<textarea'); }
  expect(html(base, { init_commit: '1234567890' })).toContain('已生成 init commit 12345678'); expect(html(base, { init_commit: null })).toContain('没有需要提交的改动');
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
test('6 项目页设置链接', () => { const router = createMemoryRouter([{ path: '/', Component: ProjectBoard }], { hydrationData: { loaderData: { '0': { project: { id: 'p', display_name: '测试' }, tasks: [], alerts: [], now: new Date().toISOString() } } } }); try { expect(renderToString(createElement(RouterProvider, { router }))).toContain('href="/loop?project=p"'); } finally { router.dispose(); } });

async function action(args: Parameters<typeof routeAction>[0]) { const result = await routeAction(args); if (result instanceof Response) throw new Error('Unexpected redirect during onboarding'); return result; }

test('7 账本会话状态与日志入口', () => {
  expect(onboardingState(null)).toEqual({ state: 'none' });
  for (const [lifecycle, label] of [[{ state: 'running' }, '会话运行中'], [{ state: 'exited', exit_code: 0 }, '正常结束'], [{ state: 'exited', exit_code: 4 }, '崩溃（退出码 4）'], [{ state: 'killed' }, '被停止'], [{ state: 'timed_out' }, '超时'], [{ state: 'vanished' }, '崩溃（进程消失）']] as const) {
    const session = onboardingState({ id: 'ses-test', started_at: '', lifecycle });
    expect(html({ ...base, session })).toContain(label); expect(html({ ...base, session })).toContain('data-session-id="ses-test"');
  }
});
test('8 账本运行中才读磁盘标记，结束时不读盘', async () => {
  for (const diskState of ['ended', 'interrupted', 'none', 'running'] as const) {
    const status = vi.fn(() => ({ state: diskState, session_id: 'ses-test', exit_code: 0 }));
    let lifecycle: { state: string; exit_code?: number } = { state: 'running' };
    const read = vi.fn(async ({ method }: { method: string }) => ok(method === 'project.list' ? { projects: [{ project: base.project }] } : method === 'project.get' ? { onboarding_session: { id: 'ses-test', started_at: '', lifecycle } } : method === 'config.get' ? { resolved: {} } : method === 'config.validate' ? { violations: [] } : { files: [] }));
    const services = { gateway: { read, write: vi.fn() }, admin: { configPaths: () => [], onboarding: { status } } } as unknown as WebServices;
    const data = await loader(args(services, {})); expect(status).toHaveBeenCalledTimes(1);
    expect(html(data)).toContain(diskState === 'ended' ? '已结束（据磁盘标记，账本尚未记录）' : diskState === 'interrupted' ? '进程已消失' : '会话运行中');
    lifecycle = { state: 'exited', exit_code: 0 }; status.mockClear(); expect(html(await loader(args(services, {})))).toContain('正常结束'); expect(status).not.toHaveBeenCalled();
  }
});
