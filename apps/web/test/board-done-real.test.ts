import { mkdtempSync, writeFileSync } from 'node:fs';
import { rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import { afterAll, expect, test } from 'vitest';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { loadBoard, doneRow, type BoardTask } from '../app/components/board-data.ts';
import ProjectBoard from '../app/routes/projects.$id.tsx';
const wt = resolve(import.meta.dirname, '../../..'), run = promisify(execFile);
const root = mkdtempSync(join(tmpdir(), 'm04-done-')), home = join(root, 'autome-home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
afterAll(async () => { await core.stop(); await rm(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 }); });
let fixture: Promise<{ projectId: string; task: BoardTask }> | undefined;
function prepare() {
  return fixture ??= (async () => {
    await run('bash', [join(wt, 'apps/web/test/fixtures/w7-flow.sh'), root, wt], { timeout: 150_000, maxBuffer: 1024 * 1024 });
    writeFileSync(join(root, 'release-main-check'), 'release');
    await run(join(wt, 'target/debug/autome'), ['merge', 'T-1'], { cwd: join(root, 'repo'), env: { ...process.env, HOME: root, AUTOME_HOME: home, AUTOMED_CLAUDE_BINARY: join(root, 'bin/fake-cli'), AUTOMED_CODEX_BINARY: join(root, 'bin/fake-cli') }, timeout: 60_000 });
    await core.start();
    const projects = await core.call('project.list', {}) as { projects: { project: { id: string } }[] };
    const projectId = projects.projects[0].project.id;
    await expect.poll(async () => { await core.call('scheduler.tick', {}); return (await core.call('task.get', { task_id: 'T-1' }) as { task: BoardTask }).task.state.state; }, { timeout: 60_000, interval: 100 }).toBe('done');
    const project = await core.call('project.get', { project_id: projectId }) as { tasks: BoardTask[] };
    const task = project.tasks.find(t => t.id === 'T-1')!;
    expect(task.state.state).toBe('done'); expect(task.metrics?.impl_rounds_used).toBeGreaterThan(0);
    return { projectId, task };
  })();
}
test('真实完成需求的六字段投影与同一内核指标相等', async () => {
  const { projectId, task } = await prepare();
  const board = await loadBoard({ gateway: createGateway({ core, home }) }, projectId, true);
  if ('error' in board) throw Error(board.error);
  const row = doneRow(board.tasks.find(t => t.id === task.id)!);
  expect(row.rounds).toBe(task.metrics!.impl_rounds_used); expect(row.reopens).toBe(task.metrics!.reopen_total);
  expect(row.cost).toBe(task.cost_usd == null ? '无计费' : '$' + task.cost_usd.toFixed(2));
}, 210_000);
test('真实已完成页面显示指标且没有门禁未跑过', async () => {
  const { projectId, task } = await prepare();
  const board = await loadBoard({ gateway: createGateway({ core, home }) }, projectId, true);
  const Stub = createRoutesStub([{ id: 'board', path: '/projects/:id', Component: ProjectBoard }]);
  const html = renderToStaticMarkup(h(Stub, { initialEntries: [`/projects/${projectId}?done=all`], hydrationData: { loaderData: { board: { ...board, alerts: [] } } } }));
  expect(html).toContain(`实现 ${task.metrics!.impl_rounds_used} 轮 · 退回 ${task.metrics!.reopen_total}`);
  expect(html).toContain(doneRow(task).cost); expect(html).not.toContain('门禁未跑过');
}, 210_000);
