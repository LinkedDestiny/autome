import { budgetTask, tickAlerts, withBudgetCli } from './fixtures/w9-alerts.ts';
import { request } from 'node:http';
import { mkdirSync, existsSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { startServer } from '../server/main.ts';
import { w9Kernel } from './fixtures/w9-replies.ts';
const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of cleanups.splice(0).reverse()) await close(); });
async function setup(terminal?: (command: string) => Promise<void>) {
  const fixture = await w9Kernel(); cleanups.push(fixture.close); await fixture.core.stop();
  // Keep the fixture's kernel when reopening its ledger, independent of the installed CLI on PATH.
  const running = await withBudgetCli(fixture.root, () => startServer({ port: 0, mode: 'development', home: fixture.automeHome, userHome: fixture.userHome, binaryPath: fixture.binaryPath, terminal })); cleanups.push(running.close);
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
    }); req.setTimeout(10000, () => req.destroy(new Error('请求超时'))); req.on('error', reject); req.end(typeof body === 'string' ? body : JSON.stringify(body));
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
test('设置 action 与 single fetch 安全头和超长保存经网关拒绝', async () => {
  const s = await setup(); s.repo('single'); const added = await s.gateway.core.call('project.add', { path: join(s.userHome, 'single') }) as { project: { id: string } }; const id = added.project.id;
  const before = await s.gateway.core.call('project.onboarding.artefacts', { project_id: id });
  for (const suffix of ['', '.data']) {
    const path = `/projects/${id}/settings${suffix}`;
    for (const intent of ['run', 'save', 'finish', 'skip']) for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }] as Record<string, string>[]) expect((await s.post(path, { intent }, headers)).status).toBe(403);
    const result = await s.post(path, new URLSearchParams({ intent: 'save', path: 'AGENTS.md', content: 'x'.repeat(262145) }).toString(), { 'Content-Type': 'application/x-www-form-urlencoded' });
    expect(result.status).toBe(200); expect(String(result.body)).toContain('已拒绝：'); expect(String(result.body)).toContain('字符上限');
  }
  expect(await s.gateway.core.call('project.onboarding.artefacts', { project_id: id })).toEqual(before);
});
test('设置页移除 action 和 single fetch 坏 Host/Origin 保留项目', async () => {
  const s = await setup(); const path = s.repo('single');
  const { project } = await s.gateway.core.call('project.add', { path }) as { project: { id: string } };
  const before = await s.gateway.core.call('project.list', {});
  for (const suffix of ['', '.data']) for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }] as Record<string, string>[]) {
    expect((await s.post(`/projects/${project.id}/settings${suffix}`, new URLSearchParams({ intent: 'remove', name: 'single' }).toString(), { 'Content-Type': 'application/x-www-form-urlencoded', ...headers })).status).toBe(403);
  }
  expect(await s.gateway.core.call('project.list', {})).toEqual(before);
});

test('报警 action 与 single fetch 坏 Host/Origin 保留报警，合法请求消除', async () => {
  const s = await setup(); const path = s.repo('alerts'); const core = s.gateway.core;
  const { project } = await core.call('project.add', { path }) as { project: { id: string } };
  await budgetTask(core, path, project.id);
  let alerts: Awaited<ReturnType<typeof tickAlerts>> = [];
  await vi.waitFor(async () => { alerts = await tickAlerts(core); expect(alerts).toHaveLength(1); }, { timeout: 20000 });
  const body = new URLSearchParams({ intent: 'dismiss', alert_id: alerts[0].id }).toString();
  for (const suffix of ['', '.data']) for (const headers of [{ Host: 'evil.example' }, { Origin: 'http://evil.example' }] as Record<string, string>[]) {
    expect((await s.post('/projects/' + project.id + suffix, body, { 'Content-Type': 'application/x-www-form-urlencoded', ...headers })).status).toBe(403);
    expect((await core.call('dashboard.get', {}) as { alerts: unknown }).alerts).toEqual(alerts);
  }
  expect((await s.post('/projects/' + project.id + '.data', body, { 'Content-Type': 'application/x-www-form-urlencoded' })).status).toBe(200);
  expect((await core.call('dashboard.get', {}) as { alerts: unknown[] }).alerts).toEqual([]);
});
test('环境安装与登录坏 Host/Origin 不启动终端，合法请求执行',async()=>{
  const terminal=vi.fn<(command: string) => Promise<void>>(async()=>{}),s=await setup(terminal);
  for(const op of ['env.install','env.login']) for(const headers of [{Host:'evil.example'},{Origin:'http://evil.example'}] as Record<string,string>[]) expect((await s.post('/api/write',{op,params:{component:'codex'}},headers)).status).toBe(403);
  expect(terminal).not.toHaveBeenCalled();expect((await s.post('/api/write',{op:'env.login',params:{component:'codex'}})).status).toBe(200);expect(terminal).toHaveBeenCalledWith('codex login');
});
test('环境 action 与 single fetch 坏 Host/Origin 拒绝，合法检测可达',async()=>{
  const terminal=vi.fn<(command: string) => Promise<void>>(async()=>{}),s=await setup(terminal);
  for(const suffix of ['', '.data']) for(const intent of ['detect','install','login']) for(const headers of [{Host:'evil.example'},{Origin:'http://evil.example'}] as Record<string,string>[]) expect((await s.post('/env'+suffix,new URLSearchParams({intent,component:'codex'}).toString(),{'Content-Type':'application/x-www-form-urlencoded',...headers})).status).toBe(403);
  expect(terminal).not.toHaveBeenCalled();expect((await s.post('/env.data','intent=detect',{'Content-Type':'application/x-www-form-urlencoded'})).status).toBe(200);
});
