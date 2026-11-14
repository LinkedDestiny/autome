import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './harness/browser.ts';
import { cli, flowSteps, liveState, newTask } from './harness/fixture.ts';

test('D-3 生产 3000ms tick 下命令行批准五秒内换列', async ({ page }) => {
  const f = liveState();
  expect(f.tickIntervalMs).toBe(3000);
  flowSteps(f.root, 'cli-approval');
  const id = newTask(f.root, 'cli-approval');
  let loads = 0;
  page.on('load', () => loads++);
  await page.goto(`${f.url}/projects/${encodeURIComponent(f.projectId)}`);
  const waiting = page.locator(`[data-testid=board-column][data-column=needs_you] [data-task-id="${id}"]`);
  await expect(waiting).toContainText('等你批准', { timeout: 60_000 });
  cli(f.root, ['approve', id]);
  const start = performance.now();
  await expect(waiting).toHaveCount(0, { timeout: 5000 });
  await expect(page.locator(`[data-testid=board-column][data-column=running] [data-task-id="${id}"]`)).toBeVisible({ timeout: Math.max(1, 5000 - (performance.now() - start)) });
  expect(performance.now() - start).toBeLessThanOrEqual(5000);
  expect(loads).toBe(1);
  cli(f.root, ['stop', id]);
});

test('D-4 内核离线横幅与恢复后补齐离线期间命令行建卡', async ({ page }) => {
  const f = liveState();
  expect(f.tickIntervalMs).toBe(3000);
  let loads = 0;
  page.on('load', () => loads++);
  await page.goto(`${f.url}/projects/${encodeURIComponent(f.projectId)}`);
  await expect(page.getByTestId('live-banner')).toHaveCount(0);
  await expect(page.getByTestId('task-card').first()).toBeVisible();
  // Enabled writes prove hydration and the live connection are ready before killing core.
  await expect(page.locator('[data-testid=task-op][data-op=start]').first()).toBeEnabled();
  const pid = Number(readFileSync(join(f.root, 'core.pid'), 'utf8').trim());
  expect(Number.isSafeInteger(pid) && pid > 1).toBe(true);
  process.kill(pid, 'SIGKILL');
  await expect(page.locator('[data-testid=live-banner][data-state=offline]')).toBeVisible();
  const id = newTask(f.root, 'offline-draft', ['--draft']);
  await expect(page.locator('[data-testid=live-banner][data-state=offline]')).toBeVisible();
  await expect(page.getByTestId('live-banner')).toHaveCount(0, { timeout: 15_000 });
  await expect(page.locator(`[data-testid=board-column][data-column=todo] [data-task-id="${id}"]`)).toBeVisible();
  expect(loads).toBe(1);
});
