import { mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect, visit, oneScreen, expandLog } from './harness/browser.ts';
import { state } from './harness/fixture.ts';
const cases = [
  ['角色抽屉', '/loop'], ['技能抽屉', '/skills'],
  ['技能', '/skills'], ['Loop 配置', '/loop'], ['总览', '/'], ['总览命令面板', '/'], ['总览铃铛', '/'],
  ['项目', 'project'], ['全部已完成', 'project?done=all'], ['新建任务', 'project/tasks/new'],
  ['需求详情展开日志', '/tasks/T-1'], ['Bug 详情', '/tasks/T-2'],
  ['添加项目补全', '/projects/new'], ['项目设置', 'project/settings'], ['环境', '/env'], ['已取消展开', 'project?cancelled=all'],
] as const;
for (const [name, path] of cases) test(`1512×944 ${name}`, async ({ page }) => {
  const skillDirs: string[] = [];
  if (name === '技能' || name === '技能抽屉') for (let i = 0; i < 24; i++) { const dir = join(state().root, '.claude/skills', 'layout-skill-' + i); mkdirSync(dir, { recursive: true }); writeFileSync(join(dir, 'SKILL.md'), '# Skill'); skillDirs.push(dir); }
  try {
  await visit(page, path.replace(/^project/, `/projects/${encodeURIComponent(state().projectId)}`));
  if (name === 'Loop 配置') { await expect(page.getByTestId('loop-role')).toHaveCount(6); await expect(page.locator('.loop-row .card-dash')).toHaveCount(4); await page.screenshot({ path: '../../.autome/output/m07-layout.png' }); }
  if (name === 'Loop 配置') {
    const scope = page.getByRole('navigation', { name: '配置范围' });
    const current = scope.locator('a[aria-current="page"]');
    const assertSelected = async () => {
      const style = await current.evaluate(el => {
        const computed = getComputedStyle(el);
        const probe = document.createElement('span');
        probe.style.backgroundColor = 'var(--ui-surface)';
        el.append(probe);
        const surface = getComputedStyle(probe).backgroundColor;
        probe.remove();
        return { background: computed.backgroundColor, shadow: computed.boxShadow, surface };
      });
      expect(style.background).toBe(style.surface);
      expect(style.shadow).not.toBe('none');
      await expect(scope).toHaveCSS('column-gap', '4px');
    };
    const assertRoleCards = async () => {
      for (const card of await page.getByTestId('loop-role').all()) {
        const bounds = await card.boundingBox();
        expect(bounds!.height).toBeLessThanOrEqual(160);
        const model = await card.locator('.loop-role-model > span').first().boundingBox();
        const effort = await card.locator('.loop-role-model .pill').boundingBox();
        expect(Math.abs((model!.y + model!.height / 2) - (effort!.y + effort!.height / 2))).toBeLessThanOrEqual(1);
      }
    };
    await expect(current).toHaveText('全局默认');
    await assertSelected();
    await assertRoleCards();
    await scope.locator('a').nth(1).click();
    await expect(current).not.toHaveText('全局默认');
    await assertSelected();
    await assertRoleCards();
    await expect(page.getByRole('navigation', { name: '项目配置页签' }).locator('[aria-current="page"]')).toHaveText('Loop 配置');
    await scope.getByRole('link', { name: '全局默认' }).click();
    await expect(current).toHaveText('全局默认');
  }
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
  if (name === '项目设置') {
    await expect(page).toHaveURL(/\/loop\?project=.+&tab=project$/);
    await expect(page.getByTestId('page-settings')).toBeVisible();
    const sections = page.locator('.project-settings > section');
    await expect(sections).toHaveCount(3);
    for (const section of await sections.all()) expect(await section.evaluate(el => el.scrollHeight <= el.clientHeight + 1)).toBe(true);
    const remove = page.locator('#remove-name');
    await remove.scrollIntoViewIfNeeded();
    expect(await remove.evaluate(el => { const r = el.getBoundingClientRect(); const parent = el.closest('.project-settings')!.getBoundingClientRect(); return r.top >= Math.max(0, parent.top) && r.bottom <= Math.min(innerHeight, parent.bottom) && r.left >= 0 && r.right <= innerWidth; })).toBe(true);
    await page.screenshot({ path: '../../.autome/output/m08-settings.png' });
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
  if (name === '技能') {
    await expect(page.getByTestId('skill-card')).toHaveCount(24);
    const grid = page.locator('.skills-grid');
    expect(await grid.evaluate(el => el.scrollHeight > el.clientHeight)).toBe(true);
    await grid.evaluate(el => { el.scrollTop = el.scrollHeight; });
    expect(await grid.evaluate(el => el.scrollTop)).toBeGreaterThan(0);
    await page.screenshot({ path: '../../.autome/output/m09-layout.png' });
  }
  if (['环境', '新建任务', '添加项目补全'].includes(name)) await page.screenshot({ path: '../../.autome/output/m10-' + name + '.png' });
  if (name === '角色抽屉' || name === '技能抽屉') {
    if (name === '角色抽屉') await page.getByTestId('loop-role').first().getByRole('button').click();
    else await page.getByTestId('skill-card').first().click();
    const drawer = page.getByRole('dialog');
    await expect(drawer.locator('.drawer-head').getByRole('button', { name: '关闭', exact: true })).toBeVisible();
    expect(await drawer.locator('input, select, textarea').evaluateAll(nodes => nodes.filter(n => getComputedStyle(n).appearance !== 'none').length)).toBe(0);
    const foot = await drawer.locator('.drawer-foot').boundingBox();
    expect(foot).not.toBeNull();
    expect(foot!.y).toBeGreaterThanOrEqual(0);
    expect(foot!.y + foot!.height).toBeLessThanOrEqual(944);
    expect(foot!.x).toBeGreaterThanOrEqual(0);
    expect(foot!.x + foot!.width).toBeLessThanOrEqual(1512);
  }
  await oneScreen(page);
  } finally { for (const dir of skillDirs) rmSync(dir, { recursive: true, force: true }); }
});
