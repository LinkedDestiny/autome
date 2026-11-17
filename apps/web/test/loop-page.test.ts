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
