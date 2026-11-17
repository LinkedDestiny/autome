import { createElement as h } from 'react';
import { renderToString } from 'react-dom/server';
import { SkillCard } from '../app/routes/skills.tsx';
import { afterEach, expect, test } from 'vitest';
import { mkdirSync, writeFileSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { RouterContextProvider } from 'react-router';
import { w9Kernel } from './fixtures/w9-replies.ts';
import { loadSkills, saveBinding } from '../app/components/skills-data.ts';
import { resolvedRoles } from '../app/components/loop-data.ts';
import { bindingReason } from '../app/lib/skills-model.ts';
import { action } from '../app/routes/skills.tsx';
import { servicesContext } from '../app/load-context.ts';
const closes: (() => Promise<void>)[] = [];
afterEach(async () => { for (const close of closes.splice(0)) await close(); });
async function setup() {
  const s = await w9Kernel(); closes.push(s.close);
  const ids: string[] = [], paths: string[] = [];
  for (const name of ['A', 'B', 'C']) { const path = s.repo(name); paths.push(path); const { project } = await s.core.call('project.add', { path }) as { project: { id: string } }; ids.push(project.id); }
  for (const [root, tool, name] of [[s.userHome, '.claude', 'alpha'], [s.userHome, '.agents', 'alpha'], [s.userHome, '.claude', 'mixed'], [paths[0], '.agents', 'mixed'], [paths[0], '.claude', 'local']]) { const dir = join(root, tool, 'skills', name); mkdirSync(dir, { recursive: true }); writeFileSync(join(dir, 'SKILL.md'), '# Skill'); }
  const bound = async (id?: string) => resolvedRoles((await loadSkills(s, id)).config!).plan.skills;
  return { ...s, ids, paths, bound };
}
test('真实 skills.list 全局与项目来源及切换', async () => { const s = await setup(); expect((await loadSkills(s)).skills.map(x => x.name)).toEqual(['alpha', 'mixed']); expect((await loadSkills(s, s.ids[0])).skills.map(x => x.name)).toEqual(['alpha', 'local', 'mixed']); expect((await loadSkills(s, s.ids[1])).skills.map(x => x.name)).toEqual(['alpha', 'mixed']); });
test('仅当前项目只写 A，B C 与全局均不变', async () => { const s = await setup(); expect(await saveBinding(s, s.ids[0], 'project', 'alpha', ['plan'])).toEqual({ saved: ['plan'], messages: [] }); expect(await s.bound(s.ids[0])).toEqual(['alpha']); for (const id of [undefined, ...s.ids.slice(1)]) expect(await s.bound(id)).toEqual([]); });
test('全局绑定进入 B C，项目 resolved 其它技能不会污染全局', async () => { const s = await setup(); await saveBinding(s, s.ids[0], 'project', 'local', ['plan']); const result = await saveBinding(s, s.ids[0], 'global', 'alpha', ['plan']); expect(result.error).toBeUndefined(); expect(await s.bound()).toEqual(['alpha']); for (const id of s.ids.slice(1)) expect(await s.bound(id)).toEqual(['alpha']); expect(await s.bound(s.ids[0])).toEqual(['local']); });
test('A 只覆盖模型仍获全局绑定，C 技能列表覆盖读回明确列名', async () => { const s = await setup(); await s.core.call('config.set_role', { project_id: s.ids[0], role: 'plan', model: 'A-model' }); await s.core.call('config.set_role', { project_id: s.ids[2], role: 'plan', skills: [] }); const result = await saveBinding(s, s.ids[0], 'global', 'alpha', ['plan']); expect(result.messages).toHaveLength(1); expect(result.messages[0]).toContain('没有拿到这条绑定：C'); expect(result.messages[0]).not.toContain('：A'); expect(await s.bound(s.ids[0])).toEqual(['alpha']); expect(await s.bound(s.ids[1])).toEqual(['alpha']); expect(await s.bound(s.ids[2])).toEqual([]); });
test('同名混合 project_scoped 为真仍可全局绑定，但可见性不合并项目来源', async () => { const s = await setup(); const data = await loadSkills(s, s.ids[0]); const mixed = data.skills.find(x => x.name === 'mixed')!; expect(mixed.project_scoped).toBe(true); expect(bindingReason(mixed, 'global', resolvedRoles(data.globalConfig!), 'review')).toBeTruthy(); expect(bindingReason(mixed, 'project', resolvedRoles(data.config!), 'review')).toBeUndefined(); expect((await saveBinding(s, s.ids[0], 'global', 'mixed', ['plan'])).error).toBeUndefined(); expect(await s.bound(s.ids[1])).toEqual(['mixed']); });
test('绕过不可见勾选内核拒绝原样到 action', async () => { const s = await setup(); const direct = await s.gateway.write({ op: 'config.set_role', params: { role: 'review', skills: ['mixed'] } }); expect(direct.body.ok).toBe(false); const context = new RouterContextProvider(); context.set(servicesContext, s); const request = new Request('http://localhost/skills', { method: 'POST', body: new URLSearchParams({ scope: 'global', name: 'mixed', roles: 'review' }) }); const result = await action({ context, request, params: {}, url: new URL(request.url), pattern: '/skills' }); expect(result.error).toBe(direct.body.error); expect(result.saved).toEqual([]); });
test('项目取消全局绑定与全局取消后保留项目绑定两方向', async () => { const s = await setup(); await saveBinding(s, undefined, 'global', 'alpha', ['plan']); await saveBinding(s, s.ids[0], 'project', 'alpha', []); expect(await s.bound(s.ids[0])).toEqual([]); expect(await s.bound(s.ids[1])).toEqual(['alpha']); await saveBinding(s, s.ids[2], 'project', 'alpha', ['plan']); // no change is intentionally no override
  await s.core.call('config.set_role', { project_id: s.ids[2], role: 'plan', skills: ['alpha'] });
  const result = await saveBinding(s, undefined, 'global', 'alpha', []); expect(result.messages[0]).toContain('仍保留这条绑定：C'); expect(await s.bound(s.ids[1])).toEqual([]); expect(await s.bound(s.ids[2])).toEqual(['alpha']); });
test('配置读盘失败保留原文并拒绝保存', async () => { const s = await setup(); writeFileSync(join(s.automeHome, 'config.toml'), '[broken'); const direct = await s.gateway.read({ method: 'config.get' }); expect((await loadSkills(s)).error).toBe(direct.body.error); expect((await saveBinding(s, undefined, 'global', 'alpha', ['plan'])).error).toBe(direct.body.error); });
test('部分角色保存后拒绝明确返回已写项，不写后续角色', async () => { const s = await setup(); const result = await saveBinding(s, undefined, 'global', 'mixed', ['plan', 'review', 'impl']); expect(result.saved).toEqual(['plan']); expect(result.error).toBeTruthy(); const roles = resolvedRoles((await loadSkills(s)).config!); expect(roles.plan.skills).toEqual(['mixed']); expect(roles.impl.skills).toEqual([]); });
test('无网关及空输入、非法角色或无项目范围不能保存', async () => { expect((await saveBinding({ gateway: null }, undefined, 'global', 'alpha', ['plan'])).error).toBe('内核不可用'); for (const [scope, name] of [['project', 'alpha'], ['global', '']] as const) expect((await saveBinding({ gateway: null }, undefined, scope, name, [])).error).toBe('绑定请求无效'); });
test('真实描述按稳定来源首个非空值回退，卡片一致且只读不影响来源绑定可见性', async () => {
  const s = await setup();
  const before = (await loadSkills(s)).skills.find(x => x.name === 'alpha')!;
  const first = join(before.sources[0].path, 'SKILL.md'), second = join(before.sources[1].path, 'SKILL.md');
  const content = '---\ndescription: >-\n  第一行\n  第二行\n---\n# Skill';
  writeFileSync(first, '---\ndescription: ""\n---'); writeFileSync(second, content);
  const skill = (await loadSkills(s)).skills.find(x => x.name === 'alpha')!;
  expect(skill.description).toBe('第一行 第二行');
  expect(skill.sources).toEqual(before.sources); expect(skill.visible_to).toEqual(before.visible_to); expect(skill.bound_roles).toEqual(before.bound_roles);
  expect(renderToString(h(SkillCard, { skill, onSelect: () => {} }))).toContain('第一行 第二行');
  expect(readFileSync(second, 'utf8')).toBe(content);
  writeFileSync(first, '---\ndescription: 优先来源\n---');
  expect((await loadSkills(s)).skills.find(x => x.name === 'alpha')!.description).toBe('优先来源');
  writeFileSync(first, '# no header'); writeFileSync(second, Buffer.from([255]));
  expect((await loadSkills(s)).skills.find(x => x.name === 'alpha')!.description).toBeNull();
});
