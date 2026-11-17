import { ensureBuilt } from './fixtures/ensure-built.ts';
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createMemoryRouter, createRequestHandler, RouterContextProvider, type LoaderFunctionArgs, type ServerBuild } from 'react-router';
import { createServer } from 'vite';
import ts from 'typescript';
import { expect, it, vi } from 'vitest';
import { createLoadContext, servicesContext, type KernelGateway } from '../app/load-context.ts';
import { loader, shouldRevalidate } from '../app/root.tsx';
import { revalidateForEvents } from '../app/lib/live-routes.ts';

const root = fileURLToPath(new URL('../', import.meta.url));
function gateway(): KernelGateway {
  return { read: vi.fn(async (request: unknown) => ({ status: 200, body: { ok: true, payload: (request as { method: string }).method === 'project.list' ? { projects: [{ project: { id: 'P-1', display_name: '演示项目甲' } }] } : { tasks: [] } } })), write: vi.fn() };
}
const args = (g: KernelGateway | null) => ({ context: createLoadContext({ gateway: g })(), request: new Request('http://localhost/'), params: {}, url: new URL('http://localhost/'), pattern: '/' }) satisfies LoaderFunctionArgs;
it('每个请求有独立 provider，services 和 null 原样交付', () => {
  const services = { gateway: gateway() }; const get = createLoadContext(services);
  const a = get(); const b = get();
  expect(a).toBeInstanceOf(RouterContextProvider); expect(a).not.toBe(b);
  expect(a.get(servicesContext)).toBe(services);
  expect(createLoadContext({ gateway: null })().get(servicesContext).gateway).toBeNull();
  expect(() => new RouterContextProvider().get(servicesContext)).toThrow('No value found for context');
});
it('不同查询串加载共用同一上下文键', async () => {
  const a = await import('../app/load-context.ts' + '?first');
  const b = await import('../app/load-context.ts' + '?second');
  expect(a.servicesContext).toBe(b.servicesContext);
  expect(a.servicesContext).toBe(servicesContext);
});
it('根 loader 白名单读取项目列表和详情，空列表和单项目都有效', async () => {
  const g = gateway(); const result = await loader(args(g));
  expect(g.read).toHaveBeenNthCalledWith(1, { method: 'project.list', params: {} });
  expect(g.read).toHaveBeenNthCalledWith(2, { method: 'project.get', params: { project_id: 'P-1' } });
  expect(g.read).toHaveBeenNthCalledWith(3, { method: 'dashboard.get', params: {} });
  expect(g.read).toHaveBeenNthCalledWith(4, { method: 'skills.list', params: {} });
  expect(g.read).toHaveBeenCalledTimes(4);
  expect(g.write).not.toHaveBeenCalled();
  expect(result.projects).toEqual([{ id: 'P-1', name: '演示项目甲', counts: { running: 0, needs_you: 0 }, done: 0 }]);
  vi.mocked(g.read).mockResolvedValue({ status: 200, body: { ok: true, payload: { projects: [] } } });
  expect((await loader(args(g))).projects).toEqual([]);
});
it('未接入 gateway 返回离线而不抛错', async () => {
  expect(await loader(args(null))).toEqual({ kernel: { ok: false, error: '内核未连接：服务未接入网关' }, projects: [], tasks: [], overview: { waiting: {}, details: {}, recent: [], doneCount: 0, allDoneHref: undefined, error: '内核未连接：服务未接入网关', listError: '内核未连接：服务未接入网关' }, clock: { now: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T.*Z$/), timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone } });
});
it('503、504、409 错误原样传递', async () => {
  for (const [status, error] of [[503, '内核未连接：离线'], [504, '内核响应超时'], [409, '项目读取冲突']] as const) {
    const g = gateway(); vi.mocked(g.read).mockResolvedValue({ status, body: { ok: false, error } });
    expect(await loader(args(g))).toEqual({ kernel: { ok: false, error }, projects: [], tasks: [], overview: { waiting: {}, details: {}, recent: [], doneCount: 0, allDoneHref: undefined, error, listError: error }, clock: { now: expect.stringMatching(/^\d{4}-\d{2}-\d{2}T.*Z$/), timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone } });
  }
});
async function checkSSR(build: ServerBuild, mode: string) {
  const g = gateway();
  const response = await createRequestHandler(build, mode)(new Request('http://localhost/'), createLoadContext({ gateway: g })());
  expect(response.status).toBe(200); expect(await response.text()).toContain('演示项目甲');
  expect(g.read).toHaveBeenNthCalledWith(1, { method: 'project.list', params: {} });
  expect(g.read).toHaveBeenNthCalledWith(2, { method: 'project.get', params: { project_id: 'P-1' } });
  expect(g.read).toHaveBeenNthCalledWith(3, { method: 'dashboard.get', params: {} });
  expect(g.read).toHaveBeenNthCalledWith(4, { method: 'skills.list', params: {} });
  expect(g.read).toHaveBeenCalledTimes(4);
}
it('开发 Vite SSR 取得 Node 注入的网关，不监听端口', async () => {
  const vite = await createServer({ root, server: { middlewareMode: true, hmr: false, watch: null } });
  try { await checkSSR(await vite.ssrLoadModule('virtual:react-router/server-build') as ServerBuild, 'development'); }
  finally { await vite.close(); }
});
it('生产构建 SSR 取得 Node 注入的网关', async () => {
  ensureBuilt();
  await checkSSR(await import(pathToFileURL(root + 'build/server/index.js').href), 'production');
});
it('HTTP 两处分支都配置 getLoadContext', () => {
  const source = ts.createSourceFile('http.ts', readFileSync(root + 'server/http.ts', 'utf8'), ts.ScriptTarget.Latest, true);
  const calls: ts.CallExpression[] = [];
  function visit(node: ts.Node) {
    if (ts.isCallExpression(node) && node.expression.getText(source) === 'createRequestHandler') calls.push(node);
    ts.forEachChild(node, visit);
  }
  visit(source); expect(calls).toHaveLength(2);
  for (const call of calls) {
    const options = call.arguments[0]; expect(ts.isObjectLiteralExpression(options)).toBe(true);
    if (ts.isObjectLiteralExpression(options)) expect(options.properties.some(p => p.name?.getText(source) === 'getLoadContext')).toBe(true);
  }
});
it('根重验覆盖 task/project/bug/merge/session/gate，忽略 config', async () => {
  let count = 0;
  const router = createMemoryRouter([{ path: '/', loader: () => ++count, shouldRevalidate }]);
  try {
    await vi.waitFor(() => expect(router.state.initialized).toBe(true));
    for (const kind of ['task.updated', 'project.added', 'bug.fix_round_started', 'merge.rebased', 'config.changed', 'session.started', 'session.ended', 'gate.result']) {
      const before = count;
      await revalidateForEvents([{ seq: 1, kind, subject: 'T-1', payload: {} }], () => router.revalidate());
      expect(count - before).toBe(/^config\./.test(kind) ? 0 : 1);
    }
  } finally { router.dispose(); }
});
it('可选 unread 和 uploads 注入 loader，旧调用保持 undefined', async () => {
  const { createUploads } = await import('../server/uploads.ts');
  const unread = { unreadByTask: () => [] }, uploads = createUploads('/tmp/context-upload-unused');
  const context = createLoadContext({ gateway: null, unread, uploads })();
  const router = createMemoryRouter([{ path: '/', loader: () => context.get(servicesContext) }]);
  try {
    await vi.waitFor(() => expect(router.state.initialized).toBe(true));
    expect(router.state.loaderData['0']).toEqual({ gateway: null, unread, uploads });
  } finally { router.dispose(); }
  const old = createLoadContext({ gateway: null })().get(servicesContext);
  expect(old.uploads).toBeUndefined(); expect(old.unread).toBeUndefined();
});
