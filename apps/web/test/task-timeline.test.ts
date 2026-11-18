import { readFileSync } from 'node:fs';
import { createElement } from 'react';
import { renderToString } from 'react-dom/server';
import { expect, it } from 'vitest';
import { logName, timelineEntries, type TimelineSession } from '../app/components/task-timeline-model.ts';
import { TaskTimeline as Timeline } from '../app/components/task-timeline.tsx';
import { MemoryRouter } from 'react-router';
import { timelineEntries as entriesForView } from '../app/components/task-timeline-model.ts';
function TaskTimeline({ sessions, events, now, ...props }: { sessions: Parameters<typeof entriesForView>[0]; events: Parameters<typeof entriesForView>[1]; now: string; truncated?: boolean; error?: string }) {
  return createElement(MemoryRouter, {}, createElement(Timeline, { entries: entriesForView(sessions, events, now), window: { from: null, earlier: 0 }, ...props }));
}
import type { TaskEvent } from '../server/task-history.ts';
// Original wire fixtures captured by w7-flow.sh; synthetic variants below exercise event boundaries.
const fixture = (name: string) => JSON.parse(readFileSync(new URL(`./fixtures/w7/${name}.json`, import.meta.url), 'utf8'));
const feature = fixture('feature'), bug = fixture('bug');
const ledger: TaskEvent[] = fixture('events').events;
const now = '2026-11-14T00:00:00Z';
const ev = (seq: number, kind: string, payload: Record<string, unknown> = {}): TaskEvent => ({ seq, kind, payload, subject: 'T-1', at: `2026-11-13T00:00:${String(seq).padStart(2, '0')}Z` });
const ses = (id = 's', running = false): TimelineSession => ({ id, running, label: '实现', round: 1, runtime: 'codex', started_at: '2026-11-13T00:00:01Z', ended_at: running ? null : '2026-11-13T00:00:11Z' });
const build = (events: TaskEvent[], sessions: TimelineSession[] = []) => timelineEntries(sessions, events, now);
it('D-2 回归：真实 Bug 会话在事件缺失和到达后均带稳定 data-node', () => {
  for (const node of ['locate', 'fix', 'fix_review']) {
    const session = { ...bug.sessions.find((s: { kind: { step: string } }) => s.kind.step === node), running: true, ended_at: null };
    const started = ev(1, 'session.started', { session_id: session.id, kind: session.kind });
    for (const events of [[], [started]]) {
      expect(build(events, [session])[0]).toMatchObject({ key: session.id, node, summary: '运行中' });
      const html = renderToString(createElement(TaskTimeline, { sessions: [session], events, now }));
      expect(html).toContain(`data-node="${node}"`);
    }
  }
});
it('需求会话以结构化 kind 映射节点，不依赖中文标签或事件 node', () => {
  for (const [role, node] of [['plan', 'design'], ['impl', 'implement'], ['review', 'review'], ['adjudicate', 'adjudicate'], ['audit', 'audit'], ['retro', 'retro']]) {
    const session = { ...ses(), label: '自定义标签', kind: { kind: 'role' as const, role } };
    for (const events of [[], [ev(1, 'session.started', { session_id: 's', kind: session.kind })]]) {
      expect(build(events, [session])[0].node).toBe(node);
    }
  }
  for (const kind of ['intake', 'onboarding'] as const) expect(build([], [{ ...ses(), kind: { kind } }])[0].node).toBe(kind);
});
it('旧会话缺 kind 时保留事件节点，缺少全部依据不伪造节点', () => {
  expect(build([ev(1, 'session.started', { session_id: 's', node: 'implement' })], [ses()])[0].node).toBe('implement');
  expect(build([], [ses()])[0].node).toBeUndefined();
});
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
  expect(build([q, start])[0]).toMatchObject({ key: 'gate:gate:1', title: '快检', summary: '运行中', details: ['test', '12345678', '日志 test.log'], log: '/tmp/test.log' });
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
  expect(build([ev(1, 'session.started', { session_id: 's' })], [{ ...ses(), lifecycle: { state: 'killed' } }])[0].summary).toBe('被停止');
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
  expect(html.match(/open=""/g)).toHaveLength(1); expect(html.match(/<details class="session-log"/g)).toHaveLength(2);
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
it('步骤报告显示执行、沿用、未跑和红色未核实提示', () => {
  const event = ev(1, 'gate.result', { node: 'gate', ok: true, exit_code: 0, steps: { ran: 2, reused: 3, failed: 0, not_run: 1, unverified: 1 } });
  expect(build([event])[0]).toMatchObject({ summary: '通过 · 执行 2 步 · 沿用 3 步 · 未跑 1 项 · 沿用来源未核实 1 步', unverified: true });
  const html = renderToString(createElement(TaskTimeline, { sessions: [], events: [event], now }));
  expect(html).toContain('class="timeline-unverified"');
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  expect(css).toMatch(/\.timeline-unverified\s*\{\s*color:\s*var\(--animal-error-active\);\s*\}/);
  const verified = ev(2, 'gate.result', { ...event.payload, steps: { ran: 0, reused: 5, failed: 0, not_run: 0, unverified: 0 } });
  expect(build([verified])[0].summary).toBe('通过 · 执行 0 步 · 沿用 5 步');
  expect(renderToString(createElement(TaskTimeline, { sessions: [], events: [verified], now }))).not.toContain('timeline-unverified');
});
it('没有报告保持旧摘要，整次沿用与步骤沿用区分', () => {
  expect(build([ev(1, 'gate.result', { node: 'gate', ok: true, exit_code: 0 })])[0].summary).toBe('通过');
  expect(build([ev(1, 'gate.reused')])[0].title).toBe('沿用已通过的结果');
});
it('可见时间不保留 ISO，重试与冷却均使用本地时区', () => {
  const entries = timelineEntries([ses()], [ev(1,'session.retry',{not_before:now}),ev(2,'runtime.cooldown',{until:now})],now,'Asia/Shanghai');
  const text = entries.map(e=>[e.title,...e.details].join(' ')).join(' ');
  expect(text).not.toMatch(/\d{4}-\d{2}-\d{2}T/);
  expect(text).toContain('11月13日 08:00 → 08:00'); expect(text).toContain('08:00 重试');
});
it('用时 8 分 50 秒且运行中、已结束、被停止都保留用时', () => {
  for (const running of [true,false]) {
    const session = {...ses(), running, started_at:'2026-11-13T00:00:00Z',ended_at:running?null:'2026-11-13T00:08:50Z'};
    expect(timelineEntries([session],[],'2026-11-13T00:08:50Z')[0].details).toContain(`${running?'已运行':'用时'} 8 分 50 秒`);
  }
});

