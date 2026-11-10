import { afterEach, expect, test, vi } from 'vitest';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { CoreSupervisor, CALL_TIMEOUT_MS, type CoreStatus } from '../server/core/supervisor.ts';

const binary = process.env.AUTOMED_BIN || path.resolve(import.meta.dirname, '../../../target/debug/automed');
const fail = path.join(import.meta.dirname, 'fixtures/automed-fail-fast.sh');
const homes: string[] = [];
const cores: CoreSupervisor[] = [];
function core(binaryPath = binary, extra = {}) {
  const home = mkdtempSync(path.join(os.tmpdir(), 'web-supervisor-'));
  homes.push(home);
  const options = { binaryPath, home, ...extra };
  const supervisor = new CoreSupervisor(options);
  cores.push(supervisor);
  return supervisor;
}
afterEach(async () => {
  for (const c of cores.splice(0)) await c.stop();
  for (const h of homes.splice(0)) rmSync(h, { recursive: true, force: true });
  vi.useRealTimers();
});
const until = (predicate: () => boolean) => vi.waitFor(() => expect(predicate()).toBe(true), { timeout: 5000, interval: 10 });

test('对账回复之后才 connected，call 保留内核 payload 和错误 code', async () => {
  const states: CoreStatus[] = [];
  const c = core(binary, { onStatus: (s: CoreStatus) => states.push(s) });
  const ready = c.start();
  expect(c.status.state).toBe('starting');
  await ready;
  expect(states.map(s => s.state)).toEqual(['starting', 'connected']);
  expect(await c.call('config.get')).toBeTruthy();
  await expect(c.call('not.a.method')).rejects.toMatchObject({ code: expect.any(String) });
});
test('真实内核存活满 5 秒被杀，1 秒后重启并清零计数', async () => {
  let now = 0;
  const c = core(binary, { now: () => now });
  await c.start();
  c.status.attempts = 2;
  const pid = c.sidecar!._child!.pid!;
  now = 5000;
  c.sidecar!._child!.kill('SIGKILL');
  await until(() => c.status.state === 'restarting');
  expect(c.status.attempts).toBe(0);
  await until(() => c.isConnected());
  expect(c.sidecar!._child!.pid).not.toBe(pid);
});
test('连续三次快速失败 offline，原因来自 stderr，之后不再 spawn', async () => {
  const states: CoreStatus[] = [];
  const c = core(fail, { onStatus: (s: CoreStatus) => states.push(s) });
  await expect(c.start()).rejects.toThrow();
  await until(() => c.status.state === 'offline');
  expect(c.status).toMatchObject({ attempts: 3, reason: '内核测试启动失败' });
  vi.useFakeTimers();
  await vi.advanceTimersByTimeAsync(9000);
  expect(states.filter(s => s.state === 'starting')).toHaveLength(3);
});
test('offline 后手动重启成功并清零', async () => {
  const home = mkdtempSync(path.join(os.tmpdir(), 'web-repair-'));
  homes.push(home);
  const executable = path.join(home, 'automed');
  writeFileSync(executable, readFileSync(fail), { mode: 0o755 });
  const c = core(executable);
  await expect(c.start()).rejects.toThrow();
  await until(() => c.status.state === 'offline');
  writeFileSync(executable, '#!/bin/sh\nexec "' + binary + '"\n');
  expect(await c.restart()).toEqual({ restarted: true });
  expect(c.status.attempts).toBe(0);
});
test('手动重启仍失败，抛出内核原话', async () => {
  const c = core(fail);
  await expect(c.restart()).rejects.toThrow('内核测试启动失败');
  expect(c.status.attempts).toBe(1);
  await expect(c.restart()).rejects.toThrow('内核测试启动失败');
  expect(c.status.attempts).toBe(1);
});
test('运行中手动重启 already_running，不换进程', async () => {
  const c = core();
  await c.start();
  const pid = c.sidecar!._child!.pid;
  expect(await c.restart()).toEqual({ already_running: true });
  expect(c.sidecar!._child!.pid).toBe(pid);
});
test('重启延迟期间 stop 取消下一次 spawn', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  const states: CoreStatus[] = [];
  const c = core(fail, { onStatus: (s: CoreStatus) => states.push(s) });
  await expect(c.start()).rejects.toThrow();
  expect(c.status.state).toBe('restarting');
  await c.stop();
  await vi.advanceTimersByTimeAsync(1000);
  expect(states.filter(s => s.state === 'starting')).toHaveLength(1);
  expect(c.status.state).toBe('stopped');
});
test('缺失二进制按三次快速失败处理，进程不崩溃', async () => {
  const c = core('/no-such-automed');
  await expect(c.start()).rejects.toThrow();
  await until(() => c.status.state === 'offline');
  expect(c.status).toMatchObject({ attempts: 3, reason: expect.stringContaining('无法启动内核') });
});
test('对账超时保持 starting 和 busy，迟到 Reply 才 connected', async () => {
  const home = mkdtempSync(path.join(os.tmpdir(), 'web-delayed-'));
  homes.push(home);
  const executable = path.join(home, 'delayed');
  writeFileSync(executable, `#!/bin/sh\nexport FAKE_CORE_DELAY_MS=150\nexec "${process.execPath}" "${path.join(import.meta.dirname, 'fixtures/fake-core.mjs')}"\n`, { mode: 0o755 });
  const c = core(executable);
  await expect(c.restart({ timeoutMs: 10 })).rejects.toMatchObject({ timeout: true });
  expect(c.status.state).toBe('starting');
  expect(c.isTickBusy()).toBe(true);
  expect(await c.restart()).toEqual({ already_running: true });
  await expect(c.tick()).rejects.toThrow('调度仍在执行');
  await until(() => c.isConnected());
  expect(c.isTickBusy()).toBe(false);
  await expect(c.call('slow', {}, { timeoutMs: 1 })).rejects.toMatchObject({ timeout: true });
  await c.stop();
  expect(c.isTickBusy()).toBe(false);
  await expect(c.call('config.get')).rejects.toThrow('内核未连接');
  const pending = c.restart();
  const rejected = expect(pending).rejects.toThrow();
  await c.stop();
  await rejected;
  expect(c.status.state).toBe('stopped');
  expect(c.isTickBusy()).toBe(false);
});
test('CALL_TIMEOUT_MS 大于 Rust DRIVER_LOCK_WAIT', () => {
  const source = readFileSync(path.resolve(import.meta.dirname, '../../../crates/automed/src/dispatch.rs'), 'utf8');
  const seconds = Number(source.match(/DRIVER_LOCK_WAIT[^=]*=\s*(?:std::time::)?Duration::from_secs\((\d+)\)/)![1]);
  expect(CALL_TIMEOUT_MS).toBeGreaterThan(seconds * 1000);
});
