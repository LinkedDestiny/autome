import { expect, it, vi } from 'vitest';
import { createTaskEvents, type TaskEvent } from '../server/task-events.ts';
const event = (seq: number, subject = 'T-1', kind = 'task.updated'): TaskEvent => ({ seq, subject, kind, payload: {}, at: '' });
function ledger(events: TaskEvent[]) {
  const call = vi.fn(async (_method: string, params: Record<string, unknown>) => ({ events: events.filter(e => e.seq > Number(params.after_seq)).slice(0, 1000), latest_seq: events.at(-1)?.seq ?? 0 }));
  return { call, source: createTaskEvents({ call }) };
}
it('首次分页并只返回本任务，空账本与单元素', async () => {
  const { call, source } = ledger(Array.from({ length: 1001 }, (_, i) => event(i + 1, i % 2 ? 'T-2' : 'T-1')));
  const result = await source.forTask('T-1');
  expect(result.events).toHaveLength(501); expect(result.events.every(e => e.subject === 'T-1')).toBe(true);
  expect(call.mock.calls.map(c => c[1].after_seq)).toEqual([0, 1000]);
  expect(await ledger([]).source.forTask('T-1')).toEqual({ events: [], truncated: false });
  expect((await ledger([event(1)]).source.forTask('T-1')).events).toEqual([event(1)]);
});
it('增量只拉游标之后，返回值无法污染缓存', async () => {
  const events = [event(1)]; const { call, source } = ledger(events);
  (await source.forTask('T-1')).events[0].payload.changed = true;
  events.push(event(2)); expect((await source.forTask('T-1')).events).toEqual(events);
  expect(call.mock.calls[1][1].after_seq).toBe(1);
});
it('账本被换后清空重建，包括空账本', async () => {
  const events = [event(3)]; const { source } = ledger(events);
  await source.forTask('T-1'); events.splice(0, 1, event(1, 'T-2'));
  expect((await source.forTask('T-1')).events).toEqual([]);
  expect((await source.forTask('T-2')).events).toEqual([event(1, 'T-2')]);
  events.length = 0; expect((await source.forTask('T-2')).events).toEqual([]);
});
it('并发共用补齐，close 后拒绝', async () => {
  const { source, call } = ledger([event(1)]);
  await Promise.all([source.forTask('T-1'), source.forTask('T-2')]); expect(call).toHaveBeenCalledTimes(1);
  source.close(); await expect(source.forTask('T-1')).rejects.toThrow('关闭');
});
it('噪声丢弃，5000 边界与超限截断', async () => {
  const events = Array.from({ length: 5000 }, (_, i) => event(i + 1)); const { source } = ledger(events);
  expect((await source.forTask('T-1')).truncated).toBe(false);
  events.push(event(5001), event(5002, 'T-1', 'task.head'), event(5003, 'T-1', 'integrity_warning'), event(5004, 'T-1', 'task.lessons'));
  const result = await source.forTask('T-1'); expect(result.truncated).toBe(true); expect(result.events).toHaveLength(5000); expect(result.events[0].seq).toBe(2); expect(result.events.at(-1)?.seq).toBe(5001);
});
it('第二页失败不推进游标或丢数据；空页不能死循环', async () => {
  const events = Array.from({ length: 1001 }, (_, i) => event(i + 1)); const { source, call } = ledger(events);
  const normal = call.getMockImplementation()!;
  call.mockImplementationOnce(normal).mockRejectedValueOnce(Error('offline'));
  await expect(source.forTask('T-1')).rejects.toThrow('offline');
  expect((await source.forTask('T-1')).events).toEqual(events); expect(call.mock.calls[2][1].after_seq).toBe(0);
  await expect(createTaskEvents({ call: async () => ({ events: [], latest_seq: 1 }) }).forTask('T-1')).rejects.toThrow('未前进');
});