it('gate log paths appear only in hover titles for start and every result', () => {
  for (const kind of ['gate.started', 'gate.result']) for (const exit_code of [0, 1, null]) {
    const log = '/private/中文 目录/gate-key.log';
    const events = [ev(1, kind, { node: 'gate', log, exit_code, ok: exit_code === 0 })];
    const html = renderToString(createElement(TaskTimeline, { sessions: [], events, now }));
    expect(html).toContain('class="timeline-log" title="' + log + '">日志 gate-key.log</span>');
    expect(html.replace(/<[^>]*>/g, '')).not.toContain(log);
    expect(build(events)[0].log).toBe(log);
  }
});
it('all four setup and merge titles project log names including input boundaries', () => {
  for (const kind of ['setup.started', 'setup.result', 'merge.after_started', 'merge.after']) for (const log of ['', 'single.log', '/tmp/中文 空格.log']) for (const ok of [true, false]) {
    const events = [ev(1, kind, { log, ok, exit_code: ok ? 0 : 1 })];
    const html = renderToString(createElement(TaskTimeline, { sessions: [], events, now }));
    expect(build(events)[0].title).toContain('日志 ' + logName(log));
    if (log) expect(html).toContain('title="' + log + '">日志 ' + logName(log));
    if (log.startsWith('/')) expect(html.replace(/<[^>]*>/g, '')).not.toContain(log);
  }
  expect(logName('')).toBe(''); expect(logName('/')).toBe('');
});
it('事件给出四种结束方式与结束时刻，崩溃区分退出码和消失', () => {
  // Real kernel timeout requires session_minutes >= 1; use its exact wire shape here.
  for (const [end, exit_code, text] of [['exited', 0, '正常结束'], ['stopped', null, '被停止'], ['timed_out', null, '超时'], ['crashed', 3, '崩溃（退出码 3）'], ['crashed', null, '崩溃（进程消失）']] as const) {
    const ended = ev(21, 'session.ended', { session_id: 's', kind: { kind: 'role', role: 'impl' }, round: 1, end, exit_code, duration_ms: null, cost_usd: null });
    for (const started of [[], [ev(1, 'session.started', { session_id: 's' })]]) {
      const entry = build([...started, ended], [ses()])[0];
      expect(entry.details).toContain(text); expect(entry.details).toContain('用时 20 秒');
    }
  }
});
it('没有 ended 事件时退回会话行，运行中不伪造结束', () => {
  for (const [state, timed_out, exit_code, text] of [['killed', false, null, '被停止'], ['exited', true, null, '超时'], ['exited', false, 3, '崩溃（退出码 3）'], ['vanished', false, null, '崩溃（进程消失）'], ['exited', false, 0, '正常结束']] as const) {
    expect(build([], [{ ...ses(), lifecycle: { state, exit_code }, timed_out }])[0].details).toContain(text);
  }
  expect(build([], [ses('s', true)])[0].details).not.toContain('正常结束');
});
it('结束事件优先于矛盾的会话行及运行标志', () => {
  for (const session of [{ ...ses(), lifecycle: { state: 'killed' } }, ses('s', true)]) {
    const entry = build([ev(1, 'session.started', { session_id: 's' }), ev(21, 'session.ended', { session_id: 's', end: 'crashed', exit_code: 4 })], [session])[0];
    expect(entry.details).toContain('崩溃（退出码 4）'); expect(entry.details).toContain('用时 20 秒');
    expect(entry.summary).not.toBe('被停止'); expect(entry.summary).not.toBe('运行中'); expect(entry.session?.running).toBe(false);
  }
});
it('F 清理结束的进程显示 pid、命令和未退出项', () => {
  const title = build([ev(1, 'task.processes_stopped', { processes: [{ pid: 42, command: 'sleep 600', signal: 'TERM' }], survivors: [{ pid: 43, error: 'EPERM' }] })])[0].title;
  expect(title).toContain('清理时结束了 1 个残留进程：42 sleep 600');
  expect(title).toContain('仍有 1 个进程未退出：43 EPERM');
});
it('F 无法判断与有控制终端分别显示，混合及旧事件保留两种原因', () => {
  const unknown = { pid: 42, command: 'sleep', kind: 'unknown' };
  const tty = { pid: 43, command: 'shell', kind: 'tty', tty: 'ttys002' };
  for (const processes of [[unknown], [{ pid: 42 }], [{ pid: 42, kind: 'future' }]]) {
    expect(build([ev(1, 'task.processes_unchecked', { processes })])[0].title).toBe('清理时有 1 个进程无法判断，未结束');
  }
  const text = '清理时有 1 个进程有控制终端（可能是你自己打开的终端），未结束：43 ttys002 shell';
  expect(build([ev(1, 'task.processes_unchecked', { processes: [tty] })])[0].title).toBe(text);
  expect(build([ev(1, 'task.processes_unchecked', { processes: [unknown, tty] })])[0].title).toBe(`清理时有 1 个进程无法判断，未结束；${text}`);
});

