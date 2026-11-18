import { readFileSync } from 'node:fs';
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
test('卡片含名称、路径、来源工具与绑定角色胶囊', () => { const text = page('A'); for (const s of skills) { expect(text).toContain(s.name); expect(text).toContain(s.sources[0].path); } for (const label of ['设计', '未绑定', 'Claude Code', 'Codex', '全局', '项目']) expect(text).toContain(label); expect(text.match(/data-testid="skill-card"/g)).toHaveLength(3); });
test('无项目时仅全局选项且隐藏本项目筛选', () => { expect(page()).not.toContain('本项目'); expect(page()).toContain('aria-label="技能范围"'); expect(page()).not.toContain('<select'); expect(page('A')).toContain('本项目 2'); });
test('抽屉六个勾选块及两个范围、全局中性提示', () => { const text = drawer(); expect(text.match(/type="checkbox"/g)).toHaveLength(6); expect(text).toContain('所有项目'); expect(text).toContain('仅当前项目'); expect(text).toContain('保存后会列出没有拿到的项目'); expect(text).not.toContain('项目 A'); });
test('每个来源独立打开按钮，混合同名全局可选且 Codex 禁用', () => { const text = drawer(); expect(text.match(/打开目录/g)).toHaveLength(2); expect(text).toMatch(/checked="" value="global"/); expect(text).toContain('看不到这个技能'); expect(text.match(/type="checkbox" disabled/g)).toHaveLength(3); });
test('纯项目禁全局并说明，项目范围继承警告', () => { const text = drawer('gamma', 'project'); expect(text).toMatch(/disabled=""[^>]*value="global"/); expect(text).toContain('项目技能只能绑定在自己的项目里'); expect(text).toContain('不再跟随全局技能列表'); expect(text).toContain('恢复跟随全局'); });
test('conflicts 卡片标红与角色工具原由', () => { const text = renderToString(h(SkillCard, { skill: { ...skills[0], conflicts: ['review'] }, roles, onSelect: noop })); expect(text).toContain('card-red'); expect(text).toContain('评审'); expect(text).toContain('看不到它'); });
test('空清单有明确提示与内部滚动区', () => { expect(page(undefined, [])).toContain('没有符合条件的技能'); expect(page()).toContain('class="skills-grid" data-scroll-region'); });
test('失去授权或忙时禁止绑定与打开目录', () => { const text = drawer('beta', 'global', true); expect(text).toContain('<fieldset disabled'); expect(text).toMatch(/disabled="">保存绑定/); expect(text.match(/disabled="">打开目录/g)).toHaveLength(2); });
test('描述为普通文本且缺失、空串、空白均明确回退', () => {
  for (const description of [undefined, null, '', '  ', '<script>alert(1)</script>']) {
    const text = renderToString(h(SkillCard, { skill: { ...skills[0], description }, roles, onSelect: noop }));
    expect(text).toContain(description?.trim() ? '&lt;script&gt;alert(1)&lt;/script&gt;' : '暂无描述');
    expect(text).not.toContain('<script>');
    expect(text).toContain(skills[0].sources[0].path);
  }
});
test('技能抽屉角色在来源之前，保存与所有结果提示位于 footer', () => {
  for (const result of [undefined, { saved: [], messages: [], error: '保存失败' }, { saved: [], messages: ['已打开目录'] }, { saved: ['plan' as const], messages: [] }]) {
    const html = renderToString(h(SkillDrawer, { skill: skills[0], scope: 'global', roles, selected: [], disabled: false, onScope: noop, onRoles: noop, onSave: noop, onOpen: noop, onClose: noop, result }));
    expect(html.indexOf('<fieldset')).toBeLessThan(html.indexOf('<h3>来源'));
    const foot = html.match(/<footer class="drawer-foot">([\s\S]*?)<\/footer>/)![1];
    expect(foot).toContain('>保存绑定</button>');
    if (result?.error) expect(foot).toContain('role="alert">保存失败');
    if (result && !result.error) expect(foot).toContain(result.saved.length ? '绑定已保存' : '无需修改绑定');
    if (result?.messages.length) expect(foot).toContain('已打开目录');
  }
});
const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
const rule = (selector: string) => css.slice(css.indexOf(selector + ' {')).split('}')[0];
test('126 张卡固定高度、三行描述、单行路径与额外来源计数', () => {
  expect(page('A', Array.from({ length: 126 }, (_, i) => ({ ...skills[1], name: `技能-${i}` }))).match(/data-testid="skill-card"/g)).toHaveLength(126);
  expect(rule('.skill-card')).toContain('height: 196px');
  expect(rule('.skill-description')).toContain('-webkit-line-clamp: 3');
  expect(rule('.skill-path > span')).toContain('white-space: nowrap');
  expect(rule('.skill-path > span')).toContain('text-overflow: ellipsis');
  const html = renderToString(h(SkillCard, { skill: skills[1], onSelect: noop }));
  expect(html).toContain(`title="${skills[1].sources[0].path}"`);
  expect(html).toContain('另 <!-- -->1<!-- --> 处');
  expect(html).not.toContain(skills[1].sources[1].path);
});
test('绑定使用一枚实心胶囊，未绑定使用描边胶囊', () => {
  for (const bound_roles of [[], ['audit', 'retro']] as SkillEntry['bound_roles'][]) {
    const html = renderToString(h(SkillCard, { skill: { ...skills[0], bound_roles }, onSelect: noop }));
    expect(html.match(/class="pill skill-binding[^"]*"/g)).toHaveLength(1);
    expect(html).toContain(bound_roles.length ? 'pill-green pill-fill" title="审计 · 复盘">审计 · 复盘' : 'skill-binding" title="未绑定">未绑定');
  }
  expect(rule('.skill-binding')).toContain('text-overflow: ellipsis');
});
test('范围为分段链接，项目编码与全局当前项准确', () => {
  expect(page()).toContain('aria-current="page" href="/skills"');
  const html = page('A/B');
  expect(html).toContain('class="seg " aria-label="技能范围"');
  expect(html).toContain('aria-current="page" href="/skills?project=A%2FB"');
  expect(html).toContain('class="muted">筛选');
});
test('抽屉使用两列勾选块，原因在标签内且完整描述先于来源', () => {
  const html = drawer();
  expect(html.match(/class="chk"/g)).toHaveLength(6);
  for (const label of html.match(/<label class="chk">[\s\S]*?<\/label>/g)!) {
    if (label.includes('disabled')) expect(label).toMatch(/aria-describedby="([^"]+)"[\s\S]*<small id="\1" class="chk-reason"/);
  }
  expect(rule('.skill-roles')).toContain('repeat(2, minmax(0, 1fr))');
  expect(rule('.chk-reason')).toContain('margin-left: auto');
  expect(html.indexOf('skill-drawer-description')).toBeLessThan(html.indexOf('<h3>来源'));
  const description = '完整描述\n' + '长文'.repeat(200);
  const full = renderToString(h(SkillDrawer, { skill: { ...skills[0], description }, scope: 'global', roles, selected: [], disabled: false, onScope: noop, onRoles: noop, onSave: noop, onOpen: noop, onClose: noop }));
  expect(full).toContain(description); expect(rule('.skill-drawer-description')).toContain('white-space: pre-line');
});
test('每个角色在两个范围都保留勾选与取消回调', () => {
  for (const scope of ['global', 'project'] as const) for (const role of Object.keys(roles) as (keyof typeof roles)[]) for (const checked of [true, false]) {
    let selected: (keyof typeof roles)[] = [];
    const element = SkillDrawer({ skill: skills[0], scope, roles, selected: checked ? [] : [role], disabled: false, project: 'A', onScope: noop, onRoles: next => { selected = next; }, onSave: noop, onOpen: noop, onClose: noop });
    const fieldset = element.props.children.find((child: { type?: string }) => child?.type === 'fieldset');
    const check = fieldset.props.children[1].find((child: { key: string }) => child.key === role);
    check.props.onChange({ target: { checked } });
    expect(selected).toEqual(checked ? [role] : []);
  }
});
