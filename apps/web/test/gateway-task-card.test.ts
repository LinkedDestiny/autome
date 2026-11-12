import { mkdtempSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { startServer } from '../server/main.ts';
import type { CoreSupervisor } from '../server/core/supervisor.ts';

const binaryPath = process.env.AUTOMED_BIN || resolve(import.meta.dirname, '../../../target/debug/automed');
const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => { for (const cleanup of cleanups.splice(0).reverse()) await cleanup(); });

async function setup() {
  const root = mkdtempSync(join(tmpdir(), 'gateway-task-card-'));
  cleanups.push(async () => { rmSync(root, { recursive: true, force: true }); });
  const repo = join(root, 'repo');
  mkdirSync(repo);
  const running = await startServer({ port: 0, mode: 'production', home: join(root, 'home'), binaryPath });
  cleanups.push(() => running.close());
  const address = running.server.address();
  if (!address || typeof address === 'string') throw new Error('无端口');
  const url = `http://127.0.0.1:${address.port}`;
  const post = (channel: 'read' | 'write', body: unknown) => fetch(`${url}/api/${channel}`, {
    method: 'POST', headers: { 'Content-Type': 'application/json', Origin: url }, body: JSON.stringify(body),
  });
  // Only fixture setup bypasses the browser gate: project.add must remain unreachable there.
  const core = running.gateway.core as CoreSupervisor;
  const { project } = await core.call('project.add', { path: repo }) as { project: { id: string } };
  const createDraft = (brief: string) => post('write', {
    op: 'task.create', params: { project_id: project.id, kind: 'bug', brief, draft: true },
  });
  return { post, createDraft };
}

test('60KB Bug 正文开头经 HTTP 到达真实内核并创建草稿，超限中文拒绝', async () => {
  const s = await setup();
  const brief = '# 网关六十KB缺陷\n' + '中a'.repeat(15360);
  expect(Buffer.byteLength(brief)).toBeGreaterThanOrEqual(60 * 1024);
  expect(Buffer.byteLength(brief)).toBeLessThanOrEqual(65536);
  const response = await s.createDraft(brief);
  expect(response.status).toBe(200);
  const { payload } = await response.json();
  expect(payload.task).toMatchObject({ kind: 'bug', request: '网关六十KB缺陷', state: { state: 'draft' } });
  const read = await s.post('read', { method: 'task.get', params: { task_id: payload.task.id } });
  expect(read.status).toBe(200);
  expect((await read.json()).payload.task).toMatchObject({ id: payload.task.id, kind: 'bug', state: { state: 'draft' } });
  const refused = await s.createDraft('a'.repeat(65537));
  expect(refused.status).toBe(400);
  expect((await refused.json()).error).toBe('已拒绝：任务卡正文超过 64KB（65537 字节，上限 65536 字节）');
});

test('开始与补充越过 HTTP 门禁进入内核分支，缺参及类型错误在门禁拒绝', async () => {
  const s = await setup();
  const created = await s.createDraft('# 分支验证\n正文');
  expect(created.status).toBe(200);
  const task = (await created.json()).payload.task;
  expect(task.state.state).toBe('draft');
  // Never start a real draft here: that would schedule a real Agent session.
  for (const [op, params, code] of [
    ['task.start', { task_id: '不存在的任务' }, 'transition_rejected'],
    ['task.reply', { task_id: task.id, info: '补充' }, 'transition_rejected'],
  ] as const) {
    const response = await s.post('write', { op, params });
    expect(response.status).toBe(409);
    const body = await response.json();
    expect(body.code).toBe(code);
    expect(body.error).toMatch(/[\u4e00-\u9fff]/);
  }
  for (const [op, params] of [
    ['task.start', {}], ['task.start', { task_id: 7 }],
    ['task.reply', { task_id: task.id }], ['task.reply', { task_id: task.id, info: 7 }],
  ] as const) {
    const response = await s.post('write', { op, params });
    expect(response.status).toBe(400);
    expect((await response.json()).error).toMatch(/^已拒绝：.*[\u4e00-\u9fff]/);
  }
  const read = await s.post('read', { method: 'task.get', params: { task_id: task.id } });
  expect(read.status).toBe(200);
  expect((await read.json()).payload.task.state.state).toBe('draft');
});

test('project.probe 经 HTTP 由真实内核展开家目录，越界路径被拒', async () => {
  const previousHome = process.env.HOME;
  const home = mkdtempSync(join(tmpdir(), 'gateway-probe-home-'));
  cleanups.push(async () => {
    if (previousHome === undefined) delete process.env.HOME;
    else process.env.HOME = previousHome;
    rmSync(home, { recursive: true, force: true });
  });
  mkdirSync(join(home, 'repo'));
  process.env.HOME = home;
  const s = await setup();
  const response = await s.post('read', { method: 'project.probe', params: { path: '~/repo' } });
  expect(response.status).toBe(200);
  expect((await response.json()).payload).toMatchObject({ path: join(home, 'repo'), exists: true });
  for (const path of ['/etc', '~/../etc']) {
    const refused = await s.post('read', { method: 'project.probe', params: { path } });
    expect(refused.status).toBe(400);
    expect((await refused.json()).error).toMatch(/[\u4e00-\u9fff]/);
  }
});
