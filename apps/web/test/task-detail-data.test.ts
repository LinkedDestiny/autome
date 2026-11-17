import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createRoutesStub } from 'react-router';
import TaskPage from '../app/routes/tasks.$id.tsx';
import { pinTimeline, withParams } from '../app/components/task-url.ts';
import { expect, it, vi } from 'vitest';
import { loadTaskDetail } from '../app/components/task-detail-data.ts';
const url = new URL('http://localhost/tasks/T-1');
const payload = { task: { id: 'T-1', title: '任务', request: '原文', state: { state: 'active', node: 'implement' } }, project: { id: 'P-1' }, sessions: [], brief: '卡片' };
it('读调用清单、合并段/授权/重试条件读取与降级', async () => {
  for (const node of ['implement', 'await_merge', 'rebase', 'merge_gate', 'merging', 'main_check', 'cleanup']) {
    const read = vi.fn().mockResolvedValueOnce({ status: 200, body: { ok: true, payload: { ...payload, task: { ...payload.task, state: { state: 'active', node } } } } }).mockResolvedValue({ status: 200, body: { ok: true, payload: {} } });
    const forTask = vi.fn().mockResolvedValue({ events: [], truncated: false });
    await loadTaskDetail({ gateway: { read, write: vi.fn() }, taskEvents: { forTask } }, 'T-1', url);
    expect(read.mock.calls.map(c => c[0].method)).toEqual(['task.get', 'config.get', ...(node === 'implement' ? [] : ['task.changes'])]); expect(forTask).toHaveBeenCalledWith('T-1');
  }
  const read = vi.fn().mockResolvedValueOnce({ status: 200, body: { ok: true, payload: { ...payload, retry: {}, task: { ...payload.task, merge: { authorized: true } } } } }).mockRejectedValue(Error('timeout'));
  const result = await loadTaskDetail({ gateway: { read, write: vi.fn() }, taskEvents: { forTask: vi.fn().mockRejectedValue(Error('offline')) } }, 'T-1', url);
  expect(read.mock.calls.map(c => c[0].method)).toEqual(['task.get', 'config.get', 'dashboard.get', 'task.changes']);
  expect(result.timeline).toMatchObject({ entries: [], error: expect.stringContaining('offline') }); expect(result.ops).toBeDefined(); expect(result.data).toBeDefined();
});
it('错误三句话不再读取其它来源', async () => {
  expect(await loadTaskDetail({ gateway: null }, 'T-1', url)).toEqual({ error: '内核离线，暂时拿不到任务' });
  for (const [status, code, message] of [[503, '', '内核离线，暂时拿不到任务'], [504, '', '内核离线，暂时拿不到任务'], [404, '', '找不到任务 T-1'], [400, 'not_found', '找不到任务 T-1'], [500, '', '无法读取任务：原文']] as const) {
    const read = vi.fn().mockResolvedValue({ status, body: { ok: false, error: '原文', code } });
    expect(await loadTaskDetail({ gateway: { read, write: vi.fn() } }, 'T-1', url)).toEqual({ error: message }); expect(read).toHaveBeenCalledTimes(1);
  }
});
it('无 doc 默认任务卡，null 回退原文，空正文仍保留', async () => {
  for (const brief of ['卡片', null, '']) {
    const read = vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload: { ...payload, brief } } }); const docRead = vi.fn();
    const result = await loadTaskDetail({ gateway: { read, write: vi.fn() }, taskDocs: { read: docRead } }, 'T-1', url);
    expect(result.document).toMatchObject({ name: null, text: brief ?? '原文', missingBrief: brief === null }); expect(docRead).not.toHaveBeenCalled();
  }
});
it('显式改动标签在 done 与取消状态按需读取', async () => {
  for (const state of ['done', 'cancelled']) {
    const read = vi.fn().mockResolvedValueOnce({ status: 200, body: { ok: true, payload: { ...payload, task: { ...payload.task, state: { state } } } } }).mockResolvedValue({ status: 200, body: { ok: true, payload: { merged: true } } });
    const result = await loadTaskDetail({ gateway: { read, write: vi.fn() } }, 'T-1', new URL(url + '?tab=changes'));
    expect(read.mock.calls.some(([request]) => request.method === 'task.changes')).toBe(true);
    if (state === 'done') expect(result.changes?.payload).toEqual({ merged: true });
  }
});

