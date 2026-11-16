import { createElement, type DependencyList, type EffectCallback } from 'react';
import { renderToString } from 'react-dom/server';
import { createRoutesStub, RouterContextProvider } from 'react-router';
import { readFileSync } from 'node:fs';
import { afterEach, expect, it, vi } from 'vitest';
import TaskPage, { loader } from '../app/routes/tasks.$id.tsx';
import { servicesContext } from '../app/load-context.ts';
import { bySubjectParamOrRelated, setRelatedSubjects } from '../app/lib/live-routes.ts';

const captured = vi.hoisted(() => [] as { effect: EffectCallback; deps: DependencyList | undefined }[]);
vi.mock('react', async importOriginal => ({
  ...await importOriginal<typeof import('react')>(),
  useEffect: (effect: EffectCallback, deps?: DependencyList) => { captured.push({ effect, deps }); },
}));
const matches = bySubjectParamOrRelated(['task.', 'session.'], 'id', ['task.', 'merge.']);
const match = (subject: string, kind = 'task.updated') => matches({ seq: 1, kind, subject, payload: {} }, { id: 'T-1' });
afterEach(() => { captured.length = 0; setRelatedSubjects('T-1', [])(); });

async function render(after: string[]) {
  const payload = JSON.parse(readFileSync(new URL('./fixtures/w7/feature.json', import.meta.url), 'utf8'));
  payload.task.id = 'T-1'; payload.task.after = after;
  const context = new RouterContextProvider();
  context.set(servicesContext, { gateway: { read: vi.fn(async () => ({ status: 200, body: { ok: true, payload } })), write: vi.fn() } });
  const url = new URL('http://localhost/tasks/T-1?tab=decisions');
  const data = await loader({ context, params: { id: 'T-1' }, request: new Request(url), url, pattern: '/tasks/:id' });
  captured.length = 0;
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  renderToString(createElement(Stub, { initialEntries: ['/tasks/T-1?tab=decisions'], hydrationData: { loaderData: { task: data } } }));
  const effects = captured.filter(item => item.deps?.[0] === 'T-1');
  expect(effects).toHaveLength(1);
  expect(effects[0].deps).toEqual(['T-1', after]);
  return effects[0];
}

it('挂载时登记前置，保留自身事件并拒绝无关事件', async () => {
  const { effect } = await render(['T-9']);
  expect(match('T-9')).toBe(false);
  const cleanup = effect();
  try {
    expect(match('T-9')).toBe(true);
    expect(match('T-9', 'merge.completed')).toBe(true);
    expect(match('T-1', 'session.finished')).toBe(true);
    expect(match('T-8')).toBe(false);
    expect(match('T-9', 'session.finished')).toBe(false);
    expect(match('')).toBe(false);
  } finally { if (typeof cleanup === 'function') cleanup(); }
});

it('after 变化时先清旧再登记新，空依赖清除全部前置', async () => {
  const first = await render(['T-9', 'T-8']);
  const cleanup = first.effect();
  expect(match('T-9')).toBe(true); expect(match('T-8')).toBe(true);
  const next = await render(['T-7']);
  expect(next.deps).not.toEqual(first.deps);
  expect(typeof cleanup).toBe('function');
  if (typeof cleanup === 'function') cleanup();
  expect(match('T-9')).toBe(false); expect(match('T-8')).toBe(false);
  const clearNext = next.effect();
  expect(match('T-7')).toBe(true); expect(match('T-9')).toBe(false);
  const empty = await render([]);
  if (typeof clearNext === 'function') clearNext();
  const clearEmpty = empty.effect();
  expect(match('T-7')).toBe(false); expect(match('T-1')).toBe(true);
  if (typeof clearEmpty === 'function') clearEmpty();
});

it('卸载时清除登记，重新挂载可再次登记', async () => {
  const { effect } = await render(['T-9']);
  const cleanup = effect();
  expect(match('T-9')).toBe(true);
  expect(typeof cleanup).toBe('function');
  if (typeof cleanup === 'function') cleanup();
  expect(match('T-9')).toBe(false);
  const remount = effect();
  expect(match('T-9')).toBe(true);
  if (typeof remount === 'function') remount();
  expect(match('T-9')).toBe(false);
});

it('依赖详情的新状态优先于未刷新的根 loader，缺失时仍回退根状态', async () => {
  const { TaskDependencies } = await import('../app/components/task-dependencies.tsx');
  const payload = JSON.parse(readFileSync(new URL('./fixtures/w7/feature.json', import.meta.url), 'utf8'));
  payload.task.after = ['T-9', 'T-8', 'T-7'];
  payload.task.blocked_by = [{ id: 'T-9', state: { state: 'active', node: 'implement' } }];
  const Stub = createRoutesStub([{ id: 'root', path: '/', children: [{ path: 'dependencies', Component: () => createElement(TaskDependencies, { data: payload, reason: '只读' }) }] }]);
  const html = renderToString(createElement(Stub, { initialEntries: ['/dependencies'], hydrationData: { loaderData: { root: { tasks: [
    { id: 'T-9', state: { state: 'active', node: 'await_design_approval' } },
    { id: 'T-8', state: { state: 'active', node: 'await_design_approval' } },
  ] } } } }));
  expect(html.replaceAll('<!-- -->', '')).toMatch(/T-9[^<]*实现/);
  expect(html.replaceAll('<!-- -->', '')).toMatch(/T-8[^<]*等你批准/);
  expect(html.replaceAll('<!-- -->', '')).toMatch(/T-7[^<]*状态未知/);
});
