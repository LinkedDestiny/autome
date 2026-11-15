// Manual README asset generation; deliberately not a CI step.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium, expect, type BrowserContext } from '@playwright/test';
import { checkout, cli, flowSteps, newTask } from '../e2e/harness/fixture.ts';
import { clearProcesses, leftoverProcesses } from '../test/fixtures/process-scan.ts';
import { startServer } from '../server/main.ts';

async function cleanupProcesses(root: string) {
  const tracked = leftoverProcesses(root).map(row => row.pid);
  await clearProcesses(root, tracked);
  if (leftoverProcesses(root, tracked).length) throw Error(`截图进程残留，保留 ${root}`);
  console.log('截图进程残留检查：0');
}

const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== '--out' || !args[1].trim())) {
  throw Error('用法：node apps/web/scripts/screenshots.ts [--out <目录>]');
}
const out = resolve(args[1] ?? join(checkout, 'docs/images'));
for (const path of ['apps/web/build/server/index.js', 'target/debug/autome', 'target/debug/automed']) {
  if (!existsSync(join(checkout, path))) {
    throw Error(`缺少 ${path}；请先运行 cargo build -p automed -p autome-cli && npm run build --prefix apps/web`);
  }
}
const root = mkdtempSync(join(tmpdir(), 'autome-readme-'));
let service: Awaited<ReturnType<typeof startServer>> | undefined;
let browser: BrowserContext | undefined;
let canInspect = false;
const originalHome = process.env.HOME;
const originalAutomeHome = process.env.AUTOME_HOME;
try {
  // Refuse to start anything if we cannot prove cleanup afterwards.
  leftoverProcesses(root);
  canInspect = true;
  process.env.HOME = root;
  process.env.AUTOME_HOME = join(root, 'autome-home');
  execFileSync('bash', [join(checkout, 'apps/web/test/fixtures/w7-flow.sh'), root, checkout], {
    env: { ...process.env, AUTOMED_BIN: join(checkout, 'target/debug/automed'), AUTOME_BIN: join(checkout, 'target/debug/autome') },
    timeout: 60_000, stdio: ['ignore', 'pipe', 'pipe'],
  });
  const title = '结算页支持优惠码';
  flowSteps(root, title);
  const taskId = newTask(root, title);
  cli(root, ['run', '--until-idle', '--interval', '0.2']);
  for (const title of ['修复导出报表时区错误', '订单列表增加筛选', '补齐支付失败提示']) {
    newTask(root, title, ['--draft']);
  }
  const { projects } = JSON.parse(cli(root, ['rpc', 'project.list', '{}'])) as { projects: { project: { id: string } }[] };
  service = await startServer({ mode: 'production', port: 0, home: join(root, 'autome-home'), userHome: root,
    binaryPath: join(checkout, 'target/debug/autome'), binaryArgs: ['core'],
    kernelEnv: { HOME: root, AUTOMED_CLAUDE_BINARY: join(root, 'bin/fake-cli'), AUTOMED_CODEX_BINARY: join(root, 'bin/fake-cli') },
  });
  const deadline = Date.now() + 10_000;
  while (!service.core.isConnected()) {
    if (Date.now() >= deadline) throw Error('截图内核连接超时');
    await delay(50);
  }
  const address = service.server.address();
  if (!address || typeof address === 'string') throw Error('截图服务没有监听端口');
  browser = await chromium.launchPersistentContext(join(root, 'browser'), { headless: true, viewport: { width: 1512, height: 944 } });
  const page = await browser.newPage();
  mkdirSync(out, { recursive: true });
  for (const [name, path] of [
    ['dashboard', '/'], ['project', `/projects/${encodeURIComponent(projects[0].project.id)}`], ['task', `/tasks/${taskId}`],
  ]) {
    const response = await page.goto(`http://127.0.0.1:${address.port}${path}`);
    expect(response?.status()).toBe(200);
    await expect(page.getByTestId('shell-main')).toBeVisible();
    await expect(page.getByTestId('live-banner')).toHaveCount(0);
    await expect(page.getByTestId('shell-main')).toContainText(title);
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: join(out, `${name}.png`), animations: 'disabled' });
  }
  console.log(`已生成三张 1512×944 截图：${out}`);
} finally {
  try {
    await browser?.close();
  } finally {
    try {
      await service?.close();
    } finally {
      if (canInspect) await cleanupProcesses(root);
      rmSync(root, { recursive: true, force: true });
      if (originalHome === undefined) delete process.env.HOME;
      else process.env.HOME = originalHome;
      if (originalAutomeHome === undefined) delete process.env.AUTOME_HOME;
      else process.env.AUTOME_HOME = originalAutomeHome;
    }
  }
}
