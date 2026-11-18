import { spawn } from 'node:child_process';
import { closeSync, existsSync, mkdirSync, readFileSync, mkdtempSync, openSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { test, expect, oneScreen } from './harness/browser.ts';
import { checkout } from './harness/fixture.ts';

// 环境页正常工具行：先等各工具行（带「查看安装方法」）真正渲染出来再量。
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
test('1512×944 环境页正常工具行高度', async ({ page }) => {
  const root = mkdtempSync(join(tmpdir(), 'autome-env-rows-'));
  mkdirSync(join(root, 'autome-home'), { recursive: true });
  writeFileSync(join(root, 'env-proxy.mjs'), proxy);
  writeFileSync(join(root, 'env-proxy'), `#!/bin/sh\nexec '${process.execPath}' '${join(root, 'env-proxy.mjs')}' '${join(checkout, 'target/debug/autome')}' "$@"\n`, { mode: 0o755 });
  writeFileSync(join(root, 'serve.mjs'), serve(root));
  const fd = openSync(join(root, 'server.log'), 'w');
  const child = spawn(process.execPath, [join(root, 'serve.mjs')], { detached: true, stdio: ['ignore', fd, fd], env: { ...process.env, HOME: root, AUTOME_HOME: join(root, 'autome-home') } });
  closeSync(fd);
  try {
    for (let i = 0; !existsSync(join(root, 'url')); i++) { if (i > 300) throw Error('env server not ready; see ' + root + '/server.log'); await delay(50); }
    await page.goto(readFileSync(join(root, 'url'), 'utf8') + '/env');
    await expect(page.getByTestId('shell-main')).toBeVisible();
    await expect.poll(() => page.locator('.env-component[data-env-ok="true"] summary').count()).toBeGreaterThanOrEqual(1);
    await expect.poll(() => page.locator('[data-env-ok]:not([data-env-ok="true"])').count()).toBe(0);
    for (const row of await page.locator('.env-component[data-env-ok="true"]:has(summary)').all()) {
      expect((await row.boundingBox())!.height).toBeLessThanOrEqual(48);
      const summary = (await row.locator('summary').boundingBox())!;
      expect(Math.abs(summary.height - 30)).toBeLessThanOrEqual(1);
    }
    const drivers = (await page.locator('.env-drivers').boundingBox())!;
    expect(drivers.y + drivers.height).toBeLessThanOrEqual(944);
    expect(await page.locator('.env-components').evaluate(el => el.scrollHeight <= el.clientHeight + 1)).toBe(true);
    await oneScreen(page);
  } finally {
    try { process.kill(-child.pid!, 'SIGKILL'); } catch { /* already gone */ }
    rmSync(root, { recursive: true, force: true });
  }
});
