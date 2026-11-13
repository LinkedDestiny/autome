import { readFileSync, mkdtempSync, mkdirSync, realpathSync, rmSync, symlinkSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';
import { createAdmin } from '../server/admin.ts';
import { createGateway } from '../server/gateway.ts';
const replies = JSON.parse(readFileSync(new URL('./fixtures/w9-replies.json', import.meta.url), 'utf8'));
let root: string, home: string;
beforeEach(() => { root = mkdtempSync(join(tmpdir(), 'w9-confirm-')); home = join(root, 'home'); mkdirSync(home); mkdirSync(join(home, 'code')); symlinkSync(root, join(home, 'escape')); });
afterEach(() => rmSync(root, { recursive: true, force: true }));
function setup() {
  const call = vi.fn(async (method: string, _params: Record<string, unknown>): Promise<unknown> => {
    void _params;
    if (method === 'project.probe') return structuredClone(replies.probe);
    if (method === 'project.add') return structuredClone(replies.add);
    return structuredClone(replies.skip);
  });
  const core = { call, restart: vi.fn() }, admin = createAdmin({ core, userHome: home, automeHome: root });
  const gateway = createGateway({ core, home: root, paths: admin.paths }); admin.register(gateway);
  const confirm = (params: Record<string, unknown> = { path: '~/code' }) => gateway.write({ op: 'project.confirm', params });
  return { core, call, gateway, confirm };
}
test('probe → add 同一 realpath，单仓库不传 docs_repo', async () => {
  const s = setup(); expect((await s.confirm({ path: '~/code', docs_repo: 'unused' })).status).toBe(200);
  expect(s.call.mock.calls).toEqual([['project.probe', { path: realpathSync(join(home, 'code')) }], ['project.add', { path: realpathSync(join(home, 'code')), workspace: false }]]);
});
test('外部路径 400 而不是内核离线，零调用', async () => {
  const s = setup(); const r = await s.confirm({ path: root }); expect(r.status).toBe(400); expect(r.body.error).toMatch(/^已拒绝：/); expect(r.body.error).not.toContain('内核未连接'); expect(s.call).not.toHaveBeenCalled();
});
test('符号链接逃逸零调用', async () => {
  const s = setup(); expect((await s.confirm({ path: '~/escape' })).status).toBe(400); expect(s.call).not.toHaveBeenCalled();
});
test('禁止整个 HOME', async () => {
  const s = setup(); expect((await s.confirm({ path: '~' })).body.error).toContain('整个家目录'); expect(s.call).not.toHaveBeenCalled();
});
test('空工作区拒绝，不调 add', async () => {
  const s = setup(); expect((await s.confirm({ path: '~/code', workspace: true })).status).toBe(400); expect(s.call.mock.calls.map(c => c[0])).toEqual(['project.probe']);
});
test('skip 成功与失败都保留已经注册的项目', async () => {
  const s = setup(); const success = await s.confirm({ path: '~/code', skip_onboarding: true });
  expect(s.call.mock.calls.map(c => c[0])).toEqual(['project.probe', 'project.add', 'project.onboarding.skip']);
  expect(success.body.payload).toMatchObject({ onboarding: replies.skip.onboarding, init_commit: replies.skip.init_commit });
  s.call.mockImplementation(async method => { if (method === 'project.probe') return replies.probe; if (method === 'project.add') return replies.add; throw new Error('原始 skip 错误'); });
  expect(await s.confirm({ path: '~/code', skip_onboarding: true })).toMatchObject({ status: 200, body: { payload: { project: replies.add.project, skip_error: '原始 skip 错误' } } });
});
test('内核拒绝原文、超时、不可达映射且停止后续调用', async () => {
  const s = setup();
  for (const [error, status, message] of [[Object.assign(new Error('原始拒绝'), { code: 'invalid_params' }), 409, '原始拒绝'], [Object.assign(new Error('超时'), { timeout: true }), 504, '超时'], [new Error('断线'), 503, '内核未连接：断线']] as const) {
    s.call.mockClear(); s.call.mockRejectedValueOnce(error);
    expect(await s.confirm()).toMatchObject({ status, body: { error: message } }); expect(s.call).toHaveBeenCalledTimes(1);
  }
});
test('probe 读入口守卫与未配置时的兼容行为', async () => {
  const s = setup(); expect((await s.gateway.read({ method: 'project.probe', params: { path: '~/escape' } })).status).toBe(400); expect(s.call).not.toHaveBeenCalled();
  await s.gateway.read({ method: 'project.probe', params: { path: '~/code' } }); expect(s.call).toHaveBeenLastCalledWith('project.probe', { path: realpathSync(join(home, 'code')) });
  await createGateway({ core: s.core, home: root }).read({ method: 'project.probe', params: { path: '~/code' } }); expect(s.call).toHaveBeenLastCalledWith('project.probe', { path: '~/code' });
});
