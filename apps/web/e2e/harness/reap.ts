import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { leftoverProcesses } from '../../test/fixtures/process-scan.ts';
import { reportDirectory } from './budget.ts';
type Registration = { root: string; pid?: number; group?: number };
function registry() { return join(reportDirectory(), 'live.json'); }
function entries(): Registration[] { return existsSync(registry()) ? JSON.parse(readFileSync(registry(), 'utf8')) : []; }
export function register(entry: Registration) {
  mkdirSync(reportDirectory(), { recursive: true });
  const rows = entries().filter(row => row.root !== entry.root);
  writeFileSync(registry(), JSON.stringify([...rows, entry]));
}
export function reapRegistered() {
  const errors: string[] = [];
  const retained: Registration[] = [];
  const deadline = Date.now() + Number(process.env.CI_E2E_REAP_MS || 20_000);
  for (const entry of entries()) {
    try {
      const tracked = leftoverProcesses(entry.root).map(row => row.pid);
      if (entry.pid) tracked.push(entry.pid);
      for (const signal of ['SIGTERM', 'SIGKILL'] as const) {
        const targets = leftoverProcesses(entry.root, tracked).map(row => row.pid);
        if (entry.group && targets.includes(entry.group)) targets.unshift(-entry.group);
        for (const pid of targets) {
          try { process.kill(pid, signal); } catch (error) {
            if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error;
          }
        }
        const until = Math.min(deadline, Date.now() + (signal === 'SIGTERM' ? 5000 : 1000));
        while (Date.now() < until && leftoverProcesses(entry.root, tracked).length) {
          Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 50);
        }
      }
      const remaining = leftoverProcesses(entry.root, tracked);
      if (remaining.length) throw Error(JSON.stringify(remaining));
      rmSync(entry.root, { recursive: true, force: true });
    } catch (error) { errors.push(`清理失败 ${entry.root}: ${String(error)}`); retained.push(entry); }
  }
  if (existsSync(registry())) writeFileSync(registry(), JSON.stringify(retained));
  return errors;
}
