import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, test } from 'vitest';
import { stageBarView } from '../app/components/task-stage-model.ts';
import { TaskStage } from '../app/components/task-stage.tsx';
import { FEATURE_STAGES, BUG_STAGES } from '../app/lib/labels.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import type { TaskEvent } from '../server/task-history.ts';
const payload = (state: NonNullable<TaskPayload['task']['state']> = { state: 'active', node: 'implement' }, kind = 'feature'): TaskPayload => ({
  task: { id: 'T-1', title: '标题', kind, state, budget_n: 30, fix_round: 2 }, sessions: [],
  status_block: { design_round: 1, design_round_limit: 5, impl_round: 3, impl_round_limit: 9, milestones_done: 1, milestones_total: 3,
    current_milestone: 'M-02', milestones: [{ id: 'M-01', state: 'done' }, { id: 'M-02', state: 'pending' }, { id: 'M-03', state: 'open' }] },
});
const ev = (seq: number, kind: string, data: Record<string, unknown>): TaskEvent => ({ seq, kind, payload: data, subject: 'T-1', at: '' });
const current = (v: ReturnType<typeof stageBarView>) => v.stages.find(s => s.current)?.label;
test('1 feature active with milestone states and task budget priority', () => {
  const p = payload(); const v = stageBarView(p, []);
  expect(current(v)).toBe('实现'); expect(v.stages.filter(s => s.passed)).toHaveLength(3);
  expect(v.rounds).toBe('设计 1 / 5 · 实现 3 / 30'); expect(v.milestones.map(m => m.label)).toEqual(['已完成', '待审', '开放']);
  expect(v.milestones.map(m => m.current)).toEqual([false, true, false]);
  p.task.budget_n = null; expect(stageBarView(p, []).rounds).toContain('3 / 9');
  p.status_error = true; p.status_error_detail = '原文错误'; expect(stageBarView(p, []).message).toBe('设计文档读不出状态块：原文错误');
  p.status_block = null; expect(stageBarView({ ...p, status_error: false }, []).message).toBe('设计文档还没写出里程碑');
});
test('2 waiting design approval', () => { expect(current(stageBarView(payload({ state: 'active', node: 'await_design_approval' }), []))).toBe('批准设计'); });
test('3 failed audit retains phase and all node/state mappings', () => {
  const v = stageBarView(payload({ state: 'failed', at: 'audit' }), []); expect(current(v)).toBe('审计'); expect(v.stages[4].status).toBe('失败');
  for (const [kind, stages] of [['feature', FEATURE_STAGES], ['bug', BUG_STAGES]] as const) for (const [index, stage] of stages.entries()) for (const node of stage.nodes) {
    for (const state of [{ state: 'active', node }, { state: 'paused', resume: node }, { state: 'queued', resume: node }, { state: 'stopped', at: node }, { state: 'failed', at: node }]) {
      const mapped = stageBarView(payload(state, kind), []); expect(mapped.stages[index].current).toBe(true); expect(mapped.stages.filter(s => s.passed)).toHaveLength(index);
    }
  }
});
test('4 done falls back to latest snapshot and metrics', () => {
  const p = { ...payload({ state: 'done' }), status_block: null };
  const events = [ev(1, 'task.snapshot', { milestones: [['M-01', 'open']] }), ev(2, 'task.snapshot', { milestones: [['M-01', 'done']] }), ev(3, 'task.metrics', { design_rounds_used: 2, design_rounds_limit: 5, impl_rounds_used: 4, budget_n: 30 })];
  const v = stageBarView(p, events); expect(current(v)).toBe('完成'); expect(v.stages.every(s => s.passed)).toBe(true); expect(v.milestones[0].label).toBe('已完成'); expect(v.rounds).toBe('设计 2 / 5 · 实现 4 / 30');
});
test('5 stopped and cancelled recover last node before cancellation including queued resume', () => {
  expect(current(stageBarView(payload({ state: 'stopped', at: 'audit' }), []))).toBe('审计');
  const p = payload({ state: 'cancelled' });
  for (const state of [{ state: 'active', node: 'implement' }, { state: 'paused', resume: 'implement' }, { state: 'queued', resume: 'implement' }, { state: 'stopped', at: 'implement' }, { state: 'failed', at: 'implement' }]) {
    const events = [ev(1, 'task.updated', { state: { state: 'active', node: 'await_design_approval' } }), ev(2, 'task.updated', { state }), ev(3, 'task.updated', { state: { state: 'cancelled' } }), ev(4, 'task.updated', { state: { state: 'active', node: 'audit' } })];
    const v = stageBarView(p, events); expect(current(v)).toBe('实现'); expect(v.status).toBe('已取消'); expect(v.stages.every(s => s.status === null)).toBe(true);
  }
  for (const state of [{ state: 'draft' }, { state: 'queued' }]) expect(current(stageBarView(p, [ev(1, 'task.updated', { state }), ev(2, 'task.updated', { state: { state: 'cancelled' } })]))).toBeUndefined();
  expect(current(stageBarView(p, []))).toBeUndefined();
});
test('6 bug locate', () => { expect(current(stageBarView(payload({ state: 'active', node: 'locate' }, 'bug'), []))).toBe('定位'); });
test('7 bug await info', () => { expect(current(stageBarView(payload({ state: 'active', node: 'await_info' }, 'bug'), []))).toBe('定位'); });
test('8 bug reproduction comes from latest business event, never fix gate', () => {
  const p = payload({ state: 'active', node: 'fix' }, 'bug');
  for (const [kind, data, expected] of [
    ['bug.reproduced', { exit_code: 1 }, '已复现：复现命令失败（符合预期）· 退出码 1'],
    ['bug.repro_missed', { why: 'exited_zero' }, '未复现：复现命令通过了'],
    ['bug.repro_missed', { why: 'not_runnable' }, '复现命令无法执行（退出码 126/127）'],
    ['bug.repro_missed', { why: 'timed_out' }, '复现检查超时，不能据此断言未复现'],
  ] as const) {
    const v = stageBarView(p, [ev(1, 'bug.reproduced', { exit_code: 2 }), ev(2, kind, data), ev(3, 'gate.result', { node: 'fix_gate', kind: 'repro', exit_code: 1, ok: false })]);
    expect(v.reproduction).toBe(expected); expect(current(v)).toBe('修复'); expect(v.rounds).toBe('修复第 2 / 30 轮');
  }
  expect(stageBarView(p, [ev(1, 'gate.result', { node: 'fix_gate', kind: 'repro', exit_code: 1 })]).reproduction).toBe('复现检查未跑');
});
test('9 bug fix budget failure', () => { const v = stageBarView(payload({ state: 'failed', at: 'fix', reason: { kind: 'fix_budget' } }, 'bug'), []); expect(current(v)).toBe('修复'); expect(v.stages[1].status).toBe('失败'); });
test('10 bug done', () => { const v = stageBarView(payload({ state: 'done' }, 'bug'), []); expect(v.stages).toHaveLength(5); expect(v.stages.every(s => s.passed)).toBe(true); expect(current(v)).toBe('完成'); expect(v.milestones).toEqual([]); });
test('11 fix budget fallback and zero/empty/single milestone boundaries', () => {
  const p = payload({ state: 'draft' }, 'bug'); p.task.budget_n = null; p.task.fix_round = 0;
  expect(stageBarView(p, [], { resolved: { bug: { fix_rounds: 7 } } }).rounds).toBe('修复第 0 / 7 轮');
  p.task.budget_n = 0; expect(stageBarView(p, []).rounds).toBe('修复第 0 / 0 轮');
  for (const node of ['', 'future', '__proto__']) expect(stageBarView(payload({ state: 'active', node }), []).stages.some(s => s.current)).toBe(false);
  const f = payload(); f.status_block!.milestones = [{ id: 'M-02', state: 'pending' }]; expect(stageBarView(f, []).milestones[0].current).toBe(true);
});
test('12 rendered current phase has aria-current and milestones', () => {
  const html = renderToStaticMarkup(createElement(TaskStage, { view: stageBarView(payload(), []) }));
  expect(html).toContain('aria-current="step"'); expect(html.match(/aria-current="step"/g)).toHaveLength(1); expect(html).toContain('M-02 · 待审'); expect(html).toContain('设计 1 / 5 · 实现 3 / 30');
});

