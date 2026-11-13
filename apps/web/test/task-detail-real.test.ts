import { mkdtempSync, readFileSync, existsSync, symlinkSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { createTaskEvents } from '../server/task-events.ts';
import { createTaskDocs } from '../server/task-docs.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
const wt = resolve(import.meta.dirname, '../../..');
const root = mkdtempSync(join(tmpdir(), 'w7-detail-'));
const home = join(root, 'autome-home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
const taskEvents = createTaskEvents({ call: (method, params) => core.call(method, params) });
const services = { gateway: createGateway({ core, home }), taskEvents, taskDocs: createTaskDocs() };
const ids = () => existsSync(join(root, 'task-ids')) ? readFileSync(join(root, 'task-ids'), 'utf8').trim().split('\n') : [];
const cleanup = () => teardown({ root, core, taskIds: ids(), stopServices: () => taskEvents.close() });
beforeAll(async () => {
  try {
    await promisify(execFile)('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), root, wt], { timeout: 150_000, maxBuffer: 1024 * 1024 });
    await core.start({ timeoutMs: 30_000 });
  } catch (error) {
    try { await cleanup(); } catch (cleanupError) { throw new AggregateError([error, cleanupError], `setup: ${String(error)}; cleanup: ${String(cleanupError)}`); }
    throw error;
  }
}, 180_000);
afterAll(cleanup);
const load = (id: string, doc?: string) => loadTaskDetail(services, id, new URL(`http://localhost/tasks/${id}${doc ? `?doc=${encodeURIComponent(doc)}` : ''}`));
it('需求真实事件只含本任务且升序，快检、rebase、合并前门禁与九次会话', async () => {
  const result = await load('T-1'); expect(result.error).toBeUndefined();
  expect(result.data?.task.state).toMatchObject({ state: 'active', node: 'await_merge' });
  expect(result.data?.sessions).toHaveLength(9);
  const events = result.history!.events;
  expect(events.length).toBeGreaterThan(0); expect(events.every(e => e.subject === 'T-1')).toBe(true);
  expect(events.map(e => e.seq)).toEqual([...new Set(events.map(e => e.seq))].sort((a, b) => a - b));
  expect(events.some(e => e.kind === 'gate.result' && e.payload.kind === 'quick')).toBe(true);
  expect(events.some(e => e.kind === 'rebase.conflicts')).toBe(true);
  expect(events.some(e => e.kind === 'gate.result' && e.payload.node === 'merge_gate')).toBe(true);
  expect(result.changes?.payload).toMatchObject({ rebase_first: true });
});
it('Bug 真实事件含定位、复现、修复门禁、复核且不伪造文档清单', async () => {
  const result = await load('T-2'); expect(result.data?.task.state).toMatchObject({ node: 'await_merge' });
  const events = result.history!.events;
  for (const kind of ['bug.located', 'bug.reproduced', 'bug.reviewed']) expect(events.some(e => e.kind === kind)).toBe(true);
  expect(events.some(e => e.kind === 'gate.result' && e.payload.node === 'fix_gate')).toBe(true);
  expect(events.every(e => e.subject === 'T-2')).toBe(true);
  expect(result.data?.documents?.some(d => d.name.endsWith('-bug.md'))).toBe(false);
});
it('按本次清单名称读取真实设计文档', async () => {
  const result = await load('T-1', 'feat.md');
  expect(result.document?.text).toContain('current-milestone: M-02');
  expect(result.document?.text).toContain('| M-02 | 已完成 |');
  expect(result.config).toHaveProperty('payload');
});
it('正常出口与异常 teardown 均无残留；杀不掉保留目录，清理可重复', async () => {
  for (const id of ids()) { try { await core.call('task.stop', { task_id: id }, { timeoutMs: 1000 }); } catch { /* await_merge rejects */ } }
  taskEvents.close(); await core.stop();
  await vi.waitFor(() => expect(leftoverProcesses(root)).toEqual([]));
  const childRoot = mkdtempSync(join(tmpdir(), 'w7-cleanup-'));
  mkdirSync(join(childRoot, 'bin')); symlinkSync('/bin/sleep', join(childRoot, 'bin/w7-sleep'));
  const child = spawn(join(childRoot, 'bin/w7-sleep'), ['60'], { stdio: 'ignore' });
  await new Promise<void>((resolve, reject) => { child.once('spawn', resolve); child.once('error', reject); });
  const failingCore = { call: vi.fn().mockRejectedValue(Error('stop refused')), stop: vi.fn().mockRejectedValue(Error('stop failed')) };
  const originalKill = process.kill.bind(process);
  const kill = vi.spyOn(process, 'kill').mockImplementation((pid, signal) => pid === child.pid ? true : originalKill(pid, signal));
  try {
    await expect(teardown({ root: childRoot, core: failingCore, taskIds: ['T-x'] })).rejects.toThrow('保留');
    expect(existsSync(childRoot)).toBe(true); expect(leftoverProcesses(childRoot).some(p => p.pid === child.pid)).toBe(true);
  } finally { kill.mockRestore(); await teardown({ root: childRoot, core: failingCore, taskIds: ['T-x'] }); }
  expect(leftoverProcesses(childRoot)).toEqual([]); expect(existsSync(childRoot)).toBe(false);
  await teardown({ root: childRoot }); expect(failingCore.stop).toHaveBeenCalled();
});
