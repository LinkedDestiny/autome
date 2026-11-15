import { driverSummary, readDrivers, type DriverStatus } from '../server/drivers.ts';
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
  // Both parents can observe the winner before the losing serve finishes its lock retry.
  // A second persistent service must still fail this bounded convergence check.
  const registered = await h.registration();
  await expect.poll(() => h.servers().map(server => server.pid), { timeout: 3000 }).toEqual([registered.pid]);
  expect(await judgeInstance(registered)).toBe('live');
});
test('status 输出端口、内核路径来源与 autome 构建标识', async () => {
  const h = live(); expect((await h.run(['start'])).code).toBe(0); const v = await h.registration();
  const result = await h.run(['status']); expect(result.code).toBe(0);
  const version = (await execute(cli, ['--version'], { env: h.env })).stdout.match(/\(([^)]+)\)/)![1];
  for (const text of [String(v.port), h.env.AUTOMED_BIN!, 'env', version]) expect(result.stdout).toContain(text);
  const json = await h.run(['status', '--json']); expect(JSON.parse(json.stdout).kernel.build_id).toBe(version);
});
test('status 使用登记而不是瞬时锁让路推断', async () => {
  const h = live(); expect((await h.run(['start'])).code).toBe(0);
  expect((await h.run(['status'])).stdout).toContain('没有其他驱动');
  const result = JSON.parse((await h.run(['status', '--json'])).stdout);
  expect(result.drivers).toMatchObject({ available: true, list: [expect.objectContaining({ self: true, command: 'autome core' })] });
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

const driver = { pid: 123, command: 'autome run --interval 1', started_at: '2026-01-01T00:00:00Z', last_tick_at: '2026-01-01T00:00:02Z', driving: true, self: false };
const status = (list = [driver]): DriverStatus => ({ available: true, list });
test('status 另一驱动推进时给出 pid 命令与 tick', () => {
  const text = driverSummary(status());
  for (const value of ['另一个驱动在推进', '123', driver.command, driver.last_tick_at]) expect(text).toContain(value);
});
test('status 已登记但最近没有推进不声称正在推进', () => {
  const text = driverSummary(status([{ ...driver, driving: false }]));
  expect(text).toContain('pid 123 已登记但最近没有推进'); expect(text).not.toContain('另一个驱动在推进');
});
test('status 只有本面板或没有登记时没有其他驱动', async () => {
  const read = await readDrivers(async () => ({ drivers: [driver] }), () => 123, () => ({ skipped_last_minute: 0, last_skipped_at: null }));
  expect(read.list[0].self).toBe(true);
  for (const value of [read, status([])]) expect(driverSummary(value)).toBe('没有其他驱动');
});
test('status 旧内核退回让路推断，其他读取失败保留错误', async () => {
  const other = () => ({ skipped_last_minute: 2, last_skipped_at: driver.last_tick_at });
  const old = await readDrivers(async () => { throw Object.assign(Error('unknown'), { code: 'unknown_method' }); }, () => 456, other);
  expect(driverSummary(old)).toContain('内核较旧，只能推断：有另一个驱动');
  expect(driverSummary(undefined, { skipped_last_minute: 0, last_skipped_at: null })).toContain('没有发现其他驱动');
  for (const message of ['拒绝授权', '读盘失败', '请求超时', '内核未连接']) {
    const failed = await readDrivers(async () => { throw Error(message); }, () => 456, other);
    expect(driverSummary(failed)).toBe('驱动信息暂不可用：' + message);
  }
});
