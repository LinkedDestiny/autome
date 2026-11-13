import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { expect, it } from 'vitest';
import { timelineEntries, type TimelineSession } from '../app/components/task-timeline-model.ts';
import { TaskTimeline } from '../app/components/task-timeline.tsx';
import type { TaskEvent } from '../server/task-events.ts';
// Original wire fixtures captured by w7-flow.sh; synthetic variants below exercise event boundaries.
const fixture = (name: string) => JSON.parse(readFileSync(new URL(`./fixtures/w7/${name}.json`, import.meta.url), 'utf8'));
const feature = fixture('feature'), bug = fixture('bug');
const ledger: TaskEvent[] = fixture('events').events;
const now = '2026-11-14T00:00:00Z';
const ev = (seq: number, kind: string, payload: Record<string, unknown> = {}): TaskEvent => ({ seq, kind, payload, subject: 'T-1', at: `2026-11-13T00:00:${String(seq).padStart(2, '0')}Z` });
const ses = (id = 's', running = false): TimelineSession => ({ id, running, label: '实现', round: 1, runtime: 'codex', started_at: '2026-11-13T00:00:01Z', ended_at: running ? null : '2026-11-13T00:00:11Z' });
const build = (events: TaskEvent[], sessions: TimelineSession[] = []) => timelineEntries(sessions, events, now);
it('需求会话按事件升序，阶段、CLI 与原始回包一致', () => {
  const entries = build(ledger.filter(e => e.subject === 'T-1'), feature.sessions);
  expect(entries.flatMap(e => e.session ? e.session.label : [])).toEqual(['任务整理', '设计', '评审', '裁决', '实现', '审计', '实现', '审计', '复盘']);
  expect(entries.filter(e => e.session).every(e => e.details.includes(e.session?.runtime === 'claude' ? 'Claude Code' : 'Codex'))).toBe(true);
  expect(entries.some(e => e.title === '快检')).toBe(true);
});
it('逐会话快照摘要不会使用当前状态，并显示设计定稿、协议失败与崩溃', () => {
  const entries = build(ledger.filter(e => e.subject === 'T-1'), feature.sessions);
  const implementations = entries.filter(e => e.session?.label === '实现');
  expect(implementations[0].summary).toContain('M-01 开放 → 待审');
  expect(implementations[0].summary).not.toContain('已完成');
  expect(entries.find(e => e.session?.label === '裁决')?.summary).toContain('等你批准设计');
  for (const reason of [{ kind: 'protocol', detail: '缺少状态字段 status' }, { kind: 'session_crashed', detail: '会话退出码 3' }]) {
    const result = build([ev(1, 'session.started', { session_id: 's' }), ev(2, 'task.updated', { trigger: 'session_ended', state: { state: 'failed', at: 'implement', reason } })], [ses()]);
    expect(result[0].summary).toContain(reason.detail);
  }
});
it('Bug 复现只依据业务事件，修复门禁失败不当作符合预期', () => {
  const entries = build(ledger.filter(e => e.subject === 'T-2'), bug.sessions);
  expect(entries.some(e => e.summary.includes('已定位'))).toBe(true);
  expect(entries.some(e => e.summary.includes('复核通过'))).toBe(true);
  for (const [kind, payload, expected] of [
    ['bug.reproduced', { exit_code: 1 }, '符合预期'],
    ['bug.repro_missed', { why: 'not_runnable', exit_code: 127 }, '复现命令无法执行'],
    ['bug.repro_missed', { why: 'timed_out' }, '复现检查超时'],
    ['bug.repro_missed', { why: 'exited_zero' }, '未复现'],
  ] as const) {
    expect(build([ev(1, 'gate.result', { node: 'repro_check', exit_code: 1, ok: false }), ev(2, kind, payload)])[0].summary).toContain(expected);
  }
  expect(build([ev(1, 'gate.result', { node: 'repro_check', exit_code: 1, ok: false })])[0].summary).toBe('退出码 1');
  expect(build([ev(1, 'gate.result', { node: 'fix_gate', kind: 'repro', exit_code: 1, ok: false }), ev(2, 'bug.reproduced', { exit_code: 1 })])[0].summary).toBe('未通过 · 退出码 1');
  expect(build([ev(1, 'session.started', { session_id: 's' }), ev(2, 'bug.located', { status: 'CannotReproduce' })], [ses()])[0].summary).toBe('无法复现');
});
it('门禁一次执行的排队、运行、结果、同提交重跑、中断与 setup 排队', () => {
  const q = ev(1, 'gate.queued', { node: 'gate', job: 'gate', position: 2 });
  const start = ev(2, 'gate.started', { node: 'gate', kind: 'quick', sha: '1234567890', command: 'test', log: '/tmp/test.log' });
  expect(build([q])[0].summary).toBe('排队第 2 位');
  expect(build([q, start])[0]).toMatchObject({ key: 'gate:gate:1', title: '快检', summary: '运行中', details: ['test', '12345678', '/tmp/test.log'] });
  for (const [exit_code, ok, summary] of [[0, true, '通过'], [1, false, '未通过 · 退出码 1'], [null, false, '超时']] as const) {
    const result = ev(3, 'gate.result', { ...start.payload, exit_code, ok });
    expect(build([q, start, result])).toHaveLength(1);
    expect(build([q, start, result])[0].summary).toBe(summary);
    const retry = build([q, start, result, { ...start, seq: 4 }]);
    expect(retry.map(e => e.key)).toEqual(['gate:gate:1', 'gate:gate:4']); expect(retry[0].summary).toBe(summary);
  }
  for (const events of [[q, { ...q, seq: 2 }], [start, { ...start, seq: 3 }], [start, { ...q, seq: 3 }]]) expect(build(events)[0].summary).toBe('已中断');
  for (const [node, kind, title] of [['gate', 'full', '门禁'], ['fix_gate', 'quick', '修复门禁（复现 + 快检）'], ['fix_gate', 'full', '修复后回归（修复门禁）'], ['merge_gate', 'full', '合并前门禁'], ['main_check', 'full', '验证 main']]) expect(build([ev(1, 'gate.started', { node, kind })])[0].title).toBe(title);
  expect(build([ev(1, 'gate.queued', { job: 'setup', node: 'gate', position: 1 })])[0]).toMatchObject({ key: 'ev:1', title: '准备命令排队第 1 位' });
});
it('rebase 冲突与合并、准备命令和门禁复用轻条目', () => {
  const events = [ev(1, 'rebase.conflicts', { count: 1, files: ['a.ts', 'b.ts'] }), ev(2, 'merge.auto', { active: true }), ev(3, 'merge.rebased', { repos: [{ repo: 'app', onto: 'main', head: '123456789' }] }), ev(4, 'merge.repo', { commit: 'abcdefghi' })];
  const text = build(events).map(e => e.title).join(' ');
  for (const value of ['a.ts、b.ts', '自动 rebase', 'main @ 12345678', '已合并 · abcdefgh']) expect(text).toContain(value);
  for (const kind of ['merge.locked', 'merge.direct', 'merge.unlocked', 'merge.after_started', 'merge.after', 'merge.after_skipped', 'setup.started', 'setup.result', 'setup.skipped', 'gate.reused', 'gate.skipped']) expect(build([ev(1, kind, { reason: '原因' })])).toHaveLength(1);
});
it('会话花费、缺省、零金额、用时、被停止与运行时间', () => {
  for (const [metrics, cost] of [[null, '无计费数据'], [{ cost_usd: 0 }, '$0.00'], [{ cost_usd: 1.235 }, '$1.24']] as const) {
    const entry = build([], [{ ...ses(), metrics }])[0]; expect(entry.details).toContain(cost); expect(entry.details).toContain('用时 10 秒');
  }
  expect(build([ev(1, 'session.started', { session_id: 's' })], [{ ...ses(), lifecycle: { state: 'killed' } }])[0].summary).toBe('已被停止');
  expect(build([], [ses('s', true)])[0].details.join()).toContain('已运行');
  expect(build([], [{ ...ses(), started_at: '2024-02-29T23:59:59Z', ended_at: '2024-03-01T00:00:00Z' }])[0].details).toContain('用时 1 秒');
  expect(build([], [{ ...ses(), started_at: '2025-12-31T23:59:59Z', ended_at: '2026-01-01T00:00:00Z' }])[0].details).toContain('用时 1 秒');
});
it('稳定键与追加顺序，截断时按会话开始时间回填，空输入与未知事件', () => {
  const events = [ev(1, 'session.started', { session_id: 's' }), ev(2, 'gate.started', { node: 'gate' })];
  const initial = build(events, [ses()]);
  expect(initial.map(e => e.key)).toEqual(['s', 'gate:gate:2']);
  expect(build([...events, ev(3, 'gate.result', { node: 'gate', ok: true, exit_code: 0 }), ev(4, 'task.updated', { trigger: 'pause' })], [ses()]).map(e => e.key)).toEqual(['s', 'gate:gate:2', 'ev:4']);
  expect(build([], [{ ...ses('later'), started_at: now }, ses('earlier')]).map(e => e.key)).toEqual(['earlier', 'later']);
  expect(build([ev(1, 'gate.result', { node: 'repro_check', exit_code: 1 }), ev(2, 'gate.started', { node: 'fix_gate' }), ev(3, 'bug.reproduced', { exit_code: 1 })])[0].summary).toBe('退出码 1');
  expect(build([])).toEqual([]); expect(build([ev(1, 'unknown')])).toEqual([]);
});
it('运行中的会话默认展开，历史折叠，截断与读取失败提示可见', () => {
  const html = renderToString(createElement(TaskTimeline, { sessions: [ses('old'), ses('new', true)], events: [], now, truncated: true, error: '内核事件暂时读不到：超时' }));
  expect(html.match(/open=""/g)).toHaveLength(1); expect(html.match(/<details/g)).toHaveLength(2);
  expect(html).toContain('更早的内核事件已省略'); expect(html).toContain('内核事件暂时读不到：超时');
});
it('全部用户操作均保留独立条目', () => {
  const triggers = ['approve', 'reject', 'reply', 'merge', 'pause', 'resume', 'stop', 'cancel', 'rerun_from', 'extend_budget'];
  expect(build(triggers.map((trigger, i) => ev(i, 'task.updated', { trigger })))).toHaveLength(10);
});
it('重试、冷却、守卫和协议失败显示原因全文', () => {
  for (const kind of ['session.retry', 'session.deferred', 'runtime.cooldown', 'task.protocol_failure', 'guard.warning']) {
    const entry = build([ev(1, kind, { reason: '原因', detail: '协议全文', code: '范围守卫', until: now })])[0];
    expect(entry.key).toBe('ev:1'); expect(entry.title).not.toBe('');
  }
});
