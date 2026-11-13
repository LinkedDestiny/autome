import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { createLiveClient, sendLiveMessage, setLiveSender, type LiveSocket, type LiveMessageListener } from '../app/lib/live-client.ts';
class Socket implements LiveSocket {
  send = vi.fn();
  onmessage: LiveSocket['onmessage'] = null;
  onclose: LiveSocket['onclose'] = null;
  onerror: LiveSocket['onerror'] = null;
  close() { this.onclose?.call(this as unknown as WebSocket, {} as CloseEvent); }
  message(data: unknown) { this.onmessage?.call(this as unknown as WebSocket, { data: JSON.stringify(data) } as MessageEvent); }
}
const hello = { type: 'hello', seq: 10, core: 'connected' };
const subscribe = { type: 'log.subscribe', session_id: 'ses-1', offset: 0 };
function setup(onMessage: LiveMessageListener = vi.fn()) {
  const sockets: Socket[] = [], onEvents = vi.fn(), onReload = vi.fn();
  const c = createLiveClient({ url: 'ws://localhost/ws', createSocket: () => { const s = new Socket(); sockets.push(s); return s; }, onEvents, onReload, onCore: vi.fn(), onConnection: vi.fn(), onMessage });
  setLiveSender(c.send); return { ...c, sockets, onEvents, onReload, onMessage };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => { setLiveSender(undefined); vi.useRealTimers(); });
it('hello 前不发，之后 JSON 发送', () => {
  const c = setup(); expect(c.send(subscribe)).toBe(false); expect(c.sockets[0].send).not.toHaveBeenCalled();
  c.sockets[0].message(hello); expect(c.send(subscribe)).toBe(true); expect(c.sockets[0].send).toHaveBeenCalledWith(JSON.stringify(subscribe)); c.stop();
});
it('关闭后不发，新 socket hello 后恢复；旧 socket 不能撤销新连接', () => {
  const c = setup(); c.sockets[0].message(hello); c.sockets[0].close(); expect(c.send(subscribe)).toBe(false);
  vi.advanceTimersByTime(500); expect(c.send(subscribe)).toBe(false); c.sockets[1].message(hello);
  c.sockets[0].close(); expect(c.send(subscribe)).toBe(true); expect(c.sockets[1].send).toHaveBeenLastCalledWith(JSON.stringify(subscribe)); c.stop();
});
it('hello 回调可发送，resume 在订阅之前', () => {
  const results: boolean[] = [];
  const c = setup(type => { if (type === 'hello') results.push(sendLiveMessage(subscribe)); });
  c.sockets[0].message(hello); c.sockets[0].close(); vi.advanceTimersByTime(500); c.sockets[1].message(hello);
  expect(results).toEqual([true, true]); expect(c.sockets[1].send.mock.calls.map(([m]) => JSON.parse(m))).toEqual([{ type: 'resume', after: 10 }, subscribe]); c.stop();
});
it('无发送者与 stop 后返回 false，发送抛错返回 false', () => {
  expect(sendLiveMessage(subscribe)).toBe(false); const c = setup(); c.sockets[0].message(hello);
  c.sockets[0].send.mockImplementation(() => { throw new Error('断开'); }); expect(c.send(subscribe)).toBe(false);
  c.stop(); expect(sendLiveMessage(subscribe)).toBe(false);
});
it('日志原样分发，不影响事件、reload、resume 检查点', () => {
  const c = setup(); c.sockets[0].message(hello);
  const messages = [
    { type: 'log.chunk', session_id: 'ses-1', offset: 0, next: 99, text: '日志' },
    { type: 'log.end', session_id: 'ses-1', offset: 99, reason: 'ended', message: '会话已结束' },
    { type: 'log.rejected', session_id: 'ses-1', code: 'limit', message: '超限' },
  ];
  for (const m of messages) { c.sockets[0].message(m); expect(c.onMessage).toHaveBeenLastCalledWith(m.type, m); }
  vi.advanceTimersByTime(0); expect(c.onEvents).not.toHaveBeenCalled(); expect(c.onReload).not.toHaveBeenCalled();
  c.sockets[0].close(); vi.advanceTimersByTime(500); c.sockets[1].message(hello);
  expect(c.sockets[1].send).toHaveBeenCalledWith(JSON.stringify({ type: 'resume', after: 10 })); c.stop();
});
