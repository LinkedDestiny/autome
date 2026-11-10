import { expect, test, vi } from 'vitest';
import { createEventBus } from '../server/eventbus.ts';
import type { LedgerEvent, ServerMessage } from '../server/eventbus-protocol.ts';
const event = (seq: number): LedgerEvent => ({ seq, kind: 'task.updated', subject: 'T-1', payload: {} });
function fixture(initial = 0) {
  let latest = initial;
  const call = vi.fn(async (_method: string, params: Record<string, unknown>) => {
    const after = params.after_seq as number;
    return { latest_seq: latest, events: Array.from({ length: Math.min(1000, Math.max(0, latest - after)) }, (_, i) => event(after + i + 1)) };
  });
  const log = vi.fn();
  const bus = createEventBus({ call, log });
  function connect() {
    const messages: ServerMessage[] = [];
    const close = vi.fn();
    const disconnect = bus.connect({ send: text => messages.push(JSON.parse(text)), close });
    return { messages, close, disconnect, seqs: () => messages.filter(m => m.type === 'event').map(m => m.seq) };
  }
  return { bus, call, log, connect, setLatest: (n: number) => { latest = n; } };
}
test('首次初始化只取最新游标，不推历史', async () => {
  const f = fixture(10); const c = f.connect(); await f.bus.pull();
  expect(f.call.mock.calls[0]).toEqual(['events.since', { after_seq: Number.MAX_SAFE_INTEGER }]);
  expect(c.seqs()).toEqual([]); expect(f.connect().messages[0]).toMatchObject({ seq: 10 });
});
test('2500 条分三页全部广播', async () => {
  const f = fixture(); await f.bus.pull(); f.call.mockClear(); const c = f.connect(); f.setLatest(2500); await f.bus.pull();
  expect(f.call.mock.calls.map(c => c[1])).toEqual([0, 1000, 2000].map(after_seq => ({ after_seq, limit: 1000 })));
  expect(c.seqs()).toEqual(Array.from({ length: 2500 }, (_, i) => i + 1)); expect(f.connect().messages[0]).toMatchObject({ seq: 2500 });
});
test('并发触发合并成一次补拉且请求单飞', async () => {
  const f = fixture(); await f.bus.pull(); f.call.mockClear();
  let release!: () => void;
  f.call.mockImplementationOnce(async () => { await new Promise<void>(r => { release = r; }); return { events: [], latest_seq: 0 }; });
  const first = f.bus.pull(); const second = f.bus.pull(); f.bus.pull();
  expect(first).toBe(second); expect(f.call).toHaveBeenCalledTimes(1); release(); await first; expect(f.call).toHaveBeenCalledTimes(2);
});
test('失败保留游标并输出中文，下次续拉', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect(); f.setLatest(6);
  f.call.mockRejectedValueOnce(new Error('读盘失败')); await f.bus.pull();
  expect(f.log).toHaveBeenCalledWith('[事件] 拉取失败：读盘失败'); expect(c.seqs()).toEqual([]);
  await f.bus.pull(); expect(f.call.mock.lastCall?.[1]).toEqual({ after_seq: 5, limit: 1000 }); expect(c.seqs()).toEqual([6]);
});
test('重复页不重复发送，连接严格递增', async () => {
  const f = fixture(); await f.bus.pull(); const c = f.connect(); f.setLatest(2); await f.bus.pull();
  f.call.mockResolvedValueOnce({ events: [event(1), event(2), event(3)], latest_seq: 3 }); await f.bus.pull(); expect(c.seqs()).toEqual([1, 2, 3]);
});
test('新连接先 hello，只收更大序号', async () => {
  const f = fixture(4); f.bus.setCoreStatus({ state: 'connected', attempts: 0 }); await f.bus.pull(); const c = f.connect();
  expect(c.messages).toEqual([{ type: 'hello', seq: 4, core: 'connected' }]); f.setLatest(5); await f.bus.pull(); expect(c.seqs()).toEqual([5]);
  c.disconnect(); f.setLatest(6); await f.bus.pull(); expect(c.seqs()).toEqual([5]);
});
test('订阅按序通知，退订停止', async () => {
  const f = fixture(); await f.bus.pull(); const seen: number[] = []; const unsubscribe = f.bus.subscribe(e => seen.push(e.seq));
  f.setLatest(2); await f.bus.pull(); unsubscribe(); f.setLatest(3); await f.bus.pull(); expect(seen).toEqual([1, 2]);
});
test('关闭连接并停止拉取，丢弃在途结果', async () => {
  const f = fixture(); await f.bus.pull(); const c = f.connect(); let release!: () => void;
  f.call.mockImplementationOnce(async () => { await new Promise<void>(r => { release = r; }); return { events: [event(1)], latest_seq: 1 }; });
  const pending = f.bus.pull(); f.bus.close(); release(); await pending; f.call.mockClear(); await f.bus.pull();
  expect(f.call).not.toHaveBeenCalled(); expect(c.close).toHaveBeenCalledWith(1001, '服务正在关闭'); expect(c.seqs()).toEqual([]);
});
