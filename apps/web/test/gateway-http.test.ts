import { request } from 'node:http';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { startServer } from '../server/main.ts';
import type { CoreSupervisor } from '../server/core/supervisor.ts';

const binaryPath = process.env.AUTOMED_BIN || resolve(import.meta.dirname, '../../../target/debug/automed');
const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => { for (const cleanup of cleanups.splice(0).reverse()) await cleanup(); });
async function setup(binary = binaryPath) {
  const home = mkdtempSync(join(tmpdir(), 'gateway-http-'));
  cleanups.push(async () => { rmSync(home, { recursive: true, force: true }); });
  const running = await startServer({ port: 0, mode: 'production', home: home, binaryPath: binary });
  let closed = false;
  const close = async () => { if (!closed) { await running.close(); closed = true; } };
  cleanups.push(async () => { await close(); rmSync(home, { recursive: true, force: true }); });
  const address = running.server.address();
  if (!address || typeof address === 'string') throw new Error('无端口');
  const url = `http://127.0.0.1:${address.port}`;
  const post = (channel: string, body: unknown, origin: string | null = url) => fetch(`${url}/api/${channel}`, {
    method: 'POST', headers: { 'Content-Type': 'application/json', ...(origin === null ? {} : { Origin: origin }) }, body: JSON.stringify(body),
  });
  return { ...running, home, close, url, post, core: running.gateway.core as CoreSupervisor };
}
function raw(url: string, headers: Record<string, string>) {
  return new Promise<number>((resolve, reject) => {
    const req = request(url, { headers }, res => { res.resume(); resolve(res.statusCode!); });
    req.setTimeout(5000, () => req.destroy(new Error('请求超时')));
    req.on('error', reject); req.end();
  });
}
test('无凭据 config.get 到达真实内核', async () => {
  const s = await setup(); const response = await s.post('read', { method: 'config.get' });
  expect(response.status).toBe(200); expect((await response.json()).ok).toBe(true);
});
test('无凭据写后读回新值', async () => {
  const s = await setup();
  expect((await s.post('write', { op: 'config.set_loop', params: { parallel: 4 } })).status).toBe(200);
  const body = await (await s.post('read', { method: 'config.get' })).json();
  expect(body.payload.resolved.loop_defaults.parallel).toBe(4);
});
test('缺失及错误 Origin 拒绝写且状态不变', async () => {
  const s = await setup(); const read = async () => (await (await s.post('read', { method: 'config.get' })).json()).payload;
  const before = await read();
  for (const origin of [null, 'http://evil.example']) expect((await s.post('write', { op: 'config.set_loop', params: { parallel: 99 } }, origin)).status).toBe(403);
  expect(await read()).toEqual(before);
});
test('错误 Host 的 GET 拒绝', async () => {
  const s = await setup(); expect(await raw(s.url + '/healthz', { Host: 'evil.example:7420' })).toBe(403);
});
test('所有响应无 Cookie/CORS 且 home 不生成 web 目录', async () => {
  const s = await setup();
  const responses = await Promise.all([
    ...['/healthz', '/'].flatMap(path => [fetch(s.url + path), fetch(s.url + path, { headers: { Origin: 'http://localhost:1' } })]),
    s.post('read', { method: 'config.get' }), s.post('write', { op: 'config.set_loop', params: { parallel: 2 } }),
    s.post('write', {}, null),
  ]);
  expect(responses.map(r => r.status)).toEqual([200, 200, 200, 200, 200, 200, 403]);
  for (const response of responses) {
    expect(response.headers.get('set-cookie')).toBeNull();
    expect([...response.headers.keys()].some(key => key.startsWith('access-control-allow-'))).toBe(false);
    await response.text();
  }
  await s.close(); expect(existsSync(join(s.home, 'web'))).toBe(false);
});
test('不合格 upgrade 回 403', async () => {
  const s = await setup();
  expect(await raw(s.url + '/ws', { Connection: 'Upgrade', Upgrade: 'websocket', Origin: 'http://evil.example' })).toBe(403);
});
test('合格但无处理器 upgrade 回 404', async () => {
  const s = await setup();
  expect(await raw(s.url + '/unregistered-ws', { Connection: 'Upgrade', Upgrade: 'websocket', Origin: s.url })).toBe(404);
});
test('/ws 已注册，缺少握手字段回 400', async () => {
  const s = await setup();
  expect(s.server.listenerCount('upgrade')).toBe(1);
  expect(await raw(s.url + '/ws', { Connection: 'Upgrade', Upgrade: 'websocket', Origin: s.url })).toBe(400);
});
test('close 后内核 pid 不存在', async () => {
  const s = await setup(); await s.post('read', { method: 'config.get' });
  const pid = s.core.sidecar!._child!.pid!; expect(pid).toBeGreaterThan(0);
  await s.close(); expect(() => process.kill(pid, 0)).toThrow(); expect(s.core.status.state).toBe('stopped');
});
test('二进制缺失仍监听，读回 503 中文原因', async () => {
  const s = await setup('/nonexistent/automed');
  await expect.poll(() => s.core.status.state).toBe('restarting');
  const response = await s.post('read', { method: 'config.get' });
  expect(response.status).toBe(503); expect((await response.json()).error).toContain('无法启动内核');
});
test('坏 JSON 回 400 中文原因', async () => {
  const s = await setup();
  const response = await fetch(s.url + '/api/write', { method: 'POST', headers: { Origin: s.url, 'Content-Type': 'application/json' }, body: '{' });
  expect(response.status).toBe(400); expect((await response.json()).error).toMatch(/JSON 无效/);
});
