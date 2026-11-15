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
  expect(result.history).toMatchObject({ events: [], error: expect.stringContaining('offline') }); expect(result.config).toMatchObject({ error: expect.stringContaining('timeout') }); expect(result.data).toBeDefined();
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
it('done 也读 changes，取消不读取', async () => {
  for (const state of ['done', 'cancelled']) {
    const read = vi.fn().mockResolvedValueOnce({ status: 200, body: { ok: true, payload: { ...payload, task: { ...payload.task, state: { state } } } } }).mockResolvedValue({ status: 200, body: { ok: true, payload: { merged: true } } });
    const result = await loadTaskDetail({ gateway: { read, write: vi.fn() } }, 'T-1', url);
    expect(read.mock.calls.some(([request]) => request.method === 'task.changes')).toBe(state === 'done');
    if (state === 'done') expect(result.changes?.payload).toEqual({ merged: true });
  }
});
