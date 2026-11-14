import { appendFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect, visit, oneScreen, expandLog } from './harness/browser.ts';
import { cli, startHeldTask, state } from './harness/fixture.ts';

test('W7 H-03 重验保留展开日志和两个滚动位置', async ({ page }) => {
  const fixture = state();
  const task = startHeldTask(fixture.root);
  appendFileSync(join(fixture.root, 'task-ids'), `${task}\n`);
  await visit(page, `/tasks/${task}`);
  const log = page.getByTestId('session-log').last();
  await expect(log).toBeVisible();
  const session = await log.getAttribute('data-session-id');
  expect(session).toBeTruthy();
  const body = log.getByTestId('session-log-body');
  await expect(body).toContainText('scroll evidence line 399');
  await log.getByRole('button', { name: '暂停滚动', exact: true }).click();
  const timeline = page.getByTestId('timeline');
  await body.evaluate(element => { element.scrollTop = (element.scrollHeight - element.clientHeight) / 2; });
  await timeline.evaluate(element => { element.scrollTop = element.scrollHeight - element.clientHeight; });
  const before = await body.evaluate(element => element.scrollTop);
  expect(before).toBeGreaterThan(0);
  expect(await timeline.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
  const count = await page.getByTestId('timeline-entry').count();
  cli(fixture.root, ['pause', task]);
  await expect.poll(() => page.getByTestId('timeline-entry').count()).toBeGreaterThan(count);
  const retained = page.getByTestId('session-log').filter({ has: page.locator(`[data-session-id="${session}"][data-testid="session-log-body"]`) });
  await expect(retained).toHaveAttribute('open', '');
  await expect.poll(async () => Math.abs(await retained.getByTestId('session-log-body').evaluate(element => element.scrollTop) - before)).toBeLessThanOrEqual(2);
  expect(await timeline.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
});

test('W7 H-04 详情一屏且时间线文档日志各自内部滚动', async ({ page }) => {
  await visit(page, '/tasks/T-1');
  await expandLog(page);
  await oneScreen(page);
  for (const selector of ['[data-testid=timeline]', '.task-docs-body', '[data-testid=session-log-body]']) {
    await expect(page.locator(selector).first()).toHaveAttribute('data-scroll-region', 'true');
  }
});

test('W9 H-01 添加项目、设置、环境三页原一屏判据', async ({ page }) => {
  for (const path of ['/projects/new', `/projects/${encodeURIComponent(state().projectId)}/settings`, '/env']) {
    await visit(page, path);
    expect(await page.evaluate(() => [...document.querySelectorAll('.shell-main, .shell-main > *')].every(e => e.scrollHeight <= e.clientHeight))).toBe(true);
  }
});

test('W9 H-02 路径补全点选且普通浏览器没有原生选择按钮', async ({ page }) => {
  await visit(page, '/projects/new');
  await page.getByTestId('path-input').fill('~/');
  const suggestion = page.getByTestId('path-suggestion').filter({ hasText: 'completion-directory' });
  await expect(suggestion).toHaveCount(1);
  await suggestion.click();
  await expect(page.getByTestId('path-input')).toHaveValue('~/completion-directory');
  await expect(page.getByTestId('pick-directory')).toHaveCount(0);
});

test('W9 H-04 复制按钮把页面原命令写入真实剪贴板', async ({ page }) => {
  await visit(page, '/env');
  const button = page.getByTestId('env-copy').first();
  await expect(button).toBeVisible();
  const command = await button.getAttribute('data-command');
  expect(command?.length).toBeGreaterThan(0);
  expect(await button.evaluate(element => element.previousElementSibling?.textContent)).toBe(command);
  await button.click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(command);
});
