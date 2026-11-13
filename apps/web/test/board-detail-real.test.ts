import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll, beforeAll, expect, test } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { loadBoard, boardColumns } from '../app/components/board-data.ts';
import { cardView } from '../app/components/task-card-model.ts';
import { columnOf } from '../app/components/task-columns.ts';
import type { KernelGateway } from '../app/load-context.ts';
const started = performance.now();
const wt = resolve(import.meta.dirname, '../../..');
const root = mkdtempSync(join(tmpdir(), 'w6-detail-')), home = join(root, 'home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
const gateway = createGateway({ core, home });
const command = promisify(execFile);
let projectId: string;
beforeAll(async () => {
  await command('sh', [join(wt, 'apps/web/test/fixtures/w5-states.sh'), root, wt], { timeout: 60_000 });
  await core.start({ timeoutMs: 30_000 });
  const raw = await gateway.read({ method: 'project.list', params: {} });
  projectId = (raw.body.payload as { projects: { project: { id: string } }[] }).projects[0].project.id;
}, 120_000);
afterAll(async () => {
  try { await command(join(wt, 'target/debug/autome'), ['stop', 'T-3'], { cwd: join(root, 'repo'), env: { ...process.env, AUTOME_HOME: home }, timeout: 30_000 }); }
  finally { await core.stop(); rmSync(root, { recursive: true, force: true }); }
  process.stdout.write(`W6 board-detail setup/tests/cleanup ${Math.ceil(performance.now() - started)} ms\n`);
});
test('1 real kernel detail produces failure reason, session activity and unrun gate', async () => {
  const loaded = await loadBoard({ gateway }, projectId); if ('error' in loaded) throw Error(loaded.error);
  expect(loaded.tasks.map(t => t.state.state).sort()).toEqual(['active', 'draft', 'failed']);
  const views = loaded.tasks.map(t => ({ state: t.state.state, view: cardView(t, loaded.details![t.id], loaded.cardContext!) }));
  expect(views.find(v => v.state === 'failed')!.view.activity).toMatch(/^失败：/);
  const active = views.find(v => v.state === 'active')!.view;
  expect(active.activity).toContain('任务整理第 1 轮');
  // Same real session, advance only the view clock to exercise elapsed time without sleeping.
  const t = loaded.tasks.find(t => t.state.state === 'active')!;
  expect(cardView(t, loaded.details![t.id], { ...loaded.cardContext!, now: new Date(Date.parse(loaded.now) + 60_000).toISOString() }).activity).toContain('运行');
  expect(active.gate).toBe('门禁未跑过');
}, 120_000);
test('2 real gateway call count equals visible non-draft tasks', async () => {
  const calls: { method: string; params?: unknown }[] = [];
  const counted: KernelGateway = { ...gateway, read: async request => { calls.push(request as { method: string; params?: unknown }); return gateway.read(request); } };
  const loaded = await loadBoard({ gateway: counted }, projectId); if ('error' in loaded) throw Error(loaded.error);
  const recent = boardColumns(loaded.tasks, loaded.now).find(c => c.id === 'done')!.recent;
  const expected = loaded.tasks.filter(t => columnOf(t.state) !== null && t.state.state !== 'draft' && (t.state.state !== 'done' || recent.includes(t)));
  expect(calls.filter(c => c.method === 'project.get')).toHaveLength(1);
  expect(calls.filter(c => c.method === 'task.get').map(c => (c.params as { task_id: string }).task_id).sort()).toEqual(expected.map(t => t.id).sort());
  expect(calls.filter(c => c.method === 'dashboard.get')).toHaveLength(0);
}, 120_000);
