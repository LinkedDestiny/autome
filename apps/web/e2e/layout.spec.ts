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
  if (name === '技能' || name === '技能抽屉') for (let i = 0; i < 126; i++) { const dir = join(state().root, '.claude/skills', 'layout-skill-' + i); mkdirSync(dir, { recursive: true }); writeFileSync(join(dir, 'SKILL.md'), '---\nname: layout-skill-' + i + '\ndescription: ' + '完整技能说明 '.repeat(100) + '\n---\n# Skill'); skillDirs.push(dir); }
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
  if (name === '需求详情展开日志') {
    const milestones = page.locator('.task-milestones');
    await expect(milestones).toHaveCSS('flex-wrap', 'nowrap');
    const tops = await milestones.locator('li').evaluateAll(items => items.map(item => item.getBoundingClientRect().top));
    expect(new Set(tops).size).toBe(1);
    const timeline = page.getByTestId('timeline');
    const rows = timeline.locator('li.timeline-row').filter({ hasNot: page.locator('[open]') });
    expect(await rows.count()).toBeGreaterThan(0);
    await timeline.evaluate(el => { el.scrollTop = 0; });
    const bounds = await timeline.boundingBox();
    await test.info().attach('timeline-geometry', {
      body: JSON.stringify(await page.locator('.task-page, .task-detail-header, .task-stage, .task-summary-tiles, .task-controls, .task-timeline, .timeline-group-title, li.timeline-row').evaluateAll(elements => elements.map(el => ({
        className: el.className, rect: el.getBoundingClientRect().toJSON(), scrollHeight: el.scrollHeight, clientHeight: el.clientHeight,
      }))), null, 2),
      contentType: 'application/json',
    });
    await page.screenshot({ path: '../../.autome/output/m07-collapsed-detail.png' });
    for (const row of await rows.all()) expect((await row.boundingBox())!.height).toBeLessThanOrEqual(44);
    for (const row of (await rows.all()).slice(0, 10)) {
      const box = (await row.boundingBox())!;
      expect(box.y).toBeGreaterThanOrEqual(bounds!.y);
      expect(box.y + box.height).toBeLessThanOrEqual(bounds!.y + bounds!.height);
    }
    const plain = timeline.locator('li.timeline-event').filter({ hasNot: page.locator('.session-log, .timeline-failure') }).first().locator('.timeline-text');
    const summary = plain.locator(':scope > summary');
    const complete = await summary.textContent();
    await summary.click();
    await expect(plain).toHaveAttribute('open', '');
    await expect(summary).toHaveCSS('white-space', 'normal');
    expect(await summary.textContent()).toBe(complete);
    await summary.click();
    await page.getByRole('link', { name: /^文档/ }).click();
    const nav = page.locator('.task-docs nav');
    for (const pill of await nav.locator(':scope > a, :scope > details > summary').all()) {
      const rect = (await pill.boundingBox())!, outer = (await nav.boundingBox())!;
      expect(rect.y).toBeGreaterThanOrEqual(outer.y);
      expect(rect.y + rect.height).toBeLessThanOrEqual(outer.y + outer.height);
      expect(await pill.evaluate(el => el.scrollHeight <= el.clientHeight + 1)).toBe(true);
    }
    await expandLog(page); await expect(page.locator('.task-summary-tiles .tile')).toHaveCount(6); await page.screenshot({ path: '../../.autome/output/m06-detail.png' }); }
  if (name === '添加项目补全') {
    await page.getByTestId('path-input').fill('~/');
    await expect(page.getByTestId('path-suggestion').first()).toBeVisible();
    const list = page.locator('.path-suggestions');
    expect((await list.boundingBox())!.height).toBeGreaterThan(160);
    await expect(list).toHaveCSS('flex', '1 1 160px');
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
  if (name === '环境') {
    await expect(page.locator('.env-tool-row')).toHaveCount(5);
    for (const row of await page.locator('.env-component[data-env-ok]').all()) {
      const details = row.locator('details.env-install');
      if (await details.count()) {
        expect(await details.evaluate(el => el.hasAttribute('open'))).toBe((await row.getAttribute('data-env-ok')) !== 'true');
        await details.locator('summary').click();
        expect(await details.evaluate(el => el.hasAttribute('open'))).toBe((await row.getAttribute('data-env-ok')) === 'true');
        await details.locator('summary').click();
      }
    }
    if (await page.locator('.env-component[data-env-ok="true"]').count() === 5) {
      expect(await page.locator('.env-components').evaluate(el => el.scrollHeight <= el.clientHeight + 1)).toBe(true);
    }
  }
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
    const last = page.getByTestId('board-cancelled').getByTestId('task-card').last();
    await last.scrollIntoViewIfNeeded();
    expect(await last.evaluate(el => { const r = el.getBoundingClientRect(); const body = el.closest('.board-column-body')!.getBoundingClientRect(); return r.top >= body.top && r.bottom <= body.bottom && r.bottom <= innerHeight && body.bottom <= innerHeight; })).toBe(true);
    await page.screenshot({ path: '../../.autome/output/m11-cancelled.png' });
  }
  if (name === '技能') {
    await expect(page.getByTestId('skill-card')).toHaveCount(126);
    const grid = page.locator('.skills-grid');
    const cards = await page.getByTestId('skill-card').evaluateAll(nodes => nodes.map(n => { const r = n.getBoundingClientRect(); return { top: r.top, bottom: r.bottom, height: r.height }; }));
    const bounds = await grid.boundingBox();
    expect(cards.filter(r => r.top >= bounds!.y && r.bottom <= Math.min(944, bounds!.y + bounds!.height)).length).toBeGreaterThanOrEqual(9);
    expect(cards.every(r => r.height === 196)).toBe(true);
    expect(await page.locator('.skill-description').first().evaluate(el => getComputedStyle(el).webkitLineClamp)).toBe('3');
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