import { timelineGroups } from '../app/components/task-timeline-model.ts';
it('用户行保留全部操作与决定', () => {
 const ops=['approve','reject','reply','merge','pause','resume','stop','cancel','rerun_from','extend_budget'];
 const rows=timelineEntries([], [...ops.map((trigger,i)=>ev(i,'task.updated',{trigger})),ev(11,'task.decided',{})],now);
 expect(rows.filter(r=>r.kind==='user')).toHaveLength(11);
});
it('门禁步数和 node/log 用时', () => {
 const rows=timelineEntries([], [ev(1,'gate.started',{node:'gate',log:'a'}),ev(4,'gate.result',{node:'gate',log:'a',ok:true,steps:{ran:2,reused:3}})],now);
 expect(rows[0].summary).toContain('执行 2 步 · 沿用 3 步'); expect(rows[0].durationMs).toBe(3000);
});
it('reused skipped 可筛选但没有 result 步数', () => {
 const rows=timelineEntries([], [ev(1,'gate.reused',{node:'gate'}),ev(2,'gate.skipped',{node:'gate',reason:'无需'})],now);
 expect(timelineGroups(rows,'gates').flatMap(g=>g.entries)).toHaveLength(2); expect(rows[1].title).toContain('无需');
});
it('真实审计退回会话可筛选且按阶段倒序', () => {
 const rows=timelineEntries(feature.sessions,ledger.filter(e=>e.subject==='T-1'),now);
 const groups=timelineGroups(rows); expect(groups.map(g=>g.end)).toEqual(groups.map(g=>g.end).toSorted().reverse());
 for(const g of groups) expect(g.entries.map(e=>e.at)).toEqual(g.entries.map(e=>e.at).toSorted().reverse());
});
it('筛选链接保留 from、doc 与四页签状态，所有筛选都有返回入口', () => {
 for(const show of ['', 'gates', 'reopens']) {
 const html=renderToString(createElement(MemoryRouter,{initialEntries:[`/?from=s1&doc=plan.md&tab=docs&show=${show}`]},createElement(Timeline,{entries:build([ev(1,'gate.result',{node:'gate',ok:false}),ev(2,'task.updated',{trigger:'reject'})]),window:{from:'s1',earlier:0}})));
 for(const value of ['', 'gates','reopens']) expect(html).toContain(`from=s1&amp;doc=plan.md&amp;tab=docs&amp;show=${value}`);
 expect(html).toContain('data-scroll-region');
 if(show==='gates')expect(html).not.toContain('timeline-user');
 }
});

