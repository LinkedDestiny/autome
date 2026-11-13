import { execFileSync } from 'node:child_process';
import { afterEach, expect, test } from 'vitest';
import { RouterContextProvider } from 'react-router';
import { w9Kernel } from './fixtures/w9-replies.ts';
import { servicesContext } from '../app/load-context.ts';
import { loader, action } from '../app/routes/projects.new.tsx';
import { loader as board } from '../app/routes/projects.$id.tsx';
import { probeView } from '../app/components/add-project-model.ts';
const closes: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of closes.splice(0).reverse()) await close(); });
async function setup() {
  const s = await w9Kernel(); closes.push(s.close); const context = new RouterContextProvider(); context.set(servicesContext, s);
  const check = (path: string) => loader({ request: new Request(`http://localhost/projects/new?path=${encodeURIComponent(path)}`), url: new URL("http://localhost/projects/new"), pattern: "/projects/new", params: {}, context });
  const confirm = (path: string, extra = {}) => action({ request: new Request('http://localhost/projects/new', { method: 'POST', body: new URLSearchParams({ intent: 'confirm', path, ...extra }) }), url: new URL("http://localhost/projects/new"), pattern: "/projects/new", params: {}, context });
  return { ...s, context, check, confirm };
}
test('单仓库检查、添加、跳转设置与注册', async () => {
  const s = await setup(); s.repo('single'); expect((await s.check('~/single')).probe?.is_repo_root).toBe(true);
  for (const input of ['', '../x', '/outside', '~/a..b', '~/bad\0', '~/' + 'a'.repeat(511)]) expect(await s.check(input)).toHaveProperty('error');
  expect(s.admin.paths.check('~/' + 'a/'.repeat(254) + 'aa').ok).toBe(true);
  expect((await s.check('~/missing')).probe?.exists).toBe(false);
  const response = await s.confirm('~/single') as Response;
  const list = await s.core.call('project.list', {}) as { projects: { project: { id: string } }[] }; expect(list.projects).toHaveLength(1);
  expect(response.headers.get('Location')).toBe(`/projects/${list.projects[0].project.id}/settings`);
});
test('工作区成员建议、docs 选择与项目看板', async () => {
  const s = await setup(); for (const name of ['app', 'docs', 'lib']) s.repo(`workspace/${name}`);
  const checked = await s.check('~/workspace'); expect(probeView(checked.probe!).join()).toContain('看起来是工作区');
  expect(checked.probe!.members.map(m => m.name)).toEqual(['app', 'docs', 'lib']); expect(checked.probe!.suggested_docs_repo).toBe('docs');
  const response = await s.confirm('~/workspace', { workspace: 'on', docs_repo: 'docs' }) as Response;
  const id = response.headers.get('Location')!.split('/')[2];
  const data = await board({ request: new Request('http://localhost/projects/' + id), url: new URL("http://localhost/projects/" + id), pattern: "/projects/:id", params: { id }, context: s.context }); expect(data).not.toHaveProperty('error');
});
test('跳过 Onboarding 与 init commit', async () => {
  const s = await setup(); const path = s.repo('single'); const response = await s.confirm('~/single', { skip_onboarding: 'on' }) as Response;
  const list = await s.core.call('project.list', {}) as { projects: { project: { id: string; onboarding: unknown } }[] };
  expect(response.headers.get('Location')).toBe(`/projects/${list.projects[0].project.id}`); expect(list.projects[0].project.onboarding).toEqual({ onboarding: 'skipped' });
  expect(execFileSync('git', ['log', '-1', '--format=%s'], { cwd: path, encoding: 'utf8' }).trim()).toBe('chore(autome): initialise project');
});
test('重复目录保留内核拒绝原文', async () => { const s = await setup(); s.repo('single'); await s.confirm('~/single'); expect(await s.confirm('~/single')).toEqual({ error: '该目录已经是一个项目' }); });
