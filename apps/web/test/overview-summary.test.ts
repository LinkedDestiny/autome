import { expect, test, vi } from 'vitest';
import captured from './fixtures/w7/events.json' with { type: 'json' };
import { createOverviewSummary, dayBounds, recentMerged } from '../server/overview-summary.ts';
import { loadOverview } from '../server/overview.ts';
import { waitingMilliseconds } from '../app/lib/overview-model.ts';
import { waitingTransition } from '../app/lib/waiting.ts';
import type { TaskEvent } from '../server/task-history.ts';
import type { ShellTask } from '../app/components/shell-data.ts';
const clock = { now: '2026-11-17T02:00:00Z', timeZone: 'Asia/Shanghai' };
const start = Date.parse('2026-11-16T16:00:00Z');
const event = (seq: number, kind: string, payload: Record<string, unknown>, at = '2026-11-17T01:00:00Z', subject = 'T-1'): TaskEvent => ({ seq, kind, payload, at, subject });
const state = (node: string) => ({ state: 'active', node });
const update = (seq: number, node: string, at?: string) => event(seq, 'task.updated', { state: state(node) }, at);
const task = (id: string, completed_at: string | null, status = 'done'): ShellTask => ({ id, title: id, project_id: 'P-1', kind: 'feature', state: { state: status }, completed_at });
function fixture(events: TaskEvent[]) {
  const call = vi.fn(async (_method: string, p: Record<string, unknown>) => ({ events: events.filter(e => e.seq > Number(p.after_seq)).slice(0, Number(p.limit)), latest_seq: events.at(-1)?.seq ?? 0 }));
  return { summary: createOverviewSummary(call), call };
}
test('时区切日含闰日、跨年和 DST 的 23/25 小时', () => {
  expect(dayBounds(clock)).toEqual({ dayStart: start, dayEnd: start + 86400_000 });
  for (const [now, timeZone, hours] of [['2026-03-08T16:00:00Z', 'America/New_York', 23], ['2026-11-01T16:00:00Z', 'America/New_York', 25], ['2024-02-29T12:00:00Z', 'UTC', 24], ['2027-01-01T00:00:00Z', 'UTC', 24]] as const) {
    const bounds = dayBounds({ now, timeZone }); expect(bounds.dayEnd - bounds.dayStart).toBe(hours * 3600_000); expect(Date.parse(now)).toBeGreaterThanOrEqual(bounds.dayStart); expect(Date.parse(now)).toBeLessThan(bounds.dayEnd);
  }
});
test('跨午夜闭合等待只算今天交集', async () => {
  const { today } = await fixture([update(1, 'await_design_approval', '2026-11-16T15:00:00Z'), update(2, 'implement', '2026-11-16T17:00:00Z')]).summary([task('T-1', null, 'active')], clock);
  expect(today.closedMs).toBe(3600_000); expect(today.ended).toEqual({ await_design_approval: 1 });
});
test('reused / skipped 不计，repro_check / main_check 不计', async () => {
  const events = ['gate.reused', 'gate.skipped'].map((kind, i) => event(i + 1, kind, { node: 'gate', ok: true }));
  ['gate', 'fix_gate', 'merge_gate', 'repro_check', 'main_check'].forEach((node, i) => events.push(event(i + 3, 'gate.result', { node, ok: i !== 1 })));
  const { today } = await fixture(events).summary([], clock); expect(today.gates).toBe(3); expect(today.passed).toBe(2);
});
test('取消后的任务与旧终结任务今天的复盘仍计花费、门禁和等待', async () => {
  const events = [update(1, 'await_merge'), event(2, 'session.ended', { cost_usd: 2 }), event(3, 'gate.result', { node: 'merge_gate', ok: true }), event(4, 'task.updated', { state: { state: 'cancelled' } }, '2026-11-17T01:10:00Z'), event(5, 'session.ended', { cost_usd: 3 }, undefined, 'T-old')];
  const { today } = await fixture(events).summary([task('T-1', null, 'cancelled'), task('T-old', '2025-01-01T00:00:00Z')], clock);
  expect(today).toMatchObject({ cost: 5, gates: 1, closedMs: 600_000, merged: 0 });
});
test('not_a_bug 等待在 reply 退出时结束，不按 await 前缀推断', async () => {
  const { today } = await fixture([update(1, 'not_a_bug'), event(2, 'task.updated', { state: state('locate'), trigger: 'reply' }, '2026-11-17T01:05:00Z')]).summary([task('T-1', null, 'active')], clock);
  expect(today.closedMs).toBe(300_000); expect(today.openStarts).toEqual([]); expect(today.ended).toEqual({ not_a_bug: 1 });
});
test('无计费单独计数，零成本仍是已计费', async () => {
  const { today } = await fixture([event(1, 'session.ended', { cost_usd: null }), event(2, 'session.ended', {}), event(3, 'session.ended', { cost_usd: 0 })]).summary([], clock);
  expect(today).toMatchObject({ cost: 0, billed: 1, unbilled: 2 });
});
test('开放区间保留起点，不按序号缓存固定时长，now 前进后增长', async () => {
  const { summary, call } = fixture([update(1, 'await_info')]);
  const tasks = [{ ...task('T-1', null), state: state('await_info') }];
  const first = await summary(tasks, clock), laterClock = { ...clock, now: '2026-11-17T02:01:00Z' }, second = await summary(tasks, laterClock);
  expect(first.today).toEqual(second.today); expect(first.today.closedMs).toBe(0); expect(first.today.openStarts).toEqual([Date.parse('2026-11-17T01:00:00Z')]);
  expect(waitingMilliseconds(second.today, laterClock.now) - waitingMilliseconds(first.today, clock.now)).toBe(60_000);
  expect(call.mock.calls[1][1]).toEqual({ after_seq: 1, limit: 1000 });
});
test('合并数只数今天 done，零点为闭区间，次日零点为开区间', async () => {
  const { today } = await fixture([]).summary([task('1', new Date(start).toISOString()), task('2', new Date(start + 86400_000).toISOString()), task('3', new Date(start).toISOString(), 'cancelled'), task('4', null), task('5', new Date(start - 1).toISOString())], clock);
  expect(today.merged).toBe(1);
});
test('最近合并倒序前八，空列表、单元素与多项目全部链接', async () => {
  const tasks = Array.from({ length: 10 }, (_, i) => task(String(i), new Date(start + i * 1000).toISOString()));
  expect(recentMerged(tasks).map(t => t.id)).toEqual(['9', '8', '7', '6', '5', '4', '3', '2']); expect(recentMerged([])).toEqual([]); expect(recentMerged(tasks.slice(0, 1))).toHaveLength(1);
  const shell = { kernel: { ok: true }, tasks, projects: [{ id: 'P-1', name: '甲', counts: null }] };
  const services = { gateway: null, overviewSummary: fixture([]).summary };
  expect((await loadOverview(services, shell, clock)).allDoneHref).toBe('/projects/P-1?done=all');
  expect((await loadOverview(services, { ...shell, projects: [...shell.projects, { id: 'P-2', name: '乙', counts: null }] }, clock)).allDoneHref).toBeUndefined();
});
test('所有等待状态 × 不变、另一等待、离开，暂停/停止/未知状态不计', () => {
  const states = ['await_design_approval', 'await_merge', 'await_info', 'not_a_bug'].map(state).concat({ state: 'failed', node: '' });
  for (const currentState of states) {
    const current = waitingTransition(undefined, currentState, 10).open!; expect(current.start).toBe(10);
    expect(waitingTransition(current, currentState, 20)).toEqual({ open: current });
    for (const next of [...states, { state: 'paused' }, { state: 'stopped' }, { state: 'done' }, { state: 'cancelled' }, state('implement'), state('await_future')]) {
      const result = waitingTransition(current, next, 30);
      if (next === currentState) expect(result.open).toEqual(current);
      else { expect(result.closed).toEqual({ ...current, end: 30 }); }
    }
  }
  for (const status of ['paused', 'stopped']) expect(waitingTransition(undefined, { state: status }, 10).open).toBeUndefined();
});
test('真实内核捕获事件与独立直接计数相符', async () => {
  const events = captured.events as TaskEvent[], now = events.at(-1)!.at;
  const latest = new Map<string, ShellTask['state']>();
  for (const e of events) if (e.kind === 'task.updated' && e.payload.state) latest.set(e.subject, e.payload.state as ShellTask['state']);
  const tasks = [...latest].map(([id, state]) => ({ ...task(id, null), state }));
  const { today } = await fixture(events).summary(tasks, { now, timeZone: 'UTC' });
  expect(today.gates).toBe(events.filter(e => e.kind === 'gate.result' && ['gate', 'fix_gate', 'merge_gate'].includes(String(e.payload.node))).length);
  expect(today.openStarts).toHaveLength(2);
});
test('分页、回退重建和并发 loader 不重复累加', async () => {
  let events = Array.from({ length: 1001 }, (_, i) => event(i + 1, 'session.ended', { cost_usd: 1 }));
  const call = vi.fn(async (_: string, p: Record<string, unknown>) => ({ events: events.filter(e => e.seq > Number(p.after_seq)).slice(0, Number(p.limit)), latest_seq: events.at(-1)?.seq ?? 0 }));
  const summary = createOverviewSummary(call);
  const values = await Promise.all([summary([], clock), summary([], clock)]); expect(values.map(v => v.today.cost)).toEqual([1001, 1001]);
  events = [event(1, 'session.ended', { cost_usd: 5 })]; expect((await summary([], clock)).today.cost).toBe(5);
});
test('读盘失败、超时与服务不可达呈现缺失而非零，恢复后重试', async () => {
  for (const message of ['读盘失败', '超时', '外部服务不可达']) {
    let fail = true;
    const summary = createOverviewSummary(async () => { if (fail) throw Error(message); return { events: [], latest_seq: 0 }; });
    const data = await loadOverview({ gateway: null, overviewSummary: summary }, { kernel: { ok: true }, projects: [], tasks: [] }, clock);
    expect(data.today).toBeUndefined(); expect(data.error).toContain(message); fail = false; expect((await summary([], clock)).today.cost).toBe(0);
  }
});
test('部分项目读盘失败时不把不完整账本显示成全局零，卡片超时保留错误', async () => {
  const failed = await loadOverview({ gateway: null }, { kernel: { ok: true }, projects: [{ id: 'P-1', name: '甲', counts: null, error: '读盘失败' }], tasks: [] }, clock);
  expect(failed.today).toBeUndefined(); expect(failed.listError).toBe('读盘失败'); expect(failed.allDoneHref).toBeUndefined();
  const gateway = { read: vi.fn(async () => ({ status: 504, body: { ok: false, error: '任务读取超时' } })), write: vi.fn() };
  const result = await loadOverview({ gateway, overviewSummary: fixture([]).summary }, { kernel: { ok: true }, projects: [], tasks: [{ ...task('T-1', null), state: state('await_design_approval') }] }, clock);
  expect(result.details['T-1'].error).toContain('任务读取超时'); expect(result.details['T-1'].status_block).toBeUndefined();
});
test('移除任务的五种等待均不计，时钟前进不增长，重新添加恢复', async () => {
  for (const status of [state('await_design_approval'), state('await_merge'), state('await_info'), state('not_a_bug'), { state: 'failed' }]) {
    const { summary } = fixture([event(1, 'task.updated', { state: status }, '2026-11-16T16:00:00Z')]);
    const current = [{ ...task('T-1', null), state: status }];
    expect((await summary(current, clock)).today.openStarts).toEqual([start]);
    for (const now of ['2026-11-16T20:52:00Z', '2026-11-16T20:57:00Z']) {
      const result = await summary([], { ...clock, now });
      expect(result.waiting).toEqual({}); expect(result.today).toMatchObject({ closedMs: 0, openStarts: [], ended: {} });
      expect(waitingMilliseconds(result.today, now)).toBe(0);
    }
    expect((await summary(current, clock)).waiting).toEqual({ 'T-1': start });
  }
});
test('账本开放但当前不等待：所有非等待状态过滤，恢复等待重新计入', async () => {
  const { summary } = fixture([update(1, 'await_info')]);
  for (const status of [{ state: 'active', node: 'implement' }, { state: 'active', node: 'await_future' }, ...['done', 'cancelled', 'paused', 'stopped', 'queued'].map(state => ({ state }))]) {
    const result = await summary([{ ...task('T-1', null), state: status }], clock);
    expect(result.today.openStarts).toEqual([]); expect(result.waiting).toEqual({});
  }
  expect((await summary([{ ...task('T-1', null), state: state('await_info') }], clock)).today.openStarts).toHaveLength(1);
});
test('闭合区间与结束计数按任务身份过滤，移除再添加不丢失缓存', async () => {
  const { summary } = fixture([update(1, 'await_merge'), update(2, 'implement', '2026-11-17T01:05:00Z'), event(3, 'task.updated', { state: state('await_info') }, '2026-11-17T01:10:00Z', 'T-2'), event(4, 'task.updated', { state: { state: 'done' } }, '2026-11-17T01:20:00Z', 'T-2')]);
  for (const ids of [['T-1', 'T-2'], ['T-2'], [], ['T-1']]) {
    const { today } = await summary(ids.map(id => task(id, null)), clock);
    expect(today.closedMs).toBe((ids.includes('T-1') ? 300_000 : 0) + (ids.includes('T-2') ? 600_000 : 0));
    expect(today.ended).toEqual({ ...(ids.includes('T-1') ? { await_merge: 1 } : {}), ...(ids.includes('T-2') ? { await_info: 1 } : {}) });
  }
});
