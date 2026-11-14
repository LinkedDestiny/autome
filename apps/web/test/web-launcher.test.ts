import { afterEach, expect, test } from 'vitest';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { openPanel } from '../server/opener.ts';
import { readInstance, judgeInstance, pidAlive } from '../server/instance.ts';
import { harness, execute, cli, processes } from './fixtures/web-launcher.ts';
const active: ReturnType<typeof harness>[] = [];
function live() { const h = harness(); active.push(h); return h; }
afterEach(async () => { for (const h of active.splice(0)) await h.cleanup(); });
test('无 NODE_ENV 无参数：生产服务、打开器、端口和页面一致', async () => {
  const h = live(); const result = await h.run([]); expect(result.code, result.stderr).toBe(0);
  const v = await h.registration(); expect(h.opened()).toEqual([v.url]); expect((await fetch(v.url)).status).toBe(200);
  expect(await readFile(join(h.home, 'web/logs/server.log'), 'utf8')).toContain('生产模式');
});
test('重复打开只复用一个服务', async () => {
  const h = live(); expect((await h.run([])).code).toBe(0); const v = await h.registration(); expect((await h.run([])).code).toBe(0);
  expect((await h.registration()).pid).toBe(v.pid); expect(h.servers()).toHaveLength(1); expect(h.opened()).toEqual([v.url, v.url]);
});
test('两个并发 start 只起一个服务', async () => {
  const h = live(); const results = await Promise.all([h.run(['start']), h.run(['start'])]); expect(results.map(r => r.code)).toEqual([0, 0]);
  expect(h.servers()).toHaveLength(1); expect(await judgeInstance(await h.registration())).toBe('live');
});
test('status 输出端口、内核路径来源与 autome 构建标识', async () => {
  const h = live(); expect((await h.run(['start'])).code).toBe(0); const v = await h.registration();
  const result = await h.run(['status']); expect(result.code).toBe(0);
  const version = (await execute(cli, ['--version'], { env: h.env })).stdout.match(/\(([^)]+)\)/)![1];
  for (const text of [String(v.port), h.env.AUTOMED_BIN!, 'env', version]) expect(result.stdout).toContain(text);
  const json = await h.run(['status', '--json']); expect(JSON.parse(json.stdout).kernel.build_id).toBe(version);
});
test('status 区分真实驱动锁让路与没有其他驱动', async () => {
  const h = live(); expect((await h.run(['start'])).code).toBe(0);
  expect((await h.run(['status'])).stdout).toContain('没有发现其他驱动');
  // The existing ledger lock is held without invoking another scheduler.
  const lock = execute('/usr/bin/lockf', ['-k', join(h.home, 'state/automed.sqlite3.driver.lock'), '/bin/sleep', '8'], { env: h.env });
  try { await expect.poll(async () => (await h.run(['status'])).stdout, { timeout: 7000 }).toContain('有另一个驱动'); }
  finally { await lock; }
});
test('没有门禁时 stop 清理进程和登记文件', async () => { const h = live(); expect((await h.run(['start'])).code).toBe(0); const v = await h.registration(); const result = await h.run(['stop']); expect(result.code).toBe(0); expect(await readInstance(h.file)).toBeNull(); await expect.poll(() => pidAlive(v.pid)).toBe(false); });
test('非 macOS 打开器打印手动打开提示', async () => { const lines: string[] = []; await openPanel('http://127.0.0.1:1234', { platform: 'linux', executable: '', log: line => lines.push(line) }); expect(lines).toEqual(['请在浏览器里打开 http://127.0.0.1:1234']); });
test('直接 serve 与 start 并发多轮始终只登记一个活服务', async () => {
  const h = live();
  for (let i = 0; i < 3; i++) { h.foreground(); const result = await h.run(['start']); expect(result.code, result.stderr).toBe(0); const v = await h.registration(); await expect.poll(() => h.servers().length).toBe(1); expect(await judgeInstance(v)).toBe('live'); expect((await h.run(['stop'])).code).toBe(0); }
});
test('stop 绑定旧 uuid 不停止新实例、不删文件', async () => {
  const h = live(); expect((await h.run(['start'])).code).toBe(0); const old = await readFile(h.file, 'utf8');
  const result = await h.run(['stop', '--instance', 'old-uuid']); expect(result.code).toBe(0); expect(result.stdout).toContain('已保留'); expect(await readFile(h.file, 'utf8')).toBe(old); expect(await judgeInstance(await h.registration())).toBe('live');
});
test('暂停期间 start/status/stop 保留登记，恢复后 reused', async () => {
  const h = live(); expect((await h.run([])).code).toBe(0); const v = await h.registration(); const old = await readFile(h.file, 'utf8');
  try {
    process.kill(v.pid, 'SIGSTOP');
    for (const command of [['start'], [], ['open']]) { const result = await h.run(command); expect(result.code).toBe(1); expect(result.stderr).toContain('没有响应'); }
    const status = await h.run(['status', '--json']); expect(status.code).toBe(4); expect(JSON.parse(status.stdout)).toMatchObject({ unresponsive: true, pid: v.pid });
    expect((await h.run(['stop'])).code).toBe(1); expect(await readFile(h.file, 'utf8')).toBe(old); expect(h.servers()).toHaveLength(1); expect(h.opened()).toHaveLength(1);
  } finally { process.kill(v.pid, 'SIGCONT'); }
  const result = await h.run(['start', '--json']); expect(JSON.parse(result.stdout)).toMatchObject({ reused: true, pid: v.pid, instance: v.instance }); expect(h.servers()).toHaveLength(1);
});
test('暂停后 stop --force 用 SIGKILL 兜底并清理登记', async () => { const h = live(); expect((await h.run(['start'])).code).toBe(0); const v = await h.registration(); process.kill(v.pid, 'SIGSTOP'); const result = await h.run(['stop', '--force']); expect(result.code, result.stderr).toBe(0); expect(await readInstance(h.file)).toBeNull(); await expect.poll(() => pidAlive(v.pid)).toBe(false); });
test('服务和内核的所有子孙无残留', async () => { const h = live(); expect((await h.run(['start'])).code).toBe(0); expect(h.servers()).toHaveLength(1); await h.cleanup(); expect(processes(h.root)).toEqual([]); active.splice(active.indexOf(h), 1); });
