import { expect, test } from 'vitest';
import { encodeMessage, parseClientMessage, mapCoreStatus } from '../server/eventbus-protocol.ts';
import type { ServerMessage } from '../server/eventbus-protocol.ts';
test('协议消息的字段集合与信封一致', () => {
  const messages: ServerMessage[] = [
    { type: 'hello', seq: 1234, core: 'connected' },
    { type: 'event', seq: 1235, kind: 'task.updated', subject: 'T-3', payload: {} },
    { type: 'core', status: 'offline', reason: '内核已离线' },
    { type: 'core', status: 'connected' }, { type: 'reload' },
  ];
  for (const message of messages) expect(JSON.parse(encodeMessage(message))).toEqual(message);
});
test('解析 resume 非负安全整数和可扩展 type，拒绝非法输入边界', () => {
  for (const after of [0, 1, 1000, Number.MAX_SAFE_INTEGER]) expect(parseClientMessage(JSON.stringify({ type: 'resume', after }))).toEqual({ type: 'resume', after });
  for (const after of [-1, 0.5, null, '', '01', Number.MAX_SAFE_INTEGER + 1]) expect(parseClientMessage(JSON.stringify({ type: 'resume', after }))).toBeUndefined();
  for (const text of ['', '{', 'null', '[]', '{}', '1', '{"type":1}', '{"type":"resume"}', '{"type":"resume","after":01}']) expect(parseClientMessage(text)).toBeUndefined();
  expect(parseClientMessage('{"type":"subscribe-log"}')).toEqual({ type: 'subscribe-log' });
});
test('五种内核状态映射及缺省中文原因', () => {
  const reasons = { starting: '内核正在启动', restarting: '内核已退出，正在重启：内核已退出', offline: '内核已离线', stopped: '服务正在关闭' };
  expect(mapCoreStatus({ state: 'connected', attempts: 0 })).toEqual({ type: 'core', status: 'connected' });
  for (const state of ['starting', 'restarting', 'offline', 'stopped'] as const) expect(mapCoreStatus({ state, attempts: 0 })).toEqual({ type: 'core', status: 'offline', reason: reasons[state] });
  expect(mapCoreStatus({ state: 'restarting', attempts: 1, reason: '进程退出' }).reason).toBe('内核已退出，正在重启：进程退出');
  expect(mapCoreStatus({ state: 'offline', attempts: 3, reason: '连接失败' }).reason).toBe('连接失败');
});
