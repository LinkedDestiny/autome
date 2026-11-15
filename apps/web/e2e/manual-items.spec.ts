import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect, visit, oneScreen, expandLog } from './harness/browser.ts';
import { cli, startHeldTask, state, newTask } from './harness/fixture.ts';

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
  for (const selector of ['[data-testid=timeline]', '[data-testid=session-log-body]']) {
    await expect(page.locator(selector).first()).toHaveAttribute('data-scroll-region', 'true');
  }
  await visit(page, '/tasks/T-1?tab=docs');
  await oneScreen(page);
  await expect(page.locator('.task-docs-body').first()).toHaveAttribute('data-scroll-region', 'true');
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

function startWindowTask(root: string, slug: string) {
  const fake = join(root, 'repo/.autome/fake', slug);
  mkdirSync(fake, { recursive: true });
  writeFileSync(join(fake, 'next'), '1');
  for (let step = 1; step <= 23; step++) {
    const round = step === 1 ? 0 : Math.floor((step + 2) / 3);
    const document = `# 窗口设计正文\n\nstatus: 设计中\ndesign-round: ${round}/15\nimplementation-round: 0/25\ncurrent-milestone: M-01\ncurrent-milestone-reopens: 0\nconvergence-mode: normal\nnext-action: 继续评审\n\n## 里程碑\n\n| ID | 状态 | 标题 | reopen | 领域 |\n|---|---|---|---|---|\n| M-01 | 开放 | 窗口验证 | 0 | |\n\n## Backlog\n\n## 争议项\n`;
    writeFileSync(join(fake, `${step}.sh`), `set -e
mkdir -p docs/${slug}
cat > docs/${slug}/${slug}.md <<'DOC'
${document}DOC
i=0
while [ "$i" -lt 400 ]; do echo "window evidence line $i"; i=$((i+1)); done
${step >= 22 ? `while [ ! -f '${root}/release-${slug}-${step}' ]; do '${root}/bin/w7-sleep' 0.1; done` : ''}
git add docs/${slug}
git -c user.name=fake -c user.email=f@f commit -qm 'window ${step}'
`);
  }
  return newTask(root, slug);
}

test('C-2 展开日志后新会话到来再加载更早的会话，日志和滚动位置保留', async ({ page }) => {
  const fixture = state(), slug = 'window-history';
  const task = startWindowTask(fixture.root, slug);
  const detail = () => JSON.parse(cli(fixture.root, ['rpc', 'task.get', JSON.stringify({ task_id: task })])) as { sessions: { id: string; running: boolean }[] };
  try {
    await expect.poll(() => detail().sessions.length, { timeout: 75_000 }).toBe(22);
    await visit(page, `/tasks/${task}?tab=docs`);
    await expect(page).toHaveURL(/from=/);
    const from = new URL(page.url()).searchParams.get('from')!;
    const log = page.locator(`[data-testid=session-log][data-session-id="${from}"]`);
    await log.locator('summary').click();
    const body = log.getByTestId('session-log-body');
    await expect(body).toContainText('window evidence line 399');
    await log.getByRole('button', { name: '暂停滚动', exact: true }).click();
    await body.evaluate(e => { e.scrollTop = (e.scrollHeight - e.clientHeight) / 2; });
    const timeline = page.getByTestId('timeline');
    await timeline.evaluate(e => { e.scrollTop = 80; });
    const inner = await body.evaluate(e => e.scrollTop);
    const outer = await timeline.evaluate(e => e.scrollTop);
    expect(inner).toBeGreaterThan(0); expect(outer).toBeGreaterThan(0);
    const retained = async (scroll = outer) => {
      await expect(log).toHaveAttribute('open', '');
      expect(Math.abs(await body.evaluate(e => e.scrollTop) - inner)).toBeLessThanOrEqual(2);
      expect(Math.abs(await timeline.evaluate(e => e.scrollTop) - scroll)).toBeLessThanOrEqual(2);
    };
    writeFileSync(join(fixture.root, `release-${slug}-22`), 'go');
    await expect.poll(() => detail().sessions.length).toBe(23);
    await expect(page.getByTestId('session-log')).toHaveCount(21);
    await retained();
    await page.getByRole('link', { name: '设计文档', exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`from=${from}`));
    await expect(page.locator('.task-docs-body')).toContainText('窗口设计正文');
    await retained();
    // Bring the actual link into view before recording its click-time scroll position.
    const earlier = page.getByRole('link', { name: /显示更早的/ });
    await earlier.scrollIntoViewIfNeeded();
    const beforeEarlier = await timeline.evaluate(e => e.scrollTop);
    await earlier.click();
    await expect(page.getByTestId('session-log')).toHaveCount(23);
    await expect(page).toHaveURL(/timeline=all/);
    expect(new URL(page.url()).searchParams.get('doc')).toBe(`${slug}.md`);
    await retained(beforeEarlier);
  } finally { cli(fixture.root, ['stop', task]); }
});

test('C-2b 直接打开文档链接，挂载后 doc 与 from 同时保留', async ({ page }) => {
  await visit(page, '/tasks/T-1?tab=docs&doc=feat.md');
  await expect(page).toHaveURL(/from=/);
  expect(new URL(page.url()).searchParams.get('doc')).toBe('feat.md');
  await expect(page.locator('.task-docs-body')).toContainText('current-milestone: M-02');
});
