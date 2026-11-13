import { execFileSync } from 'node:child_process';
import { rmSync } from 'node:fs';
import { vi } from 'vitest';
export function leftoverProcesses(root: string) {
  return execFileSync('ps', ['-axo', 'pid=,command='], { encoding: 'utf8' }).split('\n').flatMap(line => {
    const match = line.trim().match(/^(\d+)\s+(.*)$/);
    return match && Number(match[1]) !== process.pid && match[2].includes(root) ? [{ pid: Number(match[1]), command: match[2] }] : [];
  });
}
type CleanupCore = { call: (method: string, params: Record<string, unknown>, options?: { timeoutMs: number }) => Promise<unknown>; stop: () => Promise<unknown> };
export async function teardown({ root, core, stopServices, taskIds = [] }: {
  root: string; core?: CleanupCore; stopServices?: () => unknown; taskIds?: string[];
}) {
  try { await stopServices?.(); } catch { /* Remaining steps must still run. */ }
  for (const taskId of taskIds) {
    try { await core?.call('task.stop', { task_id: taskId }, { timeoutMs: 1000 }); } catch { /* Waiting/terminal tasks reject stop. */ }
  }
  try { await core?.stop(); } catch { /* Fall back to process cleanup. */ }
  for (const { pid } of leftoverProcesses(root)) { try { process.kill(pid, 'SIGTERM'); } catch { /* Already exited. */ } }
  try { await vi.waitFor(() => { if (leftoverProcesses(root).length) throw Error('still running'); }, { timeout: 5000, interval: 50 }); } catch { /* Escalate. */ }
  for (const { pid } of leftoverProcesses(root)) { try { process.kill(pid, 'SIGKILL'); } catch { /* Recheck before deleting. */ } }
  try { await vi.waitFor(() => { if (leftoverProcesses(root).length) throw Error('still running'); }, { timeout: 1000, interval: 50 }); } catch { /* Report below. */ }
  const remaining = leftoverProcesses(root);
  if (remaining.length) throw Error(`清理失败，保留 ${root}：${JSON.stringify(remaining)}`);
  rmSync(root, { recursive: true, force: true });
}
