import { execFileSync, spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, expect, test } from 'vitest';
const roots: string[] = [];
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }); });
function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'stage-panel-')); roots.push(root);
  mkdirSync(join(root, 'scripts'));
  cpSync(resolve(import.meta.dirname, '../scripts/stage-panel.ts'), join(root, 'scripts/stage-panel.ts'));
  const run = (args: string[]) => spawnSync(process.execPath, [join(root, 'scripts/stage-panel.ts'), ...args], { encoding: 'utf8' });
  return { root, run };
}
test('暂存缺输出与缺构建给明确错误', () => {
  const f = fixture();
  expect(f.run([]).stderr).toContain('需要输出目录');
  const missing = f.run([join(f.root, 'out')]);
  expect(missing.status).not.toBe(0); expect(missing.stderr).toContain('先 npm run build');
});
test('暂存拒绝现有目录，避免开发依赖残留', () => {
  const f = fixture();
  mkdirSync(join(f.root, 'build/server'), { recursive: true }); writeFileSync(join(f.root, 'build/server/index.js'), '');
  const result = f.run([tmpdir()]);
  expect(result.status).not.toBe(0); expect(result.stderr).toContain('输出目录必须不存在');
});
test('暂存拒绝覆盖源码与源码子目录', () => {
  const f = fixture();
  mkdirSync(join(f.root, 'build/server'), { recursive: true }); writeFileSync(join(f.root, 'build/server/index.js'), '');
  for (const out of [f.root, join(f.root, 'child')]) {
    const result = f.run([out]); expect(result.status).not.toBe(0); expect(result.stderr).toContain('apps/web 之外');
  }
  expect(() => execFileSync(process.execPath, [join(f.root, 'scripts/stage-panel.ts'), f.root], { stdio: 'pipe' })).toThrow();
});
