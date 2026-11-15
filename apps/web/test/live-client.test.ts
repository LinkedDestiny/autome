import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { createLiveClient, reconnectDelay, type LiveSocket } from '../app/lib/live-client.ts';
class Socket implements LiveSocket {
  send = vi.fn();
  onmessage: LiveSocket['onmessage'] = null;
  onclose: LiveSocket['onclose'] = null;
  onerror: LiveSocket['onerror'] = null;
  close() { this.onclose?.call(this as unknown as WebSocket, {} as CloseEvent); }
  message(data: unknown) { this.onmessage?.call(this as unknown as WebSocket, { data: JSON.stringify(data) } as MessageEvent); }
}
function setup() {
  const sockets: Socket[] = [];
  const callbacks = { onEvents: vi.fn(), onReload: vi.fn(), onCore: vi.fn(), onConnection: vi.fn() };
  const client = createLiveClient({ url: 'ws://localhost/ws', createSocket: () => { const s = new Socket(); sockets.push(s); return s; }, ...callbacks });
  return { ...callbacks, ...client, sockets };
}
const hello = (seq = 10) => ({ type: 'hello', seq, core: 'connected' });
const event = (seq: number) => ({ type: 'event', seq, kind: 'task.updated', subject: 'T-1', payload: {} });
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
it('浏览器只在接收新事件时更新 liveSeq，hello 和重复事件不推进', () => {
  const dataset: Record<string, string> = {};
  vi.stubGlobal('document', { documentElement: { dataset } });
  const c = setup();
  try {
    c.sockets[0].message(hello());
    expect(dataset.liveSeq).toBeUndefined();
    c.sockets[0].message(event(11));
    expect(dataset.liveSeq).toBe('11');
    c.sockets[0].message(event(10));
    expect(dataset.liveSeq).toBe('11');
  } finally { c.stop(); vi.unstubAllGlobals(); }
});
it('首次 hello 建检查点，不发 resume', () => {
  const c = setup(); c.sockets[0].message(hello());
  expect(c.sockets[0].send).not.toHaveBeenCalled();
  c.sockets[0].message(event(10)); vi.advanceTimersByTime(0);
  expect(c.onEvents).not.toHaveBeenCalled(); c.stop();
});
it('重连 hello 用最后事件检查点 resume', () => {
  const c = setup(); c.sockets[0].message(hello()); c.sockets[0].message(event(11));
  c.sockets[0].close(); vi.advanceTimersByTime(500); c.sockets[1].message(hello(20));
  expect(c.sockets[1].send).toHaveBeenCalledWith(JSON.stringify({ type: 'resume', after: 11 })); c.stop();
});
it('退避序列封顶，hello 重置退避', () => {
  expect(Array.from({ length: 8 }, (_, i) => reconnectDelay(i))).toEqual([500, 1000, 2000, 4000, 8000, 10000, 10000, 10000]);
  const c = setup();
  for (const delay of [500, 1000, 2000, 4000, 8000, 10000, 10000]) {
    const count = c.sockets.length; c.sockets.at(-1)!.close();
    vi.advanceTimersByTime(delay - 1); expect(c.sockets).toHaveLength(count);
    vi.advanceTimersByTime(1); expect(c.sockets).toHaveLength(count + 1);
  }
  c.sockets.at(-1)!.message(hello()); c.sockets.at(-1)!.close();
  const count = c.sockets.length; vi.advanceTimersByTime(500); expect(c.sockets).toHaveLength(count + 1); c.stop();
});
it('去重且同轮事件合批，零与最大安全整数可用', () => {
  const c = setup(); c.sockets[0].message(hello(0));
  for (const seq of [0, 1, 1, 0, Number.MAX_SAFE_INTEGER]) c.sockets[0].message(event(seq));
  vi.advanceTimersByTime(0);
  expect(c.onEvents).toHaveBeenCalledTimes(1);
  expect(c.onEvents.mock.calls[0][0].map((e: { seq: number }) => e.seq)).toEqual([1, Number.MAX_SAFE_INTEGER]); c.stop();
});
it('reload 和 core 各自回调，reload 丢弃旧批', () => {
  const c = setup(); c.sockets[0].message(hello()); c.sockets[0].message(event(11));
  c.sockets[0].message({ type: 'reload' });
  c.sockets[0].message({ type: 'core', status: 'offline', reason: '内核已离线' }); vi.advanceTimersByTime(0);
  expect(c.onReload).toHaveBeenCalledTimes(1); expect(c.onEvents).not.toHaveBeenCalled();
  expect(c.onCore).toHaveBeenLastCalledWith({ type: 'core', status: 'offline', reason: '内核已离线' }); c.stop();
});
it('connecting → open → reconnecting → open；连接失败仍重试', () => {
  const createSocket = vi.fn(() => { throw new Error('连接不可用'); });
  const failed = createLiveClient({ url: 'ws://localhost/ws', createSocket, onEvents: vi.fn(), onReload: vi.fn(), onCore: vi.fn(), onConnection: vi.fn() });
  vi.advanceTimersByTime(500); expect(createSocket).toHaveBeenCalledTimes(2); failed.stop();
  const c = setup(); c.sockets[0].close(); vi.advanceTimersByTime(500);
  c.sockets[1].message(hello()); c.sockets[1].close(); vi.advanceTimersByTime(500); c.sockets[2].message(hello());
  expect(c.onConnection.mock.calls.flat()).toEqual(['connecting', 'reconnecting', 'open', 'reconnecting', 'open']); c.stop();
});
it('stop 在连接中、在线、待重试时均停止，清掉待发批', () => {
  for (const state of ['connecting', 'open', 'reconnecting']) {
    const c = setup();
    if (state !== 'connecting') { c.sockets[0].message(hello()); c.sockets[0].message(event(11)); }
    if (state === 'reconnecting') c.sockets[0].close();
    c.stop(); vi.runAllTimers(); expect(c.sockets).toHaveLength(1); expect(c.onEvents).not.toHaveBeenCalled();
  }
});
it('在线及 resume 后 reload 清检查点；空检查点重连整体重验', () => {
  for (const resume of [false, true]) {
    const c = setup(); c.sockets[0].message(hello(100));
    if (resume) { c.sockets[0].close(); vi.advanceTimersByTime(500); c.sockets[1].message(hello(2)); }
    c.sockets.at(-1)!.message({ type: 'reload' }); c.sockets.at(-1)!.message(event(3)); vi.advanceTimersByTime(0);
    expect(c.onEvents.mock.calls[0][0][0].seq).toBe(3);
    c.sockets.at(-1)!.message({ type: 'reload' }); c.sockets.at(-1)!.close(); vi.advanceTimersByTime(500);
    c.sockets.at(-1)!.message(hello(5)); expect(c.onReload).toHaveBeenCalledTimes(3);
    expect(c.sockets.at(-1)!.send).not.toHaveBeenCalled();
    c.sockets.at(-1)!.message(event(5)); c.sockets.at(-1)!.message(event(6)); vi.advanceTimersByTime(0);
    expect(c.onEvents.mock.calls.at(-1)![0].map((e: { seq: number }) => e.seq)).toEqual([6]); c.stop();
  }
});
