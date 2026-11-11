import { mkdtempSync, mkdirSync, existsSync, rmSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawn, execFileSync } from 'node:child_process';
import type { ChildProcess } from 'node:child_process';
import { once } from 'node:events';
import WebSocket from 'ws';
import { afterAll, beforeAll, expect, test } from 'vitest';
import type { ServerMessage } from '../server/eventbus-protocol.ts';
const cwd = resolve(import.meta.dirname, '..');
const home = mkdtempSync(join(tmpdir(), 'w3-live-'));
const repo = join(home, 'repo');
const cli = process.env.AUTOME_BIN || resolve(cwd, '../../target/debug/autome');
const binary = process.env.AUTOMED_BIN || resolve(cwd, '../../target/debug/automed');
const env = { PATH: '/usr/bin:/bin', HOME: home, AUTOME_HOME: home, AUTOMED_BIN: resolve(cwd, 'test/fixtures/automed-pidfile.sh'), W3_REAL_AUTOMED: binary, AUTOME_WEB_PORT: '0', NODE_ENV: 'development' };
let child: ChildProcess | undefined;
let url: string;
let output = '';
const sockets: WebSocket[] = [];
function command(args: string[]) { return execFileSync(cli, args, { cwd: repo, env, encoding: 'utf8', timeout: 15000 }); }
async function waitFor<T>(get: () => T | undefined, timeout = 5000): Promise<T> {
  const start = performance.now();
  while (performance.now() - start < timeout) { const value = get(); if (value !== undefined) return value; await new Promise(r => setTimeout(r, 20)); }
  throw new Error(`等待超时：${output}`);
}
function connect(path = '/ws', protocols?: string) {
  const ws = new WebSocket(url.replace('http:', 'ws:') + path, protocols, { origin: url }); sockets.push(ws);
  const messages: ServerMessage[] = []; ws.on('message', data => messages.push(JSON.parse(data.toString())));
  return { ws, messages };
}
beforeAll(async () => {
  for (const dir of ['/usr/bin', '/bin']) for (const name of ['claude', 'codex']) expect(existsSync(join(dir, name))).toBe(false);
  expect(existsSync(cli), '请先构建 autome 命令行').toBe(true); expect(existsSync(binary), '请先构建 automed').toBe(true);
  mkdirSync(repo); execFileSync('/usr/bin/git', ['init', '-q', repo], { env });
  execFileSync('/usr/bin/git', ['-c', 'user.name=W3', '-c', 'user.email=w3@example.test', 'commit', '--allow-empty', '-qm', 'init'], { cwd: repo, env });
  command(['project', 'add', repo, '--skip-onboarding']); command(['task', 'new', '事件测试', '-p', repo]);
  expect(command(['rpc', 'task.get', '{"task_id":"T-1"}'])).toContain('failed');
  child = spawn(process.execPath, ['server/start.js'], { cwd, env, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout!.on('data', data => { output += data; }); child.stderr!.on('data', data => { output += data; });
  url = await waitFor(() => output.match(/已启动：(http:\/\/127\.0\.0\.1:\d+)/)?.[1], 30000);
  const deadline = performance.now() + 10000;
  while (true) {
    const probe = connect();
    const hello = await waitFor(() => probe.messages.find(m => m.type === 'hello'));
    probe.ws.close();
    if (hello.type === 'hello' && hello.core === 'connected') break;
    if (performance.now() >= deadline) throw new Error('内核连接超时');
    await new Promise(r => setTimeout(r, 20));
  }
});
afterAll(async () => {
  for (const ws of sockets) ws.terminate();
  if (child && child.exitCode === null && child.signalCode === null) {
    const exited = once(child, 'exit'); child.kill('SIGTERM'); const timer = setTimeout(() => child?.kill('SIGKILL'), 5000);
    try { await exited; } finally { clearTimeout(timer); }
  }
  rmSync(home, { recursive: true, force: true });
});
test('合格 /ws 升级收到 connected hello', async () => {
  const c = connect(); const hello = await waitFor(() => c.messages.find(m => m.type === 'hello'));
  expect(hello).toMatchObject({ type: 'hello', core: 'connected' });
});
test('外站 Origin 升级拒绝为 403', async () => {
  const ws = new WebSocket(url.replace('http:', 'ws:') + '/ws', { origin: 'http://evil.example' }); sockets.push(ws);
  const status = await new Promise<number>((resolve, reject) => { ws.on('unexpected-response', (_req, res) => { res.resume(); resolve(res.statusCode!); ws.terminate(); }); ws.on('error', () => {}); ws.on('open', () => reject(new Error('外站被放行'))); });
  expect(status).toBe(403);
});
test('同一开发服务 HMR 与 /ws 共存', async () => {
  const source = await (await fetch(url + '/@vite/client')).text();
  const token = source.match(/const wsToken = "([^"]+)"/)?.[1]; expect(token).toBeTruthy();
  const hmr = connect('/?token=' + token, 'vite-hmr'); const bus = connect();
  await waitFor(() => hmr.messages.find(m => (m as { type: string }).type === 'connected'));
  expect(await waitFor(() => bus.messages.find(m => m.type === 'hello'))).toMatchObject({ type: 'hello' });
});
test('命令行跨进程 cancel 在一个 tick 周期内送达且不重复', async () => {
  const c = connect(); await waitFor(() => c.messages.find(m => m.type === 'hello'));
  command(['cancel', 'T-1']); const start = performance.now();
  const received = await waitFor(() => c.messages.find(m => m.type === 'event' && m.kind === 'task.updated' && m.subject === 'T-1' && (m.payload as { state?: { state?: string } }).state?.state === 'cancelled'), 3500);
  const elapsed = performance.now() - start; expect(elapsed).toBeLessThanOrEqual(3500); console.log(`跨进程事件延迟：${elapsed.toFixed(0)} ms`);
  await new Promise(r => setTimeout(r, 3500));
  const events = c.messages.filter(m => m.type === 'event'); expect(events.filter(e => e.seq === (received as { seq: number }).seq)).toHaveLength(1);
  expect(events.every((e, i) => i === 0 || e.seq > events[i - 1].seq)).toBe(true);
});
test('断线期间命令行写入，重连 resume 补发一次', async () => {
  command(['task', 'new', '续传测试', '-p', repo]);
  const first = connect(); const hello = await waitFor(() => first.messages.find(m => m.type === 'hello'));
  const checkpoint = (hello as { seq: number }).seq;
  const closed = once(first.ws, 'close'); first.ws.close(); await closed;
  command(['cancel', 'T-2']);
  const c = connect(); await waitFor(() => c.messages.find(m => m.type === 'hello'));
  c.ws.send(JSON.stringify({ type: 'resume', after: checkpoint }));
  const received = await waitFor(() => c.messages.find(m => m.type === 'event' && m.subject === 'T-2' && m.kind === 'task.updated' && (m.payload as { state?: { state?: string } }).state?.state === 'cancelled'));
  await new Promise(r => setTimeout(r, 3500));
  const events = c.messages.filter(m => m.type === 'event');
  expect(events.filter(e => e.seq === (received as { seq: number }).seq)).toHaveLength(1);
  expect(events.every((e, i) => i === 0 || e.seq > events[i - 1].seq)).toBe(true);
});
test('杀内核推送 offline 到 connected，离线写入恢复补齐', async () => {
  command(['task', 'new', '恢复测试', '-p', repo]);
  const clients = [connect(), connect()];
  for (const c of clients) await waitFor(() => c.messages.find(m => m.type === 'hello'));
  process.kill(Number(readFileSync(join(home, 'automed.pid'), 'utf8').trim()), 'SIGKILL');
  for (const c of clients) {
    const offline = await waitFor(() => c.messages.find(m => m.type === 'core' && m.status === 'offline'));
    expect(offline).toMatchObject({ reason: expect.stringContaining('内核已退出') });
  }
  command(['cancel', 'T-3']);
  for (const c of clients) {
    await waitFor(() => c.messages.find(m => m.type === 'core' && m.status === 'connected'));
    const received = await waitFor(() => c.messages.find(m => m.type === 'event' && m.subject === 'T-3' && m.kind === 'task.updated' && (m.payload as { state?: { state?: string } }).state?.state === 'cancelled'));
    const events = c.messages.filter(m => m.type === 'event');
    expect(events.filter(e => e.seq === (received as { seq: number }).seq)).toHaveLength(1);
    expect(events.every((e, i) => i === 0 || e.seq > events[i - 1].seq)).toBe(true);
  }
});
test('SIGTERM 关闭客户端且服务五秒内退出', async () => {
  const c = connect(); await waitFor(() => c.messages.find(m => m.type === 'hello'));
  const closed = once(c.ws, 'close'); const exited = once(child!, 'exit'); child!.kill('SIGTERM');
  expect((await closed)[0]).toBe(1001);
  await waitFor(() => child!.exitCode !== null || child!.signalCode !== null ? true : undefined, 5000); await exited;
});
