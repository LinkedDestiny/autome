import { afterEach, expect, test } from 'vitest';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { describeKernel } from '../server/core/identity.ts';
const id = '0000001-20200101T000000Z-000000000000';
const dirs: string[] = [];
function script(body: string) {
  const dir = mkdtempSync(join(tmpdir(), 'identity-')); dirs.push(dir);
  const path = join(dir, 'autome'); writeFileSync(path, `#!/bin/sh\n${body}\n`, { mode: 0o755 }); return path;
}
afterEach(() => { for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true }); });
test('cli 日志含路径、core、标识', async () => {
  const path = script(`echo 'autome 0.1.0 (${id})'`);
  expect(await describeKernel({ path, args: ['core'], source: 'cli' })).toBe(`内核：${path} core（${id}，来源：cli）`);
});
test('cargo 明示源码构建', async () => {
  const path = script(`echo 'automed 0.1.0 (${id})'`);
  expect(await describeKernel({ path, args: [], source: 'cargo' })).toBe(`正在使用源码构建的内核：${path}（${id}）`);
});
test('文件不存在仍给日志', async () => {
  expect(await describeKernel({ path: '/missing-automed', args: [], source: 'env' })).toContain('标识未知：文件不存在');
});
test('version 非零仍给日志', async () => {
  expect(await describeKernel({ path: script('exit 7'), args: [], source: 'bundle' })).toContain('--version 退出码 7');
});
test('旧版本无标识与空输出', async () => {
  for (const body of ["echo 'automed 0.1.0'", 'exit 0']) expect(await describeKernel({ path: script(body), args: [], source: 'env' })).toContain('输出里没有构建标识');
});
test('真实内核标识', async () => {
  expect(await describeKernel({ path: resolve(import.meta.dirname, '../../../target/debug/automed'), args: [], source: 'cargo' })).toMatch(/（\d{7}-\d{8}T\d{6}Z-[0-9a-f]{12}）/);
});
test('version 超时仍给日志', async () => {
  expect(await describeKernel({ path: script('exec sleep 10'), args: [], source: 'env' }, { timeout: 20 })).toContain('--version 超时');
});
