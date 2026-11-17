import { rm } from 'node:fs/promises';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { afterAll, expect, test } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { createOverviewSummary } from '../server/overview-summary.ts';
import { loadOverview } from '../server/overview.ts';
import { loadShell } from '../app/components/shell-data.ts';
import { ClockProvider, currentClock } from '../app/lib/clock.tsx';
import Overview from '../app/routes/_index.tsx';
import { w9Kernel } from './fixtures/w9-replies.ts';
async function teardown({ root, core, taskIds }: { root: string; core: CoreSupervisor; taskIds: string[] }) {
  try { for (const task_id of taskIds) { try { await core.call('task.stop', { task_id }); } catch { /* Terminal or waiting tasks reject stop. */ } } }
  finally { await core.stop(); await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); }
}
const wt = resolve(import.meta.dirname, '../../..');
const cleanups: (() => Promise<unknown>)[] = [];
afterAll(async () => { for (const close of cleanups.reverse()) await close(); });
async function render(core: CoreSupervisor, home: string) {
  const gateway = createGateway({ core, home }), clock = currentClock();
  const shell = await loadShell(gateway);
  const overview = await loadOverview({ gateway, overviewSummary: createOverviewSummary((method, params) => core.call(method, params)) }, shell, clock);
  expect(overview.error).toBeUndefined();
  const data = { ...shell, clock, overview };
  const Stub = createRoutesStub([{ id: 'root', path: '/', Component: () => h(ClockProvider, { initial: clock }, h(Overview)) }]);
  return { data, html: renderToStaticMarkup(h(Stub, { hydrationData: { loaderData: { root: data } } })) };
}
let active: { core: CoreSupervisor; home: string } | undefined;
async function activeFixture() {
  if (active) return active;
  const root = mkdtempSync(join(tmpdir(), 'm03-states-')), home = join(root, 'home');
  const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
  cleanups.push(() => teardown({ root, core, taskIds: ['T-1', 'T-2', 'T-3', 'T-4'] }));
  await promisify(execFile)('sh', [join(wt, 'apps/web/test/fixtures/w5-states.sh'), root, wt], { timeout: 60_000 });
  await core.start(); active = { core, home }; return active;
}
test('idle 真实空账本显示结论、两张虚线卡与零统计', async () => {
  const fixture = await w9Kernel();
  try {
    const { html, data } = await render(fixture.core, fixture.automeHome);
    expect(html).toContain('现在没有事情等你'); expect(html.match(/card-dash/g)).toHaveLength(2); expect(data.overview.today).toMatchObject({ merged: 0, gates: 0, cost: 0 });
  } finally { await fixture.close(); }
}, 60_000);
test('running 真实运行会话与排队结论来自内核', async () => {
  const { core, home } = await activeFixture();
  await core.call('scheduler.tick', {});
  const result = await render(core, home);
  expect(result.data.tasks.find(t => t.id === 'T-3')?.state.state).toBe('active');
  expect(result.html).toContain('1 个任务在跑'); expect(result.html).toContain('本轮已跑'); expect(result.html).toContain('card-teal');
  const projectId = result.data.projects[0].id;
  await core.call('config.set_loop', { project_id: projectId, parallel: 1 });
  await core.call('task.create', { project_id: projectId, kind: 'feature', title: 'queued probe', brief: '# queued probe', draft: false });
  const queued = await render(core, home); expect(queued.html).toContain('1 个在排队'); expect(queued.html).toContain('等空闲槽位');
}, 120_000);
test('failed 真实失败原因与等待起点进入黄色卡', async () => {
  const { core, home } = await activeFixture();
  const { data, html } = await render(core, home);
  expect(data.tasks.find(t => t.id === 'T-1')?.state.state).toBe('failed'); expect(data.overview.waiting['T-1']).toBeTypeOf('number');
  expect(html).toContain('处理失败'); expect(html).toContain('card-yellow'); expect(html).toContain('有 1 件事等你决定');
}, 120_000);
test('awaiting 真实需求与 Bug 等待合并，事件门禁和卡片一致', async () => {
  const root = mkdtempSync(join(tmpdir(), 'm03-awaiting-')), home = join(root, 'autome-home');
  const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
  cleanups.push(() => teardown({ root, core, taskIds: ['T-1', 'T-2'] }));
  await promisify(execFile)('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), root, wt], { timeout: 150_000, maxBuffer: 1024 * 1024 });
  await core.start();
  const { data, html } = await render(core, home);
  expect(data.tasks.every(t => t.state.node === 'await_merge')).toBe(true); expect(html).toContain('有 2 件事等你决定'); expect(html).toContain('批准合并'); expect(html).toContain('?tab=changes');
  expect(data.overview.today!.gates).toBeGreaterThan(0); expect(data.overview.today!.openStarts).toHaveLength(2);
}, 180_000);
