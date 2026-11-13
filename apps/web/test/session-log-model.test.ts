import { expect, it } from 'vitest';
import { initialLogState, updateLog, viewOf, LOG_WINDOW_BYTES as N, type LogState } from '../app/components/session-log-model.ts';
const opened = () => updateLog(initialLogState('ses-1'), { type: 'open' }).state;
const chunk = (s: LogState, text = '中文\n', offset = s.next, next = offset + Buffer.byteLength(text)) => updateLog(s, { type: 'chunk', message: { type: 'log.chunk', session_id: s.sessionId, text, offset, next } });
function big() { let s = chunk(opened(), 'HEADER\n').state; for (let i = 0; i < 20; i++) s = chunk(s, `${i}:` + '中文\n'.repeat(5000)).state; return s; }
it('连续、重复、缺口，next 使用文件偏移而非替换字符字节数', () => {
  let s = chunk(opened(), '�', 0, 1).state; s = chunk(s, 'x').state;
  expect(s.next).toBe(2); expect(s.total).toBe(4);
  expect(chunk(s, '旧', 0).state).toEqual(s);
  expect(chunk(s, '缺', 3).send).toEqual({ type: 'log.subscribe', session_id: 'ses-1', offset: 2 });
});
it('暂停追加不滚动且窗口和文字固定', () => {
  const first = chunk(opened()); expect(first.scroll).toBe('bottom');
  const s = updateLog(first.state, { type: 'pause' }).state, r = chunk(s);
  expect(r.scroll).toBeUndefined(); expect(r.state.unseen).toBe(true);
  expect([r.state.from, r.state.to, viewOf(r.state).text]).toEqual([s.from, s.to, viewOf(s).text]);
});
it('滚离暂停，末尾恢复，非末尾不能恢复', () => {
  let s = chunk(opened()).state; s = updateLog(s, { type: 'scrolled', atBottom: false }).state;
  expect(s.follow).toBe(false);
  expect(updateLog(s, { type: 'scrolled', atBottom: true }).state).toMatchObject({ follow: true, unseen: false });
  s = updateLog(big(), { type: 'top' }).state;
  expect(updateLog(s, { type: 'scrolled', atBottom: true }).state.follow).toBe(false);
});
it('jump 返回末尾并清未读', () => {
  const s = updateLog(big(), { type: 'top' }).state, r = updateLog(chunk(s).state, { type: 'jump' });
  expect(r.scroll).toBe('bottom'); expect(r.state).toMatchObject({ follow: true, unseen: false, to: r.state.parts.length });
});
it('UTF-8 限量，保留全部字节，提示隐藏量', () => {
  const s = big(), v = viewOf(s);
  expect(Buffer.byteLength(v.text)).toBeLessThanOrEqual(N); expect(s.total).toBeGreaterThan(N);
  expect(v.hiddenBefore + Buffer.byteLength(v.text)).toBe(s.total); expect(v.hint).toContain('前面还有约');
});
it('100 KiB 历史两段全文与空日志', () => {
  let s = chunk(chunk(opened(), 'x'.repeat(51200)).state, 'y'.repeat(51200)).state;
  s = updateLog(s, { type: 'end', message: { type: 'log.end', session_id: s.sessionId, offset: s.next, reason: 'ended', message: '会话已结束' } }).state;
  expect(viewOf(s)).toMatchObject({ text: 'x'.repeat(51200) + 'y'.repeat(51200), hiddenBefore: 0, hiddenAfter: 0 });
  expect(viewOf(chunk(opened(), '').state).text).toBe('');
});
it('四种结束、五种拒绝、重试保留内容', () => {
  const s = chunk(opened()).state;
  for (const reason of ['ended', 'rotated', 'deleted', 'error'] as const) {
    const r = updateLog(s, { type: 'end', message: { type: 'log.end', session_id: s.sessionId, offset: s.next, reason, message: '服务端原因' } });
    expect(r.state).toMatchObject({ phase: 'ended', notice: '服务端原因' });
  }
  for (const code of ['invalid', 'limit', 'not_found', 'offline', 'kernel'] as const) {
    const r = updateLog(s, { type: 'rejected', message: { type: 'log.rejected', session_id: s.sessionId, code, message: '拒绝原因' } });
    expect(r.state).toMatchObject({ phase: 'rejected', notice: '拒绝原因' });
    const retry = updateLog(r.state, { type: 'retry' }); expect(retry.send).toMatchObject({ offset: s.next }); expect(retry.state.parts).toEqual(s.parts);
  }
});
it('hello 按阶段续传；关闭各阶段归 idle', () => {
  for (const phase of ['idle', 'subscribing', 'live', 'ended', 'rejected'] as const) {
    const s = { ...opened(), phase, next: Number.MAX_SAFE_INTEGER };
    expect(updateLog(s, { type: 'hello' }).send).toEqual(['subscribing', 'live'].includes(phase) ? { type: 'log.subscribe', session_id: s.sessionId, offset: s.next } : undefined);
    const r = updateLog(s, { type: 'close' }); expect(r.state.phase).toBe('idle');
    expect(Boolean(r.send)).toBe(['subscribing', 'live'].includes(phase));
  }
});
it('大历史日志双向逐页拼接全文，单项与恰好 N 边界', () => {
  const history = big();
  const all = updateLog(history, { type: 'end', message: { type: 'log.end', session_id: history.sessionId, offset: history.next, reason: 'ended', message: '会话已结束' } }).state;
  const text = all.parts.map(p => p.text).join('');
  let r = updateLog(all, { type: 'top' }), s = r.state;
  expect(r.scroll).toBe('top'); expect(s.from).toBe(0); expect(viewOf(s).text.startsWith('HEADER\n')).toBe(true);
  expect(viewOf(all).text).not.toContain('HEADER'); let forward = viewOf(s).text;
  while (viewOf(s).hiddenAfter) { r = updateLog(s, { type: 'later' }); s = r.state; expect(Buffer.byteLength(viewOf(s).text)).toBeLessThanOrEqual(N); forward += viewOf(s).text; }
  expect(forward).toBe(text); expect(s.follow).toBe(true);
  let backward = viewOf(s).text;
  while (viewOf(s).hiddenBefore) { s = updateLog(s, { type: 'earlier' }).state; backward = viewOf(s).text + backward; }
  expect(backward).toBe(text);
  expect(updateLog(s, { type: 'jump' }).state.to).toBe(all.parts.length);
  s = opened(); for (let i = 0; i < 8; i++) s = chunk(s, 'a'.repeat(N / 8)).state;
  expect(viewOf(s).hiddenBefore).toBe(0); expect(Buffer.byteLength(viewOf(s).text)).toBe(N);
  expect(viewOf(chunk(s, 'a').state).hiddenBefore).toBe(N / 8);
  expect(viewOf(chunk(opened(), '中').state).text).toBe('中');
});
it('非末尾窗口追加固定，jump 后恢复跟随', () => {
  const s = updateLog(big(), { type: 'top' }).state, r = chunk(s);
  expect(r.state).toMatchObject({ from: s.from, to: s.to, follow: false, unseen: true }); expect(r.state.total).toBeGreaterThan(s.total); expect(r.scroll).toBeUndefined();
  expect(chunk(updateLog(r.state, { type: 'jump' }).state).scroll).toBe('bottom');
});
