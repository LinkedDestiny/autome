import { setRelatedSubjects } from '../app/lib/live-routes.ts';
import { shouldRevalidate as taskShouldRevalidate } from '../app/routes/tasks.$id.tsx';
import type { ShouldRevalidateFunctionArgs } from 'react-router';
const taskArgs: ShouldRevalidateFunctionArgs = { currentUrl: new URL('http://localhost/tasks/T-1'), nextUrl: new URL('http://localhost/tasks/T-1'), currentParams: { id: 'T-1' }, nextParams: { id: 'T-1' }, defaultShouldRevalidate: false };
import { createMemoryRouter } from 'react-router';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import ts from 'typescript';
import { fileURLToPath } from 'node:url';
const webRoot = fileURLToPath(new URL('../', import.meta.url));
import { byKind, bySubjectParam, liveShouldRevalidate, revalidateAll as enqueueAll, revalidateForEvents as enqueueEvents } from '../app/lib/live-routes.ts';
import { shouldRevalidate as addProjectShouldRevalidate } from '../app/routes/projects.new.tsx';
const event = (kind = 'task.updated', subject = 'T-1') => ({ seq: 1, kind, subject, payload: {} });
const routers: ReturnType<typeof createMemoryRouter>[] = [];
beforeEach(() => vi.useFakeTimers());
afterEach(() => { for (const router of routers.splice(0)) router.dispose(); vi.useRealTimers(); });
function revalidateForEvents(...args: Parameters<typeof enqueueEvents>) {
  const result = enqueueEvents(...args); vi.advanceTimersByTime(1000); return result;
}
function revalidateAll(...args: Parameters<typeof enqueueAll>) {
  const result = enqueueAll(...args); vi.advanceTimersByTime(1000); return result;
}
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
it('添加页随项目事件及 reload 重验，忽略任务事件并保留导航与表单重验', async () => {
  const read = vi.fn(() => null);
  const router = createMemoryRouter([{ path: '/projects/new', loader: read, action: () => null,
    shouldRevalidate: addProjectShouldRevalidate }], { initialEntries: ['/projects/new?path=~/repo'] });
  routers.push(router);
  await vi.waitFor(() => expect(router.state.initialized).toBe(true));
  read.mockClear();
  for (const kind of ['project.added', 'project.removed']) {
    await revalidateForEvents([event(kind, 'P-1')], () => router.revalidate());
  }
  expect(read).toHaveBeenCalledTimes(2);
  await revalidateForEvents([event('task.updated')], () => router.revalidate());
  await revalidateForEvents([], () => router.revalidate());
  expect(read).toHaveBeenCalledTimes(2);
  await revalidateAll(() => router.revalidate());
  expect(read).toHaveBeenCalledTimes(3);
  await router.navigate('/projects/new?path=~/other');
  expect(read).toHaveBeenCalledTimes(4);
  await router.navigate('/projects/new?path=~/other', { formMethod: 'post', formData: new FormData() });
  expect(read).toHaveBeenCalledTimes(5);
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
it('同一秒内五批事件只在后沿重验一次，包括 reload', async () => {
  const revalidate = vi.fn();
  const batches = [enqueueEvents([event()], revalidate)];
  for (let i = 1; i < 5; i++) {
    await vi.advanceTimersByTimeAsync(199);
    batches.push(enqueueEvents([event()], revalidate));
  }
  batches.push(enqueueAll(revalidate));
  await vi.advanceTimersByTimeAsync(203);
  expect(revalidate).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1);
  await Promise.all(batches);
  expect(revalidate).toHaveBeenCalledOnce();
});
it('窗口内不同前缀取并集，reload 也并入同一次', async () => {
  const c = await setup();
  const first = enqueueEvents([event('project.updated')], () => c.router.revalidate());
  const second = enqueueEvents([event('task.updated')], () => c.router.revalidate());
  await vi.advanceTimersByTimeAsync(1000); await Promise.all([first, second]);
  expect(c.counts).toEqual({ project: 1, task: 1, other: 0 });
  const third = enqueueEvents([event()], () => c.router.revalidate());
  const fourth = enqueueAll(() => c.router.revalidate());
  await vi.advanceTimersByTimeAsync(1000); await Promise.all([third, fourth]);
  expect(c.counts).toEqual({ project: 2, task: 2, other: 1 });
});
it('到点后新事件另开窗口，旧完成不丢掉等待中的新事件', async () => {
  let release!: () => void;
  const old = enqueueEvents([event()], () => new Promise<void>(resolve => { release = resolve; }));
  await vi.advanceTimersByTimeAsync(1000);
  const url = new URL('http://localhost/');
  const args = { currentUrl: url, nextUrl: url, currentParams: {}, defaultShouldRevalidate: false } as never;
  const next = vi.fn(() => { expect(liveShouldRevalidate(byKind('project.'))(args)).toBe(true); });
  const newer = enqueueEvents([event('project.updated')], next);
  release(); await old;
  await vi.advanceTimersByTimeAsync(999); expect(next).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1); await newer;
  expect(next).toHaveBeenCalledOnce();
  expect(liveShouldRevalidate(byKind('project.'))(args)).toBe(false);
  const failed = enqueueEvents([event()], async () => { throw Error('loader failed'); });
  const rejected = expect(failed).rejects.toThrow('loader failed');
  await vi.advanceTimersByTimeAsync(1000); await rejected;
  expect(liveShouldRevalidate(byKind('task.'))(args)).toBe(false);
  const recovered = enqueueEvents([event('project.updated')], next);
  await vi.advanceTimersByTimeAsync(1000); await recovered;
  expect(next).toHaveBeenCalledTimes(2);
});

