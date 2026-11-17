import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll, beforeAll, expect, it } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { readTaskEvents } from '../server/task-history.ts';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
const wt = resolve(import.meta.dirname, '../../..');
const root = mkdtempSync(join(tmpdir(), 'w7-timeline-'));
const home = join(root, 'autome-home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
const taskEvents = { forTask: (id: string) => readTaskEvents((method, params) => core.call(method, params), id) };
const services = { gateway: createGateway({ core, home }), taskEvents };
const cleanup = async () => { await core.stop(); rmSync(root, { recursive: true, force: true }); };
beforeAll(async () => {
  try {
    await promisify(execFile)('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), root, wt], { timeout: 150_000, maxBuffer: 1024 * 1024 });
    await core.start({ timeoutMs: 30_000 });
  } catch (error) { try { await cleanup(); } catch (cleanupError) { throw new AggregateError([error, cleanupError], 'setup and cleanup failed', { cause: cleanupError }); } throw error; }
}, 180_000);
afterAll(cleanup);
import { taskSummary } from '../app/components/task-summary-model.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
async function summary() {
  const data = await core.call('task.get', { task_id: 'T-1' }) as TaskPayload;
  const history = await taskEvents.forTask('T-1');
  expect(data.task.state?.node).toBe('await_merge');
  return { data, events: history.events, summary: taskSummary(data, history.events, history.truncated) };
}
it('真实会话总数与工具分组等于 task.get', async () => {
  const { data, summary: s } = await summary(); expect(s.sessions.total).toBe(data.sessions.length);
  for (const [runtime, count] of Object.entries(s.sessions.byRuntime)) expect(count).toBe(data.sessions.filter(s => s.runtime === runtime).length);
});
it('真实门禁计数等于指定节点的 result', async () => {
  const { events, summary: s } = await summary();
  expect(s.gates?.total).toBe(events.filter(e => e.kind === 'gate.result' && ['gate','fix_gate','merge_gate'].includes(String(e.payload.node))).length);
  expect(s.gates!.total).toBeGreaterThan(0);
});
it('真实批准用时等于 task.updated 时间差且 loader 不返回全量会话', async () => {
  const { events, summary: s } = await summary();
  const start = events.find(e => e.kind === 'task.updated' && (e.payload.state as {node?:string})?.node === 'await_design_approval')!;
  const end = events.find(e => e.seq > start.seq && e.kind === 'task.updated' && e.payload.trigger === 'approve')!;
  expect(s.steps[3].durationMs).toBe(Date.parse(end.at)-Date.parse(start.at));
  const loaded = await loadTaskDetail(services,'T-1',new URL('http://localhost/tasks/T-1'));
  expect(loaded.summary).toEqual(s); expect(loaded.data?.sessions).toEqual([]);
});
it('终结后门禁统计与 TaskMetrics 相等', async () => {
  writeFileSync(join(root,'release-main-check'),'release');
  await core.call('task.merge',{task_id:'T-1'});
  await expect.poll(async()=> { await core.call('scheduler.tick',{}); return (await core.call('task.get',{task_id:'T-1'}) as TaskPayload).task.state?.state; },{timeout:60000,interval:100}).toBe('done');
  const data=await core.call('task.get',{task_id:'T-1'}) as TaskPayload & {task:{metrics:{round_gate_runs:number;merge_gate_runs:number}}};
  const events=await taskEvents.forTask('T-1'); const s=taskSummary(data,events.events);
  expect(s.gates?.quick).toBe(data.task.metrics.round_gate_runs); expect(s.gates?.merge).toBe(data.task.metrics.merge_gate_runs);
},90000);
