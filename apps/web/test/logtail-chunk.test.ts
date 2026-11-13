import { expect, test } from 'vitest';
import { encodeMessage } from '../server/eventbus-protocol.ts';
import { splitChunks } from '../server/logtail-chunk.ts';

function check(bytes: Buffer, sessionId = 'ses-abc', start = 73) {
  const result = splitChunks({ sessionId, start, bytes, final: true });
  let next = start;
  for (const message of result.messages) {
    expect(message.offset).toBe(next);
    expect(message.next).toBeGreaterThan(message.offset);
    expect(Buffer.byteLength(encodeMessage(message))).toBeLessThanOrEqual(65536);
    next = message.next;
  }
  expect(next).toBe(start + bytes.length);
  expect(result.consumed).toBe(bytes.length);
  expect(result.messages.map(message => message.text).join('')).toBe(bytes.toString('utf8'));
  return result;
}
test('空输入不发消息，小输入按原始字节推进', () => {
  expect(check(Buffer.alloc(0)).messages).toEqual([]);
  expect(check(Buffer.from('中a')).messages).toEqual([{ type: 'log.chunk', session_id: 'ses-abc', offset: 73, next: 77, text: '中a' }]);
});
test('200 KiB ASCII 编码限长且偏移首尾相接', () => {
  expect(check(Buffer.alloc(200 * 1024, 97)).messages.length).toBeGreaterThanOrEqual(4);
});
test('中文和四字节字符在候选切点两侧均不破坏字符', () => {
  for (let prefix = 0; prefix < 4; prefix++) {
    const result = check(Buffer.from('a'.repeat(prefix) + '中文😀'.repeat(21000)));
    for (const message of result.messages) expect(message.text).not.toContain('\uFFFD');
  }
});
test('70 KiB 转义密集文本按整条 JSON 限长', () => {
  check(Buffer.from('\u001b"'.repeat(35 * 1024)));
  check(Buffer.from('\0\\\n\r\t'.repeat(15000)));
});
test('合法残缺前缀运行中保留、结束时替换，下次补齐可续传', () => {
  for (const character of ['¢', '中', '😀']) {
    const full = Buffer.from(character);
    for (let length = 1; length < full.length; length++) {
      for (const prefix of ['', 'ab']) {
        const bytes = Buffer.concat([Buffer.from(prefix), full.subarray(0, length)]);
        const pending = splitChunks({ sessionId: 's', start: 0, bytes, final: false });
        expect(pending.consumed).toBe(prefix.length);
        expect(pending.messages.map(message => message.text).join('')).toBe(prefix);
        expect(check(bytes, 's', 0).messages.map(message => message.text).join('')).toBe(prefix + '\uFFFD');
        const resumed = splitChunks({ sessionId: 's', start: pending.consumed, bytes: full, final: false });
        expect(resumed.messages[0]).toMatchObject({ offset: prefix.length, next: prefix.length + full.length, text: character });
      }
    }
  }
});
test('非法 UTF-8 替换但偏移保留原始字节单位', () => {
  const result = check(Buffer.from([97, 255, 98]), 's', 0);
  expect(result.messages[0]).toMatchObject({ text: 'a\uFFFDb', offset: 0, next: 3 });
  expect(Buffer.byteLength(result.messages[0].text)).toBe(5);
  for (const bytes of [[0xe0, 0x80], [0xed, 0xa0], [0xf4, 0x90], [0xc0], [0x80], [0xe4, 0xb8, 0x61]]) {
    const input = Buffer.from(bytes);
    const running = splitChunks({ sessionId: 's', start: 0, bytes: input, final: false });
    expect(running.consumed).toBe(input.length);
    expect(running.messages.map(message => message.text).join('')).toBe(input.toString('utf8'));
  }
});
test('128 字符会话 ID 与大偏移的信封开销计入上限', () => {
  check(Buffer.alloc(200 * 1024, 97), 's'.repeat(128), Number.MAX_SAFE_INTEGER - 200 * 1024);
  check(Buffer.alloc(70 * 1024, 27), 's'.repeat(128));
});
