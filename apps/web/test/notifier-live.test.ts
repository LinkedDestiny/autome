import { afterAll, beforeAll, expect, test } from 'vitest';
import { mkdtempSync, mkdirSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import WebSocket from 'ws';
import { startServer } from '../server/main.ts';
import { openWebStore } from '../server/store.ts';
import { configureFailureRepo } from './fixtures/notifier-failure.ts';
const root = mkdtempSync(join(tmpdir(), 'ntf-live-')), home = join(root, 'home'), repo = join(root, 'repo');
const wt = resolve(import.meta.dirname, '../../..'), fake = join(root, 'fake-cli');
const wrapper = join(root, 'automed-wrapper');
let running: Awaited<ReturnType<typeof startServer>> | undefined, url: string;
const clients: { ws: WebSocket; messages: Record<string, unknown>[] }[] = [];
const env = () => ({ ...process.env, HOME: home, AUTOME_HOME: home, AUTOMED_CLAUDE_BINARY: fake, AUTOMED_CODEX_BINARY: fake });
const command = (file: string, args: string[]) => promisify(execFile)(file, args, { cwd: repo, env: env(), timeout: 15000 });
const cli = (...args: string[]) => command(join(wt, 'target/debug/autome'), args);
async function until(check: () => boolean | Promise<boolean>) { const end = Date.now() + 15000; while (Date.now() < end) { if (await check()) return; await new Promise(r => setTimeout(r, 20)); } throw new Error('通知验收等待超时'); }
// Exercise the deployed notification service without Vite's watcher/HMR lifecycle.
// Development SSR and HMR remain covered by server.test.ts and shell-live.test.ts.
async function start() { running = await startServer({ port: 0, mode: 'production', home, binaryPath: wrapper }); const address = running.server.address(); if (!address || typeof address === 'string') throw new Error('无监听地址'); url = `http://127.0.0.1:${address.port}`; }
async function list() { return (await (await fetch(url + '/api/notifications')).json()).payload as { unread: number; items: { id: string; title: string; read_at: string | null }[] }; }
async function connect() { const ws = new WebSocket(url.replace('http:', 'ws:') + '/ws', { origin: url }); const client = { ws, messages: [] as Record<string, unknown>[] }; ws.on('message', data => client.messages.push(JSON.parse(data.toString()))); clients.push(client); await until(() => client.messages.some(m => m.type === 'hello')); return client; }
async function fail(title: string) { await cli('task', 'new', title, '-p', repo); await cli('run', '--until-idle', '--interval', '0.2'); }
beforeAll(async () => {
  // Build explicitly so this suite also runs alone, before any other build test.
  await promisify(execFile)('npm', ['run', 'build'], { cwd: resolve(wt, 'apps/web'), timeout: 60000 });
}, 60000);
beforeAll(async () => {
  mkdirSync(repo); mkdirSync(home); writeFileSync(fake, '#!/bin/sh\ncat >/dev/null\nexit 3\n', { mode: 0o755 });
  configureFailureRepo(repo);
  writeFileSync(wrapper, '#!/bin/sh\nexport HOME="' + home + '"\nexport AUTOMED_CLAUDE_BINARY="' + fake + '" AUTOMED_CODEX_BINARY="' + fake + '"\nexec "' + join(wt, 'target/debug/automed') + '" "$@"\n', { mode: 0o755 });
  await command('git', ['init','-q','-b','main']); await command('git', ['-c','user.name=test','-c','user.email=t@t','commit','--allow-empty','-qm','init']);
  await cli('project','add',repo,'--skip-onboarding'); await start();
  const store = openWebStore(home); try { await until(() => store.getCursor('notifier') !== undefined); } finally { store.close(); }
}, 30000);
afterAll(async () => { for (const c of clients) c.ws.terminate(); await running?.close(); rmSync(root, { recursive: true, force: true }); });
test('建面板库，真实失败事件向两个连接推送严格五键消息', async () => {
  expect(existsSync(join(home,'state/web.sqlite3'))).toBe(true); await connect(); await connect(); await fail('first fail');
  for (const c of clients) { await until(() => c.messages.some(m => m.type === 'notification')); const notifications = c.messages.filter(m => m.type === 'notification'); expect(notifications).toHaveLength(1); expect(notifications[0]).toEqual({ type: 'notification', id: expect.stringMatching(/^ntf-/), kind: 'failed', task_id: 'T-1', title: 'T-1 停在失败上：会话异常退出（会话退出码 3）' }); }
}, 30000);
test('带 Origin 已读跨连接广播，无 Origin 403 且库不变', async () => {
  const before = await list(); expect(before.unread).toBe(1); const body = JSON.stringify({ ids: [before.items[0].id] });
  const denied = await fetch(url + '/api/notifications/read', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body }); expect(denied.status).toBe(403); expect(await list()).toEqual(before);
  const accepted = await fetch(url + '/api/notifications/read', { method: 'POST', headers: { 'Content-Type': 'application/json', Origin: url }, body }); expect(accepted.status).toBe(200);
  for (const c of clients) { await until(() => c.messages.some(m => m.type === 'notification-read')); expect(c.messages.find(m => m.type === 'notification-read')).toEqual({ type: 'notification-read', ids: [before.items[0].id] }); }
  expect((await list()).unread).toBe(0);
});
test('停服期间新增失败，重启只补一条且保留旧已读', async () => {
  // Keep the original deadline and assertions; identify the stalled phase in host gate logs.
  async function step<T>(name: string, action: () => Promise<T>): Promise<T> {
    const started = performance.now();
    process.stdout.write(`[notifier restart] ${name}: started\n`);
    try {
      const result = await action();
      process.stdout.write(`[notifier restart] ${name}: passed (${Math.ceil(performance.now() - started)} ms)\n`);
      return result;
    } catch (error) {
      process.stdout.write(`[notifier restart] ${name}: failed (${Math.ceil(performance.now() - started)} ms)\n`);
      throw error;
    }
  }
  const first = (await step('read existing notification', list)).items[0];
  for (const c of clients) c.ws.terminate();
  await step('close server', () => running!.close()); running = undefined;
  await step('create offline task', () => cli('task', 'new', 'second fail', '-p', repo));
  await step('run offline task', () => cli('run', '--until-idle', '--interval', '0.2'));
  await step('restart server', start);
  await step('replay notification', () => until(async () => (await list()).items.length === 2));
  const result = await list(); expect(result.unread).toBe(1); expect(result.items[1]).toEqual(first); expect(result.items[0].title).toBe('T-2 停在失败上：会话异常退出（会话退出码 3）');
}, 30000);
