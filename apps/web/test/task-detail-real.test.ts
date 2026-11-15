import { mkdtempSync, readFileSync, existsSync, symlinkSync, mkdirSync, writeFileSync, appendFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { readTaskEvents } from '../server/task-history.ts';
import { createTaskDocs } from '../server/task-docs.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
import { changesView, type TaskChanges } from '../app/components/task-changes-model.ts';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
const wt = resolve(import.meta.dirname, '../../..');
const root = mkdtempSync(join(tmpdir(), 'w7-detail-'));
const home = join(root, 'autome-home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
const taskEvents = { forTask: (id: string) => readTaskEvents((method, params) => core.call(method, params), id) };
const services = { gateway: createGateway({ core, home }), taskEvents, taskDocs: createTaskDocs() };
const ids = () => existsSync(join(root, 'task-ids')) ? readFileSync(join(root, 'task-ids'), 'utf8').trim().split('\n') : [];
const cleanup = () => { if (existsSync(root)) writeFileSync(join(root, 'release-main-check'), ''); return teardown({ root, core, taskIds: ids() }); };
beforeAll(async () => {
  try {
    await promisify(execFile)('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), root, wt], { timeout: 150_000, maxBuffer: 1024 * 1024, env: { ...process.env, AUTOME_BIN: join(wt, 'target/debug/autome'), AUTOMED_BIN: join(wt, 'target/debug/automed') } });
    await core.start({ timeoutMs: 30_000 });
  } catch (error) {
    try { await cleanup(); } catch (cleanupError) { throw new AggregateError([error, cleanupError], `setup: ${String(error)}; cleanup: ${String(cleanupError)}`, { cause: cleanupError }); }
    throw error;
  }
}, 180_000);
afterAll(cleanup);
const load = (id: string, doc?: string) => loadTaskDetail(services, id, new URL(`http://localhost/tasks/${id}${doc ? `?doc=${encodeURIComponent(doc)}` : ''}`));
it('需求真实事件只含本任务且升序，快检、rebase、合并前门禁与九次会话', async () => {
  const result = await load('T-1'); expect(result.error).toBeUndefined();
  expect(result.data?.task.state).toMatchObject({ state: 'active', node: 'await_merge' });
  expect(result.timeline?.entries.filter(e => e.session)).toHaveLength(9);
  const events = (await taskEvents.forTask('T-1')).events;
  expect(events.length).toBeGreaterThan(0); expect(events.every(e => e.subject === 'T-1')).toBe(true);
  expect(events.map(e => e.seq)).toEqual([...new Set(events.map(e => e.seq))].sort((a, b) => a - b));
  expect(events.some(e => e.kind === 'gate.result' && e.payload.kind === 'quick')).toBe(true);
  expect(events.some(e => e.kind === 'rebase.conflicts')).toBe(true);
  expect(events.some(e => e.kind === 'gate.result' && e.payload.node === 'merge_gate')).toBe(true);
  expect(result.changes?.payload).toMatchObject({ rebase_first: true });
});
it('Bug 真实事件含定位、复现、修复门禁、复核且可读取清单中的 Bug 文档', async () => {
  const result = await load('T-2'); expect(result.data?.task.state).toMatchObject({ node: 'await_merge' });
  const events = (await taskEvents.forTask('T-2')).events;
  for (const kind of ['bug.located', 'bug.reproduced', 'bug.reviewed']) expect(events.some(e => e.kind === kind)).toBe(true);
  expect(events.some(e => e.kind === 'gate.result' && e.payload.node === 'fix_gate')).toBe(true);
  expect(events.every(e => e.subject === 'T-2')).toBe(true);
  expect(result.docs!.tabs.map(t => t.label)).toContain('Bug 文档');
  const document = result.docs?.tabs.find(d => d.name?.endsWith('-bug.md'));
  expect(document).toBeDefined();
  const content = await load('T-2', document!.name!);
  expect(content.document?.text).toContain('复现命令: sh regression.sh');
});
it('按本次清单名称读取真实设计文档', async () => {
  const result = await load('T-1', 'feat.md');
  expect(result.document?.text).toContain('current-milestone: M-02');
  expect(result.document?.text).toContain('| M-02 | 已完成 |');
  expect(result.ops?.length).toBeGreaterThan(0);
});
it('合并并清理后文档标签及来源、设计正文、里程碑终态和改动仍可读', async () => {
  const before = await load('T-1');
  writeFileSync(join(root, 'release-main-check'), '');
  const cli = (args: string[]) => promisify(execFile)(join(wt, 'target/debug/autome'), args, { cwd: join(root, 'repo'), env: { ...process.env, AUTOME_HOME: home, AUTOMED_CLAUDE_BINARY: join(root, 'bin/fake-cli'), AUTOMED_CODEX_BINARY: join(root, 'bin/fake-cli') }, timeout: 60000 });
  for (const id of ['T-1', 'T-2']) { await cli(['merge', id]); await cli(['run', '--until-idle', '--interval', '0.2']); }
  const done = await load('T-1', 'feat.md');
  expect(done.data?.task.state?.state).toBe('done');
  expect(existsSync(join(root, 'repo/.worktree/feat'))).toBe(false);
  expect(done.docs!.tabs).toEqual(before.docs!.tabs);
  expect(done.docs!.source).toBe('来自默认分支（主检出）');
  expect(done.docs?.source).toBeTruthy();
  expect(done.document?.text).toContain('| M-02 | 已完成 |');
  expect(done.data?.status_block).toMatchObject({ milestones_done: 2, milestones_total: 2 });
  expect(changesView((await loadTaskDetail(services, 'T-1', new URL('http://localhost/tasks/T-1?tab=changes'))).changes?.payload as TaskChanges, done.data!)).toMatchObject({ available: true, merged: true });
}, 150000);
it('真实结束事件的正常退出、退出码 3 崩溃和停止均进入时间线', async () => {
  const normal = await load('T-1');
  expect(normal.timeline!.entries.some(e => e.session?.label === '设计' && e.details.includes('正常结束'))).toBe(true);
  for (const [slug, script, expected] of [['crash-end', 'exit 3', '崩溃（退出码 3）'], ['stop-end', `exec '${root}/bin/w7-sleep' 300`, '被停止']]) {
    const fake = join(root, 'repo/.autome/fake', slug); mkdirSync(fake, { recursive: true });
    writeFileSync(join(fake, 'next'), '1'); writeFileSync(join(fake, '1.sh'), script);
    const { task } = await core.call('task.create', { project_id: normal.data!.project!.id, brief: `# ${slug}`, draft: true }) as { task: { id: string } };
    appendFileSync(join(root, 'task-ids'), task.id + '\n');
    await core.call('task.start', { task_id: task.id });
    if (slug === 'stop-end') {
      await vi.waitFor(async () => { await core.call('scheduler.tick', {}); expect((await load(task.id)).timeline?.entries.some(e => e.session?.running)).toBe(true); }, { timeout: 20000, interval: 100 });
      await core.call('task.stop', { task_id: task.id });
    }
    await vi.waitFor(async () => {
      await core.call('scheduler.tick', {});
      const result = await load(task.id);
      expect((await taskEvents.forTask(task.id)).events.some(e => e.kind === 'session.ended')).toBe(true);
      expect(result.timeline!.entries.some(e => e.details.includes(expected))).toBe(true);
    }, { timeout: 20000, interval: 100 });
  }
}, 60000);
it('正常出口与异常 teardown 均无残留；杀不掉保留目录，清理可重复', async () => {
  for (const id of ids()) { try { await core.call('task.stop', { task_id: id }, { timeoutMs: 1000 }); } catch { /* await_merge rejects */ } } await core.stop();
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
