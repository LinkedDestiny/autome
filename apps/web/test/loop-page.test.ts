import { effortOptions } from '../app/lib/loop-labels.ts';
import { expect, test } from 'vitest';
import { createElement as h } from 'react';
import { renderToString } from 'react-dom/server';
import { createMemoryRouter, RouterProvider } from 'react-router';
import global from './fixtures/m07-global.json' with { type: 'json' };
import project from './fixtures/m07-project.json' with { type: 'json' };
import { LoopView, colors, resetMessage } from '../app/routes/loop.tsx';
import { loadLoop, resolvedRoles, saveLoop, type ConfigReply, type LoopData } from '../app/components/loop-data.ts';
import { changedFields, roleOrder } from '../app/lib/role-rules.ts';
const cfg=global as ConfigReply;
function html(config=cfg, extra:Partial<LoopData>={}) { const router=createMemoryRouter([{path:'/',element:h(LoopView,{data:{config,projects:[],skills:[],...extra}})}]);try{return renderToString(h(RouterProvider,{router}));}finally{router.dispose();} }
test('六张角色卡按循环顺序颜色排列',()=>{const text=html();let prev=0;for(const role of roleOrder){const pos=text.indexOf(`data-role="${role}"`);expect(pos).toBeGreaterThan(prev);prev=pos;expect(text).toContain(`card-${colors[role]}`);} });
test('四张固定或人工虚线卡',()=>{expect(html().match(/card-dash/g)).toHaveLength(4);for(const name of ['任务整理','你批准设计','宿主快检','合并前门禁'])expect(html()).toContain(name);});
test('CLI 和 Effort 默认值',()=>{expect(html()).toContain('CLI 默认');expect(html()).toContain('Effort 默认');});
test('项目覆盖与跟随全局',()=>{const text=html(project as ConfigReply,{project:'p'});expect(text).toContain('项目覆盖');expect(text).toContain('跟随全局');});
test('冲突的两张卡标红且说明',()=>{const c=structuredClone(cfg);c.resolved.roles.find(r=>r.role==='review')!.config.runtime='claude';expect(html(c).match(/loop-conflict/g)).toHaveLength(2);expect(html(c)).toContain('都用 CLI 默认');});
test('项目只带改过的字段',()=>{const b=resolvedRoles(cfg).plan;expect(changedFields(b,{...b,model:'new'})).toEqual({model:'new'});expect(changedFields(b,{...b,effort:null})).toEqual({effort:null});});
test('不修改任意 effort 值时不生成覆盖',()=>{const b={...resolvedRoles(cfg).plan,effort:'xhigh'};expect(changedFields(b,{...b,model:'new'})).toEqual({model:'new'});});
test('恢复确认覆盖全部五字段',()=>{expect(resetMessage).toBe('这个角色的工具、模型、Effort、技能、启用的项目覆盖会一起清掉');});
test('轮数预算来自回包',()=>{expect(html()).toContain('15');expect(html()).toContain('5');});
test('离线失败明确且无配置卡',async()=>{const data=await loadLoop({gateway:null});expect(data.error).toBeTruthy();expect(html(cfg,{config:undefined,error:data.error})).not.toContain('data-role=');});
test('没有网关时保存失败并无已保存项',async()=>{expect(await saveLoop({gateway:null},undefined,[])).toEqual({saved:[],error:'内核不可用'});});

test('未知 Effort 只读其它选项保留原值',()=>{expect(effortOptions('xhigh').at(-1)).toEqual({value:'xhigh',label:'其它：xhigh',disabled:true});expect(effortOptions(null)).toHaveLength(4);});
test('写入未授权时角色开关禁用',()=>{const text=html();expect((text.match(/<input[^>]*>/g) ?? []).filter(input=>input.includes('role="switch"') && input.includes('disabled'))).toHaveLength(6);});

