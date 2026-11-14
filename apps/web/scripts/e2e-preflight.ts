import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { chromium } from '@playwright/test';

// Playwright resolves both downloads internally; derive the matching shell from
// its browser registry rather than accepting a different installed revision.
export function preflight({ browser = chromium.executablePath(),
  build = resolve(import.meta.dirname, '../build/server/index.js'),
  core = resolve(import.meta.dirname, '../../../target/debug/autome'), exists = existsSync } = {}) {
  const shell = browser.replace(/chromium-(\d+)/, 'chromium_headless_shell-$1')
    .replace(/chrome-mac-(arm64|x64)\/Google Chrome for Testing\.app\/Contents\/MacOS\/Google Chrome for Testing$/, 'chrome-headless-shell-mac-$1/chrome-headless-shell')
    .replace(/chrome-linux(?:64)?\/chrome$/, 'chrome-headless-shell-linux64/chrome-headless-shell');
  if (!exists(browser) || shell === browser || !exists(shell)) return '缺浏览器：Playwright 1.64.1 需要 chromium-1248 与 headless shell。安装：npx --prefix apps/web playwright install chromium';
  if (!exists(build)) return '缺面板构建：npm run build --prefix apps/web';
  if (!exists(core)) return '缺内核：cargo build -p autome-cli';
  return null;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const error = preflight();
  if (error) { console.error(error); process.exitCode = 1; }
}
