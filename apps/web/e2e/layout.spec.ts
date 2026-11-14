import { test, expect, visit, oneScreen, expandLog } from './harness/browser.ts';
import { state } from './harness/fixture.ts';
const cases = [
  ['总览', '/'], ['总览命令面板', '/'], ['总览铃铛', '/'],
  ['项目', 'project'], ['全部已完成', 'project?done=all'], ['新建任务', 'project/tasks/new'],
  ['需求详情展开日志', '/tasks/T-1'], ['Bug 详情', '/tasks/T-2'],
  ['添加项目补全', '/projects/new'], ['项目设置', 'project/settings'], ['环境', '/env'],
] as const;
for (const [name, path] of cases) test(`1512×944 ${name}`, async ({ page }) => {
  await visit(page, path.replace(/^project/, `/projects/${encodeURIComponent(state().projectId)}`));
  if (name === '总览命令面板') {
    await page.keyboard.press('Meta+k');
    await expect(page.getByTestId('palette')).toBeVisible();
  }
  if (name === '总览铃铛') {
    await page.getByTestId('bell-button').click();
    await expect(page.getByTestId('bell-panel')).toBeVisible();
  }
  if (name === '需求详情展开日志') await expandLog(page);
  if (name === '添加项目补全') {
    await page.getByTestId('path-input').fill('~/');
    await expect(page.getByTestId('path-suggestion').first()).toBeVisible();
  }
  if (name === '环境') await expect(page.getByTestId('env-copy').first()).toBeVisible();
  await oneScreen(page);
});
