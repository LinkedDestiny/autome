import { afterEach, expect, it, vi } from 'vitest';
import { createBellClient, type BellSnapshot } from '../app/components/bell-client.ts';
import { createLiveClient, publishLiveMessage, subscribeLiveMessages, type LiveSocket, type LiveMessageListener } from '../app/lib/live-client.ts';

const data = { unread: 7, items: [] };
const response = (payload = data) => new Response(JSON.stringify({ ok: true, payload }));
const settle = async () => { for (let i = 0; i < 30; i++) await Promise.resolve(); };
const notification = { type: 'notification', id: 'ntf-1', kind: 'await_merge', task_id: 'T-3', title: 'T-3 可以合并了' };
class Socket implements LiveSocket {
  send = vi.fn();
  onmessage: LiveSocket['onmessage'] = null;
  onclose: LiveSocket['onclose'] = null;
  onerror: LiveSocket['onerror'] = null;
  close() { this.onclose?.call(this as unknown as WebSocket, {} as CloseEvent); }
  raw(data: string) { this.onmessage?.call(this as unknown as WebSocket, { data } as MessageEvent); }
  message(data: unknown) { this.raw(JSON.stringify(data)); }
}
function live(onMessage?: LiveMessageListener) {
  const sockets: Socket[] = [];
  const callbacks = { onEvents: vi.fn(), onReload: vi.fn(), onCore: vi.fn(), onConnection: vi.fn() };
  const client = createLiveClient({ url: 'ws://localhost/ws', createSocket: () => { const s = new Socket(); sockets.push(s); return s; }, ...callbacks, onMessage });
  return { ...client, ...callbacks, sockets };
}
afterEach(() => vi.useRealTimers());
it('挂载与三类消息重新读取服务端；失败仍交通知与 null，恢复清错误', async () => {
  const fetch = vi.fn<typeof globalThis.fetch>().mockImplementation(async () => response());
  const onChange = vi.fn(), onNotification = vi.fn();
  const c = createBellClient({ fetch, onChange, onNotification });
  await c.start(); expect(fetch).toHaveBeenCalledTimes(1);
  for (const type of ['hello', 'notification', 'notification-read']) {
    const count = fetch.mock.calls.length;
    c.handleMessage(type, { ...notification, type }); await settle();
    expect(fetch).toHaveBeenCalledTimes(count + 1);
    expect(onChange).toHaveBeenLastCalledWith(data);
  }
  expect(onNotification).toHaveBeenLastCalledWith(notification, []);
  for (const type of ['event', 'core', 'reload']) c.handleMessage(type, { type });
  await settle(); expect(fetch).toHaveBeenCalledTimes(4);
  fetch.mockRejectedValueOnce(new Error('网络不可达'));
  c.handleMessage('notification', notification); await settle();
  expect(onNotification).toHaveBeenLastCalledWith(notification, null);
  expect(onChange.mock.lastCall?.[0]).toEqual({ ...data, error: '通知加载失败：网络不可达' });
  c.handleMessage('hello', { type: 'hello' }); await settle(); expect(onChange).toHaveBeenLastCalledWith(data);
  c.stop();
});
it('在途消息合并一次补拉，每条通知在其刷新后交付；停止丢弃在途响应', async () => {
  const resolves: ((value: Response) => void)[] = [];
  const fetch = vi.fn<typeof globalThis.fetch>(() => new Promise(resolve => resolves.push(resolve)));
  const onChange = vi.fn(), onNotification = vi.fn();
  const c = createBellClient({ fetch, onChange, onNotification });
  const start = c.start();
  c.handleMessage('notification', notification); c.handleMessage('notification', { ...notification, id: 'ntf-2' });
  expect(fetch).toHaveBeenCalledTimes(1);
  resolves[0](response()); await settle(); expect(fetch).toHaveBeenCalledTimes(2);
  expect(onNotification).not.toHaveBeenCalled();
  resolves[1](response({ unread: 9, items: [] })); await start;
  expect(onNotification.mock.calls.map(call => call[0].id)).toEqual(['ntf-1', 'ntf-2']);
  expect(onChange).toHaveBeenLastCalledWith({ unread: 9, items: [] });
  c.handleMessage('hello', { type: 'hello' }); c.stop(); resolves[2](response()); await settle();
  expect(onChange).toHaveBeenCalledTimes(2);
  await c.start(); await c.markAll(); c.handleMessage('notification', notification);
  expect(fetch).toHaveBeenCalledTimes(3);
});
it('单条和全部已读 POST 后重拉；拒绝或网络失败保留计数并报中文错误', async () => {
  const fetch = vi.fn<typeof globalThis.fetch>().mockImplementation(async () => response());
  const onChange = vi.fn<(s: BellSnapshot) => void>();
  const c = createBellClient({ fetch, onChange }); await c.start();
  await c.markRead('ntf-1'); await c.markAll();
  expect(fetch.mock.calls.filter(call => call[1]?.method === 'POST').map(call => [call[0], call[1]?.body])).toEqual([
    ['/api/notifications/read', '{"ids":["ntf-1"]}'], ['/api/notifications/read', '{"all":true}'],
  ]);
  expect(fetch).toHaveBeenCalledTimes(5);
  fetch.mockResolvedValueOnce(new Response(JSON.stringify({ ok: false, error: '已拒绝：来源不符' }), { status: 403 }));
  await c.markRead('ntf-1'); expect(onChange).toHaveBeenLastCalledWith({ ...data, error: '标记已读失败：已拒绝：来源不符' });
  fetch.mockRejectedValueOnce(new Error('网络不可达')); await c.markAll();
  expect(onChange).toHaveBeenLastCalledWith({ ...data, error: '标记已读失败：网络不可达' });
  expect(fetch).toHaveBeenCalledTimes(7); c.stop();
});
it('内置与扩展消息出口保留原回调；坏消息、旧 socket、停止后不交出', () => {
  vi.useFakeTimers(); const onMessage = vi.fn(); const c = live(onMessage); const socket = c.sockets[0];
  socket.message({ type: 'hello', seq: 1, core: 'connected' });
  socket.message({ type: 'event', seq: 2, kind: 'task.updated', subject: 'T-3', payload: {} }); vi.advanceTimersByTime(0);
  expect(c.onEvents).toHaveBeenCalledTimes(1);
  socket.message({ type: 'core', status: 'offline' }); socket.message({ type: 'reload' });
  expect(c.onCore).toHaveBeenCalledTimes(2); expect(c.onReload).toHaveBeenCalledTimes(1);
  socket.message(notification); socket.message({ type: 'notification-read', ids: ['ntf-1'] });
  expect(onMessage.mock.calls.map(call => call[0])).toEqual(['hello', 'event', 'core', 'reload', 'notification', 'notification-read']);
  expect(c.onCore).toHaveBeenCalledTimes(2); expect(c.onEvents).toHaveBeenCalledTimes(1); expect(c.onReload).toHaveBeenCalledTimes(1);
  for (const invalid of ['{', 'null', '[]', '1', '"hello"', '{}', '{"type":1}']) socket.raw(invalid);
  socket.close(); vi.advanceTimersByTime(500); socket.message(notification);
  expect(onMessage).toHaveBeenCalledTimes(6); c.stop(); c.sockets[1].message(notification);
  expect(onMessage).toHaveBeenCalledTimes(6);
  const old = live(); old.sockets[0].message(notification); expect(old.onEvents).not.toHaveBeenCalled(); old.stop();
});
it('订阅者全部收到，退订生效，异常不阻断其它订阅者或内置事件', () => {
  vi.useFakeTimers(); const a = vi.fn(), b = vi.fn();
  const offBad = subscribeLiveMessages(() => { throw new Error('订阅错误'); });
  const offA = subscribeLiveMessages(a), offB = subscribeLiveMessages(b);
  const c = live(publishLiveMessage);
  try {
    c.sockets[0].message(notification); expect(a).toHaveBeenCalledTimes(1); expect(b).toHaveBeenCalledTimes(1);
    offA(); c.sockets[0].message({ type: 'event', seq: 1 }); vi.advanceTimersByTime(0);
    expect(a).toHaveBeenCalledTimes(1); expect(b).toHaveBeenCalledTimes(2); expect(c.onEvents).toHaveBeenCalledTimes(1);
  } finally { offBad(); offA(); offB(); c.stop(); }
});
it('单 socket 消息出口贯通控制器；重连 hello 补拉；两个标签页已读同步', async () => {
  vi.useFakeTimers();
  let unread = 2;
  const fetch = vi.fn<typeof globalThis.fetch>(async (_url, init) => {
    if (init?.method === 'POST') unread = JSON.parse(String(init.body)).all ? 0 : 1;
    return response({ unread, items: [] });
  });
  const onChange = vi.fn(), otherChange = vi.fn();
  const controller = createBellClient({ fetch, onChange }); const other = createBellClient({ fetch, onChange: otherChange });
  const off = subscribeLiveMessages(controller.handleMessage);
  const c = live(publishLiveMessage);
  try {
    await controller.start(); await other.start();
    expect(c.sockets).toHaveLength(1);
    c.sockets[0].message(notification); await settle(); expect(fetch).toHaveBeenCalledTimes(3);
    await controller.markRead('ntf-1'); other.handleMessage('notification-read', { type: 'notification-read' }); await settle();
    expect(onChange.mock.lastCall?.[0].unread).toBe(1); expect(otherChange.mock.lastCall?.[0].unread).toBe(1);
    await controller.markAll(); other.handleMessage('notification-read', { type: 'notification-read' }); await settle();
    expect(onChange.mock.lastCall?.[0].unread).toBe(0); expect(otherChange.mock.lastCall?.[0].unread).toBe(0);
    c.sockets[0].close(); vi.advanceTimersByTime(500); expect(c.sockets).toHaveLength(2);
    const count = fetch.mock.calls.length; c.sockets[1].message({ type: 'hello', seq: 10, core: 'connected' }); await settle();
    expect(fetch).toHaveBeenCalledTimes(count + 1);
    c.stop(); c.sockets[1].message(notification); vi.runAllTimers(); await settle();
    expect(fetch).toHaveBeenCalledTimes(count + 1); expect(c.sockets).toHaveLength(2);
  } finally { c.stop(); off(); controller.stop(); other.stop(); }
});