function windowFixture(count: number) {
  const sessions = Array.from({ length: count }, (_, i) => ({ id: 's' + i, label: '实现', round: i + 1, running: false, started_at: new Date(Date.UTC(2026, 0, 1, 0, i)).toISOString() }));
  const read = vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload: { ...payload, sessions } } });
  return { gateway: { read, write: vi.fn() }, taskEvents: { forTask: vi.fn().mockResolvedValue({ events: [0, count - 1].map((i, seq) => ({ seq, at: sessions[i]?.started_at, subject: 'T-1', kind: 'task.updated', payload: { trigger: 'pause' } })), truncated: false }) } };
}
it('未选中的文档/改动不读取，切换标签按需取且读盘拒绝保留', async () => {
  for (const tab of ['docs', 'changes', 'notes', 'decisions']) {
    const read = vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload: { ...payload, documents: [{ name: 'x.md', absolute: '/x' }] } } });
    const docRead = vi.fn().mockResolvedValue({ error: 'EACCES' });
    const result = await loadTaskDetail({ gateway: { read, write: vi.fn() }, taskDocs: { read: docRead } }, 'T-1', new URL(url + '?tab=' + tab + '&doc=x.md'));
    expect(read.mock.calls.some(([r]) => r.method === 'task.changes')).toBe(tab === 'changes');
    expect(docRead).toHaveBeenCalledTimes(tab === 'docs' ? 1 : 0);
    if (tab === 'docs') expect(result.document?.error).toBe('EACCES');
    else expect(result.document?.nodes).toEqual([]);
  }
});
it('窗口边界、较早数量、from 钉住新增会话及非法值回退', async () => {
  for (const count of [0, 1, 19, 20, 21, 60]) {
    const result = await loadTaskDetail(windowFixture(count), 'T-1', url);
    expect(result.timeline?.entries.filter(e => e.session)).toHaveLength(Math.min(20, count));
    expect(result.window).toEqual({ from: count ? 's' + Math.max(0, count - 20) : null, earlier: Math.max(0, count - 20) });
    if (count > 20) expect(result.timeline?.entries.filter(e => !e.session)).toHaveLength(1);
  }
  for (const from of ['s40', 's0', 'foreign', '', 's040']) {
    const result = await loadTaskDetail(windowFixture(65), 'T-1', new URL(url + '?from=' + from));
    const start = from === 's40' ? 40 : from === 's0' ? 0 : 45;
    expect(result.window?.earlier).toBe(start);
    expect(result.timeline?.entries.filter(e => e.session).map(e => e.key)).toEqual(Array.from({ length: 65 - start }, (_, i) => 's' + (i + start)));
  }
  const all = await loadTaskDetail(windowFixture(65), 'T-1', new URL(url + '?timeline=all&from=s40'));
  expect(all.timeline?.entries.filter(e => e.session)).toHaveLength(65);
});
it('文档和标签链接保留 from/timeline 且不改输入', () => {
  const input = new URLSearchParams('tab=notes&doc=old&from=s0&timeline=all');
  for (const set of [{ tab: 'docs', doc: '中文 &.md' }, { tab: 'changes' }] as Record<string, string>[]) {
    const next = withParams(input, { set }); expect(next.get('from')).toBe('s0'); expect(next.get('timeline')).toBe('all');
  }
  expect(withParams(input, { set: { tab: 'docs' }, remove: ['doc'] }).has('doc')).toBe(false);
  expect(input.get('doc')).toBe('old');
});
it('显示更早保留 tab/doc 并删除 from', () => {
  const next = withParams(new URLSearchParams('tab=docs&doc=x&from=s1'), { set: { timeline: 'all' }, remove: ['from'] });
  expect([...next]).toEqual([['tab', 'docs'], ['doc', 'x'], ['timeline', 'all']]);
});
it('钉住保留 tab/doc，已有 from 或 timeline=all 不补', () => {
  const input = new URLSearchParams('tab=docs&doc=x');
  expect([...pinTimeline(input, 's1')]).toEqual([['tab', 'docs'], ['doc', 'x'], ['from', 's1']]);
  for (const query of ['from=s0', 'from=', 'timeline=all']) { const search = new URLSearchParams(query); expect(pinTimeline(search, 's1')).toBe(search); }
  expect(pinTimeline(input, null)).toBe(input);
});
it('文档名空串与路径拒绝，缺少读取服务有明确提示', async () => {
  for (const name of ['', '../x', '/x', 'missing']) {
    const result = await loadTaskDetail(windowFixture(0), 'T-1', new URL(url + '?doc=' + encodeURIComponent(name)));
    expect(result.document?.error).toBeTruthy();
  }
});

it('已实现后重跑设计的依赖只读原因不受会话窗口裁剪影响', async () => {
  const sessions = Array.from({ length: 25 }, (_, i) => ({ id: 's' + i, label: '会话', round: i + 1, running: false, started_at: new Date(Date.UTC(2026, 0, 1, 0, i)).toISOString(), kind: { kind: 'role', role: i === 0 ? 'impl' : 'plan' } }));
  const read = vi.fn().mockResolvedValue({ status: 200, body: { ok: true, payload: { ...payload, task: { ...payload.task, kind: 'feature', state: { state: 'active', node: 'design' } }, sessions } } });
  const data = await loadTaskDetail({ gateway: { read, write: vi.fn() } }, 'T-1', new URL(url + '?tab=notes'));
  expect(data.window?.earlier).toBe(5);
  const Stub = createRoutesStub([{ id: 'task', path: '/tasks/:id', Component: TaskPage }]);
  const html = renderToStaticMarkup(createElement(Stub, { initialEntries: ['/tasks/T-1?tab=notes'], hydrationData: { loaderData: { task: data } } }));
  expect(html).toContain('依赖只能在设计批准之前修改'); expect(html).not.toContain('保存依赖');
});
it('汇总使用全量会话但仅传输窗口，阶段组只传 key', async () => {
  const result = await loadTaskDetail(windowFixture(60), 'T-1', url);
  expect(result.summary?.sessions.total).toBe(60); expect(result.data?.sessions).toEqual([]);
  expect(result.timeline?.entries.filter(e=>e.session)).toHaveLength(20);
  expect(result.timeline?.groups.flatMap(g=>g.entries).every(key=>typeof key==='string')).toBe(true);
});
