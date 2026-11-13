import { request } from 'node:http';
import { mkdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { startServer } from '../server/main.ts';
import { w9Kernel } from './fixtures/w9-replies.ts';
const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of cleanups.splice(0).reverse()) await close(); });
async function setup() {
  const fixture = await w9Kernel(); cleanups.push(fixture.close); await fixture.core.stop();
  const running = await startServer({ port: 0, mode: 'development', home: fixture.automeHome, userHome: fixture.userHome }); cleanups.push(running.close);
  const address = running.server.address(); if (!address || typeof address === 'string') throw new Error('没有端口');
  const url = `http://127.0.0.1:${address.port}`;
  const post = (path: string, body: unknown, headers: Record<string, string> = {}) => new Promise<{ status: number; body: string | { ok: boolean; payload?: unknown; error?: string } }>((resolve, reject) => {
    const req = request(url + path, { method: 'POST', headers: { Origin: url, 'Content-Type': 'application/json', ...headers } }, res => {
      let text = ''; res.setEncoding('utf8'); res.on('data', chunk => { text += chunk; }); res.on('end', () => {
        try {
          const isJson = res.headers['content-type']?.split(';')[0].trim() === 'application/json';
          resolve({ status: res.statusCode!, body: isJson ? JSON.parse(text) : text });
        } catch (error) { reject(error); }
      });
    }); req.setTimeout(10000, () => req.destroy(new Error('请求超时'))); req.on('error', reject); req.end(JSON.stringify(body));
  });
  return { ...fixture, ...running, post };
}
test('目录补全合法头、Host/Origin 拒绝与输入形状', async () => {
  const s = await setup(); mkdirSync(join(s.userHome, 'code'));
  expect(await s.post('/api/paths', { input: '~/' })).toMatchObject({ status: 200, body: { payload: { entries: [{ name: 'code', path: '~/code' }] } } });
  for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }] as Record<string, string>[]) expect((await s.post('/api/paths', { input: '~/' }, headers)).status).toBe(403);
  for (const input of [{}, { input: 1 }, { input: '~/', extra: true }, { input: '~/a..b' }]) expect((await s.post('/api/paths', input)).status).toBe(400);
});
test('confirm 坏头或外部路径均不注册', async () => {
  const s = await setup(); s.repo('single'); const before = await s.gateway.core.call('project.list', {});
  for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }] as Record<string, string>[]) expect((await s.post('/api/write', { op: 'project.confirm', params: { path: '~/single' } }, headers)).status).toBe(403);
  expect((await s.post('/api/write', { op: 'project.confirm', params: { path: s.root } })).status).toBe(400);
  expect(await s.gateway.core.call('project.list', {})).toEqual(before);
});
test('remove 坏头保留项目，合法头能够取消注册且文件仍在', async () => {
  const s = await setup(); s.repo('single');
  const added = await s.post('/api/write', { op: 'project.confirm', params: { path: '~/single' } }); expect(added.status).toBe(200);
  if (typeof added.body === 'string') throw new Error('添加项目应返回 JSON');
  const project = (added.body.payload as { project: { id: string } }).project;
  const before = await s.gateway.core.call('project.list', {});
  for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }] as Record<string, string>[]) expect((await s.post('/api/write', { op: 'project.remove', params: { project_id: project.id } }, headers)).status).toBe(403);
  expect(await s.gateway.core.call('project.list', {})).toEqual(before);
  expect((await s.post('/api/write', { op: 'project.remove', params: { project_id: project.id } })).status).toBe(200);
  expect(await s.gateway.core.call('project.list', {})).toMatchObject({ projects: [] });
  expect(existsSync(join(s.userHome, 'single', '.git'))).toBe(true);
});
test('添加页 action 与 single fetch 坏 Host/Origin 拒绝且无注册', async () => {
  const s = await setup(); s.repo('single'); const before = await s.gateway.core.call('project.list', {});
  for (const path of ['/projects/new', '/projects/new.data']) for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }] as Record<string, string>[]) {
    expect((await s.post(path, { intent: 'confirm', path: '~/single' }, headers)).status).toBe(403);
  }
  expect(await s.gateway.core.call('project.list', {})).toEqual(before);
});
