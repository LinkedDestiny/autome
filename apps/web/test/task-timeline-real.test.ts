import { mkdtempSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { createTaskEvents } from '../server/task-events.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { TaskTimeline } from '../app/components/task-timeline.tsx';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
const wt = resolve(import.meta.dirname, '../../..');
const root = mkdtempSync(join(tmpdir(), 'w7-timeline-'));
const home = join(root, 'autome-home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
const taskEvents = createTaskEvents({ call: (method, params) => core.call(method, params) });
const services = { gateway: createGateway({ core, home }), taskEvents };
const ids = () => existsSync(join(root, 'task-ids')) ? readFileSync(join(root, 'task-ids'), 'utf8').trim().split('\n') : [];
const cleanup = () => teardown({ root, core, taskIds: ids(), stopServices: () => taskEvents.close() });
beforeAll(async () => {
  try {
    await promisify(execFile)('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), root, wt], { timeout: 150_000, maxBuffer: 1024 * 1024 });
    await core.start({ timeoutMs: 30_000 });
  } catch (error) { try { await cleanup(); } catch (cleanupError) { throw new AggregateError([error, cleanupError], 'setup and cleanup failed', { cause: cleanupError }); } throw error; }
}, 180_000);
afterAll(cleanup);
async function render(id: string) {
  const result = await loadTaskDetail(services, id, new URL(`http://localhost/tasks/${id}`));
  expect(result.error).toBeUndefined(); expect(result.data?.task.state).toMatchObject({ node: 'await_merge' });
  return renderToString(createElement(TaskTimeline, { sessions: result.data!.sessions, events: result.history!.events, now: new Date().toISOString() }));
}
it('真实需求任务页面按会话顺序显示阶段与结果，包含快检、rebase 和合并前门禁', async () => {
  const html = await render('T-1');
  let last = -1;
  for (const label of ['任务整理', '设计', '评审', '裁决', '实现', '审计', '实现', '审计', '复盘']) {
    const index = html.indexOf(`<h3>${label}`, last + 1); expect(index).toBeGreaterThan(last); last = index;
  }
  for (const text of ['快检', 'rebase 无冲突', '合并前门禁', 'M-01 开放 → 待审', 'M-01 待审 → 已完成', 'M-02 待审 → 已完成', 'Codex', '用时', '无计费数据']) expect(html).toContain(text);
});
it('真实 Bug 页面显示定位、复现检查、修复、复核及正确业务结果', async () => {
  const html = await render('T-2');
  for (const text of ['定位', '复现检查', '修复', '复核', '符合预期', '复核通过']) expect(html).toContain(text);
});
it('正常出口无存活进程', async () => {
  await cleanup(); await vi.waitFor(() => expect(leftoverProcesses(root)).toEqual([]));
});
