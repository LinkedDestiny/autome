import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { availableOps, RERUN_NODES, EXTEND_REASONS, type OpsInput } from '../app/components/task-ops.ts';
const state = (s: string, node?: string, reason?: string): OpsInput['state'] => ({ state: s, ...(s === 'active' ? { node } : s === 'paused' || s === 'queued' ? { resume: node } : { at: node }), ...(reason ? { reason: { kind: reason } } : {}) });
const input = (kind: string, s: OpsInput['state']): OpsInput => ({ kind, state: s, roles: null, mergeQueue: false, openDecisions: false });
const rows: [string, OpsInput['state'], string[]][] = [
  ['草稿',state('draft'),['start','cancel']], ['排队',state('queued'),['cancel']], ['重新排队',state('queued','implement'),['cancel']],
  ['角色运行',state('active','design'),['pause','stop','cancel']], ['内核运行',state('active','gate'),['pause','stop','cancel']],
  ['等批准',state('active','await_design_approval'),['approve','reject','cancel']], ['等合并',state('active','await_merge'),['merge','cancel']],
  ['合并中',state('active','merging'),['pause','stop','cancel']], ['暂停',state('paused','design'),['resume','stop','cancel']],
  ['停止',state('stopped','design'),['resume','cancel','rerun']], ['崩溃',state('failed','design','session_crashed'),['cancel','rerun']],
  ['设计预算',state('failed','design','design_budget'),['cancel','rerun','extend']], ['实现预算',state('failed','implement','impl_budget'),['cancel','rerun','extend']],
  ['快检熔断',state('failed','gate','gate_same_failure'),['cancel','rerun','extend']], ['合并门禁熔断',state('failed','merge_gate','gate_same_failure'),['cancel','rerun','extend']],
  ['协议失败',state('failed','audit','protocol'),['cancel','rerun']], ['完成',state('done'),[]], ['取消',state('cancelled'),[]],
];
for (const [label,s,expected] of rows) it(`需求：${label}`, () => {
  const states = label === '合并中' ? ['merging','main_check','cleanup'].map(n => state('active',n)) : label === '角色运行' ? RERUN_NODES.feature.filter(n=>n!=='rebase').map(n=>state('active',n)) : [s];
  for (const current of states) {
    const ops = availableOps(input('feature',current)); expect(ops.map(o=>o.op)).toEqual(expected);
    if (expected.includes('rerun')) expect(ops.find(o=>o.op==='rerun')?.nodes).toEqual([...RERUN_NODES.feature, ...(current.reason?.kind==='gate_same_failure' ? [current.at!] : [])]);
  }
});
const bugs: [string, OpsInput['state'], string[]][] = [
  ['草稿',state('draft'),['start','cancel']], ['排队',state('queued'),['cancel']], ['运行',state('active','locate'),['pause','stop','cancel']],
  ['门禁',state('active','repro_check'),['pause','stop','cancel']], ['补充',state('active','await_info'),['reply','cancel']],
  ['不是Bug',state('active','not_a_bug'),['reply','convert','cancel']], ['等合并',state('active','await_merge'),['merge','cancel']],
  ['暂停',state('paused','fix'),['resume','stop','cancel']], ['停止复现',state('stopped','repro_check'),['resume','cancel','rerun']],
  ['预算',state('failed','fix','fix_budget'),['cancel','rerun','extend']], ['熔断',state('failed','fix_gate','gate_same_failure'),['cancel','rerun','extend']],
  ['崩溃',state('failed','locate','session_crashed'),['cancel','rerun']], ['完成',state('done'),[]], ['取消',state('cancelled'),[]],
];
for (const [label,s,expected] of bugs) it(`Bug：${label}`, () => {
  const states = label === '运行' ? ['locate','fix','fix_review'].map(n=>state('active',n)) : label === '门禁' ? ['repro_check','fix_gate'].map(n=>state('active',n)) : [s];
  for (const current of states) {
    const ops=availableOps(input('bug',current)); expect(ops.map(o=>o.op)).toEqual(expected);
    if(expected.includes('rerun')) expect(ops.find(o=>o.op==='rerun')?.nodes).toEqual([...RERUN_NODES.bug,...(current.reason?.kind==='gate_same_failure'?[current.at!]:[])]);
  }
});
it('Bug 审计关闭草稿不可开始',()=>expect(availableOps({...input('bug',state('draft')),roles:{audit:false}})[0]).toMatchObject({op:'start',enabled:false,reason:expect.stringContaining('独立复核')}));
it('角色关闭过滤重跑并禁用恢复，未知角色按启用处理',()=>{
  for(const kind of ['feature','bug']) {
    const i={...input(kind,state('stopped','fix')),roles:{impl:false,audit:false}};
    expect(availableOps(i).find(o=>o.op==='rerun')?.nodes).toEqual(kind==='bug'?['rebase']:['intake','design','review','adjudicate','retro','rebase']);
    expect(availableOps(i).find(o=>o.op==='resume')).toMatchObject({enabled:false,reason:expect.any(String)});
  }
});
it('T-70 授权、撤销与旧内核隔离',()=>{
  for(const s of ['draft','queued','paused','stopped','failed','done','cancelled']) {
    const i={...input('feature',state(s,'design')),mergeQueue:true,merge:{authorized:true}};
    expect(availableOps(i).some(o=>o.op==='merge_cancel')).toBe(!['done','cancelled'].includes(s));
    expect(availableOps({...i,merge:{authorized:false}}).some(o=>o.op==='merge')).toBe(!['failed','done','cancelled'].includes(s));
  }
  for(const node of ['merging','main_check','cleanup']) expect(availableOps({...input('feature',state('active',node)),mergeQueue:true,merge:{authorized:true}}).some(o=>o.op.startsWith('merge'))).toBe(false);
  expect(availableOps({...input('feature',state('active','await_merge')),merge:{authorized:true}}).map(o=>o.op)).toEqual(['merge','cancel']);
});
it('表态仅看未消费条目，不看任务状态',()=>{
  for(const [,s] of rows) for(const openDecisions of [false,true]) expect(availableOps({...input('feature',s),openDecisions}).some(o=>o.op==='decide')).toBe(openDecisions);
});
it('读取 Rust 重跑臂和追加预算臂交叉核对',()=>{
  const snake=(s:string)=>s.replace(/[A-Z]/g,(c,i)=>(i?'_':'')+c.toLowerCase());
  for(const kind of ['feature','bug'] as const) {
    const source=readFileSync(new URL(`../../../crates/autome-domain/src/${kind==='feature'?'task':'bug_loop'}.rs`,import.meta.url),'utf8');
    const start=source.indexOf(kind==='feature'?'(TaskState::Failed { .. } | TaskState::Stopped { .. }, Trigger::RerunFrom':'(TaskState::Failed { at, .. } | TaskState::Stopped { at }, Trigger::RerunFrom');
    expect(start).toBeGreaterThan(0);
    const arm=source.slice(start,source.indexOf('_ =>',start));
    const nodes=[...new Set([...arm.matchAll(/Node::(\w+)/g)].map(m=>snake(m[1])))];
    expect(nodes.sort()).toEqual([...RERUN_NODES[kind]].sort());
    const segment=kind==='feature'?source.slice(source.indexOf('(TaskState::Failed { at, reason }, Trigger::ExtendBudget'),start):source.slice(source.indexOf('pub fn apply_for'),start);
    const reasons=[...new Set([...segment.matchAll(/FailureReason::(\w+)/g)].map(m=>snake(m[1])))];
    expect(reasons.sort()).toEqual([...EXTEND_REASONS[kind]].sort());
  }
});
it('所有置灰项有中文原因，归档 Bug 不提供改建',()=>{
  expect(availableOps({...input('bug',state('active','not_a_bug')),archived:true}).map(o=>o.op)).toEqual(['reply','cancel']);
  for(const kind of ['feature','bug']) for(const [,s] of [...rows,...bugs]) for(const op of availableOps({...input(kind,s),roles:{plan:false,impl:false,audit:false}})) if(!op.enabled) expect(op.reason).toMatch(/[\u4e00-\u9fff]/);
});
