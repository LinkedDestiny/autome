import { execFileSync } from 'node:child_process';
import { writeFileSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { RouterContextProvider, createMemoryRouter } from 'react-router';
import { w9Kernel } from './fixtures/w9-replies.ts';
import { createAdmin } from '../server/admin.ts';
import { servicesContext } from '../app/load-context.ts';
import { action as routeAction, loader, shouldRevalidate } from '../app/routes/projects.$id.settings.tsx';
import { revalidateForEvents } from '../app/lib/live-routes.ts';
import type { LedgerEvent } from '../server/eventbus-protocol.ts';
const closes: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of closes.splice(0).reverse()) await close(); });
async function setup() {
  const s = await w9Kernel(); closes.push(s.close);
  await s.core.stop();
  const fake = join(s.root, 'fake-cli'); writeFileSync(fake, '#!/bin/sh\ncat >/dev/null\nmkdir -p docs\nprintf "fake profile" > docs/agent-project-profile.md\nprintf "fake agents" > AGENTS.md\n', { mode: 0o755 });
  const oldClaude = process.env.AUTOMED_CLAUDE_BINARY, oldCodex = process.env.AUTOMED_CODEX_BINARY;
  process.env.AUTOMED_CLAUDE_BINARY = fake; process.env.AUTOMED_CODEX_BINARY = fake;
  try { await s.core.start(); } finally { for (const [key, value] of [['AUTOMED_CLAUDE_BINARY', oldClaude], ['AUTOMED_CODEX_BINARY', oldCodex]]) { if (value === undefined) delete process.env[key!]; else process.env[key!] = value; } }
  const path = s.repo('single');
  const added = await s.gateway.write({ op: 'project.confirm', params: { path: '~/single' } });
  const id = (added.body.payload as { project: { id: string } }).project.id;
  const publish = vi.fn(); const admin = createAdmin({ core: s.core, automeHome: s.automeHome, userHome: s.userHome, publish });
  const context = new RouterContextProvider(); context.set(servicesContext, { ...s, admin });
  const args = { params: { id }, context, request: new Request('http://localhost/projects/' + id + '/settings'), url: new URL('http://localhost/projects/' + id + '/settings'), pattern: '/projects/:id/settings' };
  const submit = (form: Record<string, string>) => action({ ...args, request: new Request(args.request.url, { method: 'POST', body: new URLSearchParams(form) }) });
  const git = (...args: string[]) => execFileSync('git', args, { cwd: path, encoding: 'utf8' }).trim();
  return { ...s, admin, id, path, publish, args, submit, git };
}
test('运行、结束通知、读取编辑两份产物与完成 init commit', async () => {
  const s = await setup(); expect(await s.submit({ intent: 'run' })).toEqual({});
  await vi.waitFor(() => { s.admin.onTick(); expect(s.admin.onboarding.status(s.path).state).toBe('ended'); expect(s.publish).toHaveBeenCalledWith(expect.objectContaining({ type: 'onboarding.ended', project_id: s.id })); }, { timeout: 20000 });
  const data = await loader(s.args); expect(data.files.map(f => f.content)).toEqual(['fake profile', 'fake agents']);
  for (const file of data.files) expect(await s.submit({ intent: 'save', path: file.path, content: file.content + ' edited' })).toEqual({ saved: true });
  expect(await s.submit({ intent: 'finish' })).toHaveProperty('init_commit', s.git('rev-parse', 'HEAD'));
  expect(s.git('log', '-1', '--format=%s')).toBe('chore(autome): initialise project');
  const files = s.git('show', '--pretty=', '--name-only', 'HEAD'); for (const path of ['docs/agent-project-profile.md', 'AGENTS.md', '.gitignore', '.autome/']) expect(files).toContain(path);
  for (const file of data.files) expect(s.git('show', 'HEAD:' + file.path)).toBe(file.content + ' edited');
  expect((await loader(s.args)).project?.onboarding.onboarding).toBe('completed');
  expect(await s.submit({ intent: 'finish' })).toHaveProperty('error', 'Onboarding 已经结束'); expect(await s.submit({ intent: 'run' })).toHaveProperty('error', 'Onboarding 已经结束');
});
test('跳过生成 init commit；工作区内核失败原样显示且状态重读', async () => {
  const s = await setup(); expect(await s.submit({ intent: 'skip' })).toHaveProperty('init_commit'); expect(s.git('log', '-1', '--format=%s')).toBe('chore(autome): initialise project'); expect((await loader(s.args)).project?.onboarding.onboarding).toBe('skipped');
  s.repo('workspace/docs'); const added = await s.gateway.write({ op: 'project.confirm', params: { path: '~/workspace', workspace: true, docs_repo: 'docs' } }); const id = (added.body.payload as { project: { id: string } }).project.id;
  const args = { ...s.args, params: { id } }; const result = await action({ ...args, request: new Request(args.request.url, { method: 'POST', body: new URLSearchParams({ intent: 'skip' }) }) }); expect(result.error).toContain('not a git repository'); expect((await loader(args)).project?.onboarding.onboarding).toBe('skipped');
});
test('超长保存不改变磁盘内容', async () => { const s = await setup(); const before = readFileSync(join(s.path, 'AGENTS.md'), 'utf8'); expect((await s.submit({ intent: 'save', path: 'AGENTS.md', content: 'x'.repeat(262145) })).error).toMatch(/^已拒绝：.*字符上限/); expect(readFileSync(join(s.path, 'AGENTS.md'), 'utf8')).toBe(before); });
test('账本事件刷新持久 router 的步骤', async () => {
  const s = await setup(); const router = createMemoryRouter([{ path: '/', loader: () => loader(s.args), shouldRevalidate }]);
  try { await vi.waitFor(() => expect(router.state.initialized).toBe(true)); expect((router.state.loaderData['0'] as Awaited<ReturnType<typeof loader>>).project?.onboarding.step).toBe(3);
    await s.core.call('project.onboarding.advance', { project_id: s.id }); const events = (await s.core.call('events.since', { after_seq: 0, limit: 1000 }) as { events: LedgerEvent[] }).events.filter(e => e.kind === 'project.onboarding'); expect(events.length).toBeGreaterThan(0);
    await revalidateForEvents(events, () => router.revalidate()); expect((router.state.loaderData['0'] as Awaited<ReturnType<typeof loader>>).project?.onboarding.step).toBe(4);
  } finally { router.dispose(); }
});

async function action(args: Parameters<typeof routeAction>[0]) { const result = await routeAction(args); if (result instanceof Response) throw new Error('Unexpected redirect during onboarding'); return result; }
