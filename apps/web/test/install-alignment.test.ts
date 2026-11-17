import { afterAll, afterEach, beforeAll, expect, test } from 'vitest';
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve, basename } from 'node:path';
import { pathToFileURL } from 'node:url';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';

const root = resolve(import.meta.dirname, '../../..');
const scratch = mkdtempSync(join(tmpdir(), 'autome package '));
const panel = join(scratch, 'panel');
let archive: string;
const dirs: string[] = [];
const services = new Set<string>();
const cleaned: string[] = [];
const shellQuote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
beforeAll(() => {
  execFileSync(process.execPath, ['apps/web/scripts/stage-panel.ts', panel], { cwd: root });
  execFileSync('/bin/sh', ['scripts/package-cli.sh', '--binary', 'target/debug/autome', '--web', panel, '--out', join(scratch, 'dist')], { cwd: root, env: { ...process.env, PATH: '/usr/bin:/bin' } });
  archive = join(scratch, 'dist', readdirSync(join(scratch, 'dist')).find(f => f.endsWith('.tar.gz'))!);
}, 60000);
afterEach(async () => {
  const errors: unknown[] = [];
  for (const dir of dirs.splice(0)) {
    try {
      if (services.has(dir)) await teardown({ root: dir });
      else rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
      cleaned.push(dir);
    } catch (error) { errors.push(error); }
  }
  if (errors.length) throw new AggregateError(errors, '安装验收清理失败');
});
afterAll(() => rmSync(scratch, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }));
function fixture() {
  expect(process.platform, '安装验收需要 macOS，与 CI 平台一致').toBe('darwin');
  const dir = mkdtempSync(join(tmpdir(), 'autome install ')); dirs.push(dir);
  const prefix = join(dir, 'custom prefix');
  const home = join(dir, 'home');
  const opener = join(dir, 'opener');
  writeFileSync(opener, `#!/bin/sh\nprintf '%s' "$1" > ${shellQuote(join(dir, 'opened'))}\n`, { mode: 0o755 });
  const env: NodeJS.ProcessEnv = { ...process.env, PATH: `${prefix}:/usr/bin:/bin:/usr/sbin:/sbin`, AUTOME_NODE: process.execPath,
    AUTOME_HOME: home, AUTOMED_DB_PATH: join(home, 'state/ledger.sqlite'), AUTOME_TEST_BUILD_ID: '', AUTOME_ALLOW_OLDER_KERNEL: '',
    PREFIX: prefix, AUTOME_WEB_DEFAULT_PORT: '0', AUTOME_WEB_OPENER: opener };
  for (const key of ['NODE_ENV', 'NODE_OPTIONS', 'AUTOMED_BIN', 'AUTOME_BUNDLED_CORE', 'AUTOME_WEB_ROOT', 'AUTOME_WEB_PORT']) delete env[key];
  const run = (file: string, args: string[]) => execFileSync(file, args, { cwd: dir, env, encoding: 'utf8', timeout: 45000 });
  const install = () => run('/bin/sh', [join(root, 'scripts/install-cli.sh'), '--from', archive]);
  return { dir, prefix, home, env, run, install, cli: join(prefix, 'autome'), installedPanel: join(dir, 'lib/autome/web') };
}

test('发布包带运行面板而无开发依赖，校验和正确', () => {
  // Load the staged server with native Node, without Vite or the source checkout.
  const smoke = spawnSync(process.execPath, ['--input-type=module', '-e', `await import(${JSON.stringify(pathToFileURL(join(panel, 'server/overview.ts')).href)})`], { cwd: scratch, encoding: 'utf8' });
  expect(smoke.status, smoke.stderr).toBe(0);
  const entries = execFileSync('/usr/bin/tar', ['-tzf', archive], { encoding: 'utf8' }).trim().split('\n');
  const name = basename(archive, '.tar.gz');
  for (const file of ['autome', 'README.md', 'web/build/server/index.js', 'web/server/start.js', 'web/bin/autome-web.js', 'web/app/load-context.ts', 'web/package.json', 'web/node_modules/react/package.json']) expect(entries).toContain(`${name}/${file}`);
  for (const dependency of ['vite', 'vitest', 'typescript']) expect(entries.some(e => e.includes(`/node_modules/${dependency}/`))).toBe(false);
  expect(readFileSync(`${archive}.sha256`, 'utf8').trim()).toBe(`${createHash('sha256').update(readFileSync(archive)).digest('hex')}  ${basename(archive)}`);
  const readme = execFileSync('/usr/bin/tar', ['-xOzf', archive, `${name}/README.md`], { encoding: 'utf8' });
  for (const text of ['autome web', 'autome core', 'autome --version']) expect(readme).toContain(text);
});

