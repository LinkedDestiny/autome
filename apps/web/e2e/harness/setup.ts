import { spawn } from 'node:child_process';
import { closeSync, existsSync, mkdtempSync, openSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { leftoverProcesses } from '../../test/fixtures/process-scan.ts';
import { prepare } from './fixture.ts';
import teardown from './teardown.ts';
export default async function setup() {
  const root = mkdtempSync(join(tmpdir(), 'autome-e2e-'));
  process.env.AUTOME_E2E_ROOT = root;
  try {
    leftoverProcesses(root); // Refuse to spawn if cleanup cannot inspect processes.
    const projectId = prepare(root);
    const fd = openSync(join(root, 'server.log'), 'w');
    const child = spawn(process.execPath, [join(import.meta.dirname, 'serve.ts'), root, projectId], { stdio: ['ignore', fd, fd], env: { ...process.env, HOME: root, AUTOME_HOME: join(root, 'autome-home') } });
    closeSync(fd);
    let error: Error | undefined;
    child.on('error', value => { error = value; });
    const deadline = Date.now() + 15_000;
    while (!existsSync(join(root, 'state.json'))) {
      if (error) throw error;
      if (child.exitCode !== null || child.signalCode !== null) throw Error(`e2e server exited; inspect ${root}/server.log`);
      if (Date.now() >= deadline) throw Error(`e2e server readiness timed out; inspect ${root}/server.log`);
      await delay(50);
    }
  } catch (error) { await teardown(); throw error; }
}
