import { ensureBuilt } from './fixtures/ensure-built.ts';
import { afterAll, beforeAll, expect, test, vi } from 'vitest';
import { closeSync, mkdirSync, mkdtempSync, openSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { randomUUID } from 'node:crypto';
import WebSocket from 'ws';
import { startServer } from '../server/main.ts';
import type { CoreSupervisor } from '../server/core/supervisor.ts';
import type { LogServerMessage } from '../server/eventbus-protocol.ts';
import { configureFailureRepo } from './fixtures/notifier-failure.ts';

const root = mkdtempSync(join(tmpdir(), 'logtail-live-'));
const home = join(root, 'home'), repo = join(root, 'repo'), fake = join(root, 'fake-cli');
const wt = resolve(import.meta.dirname, '../../..'), wrapper = join(root, 'automed-wrapper');
const marker = `marker-${randomUUID()}`;
const empty = { connections: 0, subscriptions: 0, openHandles: 0, timer: false };
const exec = promisify(execFile);
let running: Awaited<ReturnType<typeof startServer>> | undefined;
let core: CoreSupervisor, ws: WebSocket, projectId: string, sessionId: string, logPath: string;
const messages: LogServerMessage[] = [];
const chunks = () => messages.filter(m => m.type === 'log.chunk');
const text = () => chunks().map(m => m.text).join('');
const send = (message: object) => ws.send(JSON.stringify(message));
const handles = async () => (await exec('lsof', ['-nP', '-p', String(process.pid), '-Fn'])).stdout.split('\n');

async function startTask() {
  const { task } = await core.call('task.create', { project_id: projectId, kind: 'bug', brief: `# 日志验证-${randomUUID()}\n测试会话日志`, draft: true }) as { task: { id: string } };
  await core.call('task.start', { task_id: task.id });
  let id = '';
  await expect.poll(async () => {
    await core.call('scheduler.tick', {});
    const result = await core.call('task.get', { task_id: task.id }) as { sessions: { id: string; running: boolean }[] };
    id = result.sessions.find(s => s.running)?.id ?? '';
    return id;
  }, { timeout: 15000, interval: 50 }).not.toBe('');
  return id;
}

beforeAll(async () => {
  ensureBuilt();
  mkdirSync(home); mkdirSync(repo);
  configureFailureRepo(repo);
  writeFileSync(fake, `#!/bin/sh
set -eu
cat >/dev/null
root=$(dirname "$0")
printf '第一行\\n'
while [ ! -e "$root/go" ]; do
  [ -d "$root" ] || exit 0
  sleep 0.05
done
printf '%s\\n' '${marker}'
while [ ! -e "$root/stop" ]; do
  [ -d "$root" ] || exit 0
  sleep 0.05
done
printf '最后一行\\n'
`, { mode: 0o755 });
  writeFileSync(wrapper, `#!/bin/sh
export HOME="${home}"
export AUTOMED_CLAUDE_BINARY="${fake}" AUTOMED_CODEX_BINARY="${fake}"
exec "${join(wt, 'target/debug/automed')}" "$@"
`, { mode: 0o755 });
  await exec('git', ['init', '-q', '-b', 'main'], { cwd: repo });
  await exec('git', ['-c', 'user.name=test', '-c', 'user.email=t@t', 'commit', '--allow-empty', '-qm', 'init'], { cwd: repo });
  running = await startServer({ port: 0, mode: 'production', home, binaryPath: wrapper });
  core = running.gateway.core as CoreSupervisor;
  await vi.waitFor(() => expect(core.isConnected()).toBe(true), { timeout: 10000 });
  const { project } = await core.call('project.add', { path: repo }) as { project: { id: string } };
  projectId = project.id;
  await core.call('project.onboarding.skip', { project_id: projectId });
  const address = running.server.address();
  if (!address || typeof address === 'string') throw new Error('无监听地址');
  const origin = `http://127.0.0.1:${address.port}`;
  ws = new WebSocket(origin.replace('http:', 'ws:') + '/ws', { origin });
  let hello = false;
  ws.on('message', data => {
    const message = JSON.parse(data.toString());
    if (message.type === 'hello') hello = true;
    if (message.type.startsWith('log.')) messages.push(message);
  });
  await vi.waitFor(() => expect(hello).toBe(true));
}, 90000);

afterAll(async () => {
  // Release fake CLIs even when an earlier assertion fails.
  writeFileSync(join(root, 'go'), ''); writeFileSync(join(root, 'stop'), '');
  ws?.terminate();
  await running?.close();
  rmSync(root, { recursive: true, force: true });
});

test('真实会话通过同一 /ws 在一个 tick 内收到增量', async () => {
  sessionId = await startTask();
  logPath = (await core.call('session.log', { session_id: sessionId }) as { path: string }).path;
  send({ type: 'log.subscribe', session_id: sessionId, offset: 0 });
  await expect.poll(text, { timeout: 5000, interval: 20 }).toContain('第一行');
  expect(text()).toContain('started');
  const started = performance.now();
  writeFileSync(join(root, 'go'), '');
  await expect.poll(text, { timeout: 3000, interval: 10 }).toContain(marker);
  const elapsed = performance.now() - started;
  expect(elapsed).toBeLessThan(3000);
  console.log(`日志增量耗时：${Math.ceil(elapsed)}ms`);
});

test('收割后先发完整尾段再结束，并通过 lsof 正向对照证明无句柄', async () => {
  writeFileSync(join(root, 'stop'), '');
  await expect.poll(async () => {
    await core.call('scheduler.tick', {});
    return messages.some(m => m.type === 'log.end');
  }, { timeout: 15000, interval: 50 }).toBe(true);
  expect(text()).toContain('最后一行');
  expect(text()).toContain('finished, exit 0');
  expect(messages.at(-1)).toMatchObject({ type: 'log.end', reason: 'ended', offset: readFileSync(logPath).length });
  expect(Buffer.from(text())).toEqual(readFileSync(logPath));
  let offset = 0;
  for (const chunk of chunks()) { expect(chunk.offset).toBe(offset); offset = chunk.next; }
  await expect.poll(() => running!.logtail.stats()).toEqual(empty);
  const fd = openSync(logPath, 'r');
  try { expect(await handles()).toContain(`n${logPath}`); } finally { closeSync(fd); }
  expect(await handles()).not.toContain(`n${logPath}`);
});

test('拒绝客户端路径，关闭服务释放正在跟随的订阅', async () => {
  messages.length = 0;
  send({ type: 'log.subscribe', session_id: sessionId, path: logPath });
  await expect.poll(() => messages.find(m => m.type === 'log.rejected')).toMatchObject({ code: 'invalid' });
  expect(running!.logtail.stats()).toEqual(empty);
  rmSync(join(root, 'stop')); rmSync(join(root, 'go'));
  const activeId = await startTask();
  const activePath = (await core.call('session.log', { session_id: activeId }) as { path: string }).path;
  send({ type: 'log.subscribe', session_id: activeId, offset: 0 });
  await expect.poll(() => text(), { timeout: 5000 }).toContain('第一行');
  expect(running!.logtail.stats()).toMatchObject({ subscriptions: 1, timer: true });
  // Capture cleanup at bus.close entry: bus disconnect alone would hide a missing logtail.close.
  const service = running!;
  const busClose = service.bus.close;
  let beforeBusClose: ReturnType<typeof service.logtail.stats> | undefined;
  service.bus.close = () => { beforeBusClose = service.logtail.stats(); busClose(); };
  await service.close();
  running = undefined;
  expect(beforeBusClose).toEqual(empty);
  expect(service.logtail.stats()).toEqual(empty);
  expect(await handles()).not.toContain(`n${activePath}`);
});