const renderEntries = (entries: ReturnType<typeof build>, earlier = 0) => renderToString(createElement(MemoryRouter, {}, createElement(Timeline, { entries, window: { from: null, earlier } }))).replaceAll('<!-- -->', '');
it('紧凑行按时间、类别、文字、耗费、结果排列，会话携带 cost/outcome', () => {
  const entry = build([], [{ ...ses(), metrics: { cost_usd: 1.25 } }])[0];
  expect(entry).toMatchObject({ cost: '$1.25', outcome: 'pass', durationMs: 10000 });
  expect(renderEntries([entry])).toMatch(/<li[^>]*class="timeline-row [^"]*"><time[^>]*>00:00<\/time><span class="pill pill-purple">实现<\/span><details class="timeline-text">[\s\S]*<\/details><span class="timeline-metrics">10 秒 · \$1.25<\/span><div class="timeline-result"><span class="pill pill-green">通过<\/span><\/div><details class="session-log"/);
  for (const [kind, label] of [['gate.result', '门禁'], ['task.decided', '你'], ['task.protocol_failure', '系统']]) expect(renderEntries(build([ev(1, kind, { ok: true })]))).toContain(`>${label}</span><details class="timeline-text">`);
});
it('文字格保留完整标题摘要和详情，不再有 h3/meta 或详情 title 属性', () => {
  const entry = { ...build([], [ses()])[0], title: '完整标题', summary: '完整摘要', details: ['模型细节', '结束方式'], unverified: true };
  const html = renderEntries([entry]);
  expect(html).toContain('<details class="timeline-text"><summary><b>完整标题</b> <span class="timeline-unverified">完整摘要</span></summary><div><p>模型细节</p><p>结束方式</p></div></details>');
  expect(html).not.toMatch(/<h3|timeline-meta|title="[^"]*(模型细节|结束方式)/);
});
it('运行会话日志默认展开，结束会话折叠，失败标记保留', () => {
  for (const running of [true, false]) {
    const html = renderEntries(build([], [ses('s', running)]));
    const tag = html.match(/<details class="session-log"[^>]*>/)![0];
    expect(tag.includes('open=""')).toBe(running);
  }
  const entry = { ...build([], [ses()])[0], failures: ['错误原因'] };
  expect(renderEntries([entry])).toContain('<details class="timeline-failure"><summary>失败详情</summary><p>错误原因</p></details>');
});
it('40px 紧凑行、折叠省略与展开换行，更早会话是按钮链接', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  expect(css).toMatch(/\.timeline-row\s*\{[^}]*min-block-size: 40px/);
  expect(css).toMatch(/\.timeline-text > summary\s*\{[^}]*white-space: nowrap;[^}]*text-overflow: ellipsis/);
  expect(css).toMatch(/\.timeline-text\[open\] > summary\s*\{[^}]*white-space: normal/);
  expect(css).toMatch(/\.timeline-row > \.session-log\[open\]\s*\{[^}]*grid-column: 1 \/ -1/);
  expect(renderEntries([], 1)).toMatch(/<a class="btn btn-sm"[^>]*timeline=all[^>]*>显示更早的 1 个会话<\/a>/);
  expect(renderEntries([], 0)).not.toContain('显示更早的');
  expect(renderEntries([])).toContain('还没有执行记录');
});
it('首屏十条的密度预算包含行间距与分组标题外边距', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  expect(css).toMatch(/\.task-timeline li\s*\{[^}]*margin: 0 12px;/);
  expect(css).toMatch(/\.timeline-group-title\s*\{[^}]*margin: 4px 12px;/);
});
it('桌面详情给时间线留出高度，上方阶段与提示保留内部滚动', () => {
  const css = readFileSync(new URL('../app/styles/app.css', import.meta.url), 'utf8');
  expect(css).toMatch(/@media \(min-width: 1100px\) and \(min-height: 800px\)\s*\{\s*\.task-page\s*\{ gap: 4px; \}/);
  expect(css).toMatch(/\.task-page > \.task-stage\s*\{ max-height: 96px; \}/);
  expect(css).toMatch(/\.task-page > \.task-summary-tiles\s*\{ margin: 0; \}/);
  expect(css).toMatch(/\.task-page > \.task-controls\s*\{ max-height: 48px; \}/);
  expect(css).toMatch(/\.task-controls > \.task-notice\s*\{ margin: 0; padding: 8px 12px; \}/);
  for (const selector of ['.task-page > .task-stage', '.task-controls']) {
    expect(css.split(selector).some(rule => rule.slice(0, rule.indexOf('}')).includes('overflow-y: auto'))).toBe(true);
  }
});
it('R1-P01 长系统事件与多里程碑摘要均有独立完整文字展开入口', () => {
  const rows = build([
    ev(1, 'task.processes_stopped', { processes: [1, 2, 3].map(pid => ({ pid, command: 'long-command '.repeat(30) })), survivors: [{ pid: 4, error: '末尾错误原因 EPERM' }] }),
    ev(2, 'task.protocol_failure', { detail: '协议字段缺失 '.repeat(50) }),
    ev(3, 'task.snapshot', { milestones: [['M-01', '开放'], ['M-02', '开放']] }),
    ev(4, 'session.started', { session_id: 's' }),
    ev(5, 'task.snapshot', { milestones: [['M-01', '待审'], ['M-02', '已完成']] }),
  ], [ses()]);
  expect(rows[0].session).toBeUndefined(); expect(rows[0].failures).toBeUndefined();
  expect(rows[0].title).toContain('仍有 1 个进程未退出：4 末尾错误原因 EPERM');
  expect(rows[2].summary).toContain('M-01 开放 → 待审；M-02 开放 → 已完成');
  for (const row of rows) {
    const html = renderEntries([row]);
    expect(html).toContain(`<details class="timeline-text"><summary><b>${row.title}</b> <span>${row.summary}</span></summary>`);
    for (const detail of row.details) expect(html).toContain(`<p>${detail}</p>`);
  }
});
it('结果状态覆盖运行、结束、停止、超时、崩溃、门禁和退回', () => {
  expect(build([], [ses('s', true)])[0].outcome).toBe('running');
  expect(build([], [ses()])[0].cost).toBe('无计费');
  expect(build([], [{ ...ses(), metrics: { cost_usd: 0 } }])[0].cost).toBe('$0.00');
  for (const end of ['exited', 'stopped', 'timed_out', 'crashed']) expect(build([ev(2, 'session.ended', { session_id: 's', end, exit_code: end === 'exited' ? 0 : null })], [ses('s', true)])[0].outcome).toBe(end === 'exited' ? 'pass' : 'fail');
  for (const [kind, payload, outcome] of [
    ['gate.queued', { job: 'gate', position: 1 }, undefined], ['gate.started', {}, 'running'],
    ['gate.result', { ok: true, exit_code: 0 }, 'pass'], ['gate.result', { ok: false, exit_code: 1 }, 'fail'], ['gate.result', { ok: false, exit_code: null }, 'fail'],
    ['milestone.contradicted', {}, 'reopen'], ['task.updated', { trigger: 'reject' }, 'reopen'],
  ] as const) expect(build([ev(1, kind, { node: 'gate', ...payload })])[0].outcome).toBe(outcome);
});
