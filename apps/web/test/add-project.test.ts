import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider, RouterContextProvider } from 'react-router';
import { expect, test, vi } from 'vitest';
import { action, loader, AddProjectView } from '../app/routes/projects.new.tsx';
import { canConfirm, confirmDefaults, probeView, type Probe, type AddResult } from '../app/components/add-project-model.ts';
import { nativePicker } from '../app/components/native-picker.ts';
import { pathCompleteClient } from '../app/components/path-complete-client.ts';
import { servicesContext, type WebServices } from '../app/load-context.ts';
import { WriteGateProvider } from '../app/lib/live.tsx';
import { Shell } from '../app/components/shell.tsx';
import { buildCandidates } from '../app/components/palette-model.ts';
export const probe: Probe = { path: '/home/code', exists: true, is_repo_root: true, members: [], suggest_workspace: false, suggested_docs_repo: null };
function context(services: WebServices) { const c = new RouterContextProvider(); c.set(servicesContext, services); return c; }
const read = vi.fn(async () => ({ status: 200, body: { ok: true, payload: probe } }));
const write = vi.fn(async () => ({ status: 200, body: { ok: true, payload: { project: { id: 'p' } } } }));
const args = (form: Record<string, string> = {}) => ({ request: new Request('http://localhost/projects/new', { method: 'POST', body: new URLSearchParams({ intent: 'confirm', path: '~/code', ...form }) }), url: new URL("http://localhost/projects/new"), pattern: "/projects/new", params: {}, context: context({ gateway: { read, write } }) });
function html(result?: AddResult, allowed = true, p = probe) {
  const router = createMemoryRouter([{ path: '/', element: createElement(WriteGateProvider, { value: { allowed, reason: '离线原因' } }, createElement(AddProjectView, { checked: { display: '~/code', probe: p }, result })) }]);
  return renderToString(createElement(RouterProvider, { router }));
}
test('1 probe 四种事实提示', () => {
  expect(probeView({ ...probe, exists: false }).join()).toContain('目录不存在');
  expect(probeView({ ...probe, is_repo_root: false }).join()).toContain('不是 Git 仓库');
  expect(probeView(probe).join()).toContain('直接接管');
  expect(probeView({ ...probe, members: [{ name: 'docs', default_branch: 'main' }], suggest_workspace: true }).join()).toContain('看起来是工作区');
});
test('2 默认选择与成员下拉', () => {
  const p = { ...probe, members: [{ name: 'docs', default_branch: 'main' }], suggest_workspace: true, suggested_docs_repo: 'docs' };
  expect(confirmDefaults(p)).toEqual({ workspace: true, docs_repo: 'docs', members: ['docs'] });
  expect(html(undefined, true, p)).toContain('<option value="docs" selected="">docs</option>');
});
test('3 确认门槛：无检查、路径修改、相同路径', () => {
  expect(canConfirm('', { display: '' })).toBe(false);
  expect(canConfirm('~/other', { display: '~/code', probe })).toBe(false);
  expect(canConfirm('~/code', { display: '~/code', probe })).toBe(true);
});
test('4 loader 规范路径只读 probe，拒绝/离线/超时原文', async () => {
  const check = vi.fn(() => ({ ok: true as const, absolute: '/home/code', display: '~/code', isHome: false }));
  const services = { gateway: { read, write }, admin: { paths: { check } } } as unknown as WebServices;
  const a = { request: new Request('http://localhost/projects/new?path=/home/code'), url: new URL("http://localhost/projects/new"), pattern: "/projects/new", params: {}, context: context(services) };
  write.mockClear(); expect(await loader(a)).toEqual({ display: '~/code', probe });
  expect(read).toHaveBeenLastCalledWith({ method: 'project.probe', params: { path: '~/code' } }); expect(write).not.toHaveBeenCalled();
  for (const error of ['拒绝授权', '读盘失败', '请求超时', '内核未连接']) {
    read.mockResolvedValueOnce({ status: 503, body: { ok: false, error } } as never);
    expect(await loader(a)).toMatchObject({ error });
  }
});
test('5 action 参数限定与两条重定向', async () => {
  for (const skip of [false, true]) {
    const result = await action(args({ workspace: 'on', docs_repo: 'docs', command: 'ignored', ...(skip ? { skip_onboarding: 'on' } : {}) }));
    expect(write).toHaveBeenLastCalledWith({ op: 'project.confirm', params: { path: '~/code', workspace: true, docs_repo: 'docs', skip_onboarding: skip } });
    expect((result as Response).headers.get('Location')).toBe(skip ? '/projects/p' : '/loop?project=p&tab=project');
  }
});
test('6 内核拒绝原文呈现', async () => {
  write.mockResolvedValueOnce({ status: 409, body: { ok: false, error: '该目录已经是一个项目' } } as never);
  const result = await action(args()); expect(result).toEqual({ error: '该目录已经是一个项目' }); expect(html(result as AddResult)).toContain('该目录已经是一个项目');
});
test('7 skip 失败保留项目与设置链接，不再允许重复添加', async () => {
  write.mockResolvedValueOnce({ status: 200, body: { ok: true, payload: { project: { id: 'p' }, skip_error: 'not a git repository' } } } as never);
  const result = await action(args({ skip_onboarding: 'on' })); const rendered = html(result as AddResult);
  expect(result).not.toBeInstanceOf(Response); expect(rendered).toContain('not a git repository');
  expect(rendered).toContain('href="/projects/p"'); expect(rendered).toContain('href="/loop?project=p&amp;tab=project"'); expect(rendered).not.toContain('确认添加');
});
test('8 SSR 隐藏原生按钮，只识别函数', () => {
  expect(html()).not.toContain('选择目录'); expect(nativePicker({})).toBeUndefined(); expect(nativePicker({ autome: { pickDirectory: 'x' } } as never)).toBeUndefined();
  expect(nativePicker({ autome: { pickDirectory: async () => null } })).toBeTypeOf('function');
});
test('9 空/非空侧栏与命令面板导航', () => {
  for (const projects of [[], [{ id: 'p', name: '项目', counts: { running: 0, needs_you: 0 } }]]) {
    const router = createMemoryRouter([{ path: '/', element: createElement(Shell, { projects, banner: null, children: null }) }]);
    expect(renderToString(createElement(RouterProvider, { router }))).toContain('href="/projects/new"');
    expect(buildCandidates({ projects, tasks: [], kernel: { ok: true } }, { allowed: false })).toContainEqual(expect.objectContaining({ title: '添加项目', href: '/projects/new', disabled: false }));
  }
});
test('10 离线检查与确认禁用并带原因', () => { expect(html(undefined, false)).toMatch(/disabled="" title="离线原因"/); expect(html(undefined, false).match(/disabled="" title="离线原因"/g)).toHaveLength(2); });
test('11 补全防抖、乱序、失败与卸载取消', async () => {
  vi.useFakeTimers();
  try {
    const pending: ((value: Response) => void)[] = [];
    const fetcher = vi.fn(() => new Promise<Response>(resolve => pending.push(resolve)));
    const update = vi.fn(); const client = pathCompleteClient(fetcher, update);
    client.input(''); await vi.advanceTimersByTimeAsync(149); expect(fetcher).not.toHaveBeenCalled();
    client.input('~/a'); await vi.advanceTimersByTimeAsync(150);
    client.input('~/b'); await vi.advanceTimersByTimeAsync(150);
    pending[1](Response.json({ ok: true, payload: { entries: [{ name: 'b', path: '~/b' }], truncated: true } })); await vi.advanceTimersByTimeAsync(0);
    pending[0](Response.json({ ok: true, payload: { entries: [{ name: 'a' }], truncated: false } })); await vi.advanceTimersByTimeAsync(0);
    expect(update).toHaveBeenLastCalledWith({ entries: [{ name: 'b', path: '~/b' }], truncated: true });
    fetcher.mockRejectedValueOnce(new Error('不可达')); client.input('~/c'); await vi.advanceTimersByTimeAsync(150); expect(update.mock.lastCall?.[0].error).toContain('不可达');
    client.input('~/d'); client.dispose(); const calls = fetcher.mock.calls.length; await vi.advanceTimersByTimeAsync(150); expect(fetcher).toHaveBeenCalledTimes(calls);
  } finally { vi.useRealTimers(); }
});

