import { afterEach, expect, test, vi } from 'vitest';
import { spawn, type ChildProcess } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createTicker } from '../server/ticker.ts';
import { CoreSupervisor, type TickReport } from '../server/core/supervisor.ts';

const stops: (() => void)[] = [];
afterEach(() => { for (const stop of stops.splice(0)) stop(); vi.useRealTimers(); });
function ticker(options = {}) {
  vi.useFakeTimers();
  const call = vi.fn(async (): Promise<TickReport> => ({}));
  const log = vi.fn();
  const onTick = vi.fn();
  const args = { call, log, onTick, isConnected: () => true, isBusy: () => false, ...options };
  const t = createTicker(args);
  stops.push(t.stop);
  return { ...args, ...t };
}
test('未连接不发', async () => {
  const t = ticker({ isConnected: () => false });
  await vi.advanceTimersByTimeAsync(9000);
  expect(t.call).not.toHaveBeenCalled();
});
test('上次未返回不叠加，也不排队', async () => {
  let resolve!: (r: TickReport) => void;
  const call = vi.fn(() => new Promise<TickReport>(r => { resolve = r; }));
  ticker({ call });
  await vi.advanceTimersByTimeAsync(9000);
  expect(call).toHaveBeenCalledTimes(1);
  resolve({});
  await vi.advanceTimersByTimeAsync(2999);
  expect(call).toHaveBeenCalledTimes(1);
  await vi.advanceTimersByTimeAsync(1);
  expect(call).toHaveBeenCalledTimes(2);
});
test('skipped 安静但仍回调 onTick', async () => {
  const report = { skipped: true, errors: ['不得记录'] };
  const t = ticker({ call: vi.fn(async () => report) });
  await vi.advanceTimersByTimeAsync(3000);
  expect(t.log).not.toHaveBeenCalled();
  expect(t.onTick).toHaveBeenCalledWith(report);
});
test('errors 每条记录一行', async () => {
  const t = ticker({ call: vi.fn(async () => ({ errors: ['甲', '乙'] })) });
  await vi.advanceTimersByTimeAsync(3000);
  expect(t.log.mock.calls).toEqual([['[调度] 甲'], ['[调度] 乙']]);
});
test('调用失败记录原因，下一拍可恢复', async () => {
  const call = vi.fn<() => Promise<TickReport>>().mockRejectedValueOnce(new Error('内核退出')).mockResolvedValue({});
  const t = ticker({ call });
  await vi.advanceTimersByTimeAsync(6000);
  expect(t.log).toHaveBeenCalledExactlyOnceWith('[调度] tick 失败：内核退出');
  expect(call).toHaveBeenCalledTimes(2);
});
test('stop 后不再发', async () => {
  const t = ticker();
  t.stop();
  await vi.advanceTimersByTimeAsync(9000);
  expect(t.call).not.toHaveBeenCalled();
});
test('9000ms 恰好三次，每拍 3000ms', async () => {
  const t = ticker();
  await vi.advanceTimersByTimeAsync(2999);
  expect(t.call).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(6001);
  expect(t.call).toHaveBeenCalledTimes(3);
});
test('本地超时仍 busy 时不叠加，Reply 后下一拍恢复', async () => {
  let busy = false;
  const call = vi.fn(async () => { busy = true; throw new Error('超时'); });
  ticker({ call, isBusy: () => busy });
  await vi.advanceTimersByTimeAsync(9000);
  expect(call).toHaveBeenCalledTimes(1);
  busy = false;
  await vi.advanceTimersByTimeAsync(3000);
  expect(call).toHaveBeenCalledTimes(2);
});
test('真实内核驱动锁占用时 skipped 且日志零调用', async () => {
  const home = mkdtempSync(path.join(os.tmpdir(), 'web-tick-lock-'));
  const options = { home, binaryPath: process.env.AUTOMED_BIN || path.resolve(import.meta.dirname, '../../../target/debug/automed') };
  const c = new CoreSupervisor(options);
  let lock: ChildProcess | undefined;
  let stop: (() => void) | undefined;
  try {
    await c.start();
    lock = spawn('perl', ['-e', 'open(my $f, ">>", $ARGV[0]) or die $!; flock($f, 2) or die $!; $|=1; print "locked\\n"; <STDIN>;', path.join(home, 'state/automed.sqlite3.driver.lock')]);
    await once(lock.stdout!, 'data');
    const log = vi.fn();
    const onTick = vi.fn();
    stop = createTicker({ call: () => c.tick(), isConnected: () => c.isConnected(), isBusy: () => c.isTickBusy(), intervalMs: 10, log, onTick }).stop;
    await vi.waitFor(() => expect(onTick).toHaveBeenCalled(), { timeout: 3000 });
    expect(onTick.mock.calls[0][0]).toMatchObject({ skipped: true });
    expect(log).not.toHaveBeenCalled();
  } finally {
    stop?.();
    if (lock) { const exited = once(lock, 'exit'); lock.stdin!.end(); await exited; }
    await c.stop();
    rmSync(home, { recursive: true, force: true });
  }
});
