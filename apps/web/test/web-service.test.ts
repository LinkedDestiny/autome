import { afterEach, expect, test, vi } from 'vitest';
import { mkdtempSync, writeFileSync, readFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
import { agentsDirectory, service } from '../server/service.ts';
import { harness } from './fixtures/web-launcher.ts';
const roots: string[] = [];
afterEach(() => { vi.restoreAllMocks(); for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
function setup() {
  const root = mkdtempSync(join(tmpdir(), 'web-service-')); roots.push(root);
  const ctl = join(root, 'launchctl'); const calls = join(root, 'calls');
  writeFileSync(ctl, `#!/bin/sh\nprintf '%s\\n' "$*" >> '${calls}'\n`, { mode: 0o755 });
  const env = { HOME: root, AUTOME_HOME: join(root, 'home & data'), AUTOME_LAUNCH_AGENTS_DIR: join(root, 'agents'), AUTOME_LAUNCHCTL: ctl, AUTOME_BUNDLED_CORE: join(root, 'bin/autome'), AUTOME_WEB_ROOT: join(root, 'panel') };
  return { root, env, file: join(env.AUTOME_LAUNCH_AGENTS_DIR, 'com.autome.web.plist'), calls, run: (action: string) => service(action, { env, platform: 'darwin' }) };
}
test('plist 的路径、环境、KeepAlive 和日志可被 plutil 读取', async () => {
  const s = setup(); await s.run('install'); expect(execFileSync('/usr/bin/plutil', ['-lint', s.file], { encoding: 'utf8' })).toContain('OK');
  const value = JSON.parse(execFileSync('/usr/bin/plutil', ['-convert', 'json', '-o', '-', s.file], { encoding: 'utf8' }));
  expect(value).toMatchObject({ Label: 'com.autome.web', ProgramArguments: [s.env.AUTOME_BUNDLED_CORE, 'web', 'serve'], EnvironmentVariables: { AUTOME_NODE: process.execPath, AUTOME_WEB_ROOT: s.env.AUTOME_WEB_ROOT, AUTOME_HOME: s.env.AUTOME_HOME }, RunAtLoad: true, KeepAlive: { SuccessfulExit: false }, StandardOutPath: join(s.env.AUTOME_HOME, 'web/logs/server.log'), StandardErrorPath: join(s.env.AUTOME_HOME, 'web/logs/server.log') });
});
test('重复 install 保持文件一致，先 bootout 再 bootstrap', async () => { const s = setup(); await s.run('install'); const first = readFileSync(s.file, 'utf8'); await s.run('install'); expect(readFileSync(s.file, 'utf8')).toBe(first); expect(readFileSync(s.calls, 'utf8').trim().split('\n')).toEqual(Array(2).fill([`bootout gui/${process.getuid!()}/com.autome.web`, `bootstrap gui/${process.getuid!()} ${s.file}`]).flat()); });
test('重复 uninstall 第二次报告未安装', async () => { const s = setup(); await s.run('install'); expect(await s.run('uninstall')).toBe(0); const log = vi.spyOn(console, 'log'); expect(await s.run('uninstall')).toBe(0); expect(log).toHaveBeenCalledWith('没有安装自启动'); expect(existsSync(s.file)).toBe(false); });
test('拒绝其它 label', async () => { const s = setup(); await expect(service('install', { env: s.env, platform: 'darwin', label: 'com.autome.loop' })).rejects.toThrow('只允许'); expect(existsSync(s.file)).toBe(false); });
test('非 macOS 明确拒绝', async () => { await expect(service('install', { platform: 'linux' })).rejects.toThrow('只支持 macOS'); });
test('默认目录只解析、不写入', () => { expect(agentsDirectory({ HOME: '/user/example' })).toBe('/user/example/Library/LaunchAgents'); });
test('serve 复用活实例以 0 退出', async () => { const h = harness(); try { expect((await h.run(['start'])).code).toBe(0); const v = await h.registration(); const result = await h.run(['serve']); expect(result.code).toBe(0); expect(result.stdout).toContain('已有面板'); expect((await h.registration()).instance).toBe(v.instance); } finally { await h.cleanup(); } });
test('未加载可安装，拒绝授权与 bootstrap 失败不吞掉', async () => {
  const s = setup();
  writeFileSync(s.env.AUTOME_LAUNCHCTL, '#!/bin/sh\n[ "$1" = bootout ] && exit 3\nexit 0\n');
  expect(await s.run('install')).toBe(0);
  writeFileSync(s.env.AUTOME_LAUNCHCTL, '#!/bin/sh\nexit 1\n');
  await expect(s.run('install')).rejects.toThrow(); await expect(s.run('uninstall')).rejects.toThrow(); expect(existsSync(s.file)).toBe(true);
  writeFileSync(s.env.AUTOME_LAUNCHCTL, '#!/bin/sh\n[ "$1" = bootout ] && exit 3\nexit 1\n');
  await expect(s.run('install')).rejects.toThrow();
});
test('无效命令、空内核路径与磁盘写入失败明确拒绝', async () => {
  const s = setup(); await expect(s.run('')).rejects.toThrow('用法');
  await expect(service('install', { env: { ...s.env, AUTOME_BUNDLED_CORE: '' }, platform: 'darwin' })).rejects.toThrow('绝对路径');
  writeFileSync(s.env.AUTOME_LAUNCH_AGENTS_DIR, 'not a directory'); await expect(s.run('install')).rejects.toThrow(); expect(existsSync(s.calls)).toBe(false);
});
