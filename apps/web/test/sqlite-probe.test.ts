import { afterEach, expect, test, vi } from 'vitest';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { locateElectron } from '../scripts/electron-locate.ts';
import { checkRuntime } from '../scripts/probe-sqlite.ts';

const dirs: string[] = [];
function root() {
  const dir = mkdtempSync(join(tmpdir(), 'electron-layout-'));
  dirs.push(dir);
  return dir;
}
function layout(dir: string, complete = true) {
  const pkg = join(dir, 'apps/desktop/node_modules/electron');
  mkdirSync(join(pkg, 'dist'), { recursive: true });
  writeFileSync(join(pkg, 'path.txt'), 'Electron');
  writeFileSync(join(pkg, 'index.js'), 'throw new Error("不得加载 electron 包")');
  if (complete) writeFileSync(join(pkg, 'dist/Electron'), '');
  return join(pkg, 'dist/Electron');
}
afterEach(() => { for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true }); vi.restoreAllMocks(); });

test('系统 Node 使用文件库 WAL，关闭后重开读回', () => {
  const child = spawnSync(process.execPath, [resolve('scripts/sqlite-check.mjs')], { encoding: 'utf8' });
  expect(child.status, child.stderr).toBe(0);
  expect(JSON.parse(child.stdout)).toMatchObject({ ok: true, journalMode: 'wal', runtime: 'node' });
});
test('AUTOME_ELECTRON 优先', () => {
  const a = root(); const b = root(); const override = layout(a);
  layout(b);
  expect(locateElectron(override, [b])).toBe(override);
});
test('缺 dist 二进制时不加载包，落到主检出', () => {
  const a = root(); const b = root(); layout(a, false); const binary = layout(b);
  expect(locateElectron(undefined, [a, b])).toBe(binary);
});
test('worktree 完整时优先 worktree', () => {
  const a = root(); const b = root(); const binary = layout(a); layout(b);
  expect(locateElectron(undefined, [a, b])).toBe(binary);
});
test('所有路径都没有时返回空，空覆盖值同未设置', () => {
  expect(locateElectron('', [root(), root()])).toBeUndefined();
});
test('实际运行时版本不等于 Desktop Electron 版本时失败', () => {
  const error = vi.spyOn(console, 'error').mockImplementation(() => {});
  expect(checkRuntime(process.execPath, '43.2.0')).toBe(false);
  expect(error).toHaveBeenCalledWith(expect.stringContaining('Electron 版本不符'));
});
test('运行时不存在时返回失败', () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  expect(checkRuntime(join(root(), 'missing-node'))).toBe(false);
});
test('运行时非零退出时返回失败', () => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
  expect(checkRuntime('/usr/bin/false')).toBe(false);
});