it('前置 task 与 merge 事件重验依赖任务，自己的原有事件仍重验', async () => {
  const clear = setRelatedSubjects('T-1', ['T-9', 'T-8']);
  try {
    for (const [kind, subject] of [['task.updated', 'T-9'], ['merge.completed', 'T-8'], ['session.finished', 'T-1']]) {
      await revalidateForEvents([event(kind, subject)], async () => {
        expect(taskShouldRevalidate(taskArgs)).toBe(true);
      });
    }
  } finally { clear(); }
});
it('无关任务、前置的非 task/merge 事件与空依赖不触发', async () => {
  const clear = setRelatedSubjects('T-1', ['T-9']);
  try {
    for (const [kind, subject] of [['task.updated', 'T-8'], ['session.finished', 'T-9'], ['', 'T-9'], ['task.updated', '']]) {
      await revalidateForEvents([event(kind, subject)], async () => {
        expect(taskShouldRevalidate(taskArgs)).toBe(false);
      });
    }
    setRelatedSubjects('T-1', []);
    await revalidateForEvents([event('task.updated', 'T-9')], async () => expect(taskShouldRevalidate(taskArgs)).toBe(false));
  } finally { clear(); }
});
it('after 替换与卸载清除旧前置，登记按任务隔离', async () => {
  const clear = setRelatedSubjects('T-1', ['T-9']);
  const other = setRelatedSubjects('T-2', ['T-9']);
  try {
    setRelatedSubjects('T-1', ['T-8']);
    await revalidateForEvents([event('task.updated', 'T-9')], async () => expect(taskShouldRevalidate(taskArgs)).toBe(false));
    await revalidateForEvents([event('task.updated', 'T-8')], async () => expect(taskShouldRevalidate(taskArgs)).toBe(true));
    clear();
    await revalidateForEvents([event('task.updated', 'T-8')], async () => expect(taskShouldRevalidate(taskArgs)).toBe(false));
    await revalidateForEvents([event('task.updated', 'T-9')], async () => {
      expect(taskShouldRevalidate({ ...taskArgs, currentParams: { id: 'T-2' } })).toBe(true);
    });
  } finally { clear(); other(); }
});

it('Loop 和技能收到 config.changed 与 project 重新验证', async () => {
  for (const name of ['loop', 'skills']) {
    const { shouldRevalidate } = await import('../app/routes/' + name + '.tsx');
    for (const kind of ['config.changed', 'project.updated', 'task.updated']) await revalidateForEvents([event(kind)], async () => {
      expect(shouldRevalidate({ ...taskArgs, currentUrl: new URL('http://localhost/' + name), nextUrl: new URL('http://localhost/' + name) })).toBe(kind !== 'task.updated');
    });
  }
});
