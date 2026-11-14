import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, test } from 'vitest';
const root = resolve(import.meta.dirname, '../../..');
test('README 说明面板入口与同包内核', () => {
  const text = readFileSync(resolve(root, 'README.md'), 'utf8');
  for (const fragment of ['autome web', '### The panel', 'AUTOME_BUNDLED_CORE']) expect(text).toContain(fragment);
});
test('计划 7.3 使用安装后的入口', () => {
  const text = readFileSync(resolve(root, 'docs/plans/2026-11-10-web-dashboard-via-loop.md'), 'utf8').split('### 7.3')[1].split('### 7.4')[0];
  expect(text).not.toContain('npm --prefix apps/web start'); expect(text).toContain('autome web');
});
