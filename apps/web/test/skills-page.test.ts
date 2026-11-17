import { expect, test } from 'vitest';
import { createElement as h } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider } from 'react-router';
import captured from './fixtures/m09-skills.json' with { type: 'json' };
import config from './fixtures/m07-global.json' with { type: 'json' };
import { SkillDrawer, SkillCard, SkillsView } from '../app/routes/skills.tsx';
import { resolvedRoles, type ConfigReply } from '../app/components/loop-data.ts';
import type { SkillEntry } from '../app/lib/skills-model.ts';
const skills = captured.skills as SkillEntry[], roles = resolvedRoles(config as ConfigReply);
function page(project?: string, list = skills) { const router = createMemoryRouter([{ path: '/', element: h(SkillsView, { data: { project, projects: project ? [{ id: project, display_name: '项目 A' }] : [], skills: list, config: config as ConfigReply, globalConfig: config as ConfigReply } }) }]); try { return renderToString(h(RouterProvider, { router })); } finally { router.dispose(); } }
const noop = () => {};
function drawer(name = 'beta', scope: 'global' | 'project' = 'global', disabled = false) { return renderToString(h(SkillDrawer, { skill: skills.find(s => s.name === name)!, scope, roles, selected: [], project: 'A', disabled, onScope: noop, onRoles: noop, onSave: noop, onOpen: noop, onClose: noop })); }
test('卡片含名称、路径、来源工具与绑定角色胶囊', () => { const text = page('A'); for (const s of skills) { expect(text).toContain(s.name); for (const p of s.sources) expect(text).toContain(p.path); } for (const label of ['设计', '必用', '未绑定', 'Claude Code', 'Codex', '全局', '项目']) expect(text).toContain(label); expect(text.match(/data-testid="skill-card"/g)).toHaveLength(3); });
test('无项目时仅全局选项且隐藏本项目筛选', () => { expect(page()).not.toContain('本项目'); expect(page().match(/<option/g)).toHaveLength(1); expect(page('A')).toContain('本项目 2'); });
test('抽屉六个勾选块及两个范围、全局中性提示', () => { const text = drawer(); expect(text.match(/type="checkbox"/g)).toHaveLength(6); expect(text).toContain('所有项目'); expect(text).toContain('仅当前项目'); expect(text).toContain('保存后会列出没有拿到的项目'); expect(text).not.toContain('项目 A'); });
test('每个来源独立打开按钮，混合同名全局可选且 Codex 禁用', () => { const text = drawer(); expect(text.match(/打开目录/g)).toHaveLength(2); expect(text).toMatch(/checked="" value="global"/); expect(text).toContain('看不到这个技能'); expect(text.match(/type="checkbox" disabled/g)).toHaveLength(3); });
test('纯项目禁全局并说明，项目范围继承警告', () => { const text = drawer('gamma', 'project'); expect(text).toMatch(/disabled=""[^>]*value="global"/); expect(text).toContain('项目技能只能绑定在自己的项目里'); expect(text).toContain('不再跟随全局技能列表'); expect(text).toContain('恢复跟随全局'); });
test('conflicts 卡片标红与角色工具原由', () => { const text = renderToString(h(SkillCard, { skill: { ...skills[0], conflicts: ['review'] }, roles, onSelect: noop })); expect(text).toContain('card-red'); expect(text).toContain('评审'); expect(text).toContain('看不到它'); });
test('空清单有明确提示与内部滚动区', () => { expect(page(undefined, [])).toContain('没有符合条件的技能'); expect(page()).toContain('class="skills-grid" data-scroll-region'); });
test('失去授权或忙时禁止绑定与打开目录', () => { const text = drawer('beta', 'global', true); expect(text).toContain('<fieldset disabled'); expect(text).toMatch(/disabled="">保存绑定/); expect(text.match(/disabled="">打开目录/g)).toHaveLength(2); });
