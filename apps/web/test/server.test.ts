import { spawn } from 'node:child_process';
import type { ChildProcess } from 'node:child_process';
import { createServer } from 'node:net';
import { once } from 'node:events';
import { fileURLToPath } from 'node:url';
import { afterAll, beforeAll, expect, test } from 'vitest';
import { startServer } from '../server/main.ts';

const cwd = fileURLToPath(new URL('..', import.meta.url));
const children = new Set<ChildProcess>();
function launch(args: string[], env: NodeJS.ProcessEnv = {}) {
  const child = spawn('npm', args, {
    cwd, detached: true, env: { ...process.env, NODE_ENV: 'development', AUTOME_WEB_PORT: '0', ...env },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  children.add(child);
  let output = '';
  let stderr = '';
  child.stdout!.on('data', chunk => { output += chunk; });
  child.stderr!.on('data', chunk => { stderr += chunk; });
  const exited = new Promise<number | null>((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', resolve);
  });
  return { child, exited, output: () => output, stderr: () => stderr };
}
async function stop(child: ChildProcess) {
  if (!child.pid) return;
  const exited = child.exitCode !== null || child.signalCode !== null ? Promise.resolve() : once(child, 'exit');
  try { process.kill(-child.pid, 'SIGTERM'); } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error;
  }
  const timer = setTimeout(() => {
    try { process.kill(-child.pid!, 'SIGKILL'); } catch { /* already exited */ }
  }, 5000);
  try { await exited; } finally { clearTimeout(timer); children.delete(child); }
}
async function ready(args: string[]) {
  const running = launch(args);
  const url = await new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`启动超时：${running.stderr()}`)), 30_000);
    running.child.stdout!.on('data', () => {
      const match = running.output().match(/已启动：(http:\/\/127\.0\.0\.1:\d+)/);
      if (match) { clearTimeout(timer); resolve(match[1]); }
    });
    running.exited.then(code => { clearTimeout(timer); reject(new Error(`提前退出 ${code}: ${running.stderr()}`)); }, reject);
  });
  return { ...running, url };
}
let dev: Awaited<ReturnType<typeof ready>>;
let prod: Awaited<ReturnType<typeof ready>>;
beforeAll(async () => {
  const build = launch(['run', 'build']);
  expect(await build.exited, build.stderr()).toBe(0);
  dev = await ready(['run', 'dev']);
  prod = await ready(['start']);
});
afterAll(async () => { await Promise.all([...children].map(stop)); });

test('dev /healthz 返回 200 与 ok', async () => {
  const response = await fetch(`${dev.url}/healthz`);
  expect(response.status).toBe(200);
  expect(await response.json()).toEqual({ ok: true });
});
test('dev 首页 SSR 与模块脚本', async () => {
  const response = await fetch(dev.url);
  expect(response.status).toBe(200);
  const html = await response.text();
  expect(html).toContain('Autome 面板');
  expect(html).toContain('面板骨架已就绪');
  expect(html).toContain('<script type="module"');
});
test('prod /healthz 返回 200 与 ok', async () => {
  const response = await fetch(`${prod.url}/healthz`);
  expect(response.status).toBe(200);
  expect(await response.json()).toEqual({ ok: true });
});
test('prod 首页与引用的 JS 资源可访问', async () => {
  const response = await fetch(prod.url);
  expect(response.status).toBe(200);
  const html = await response.text();
  expect(html).toContain('面板骨架已就绪');
  const asset = html.match(/(?:src|href)="(\/assets\/[^"\s]+\.js)"/);
  expect(asset).not.toBeNull();
  const script = await fetch(new URL(asset![1], prod.url));
  expect(script.status).toBe(200);
  expect(script.headers.get('content-type')).toContain('javascript');
});
test('prod 未知路径返回 404', async () => {
  expect((await fetch(`${prod.url}/not-found`)).status).toBe(404);
});
test('只监听 IPv4 回环地址', async () => {
  const running = await startServer({ port: 0, mode: 'production' });
  try { expect(running.server.address()).toMatchObject({ address: '127.0.0.1', family: 'IPv4' }); }
  finally { await running.close(); }
});
test('端口被占时非零退出并提示中文', async () => {
  const occupied = createServer();
  occupied.listen(0, '127.0.0.1');
  await once(occupied, 'listening');
  try {
    const address = occupied.address();
    if (!address || typeof address === 'string') throw new Error('未获得端口');
    const failed = launch(['start'], { AUTOME_WEB_PORT: String(address.port) });
    expect(await failed.exited).not.toBe(0);
    expect(failed.stderr()).toContain('已被占用');
    expect(failed.output()).not.toContain('已启动');
  } finally { await new Promise<void>(resolve => occupied.close(() => resolve())); }
});
