import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { MemoryRouter } from 'react-router';
import { expect, test, vi } from 'vitest';
import { cardView, type CardContext, type TaskDetail } from '../app/components/task-card-model.ts';
import { TaskCard } from '../app/components/task-card.tsx';
import { loadBoard, needYouKinds, type BoardTask } from '../app/components/board-data.ts';
import { AWAIT_PHRASES, FEATURE_STAGES, BUG_STAGES } from '../app/lib/labels.ts';
import { NOTIFICATION_RULES, renderReason } from '../server/notifier-rules.ts';
import { openWebStore } from '../server/store.ts';
import type { KernelGateway } from '../app/load-context.ts';
process.env.TZ = 'Asia/Shanghai';
const ctx: CardContext = { timeZone: 'Asia/Shanghai', now: '2026-11-13T06:20:00Z', fixRounds: 3, cooldowns: [], unread: {} };
const task = (node = 'implement', kind = 'feature'): BoardTask => ({ id: 'T-1', title: '任务标题', project_id: 'P-1', kind, state: { state: 'active', node } });
const block = { milestones_done: 3, milestones_total: 5, current_milestone: 'M-02' };
const view = (t = task(), d?: TaskDetail, context = ctx) => cardView(t, d, context);
const html = (t: BoardTask, d?: TaskDetail) => renderToStaticMarkup(createElement(MemoryRouter, null, createElement(TaskCard, { task: t, detail: d, context: ctx })));
test('1 feature seven stages and unknown nodes', () => {
  for (const [i, stage] of FEATURE_STAGES.entries()) for (const node of stage.nodes) {
    for (const state of [{ state: 'active', node }, { state: 'paused', resume: node }, { state: 'queued', resume: node }, { state: 'stopped', at: node }, { state: 'failed', at: node }]) {
      const v = view({ ...task(node), state }); expect(v.stages).toHaveLength(7); expect(v.stages[i].current).toBe(true);
      expect(v.stages.filter(s => s.passed)).toHaveLength(i);
    }
  }
  for (const node of ['future_node', '', '__proto__']) { const v = view(task(node)); expect(v.unknownStage).toBe(node); expect(v.stages.some(s => s.current || s.passed)).toBe(false); }
  expect(view({ ...task(), state: { state: 'done' } }).stages[6].current).toBe(true);
  for (const state of ['draft', 'cancelled', 'future']) expect(view({ ...task(), state: { state } }).stages.some(s => s.current)).toBe(false);
  expect(html(task())).toContain('aria-current="step"'); expect(html(task())).toContain('href="/tasks/T-1"');
});
test('2 bug five stages across every node and stopped/paused/queued/failed', () => {
  for (const [index, stage] of BUG_STAGES.entries()) for (const node of stage.nodes) {
    for (const state of [{ state: 'active', node }, { state: 'paused', resume: node }, { state: 'queued', resume: node }, { state: 'stopped', at: node }, { state: 'failed', at: node }]) {
      const v = view({ ...task(node, 'bug'), state }); expect(v.stages).toHaveLength(5); expect(v.stages[index].current).toBe(true);
    }
  }
  expect(view({ ...task('', 'bug'), state: { state: 'done' } }).stages[4].current).toBe(true);
});
test('3 feature implementation milestone progress only', () => {
  expect(view(task(), { status_block: block }).progress).toBe('已完成 3 / 5');
  expect(view(task('audit'), { status_block: block }).progress).toBeNull();
  expect(view(task(), { status_block: { ...block, milestones_total: 0 } }).progress).toBeNull();
  expect(view(task()).progress).toBeNull();
  expect(view(task(), { status_block: { ...block, milestones_done: 1, milestones_total: 1 } }).progress).toBe('已完成 1 / 1');
});
test('4 bug fix rounds never use milestone block', () => {
  const t = { ...task('fix', 'bug'), fix_round: 2 };
  expect(view(t, { status_block: block }).progress).toBe('修复第 2 / 3 轮');
  expect(html(t, { status_block: block })).not.toContain('已完成 3 / 5');
  expect(view(task('fix_review', 'bug'), { status_block: block }).progress).toBeNull();
});
test('5 running session activity and minute boundaries', () => {
  const detail = { status_block: block, sessions: [{ label: '实现', round: 3, running: true, started_at: '2026-11-13T06:08:00Z' }] };
  expect(view(task(), detail).activity).toBe('实现第 3 轮 · M-02 · 运行 12 分');
  expect(view(task('fix', 'bug'), detail).activity).toBe('实现第 3 轮 · 运行 12 分');
  for (const started_at of [ctx.now, '2026-11-13T06:19:01Z', '2026-11-13T06:21:00Z']) expect(view(task(), { sessions: [{ ...detail.sessions[0], started_at }] }).activity).toBe('实现第 3 轮 · 刚开始');
  expect(view(task(), { sessions: [{ ...detail.sessions[0], running: false }] }).activity).toBe('实现');
});
test('6 cooldown exact timestamp match and retry fallback', () => {
  const detail = { retry: { attempts: 2, not_before: ctx.now } };
  expect(view(task(), detail, { ...ctx, cooldowns: [{ runtime: 'claude', until: ctx.now }] }).activity).toBe('等 Claude Code 冷却到 14:20');
  expect(view(task(), detail, { ...ctx, cooldowns: [{ runtime: 'codex', until: '2026-11-13T06:21:00Z' }] }).activity).toBe('等待重试（第 2 次）· 14:20');
});
test('7 waiting priority, failure reasons and queue positions', () => {
  for (const [node, phrase] of Object.entries(AWAIT_PHRASES)) expect(view(task(node), { retry: { attempts: 2, not_before: ctx.now } }).activity).toBe(phrase);
  const reason = { kind: 'session_crashed', detail: '会话退出码 92' };
  expect(view({ ...task(), state: { state: 'failed', at: 'intake', reason } }).activity).toBe(`失败：${renderReason(reason)}`);
  expect(view({ ...task(), state: { state: 'queued' } }, { queue_position: 1 }).activity).toBe('排队第 1 位');
  expect(view({ ...task(), state: { state: 'queued' } }).activity).toBe('排队中');
});
test('8 gate queue, absent, pass, fail and recorded done gate plus merge', () => {
  expect(view(task(), { gate_queue_position: 2 }).gate).toBe('门禁排队第 2 位');
  expect(view(task(), { gate: null }).gate).toBe('门禁未跑过');
  expect(view({ ...task(), state: { state: 'draft' } }).gate).toBe('门禁未跑过');
  expect(view(task(), { gate: { ok: true, node: 'gate', exit_code: 0 } }).gate).toBe('门禁通过 · 门禁');
  expect(view(task(), { gate: { ok: false, node: 'gate', exit_code: 7 } }).gate).toBe('门禁未通过 · 退出码 7');
  const recorded = JSON.parse(readFileSync(new URL('./fixtures/w6-task-get.json', import.meta.url), 'utf8'));
  expect(recorded.task.state.state).toBe('done'); expect(recorded.gate.ok).toBe(true);
  const rendered = html(recorded.task, recorded); expect(rendered).toContain('门禁通过 · 验证 main'); expect(rendered).toContain(`已合并 · ${recorded.task.merge_commit.slice(0, 7)}`);
  expect(view(recorded.task, { ...recorded, gate: null }).gate).toBe('门禁未跑过');
});
test('9 needs-you badge with and without unread counts', () => {
  for (const count of [0, 1, 2]) {
    const context = { ...ctx, unread: { 'T-1': count } };
    expect(view(task('await_merge'), undefined, context).badge).toBe(`等你处理${count > 0 ? ` ${count}` : ''}`);
    for (const state of ['draft', 'queued', 'done', 'cancelled']) expect(view({ ...task(), state: { state } }, undefined, context).badge).toBeNull();
    expect(view(task(), undefined, context).badge).toBeNull();
  }
});
test('10 loader omits draft and completed details and fetches dashboard only for retries', async () => {
  vi.useFakeTimers(); vi.setSystemTime(new Date(ctx.now));
  try {
    const project = JSON.parse(readFileSync(new URL('./fixtures/w5-project-get.json', import.meta.url), 'utf8'));
    const detail = JSON.parse(readFileSync(new URL('./fixtures/w6-task-get.json', import.meta.url), 'utf8'));
    project.tasks.push({ ...detail.task, id: 'recent', completed_at: ctx.now }, { ...detail.task, id: 'older', completed_at: '2020-01-01T00:00:00Z' }, { ...task(), id: 'draft', state: { state: 'draft' } });
    const calls: { method: string; params?: unknown }[] = [];
    let retry = false;
    const gateway: KernelGateway = { read: async request => { calls.push(request as { method: string; params?: unknown }); const method = (request as { method: string }).method; return { status: 200, body: { ok: true, payload: method === 'project.get' ? project : method === 'dashboard.get' ? { cooldowns: [] } : { ...detail, retry: retry ? { attempts: 2, not_before: ctx.now } : null } } }; }, write: async () => { throw Error('unexpected write'); } };
    for (const expanded of [false, true]) {
      calls.length = 0; const loaded = await loadBoard({ gateway }, 'P-1', expanded); expect('error' in loaded).toBe(false);
      const ids = calls.filter(c => c.method === 'task.get').map(c => (c.params as { task_id: string }).task_id);
      expect(ids).not.toContain('recent'); expect(ids).not.toContain('older'); expect(ids).not.toContain('draft');
      expect(calls.filter(c => c.method === 'dashboard.get')).toHaveLength(0);
      if (!('error' in loaded)) expect(loaded.details?.recent).toBeUndefined();
    }
    retry = true; calls.length = 0; await loadBoard({ gateway }, 'P-1'); expect(calls.filter(c => c.method === 'dashboard.get')).toHaveLength(1);
    for (const failedMethod of ['task.get', 'dashboard.get']) for (const status of [409, 503, 504]) {
      const rejected: KernelGateway = { ...gateway, read: async request => (request as { method: string }).method === failedMethod ? { status, body: { ok: false, error: '详情读取失败' } } : gateway.read(request) };
      const result = await loadBoard({ gateway: rejected }, 'P-1');
      if (failedMethod === 'task.get') { expect(result).not.toHaveProperty('error'); expect(result).toHaveProperty('details', {}); expect(result).toHaveProperty('tasks', expect.arrayContaining([expect.objectContaining({ id: 'draft' })])); }
      else expect(result).toEqual({ error: '详情读取失败' });
    }
  } finally { vi.useRealTimers(); }
});
test('11 needs-you kinds come from notification rules', () => {
  expect(needYouKinds()).toEqual(NOTIFICATION_RULES.filter(r => r.level === 'need_you').map(r => r.kind));
  expect(needYouKinds()).toContain('not_a_bug'); expect(needYouKinds()).not.toContain('done');
});
test('12 actual SQLite unread counts feed the loader and badge', async () => {
  const root = mkdtempSync(join(tmpdir(), 'w6-unread-')); const store = openWebStore(root);
  try {
    for (const [i, kind] of ['await_merge', 'await_merge', 'done', 'await_merge'].entries()) {
      const row = store.commit({ cursor: { name: 'test', seq: i }, notification: { kind, project_id: 'P-1', task_id: 'T-1', title: kind, body: null, event_seq: i } });
      if (i === 3) store.markRead([row!.id]);
    }
    const gateway: KernelGateway = { read: async request => ({ status: 200, body: { ok: true, payload: (request as { method: string }).method === 'project.get' ? { project: { id: 'P-1', display_name: '项目' }, tasks: [task('await_merge')] } : {} } }), write: async () => { throw Error('unexpected write'); } };
    const loaded = await loadBoard({ gateway, unread: store }, 'P-1'); if ('error' in loaded) throw Error(loaded.error);
    expect(loaded.cardContext!.unread['T-1']).toBe(store.unreadByTask(needYouKinds())[0].count);
    expect(view(loaded.tasks[0], loaded.details!['T-1'], loaded.cardContext).badge).toBe('等你处理 2');
    store.markAllRead(); const read = await loadBoard({ gateway, unread: store }, 'P-1'); if ('error' in read) throw Error(read.error);
    expect(view(read.tasks[0], read.details!['T-1'], read.cardContext).badge).toBe('等你处理');
  } finally { store.close(); rmSync(root, { recursive: true, force: true }); }
});

test('13 卡片显示依赖等待且使用项目队列位置', () => {
  const t = { ...task(), state: { state: 'queued' }, queue_position: 2, blocked_by: ['T-3', 'T-5'].map(id => ({ id, state: { state: 'queued' }, reason: null })) };
  expect(view(t).dependency).toBe('等 T-3、T-5 合并');
  expect(html(t)).toContain('等 T-3、T-5 合并'); expect(html(t)).toContain('排队第 2 位');
});
test('14 取消或失败前置给警告且无依赖不显示', () => {
  for (const [reason, label] of [['cancelled', '已取消'], ['failed', '已失败']]) {
    const t = { ...task(), blocked_by: [{ id: 'T-3', state: { state: reason }, reason }] };
    expect(html(t)).toContain(`T-3 ${label}，依赖永远等不到——去详情页修改依赖`);
    expect(html(t)).toContain('dependency-warning');
  }
  expect(view(task()).dependency).toBeNull();
});
