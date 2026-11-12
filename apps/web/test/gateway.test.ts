import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { resolveAutomeHome } from '../server/home.ts';
import { expect, test, vi } from 'vitest';
import { createGateway } from '../server/gateway.ts';

function setup(resolve: (id: string) => string | null = () => null) {
  const core = { call: vi.fn().mockResolvedValue({ value: 1 }), restart: vi.fn().mockResolvedValue({ restarted: true }) };
  return { core, gateway: createGateway({ core, home: '/tmp/gateway', uploads: { resolve } }) };
}
test('读转发', async () => {
  const { core, gateway } = setup();
  expect(await gateway.read({ method: 'config.get' })).toEqual({ status: 200, body: { ok: true, payload: { value: 1 } } });
  expect(core.call).toHaveBeenCalledWith('config.get', {});
});
test('读拒绝不调用内核', async () => {
  const { core, gateway } = setup();
  for (const request of [null, {}, { method: 'project.probe' }, { method: 'session.log', params: { session_id: '/tmp/log' } }]) {
    expect((await gateway.read(request)).status).toBe(400);
  }
  expect(core.call).not.toHaveBeenCalled();
});
test('写转发与拒绝', async () => {
  const { core, gateway } = setup();
  expect((await gateway.write({ op: 'config.set_loop', params: { parallel: 4 } })).status).toBe(200);
  expect(core.call).toHaveBeenCalledWith('config.set_loop', { parallel: 4 });
  core.call.mockClear();
  for (const request of [null, {}, { op: 'project.add', params: { path: '/tmp' } }]) expect((await gateway.write(request)).status).toBe(400);
  expect(core.call).not.toHaveBeenCalled();
});
test('core.restart 本地处理', async () => {
  const { core, gateway } = setup();
  expect((await gateway.write({ op: 'core.restart' })).body.payload).toEqual({ restarted: true });
  expect(core.restart).toHaveBeenCalledOnce();
  expect(core.call).not.toHaveBeenCalled();
});
test('六种本地 op 未注册 501，注册后调用处理器', async () => {
  const { core, gateway } = setup();
  for (const [op, params] of [
    ['project.pick', {}], ['project.confirm', {}], ['open.path', { kind: 'log', session_id: 's' }],
    ['open.terminal', {}], ['env.install', { component: 'git' }], ['env.login', { component: 'claude' }],
  ] as const) {
    expect((await gateway.write({ op, params })).status).toBe(501);
    const handler = vi.fn().mockResolvedValue('本地');
    gateway.registerLocalOp(op, handler);
    expect((await gateway.write({ op, params })).body.payload).toBe('本地');
    expect(handler).toHaveBeenCalledWith(params);
  }
  expect(core.call).not.toHaveBeenCalled();
});
test('附件编号解析、空数组及目录边界', async () => {
  const id = '12345678-1234-1234-1234-123456789abc';
  for (const path of [null, '/tmp/outside', '/tmp/gateway/web/uploads-evil/a', '/tmp/gateway/web/uploads', '/tmp/gateway/web/uploads/../a', 'relative/a']) {
    const { core, gateway } = setup(() => path);
    expect((await gateway.write({ op: 'task.create', params: { attachments: [id] } })).status).toBe(400);
    expect(core.call).not.toHaveBeenCalled();
  }
  const { core, gateway } = setup(() => '/tmp/gateway/web/uploads/a');
  expect((await gateway.write({ op: 'task.create', params: { attachments: [id], doc_refs: [] } })).status).toBe(200);
  expect(core.call).toHaveBeenLastCalledWith('task.create', { attachments: ['/tmp/gateway/web/uploads/a'] });
  expect((await gateway.write({ op: 'task.create' })).status).toBe(200);
  expect(core.call).toHaveBeenLastCalledWith('task.create', { attachments: [] });
});
test('离线回 503', async () => {
  const { core, gateway } = setup(); core.call.mockRejectedValue(new Error('内核未连接：已停止'));
  expect(await gateway.read({ method: 'config.get' })).toEqual({ status: 503, body: { ok: false, error: '内核未连接：已停止' } });
});
test('内核错误原话及 code 回 409', async () => {
  const { core, gateway } = setup(); core.call.mockRejectedValue(Object.assign(new Error('内核原话'), { code: 'conflict' }));
  expect(await gateway.write({ op: 'config.set_loop' })).toEqual({ status: 409, body: { ok: false, error: '内核原话', code: 'conflict' } });
});
test('本地等待超时回 504', async () => {
  const { core, gateway } = setup(); core.call.mockRejectedValue(Object.assign(new Error('内核响应超时：config.get，操作可能仍在执行'), { timeout: true }));
  expect(await gateway.read({ method: 'config.get' })).toEqual({ status: 504, body: { ok: false, error: '内核响应超时：config.get，操作可能仍在执行' } });
});

