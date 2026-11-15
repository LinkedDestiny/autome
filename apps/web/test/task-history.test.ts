import { expect, it, vi } from 'vitest';
import { readTaskEvents, type TaskEvent } from '../server/task-history.ts';
const event = (seq: number, kind = 'session.started'): TaskEvent => ({ seq, kind, subject: 'T-1', payload: {}, at: '2026-11-15T00:00:00Z' });
const page = (start: number, count: number, kind?: string) => Array.from({ length: count }, (_, i) => event(start + i, kind));
it('每次调用带 subject，从零重读，不缓存', async () => {
  const call = vi.fn().mockResolvedValue({ events: [event(4)], latest_seq: 99 });
  await readTaskEvents(call, 'T-1'); await readTaskEvents(call, 'T-2');
  expect(call.mock.calls).toEqual(['T-1', 'T-2'].map(subject => ['events.since', { subject, after_seq: 0, limit: 1000 }]));
});
it('短页和整页后空页结束，游标在过滤噪声前推进', async () => {
  for (const kind of ['session.started', 'task.head']) for (const tail of [[], [event(2001)]]) {
    const call = vi.fn().mockResolvedValueOnce({ events: page(1, 1000, kind), latest_seq: 9000 }).mockResolvedValueOnce({ events: tail, latest_seq: 9000 });
    const result = await readTaskEvents(call, 'T-1');
    expect(call).toHaveBeenCalledTimes(2);
    expect(call.mock.calls[1][1]).toEqual({ subject: 'T-1', after_seq: 1000, limit: 1000 });
    expect(result.events).toHaveLength((kind === 'task.head' ? 0 : 1000) + tail.length);
  }
});
it('其它任务水位更大及没有事件的任务正常结束，不依赖 latest_seq', async () => {
  for (const events of [[], [event(1)]]) for (const latest_seq of [0, 10000]) {
    const call = vi.fn().mockResolvedValueOnce({ events, latest_seq }).mockRejectedValue(Error('不应再取下一页'));
    expect(await readTaskEvents(call, 'T-1')).toEqual({ events, truncated: false });
    expect(call).toHaveBeenCalledTimes(1);
  }
});
it('保留最新 5000，5000 边界不截断，5001 开始标截断', async () => {
  for (const count of [5000, 5001, 6001]) {
    const all = page(1, count);
    const call = vi.fn(async (_method, params) => ({ events: all.filter(e => e.seq > params.after_seq).slice(0, params.limit), latest_seq: count + 100 }));
    const result = await readTaskEvents(call, 'T-1');
    expect(result).toEqual({ events: all.slice(-5000), truncated: count > 5000 });
  }
});
it('过滤三类噪声，读取失败直接拒绝，下次从零完整读取', async () => {
  const events = ['task.head', 'integrity_warning', 'task.lessons', 'session.ended'].map((kind, i) => event(i + 1, kind));
  const call = vi.fn().mockRejectedValueOnce(Error('offline')).mockResolvedValue({ events, latest_seq: 4 });
  await expect(readTaskEvents(call, 'T-1')).rejects.toThrow('offline');
  expect((await readTaskEvents(call, 'T-1')).events).toEqual([events[3]]);
  expect(call.mock.calls[1][1].after_seq).toBe(0);
});
it('非空页最后 seq 未前进时报错，包括回退与噪声页', async () => {
  for (const seq of [0, -1]) for (const kind of ['task.head', 'session.started']) {
    await expect(readTaskEvents(async () => ({ events: [event(seq, kind)], latest_seq: 9999 }), 'T-1')).rejects.toThrow('任务事件暂不可用：账本分页未前进');
  }
});
