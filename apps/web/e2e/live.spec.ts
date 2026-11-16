import { timedExpect } from './harness/budget.ts';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './harness/browser.ts';
import { cli, flowSteps, liveState, newTask } from './harness/fixture.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';

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
  await timedExpect(test.info(), 'live 动态断言', Math.max(1, 5000 - (performance.now() - start)), timeout => expect(page.locator(`[data-testid=board-column][data-column=running] [data-task-id="${id}"]`)).toBeVisible({ timeout }));
  expect(performance.now() - start).toBeLessThanOrEqual(5000);
  expect(loads).toBe(1);
  cli(f.root, ['stop', id]);
});

test('D-4 内核离线横幅与恢复后补齐离线期间命令行建卡', async ({ page }) => {
  const f = liveState();
  expect(f.tickIntervalMs).toBe(3000);
  const events: LedgerEvent[] = [];
  page.on('websocket', ws => ws.on('framereceived', ({ payload }) => {
    const message = JSON.parse(payload.toString());
    if (message.type === 'event') events.push(message);
  }));
  let loads = 0;
  page.on('load', () => loads++);
  await page.goto(`${f.url}/projects/${encodeURIComponent(f.projectId)}`);
  await expect(page.getByTestId('live-banner')).toHaveCount(0);
  await expect(page.getByTestId('task-card').first()).toBeVisible();
  // Enabled writes prove hydration and the live connection are ready before killing core.
  await expect(page.locator('[data-testid=task-op][data-op=start]').first()).toBeEnabled();
  const online = newTask(f.root, 'online-draft', ['--draft']);
  await expect(page.locator(`[data-task-id="${online}"]`)).toBeVisible();
  await expect.poll(() => events.find(event => event.subject === online && event.kind === 'task.created')?.seq).toBeGreaterThan(0);
  const checkpoint = events.find(event => event.subject === online && event.kind === 'task.created')!.seq;
  const pid = Number(readFileSync(join(f.root, 'core.pid'), 'utf8').trim());
  expect(Number.isSafeInteger(pid) && pid > 1).toBe(true);
  process.kill(pid, 'SIGKILL');
  await expect(page.locator('[data-testid=live-banner][data-state=offline]')).toBeVisible();
  const id = newTask(f.root, 'offline-draft', ['--draft']);
  const history = JSON.parse(cli(f.root, ['rpc', 'events.since', JSON.stringify({ subject: id, after_seq: 0, limit: 1000 })])) as { events: { seq: number; kind: string }[] };
  const created = history.events.find(event => event.kind === 'task.created');
  expect(created).toBeDefined();
  expect(created!.seq).toBeGreaterThan(checkpoint);
  await expect(page.locator('[data-testid=live-banner][data-state=offline]')).toBeVisible();
  await expect(page.getByTestId('live-banner')).toHaveCount(0, { timeout: 15_000 });
  await expect(page.locator(`[data-testid=board-column][data-column=todo] [data-task-id="${id}"]`)).toBeVisible();
  await expect.poll(() => events.some(event => event.seq === created!.seq
    && event.subject === id && event.kind === 'task.created')).toBe(true);
  expect(loads).toBe(1);
});
