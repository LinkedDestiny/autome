import { expect, test, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { createAdmin } from '../server/admin.ts';
import { createGateway } from '../server/gateway.ts';
import { createMacTerminal, TerminalUnavailable } from '../server/terminal.ts';
import { recipe } from './fixtures/w9-env.ts';
function setup(value = recipe, terminal = vi.fn<(command: string) => Promise<void>>(async () => {})) {
  const core = { call: vi.fn(async () => value), restart: vi.fn() }, publish = vi.fn();
  const admin = createAdmin({ core, userHome: '/tmp', automeHome: '/tmp', terminal, publish });
  const gateway = createGateway({ core, home: '/tmp' }); admin.register(gateway);
  return { core, admin, gateway, terminal, publish };
}
test('1 install 当场取 recipe，执行原文', async () => {
  const s = setup(); expect(await s.gateway.write({ op: 'env.install', params: { component: 'claude' } })).toMatchObject({ status: 200, body: { payload: { launched: true, command: recipe.recipe.command } } });
  expect(s.core.call.mock.calls).toEqual([['env.install_recipe', { component: 'claude' }]]); expect(s.terminal.mock.calls).toEqual([[recipe.recipe.command]]);
});
test('2 command 和任意额外键拒绝，零副作用；组件域边界', async () => {
  const s = setup();
  for (const op of ['env.install', 'env.login']) {
    for (const extra of [{ command: 'rm -rf ~' }, { extra: null }, { force: false }]) expect((await s.gateway.write({ op, params: { component: 'claude', ...extra } })).status).toBe(400);
    for (const component of ['', 'Git', 'unknown', null, 1, 'git ', 'x'.repeat(513)]) expect((await s.gateway.write({ op, params: { component } })).status).toBe(400);
  }
  expect(s.core.call).not.toHaveBeenCalled(); expect(s.terminal).not.toHaveBeenCalled();
});
test('3 登录用 login_command，无登录命令拒绝', async () => {
  const s = setup(); await s.gateway.write({ op: 'env.login', params: { component: 'claude' } }); expect(s.terminal).toHaveBeenCalledWith('claude /login');
  const git = setup({ ...recipe, login_command: null }); expect((await git.gateway.write({ op: 'env.login', params: { component: 'git' } })).status).toBe(400); expect(git.terminal).not.toHaveBeenCalled();
});
test('4 前置不满足拒绝，内核失败不执行终端', async () => {
  const s = setup({ ...recipe, prerequisite_ok: false }); expect(await s.gateway.write({ op: 'env.install', params: { component: 'claude' } })).toMatchObject({status:409,body:{error:'已拒绝：请先安装 npm'}});
  for (const failure of [Object.assign(new Error('超时'), { timeout: true }), Object.assign(new Error('拒绝授权'), { code: 'denied' }), new Error('服务不可达')]) {
    s.core.call.mockRejectedValueOnce(failure); expect((await s.gateway.write({ op: 'env.install', params: { component: 'git' } })).body.ok).toBe(false);
  }
  expect(s.terminal).not.toHaveBeenCalled();
});
test('5 终端不可用正常返回命令，startServer 默认安全启动器', async () => {
  const s = setup(recipe, vi.fn<(command: string) => Promise<void>>(async () => { throw new TerminalUnavailable('不可用'); }));
  expect(await s.gateway.write({op:'env.install',params:{component:'claude'}})).toMatchObject({status:200,body:{payload:{launched:false,command:recipe.recipe.command,reason:'不可用'}}});
  const source = readFileSync('server/main.ts','utf8'); expect(source).toContain('terminal = unavailableTerminal'); expect(source).toContain('userHome, terminal, publish:'); expect(source).toContain('terminal: createMacTerminal()');
  const core = {call:vi.fn(async()=>recipe),restart:vi.fn()}, gateway=createGateway({core,home:'/tmp'}); createAdmin({core,userHome:'/tmp',automeHome:'/tmp'}).register(gateway);
  expect((await gateway.write({op:'env.login',params:{component:'claude'}})).body.payload).toMatchObject({launched:false,command:'claude /login'});
});
test('6 macOS 转义与非 macOS/启动失败降级', async () => {
  const execFile=vi.fn(async()=>{}); await createMacTerminal({platform:'darwin',execFile})('echo "a\\b"');
  expect(execFile.mock.calls).toEqual([['osascript',['-e','tell application "Terminal"','-e','activate','-e','do script "echo \\"a\\\\b\\""','-e','end tell']]]);
  execFile.mockClear(); await expect(createMacTerminal({platform:'linux',execFile})('x')).rejects.toBeInstanceOf(TerminalUnavailable); expect(execFile).not.toHaveBeenCalled();
  execFile.mockRejectedValueOnce(new Error('denied')); await expect(createMacTerminal({platform:'darwin',execFile})('x')).rejects.toBeInstanceOf(TerminalUnavailable);
});
test('7 首份报告、generation 和 probing 的全部变化及重复报告', () => {
  const s=setup(); s.admin.onTick(); expect(s.publish).not.toHaveBeenCalled();
  const reports=[{env_generation:1,env_probing:false},{env_generation:1,env_probing:true},{env_generation:2,env_probing:false},{env_generation:3,env_probing:false}];
  for(const report of reports){ s.admin.onTick(report); s.admin.onTick(report); }
  expect(s.publish.mock.calls).toEqual(reports.map(r=>[{type:'env.changed',generation:r.env_generation,probing:r.env_probing}]));
});
