import { test, expect, visit, oneScreen, expandLog } from './harness/browser.ts';
import { state } from './harness/fixture.ts';
const cases = [
  ['Loop 配置', '/loop'], ['总览', '/'], ['总览命令面板', '/'], ['总览铃铛', '/'],
  ['项目', 'project'], ['全部已完成', 'project?done=all'], ['新建任务', 'project/tasks/new'],
  ['需求详情展开日志', '/tasks/T-1'], ['Bug 详情', '/tasks/T-2'],
  ['添加项目补全', '/projects/new'], ['项目设置', 'project/settings'], ['环境', '/env'], ['已取消展开', 'project?cancelled=all'],
] as const;
for (const [name, path] of cases) test(`1512×944 ${name}`, async ({ page }) => {
  await visit(page, path.replace(/^project/, `/projects/${encodeURIComponent(state().projectId)}`));
  if (name === 'Loop 配置') { await expect(page.getByTestId('loop-role')).toHaveCount(6); await expect(page.locator('.loop-row .card-dash')).toHaveCount(4); await page.screenshot({ path: '../../.autome/output/m07-layout.png' }); }
  if (name.startsWith('总览')) {
    const sidebar = page.locator('.shell-sidebar');
    for (const href of ['/', '/loop', '/skills', '/env']) await expect(sidebar.locator('a[href="' + href + '"]')).toBeVisible();
    await expect(sidebar.getByRole('region', { name: '驱动状态' })).toBeVisible();
    if (name === '总览') {
      await expect(page.locator('.overview-conclusion')).toContainText('件事等你决定');
      await expect(page.locator('.overview-task-card.card-yellow').first()).toBeVisible();
      await expect(page.locator('.overview-tiles .tile')).toHaveCount(4);
      await expect(page.locator('.overview-tiles [data-missing]')).toHaveCount(0);
      await page.screenshot({ path: '../../.autome/output/m03-overview.png' });
    }
  }
  if (name === '总览命令面板') {
    await page.keyboard.press('Meta+k');
    await expect(page.getByTestId('palette')).toBeVisible();
  }
  if (name === '总览铃铛') {
    await page.getByTestId('bell-button').click();
    await expect(page.getByTestId('bell-panel')).toBeVisible();
  }
  if (name === '需求详情展开日志') { await expandLog(page); await expect(page.locator('.task-summary-tiles .tile')).toHaveCount(6); await page.screenshot({ path: '../../.autome/output/m06-detail.png' }); }
  if (name === '添加项目补全') {
    await page.getByTestId('path-input').fill('~/');
    await expect(page.getByTestId('path-suggestion').first()).toBeVisible();
  }
  if (name === '环境') await expect(page.getByTestId('env-copy').first()).toBeVisible();
  if (name === '项目') {
    await expect(page.locator('.board-column .ribbon-sm')).toHaveCount(4);
    await expect(page.locator('.board')).not.toContainText('门禁未跑过');
    await page.screenshot({ path: '../../.autome/output/m04-board.png' });
    await expect(page.locator('.board-column-body[data-scroll-region]')).toHaveCount(4);
    const body = page.locator('[data-column=todo] .board-column-body');
    expect(await body.evaluate(el => el.scrollHeight > el.clientHeight)).toBe(true);
    expect(await body.evaluate(el => getComputedStyle(el).overflowY)).toMatch(/^(auto|scroll)$/);
    await body.evaluate(el => { el.scrollTop = el.scrollHeight; });
    expect(await body.evaluate(el => el.scrollTop)).toBeGreaterThan(0);
    expect(await page.evaluate(() => document.documentElement.scrollTop)).toBe(0);
  }
  if (name === '已取消展开') {
    await expect(page.getByTestId('board-cancelled').getByTestId('task-card')).toHaveCount(30);
    await expect(page.locator('.board-column-body[data-scroll-region]').filter({ has: page.getByTestId('board-cancelled') })).toHaveCount(1);
    await page.screenshot({ path: '../../.autome/output/m11-cancelled.png' });
  }
  await oneScreen(page);
});
