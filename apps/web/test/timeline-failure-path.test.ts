import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, it } from 'vitest';
import { timelineEntries, type TimelineSession } from '../app/components/task-timeline-model.ts';
import { TaskTimeline as Timeline } from '../app/components/task-timeline.tsx';
import { MemoryRouter } from 'react-router';
import { timelineEntries as entriesForView } from '../app/components/task-timeline-model.ts';
function TaskTimeline({ sessions, events, now, ...props }: { sessions: Parameters<typeof entriesForView>[0]; events: Parameters<typeof entriesForView>[1]; now: string; truncated?: boolean; error?: string }) {
  return createElement(MemoryRouter, {}, createElement(Timeline, { entries: entriesForView(sessions, events, now), window: { from: null, earlier: 0 }, ...props }));
}
import { failureText } from '../app/lib/labels.ts';
import type { TaskEvent } from '../server/task-history.ts';
const now = '2026-11-14T00:00:00Z';
const session: TimelineSession = { id: 's', label: '实现', round: 1, running: false, started_at: now, ended_at: now };
function check(reason: Record<string, unknown>, expected: string) {
  for (const trigger of ['session_ended', 'bug_session_ended']) {
    const events: TaskEvent[] = [
      { seq: 1, kind: 'session.started', subject: 'T-1', at: now, payload: { session_id: 's' } },
      { seq: 2, kind: 'task.updated', subject: 'T-1', at: now, payload: { trigger, state: { state: 'failed', at: 'implement', reason } } },
    ];
    const original = structuredClone(events);
    const entry = timelineEntries([session], events, now)[0];
    expect(entry.summary).toBe(expected);
    expect(entry.failures).toEqual([failureText(reason)]);
    const html = renderToStaticMarkup(createElement(TaskTimeline, { sessions: [session], events, now }));
    expect(html).toContain('<details class="timeline-failure"><summary>失败详情</summary><p>' + failureText(reason) + '</p></details>');
    expect(events).toEqual(original);
  }
}
it('门禁连续失败只投影日志路径', () => {
  check({ kind: 'gate_same_failure', count: 2, summary: '检查失败', log: '/tmp/gate.log' }, '门禁连续 2 次因同一原因失败：检查失败（日志：gate.log）');
});
it('门禁超时只投影日志路径', () => {
  check({ kind: 'gate_same_failure', count: 3, summary: '门禁运行超时，被内核终止', log: '/tmp/timeout.log' }, '门禁超时：连续 3 次运行超过时限被内核终止（日志：timeout.log）');
});
it('准备失败只投影日志路径', () => {
  check({ kind: 'setup', exit_code: 1, command: 'sh /tmp/setup.sh', log: '/tmp/setup.log' }, '准备命令失败（退出码 1）：sh /tmp/setup.sh（日志：setup.log）');
});
it('空和缺失路径不伪造路径', () => {
  for (const log of [undefined, '']) check({ kind: 'setup', exit_code: 1, command: 'test', ...(log === undefined ? {} : { log }) }, '准备命令失败（退出码 1）：test（日志：）');
});
it('纯文件名和中文空格路径保留文件名', () => {
  for (const log of ['中文 空格.log', '/tmp/中文 目录/中文 空格.log']) check({ kind: 'setup', exit_code: 1, command: 'test', log }, '准备命令失败（退出码 1）：test（日志：中文 空格.log）');
});
it('原始事件与共享 failureText 不变且不裁剪其它字段', () => {
  const reason = Object.freeze({ kind: 'gate_same_failure', count: 2, summary: '执行 /tmp/gate.log 失败', log: '/tmp/gate.log' });
  check(reason, '门禁连续 2 次因同一原因失败：执行 /tmp/gate.log 失败（日志：gate.log）');
  expect(failureText(reason)).toBe('门禁连续 2 次因同一原因失败：执行 /tmp/gate.log 失败（日志：/tmp/gate.log）');
});
