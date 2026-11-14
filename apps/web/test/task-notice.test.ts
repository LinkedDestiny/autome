import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { expect, test } from 'vitest';
import { noticeView } from '../app/components/task-notice-model.ts';
import { TaskNotice } from '../app/components/task-notice.tsx';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import type { CardContext } from '../app/components/task-card-model.ts';
process.env.TZ = 'Asia/Shanghai';
const ctx: CardContext = { timeZone: 'Asia/Shanghai', now: '2026-11-13T06:20:00Z', fixRounds: 3, cooldowns: [], unread: {} };
const p = (reason?: Record<string, unknown>): TaskPayload => ({ task: { id: 'T-1', title: '标题', kind: 'feature', state: reason ? { state: 'failed', at: 'gate', reason } : { state: 'active', node: 'implement' } }, sessions: [] });
const long = '状态块读取失败，保留错误原文与每一条提示。'.repeat(12);
const cases: [Record<string, unknown>, string][] = [
  [{ kind: 'design_budget', limit: 5 }, '设计轮用完（上限 5）'],
  [{ kind: 'impl_budget', limit: 30 }, '实现预算用完（N = 30）'],
  [{ kind: 'fix_budget', limit: 3 }, '修复轮次用完（上限 3）'],
  [{ kind: 'gate_same_failure', count: 3, summary: '错误全文', log: '/log' }, '门禁连续 3 次因同一原因失败：错误全文（日志：/log）'],
  [{ kind: 'gate_same_failure', count: 2, summary: '门禁运行超时，被内核终止', log: '/log' }, '门禁超时：连续 2 次运行超过时限被内核终止（日志：/log）'],
  [{ kind: 'infeasible' }, '设计文档判定不可实现'],
  [{ kind: 'protocol', detail: long }, `协议失败：${long}`],
  [{ kind: 'session_crashed', detail: '退出 92' }, '会话崩溃：退出 92'],
  [{ kind: 'rebase_conflict', detail: 'a.ts' }, 'rebase 冲突：a.ts'],
  [{ kind: 'config', detail: '角色关闭' }, '配置问题：角色关闭'],
  [{ kind: 'core_step', node: 'cleanup', detail: '读盘失败' }, '清理 失败：读盘失败'],
  [{ kind: 'setup', exit_code: 127, command: 'missing', log: '/setup.log' }, '准备命令失败（退出码 127）：missing（日志：/setup.log）'],
];
test('1 twelve failure branches preserve full reason in rendered notice', () => {
  for (const [reason, expected] of cases) {
    const v = noticeView(p(reason), ctx); expect(v.messages).toEqual([expected]); expect(v.failed).toBe(true);
    expect(renderToStaticMarkup(createElement(TaskNotice, { view: v }))).toContain(expected);
  }
  for (const kind of ['future', '', '__proto__']) expect(noticeView(p({ kind }), ctx).messages).toEqual([kind]);
});
test('2 next steps for every reason exclude unavailable extend', () => {
  for (const [reason] of cases) {
    const next = noticeView(p(reason), ctx).next;
    if (reason.kind === 'fix_budget') expect(next).toBe('追加轮次');
    else if (reason.kind === 'gate_same_failure') expect(next).toBe('从 门禁 重跑或追加轮次');
    else expect(next).toBe(['design_budget', 'impl_budget'].includes(String(reason.kind)) ? '重跑 / 追加轮次 / 取消' : '重跑 / 取消');
  }
});
test('3 cooldown matches exact deadline', () => {
  const data = { ...p(), retry: { attempts: 2, not_before: ctx.now } };
  expect(noticeView(data, { ...ctx, cooldowns: [{ runtime: 'codex', until: ctx.now }] }).messages).toEqual(['等 Codex 冷却到 14:20']);
});
test('4 retry without matching cooldown', () => {
  expect(noticeView({ ...p(), retry: { attempts: 2, not_before: ctx.now } }, { ...ctx, cooldowns: [{ runtime: 'codex', until: '2026-11-13T07:20:00Z' }] }).messages).toEqual(['等待重试（第 2 次）· 14:20']);
  expect(noticeView(p(), ctx).messages).toEqual([]);
});
test('5 gate queue first and later positions', () => {
  for (const n of [1, 2, 100]) expect(noticeView({ ...p(), gate_queue_position: n }, ctx).messages).toEqual([`门禁排队第 ${n} 位`]);
});
test('6 queued waits for slot', () => {
  const data = p(); data.task.state = { state: 'queued' };
  expect(noticeView(data, ctx).messages).toEqual(['排队中']);
  expect(noticeView({ ...data, queue_position: 1 }, ctx).messages).toEqual(['排队第 1 位']);
});
test('7 T70 stale and waiting_for optional fields', () => {
  const data = p(); data.task.state = { state: 'active', node: 'await_merge' };
  for (const merge of [undefined, null, { stale: null }]) { data.task.merge = merge; expect(noticeView(data, ctx).messages).toEqual([]); }
  data.task.merge = { stale: { behind: 3 }, waiting_for: 'T-2' };
  expect(noticeView(data, ctx).messages).toEqual(['等你合并，但默认分支已前进 3 个提交，按合并会先 rebase 并重跑合并前门禁', '合并前门禁在等 T-2 合并完']);
});
test('8 stopped and paused suggest resume, finished has no notice', () => {
  for (const state of [{ state: 'stopped', at: 'audit' }, { state: 'paused', resume: 'audit' }]) {
    const data = p(); data.task.state = state; const v = noticeView(data, ctx); expect(v.next).toBe('恢复'); expect(v.messages[0]).toContain('审计');
  }
  for (const state of ['done', 'cancelled']) { const data = p(); data.task.state = { state }; expect(renderToStaticMarkup(createElement(TaskNotice, { view: noticeView(data, ctx) }))).toBe(''); }
});
