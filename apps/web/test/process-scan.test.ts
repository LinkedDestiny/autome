import { afterEach, expect, test, vi } from 'vitest';
import { execFileSync } from 'node:child_process';
import { clearProcesses, leftoverProcesses } from './fixtures/process-scan.ts';
vi.mock('node:child_process', () => ({ execFileSync: vi.fn() }));
afterEach(() => { vi.restoreAllMocks(); vi.useRealTimers(); });
const ps = vi.mocked(execFileSync);

test('扫描私有根及全部子孙，排除无关进程和测试进程自身', () => {
  ps.mockReturnValue(`100 1 node /private-test/server\n101 100 sh\n102 101 sleep\n200 1 unrelated\n${process.pid} 1 /private-test/runner\n`);
  expect(leftoverProcesses('/private-test').map(row => row.pid)).toEqual([100, 101, 102]);
  expect(ps).toHaveBeenCalledWith('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' });
});

test('ps 被拒绝不能解释成无残留', () => {
  ps.mockImplementation(() => { throw Object.assign(Error('ps denied'), { code: 'EPERM' }); });
  expect(() => leftoverProcesses('/private-test')).toThrow('ps denied');
});

test('捕获后父进程退出，仍清理被收养且命令不含根路径的子孙', async () => {
  let rows = '101 1 sh\n102 101 sleep\n200 1 unrelated\n';
  ps.mockImplementation(() => rows);
  const kill = vi.spyOn(process, 'kill').mockImplementation(pid => {
    rows = rows.split('\n').filter(row => !row.startsWith(`${pid} `)).join('\n');
    return true;
  });
  await clearProcesses('/private-test', [101, 102]);
  expect(kill.mock.calls).toEqual([[101, 'SIGTERM'], [102, 'SIGTERM']]);
  expect(rows).toContain('200 1 unrelated');
});

test('TERM 超时后 KILL 并复验，不把超时算成功', async () => {
  let alive = true;
  ps.mockImplementation(() => alive ? '100 1 node /private-test/server\n' : '');
  let now = 0;
  vi.spyOn(Date, 'now').mockImplementation(() => { now += 6000; return now; });
  const kill = vi.spyOn(process, 'kill').mockImplementation((_pid, signal) => {
    if (signal === 'SIGKILL') alive = false;
    return true;
  });
  await clearProcesses('/private-test');
  expect(kill.mock.calls).toEqual([[100, 'SIGTERM'], [100, 'SIGKILL']]);
  expect(leftoverProcesses('/private-test')).toEqual([]);
});