test('空 PREFIX 安装、打开生产面板、同包内核、重复安装与停止', async () => {
  leftoverProcesses('install-acceptance-preflight'); // Refuse to spawn if cleanup cannot inspect processes.
  const f = fixture();
  services.add(f.dir);
  expect(existsSync(f.prefix)).toBe(false); expect(existsSync(f.home)).toBe(false);
  expect(f.env.NODE_ENV).toBeUndefined();
  expect(f.install()).toContain('sha256 校验通过');
  expect(existsSync(join(f.installedPanel, 'bin/autome-web.js'))).toBe(true);
  expect(existsSync(join(f.installedPanel, 'node_modules/vite'))).toBe(false);
  writeFileSync(join(f.installedPanel, 'obsolete'), 'old');
  f.install(); expect(existsSync(join(f.installedPanel, 'obsolete'))).toBe(false);
  expect(readdirSync(dirname(f.installedPanel))).toEqual(['web']);
  f.run(f.cli, ['web']);
  const url = readFileSync(join(f.dir, 'opened'), 'utf8');
  expect((await fetch(url)).status).toBe(200);
  const status = JSON.parse(f.run(f.cli, ['web', 'status', '--json']));
  const buildId = f.run(f.cli, ['--version']).match(/\(([^)]+)\)/)?.[1]; expect(buildId).toBeTruthy();
  expect(status.kernel).toMatchObject({ source: 'bundle', path: f.cli, build_id: buildId });
  expect(readFileSync(join(f.home, 'web/logs/server.log'), 'utf8')).toContain('生产模式');
  f.run(f.cli, ['web', 'stop']);
  await expect.poll(() => leftoverProcesses(f.dir), { timeout: 5000 }).toEqual([]);
  expect(existsSync(join(f.home, 'web/server.json'))).toBe(false);
}, 60000);

test('PATH 只有旧 Node 时只装 CLI，缺 Node 与版本边界也正确', () => {
  for (const version of ['21.7.3', '', '22.21.9', '22.22.0', '23.0.0']) {
    const f = fixture();
    const nodeDir = join(f.dir, 'node-bin'); mkdirSync(nodeDir);
    writeFileSync(join(nodeDir, 'node'), `#!/bin/sh\nexec ${shellQuote(process.execPath)} --import ${shellQuote(join(root, 'apps/web/test/fixtures/fake-node-21.7.3.mjs'))} "$@"\n`, { mode: 0o755 });
    delete f.env.AUTOME_NODE;
    f.env.PATH = `${nodeDir}:/usr/bin:/bin`;
    if (!version) f.env.PATH = '/usr/bin:/bin';
    else if (version !== '21.7.3') {
      f.env.AUTOME_NODE = join(f.dir, 'version-node');
      writeFileSync(f.env.AUTOME_NODE, `#!/bin/sh\nprintf '%s' ${shellQuote(version)}\n`, { mode: 0o755 });
    }
    const output = f.install();
    const supported = version === '22.22.0' || version === '23.0.0';
    expect(existsSync(f.installedPanel)).toBe(supported);
    if (!supported) {
      expect(output).toContain('只装了命令行'); expect(output).toContain('brew install node');
      const result = spawnSync(f.cli, ['web'], { env: f.env, encoding: 'utf8' });
      expect(result.status).toBe(1); expect(result.stderr).toContain(version ? '没有找到面板' : '没有找到 Node.js');
    }
  }
}, 60000);

test('--binary 无 cargo/node/npm 仍可打包，缺 --web 与空路径拒绝', () => {
  const env = { ...process.env, PATH: '/usr/bin:/bin' };
  for (const command of ['node', 'npm', 'cargo']) expect(spawnSync(command, ['--version'], { env }).error).toBeDefined();
  const out = join(scratch, 'offline');
  const run = (args: string[]) => spawnSync('/bin/sh', ['scripts/package-cli.sh', ...args], { cwd: root, env, encoding: 'utf8' });
  expect(run(['--binary', 'target/debug/autome', '--web', panel, '--out', out]).status).toBe(0);
  const missing = run(['--binary', 'target/debug/autome', '--out', out]);
  expect(missing.status).toBe(2); expect(missing.stderr).toContain('--binary 需要配合 --web');
  expect(run(['--web', '']).status).toBe(2);
}, 60000);

test('安装成功消息与 PATH 提示完整且 stderr 为空（审计回归）', () => {
  const f = fixture();
  const checkTmp = mkdtempSync(join(tmpdir(), 'autome-install-msg-')); dirs.push(checkTmp);
  delete f.env.AUTOME_TEST_BUILD_ID;
  const doctorBin = join(f.dir, 'doctor-bin'); mkdirSync(doctorBin);
  for (const name of ['git', 'claude', 'codex']) writeFileSync(join(doctorBin, name), '#!/bin/sh\necho "logged in test 1.0.0"\n', { mode: 0o755 });
  f.env.PATH = `${doctorBin}:/usr/bin:/bin`;
  const result = spawnSync('/bin/sh', ['apps/web/test/fixtures/install-msg-check.sh', archive], {
    cwd: root, env: { ...f.env, TMPDIR: checkTmp }, encoding: 'utf8', timeout: 45000,
  });
  const installOutput = readdirSync(checkTmp).map(dir => readFileSync(join(checkTmp, dir, 'out'), 'utf8')).join('\n');
  expect(result.stdout, result.stderr + installOutput).toBe('OK\n');
  expect(result.status).toBe(0);
  expect(result.stderr).toBe('');
});

test('安装验收清理全部临时服务与内核，无残留', () => {
  expect(cleaned.length).toBeGreaterThan(0);
  for (const dir of cleaned) { expect(leftoverProcesses(dir)).toEqual([]); expect(existsSync(dir)).toBe(false); }
});
