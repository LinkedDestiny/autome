import { setTimeout as delay } from 'node:timers/promises';
import { budgetTask, tickAlerts, withBudgetCli } from './w9-alerts.ts';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { CoreSupervisor } from '../../server/core/supervisor.ts';
import { createAdmin } from '../../server/admin.ts';
import { createGateway } from '../../server/gateway.ts';

export async function w9Kernel() {
  const root = mkdtempSync(join(tmpdir(), 'w9-'));
  const userHome = join(root, 'home'), automeHome = join(root, 'autome');
  mkdirSync(userHome);
  const oldHome = process.env.HOME;
  process.env.HOME = userHome;
  const binaryPath = process.env.AUTOMED_BIN || resolve(import.meta.dirname, '../../../../target/debug/automed');
  const core = new CoreSupervisor({ home: automeHome, binaryPath });
  const close = async () => {
    await core.stop();
    if (oldHome === undefined) delete process.env.HOME; else process.env.HOME = oldHome;
    // stop() waits for the direct child; transient filesystem/descendant writes
    // can still race recursive removal. Exhausting the retry budget must fail.
    await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  };
  try { await core.start(); } catch (error) { await close(); throw error; }
  const admin = createAdmin({ core, userHome, automeHome });
  const gateway = createGateway({ core, home: automeHome, paths: admin.paths });
  admin.register(gateway);
  function repo(name: string) {
    const path = join(userHome, name);
    mkdirSync(path, { recursive: true });
    for (const args of [['init', '-q', '-b', 'main'], ['config', 'user.name', 'fixture'], ['config', 'user.email', 'fixture@example.com'], ['commit', '--allow-empty', '-qm', 'init']]) execFileSync('git', args, { cwd: path });
    return path;
  }
  return { root, userHome, automeHome, binaryPath, core, admin, gateway, repo, close };
}
export async function captureReplies() {
  const s = await w9Kernel();
  try {
    const path = s.repo('single');
    const probe = await s.core.call('project.probe', { path });
    const add = await s.core.call('project.add', { path }) as { project: { id: string } };
    const skip = await s.core.call('project.onboarding.skip', { project_id: add.project.id });
    const config = await s.core.call('config.get', { project_id: add.project.id });
    await s.core.stop();
    await withBudgetCli(s.root, () => s.core.start());
    const alertPath = s.repo('alerts');
    const alertProject = await s.core.call('project.add', { path: alertPath }) as { project: { id: string } };
    await budgetTask(s.core, alertPath, alertProject.project.id);
    const deadline = Date.now() + 20000;
    while (Date.now() < deadline) {
      const alerts = await tickAlerts(s.core);
      if (alerts.length) return { probe, add, skip, config, alert: alerts[0] };
      await delay(50);
    }
    throw new Error('等待真实预算报警超时');
  } finally { await s.close(); }
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  writeFileSync(new URL('./w9-replies.json', import.meta.url), JSON.stringify(await captureReplies(), null, 2) + '\n');
}
