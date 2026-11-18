import { expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { SummarySteps } from '../app/components/task-summary.tsx';
import feature from './fixtures/w7/feature.json' with { type: 'json' };
import bug from './fixtures/w7/bug.json' with { type: 'json' };
import history from './fixtures/w7/events.json' with { type: 'json' };
import { taskSummary, summaryElapsed } from '../app/components/task-summary-model.ts';
import { timelineEntries, timelineGroups } from '../app/components/task-timeline-model.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import type { TaskEvent } from '../server/task-history.ts';
const data = feature as unknown as TaskPayload;
it('阶段说明单行省略且完整工具与混合计费文本保留在 title', () => {
 const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
 expect(css).toMatch(/\.summary-steps small\s*\{[^}]*white-space: nowrap;[^}]*overflow: hidden;[^}]*text-overflow: ellipsis/);
 const summary = taskSummary(data, []);
 for (const unbilled of [12, 13]) {
  summary.steps[0] = { ...summary.steps[0], sessions: 13, unbilled, cost: 123.45, tools: ['claude', 'codex'] };
  const text = unbilled === 13 ? 'claude · codex · 无计费' : '$123.45 + 12 个无计费';
  expect(renderToStaticMarkup(createElement(SummarySteps, { summary }))).toContain(`<small title="${text}">${text}</small>`);
 }
});
const at = (n: number) => new Date(Date.UTC(2026, 0, 1, 0, n)).toISOString();
const event = (n: number, kind: string, payload: Record<string, unknown>): TaskEvent => ({ seq: n, at: at(n), subject: data.task.id, kind, payload });
const state = (n: number, node: string, trigger = '') => event(n, 'task.updated', { state: { state: node === 'failed' || node === 'paused' ? node : 'active', node }, trigger });
const waits = [state(0, 'await_design_approval'), state(1, 'design', 'reject'), state(2, 'await_design_approval'), state(4, 'implement', 'approve'), state(5, 'not_a_bug'), state(8, 'fix', 'reply'), state(9, 'failed'), state(10, 'paused', 'pause'), state(12, 'fix', 'resume')];
it('需求九步来自真实回包，花费与会话用时相等', () => {
  const s = taskSummary(data, history.events); expect(s.steps).toHaveLength(9);
  expect(s.steps[4].sessions).toBe(2);
  expect(s.steps[4].durationMs).toBe(data.sessions.filter(s => s.kind?.kind === 'role' && s.kind.role === 'impl').reduce((n, s) => n + Date.parse(s.ended_at!) - Date.parse(s.started_at), 0));
  expect(s.cost).toEqual(data.cost);
});
it('Bug 六步含独立复现检查与修复门禁', () => { expect(taskSummary(bug as unknown as TaskPayload, history.events).steps.map(s => s.label)).toEqual(['定位', '复现检查', '修复', '修复门禁', '复核', '合并']); });
it('两次批准等待之和不取会话间隙', () => { expect(taskSummary(data, waits).steps[3].durationMs).toBe(180000); });
it('not_a_bug 与 failed 计入等待，paused 排除', () => { expect(taskSummary(data, waits).time.waitingMs).toBe(420000); });
it('开放等待和总时长随 now 增长', () => {
 const s = taskSummary({ ...data, task: { ...data.task, created_at: at(0), completed_at: null } }, [state(2, 'await_design_approval')]);
 expect(summaryElapsed(s, at(5))).toMatchObject({ totalMs: 300000, waitingMs: 180000 }); expect(summaryElapsed(s, at(6)).steps[3]).toBe(240000);
});
it('完成时间冻结总时长', () => { const s = taskSummary({ ...data, task: { ...data.task, created_at: at(0), completed_at: at(3) } }, []); expect(summaryElapsed(s, at(20)).totalMs).toBe(180000); });
it('工具分组与真实会话逐条相等', () => { const s = taskSummary(data, []); for (const [tool, count] of Object.entries(s.sessions.byRuntime)) expect(count).toBe(data.sessions.filter(s => s.runtime === tool).length); });
it('门禁只数指定节点 result', () => {
 const events = ['gate', 'fix_gate', 'merge_gate', 'repro_check', 'main_check'].flatMap((node, i) => ['gate.result','gate.reused','gate.skipped'].map((kind,j) => event(i*3+j,kind,{node,ok: node !== 'fix_gate'})));
 expect(taskSummary(data, events).gates).toEqual({ total: 3, passed: 2, quick: 2, merge: 1 });
});
it('截断门禁不能显示零', () => { expect(taskSummary(data, [], true).gates).toBeNull(); });
it('在途退回取里程碑 reopen 之和', () => { expect(taskSummary({ ...data, task: { ...data.task, completed_at: null }, status_block: { ...data.status_block!, milestones: [{id:'M-1',state:'open',reopen_count:2},{id:'M-2',state:'pending',reopen_count:3}] } }, []).implementation.reopens).toBe(5); });
it('终结退回取 TaskMetrics', () => { expect(taskSummary({ ...data, task: { ...data.task, completed_at: at(3), metrics: { reopen_total: 7 } } }, []).implementation.reopens).toBe(7); });
it('无计费和零花费不同', () => { const s = taskSummary({ ...data, sessions: data.sessions.map((s,i) => ({...s,metrics:i ? null : {cost_usd:0}})) }, []); expect(s.steps.reduce((n,s)=>n+s.unbilled,0)).toBe(data.sessions.length-1); });
it('用户操作不丢失，只有退出等待带时长', () => { const rows = timelineEntries([], waits, at(20)); expect(rows.find(r=>r.title==='你补充了信息')?.waitMs).toBe(180000); expect(rows.find(r=>r.title==='你恢复了任务')?.waitMs).toBeUndefined(); });
it('只看门禁保留 reused skipped', () => { const rows=timelineEntries([],['gate.result','gate.reused','gate.skipped'].map((kind,i)=>event(i,kind,{node:'gate',ok:true})),at(9)); expect(timelineGroups(rows,'gates').flatMap(g=>g.entries)).toHaveLength(3); });
it('只看退回包含失败门禁驳回和矛盾事件', () => { const rows=timelineEntries([], [event(1,'gate.result',{node:'gate',ok:false}),state(2,'design','reject'),event(3,'milestone.contradicted',{})],at(9)); expect(timelineGroups(rows,'reopens').flatMap(g=>g.entries)).toHaveLength(3); });
it('阶段与组内均倒序', () => { const rows=timelineEntries([], [event(1,'gate.result',{node:'gate',ok:true}),event(2,'gate.result',{node:'gate',ok:true}),event(3,'gate.result',{node:'merge_gate',ok:true})],at(9)); const groups=timelineGroups(rows); expect(groups.map(g=>g.label)).toEqual(['合并','实现与审计']); expect(groups[1].entries.map(e=>e.at)).toEqual([at(2),at(1)]); });
it('门禁时长只匹配同 node 和 log', () => { const events=[event(1,'gate.started',{node:'merge_gate',log:'a'}),event(2,'gate.started',{node:'gate',log:'b'}),event(4,'gate.result',{node:'merge_gate',log:'a',ok:true})]; expect(taskSummary(data,events).steps[7]).toMatchObject({durationMs:180000,attempts:1}); expect(taskSummary({...data,sessions:[]},[...events,event(5,'gate.result',{node:'gate',log:'b',ok:true})]).steps[4].durationMs).toBe(0); });
it('空输入和单个运行会话', () => { const s=taskSummary({...data,sessions:[],status_block:null},[]); expect(s.sessions.total).toBe(0); const running=taskSummary({...data,sessions:[{...data.sessions[0],kind:{kind:'intake'},started_at:at(1),ended_at:null,running:true}]},[]); expect(summaryElapsed(running,at(2)).steps[0]).toBe(60000); });
it('所有等待节点进入保持退出迁移及等待切换', () => {
 for(const node of ['await_design_approval','await_merge','await_info','not_a_bug','failed']) {
  const s=taskSummary(data,[state(1,node),state(2,node),state(4,'implement','reply')]); expect(s.time.waitingMs).toBe(180000); expect(s.time.openWaiting).toBeUndefined();
 }
 const s=taskSummary(data,[state(1,'await_info'),state(2,'failed'),state(4,'paused')]); expect(s.time.waitingMs).toBe(180000);
});
it('审计从待审改开放才作为退回会话', () => {
 const session={...data.sessions[0],id:'audit-1',kind:{kind:'role' as const,role:'audit'},started_at:at(2),ended_at:at(4),running:false};
 const rows=timelineEntries([session],[event(1,'task.snapshot',{milestones:[['M-1','pending']]}),event(2,'session.started',{session_id:'audit-1',node:'audit'}),event(3,'task.snapshot',{milestones:[['M-1','open']]})],at(5));
 expect(timelineGroups(rows,'reopens').flatMap(g=>g.entries).map(e=>e.key)).toEqual(['audit-1']);
});
