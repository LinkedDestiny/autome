import { appendFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect, visit } from './harness/browser.ts';
import { flowSteps, newTask, state } from './harness/fixture.ts';

// All subsequent navigation uses visible SPA links, so a document reload fails.
test('D-1 网页草稿到批准合并完成，全程实时且不刷新', async ({ page }) => {
  const f = state();
  flowSteps(f.root, 'browser-feature');
  let loads = 0;
  page.on('load', () => loads++);
  await visit(page, `/projects/${encodeURIComponent(f.projectId)}/tasks/new`);
  await page.getByLabel('任务正文（Markdown）').fill('# browser-feature\n浏览器链路');
  await page.getByTestId('task-new-draft').click();
  const card = page.getByTestId('task-card').filter({ has: page.getByRole('link', { name: 'browser-feature', exact: true }) });
  await expect(card).toBeVisible();
  const id = await card.getAttribute('data-task-id');
  expect(id).toBeTruthy();
  appendFileSync(join(f.root, 'task-ids'), `${id}\n`);
  const inColumn = (column: string) => page.locator(`[data-testid=board-column][data-column=${column}] [data-task-id="${id}"]`);
  await expect(inColumn('todo')).toBeVisible();
  await card.getByRole('button', { name: '开始', exact: true }).click();
  await expect(inColumn('running')).toBeVisible();
  await expect(inColumn('needs_you')).toBeVisible({ timeout: 40_000 });
  await page.getByTestId('bell-button').click();
  await expect(page.locator(`[data-testid=bell-item][data-task-id="${id}"]`).first()).toContainText('等你批准');
  await page.getByTestId('bell-button').click();
  await card.getByRole('link', { name: 'browser-feature', exact: true }).click();
  await page.locator('[data-testid=task-op][data-op=approve]').click();
  await expect(page.getByRole('button', { name: '合并', exact: true })).toBeVisible({ timeout: 40_000 });
  await page.getByRole('button', { name: '合并', exact: true }).click();
  await page.locator(`a[href="/projects/${encodeURIComponent(f.projectId)}"]`).first().click();
  await expect(inColumn('done')).toBeVisible({ timeout: 30_000 });
  expect(loads).toBe(1);
});

test('D-2 命令行 Bug 的定位、复现检查、修复、复核逐轮出现并合并', async ({ page }) => {
  const f = state();
  flowSteps(f.root, 'browser-bug', 'bug');
  const id = newTask(f.root, 'browser-bug', ['--kind', 'bug']);
  let loads = 0;
  page.on('load', () => loads++);
  await visit(page, `/tasks/${id}`);
  for (const [index, node] of ['locate', 'fix', 'fix_review'].entries()) {
    await expect(page.locator(`[data-testid=timeline-entry][data-node=${node}]`).first()).toBeVisible({ timeout: 30_000 });
    if (node === 'locate') await expect(page.locator('[data-testid=timeline-entry][data-node=fix]')).toHaveCount(0);
    if (node === 'fix') {
      await expect(page.locator('[data-testid=timeline-entry][data-node=repro_check]').first()).toContainText('已复现');
      await expect(page.locator('[data-testid=timeline-entry][data-node=fix_review]')).toHaveCount(0);
    }
    writeFileSync(join(f.root, `release-browser-bug-${index + 1}`), 'release');
  }
  const nodes = await page.getByTestId('timeline-entry').evaluateAll(entries => entries.map(e => e.getAttribute('data-node')));
  const positions = ['locate', 'repro_check', 'fix', 'fix_review'].map(node => nodes.indexOf(node));
  expect(positions).toEqual([...positions].sort((a, b) => a - b));
  await expect(page.getByRole('button', { name: '合并', exact: true })).toBeVisible({ timeout: 30_000 });
  await page.getByRole('button', { name: '合并', exact: true }).click();
  await page.locator(`a[href="/projects/${encodeURIComponent(f.projectId)}"]`).first().click();
  await expect(page.locator(`[data-testid=board-column][data-column=done] [data-task-id="${id}"]`)).toBeVisible({ timeout: 30_000 });
  expect(loads).toBe(1);
});
