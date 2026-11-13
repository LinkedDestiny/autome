import type { LogChunk, LogClientMessage, LogServerMessage } from '../../server/eventbus-protocol.ts';

export const LOG_WINDOW_BYTES = 256 * 1024;
export type LogState = {
  sessionId: string; phase: 'idle' | 'subscribing' | 'live' | 'ended' | 'rejected';
  parts: { text: string; bytes: number }[]; total: number; from: number; to: number;
  next: number; follow: boolean; unseen: boolean; notice: string;
};
export type LogAction = { type: 'open' | 'hello' | 'pause' | 'jump' | 'earlier' | 'later' | 'top' | 'retry' | 'close' }
  | { type: 'scrolled'; atBottom: boolean }
  | { type: 'chunk'; message: LogChunk }
  | { type: 'end'; message: Extract<LogServerMessage, { type: 'log.end' }> }
  | { type: 'rejected'; message: Extract<LogServerMessage, { type: 'log.rejected' }> };
export type LogUpdate = { state: LogState; send?: LogClientMessage; scroll?: 'bottom' | 'top' };
export function initialLogState(sessionId: string): LogState {
  return { sessionId, phase: 'idle', parts: [], total: 0, from: 0, to: 0, next: 0, follow: true, unseen: false, notice: '' };
}
const active = (s: LogState) => s.phase === 'subscribing' || s.phase === 'live';
function windowBefore(s: LogState, to: number) {
  let from = to, bytes = 0;
  while (from > 0 && bytes + s.parts[from - 1].bytes <= LOG_WINDOW_BYTES) bytes += s.parts[--from].bytes;
  return { from, to };
}
function windowAfter(s: LogState, from: number) {
  let to = from, bytes = 0;
  while (to < s.parts.length && bytes + s.parts[to].bytes <= LOG_WINDOW_BYTES) bytes += s.parts[to++].bytes;
  return { from, to };
}
export function viewOf(s: LogState) {
  const bytes = (from: number, to: number) => s.parts.slice(from, to).reduce((sum, p) => sum + p.bytes, 0);
  const hiddenBefore = bytes(0, s.from), hiddenAfter = bytes(s.to, s.parts.length);
  return { text: s.parts.slice(s.from, s.to).map(p => p.text).join(''), hiddenBefore, hiddenAfter,
    hint: hiddenBefore ? `前面还有约 ${Math.ceil(hiddenBefore / 1024)} KB 未显示` : '' };
}
export function updateLog(state: LogState, action: LogAction): LogUpdate {
  let s = { ...state };
  const subscribe = (): LogUpdate => ({ state: s, send: { type: 'log.subscribe', session_id: s.sessionId, offset: s.next } });
  switch (action.type) {
    case 'open': s = { ...initialLogState(s.sessionId), phase: 'subscribing' }; return subscribe();
    case 'retry': s.phase = 'subscribing'; s.notice = ''; return subscribe();
    case 'hello': return active(s) ? subscribe() : { state: s };
    case 'close': return { state: { ...s, phase: 'idle' }, ...(active(s) ? { send: { type: 'log.unsubscribe' as const, session_id: s.sessionId } } : {}) };
    case 'chunk': {
      if (!active(s)) break;
      const m = action.message;
      if (m.offset < s.next) break;
      if (m.offset > s.next) return subscribe();
      const bytes = new TextEncoder().encode(m.text).length;
      s.parts = [...s.parts, { text: m.text, bytes }];
      s.total += bytes; s.next = m.next; s.phase = 'live';
      if (s.follow) return { state: { ...s, ...windowBefore(s, s.parts.length) }, scroll: 'bottom' };
      s.unseen = true; break;
    }
    case 'end': if (active(s)) { s.phase = 'ended'; s.notice = action.message.message; } break;
    case 'rejected': if (active(s)) { s.phase = 'rejected'; s.notice = action.message.message; } break;
    case 'pause': s.follow = false; break;
    case 'scrolled': s.follow = action.atBottom && s.to === s.parts.length; if (s.follow) s.unseen = false; break;
    case 'jump': return { state: { ...s, ...windowBefore(s, s.parts.length), follow: true, unseen: false }, scroll: 'bottom' };
    case 'top': return { state: { ...s, ...windowAfter(s, 0), follow: false }, scroll: 'top' };
    case 'earlier': if (s.from > 0) return { state: { ...s, ...windowBefore(s, s.from), follow: false }, scroll: 'bottom' }; break;
    case 'later': {
      if (s.to === s.parts.length) break;
      const window = windowAfter(s, s.to), follow = window.to === s.parts.length;
      return { state: { ...s, ...window, follow, unseen: follow ? false : s.unseen }, scroll: 'top' };
    }
  }
  return { state: s };
}
