import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { expect, test } from 'vitest';
import { leftoverProcesses } from './fixtures/process-scan.ts';
const web = resolve(import.meta.dirname, '..');
const env = { ...process.env, NO_COLOR: '1', FORCE_COLOR: '0' };
test('真实 list 的清单数量等于 Playwright 总数并包含准备步骤', () => {
  const result = spawnSync(process.execPath, ['scripts/e2e-catalog.mjs'], { cwd: web, env, encoding: 'utf8', timeout: 40_000 });
  expect(result.status, result.stderr).toBe(0);
  const listed = spawnSync(process.execPath, ['node_modules/@playwright/test/cli.js', 'test', '--list', '--reporter=list'], { cwd: web, env, encoding: 'utf8', timeout: 40_000 });
  expect(listed.status, listed.stderr).toBe(0);
  const count = Number(listed.stdout.match(/Total: (\d+) tests/)?.[1]);
  expect(count).toBeGreaterThan(0);
  expect(result.stdout.split('\n').filter(line => line.startsWith('case\t'))).toHaveLength(count);
  for (const term of ['globalSetup', 'globalTeardown', 'setup', 'teardown', 'prepare()', 'fixture.ts', 'w7-flow.sh', 'process-scan.ts']) expect(result.stdout).toContain(`setup\t${term}\n`);
  expect(result.stdout).toContain('autome-gate-cases end');
  expect(leftoverProcesses(join(web, 'node_modules/@playwright/test/cli.js'))).toEqual([]);
}, 90_000);
test('缺 node_modules 明示未取得清单并退出零', () => {
  const root = mkdtempSync(join(tmpdir(), 'e2e-catalog-'));
  try {
    mkdirSync(join(root, 'scripts'));
    copyFileSync(join(web, 'scripts/e2e-catalog.mjs'), join(root, 'scripts/e2e-catalog.mjs'));
    const result = spawnSync(process.execPath, [join(root, 'scripts/e2e-catalog.mjs')], { cwd: root, env, encoding: 'utf8', timeout: 40_000 });
    expect(result.status).toBe(0);
    expect(result.stdout).toContain('未取得浏览器用例清单');
    expect(result.stdout).not.toContain('autome-gate-cases begin');
    expect(leftoverProcesses(root)).toEqual([]);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