test('12 四种拒绝在填表前显示且无确认表单', () => {
  for (const [kind, label] of Object.entries({ already_project: '这个目录已经是项目', nested_in_project: '它在已有项目里面', contains_project: '它包含已有项目', not_a_directory: '这不是目录' })) {
    const p = { ...probe, rejection: { kind, message: '内核拒绝原文' } };
    expect(canConfirm('~/code', { display: '~/code', probe: p })).toBe(false);
    const text = html(undefined, true, p); expect(text).toContain(label); expect(text).toContain('内核拒绝原文'); expect(text).not.toContain('name="intent"');
  }
});
test('13 already_project 链到项目', () => { expect(html(undefined, true, { ...probe, rejection: { kind: 'already_project', message: '原文', project_id: 'p /' } })).toContain('href="/projects/p%20%2F"'); });
test('14 嵌套两种链到对方', () => { for (const kind of ['nested_in_project', 'contains_project']) expect(html(undefined, true, { ...probe, rejection: { kind, message: '原文', project_id: 'other' } })).toContain('href="/projects/other"'); });
test('15 未知 kind 回退 message，null 仍可确认', () => { for (const kind of ['future', 'toString', '__proto__', '']) expect(html(undefined, true, { ...probe, rejection: { kind, message: '未来拒绝' } })).toContain('未来拒绝'); expect(canConfirm('x', { display: 'x', probe: { ...probe, rejection: null } })).toBe(true); });

test('添加成功后项目信息地址对项目 ID 编码', async () => {
  write.mockResolvedValueOnce({ status: 200, body: { ok: true, payload: { project: { id: 'p&other=1' } } } } as never);
  const result = await action(args());
  expect((result as Response).headers.get('Location')).toBe('/loop?project=p%26other%3D1&tab=project');
});

test('新控件保留路径工作区文档仓库与跳过字段', () => {
  const text = html();
  for (const name of ['path', 'workspace', 'docs_repo', 'skip_onboarding']) expect(text).toContain('name="' + name + '"');
  expect(text.match(/class="chk"/g)).toHaveLength(2);
  expect(text).toContain('class="inp"');
  expect(text).not.toContain('data-testid="pick-directory"');
});
