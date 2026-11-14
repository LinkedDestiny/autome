import { setTimeout as delay } from 'node:timers/promises';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from 'node:fs';
import { basename, join } from 'node:path';
import { clearProcesses, leftoverProcesses } from '../../test/fixtures/process-scan.ts';
import { checkout, cli, state } from './fixture.ts';
export default async function teardown() {
  const errors: unknown[] = [];
  for (const root of [process.env.AUTOME_E2E_ROOT, process.env.AUTOME_E2E_LIVE_ROOT]) {
    if (root) try { await cleanup(root); } catch (error) { errors.push(error); }
  }
  if (errors.length) throw new AggregateError(errors, `e2e cleanup failed: ${errors.map(String).join('; ')}`);
}
async function cleanup(root: string) {
  const tracked = leftoverProcesses(root).map(row => row.pid);
  // Stop task groups while the CLI can still find them, then let the server close.
  if (existsSync(join(root, 'task-ids'))) for (const id of readFileSync(join(root, 'task-ids'), 'utf8').trim().split('\n')) {
    try { cli(root, ['stop', id]); } catch { /* Terminal tasks reject stop. */ }
  }
  if (existsSync(join(root, 'state.json'))) {
    const pid = state(root).pid;
    try { process.kill(pid, 'SIGTERM'); } catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error; }
    const deadline = Date.now() + 5000;
    while (leftoverProcesses(root, tracked).some(row => row.pid === pid) && Date.now() < deadline) await delay(50);
  }
  await clearProcesses(root, tracked);
  if (leftoverProcesses(root, tracked).length) throw Error(`e2e leaked processes: ${root}`);
  if (existsSync(join(root, 'server.log'))) {
    const output = join(checkout, '.autome/output');
    mkdirSync(output, { recursive: true });
    copyFileSync(join(root, 'server.log'), join(output, `${basename(root)}-server.log`));
  }
  rmSync(root, { recursive: true, force: true });
}
