import { afterEach, expect, test, vi } from 'vitest';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, Outlet, RouterProvider } from 'react-router';
import { w9Kernel } from './fixtures/w9-replies.ts';
import { budgetTask, tickAlerts, withBudgetCli } from './fixtures/w9-alerts.ts';
import ProjectBoard, { action, loader, shouldRevalidate } from '../app/routes/projects.$id.tsx';
import { loader as rootLoader, shouldRevalidate as rootRevalidate } from '../app/root.tsx';
import { ProjectList } from '../app/components/project-list.tsx';
import { createLoadContext } from '../app/load-context.ts';
import type { ShellData } from '../app/components/shell-data.ts';
import type { ProjectAlert } from '../app/components/alerts-data.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import fixture from './fixtures/w9-replies.json' with { type: 'json' };
const closes: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of closes.splice(0).reverse()) await close(); });
async function setup() {
  const s = await w9Kernel(); closes.push(s.close); await s.core.stop(); await withBudgetCli(s.root, () => s.core.start());
  const path = s.repo('alerts'); const { project } = await s.core.call('project.add', { path }) as { project: { id: string } };
  const args = { context: createLoadContext(s)(), params: { id: project.id }, request: new Request('http://localhost/projects/' + project.id), url: new URL('http://localhost/projects/' + project.id), pattern: '/projects/:id' };
  return { ...s, path, id: project.id, args };
}
test('1 真实报警产生和消除由账本事件重载持久 router，无手动 loader', async () => {
  const s = await setup();
  const router = createMemoryRouter([{ id: 'root', path: '/', loader: () => rootLoader(s.args), shouldRevalidate: rootRevalidate, Component: Outlet,
    children: [{ id: 'project', path: 'projects/:id', loader: () => loader(s.args), shouldRevalidate, Component: ProjectBoard }] }], { initialEntries: ['/projects/' + s.id] });
  const board = () => router.state.loaderData.project as Awaited<ReturnType<typeof loader>>;
  const shell = () => router.state.loaderData.root as ShellData;
  try {
    await vi.waitFor(() => expect(router.state.initialized).toBe(true)); expect(board().alerts).toEqual([]); expect(shell().projects[0]).not.toHaveProperty('alerts');
    await budgetTask(s.core, s.path, s.id);
    let alerts: ProjectAlert[] = [];
    await vi.waitFor(async () => { alerts = await tickAlerts(s.core); expect(alerts).toHaveLength(1); }, { timeout: 20000 });
    const events = (await s.core.call('events.since', { after_seq: 0, limit: 1000 }) as { events: LedgerEvent[] }).events.filter(e => e.kind === 'project.alert');
    expect(events).toHaveLength(1); expect(events[0].subject).toBe(s.id);
    await revalidateForEvents(events, () => router.revalidate());
    expect(board().alerts).toEqual(alerts); expect(shell().projects[0].alerts).toBe(1);
    expect(renderToString(createElement(RouterProvider, { router }))).toContain(alerts[0].detail);
    const sidebar = createMemoryRouter([{ path: '/', element: createElement(ProjectList, { projects: shell().projects }) }]);
    try { expect(renderToString(createElement(RouterProvider, { router: sidebar }))).toContain('报警 1'); } finally { sidebar.dispose(); }
    for (const id of ['', 'not-an-alert', 'x'.repeat(512)]) {
      expect(await s.gateway.write({ op: 'project.alert.dismiss', params: { alert_id: id } })).toMatchObject({ status: 409, body: { ok: false } });
      expect((await s.core.call('dashboard.get', {}) as { alerts: ProjectAlert[] }).alerts).toEqual(alerts);
    }
    expect(await action({ ...s.args, request: new Request(s.args.request.url, { method: 'POST', body: new URLSearchParams({ intent: 'dismiss', alert_id: alerts[0].id }) }) })).toMatchObject({ ok: true });
    const dismissed = (await s.core.call('events.since', { after_seq: events[0].seq, limit: 1000 }) as { events: LedgerEvent[] }).events.filter(e => e.kind === 'project.alert_dismissed' && e.subject === alerts[0].id);
    expect(dismissed).toHaveLength(1); expect(dismissed[0].subject).toBe(alerts[0].id);
    await revalidateForEvents(dismissed, () => router.revalidate()); expect(board().alerts).toEqual([]); expect(shell().projects[0]).not.toHaveProperty('alerts');
    expect(renderToString(createElement(RouterProvider, { router }))).not.toContain('board-alerts');
  } finally { router.dispose(); }
});
test('2 报警替身字段与真实 dashboard 完全对齐', async () => {
  const s = await setup(); await budgetTask(s.core, s.path, s.id); let alerts: ProjectAlert[] = [];
  await vi.waitFor(async () => { alerts = await tickAlerts(s.core); expect(alerts).toHaveLength(1); }, { timeout: 20000 });
  const sample = fixture;
  expect(sample.alert).toBeDefined(); expect(Object.keys(sample.alert!).sort()).toEqual(Object.keys(alerts[0]).sort());
  for (const key of Object.keys(alerts[0]) as (keyof ProjectAlert)[]) expect(typeof sample.alert![key]).toBe(typeof alerts[0][key]);
});
