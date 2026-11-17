import { expect, test } from 'vitest';
import fixture from './fixtures/m07-global.json' with { type: 'json' };
import { resolvedRoles, type ConfigReply } from '../app/components/loop-data.ts';
import { roleRules, saveOrder, type Role } from '../app/lib/role-rules.ts';
const base = () => resolvedRoles(structuredClone(fixture) as ConfigReply);
for (const [evaluator, generator] of [['review','plan'], ['audit','impl'], ['retro','impl']] as [Role,Role][]) {
  test(`${evaluator} 同模型拒绝`, () => { const r=base(); r[evaluator]={...r[generator],model:'same'};r[generator].model='same'; expect(roleRules(r,[])[0]).toMatchObject({kind:'same_model',roles:[evaluator,generator]}); });
  test(`${evaluator} 任一方停用免检`, () => { for(const role of [evaluator,generator]) {const r=base();r[evaluator]={...r[generator]};r[role].enabled=false;expect(roleRules(r,[])).toEqual([]);} });
  test(`${evaluator} 异工具或异模型合法`, () => {const r=base();expect(roleRules(r,[])).toEqual([]);r[evaluator].runtime=r[generator].runtime;r[evaluator].model='different';expect(roleRules(r,[])).toEqual([]);});
}
test('both_default 与空白按原值比较',()=>{const r=base();r.review.runtime='claude';expect(roleRules(r,[])[0].kind).toBe('both_default_models');r.review.model=' ';expect(roleRules(r,[])).toEqual([]);r.plan.model=' ';expect(roleRules(r,[])[0].kind).toBe('both_default_models');});
test('技能不存在及停用仍检查技能',()=>{const r=base();r.plan.skills=['missing'];r.plan.enabled=false;expect(roleRules(r,[])[0].kind).toBe('skill_not_found');});
test('项目覆盖工具导致技能不可见',()=>{const r=base();r.plan.skills=['skill'];const skills=[{name:'skill',visible_to:['claude']}];expect(roleRules(r,skills)).toEqual([]);r.plan.runtime='codex';r.plan.model='other';expect(roleRules(r,skills)[0].kind).toBe('skill_not_visible');});
test('保存顺序可避开中间碰撞',()=>{const r=base();r.plan.model='a';r.review={...r.review,runtime:'claude',model:'b'};expect(saveOrder(r,{plan:{model:'b'},review:{model:'c'}},[])).toEqual(['review','plan']);});
test('互换模型无合法保存顺序',()=>{const r=base();r.plan.model='a';r.review={...r.review,runtime:'claude',model:'b'};expect(saveOrder(r,{plan:{model:'b'},review:{model:'a'}},[])).toBeNull();});
test('空改动与单角色边界',()=>{expect(saveOrder(base(),{},[])).toEqual([]);expect(saveOrder(base(),{plan:{model:'a'}},[])).toEqual(['plan']);});