test('真实内核经网关写读、手动重启及停止', async () => {
  const home = mkdtempSync(join(tmpdir(), 'gateway-core-'));
  const core = new CoreSupervisor({ home: home, binaryPath: process.env.AUTOMED_BIN || resolve(import.meta.dirname, '../../../target/debug/automed') });
  const gateway = createGateway({ core, home });
  try {
    await core.start();
    expect((await gateway.write({ op: 'config.set_loop', params: { parallel: 4 } })).status).toBe(200);
    expect((await gateway.read({ method: 'config.get' })).body.payload).toMatchObject({ resolved: { loop_defaults: { parallel: 4 } } });
    expect((await gateway.write({ op: 'core.restart' })).body.payload).toEqual({ already_running: true });
    await core.stop();
    expect((await gateway.read({ method: 'config.get' })).status).toBe(503);
    expect((await gateway.write({ op: 'core.restart' })).body.payload).toEqual({ restarted: true });
    expect((await gateway.read({ method: 'config.get' })).body.payload).toMatchObject({ resolved: { loop_defaults: { parallel: 4 } } });
  } finally { await core.stop(); rmSync(home, { recursive: true, force: true }); }
});
test('home 环境解析与内核默认值一致，保留空串', () => {
  expect(resolveAutomeHome({ AUTOME_HOME: '/tmp/isolated', HOME: '/tmp/user' })).toBe('/tmp/isolated');
  expect(resolveAutomeHome({ AUTOME_HOME: '' })).toBe('');
  expect(resolveAutomeHome({ HOME: '/tmp/user' })).toBe('/tmp/user/.autome');
  expect(resolveAutomeHome({})).toBe('/tmp/.autome');
});

test('任务卡字段、开始与补充原样转发', async () => {
  const { core, gateway } = setup();
  const params = { kind: 'bug', brief: '# 缺陷\n  正文\r\n', draft: true };
  expect((await gateway.write({ op: 'task.create', params })).status).toBe(200);
  expect(core.call).toHaveBeenLastCalledWith('task.create', { ...params, attachments: [] });
  for (const [op, params] of [
    ['task.start', { task_id: 't1' }],
    ['task.reply', { task_id: 't1', info: '  补充\n原文  ' }],
    ['task.reply', { task_id: 't1', info: '' }],
  ] as const) {
    expect((await gateway.write({ op, params })).status).toBe(200);
    expect(core.call).toHaveBeenLastCalledWith(op, params);
  }
  core.call.mockClear();
  for (const [op, params] of [
    ['task.start', {}], ['task.reply', { task_id: 't1', info: 7 }],
    ['task.create', { brief: '中'.repeat(21846) }],
  ] as const) expect((await gateway.write({ op, params })).status).toBe(400);
  expect(core.call).not.toHaveBeenCalled();
});

test('project.probe 只转发家目录路径，拒绝时不调用内核', async () => {
  const { core, gateway } = setup();
  expect((await gateway.read({ method: 'project.probe', params: { path: '~/x' } })).status).toBe(200);
  expect(core.call).toHaveBeenCalledExactlyOnceWith('project.probe', { path: '~/x' });
  core.call.mockClear();
  expect((await gateway.read({ method: 'project.probe', params: { path: '/etc' } })).status).toBe(400);
  expect(core.call).not.toHaveBeenCalled();
});
