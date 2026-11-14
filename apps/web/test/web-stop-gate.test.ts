import { afterAll, expect, test } from 'vitest';
import { readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { harness, execute, wt, cli, processes } from './fixtures/web-launcher.ts';
import { judgeInstance, pidAlive, readInstance, type InstanceStatus } from '../server/instance.ts';
let h: ReturnType<typeof harness> | undefined;
let gatePid: number;
let cleaned = false;
async function command(args: string[]) { return execute(cli, args, { cwd: join(h!.root, 'repo'), env: h!.env, timeout: 60000, maxBuffer: 1024 * 1024 }); }
async function setup() {
  h = harness();
  await execute('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), h.root, wt], { env: h.env, timeout: 150000, maxBuffer: 1024 * 1024 });
  h.env.AUTOMED_BIN = join(h.root, 'core-wrapper');
  await writeFile(join(h.root, 'gate.sh'), `while [ ! -f "${h.root}/release-gate" ]; do "${h.root}/bin/w7-sleep" 0.1; done\nexit 0\n`);
  const start = await h.run(['start']); expect(start.code, start.stderr).toBe(0);
  await command(['merge', 'T-1']);
  const v = await h.registration();
  await expect.poll(async () => {
    const info = await (await fetch(v.url + '/api/instance')).json() as InstanceStatus;
    return info.gates.some(gate => gate.task_id === 'T-1');
  }, { timeout: 20000 }).toBe(true);
  // Entering merge_gate precedes spawning its process on a later scheduler tick.
  const root = h.root;
  await expect.poll(() => {
    gatePid = processes(root).find(row => row.command.includes('/gate.sh'))?.pid ?? 0;
    return gatePid;
  }, { timeout: 20000 }).toBeGreaterThan(0);
}
let setupPromise: Promise<void> | undefined;
function ready() { return setupPromise ??= setup(); }
afterAll(async () => { if (h && !cleaned) { await writeFile(join(h.root, 'release-gate'), 'release'); await h.cleanup(); } });
test('门禁未结束时 stop --wait 2 拒绝停止并解释独立进程', async () => {
  await ready();
  const result = await h!.run(['stop', '--wait', '2']); expect(result.code).toBe(1);
  for (const text of ['T-1', '独立进程', '没有停止']) expect(result.stdout).toContain(text);
  expect(await judgeInstance(await h!.registration())).toBe('live'); expect(pidAlive(gatePid)).toBe(true);
}, 180000);
test('stop --force 停服务但门禁继续存活', async () => {
  await ready();
  const result = await h!.run(['stop', '--force']); expect(result.code, result.stderr).toBe(0); expect(result.stdout).toContain('继续在后台运行');
  expect(await readInstance(h!.file)).toBeNull(); expect(pidAlive(gatePid)).toBe(true);
});
test('释放门禁后 autome run 收取结果并完成任务', async () => {
  await ready();
  await writeFile(join(h!.root, 'release-gate'), 'release');
  // The original fixture also blocks main_check, now replaced by our released gate.
  await writeFile(join(h!.root, 'release-main-check'), 'release');
  await command(['run', '--until-idle', '--interval', '0.2']);
  const result = await command(['rpc', 'task.get', '{"task_id":"T-1"}']);
  expect(JSON.parse(result.stdout).task.state.state).toBe('done');
  expect(await readFile(join(h!.root, 'release-gate'), 'utf8')).toBe('release');
});
test('服务、内核、假 CLI 与门禁无残留', async () => { await ready(); await h!.cleanup(); cleaned = true; expect(processes(h!.root)).toEqual([]); });
