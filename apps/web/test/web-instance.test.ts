import { test, expect, afterEach } from 'vitest';
import { mkdtemp, readFile, rm, stat, writeFile, utimes } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { request } from 'node:http';
import { acquireLaunchLock, judgeInstance, readInstance, removeInstance, writeInstance, type Instance } from '../server/instance.ts';
import { harness, occupiedPort } from './fixtures/web-launcher.ts';
const roots: string[] = [];
const harnesses: ReturnType<typeof harness>[] = [];
const value: Instance = { port: 7420, pid: process.pid, instance: 'test-instance', url: 'http://127.0.0.1:7420', started_at: new Date().toISOString(), web_root: '/panel', kernel: { path: '/core', source: 'env', build_id: 'build' } };
async function file() { const root = await mkdtemp(join(tmpdir(), 'web-instance-')); roots.push(root); return join(root, 'server.json'); }
function live() { const h = harness(); harnesses.push(h); return h; }
afterEach(async () => { for (const h of harnesses.splice(0)) await h.cleanup(); for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true }); });
test('原子排他写入、单行、0600 与全部字段', async () => {
  const f = await file(); await writeInstance(f, value);
  expect(await readInstance(f)).toEqual(value); expect((await stat(f)).mode & 0o777).toBe(0o600);
  expect((await readFile(f, 'utf8')).trim().split('\n')).toHaveLength(1);
  await expect(writeInstance(f, { ...value, instance: 'second' })).rejects.toMatchObject({ code: 'EEXIST' });
  expect(await readInstance(f)).toEqual(value);
});
test('只删自己的文件', async () => { const f = await file(); await writeInstance(f, value); await removeInstance(f, 'other'); expect(await readInstance(f)).toEqual(value); await removeInstance(f, value.instance); expect(await readInstance(f)).toBeNull(); });
test('pid 已死即陈旧', async () => { expect(await judgeInstance(value, { alive: () => false })).toBe('stale'); });
test('端口无监听即陈旧', async () => { expect(await judgeInstance(value, { alive: () => true, health: async () => { throw Object.assign(Error(), { code: 'ECONNREFUSED' }); } })).toBe('stale'); });
test('端口换主人时 instance 或 pid 不符均陈旧', async () => { for (const reply of [{ ok: true, pid: value.pid, instance: 'other' }, { ok: true, pid: value.pid + 1, instance: value.instance }]) expect(await judgeInstance(value, { alive: () => true, health: async () => reply })).toBe('stale'); });
test('缺省端口被占用时换到下一个', async () => { const h = live(); const held = await occupiedPort(); try { const result = await h.run(['start'], { AUTOME_WEB_DEFAULT_PORT: String(held.port) }); expect(result.code, result.stderr).toBe(0); expect((await h.registration()).port).toBe(held.port + 1); } finally { await held.close(); } });
test('显式端口被占用时报错且退出码 1', async () => { const h = live(); const held = await occupiedPort(); try { const result = await h.run(['serve', '--port', String(held.port)]); expect(result.code).toBe(1); expect(result.stderr).toContain(`端口 ${held.port} 已被占用`); expect(await readInstance(h.file)).toBeNull(); } finally { await held.close(); } });
test('healthz 匹配 panel-url 正则且包含实例身份', async () => { const h = live(); expect((await h.run(['start'])).code).toBe(0); const v = await h.registration(); const body = await (await fetch(v.url + '/healthz')).text(); expect(body).toMatch(/"ok"\s*:\s*true\s*[,}]/); expect(body.trim().split('\n')).toHaveLength(1); expect(JSON.parse(body)).toMatchObject({ instance: v.instance, pid: v.pid }); });
test('/api/instance 拒绝错误 Host 与 Origin', async () => {
  const h = live(); expect((await h.run(['start'])).code).toBe(0); const v = await h.registration();
  for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }]) {
    const status = await new Promise<number | undefined>((resolve, reject) => { const req = request(v.url + '/api/instance', { headers, method: 'Origin' in headers ? 'POST' : 'GET' }, res => { res.resume(); resolve(res.statusCode); }); req.on('error', reject); req.end(); }); expect(status).toBe(403);
  }
  expect((await fetch(v.url + '/api/instance')).status).toBe(200);
});
test('已有活实例 serve 以 0 退出', async () => { const h = live(); expect((await h.run(['start'])).code).toBe(0); const old = await readFile(h.file, 'utf8'); const result = await h.run(['serve']); expect(result.code).toBe(0); expect(result.stdout).toContain('已有面板'); expect(await readFile(h.file, 'utf8')).toBe(old); });
test('锁再旧也不接管活持有者', async () => { const f = await file(); await writeFile(f, JSON.stringify({ pid: process.pid, token: 'owner' })); await utimes(f, 0, 0); await expect(acquireLaunchLock(f, { timeout: 0 })).rejects.toThrow('另一个面板正在启动'); expect(JSON.parse(await readFile(f, 'utf8')).token).toBe('owner'); });
test('死锁持有者可被接管，释放只删自己的 token', async () => { const f = await file(); await writeFile(f, JSON.stringify({ pid: 987654, token: 'dead' })); const release = await acquireLaunchLock(f, { alive: pid => pid !== 987654 }); expect(JSON.parse(await readFile(f, 'utf8')).pid).toBe(process.pid); await release!(); await expect(stat(f)).rejects.toMatchObject({ code: 'ENOENT' }); });
test('三态判定表逐行，包括 ps 不可读与错误健康响应', async () => {
  for (const [alive, reply, command, expected] of [
    [false, 'timeout', 'autome-web', 'stale'], [true, 'refused', 'autome-web', 'stale'], [true, 'mismatch', 'autome-web', 'stale'],
    [true, 'timeout', 'autome-web serve', 'unresponsive'], [true, 'timeout', 'unrelated', 'stale'], [true, 'timeout', null, 'unresponsive'],
    [true, 'healthy', 'autome-web', 'live'], [true, 'bad', 'autome-web', 'stale'],
  ] as const) {
    expect(await judgeInstance(value, { alive: () => alive, health: async () => {
      if (reply === 'timeout' || reply === 'refused') throw Object.assign(Error(), { code: reply === 'refused' ? 'ECONNREFUSED' : 'ETIMEDOUT' });
      return { ok: reply !== 'bad', instance: reply === 'mismatch' ? 'other' : value.instance, pid: value.pid };
    }, command: async () => command }), `${alive}/${reply}/${command}`).toBe(expected);
  }
});
test('SIGSTOP 时新 serve 不接管，SIGCONT 后原实例恢复', async () => {
  const h = live(); expect((await h.run(['start'])).code).toBe(0); const v = await h.registration(); const old = await readFile(h.file, 'utf8');
  try { process.kill(v.pid, 'SIGSTOP'); const result = await h.run(['serve']); expect(result.code).toBe(0); expect(result.stdout).toContain('没有响应'); expect(await readFile(h.file, 'utf8')).toBe(old); expect(h.servers()).toHaveLength(1); }
  finally { process.kill(v.pid, 'SIGCONT'); }
  await expect.poll(() => judgeInstance(v)).toBe('live'); expect(await readFile(h.file, 'utf8')).toBe(old);
});
