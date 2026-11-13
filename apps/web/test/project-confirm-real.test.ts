import { readFileSync, existsSync, mkdirSync, realpathSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, expect, test } from 'vitest';
import { w9Kernel, captureReplies } from './fixtures/w9-replies.ts';
const replies = JSON.parse(readFileSync(new URL('./fixtures/w9-replies.json', import.meta.url), 'utf8'));
const cleanup: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of cleanup.splice(0).reverse()) await close(); });
async function setup() { const s = await w9Kernel(); cleanup.push(s.close); return s; }
test('单仓库 probe → confirm → list，step 3', async () => {
  const s = await setup(); const path = s.repo('single');
  expect(await s.gateway.read({ method: 'project.probe', params: { path: '~/single' } })).toMatchObject({ status: 200, body: { payload: { is_repo_root: true } } });
  const r = await s.gateway.write({ op: 'project.confirm', params: { path: '~/single' } });
  expect(r).toMatchObject({ status: 200, body: { payload: { project: { path: realpathSync(path), onboarding: { onboarding: 'in_progress', step: 3 } } } } });
  expect(await s.core.call('project.list', {})).toMatchObject({ projects: [expect.objectContaining({ project: expect.objectContaining({ path: realpathSync(path) }) })] });
});
test('三个成员工作区 probe 与 confirm', async () => {
  const s = await setup(); for (const name of ['app', 'docs', 'lib']) s.repo('workspace/' + name);
  const probe = await s.gateway.read({ method: 'project.probe', params: { path: '~/workspace' } });
  expect(probe).toMatchObject({ status: 200, body: { payload: { suggest_workspace: true, members: expect.any(Array) } } });
  expect((probe.body.payload as { members: unknown[] }).members).toHaveLength(3);
  expect(await s.gateway.write({ op: 'project.confirm', params: { path: '~/workspace', workspace: true, docs_repo: 'docs' } })).toMatchObject({ status: 200, body: { payload: { project: { kind: 'workspace', docs_repo: 'docs' } } } });
});
test('重复 confirm 原文 409', async () => {
  const s = await setup(); s.repo('single'); const request = { op: 'project.confirm', params: { path: '~/single' } };
  expect((await s.gateway.write(request)).status).toBe(200);
  expect(await s.gateway.write(request)).toMatchObject({ status: 409, body: { error: '该目录已经是一个项目' } });
  const list = await s.core.call('project.list', {}) as { projects: { project: { id: string } }[] };
  expect((await s.gateway.write({ op: 'project.remove', params: { project_id: list.projects[0].project.id } })).status).toBe(200);
  expect(await s.core.call('project.list', {})).toMatchObject({ projects: [] });
  expect(existsSync(join(s.userHome, 'single', '.git'))).toBe(true);
  expect((await s.gateway.write(request)).status).toBe(200);
});
test('外部目录拒绝，注册与文件无变化', async () => {
  const s = await setup(), outside = join(s.root, 'outside'); mkdirSync(outside);
  const before = await s.core.call('project.list', {});
  expect((await s.gateway.write({ op: 'project.confirm', params: { path: outside } })).status).toBe(400);
  expect(await s.core.call('project.list', {})).toEqual(before); expect(existsSync(join(outside, '.autome'))).toBe(false);
});
test('fixture 的 probe/add/skip 与真实回包逐层键集合一致', async () => {
  const actual = await captureReplies();
  function shape(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(shape);
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => [k, shape(v)]));
    return value === null ? null : typeof value;
  }
  expect(shape(actual)).toEqual(shape(replies));
});
