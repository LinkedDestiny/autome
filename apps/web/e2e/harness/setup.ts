import { spawn } from 'node:child_process';
import { closeSync, existsSync, mkdtempSync, openSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { leftoverProcesses } from '../../test/fixtures/process-scan.ts';
import { checkout, prepare } from './fixture.ts';
import { register } from './reap.ts';
import { recordTiming, reportDirectory } from './budget.ts';
import { mkdirSync } from 'node:fs';
import teardown from './teardown.ts';
export default async function setup() {
  mkdirSync(reportDirectory(), { recursive: true });
  writeFileSync(join(reportDirectory(), 'harness.jsonl'), '');
  const root = mkdtempSync(join(tmpdir(), 'autome-e2e-'));
  register({ root });
  process.env.AUTOME_E2E_ROOT = root;
  const liveRoot = mkdtempSync(join(tmpdir(), 'autome-e2e-live-'));
  register({ root: liveRoot });
  process.env.AUTOME_E2E_LIVE_ROOT = liveRoot;
  try {
    // Functional scenarios drive many sessions (C-2 alone needs 23). Avoid a
    // one-second idle wait per tick; production timing is covered by liveRoot.
    await start(root, 100);
    await start(liveRoot, 3000);
  } catch (error) {
    console.error('e2e setup failed:', error);
    await teardown();
    throw error;
  }
}
async function start(root: string, interval: number) {
    leftoverProcesses(root); // Refuse to spawn if cleanup cannot inspect processes.
    const projectId = prepare(root);
    if (interval === 3000) writeFileSync(join(root, 'restart-core'), `#!/bin/sh
if [ -f '${root}/core.pid' ]; then sleep 3; fi
printf '%s\n' "$$" > '${root}/core.pid'
exec '${checkout}/target/debug/autome' "$@"
`, { mode: 0o755 });
    const fd = openSync(join(root, 'server.log'), 'w');
    const child = spawn(process.execPath, [join(import.meta.dirname, 'serve.ts'), root, projectId, String(interval)], { detached: true, stdio: ['ignore', fd, fd], env: { ...process.env, HOME: root, AUTOME_HOME: join(root, 'autome-home') } });
    register({ root, pid: child.pid, group: child.pid });
    closeSync(fd);
    let error: Error | undefined;
    child.on('error', value => { error = value; });
    const started = performance.now();
    const deadline = Date.now() + 15_000;
    try {
    while (!existsSync(join(root, 'state.json'))) {
      if (error) throw error;
      if (child.exitCode !== null || child.signalCode !== null) throw Error(`e2e server exited; inspect ${root}/server.log`);
      if (Date.now() >= deadline) throw Error(`e2e server readiness timed out; inspect ${root}/server.log`);
      await delay(50);
    }
    } finally { recordTiming({ label: `服务就绪 ${root}`, budget: 15_000, duration: performance.now() - started }); }
}
