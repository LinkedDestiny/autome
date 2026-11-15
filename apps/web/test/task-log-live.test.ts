import { ensureBuilt } from './fixtures/ensure-built.ts';
import { afterAll, beforeAll, expect, it, vi } from 'vitest';
import { mkdirSync, mkdtempSync, writeFileSync, symlinkSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createServer } from 'node:net';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import WebSocket from 'ws';
import { startServer } from '../server/main.ts';
import type { CoreSupervisor } from '../server/core/supervisor.ts';
import type { LogServerMessage } from '../server/eventbus-protocol.ts';
import { TaskTimeline as Timeline } from '../app/components/task-timeline.tsx';
import { MemoryRouter } from 'react-router';
import { timelineEntries as entriesForView } from '../app/components/task-timeline-model.ts';
function TaskTimeline({ sessions, events, now, ...props }: { sessions: Parameters<typeof entriesForView>[0]; events: Parameters<typeof entriesForView>[1]; now: string; truncated?: boolean; error?: string }) {
  return createElement(MemoryRouter, {}, createElement(Timeline, { entries: entriesForView(sessions, events, now), window: { from: null, earlier: 0 }, ...props }));
}
import type { TimelineSession } from '../app/components/task-timeline-model.ts';
import { leftoverProcesses, teardown } from './fixtures/process-leak.ts';
const root = mkdtempSync(join(tmpdir(), 'w7-log-')), home = join(root, 'home'), repo = join(root, 'repo');
const wt = resolve(import.meta.dirname, '../../..'), wrapper = join(root, 'core-wrapper'), fake = join(root, 'fake-cli');
const exec = promisify(execFile);
let running: Awaited<ReturnType<typeof startServer>> | undefined, core: CoreSupervisor | undefined, ws: WebSocket | undefined;
const ids: string[] = [], sessions: TimelineSession[] = [], messages: LogServerMessage[] = [];
const cleanup = async () => {
  if (existsSync(root)) writeFileSync(join(root, 'release'), '');
  ws?.terminate();
  await teardown({ root, core, taskIds: ids, stopServices: async () => { const service = running; running = undefined; await service?.close(); } });
};
beforeAll(async () => {
  try {
    // Fail before spawning any fixture if the environment cannot listen. Never silently skip.
    await new Promise<void>((resolve, reject) => { const probe = createServer(); probe.once('error', reject); probe.listen(0, '127.0.0.1', () => probe.close(error => error ? reject(error) : resolve())); });
    ensureBuilt();
    mkdirSync(home); mkdirSync(repo); mkdirSync(join(repo, '.autome'));
    writeFileSync(join(repo, '.autome/config.toml'), '[retry]\nattempts = 0\n');
    symlinkSync('/bin/sleep', join(root, 'w7-sleep'));
    writeFileSync(fake, `#!/bin/sh\ncat >/dev/null\necho '时间线日志首段'\nwhile [ ! -e '${root}/release' ]; do '${root}/w7-sleep' 0.05; done\necho '时间线日志增量'\n`, { mode: 0o755 });
    writeFileSync(wrapper, `#!/bin/sh\nexport HOME='${home}' AUTOMED_CLAUDE_BINARY='${fake}' AUTOMED_CODEX_BINARY='${fake}'\nexec '${process.env.AUTOMED_BIN ?? join(wt, 'target/debug/automed')}' "$@"\n`, { mode: 0o755 });
    await exec('git', ['init', '-q', '-b', 'main'], { cwd: repo });
    await exec('git', ['-c', 'user.name=test', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-qm', 'init'], { cwd: repo });
    running = await startServer({ port: 0, mode: 'production', home, binaryPath: wrapper }); core = running.gateway.core as CoreSupervisor;
    await vi.waitFor(() => { if (!core?.isConnected()) throw Error('core not ready'); });
    const { project } = await core.call('project.add', { path: repo }) as { project: { id: string } };
    await core.call('project.onboarding.skip', { project_id: project.id });
    for (let i = 0; i < 2; i++) {
      const { task } = await core.call('task.create', { project_id: project.id, kind: 'bug', brief: `# 日志验证 ${i}\n日志接线`, draft: true }) as { task: { id: string } };
      ids.push(task.id); await core.call('task.start', { task_id: task.id });
      let session: TimelineSession | undefined;
      await vi.waitFor(async () => { await core!.call('scheduler.tick', {}); const result = await core!.call('task.get', { task_id: task.id }) as { sessions: TimelineSession[] }; session = result.sessions.find(s => s.running); if (!session) throw Error('session not started'); }, { timeout: 15000, interval: 50 });
      if (i === 0) {
        const log = await core.call('session.log', { session_id: session!.id }) as { path: string };
        await vi.waitFor(() => { if (!readFileSync(log.path, 'utf8').includes('时间线日志首段')) throw Error('log not ready'); }, { timeout: 5000 });
        await core.call('task.stop', { task_id: task.id });
        const result = await core.call('task.get', { task_id: task.id }) as { sessions: TimelineSession[] };
        sessions.push(result.sessions.find(s => s.id === session!.id)!);
      } else sessions.push(session!);
    }
    const address = running.server.address(); if (!address || typeof address === 'string') throw Error('no address');
    const origin = `http://127.0.0.1:${address.port}`; ws = new WebSocket(origin.replace('http:', 'ws:') + '/ws', { origin });
    let hello = false;
    ws.on('message', data => { const m = JSON.parse(data.toString()); if (m.type === 'hello') hello = true; if (m.type.startsWith('log.')) messages.push(m); });
    await vi.waitFor(() => { if (!hello) throw Error('no hello'); });
  } catch (error) { try { await cleanup(); } catch (cleanupError) { throw new AggregateError([error, cleanupError], 'setup and cleanup failed', { cause: cleanupError }); } throw error; }
}, 180_000);
afterAll(cleanup);
it('页面历史会话折叠，运行中会话默认展开', () => {
  expect(sessions.map(s => s.running)).toEqual([false, true]);
  const html = renderToString(createElement(TaskTimeline, { sessions, events: [], now: new Date().toISOString() }));
  expect(html.match(/<details/g)).toHaveLength(2); expect(html.match(/open=""/g)).toHaveLength(1);
});
it('时间线历史与运行会话均能接入 W8 日志订阅且收到增量', async () => {
  for (const session of sessions) ws!.send(JSON.stringify({ type: 'log.subscribe', session_id: session.id, offset: 0 }));
  const text = (id: string) => messages.filter(m => m.type === 'log.chunk' && m.session_id === id).map(m => m.type === 'log.chunk' ? m.text : '').join('');
  for (const session of sessions) await vi.waitFor(() => expect(text(session.id)).toContain('时间线日志首段'), { timeout: 5000 });
  writeFileSync(join(root, 'release'), '');
  await vi.waitFor(() => expect(text(sessions[1].id)).toContain('时间线日志增量'), { timeout: 5000 });
});
it('收尾无存活进程', async () => { await cleanup(); expect(leftoverProcesses(root)).toEqual([]); });
