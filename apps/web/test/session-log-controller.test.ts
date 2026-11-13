import { expect, it, vi } from 'vitest';
import { createSessionLogController } from '../app/components/session-log-controller.ts';
import { publishLiveMessage, type LiveMessage } from '../app/lib/live-client.ts';
import { parseLogSubscribe, parseLogUnsubscribe } from '../server/eventbus-protocol.ts';
function setup(success = true) {
  const send = vi.fn(() => success), onChange = vi.fn();
  const c = createSessionLogController({ sessionId: 'ses-1', send, onChange });
  return { ...c, send, onChange };
}
const emit = (m: LiveMessage) => publishLiveMessage(m.type, m);
const chunk = (offset = 0) => ({ type: 'log.chunk', session_id: 'ses-1', offset, next: offset + 1, text: 'x' });
it('open 零偏移，失败停 subscribing 等 hello 重试', () => {
  const c = setup(false); c.open(); expect(c.send).toHaveBeenLastCalledWith({ type: 'log.subscribe', session_id: 'ses-1', offset: 0 });
  expect(c.onChange.mock.calls.at(-1)![0].state.phase).toBe('subscribing'); emit({ type: 'hello' }); expect(c.send).toHaveBeenCalledTimes(2); c.close();
});
it('其它会话的全部日志消息忽略', () => {
  const c = setup(); c.open(); const count = c.onChange.mock.calls.length;
  for (const type of ['log.chunk', 'log.end', 'log.rejected']) emit({ ...chunk(), type, session_id: 'other' });
  expect(c.onChange).toHaveBeenCalledTimes(count); emit(chunk()); expect(c.onChange.mock.calls.at(-1)![0].state.next).toBe(1); c.close();
});
it('两段后 hello 带 next', () => {
  const c = setup(); c.open(); emit(chunk()); emit(chunk(1)); emit({ type: 'hello' });
  expect(c.send).toHaveBeenLastCalledWith({ type: 'log.subscribe', session_id: 'ses-1', offset: 2 }); c.close();
});
it('活跃关闭退订并解除监听，结束关闭不退订，重复关闭无副作用', () => {
  for (const ended of [false, true]) {
    const c = setup(); c.open();
    if (ended) emit({ type: 'log.end', session_id: 'ses-1', offset: 0, reason: 'ended', message: '会话已结束' });
    c.close(); expect(c.send).toHaveBeenCalledTimes(ended ? 1 : 2); const count = c.onChange.mock.calls.length;
    emit(chunk()); emit({ type: 'hello' }); expect(c.onChange).toHaveBeenCalledTimes(count);
    c.close(); expect(c.send).toHaveBeenCalledTimes(ended ? 1 : 2);
  }
});
it('所有发送消息通过协议解析且没有多余键', () => {
  const c = setup(); c.open(); emit(chunk()); emit({ ...chunk(5) }); emit({ type: 'hello' }); c.dispatch({ type: 'retry' }); c.close();
  for (const [m] of c.send.mock.calls as unknown as [LiveMessage][]) {
    expect((m.type === 'log.subscribe' ? parseLogSubscribe(m) : parseLogUnsubscribe(m)).ok).toBe(true);
    expect(Object.keys(m).every(k => ['type', 'session_id', 'offset'].includes(k))).toBe(true);
  }
});
