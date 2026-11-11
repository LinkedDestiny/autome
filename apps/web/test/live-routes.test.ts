import { createMemoryRouter } from 'react-router';
import { afterEach, expect, it, vi } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import ts from 'typescript';
import { fileURLToPath } from 'node:url';
const webRoot = fileURLToPath(new URL('../', import.meta.url));
import { byKind, bySubjectParam, liveShouldRevalidate, revalidateAll, revalidateForEvents } from '../app/lib/live-routes.ts';
const event = (kind = 'task.updated', subject = 'T-1') => ({ seq: 1, kind, subject, payload: {} });
const routers: ReturnType<typeof createMemoryRouter>[] = [];
afterEach(() => { for (const router of routers.splice(0)) router.dispose(); });
async function setup() {
  const counts = { project: 0, task: 0, other: 0 };
  let version = 0;
  let block: Promise<void> | undefined;
  const loader = (key: keyof typeof counts) => async () => { counts[key]++; const value = version; await block; return value; };
  const router = createMemoryRouter([{
    id: 'project', path: '/', loader: loader('project'), shouldRevalidate: liveShouldRevalidate(byKind('project.', 'rule.')),
    children: [{ id: 'task', path: 'tasks/:taskId', loader: loader('task'), action: () => null,
      shouldRevalidate: liveShouldRevalidate(bySubjectParam(['task.', 'session.', 'gate.', 'merge.'], 'taskId')),
      children: [{ id: 'other', path: '', loader: loader('other'), shouldRevalidate: liveShouldRevalidate((e) => e.subject === 'T-2') }],
    }],
  }], { initialEntries: ['/tasks/T-1'] });
  routers.push(router); await vi.waitFor(() => expect(router.state.initialized).toBe(true));
  counts.project = counts.task = counts.other = 0;
  return { router, counts, update() { version++; }, block(p?: Promise<void>) { block = p; } };
}
it('task.updated 只重验 T-1；相同 href 不同 URL 对象', async () => {
  const c = await setup(); c.update();
  await revalidateForEvents([event()], () => c.router.revalidate());
  expect(c.counts).toEqual({ project: 0, task: 1, other: 0 }); expect(c.router.state.loaderData.task).toBe(1);
  await revalidateForEvents([event('task.updated', 'T-2')], () => c.router.revalidate());
  expect(c.counts).toEqual({ project: 0, task: 1, other: 1 });
});
it('project.* 只重验项目路由；空批及未知前缀不重验', async () => {
  const c = await setup();
  await revalidateForEvents([event('project.added', 'P-1')], () => c.router.revalidate());
  await revalidateForEvents([], () => c.router.revalidate());
  await revalidateForEvents([event('', '')], () => c.router.revalidate());
  expect(c.counts).toEqual({ project: 1, task: 0, other: 0 });
});
it('在途重验叠加新批，两批路由最终取得新数据', async () => {
  const c = await setup(); let release!: () => void;
  c.block(new Promise<void>(r => { release = r; })); c.update();
  const first = revalidateForEvents([event()], () => c.router.revalidate());
  expect(c.counts.task).toBe(1); c.update();
  const second = revalidateForEvents([event('project.updated')], () => c.router.revalidate());
  release(); await Promise.all([first, second]); c.block();
  expect(c.router.state.loaderData.task).toBe(2); expect(c.router.state.loaderData.project).toBe(2);
  await c.router.revalidate(); expect(c.counts.other).toBe(1);
});
it('被中断的旧重验先 settle 时不清掉仍在进行的新批', async () => {
  let releaseNew!: () => void;
  const first = revalidateForEvents([event()], async () => {});
  const second = revalidateForEvents([event('project.updated')], () => new Promise<void>(r => { releaseNew = r; }));
  await first;
  const url = new URL('http://localhost/x');
  const args = { currentUrl: url, nextUrl: new URL(url.href), currentParams: {}, defaultShouldRevalidate: false } as never;
  expect(liveShouldRevalidate(byKind('project.'))(args)).toBe(true);
  releaseNew(); await second;
  expect(liveShouldRevalidate(byKind('project.'))(args)).toBe(false);
});
it('事件待处理中参数导航与表单仍遵循默认重验', async () => {
  const c = await setup(); let release!: () => void;
  const pending = revalidateForEvents([], () => new Promise<void>(r => { release = r; }));
  await c.router.navigate('/tasks/T-2'); expect(c.counts.task).toBe(1);
  const before = { ...c.counts };
  await c.router.navigate('/tasks/T-2', { formMethod: 'post', formData: new FormData() });
  expect(c.counts.project).toBeGreaterThan(before.project); expect(c.counts.task).toBeGreaterThan(before.task);
  release(); await pending;
});
it('reload 重验所有声明 live 的路由', async () => {
  const c = await setup(); await revalidateAll(() => c.router.revalidate());
  expect(c.counts).toEqual({ project: 1, task: 1, other: 1 });
});
function missingLive(files: string[]) {
  return files.filter(file => {
    const source = ts.createSourceFile(file, readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const exports = new Map<string, string>();
    for (const statement of source.statements) {
      if (!ts.canHaveModifiers(statement) || !ts.getModifiers(statement)?.some(m => m.kind === ts.SyntaxKind.ExportKeyword)) continue;
      if (ts.isFunctionDeclaration(statement) && statement.name) exports.set(statement.name.text, statement.getText(source));
      if (ts.isVariableStatement(statement)) for (const d of statement.declarationList.declarations) exports.set(d.name.getText(source), d.initializer?.getText(source) ?? '');
    }
    return (exports.has('loader') || exports.has('clientLoader')) && !exports.get('shouldRevalidate')?.startsWith('liveShouldRevalidate(');
  });
}
it('路由 loader 导出守卫报告缺少 liveShouldRevalidate 的文件', () => {
  const bad = resolve(webRoot, 'test/fixtures/live-routes-bad/missing.tsx'); expect(missingLive([bad])).toEqual([bad]);
  const files = readdirSync(resolve(webRoot, 'app/routes'), { recursive: true }).filter(f => /\.[jt]sx?$/.test(String(f))).map(f => resolve(webRoot, 'app/routes', String(f)));
  expect(missingLive([resolve(webRoot, 'app/root.tsx'), ...files])).toEqual([]);
});
