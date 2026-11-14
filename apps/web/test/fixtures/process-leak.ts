import { rmSync } from 'node:fs';
import { clearProcesses, leftoverProcesses } from './process-scan.ts';
export { leftoverProcesses } from './process-scan.ts';
type CleanupCore = { call: (method: string, params: Record<string, unknown>, options?: { timeoutMs: number }) => Promise<unknown>; stop: () => Promise<unknown> };
export async function teardown({ root, core, stopServices, taskIds = [] }: {
  root: string; core?: CleanupCore; stopServices?: () => unknown; taskIds?: string[];
}) {
  let tracked: number[] = [];
  let scanError: unknown;
  try { tracked = leftoverProcesses(root).map(row => row.pid); } catch (error) { scanError = error; }
  try { await stopServices?.(); } catch { /* Remaining steps must still run. */ }
  for (const taskId of taskIds) {
    try { await core?.call('task.stop', { task_id: taskId }, { timeoutMs: 1000 }); } catch { /* Waiting/terminal tasks reject stop. */ }
  }
  try { await core?.stop(); } catch { /* Fall back to process cleanup. */ }
  if (scanError) throw scanError;
  await clearProcesses(root, tracked);
  rmSync(root, { recursive: true, force: true });
}
