import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import { TaskDocs } from '../app/components/task-docs.tsx';
import { mkdtempSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { readTaskEvents } from '../server/task-history.ts';
import { createTaskDocs } from '../server/task-docs.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
const wt = resolve(import.meta.dirname, '../../..');
const root = mkdtempSync(join(tmpdir(), 'w7-docs-real-'));
const home = join(root, 'autome-home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
const taskEvents = { forTask: (id: string) => readTaskEvents((method, params) => core.call(method, params), id) };
const services = { gateway: createGateway({ core, home }), taskEvents, taskDocs: createTaskDocs() };
const ids = () => existsSync(join(root, 'task-ids')) ? readFileSync(join(root, 'task-ids'), 'utf8').trim().split('\n') : [];
const cleanup = () => teardown({ root, core, taskIds: ids() });
beforeAll(async () => {
  try {
    await promisify(execFile)('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), root, wt], { timeout: 150_000, maxBuffer: 1024 * 1024 });
    await core.start({ timeoutMs: 30_000 });
  } catch (error) {
    try { await cleanup(); } catch (cleanupError) { throw new AggregateError([error, cleanupError], `setup: ${String(error)}; cleanup: ${String(cleanupError)}`, { cause: cleanupError }); }
    throw error;
  }
}, 180_000);
afterAll(cleanup);
const load = (id: string, doc?: string) => loadTaskDetail(services, id, new URL(`http://localhost/tasks/${id}${doc ? `?doc=${encodeURIComponent(doc)}` : ''}`));
it('真实任务卡安全渲染与设计文档当前里程碑高亮', async () => {
  const brief = await load('T-1');
  expect(brief.data!.brief).toContain('<b>raw</b>');
  const render = (result: Awaited<ReturnType<typeof load>>) => renderToStaticMarkup(createElement(MemoryRouter, {}, createElement(TaskDocs, { data: result.data!, document: result.document })));
  const html = render(brief);
  expect(html).toContain('&lt;b&gt;raw&lt;/b&gt;'); expect(html).not.toContain('<b>'); expect(html).not.toContain('<script'); expect(html).not.toContain('href="javascript');
  const doc = await load('T-1', 'feat.md');
  expect(render(doc)).toContain('aria-current="true"><td>M-02');
});
it('真实内核清单下路径形状名字不读盘', async () => {
  const spy = vi.spyOn(services.taskDocs, 'read');
  try {
    for (const name of ['/tmp/x', 'a\\b', '..', '~x', '\0']) {
      const result = await load('T-1', name);
      expect(result.document?.error).toBe('已拒绝：文档名不能是路径');
    }
    expect(spy).not.toHaveBeenCalled();
  } finally { spy.mockRestore(); }
});
it('收尾无存活进程', async () => {
  await cleanup(); expect(leftoverProcesses(root)).toEqual([]); expect(existsSync(root)).toBe(false);
});
