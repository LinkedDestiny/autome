import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { afterAll, beforeAll, expect, test, vi } from 'vitest';
import { createMemoryRouter } from 'react-router';
import { CoreSupervisor } from '../server/core/supervisor.ts';
import { createGateway } from '../server/gateway.ts';
import { startServer } from '../server/main.ts';
import { loadShell } from '../app/components/shell-data.ts';
import { shouldRevalidate } from '../app/root.tsx';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
const suiteStarted = performance.now();
const wt = resolve(import.meta.dirname, '../../..');
const root = mkdtempSync(join(tmpdir(), 'w5-shell-')), home = join(root, 'home');
const core = new CoreSupervisor({ home, binaryPath: join(root, 'core-wrapper') });
const gateway = createGateway({ core, home });
const command = promisify(execFile);
const cli = (...args: string[]) => command(join(wt, 'target/debug/autome'), args, { cwd: join(root, 'repo'), env: { ...process.env, AUTOME_HOME: home }, timeout: 30_000 });
beforeAll(async () => {
  await command('sh', [join(wt, 'apps/web/test/fixtures/w5-states.sh'), root, wt], { timeout: 60_000 });
  await core.start({ timeoutMs: 30_000 });
}, 120_000);
afterAll(async () => {
  try { await cli('stop', 'T-3'); } catch { /* already stopped by the cross-process test */ }
  await core.stop(); rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
  process.stdout.write(`W5 shell-live including setup/cleanup ${Math.ceil(performance.now() - suiteStarted)} ms\n`);
});
test('1 real kernel counts agree with project.list and preserve draft/active/failed', async () => {
  const started = performance.now(); const shell = await loadShell(gateway);
  process.stdout.write(`W5 loadShell ${Math.ceil(performance.now() - started)} ms\n`);
  expect(shell.kernel.ok).toBe(true);
  expect(shell.tasks.map(task => task.state.state).sort()).toEqual(['active', 'draft', 'failed']);
  const raw = await gateway.read({ method: 'project.list', params: {} }); expect(raw.body.ok).toBe(true);
  const projects = (raw.body.payload as { projects: { project: { id: string }; counts: Record<string, number> }[] }).projects;
  for (const { project, counts } of projects) expect(shell.projects.find(p => p.id === project.id)?.counts).toEqual({ running: counts.running ?? 0, needs_you: ['awaiting_user', 'failed', 'stopped', 'paused'].reduce((sum, key) => sum + (counts[key] ?? 0), 0) });
}, 120_000);
test('2 another CLI process stops active work; ledger event revalidates the root and updates counts', async () => {
  const before = await loadShell(gateway); const active = before.tasks.find(t => t.state.state === 'active')!;
  expect(active).toBeDefined();
  const router = createMemoryRouter([{ path: '/', loader: () => loadShell(gateway), shouldRevalidate }]);
  try {
    await vi.waitFor(() => expect(router.state.initialized).toBe(true));
    await cli('stop', active.id);
    const result = await core.call('events.since', { after_seq: 0, limit: 1000 });
    const events = (result as { events: LedgerEvent[] }).events.filter(e => e.subject === active.id && e.kind === 'task.updated');
    expect(events.length).toBeGreaterThan(0);
    await revalidateForEvents(events, () => router.revalidate());
    const after = router.state.loaderData['0'] as Awaited<ReturnType<typeof loadShell>>;
    expect(after.projects[0].counts).toEqual({ running: before.projects[0].counts!.running - 1, needs_you: before.projects[0].counts!.needs_you + 1 });
    expect(after.tasks.find(t => t.id === active.id)?.state.state).toBe('stopped');
  } finally { router.dispose(); }
}, 120_000);
test('3 listening development SSR contains the real project and needs-you count', async () => {
  const server = await startServer({ mode: 'development', port: 0, home, binaryPath: join(root, 'core-wrapper') });
  try {
    const address = server.server.address(); if (!address || typeof address === 'string') throw new Error('无监听地址');
    const response = await fetch(`http://127.0.0.1:${address.port}/`); expect(response.status).toBe(200);
    const html = await response.text(); const shell = await loadShell(gateway);
    expect(html).toContain(shell.projects[0].name); expect(html).toContain(`等你处理 ${shell.projects[0].counts!.needs_you}`);
  } finally { await server.close(); }
}, 120_000);
