import { afterEach, expect, test } from 'vitest';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { resolveAutomedBinary } from '../server/core/binary.ts';
import { CoreSupervisor } from '../server/core/supervisor.ts';

const root = path.resolve(import.meta.dirname, '../../..');
const dirs: string[] = [];
const cores: CoreSupervisor[] = [];
afterEach(async () => {
  for (const core of cores.splice(0)) await core.stop();
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
});
function fixture() {
  // Installation acceptance requires macOS, just like CI; never silently skip it.
  expect(process.platform, '安装验收需要 macOS，与 CI 平台一致').toBe('darwin');
  const dir = mkdtempSync(path.join(os.tmpdir(), 'autome install '));
  dirs.push(dir);
  const prefix = path.join(dir, 'prefix');
  const home = path.join(dir, 'home');
  const env = { ...process.env, PATH: `${prefix}:/usr/bin:/bin:/usr/sbin:/sbin`, AUTOME_HOME: home,
    AUTOMED_DB_PATH: path.join(home, 'state/ledger.sqlite'), AUTOME_TEST_BUILD_ID: '', AUTOME_ALLOW_OLDER_KERNEL: '', PREFIX: prefix };
  const run = (file: string, args: string[]) => execFileSync(file, args, { cwd: root, env, encoding: 'utf8', timeout: 60000 });
  // PATH intentionally excludes cargo: --binary must never trigger a build.
  run('/bin/sh', ['scripts/package-cli.sh', '--binary', 'target/debug/autome', '--out', path.join(dir, 'dist')]);
  const files = readdirSync(path.join(dir, 'dist'));
  const archives = files.filter(f => f.endsWith('.tar.gz'));
  expect(archives).toHaveLength(1);
  const archive = path.join(dir, 'dist', archives[0]);
  return { dir, prefix, home, env, run, archive };
}

test('发布包包含同一 CLI 内核及说明，生成正确校验和', () => {
  const { run, archive } = fixture();
  const entries = run('/usr/bin/tar', ['-tzf', archive]).trim().split('\n');
  const name = path.basename(archive, '.tar.gz');
  expect(entries).toContain(`${name}/autome`);
  expect(entries).toContain(`${name}/README.md`);
  const readme = run('/usr/bin/tar', ['-xOzf', archive, `${name}/README.md`]);
  expect(readme).toContain('autome core');
  expect(readme).toContain('autome --version');
  expect(readFileSync(`${archive}.sha256`, 'utf8').trim()).toBe(
    `${createHash('sha256').update(readFileSync(archive)).digest('hex')}  ${path.basename(archive)}`);
}, 60000);

test('空 PREFIX 安装后按 PATH 找到 core，真实握手标识与 CLI 相同', async () => {
  const { dir, prefix, home, env, run, archive } = fixture();
  expect(existsSync(prefix)).toBe(false);
  expect(run('/bin/sh', ['scripts/install-cli.sh', '--from', archive])).toContain('sha256 校验通过');
  const resolved = resolveAutomedBinary({ env: { PATH: env.PATH }, home: path.join(dir, 'user'), webRoot: path.join(dir, 'web') });
  expect(resolved).toEqual({ path: path.join(prefix, 'autome'), args: ['core'], source: 'cli' });
  const version = run(resolved.path, ['--version']);
  expect(version).toBe(run(path.join(root, 'target/debug/autome'), ['--version']));
  const buildId = version.match(/\(([^)]+)\)/)?.[1];
  expect(buildId).toBeTruthy();
  const core = new CoreSupervisor({ binaryPath: resolved.path, binaryArgs: resolved.args, home, env, dbPath: env.AUTOMED_DB_PATH });
  cores.push(core);
  await core.start();
  expect(core.status.state).toBe('connected');
  expect(await core.call('env.get')).toMatchObject({ kernel: { build_id: buildId, may_advance: true } });
}, 60000);
