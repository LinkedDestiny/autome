import { execFileSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';

export function leftoverProcesses(root: string, tracked: readonly number[] = []) {
  const rows = execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' }).split('\n').flatMap(line => {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/);
    return match ? [{ pid: Number(match[1]), parent: Number(match[2]), command: match[3] }] : [];
  });
  const owned = new Set([...tracked, ...rows.filter(row => row.pid !== process.pid && row.command.includes(root)).map(row => row.pid)]);
  let size = -1;
  while (owned.size !== size) {
    size = owned.size;
    for (const row of rows) if (owned.has(row.parent) && row.pid !== process.pid) owned.add(row.pid);
  }
  return rows.filter(row => owned.has(row.pid) && row.pid !== process.pid);
}
export async function clearProcesses(root: string, tracked: readonly number[] = []) {
  const pids = leftoverProcesses(root, tracked).map(row => row.pid);
  for (const signal of ['SIGTERM', 'SIGKILL'] as const) {
    for (const { pid } of leftoverProcesses(root, pids)) {
      try { process.kill(pid, signal); } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error; }
    }
    const deadline = Date.now() + (signal === 'SIGTERM' ? 5000 : 1000);
    while (leftoverProcesses(root, pids).length && Date.now() < deadline) await delay(50);
  }
  const remaining = leftoverProcesses(root, pids);
  if (remaining.length) throw Error(`清理失败，保留 ${root}：${JSON.stringify(remaining)}`);
}
