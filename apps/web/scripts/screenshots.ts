// Manual README asset generation; deliberately not a CI step.
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium, expect, type BrowserContext } from '@playwright/test';
import { checkout, cli, flowSteps, newTask, PREPARE_TIMEOUT_MS } from '../e2e/harness/fixture.ts';
import { clearProcesses, leftoverProcesses } from '../test/fixtures/process-scan.ts';
import { dirtyText } from './dirty-text.ts';
import { parseScreenshotArgs, screenshotShots } from './screenshot-shots.ts';
import { startServer } from '../server/main.ts';

async function cleanupProcesses(root: string) {
  const tracked = leftoverProcesses(root).map(row => row.pid);
  await clearProcesses(root, tracked);
  if (leftoverProcesses(root, tracked).length) throw Error(`截图进程残留，保留 ${root}`);
  console.log('截图进程残留检查：0');
}

const options = parseScreenshotArgs(process.argv.slice(2));
const out = resolve(checkout, options.out);
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
    timeout: PREPARE_TIMEOUT_MS, stdio: ['ignore', 'pipe', 'pipe'],
  });
  // Keep the shared fixture's completed history, but leave room for both demo cards.
  const { projects } = JSON.parse(cli(root, ['rpc', 'project.list', '{}'])) as { projects: { project: { id: string } }[] };
  for (const { project } of projects) {
    const { tasks } = JSON.parse(cli(root, ['rpc', 'project.get', JSON.stringify({ project_id: project.id })])) as { tasks: { id: string; state: { state: string } }[] };
    for (const task of tasks) if (!['done', 'cancelled'].includes(task.state.state)) cli(root, ['cancel', task.id]);
  }
  const title = '结算页支持优惠码';
  flowSteps(root, title);
  const taskId = newTask(root, title);
  cli(root, ['run', '--until-idle', '--interval', '0.2']);
  const runningTitle = '优化订单搜索体验';
  flowSteps(root, runningTitle, 'feature', { holdFirstStep: true });
  newTask(root, runningTitle);
  for (const name of ['界面检查', '回归验证', '文档整理']) {
    const dir = join(root, '.claude/skills', name);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, 'SKILL.md'), `---\nname: ${name}\ndescription: 演示用技能\n---\n# ${name}\n`);
  }
  for (const title of ['修复导出报表时区错误', '订单列表增加筛选', '补齐支付失败提示']) {
    newTask(root, title, ['--draft']);
  }
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
  const shots = screenshotShots(options.set, projects[0].project.id, taskId);
  for (const { name, path, click, scroll } of shots) {
    const response = await page.goto(`http://127.0.0.1:${address.port}${path}`);
    expect(response?.status()).toBe(200);
    await expect(page.getByTestId('shell-main')).toBeVisible();
    await expect(page.getByTestId('live-banner')).toHaveCount(0);
    if (['dashboard', 'project', 'task'].includes(name)) await expect(page.getByTestId('shell-main')).toContainText(title);
    if (name === 'dashboard') {
      await expect(page.locator('.overview-task-card.card-yellow').filter({ hasText: title })).toBeInViewport({ ratio: 1 });
      await expect(page.locator('.overview-task-card.card-teal').filter({ hasText: runningTitle })).toBeInViewport({ ratio: 1 });
      await expect(page.getByTestId('shell-main')).not.toContainText('undefined');
    }
    if (name === 'loop') await expect(page.getByTestId('loop-role')).toHaveCount(6);
    if (name === 'skills') await expect(page.getByTestId('skill-card')).toHaveCount(3);
    if (click) {
      const target = page.getByTestId(click.testId).first();
      await (click.button ? target.getByRole('button') : target).click();
      await expect(page.getByRole('dialog')).toBeVisible();
    }
    if (scroll) await page.locator(scroll).scrollIntoViewIfNeeded();
    await page.evaluate(() => document.fonts.ready);
    expect(dirtyText(await page.locator('body').innerText()), name).toEqual([]);
    await page.screenshot({ path: join(out, `${name}.png`), animations: 'disabled' });
  }
  console.log(`已生成 ${shots.length} 张 1512×944 截图：${out}`);
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
