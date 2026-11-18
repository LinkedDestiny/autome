import { spawn } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, readFileSync, mkdtempSync, openSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { test, expect, visit, oneScreen } from './harness/browser.ts';
import { checkout, state } from './harness/fixture.ts';

// T-103 重提：浮层与面板内部排布。全部浏览器断言都在这个新文件里，不改 layout.spec.ts。

test('1512×944 浮层排布 · 角色抽屉', async ({ page }) => {
  await visit(page, '/loop');
  await page.getByTestId('loop-role').first().getByRole('button').click();
  const drawer = page.getByRole('dialog');
  await expect(drawer.locator('.drawer-head').getByRole('button', { name: '关闭', exact: true })).toBeVisible();
  const body = (await drawer.locator('.drawer-body').boundingBox())!;
  const label = (await drawer.getByText('模型', { exact: true }).first().boundingBox())!;
  const model = (await drawer.locator('label', { hasText: '模型' }).locator('input').boundingBox())!;
  expect(model.y).toBeGreaterThanOrEqual(label.y + label.height - 1);
  expect(Math.abs(model.x - label.x)).toBeLessThanOrEqual(2);
  expect(model.width).toBeGreaterThanOrEqual(body.width * 0.9);
  const enable = (await drawer.getByText('启用', { exact: true }).boundingBox())!;
  const runtime = (await drawer.getByRole('radiogroup', { name: '工具' }).boundingBox())!;
  expect(runtime.y).toBeGreaterThanOrEqual(enable.y + enable.height - 1);
  await drawer.getByRole('button', { name: '关闭', exact: true }).click();
  await visit(page, `/loop?project=${encodeURIComponent(state().projectId)}`);
  await page.getByTestId('loop-role').first().getByRole('button').click();
  const foot = drawer.locator('.drawer-foot');
  const [save, reset] = await Promise.all([foot.getByRole('button', { name: '保存', exact: true }).boundingBox(), foot.getByRole('button', { name: '恢复跟随全局' }).boundingBox()]);
  expect(reset!.x - (save!.x + save!.width)).toBeGreaterThanOrEqual(8);
  await oneScreen(page);
});

