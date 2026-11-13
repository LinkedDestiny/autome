import fixture from './fixtures/w9-replies.json' with { type: 'json' };
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { expect, test, vi } from 'vitest';
import { projectAlerts, alertCounts, type ProjectAlert } from '../app/components/alerts-data.ts';
import { ProjectAlerts } from '../app/components/project-alerts.tsx';
import { ProjectList } from '../app/components/project-list.tsx';
import { loadShell } from '../app/components/shell-data.ts';
import ProjectBoard, { loader, action, shouldRevalidate } from '../app/routes/projects.$id.tsx';
import { shouldRevalidate as rootRevalidate } from '../app/root.tsx';
import { createLoadContext, type KernelGateway } from '../app/load-context.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import { ALERT_KINDS } from '../app/lib/admin-labels.ts';
import { createGateway } from '../server/gateway.ts';
export const alert: ProjectAlert = { ...fixture.alert, id: 'a1', project_id: 'P1', kind: 'budget_exceeded', task_id: 'T-1', detail: '原文\nsecond <script>', created_at: '2026-11-13T00:00:00Z' };
function markup(element: ReturnType<typeof createElement>, allowed = true) {
  const Stub = createRoutesStub([{ path: '/', Component: () => createElement(WriteGateProvider, { value: { allowed, reason: '内核离线' } }, element) }]);
  return renderToStaticMarkup(createElement(Stub));
}
function gateway(error?: string): KernelGateway {
  return { read: vi.fn(async input => {
    const { method } = input as { method: string };
    if (method === 'dashboard.get' && error) return { status: 504, body: { ok: false, error } };
    return { status: 200, body: { ok: true, payload: method === 'project.list' ? { projects: [{ project: { id: 'P1', display_name: '项目甲' } }, { project: { id: 'P2', display_name: '项目乙' } }] } : method === 'dashboard.get' ? { alerts: [alert], environment: { probed: false } } : { project: { id: 'P1', display_name: '项目甲' }, tasks: [] } } };
  }), write: vi.fn(async () => ({ status: 200, body: { ok: true, payload: {} } })) };
}
function args(g: KernelGateway | null) { return { context: createLoadContext({ gateway: g })(), params: { id: 'P1' }, request: new Request('http://localhost/projects/P1'), url: new URL('http://localhost/projects/P1'), pattern: '/projects/:id' }; }
test('1 仅本项目且最新在前，空与单元素不改输入', () => {
  const old = { ...alert, id: 'old', created_at: '2025-12-31T23:59:59Z' }, other = { ...alert, project_id: 'P2' };
  const dashboard = { alerts: [old, other, alert] };
  expect(projectAlerts(dashboard, 'P1')).toEqual([alert, old]); expect(dashboard.alerts).toEqual([old, other, alert]);
  expect(projectAlerts({}, 'P1')).toEqual([]); expect(projectAlerts({ alerts: [alert] }, 'P1')).toEqual([alert]);
  expect(alertCounts(dashboard)).toEqual(new Map([['P1', 2], ['P2', 1]]));
});
test('2 四种名与未知原文、详情转义换行、时间与可选任务链接', () => {
  for (const kind of [...Object.keys(ALERT_KINDS), 'future_kind', '__proto__', 'constructor']) {
    const html = markup(createElement(ProjectAlerts, { alerts: [{ ...alert, kind }] }));
    expect(html).toContain(Object.hasOwn(ALERT_KINDS, kind) ? ALERT_KINDS[kind] : kind); expect(html).toContain('原文\nsecond &lt;script&gt;'); expect(html).toContain(alert.created_at); expect(html).toContain('href="/tasks/T-1"');
  }
  expect(markup(createElement(ProjectAlerts, { alerts: [{ ...alert, task_id: null }] }))).not.toContain('/tasks/');
  expect(markup(createElement(ProjectAlerts, { alerts: [] }))).not.toContain('board-alerts');
});
test('3 dismiss 参数、门禁禁用、拒绝原文及空 id 边界', async () => {
  const g = gateway(); const submit = (id: string, gw: KernelGateway | null = g) => action({ ...args(gw), request: new Request('http://localhost/', { method: 'POST', body: new URLSearchParams({ intent: 'dismiss', alert_id: id }) }) });
  await submit('a1'); expect(g.write).toHaveBeenCalledExactlyOnceWith({ op: 'project.alert.dismiss', params: { alert_id: 'a1' } });
  expect(markup(createElement(ProjectAlerts, { alerts: [alert] }), false)).toMatch(/<button[^>]*disabled=""[^>]*title="内核离线"/);
  const data = await loader(args(gateway()));
  for (const error of ['拒绝授权', '请求超时', '内核未连接']) {
    g.write = vi.fn(async () => ({ status: 409, body: { ok: false, error } })); const result = await submit('a1');
    expect(result).toEqual({ ok: false, error });
    const Stub = createRoutesStub([{ path: '/', Component: ProjectBoard }]);
    const html = renderToStaticMarkup(createElement(Stub, { hydrationData: { loaderData: { '0': data }, actionData: { '0': result } } }));
    expect(html).toContain(error); expect(html).toContain('知道了'); expect(data.alerts).toEqual([alert]);
  }
  const call = vi.fn(); const real = createGateway({ core: { call, restart: vi.fn() }, home: '/tmp' });
  for (const id of ['bad\0', 'x'.repeat(513)]) expect(await submit(id, real)).toMatchObject({ ok: false, error: expect.stringContaining('已拒绝') });
  expect(call).not.toHaveBeenCalled();
  for (const id of ['', 'a', 'x'.repeat(512)]) { await submit(id, real); expect(call).toHaveBeenLastCalledWith('project.alert.dismiss', { alert_id: id }); }
  expect(await submit('a1', null)).toEqual({ error: '内核未连接：服务未接入网关' });
});
test('4 侧栏显示计数与 aria，零报警没有键与标签', async () => {
  const data = await loadShell(gateway()); expect(data.projects[0].alerts).toBe(1); expect(data.projects[1]).not.toHaveProperty('alerts'); expect(data.environment).toEqual({ probed: false });
  const html = markup(createElement(ProjectList, { projects: data.projects })); expect(html).toContain('报警 1'); expect(html).toMatch(/aria-label="项目甲[^"次]*报警 1"/);
  expect(markup(createElement(ProjectList, { projects: [data.projects[1]] }))).not.toContain('报警');
});
test('5 dashboard 失败保持看板与在线外壳，恢复后报警重现', async () => {
  for (const error of ['读盘失败', '内核响应超时', '内核未连接', '已拒绝：禁止读取']) {
    const g = gateway(error), data = await loader(args(g)); expect(data).not.toHaveProperty('error'); expect(data.alertsError).toBe(error);
    const Stub = createRoutesStub([{ path: '/projects/:id', Component: ProjectBoard }]);
    const html = renderToStaticMarkup(createElement(Stub, { initialEntries: ['/projects/P1'], hydrationData: { loaderData: { '0': data } } }));
    expect(html).toContain('报警读取失败：' + error); expect(html).toContain('board-columns');
    const shell = await loadShell(g); expect(shell.kernel.ok).toBe(true); expect(shell.projects).toHaveLength(2); shell.projects.forEach(project => expect(project).not.toHaveProperty('alerts'));
  }
  expect((await loader(args(gateway()))).alerts).toEqual([alert]);
});
test('6 两种事件的不同 subject 都重载根和项目', async () => {
  const url = new URL('http://localhost/projects/P1'); const args = { currentUrl: url, nextUrl: url, currentParams: { id: 'P1' }, nextParams: { id: 'P1' }, defaultShouldRevalidate: false };
  for (const [kind, subject] of [['project.alert', 'P1'], ['project.alert_dismissed', 'a1']]) await revalidateForEvents([{ kind, subject, seq: 1, payload: {} }], () => { expect(shouldRevalidate(args)).toBe(true); expect(rootRevalidate(args)).toBe(true); });
});