import { taskSummary } from '../app/components/task-summary-model.ts';
test('详细步骤沿用暂停失败排队停止的位置', () => {
  for (const state of [{state:'paused',resume:'audit'},{state:'queued',resume:'audit'},{state:'failed',at:'audit'},{state:'stopped',at:'audit'}]) expect(taskSummary(payload(state),[]).steps.find(s=>s.current)?.label).toBe('审计');
});
test('详细步骤取消保留位置，完成九步通过，Bug 六步通过', () => {
 expect(taskSummary(payload({state:'cancelled'}),[ev(1,'task.updated',{state:{state:'active',node:'implement'}})]).steps.find(s=>s.current)?.label).toBe('实现');
 expect(taskSummary(payload({state:'done'}),[]).steps.filter(s=>s.passed)).toHaveLength(9);
 expect(taskSummary(payload({state:'done'},'bug'),[]).steps.filter(s=>s.passed)).toHaveLength(6);
});

import { readFileSync } from 'node:fs';
import { fixture } from './fixtures/task-detail-60.ts';
import { revealCurrentMilestone } from '../app/components/task-stage.tsx';
test('30 个里程碑全量单行滚动，概要固定在行首，空和单元素边界', () => {
 const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
 expect(css).toMatch(/\.task-milestones\s*\{[^}]*flex-wrap: nowrap;[^}]*overflow-x: auto;[^}]*overflow-y: hidden/);
 expect(css).toMatch(/\.task-milestones li\s*\{[^}]*flex: none/);
 const view = stageBarView(fixture().data, []);
 for (const count of [0, 1, 30]) {
  const html = renderToStaticMarkup(createElement(TaskStage, { view: { ...view, milestones: view.milestones.slice(0, count) } }));
  expect(html.match(/<li/g)!.length).toBe(view.stages.length + count);
  if(count) expect(html).toContain(`里程碑 已完成 0 / ${count}`); else expect(html).not.toContain('task-milestone-bar');
 }
});
test('当前里程碑黄色实心，开放待审完成都保持当前标记', () => {
 for (const state of ['open', 'pending', 'done']) {
  const p = payload(); p.status_block!.milestones![1].state = state;
  const html = renderToStaticMarkup(createElement(TaskStage, { view: stageBarView(p, []) }));
  expect(html).toContain('<li class="is-current"><span class="pill pill-yellow pill-fill">M-02');
  expect(html).toContain(`里程碑 已完成 ${state === 'done' ? 2 : 1} / 3`);
 }
});
test('当前项左右越界只调整本条 scrollLeft，已可见和无当前项不动', () => {
 for(const [left,right,expected] of [[250,300,150],[-50,0,-50],[20,80,0],[20,400,20]]) {
  const list = {scrollLeft:0,getBoundingClientRect:()=>({left:0,right:150}),querySelector:()=>({getBoundingClientRect:()=>({left,right})})};
  revealCurrentMilestone(list as unknown as HTMLUListElement); expect(list.scrollLeft).toBe(expected);
 }
 const list={scrollLeft:50,querySelector:()=>null}; revealCurrentMilestone(list as unknown as HTMLUListElement); expect(list.scrollLeft).toBe(50);
 const source=readFileSync(new URL('../app/components/task-stage.tsx',import.meta.url),'utf8');
 expect(source).toContain('}, [currentId])'); expect(source).not.toContain('scrollIntoView');
});
