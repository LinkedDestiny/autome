import { expect, test } from 'vitest';
import { execFileSync, spawn } from 'node:child_process';
import { closeSync, mkdtempSync, openSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

type ProcessRow = { pid: number; parent: number; command: string };

function processes(): ProcessRow[] {
  return execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' })
    .trim().split('\n').map(line => {
      const match = line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/);
      if (!match) throw new Error(`Cannot parse ps row: ${line}`);
      return { pid: Number(match[1]), parent: Number(match[2]), command: match[3] };
    });
}

function ownedProcesses(root: string, runner: number | undefined): ProcessRow[] {
  const rows = processes();
  const owned = new Set(rows.filter(row => row.command.includes(root) || row.pid === runner).map(row => row.pid));
  let size = -1;
  while (size !== owned.size) {
    size = owned.size;
    for (const row of rows) if (owned.has(row.parent)) owned.add(row.pid);
  }
  return rows.filter(row => owned.has(row.pid));
}

function killProcess(pid: number) {
  try { process.kill(pid, 'SIGKILL'); } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error;
  }
}

test('logtail-live 子测试退出后两秒内释放全部假会话进程', async () => {
  processes(); // Fail before spawning anything if process inspection is unavailable.
  // A private TMPDIR isolates this invocation from existing or concurrent suites.
  const root = mkdtempSync(join(tmpdir(), 'logtail-cleanup-'));
  const output = join(root, 'vitest.log');
  const fd = openSync(output, 'w');
  const web = resolve(import.meta.dirname, '..');
  const child = spawn(process.execPath, [join(web, 'node_modules/vitest/vitest.mjs'), 'run', 'test/logtail-live.test.ts'], {
    cwd: web,
    env: { ...process.env, TMPDIR: root, TMP: root, TEMP: root },
    stdio: ['ignore', fd, fd],
  });
  closeSync(fd);
  try {
    const result = await new Promise<{ code: number | null; signal: string | null }>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Nested logtail-live suite timed out')), 120_000);
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('exit', (code, signal) => { clearTimeout(timer); resolve({ code, signal }); });
    });
    expect(result, readFileSync(output, 'utf8')).toEqual({ code: 0, signal: null });
    // Allow the full contractual grace period; inspect processes, not removed files.
    await delay(2000);
    expect(ownedProcesses(root, undefined), 'Leaked logtail-live processes after child suite exited').toEqual([]);
  } finally {
    // Capture descendants before killing parents, including render-stream and sleep.
    const survivors = ownedProcesses(root, child.exitCode === null && child.signalCode === null ? child.pid : undefined);
    for (const { pid } of survivors.reverse()) killProcess(pid);
    await expect.poll(() => ownedProcesses(root, undefined), { timeout: 2000, interval: 50 }).toEqual([]);
    rmSync(root, { recursive: true, force: true });
  }
}, 130_000);