import { RoleDrawer } from '../app/routes/loop.tsx';
test('角色抽屉全局和项目的保存、恢复、成功与失败提示均在 footer', () => {
  for (const project of [undefined, 'P-1']) for (const result of [undefined, { saved: ['plan'] as const }, { saved: [], error: '保存失败' }]) {
    const router = createMemoryRouter([{ path: '/', element: h(RoleDrawer, { role: 'plan', config: resolvedRoles(cfg).plan, project, disabled: false, saveDisabled: false, onEdit() {}, onSave() {}, onReset() {}, onClose() {}, result: result && { ...result, saved: [...result.saved] } }) }]);
    try {
      const html = renderToString(h(RouterProvider, { router }));
      const foot = html.match(/<footer class="drawer-foot">([\s\S]*?)<\/footer>/)![1];
      expect(foot).toContain('>保存</button>');
      expect(foot.includes('恢复跟随全局')).toBe(!!project);
      if (result?.error) expect(foot).toContain('role="alert">保存失败');
      if (result?.saved.length) expect(foot).toContain('role="status">已保存');
    } finally { router.dispose(); }
  }
});

import { readFileSync } from 'node:fs';
import { roleNames } from '../app/lib/role-rules.ts';
const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
const rule = (selector: string) => css.slice(css.indexOf(selector + ' {')).split('}')[0];
test('范围和项目页签使用 seg 且仅当前项标记 aria-current', () => {
  const projects = [{ id: 'p', display_name: '项目' }] as LoopData['projects'];
  for (const scope of [undefined, 'p']) {
    const text = html(cfg, { project: scope, projects });
    const nav = text.match(/<nav class="seg" aria-label="配置范围">([\s\S]*?)<\/nav>/)![1];
    expect(nav.match(/aria-current="page"/g)).toHaveLength(1);
    expect(nav).toContain(scope ? 'aria-current="page" href="/loop?project=p"' : 'aria-current="page" href="/loop"');
    if (scope) expect(text).toMatch(/<nav class="seg" aria-label="项目配置页签">[\s\S]*?aria-current="page"[^>]*>Loop 配置/);
  }
});
test('模型和 Effort 同行且长模型完整保留 title', () => {
  for (const model of ['', 'a', 'model-'.repeat(80)]) {
    const config = structuredClone(cfg);
    config.resolved.roles.forEach(r => { r.config.model = model; r.config.effort = 'medium'; });
    const rows = html(config).match(/<div class="loop-role-model">[\s\S]*?<\/div>/g)!;
    expect(rows).toHaveLength(6);
    for (const row of rows) {
      expect(row).toContain(`title="${model || 'CLI 默认'}"`);
      expect(row).toContain('<span class="pill">medium</span>');
    }
  }
  expect(rule('.loop-role-model')).toContain('white-space: nowrap');
  expect(rule('.loop-role-model > span:first-child')).toContain('text-overflow: ellipsis');
  expect(rule('.loop-role-model .pill')).toContain('flex: none');
});
test('六张卡有编辑提示且开关独立于伸展按钮', () => {
  const cards = html().match(/<section[^>]*data-role="[^"]+"[\s\S]*?<\/section>/g)!;
  expect(cards).toHaveLength(6);
  for (const [index, card] of cards.entries()) {
    const button = card.match(/<button[\s\S]*?<\/button>/)![0];
    expect(button).toContain(`aria-label="编辑${roleNames[roleOrder[index]]}"`);
    expect(button).toContain('<span class="loop-role-edit">编辑</span>');
    expect(button).not.toContain('<input');
    expect(card).toContain('role="switch"');
  }
  expect(rule('.loop-role-head button::after')).toContain('inset: 0');
  expect(rule('.loop-role-head .switch')).toContain('z-index: 1');
});
test('卡片密度及悬停聚焦反馈遵循角色卡预算', () => {
  expect(rule('.loop-row .card')).toContain('padding: 12px 14px');
  expect(rule('.loop-row .card')).not.toContain('overflow-wrap');
  expect(rule('.loop-role')).toContain('gap: 5px');
  expect(css).toContain('.loop-role:hover, .loop-role:focus-within { transform: translateY(-2px); box-shadow: var(--animal-shadow-base)');
  expect(rule('.loop-role-disabled')).toContain('-webkit-line-clamp: 2');
  for (const enabled of [false, true]) {
    const config = structuredClone(cfg);
    config.resolved.roles.forEach(r => { r.config.enabled = enabled; });
    const text = html(config, { project: 'p' });
    expect((text.match(/class="loop-role-disabled"/g) ?? []).length).toBe(enabled ? 0 : 6);
    expect(text).toContain('class="loop-role-badges"');
    if (!enabled) expect(text).toContain('title="已停用：Autome');
  }
});
