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
    const transport = { send: (text: string) => messages.push(JSON.parse(text)), close };
    const disconnect = bus.connect(transport);
    return { messages, close, disconnect, receive: (text: string) => bus.receive(transport, text), resume: (after: number) => bus.receive(transport, JSON.stringify({ type: 'resume', after })), seqs: () => messages.filter(m => m.type === 'event').map(m => m.seq) };
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
test('相同检查点无消息；领先游标的客户端检查点跳过已收事件', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect(); c.resume(5);
  expect(c.messages).toHaveLength(1);
  f.setLatest(7); c.resume(6); await f.bus.pull(); expect(c.seqs()).toEqual([7]);
});
test('小缺口分页补发与期间 live 排队合并，严格递增', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect();
  let release!: () => void;
  f.call.mockImplementationOnce(async () => { await new Promise<void>(r => { release = r; }); return { events: [event(6)], latest_seq: 6 }; });
  c.resume(2); f.setLatest(7); const pending = f.bus.pull(); release(); await pending;
  expect(c.seqs()).toEqual([3, 4, 5, 6, 7]);
  f.setLatest(8); await f.bus.pull(); expect(c.seqs()).toEqual([3, 4, 5, 6, 7, 8]);
});
test('缺口 1000 补发，1001 只 reload', async () => {
  for (const gap of [1000, 1001]) {
    const f = fixture(gap); await f.bus.pull(); const c = f.connect(); c.resume(0); await f.bus.pull();
    if (gap === 1000) expect(c.seqs()).toEqual(Array.from({ length: 1000 }, (_, i) => i + 1));
    else expect(c.messages).toEqual([{ type: 'hello', seq: gap, core: 'offline', reason: '服务正在关闭' }, { type: 'reload' }]);
    f.setLatest(gap + 1); await f.bus.pull(); expect(c.seqs().at(-1)).toBe(gap + 1);
  }
});
test('检查点超过账本 reload 后接受较小的新序号', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect(); c.resume(100); await f.bus.pull();
  expect(c.messages.at(-1)).toEqual({ type: 'reload' }); f.setLatest(6); await f.bus.pull(); expect(c.seqs()).toEqual([6]);
});
test('hello 后发过事件再请求旧检查点只 reload', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect(); f.setLatest(6); await f.bus.pull();
  c.resume(2); expect(c.messages.at(-1)).toEqual({ type: 'reload' }); f.setLatest(7); await f.bus.pull(); expect(c.seqs()).toEqual([6, 7]);
});
test('续传内核不可用、超时或读盘失败，成功拉取后重做', async () => {
  for (const reason of ['内核不可用', '请求超时', '读盘失败']) {
    const f = fixture(5); await f.bus.pull(); const c = f.connect();
    f.call.mockResolvedValueOnce({ events: [], latest_seq: 5 }).mockRejectedValueOnce(new Error(reason));
    c.resume(2); await vi.waitFor(() => expect(f.log).toHaveBeenCalledWith(`[事件] 续传失败：${reason}`));
    expect(c.seqs()).toEqual([]); f.setLatest(6); await f.bus.pull(); expect(c.seqs()).toEqual([3, 4, 5, 6]);
  }
});
test('五种 core 状态和原因变化推送，同值去重', async () => {
  const f = fixture(); await f.bus.pull(); const c = f.connect();
  for (const state of ['starting', 'connected', 'restarting', 'offline', 'stopped'] as const) {
    f.bus.setCoreStatus({ state, attempts: 0 }); f.bus.setCoreStatus({ state, attempts: 0 }); await f.bus.pull();
  }
  expect(c.messages.filter(m => m.type === 'core').map(m => m.status)).toEqual(['offline', 'connected', 'offline', 'offline', 'offline']);
  f.bus.setCoreStatus({ state: 'offline', attempts: 0, reason: '读取失败' });
  expect(c.messages.at(-1)).toEqual({ type: 'core', status: 'offline', reason: '读取失败' });
});
test('恢复后立即从原游标补拉离线事件，不丢不重', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect();
  f.bus.setCoreStatus({ state: 'offline', attempts: 0 }); f.setLatest(7);
  f.bus.setCoreStatus({ state: 'connected', attempts: 0 });
  await vi.waitFor(() => expect(c.seqs()).toEqual([6, 7])); await f.bus.pull(); expect(c.seqs()).toEqual([6, 7]);
});
test('账本回退取消续传并重置所有连接基线', async () => {
  const f = fixture(100); await f.bus.pull(); const live = f.connect(); const resuming = f.connect();
  f.setLatest(2); resuming.resume(90); await f.bus.pull();
  for (const c of [live, resuming]) expect(c.messages.at(-1)).toEqual({ type: 'reload' });
  f.setLatest(3); await f.bus.pull(); await f.bus.pull();
  for (const c of [live, resuming]) expect(c.seqs()).toEqual([3]);
});
test('扩展消息收发与断开清理，不干扰事件过滤，忽略未知和非法消息', async () => {
  const f = fixture(); await f.bus.pull(); const c = f.connect(); const disconnected = vi.fn();
  const offDisconnect = f.bus.onDisconnect(disconnected);
  const handler = vi.fn((_message, connection) => connection.send({ type: 'log', line: '日志' }));
  const off = f.bus.onClientMessage('subscribe-log', handler);
  c.receive('{"type":"subscribe-log","task":"T-1"}'); expect(handler).toHaveBeenCalledTimes(1);
  expect(c.messages.at(-1)).toEqual({ type: 'log', line: '日志' });
  off(); c.receive('{"type":"subscribe-log"}');
  for (const text of ['', '{', 'null', '[]', '{}', '{"type":"unknown"}', '{"type":"resume","after":-1}']) c.receive(text);
  expect(handler).toHaveBeenCalledTimes(1); expect(c.close).not.toHaveBeenCalled();
  f.bus.broadcast({ type: 'notification', title: '通知' }); f.setLatest(1); await f.bus.pull(); expect(c.seqs()).toEqual([1]);
  c.disconnect(); c.disconnect(); expect(disconnected).toHaveBeenCalledTimes(1); offDisconnect();
});
test('断开和关闭丢弃在途续传；续传期间重复 resume 不重启', async () => {
  for (const closeBus of [false, true]) {
    const f = fixture(5); await f.bus.pull(); const c = f.connect(); let release!: () => void;
    f.call.mockResolvedValueOnce({ events: [], latest_seq: 5 }).mockImplementationOnce(async () => {
      await new Promise<void>(r => { release = r; }); return { events: [event(4), event(5)], latest_seq: 5 };
    });
    c.resume(3); await vi.waitFor(() => expect(release).toBeTypeOf('function')); c.resume(0);
    if (closeBus) f.bus.close(); else c.disconnect(); release(); await f.bus.pull(); expect(c.seqs()).toEqual([]);
  }
});
test('续传多页读取固定上界，后续 live 事件继续送达', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect();
  f.call.mockResolvedValueOnce({ events: [], latest_seq: 5 })
    .mockResolvedValueOnce({ events: [event(3)], latest_seq: 5 })
    .mockResolvedValueOnce({ events: [event(4), event(5), event(6)], latest_seq: 6 });
  c.resume(2); await f.bus.pull(); expect(c.seqs()).toEqual([3, 4, 5]);
  f.setLatest(6); await f.bus.pull(); expect(c.seqs()).toEqual([3, 4, 5, 6]);
});
test('续传空页不提交半批事件，下一次成功拉取重试', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect();
  f.call.mockResolvedValueOnce({ events: [], latest_seq: 5 })
    .mockResolvedValueOnce({ events: [event(3)], latest_seq: 5 })
    .mockResolvedValueOnce({ events: [], latest_seq: 5 });
  c.resume(2); await vi.waitFor(() => expect(f.log).toHaveBeenCalledWith('[事件] 续传失败：续传事件暂不可用'));
  expect(c.seqs()).toEqual([]); await f.bus.pull(); expect(c.seqs()).toEqual([3, 4, 5]);
});
test('续传分页期间账本回退，丢弃旧批次并通知全部连接', async () => {
  const f = fixture(5); await f.bus.pull(); const c = f.connect(); const other = f.connect();
  f.call.mockResolvedValueOnce({ events: [], latest_seq: 5 })
    .mockResolvedValueOnce({ events: [event(3)], latest_seq: 5 })
    .mockResolvedValueOnce({ events: [], latest_seq: 1 });
  c.resume(2); await vi.waitFor(() => expect(c.messages.at(-1)).toEqual({ type: 'reload' }));
  f.setLatest(2); await f.bus.pull();
  for (const client of [c, other]) {
    expect(client.messages).toContainEqual({ type: 'reload' }); expect(client.seqs()).toEqual([2]);
  }
});