test('1512×944 浮层排布 · 技能抽屉', async ({ page }) => {
  // HOME 下的 .claude/skills 只对 Claude Code 角色可见：Codex 角色必然置灰。描述不少于 15 行。
  const dir = join(state().root, '.claude/skills', 'overlay-long-skill');
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'SKILL.md'), '---\nname: overlay-long-skill\ndescription: ' + '完整技能说明 '.repeat(300) + '\n---\n# Skill');
  try {
    await visit(page, '/skills');
    await page.getByTestId('skill-card').filter({ hasText: 'overlay-long-skill' }).first().click();
    const drawer = page.getByRole('dialog');
    const checks = drawer.locator('.skill-roles .chk');
    await expect(checks).toHaveCount(6);
    expect(await drawer.locator('.skill-roles .chk-reason').count()).toBeGreaterThan(0);
    const description = (await drawer.locator('.skill-drawer-description').boundingBox())!;
    expect(description.height).toBeLessThanOrEqual(24 * 4 + 8);
    for (const chk of await checks.all()) {
      expect(await chk.evaluate(el => { const text = [...el.childNodes].find(n => n.nodeType === 3 && n.textContent?.trim()); const range = document.createRange(); range.selectNodeContents(text!); return range.getBoundingClientRect().height; })).toBeLessThanOrEqual(24);
      const row = (await chk.boundingBox())!;
      expect(row.y + row.height).toBeLessThanOrEqual(944);
    }
    const save = (await drawer.getByRole('button', { name: '保存绑定' }).boundingBox())!;
    expect(save.y + save.height).toBeLessThanOrEqual(944);
    const roles = (await drawer.locator('.skill-roles').boundingBox())!, source = (await drawer.locator('.skill-source').first().boundingBox())!;
    expect(roles.y + roles.height).toBeLessThanOrEqual(source.y);
    for (const open of await drawer.locator('.skill-source button').all()) expect((await open.boundingBox())!.height).toBeLessThanOrEqual(32);
    await oneScreen(page);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('1512×944 浮层排布 · 技能卡片绑定胶囊', async ({ page }) => {
  const dir = join(state().root, '.claude/skills', 'overlay-pill-skill');
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'SKILL.md'), '---\nname: overlay-pill-skill\ndescription: 绑定胶囊宽度\n---\n# Skill');
  try {
  await visit(page, '/skills');
  const card = (await page.getByTestId('skill-card').filter({ hasText: 'overlay-pill-skill' }).boundingBox())!, binding = (await page.getByTestId('skill-card').filter({ hasText: 'overlay-pill-skill' }).locator('.skill-binding').boundingBox())!;
  expect(binding.width).toBeLessThan(card.width / 2);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('1512×944 浮层排布 · 铃铛面板', async ({ page }) => {
  // 整套 e2e 一起跑时夹具里已有十几条通知；不足 6 条才补，断言「至少 6 条」。
  await page.route('**/api/notifications', async route => {
    const payload = (await (await route.fetch()).json()) as { ok: boolean; payload: { unread: number; items: Record<string, unknown>[] } };
    const items = payload.payload.items;
    for (let i = 0; items.length < 6; i++) items.push({ ...items[0], id: 'pad-' + i, title: '补足通知 ' + '很长的通知标题文字 '.repeat(12), body: '补足正文 '.repeat(30), read_at: null });
    payload.payload.unread = items.filter(item => !item.read_at).length;
    await route.fulfill({ json: payload });
  });
  await visit(page, '/');
  await page.getByTestId('bell-button').click();
  const panel = page.getByTestId('bell-panel');
  await expect(panel).toBeVisible();
  const items = panel.getByTestId('bell-item');
  expect(await items.count()).toBeGreaterThanOrEqual(6);
  const view = (await panel.locator('.bell-list').boundingBox())!;
  for (const item of (await items.all()).slice(0, 5)) {
    const box = (await item.boundingBox())!;
    expect(box.height).toBeLessThanOrEqual(96);
    expect(box.y + box.height).toBeLessThanOrEqual(view.y + view.height);
  }
  for (const button of await panel.locator('button:not(.bell-request)').all()) expect((await button.boundingBox())!.height).toBeLessThanOrEqual(32);
  await oneScreen(page);
});

test('1512×944 浮层排布 · 任务详情页头与文档页签', async ({ page }) => {
  await visit(page, '/tasks/T-1');
  const header = page.locator('.task-detail-header');
  const head = (await header.boundingBox())!, title = (await header.locator('h1').boundingBox())!, status = (await header.locator('.pill').first().boundingBox())!;
  expect(Math.abs(title.x - head.x)).toBeLessThanOrEqual(2);
  expect(title.y).toBeGreaterThanOrEqual(status.y + status.height - 1);
  await page.getByRole('link', { name: /^文档/ }).click();
  const nav = page.locator('.task-docs nav');
  const docs = (await page.locator('.task-docs').boundingBox())!, pills = await nav.locator(':scope > a, :scope > details > summary').all();
  const last = (await pills[pills.length - 1].boundingBox())!;
  expect(last.x + last.width).toBeLessThanOrEqual(docs.x + docs.width - 8);
  const above = (await page.locator('.task-doc-heading').boundingBox())!, navBox = (await nav.boundingBox())!;
  expect(navBox.y).toBeGreaterThanOrEqual(above.y + above.height + 6);
  await expect(page.locator('.task-doc-heading h2')).toHaveCount(0);
  await oneScreen(page);
});

// 环境页：必须确定性地构造「全部正常」。真实探测依赖宿主（iTerm2 固定读 /Applications/iTerm.app），
// 所以给这个用例单独起一个面板，把内核回包里的 env.get 改写成全部正常，再断言确实到达了该场景。
const proxy = `import { spawn } from 'node:child_process';
const [real, ...args] = process.argv.slice(2);
if (args[0] !== 'core') { const r = spawnSync(real, args, { stdio: 'inherit' }); process.exit(r.status ?? 1); }
const child = spawn(real, args, { stdio: ['pipe', 'pipe', 'inherit'] });
process.stdin.pipe(child.stdin);
child.on('exit', code => process.exit(code ?? 0));
const fix = value => {
  if (Array.isArray(value)) return value.forEach(fix);
  if (!value || typeof value !== 'object') return;
  if (Array.isArray(value.components) && value.components.every(c => c && typeof c.component === 'string' && 'present' in c)) {
    for (const c of value.components) { c.present = true; c.version = c.version ?? '1.0.0'; c.path = c.path ?? '/usr/bin/' + c.component; c.login = { login: 'not_applicable' }; }
    if ('problems' in value) value.problems = [];
  }
  Object.values(value).forEach(fix);
};
let buffer = Buffer.alloc(0);
child.stdout.on('data', chunk => {
  buffer = Buffer.concat([buffer, chunk]);
  for (;;) {
    if (buffer.length < 4) return;
    const length = buffer.readUInt32BE(0);
    if (buffer.length < 4 + length) return;
    const message = JSON.parse(buffer.subarray(4, 4 + length).toString('utf8'));
    buffer = buffer.subarray(4 + length);
    fix(message);
    const body = Buffer.from(JSON.stringify(message)), header = Buffer.alloc(4);
    header.writeUInt32BE(body.length, 0);
    process.stdout.write(Buffer.concat([header, body]));
  }
});
`;
const serve = (root: string) => `import { setTimeout as delay } from 'node:timers/promises';
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { startServer } from ${JSON.stringify(join(checkout, 'apps/web/server/main.ts'))};
const service = await startServer({ mode: 'production', port: 0, home: join(${JSON.stringify(root)}, 'autome-home'), userHome: ${JSON.stringify(root)},
  binaryPath: join(${JSON.stringify(root)}, 'env-proxy'), binaryArgs: ['core'], tickIntervalMs: 1000, kernelEnv: { HOME: ${JSON.stringify(root)} } });
const deadline = Date.now() + 10000;
while (!service.core.isConnected()) { if (Date.now() >= deadline) throw Error('core timeout'); await delay(50); }
writeFileSync(join(${JSON.stringify(root)}, 'url'), 'http://127.0.0.1:' + service.server.address().port);
process.on('SIGTERM', () => { void service.close().then(() => process.exit(0)); });
`;
test('1512×944 浮层排布 · 环境页', async ({ page }) => {
  const root = mkdtempSync(join(tmpdir(), 'autome-env-ok-'));
  mkdirSync(join(root, 'autome-home'), { recursive: true });
  writeFileSync(join(root, 'env-proxy.mjs'), proxy);
  writeFileSync(join(root, 'env-proxy'), `#!/bin/sh\nexec '${process.execPath}' '${join(root, 'env-proxy.mjs')}' '${join(checkout, 'target/debug/autome')}' "$@"\n`, { mode: 0o755 });
  writeFileSync(join(root, 'serve.mjs'), serve(root));
  const fd = openSync(join(root, 'server.log'), 'w');
  const child = spawn(process.execPath, [join(root, 'serve.mjs')], { detached: true, stdio: ['ignore', fd, fd], env: { ...process.env, HOME: root, AUTOME_HOME: join(root, 'autome-home') } });
  closeSync(fd);
  try {
    for (let i = 0; !existsSync(join(root, 'url')); i++) { if (i > 300) throw Error('env server not ready; see ' + root + '/server.log'); await delay(50); }
    const url = readFileSync(join(root, 'url'), 'utf8');
    await page.goto(url + '/env');
    await expect(page.getByTestId('shell-main')).toBeVisible();
    const rows = page.locator('.env-component[data-env-ok]');
    await expect(rows.first()).toBeVisible();
    const total = await rows.count();
    expect(total).toBeGreaterThan(0);
    // 确定性到达「全部正常」：不允许条件分支或跳过。
    await expect(page.locator('.env-component[data-env-ok="true"]')).toHaveCount(total);
    for (const row of await page.locator('.env-component[data-env-ok="true"]').all()) expect((await row.boundingBox())!.height).toBeLessThanOrEqual(48);
    const head = await page.locator('.env-page > header > div').first().evaluate(el => [...el.children].map(c => c.getBoundingClientRect().top));
    expect(new Set(head.map(Math.round)).size).toBe(1);
    const drivers = (await page.locator('.env-drivers').boundingBox())!;
    expect(drivers.y + drivers.height).toBeLessThanOrEqual(944);
    expect(await page.locator('.env-components').evaluate(el => el.scrollHeight <= el.clientHeight + 1)).toBe(true);
    await oneScreen(page);
  } finally {
    try { process.kill(-child.pid!, 'SIGKILL'); } catch { /* already gone */ }
    rmSync(root, { recursive: true, force: true });
  }
});
