import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect, visit } from './harness/browser.ts';
import { cli, flowSteps, liveState, newTask, state } from './harness/fixture.ts';

test('B-1 网页留备注后实现轮简报含它且页面标已送达', async ({ page }) => {
  const f=state(), slug='browser-notes';flowSteps(f.root,slug);const id=newTask(f.root,slug);
  await visit(page,`/tasks/${id}`);
  await expect(page.getByRole('button',{name:'批准',exact:true})).toBeVisible({timeout:60_000});
  await page.getByRole('link',{name:'备注与依赖',exact:true}).click();
  await page.getByLabel('备注正文').fill('实现时保留这条网页备注');
  await page.getByRole('button',{name:'提交备注',exact:true}).click();
  await expect(page.getByRole('region',{name:'备注',exact:true})).toContainText('等待下一轮');
  await page.getByRole('button',{name:'批准',exact:true}).click();
  await expect(page.getByRole('region',{name:'备注',exact:true})).toContainText('已送达 · 实现 第 1 轮',{timeout:60_000});
  expect(readFileSync(join(f.repo,'.worktree',slug,'docs',slug,'brief/impl-1.md'),'utf8')).toContain('实现时保留这条网页备注');
  await expect(page.getByLabel('同时送回实现轮处理（不合并）')).toBeVisible({timeout:60_000});
});

test('B-4 命令行备注五秒内出现在详情页', async ({ page }) => {
  const f=liveState();expect(f.tickIntervalMs).toBe(3000);const id=newTask(f.root,'cli-notes',['--draft']);
  let loads=0;page.on('load',()=>loads++);
  await page.goto(`${f.url}/tasks/${id}?tab=notes`);
  await expect(page.getByRole('button',{name:'提交备注',exact:true})).toBeEnabled();
  cli(f.root,['note',id,'命令行备注实时显示']);const start=performance.now();
  await expect(page.getByRole('region',{name:'备注',exact:true})).toContainText('命令行备注实时显示',{timeout:5000});
  expect(performance.now()-start).toBeLessThanOrEqual(5000);expect(loads).toBe(1);
});
