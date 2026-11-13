import { expect, test } from 'vitest';
import { encodeMessage, isKernelLogPath, parseClientMessage, parseLogSubscribe, parseLogUnsubscribe, type LogServerMessage } from '../server/eventbus-protocol.ts';

const subscribe = { type: 'log.subscribe', session_id: 'ses-abc' };
const unsubscribe = { type: 'log.unsubscribe', session_id: 'ses-abc' };

test('日志下行消息编码往返保留全部字段', () => {
  const messages: LogServerMessage[] = [
    { type: 'log.chunk', session_id: 'ses-abc', offset: 0, next: 3, text: '中' },
    ...(['ended', 'rotated', 'deleted', 'error'] as const).map(reason => ({ type: 'log.end' as const, session_id: 'ses-abc', offset: 3, reason, message: '日志结束' })),
    ...(['invalid', 'limit', 'not_found', 'offline', 'kernel'] as const).map(code => ({ type: 'log.rejected' as const, session_id: null, code, message: '拒绝订阅' })),
  ];
  for (const message of messages) expect(JSON.parse(encodeMessage(message))).toEqual(message);
});
test('订阅接受省略偏移、安全整数边界及合法 ID 边界', () => {
  for (const session_id of ['a', 'a'.repeat(128), 'ses-ABC_09']) {
    const value = { ...subscribe, session_id };
    expect(parseLogSubscribe(value)).toEqual({ ok: true, value });
    for (const offset of [0, 1, Number.MAX_SAFE_INTEGER]) {
      expect(parseLogSubscribe({ ...value, offset })).toEqual({ ok: true, value: { ...value, offset } });
    }
  }
});
test('拒绝非法 ID 和非对象信封，非法 ID 回 null', () => {
  for (const session_id of [undefined, 1, '', 'a'.repeat(129), '/x', 'a\\b', 'a..b', '~x', 'a\0b', 'a.b', 'a b', '-a', '_a', '中', 'a\n', 'a\r\n', 'a\t']) {
    expect(parseLogSubscribe({ ...subscribe, session_id })).toMatchObject({ ok: false, code: 'invalid', session_id: null, message: 'session_id 不合法' });
  }
  for (const value of [null, [], 1, 'x', {}, { ...subscribe, type: 'log.unsubscribe' }]) {
    expect(parseLogSubscribe(value)).toMatchObject({ ok: false, code: 'invalid' });
  }
});
test('任意额外字段拒绝并说明路径只能来自内核', () => {
  for (const key of ['path', 'log_path', 'file', 'dir', 'foo']) {
    const result = parseLogSubscribe({ ...subscribe, [key]: key === 'foo' ? 1 : '/etc/passwd' });
    expect(result).toMatchObject({ ok: false, code: 'invalid', session_id: 'ses-abc' });
    if (!result.ok) expect(result.message).toContain('日志路径只能来自内核');
  }
});
test('拒绝非法偏移，包括显式 undefined、非有限值及数字字符串', () => {
  for (const offset of [-1, 0.5, '0', '01', null, undefined, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
    expect(parseLogSubscribe({ ...subscribe, offset })).toMatchObject({ ok: false, code: 'invalid', message: 'offset 必须是非负整数' });
  }
});
test('退订只接受类型和合法 ID', () => {
  expect(parseLogUnsubscribe(unsubscribe)).toEqual({ ok: true, value: unsubscribe });
  for (const key of ['offset', 'path', 'log_path', 'file', 'dir', 'foo']) {
    expect(parseLogUnsubscribe({ ...unsubscribe, [key]: 0 })).toMatchObject({ ok: false, code: 'invalid' });
  }
  for (const value of [null, [], {}, { ...unsubscribe, session_id: '../x' }, subscribe]) {
    expect(parseLogUnsubscribe(value)).toMatchObject({ ok: false, code: 'invalid' });
  }
});
test('通用解析器保持日志扩展透传和 resume 既有行为', () => {
  for (const value of [subscribe, unsubscribe, { ...subscribe, path: '/etc/passwd' }, { type: 'subscribe-log' }, { type: 'resume', after: 0 }]) {
    expect(parseClientMessage(JSON.stringify(value))).toEqual(value);
  }
  expect(parseClientMessage('{"type":"resume","after":-1}')).toBeUndefined();
});
test('内核路径只接受规范化的绝对 log 路径', () => {
  expect(isKernelLogPath('/tmp/ses-abc.log')).toBe(true);
  for (const value of ['a.log', '/tmp/../a.log', '/tmp/./a.log', '/tmp//a.log', '/tmp/a.jsonl', '/tmp/a\0.log', '', null, 1]) {
    expect(isKernelLogPath(value)).toBe(false);
  }
});
