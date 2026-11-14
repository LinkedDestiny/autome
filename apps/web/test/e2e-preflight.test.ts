import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { expect, test } from 'vitest';
import { preflight } from '../scripts/e2e-preflight.ts';
const browser = '/cache/chromium-1248/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing';
test('缺任一浏览器均失败；空下载目录退出 1 并给出安装命令', () => {
  const root = mkdtempSync(join(tmpdir(), 'e2e-preflight-'));
  try {
    const result = spawnSync(process.execPath, ['scripts/e2e-preflight.ts'], { cwd: resolve(import.meta.dirname, '..'), env: { ...process.env, PLAYWRIGHT_BROWSERS_PATH: root }, encoding: 'utf8' });
    expect(result.status).toBe(1);
    expect(result.stderr).toContain('缺浏览器');
    expect(result.stderr).toContain('npx --prefix apps/web playwright install chromium');
    for (const missing of ['chromium-1248', 'chromium_headless_shell-1248']) {
      expect(preflight({ browser, exists: path => !String(path).includes(missing) })).toContain('缺浏览器');
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test('缺 build 或内核时给出对应构建命令', () => {
  expect(preflight({ browser, build: '/missing', exists: path => path !== '/missing' })).toContain('npm run build');
  expect(preflight({ browser, core: '/missing', exists: path => path !== '/missing' })).toContain('cargo build');
});
test('所有前置齐全退出 0', () => {
  expect(preflight({ browser, exists: () => true })).toBeNull();
  const result = spawnSync(process.execPath, ['scripts/e2e-preflight.ts'], { cwd: resolve(import.meta.dirname, '..'), encoding: 'utf8' });
  expect(result.status, result.stderr).toBe(0);
});
