import { isAbsolute, normalize } from 'node:path';
import type { CoreStatus } from './core/supervisor.ts';

export type LedgerEvent = { seq: number; kind: string; subject: string; payload: unknown };
export type CoreMessage = { type: 'core'; status: 'connected' | 'offline'; reason?: string };
export type ServerMessage = { type: 'hello'; seq: number; core: CoreMessage['status'] } | ({ type: 'event' } & LedgerEvent) | CoreMessage | { type: 'reload' } | LogServerMessage;
export function encodeMessage(message: ServerMessage | ExtensionMessage) { return JSON.stringify(message); }
export function mapCoreStatus(status: CoreStatus): CoreMessage {
  switch (status.state) {
    case 'connected': return { type: 'core', status: 'connected' };
    case 'starting': return { type: 'core', status: 'offline', reason: '内核正在启动' };
    case 'restarting': return { type: 'core', status: 'offline', reason: `内核已退出，正在重启：${status.reason ?? '内核已退出'}` };
    case 'offline': return { type: 'core', status: 'offline', reason: status.reason ?? '内核已离线' };
    case 'stopped': return { type: 'core', status: 'offline', reason: '服务正在关闭' };
  }
}

export type ExtensionMessage = { type: string; [key: string]: unknown };
export function parseClientMessage(text: string): ExtensionMessage | undefined {
  let message: unknown;
  try { message = JSON.parse(text); } catch { return; }
  if (!message || typeof message !== 'object' || Array.isArray(message)) return;
  const value = message as ExtensionMessage;
  if (typeof value.type !== 'string') return;
  if (value.type === 'resume' && (!Number.isSafeInteger(value.after) || (value.after as number) < 0)) return;
  return value;
}

export type LogSubscribe = { type: 'log.subscribe'; session_id: string; offset?: number };
export type LogUnsubscribe = { type: 'log.unsubscribe'; session_id: string };
export type LogClientMessage = LogSubscribe | LogUnsubscribe;
export type LogChunk = { type: 'log.chunk'; session_id: string; offset: number; next: number; text: string };
export type LogServerMessage = LogChunk
  | { type: 'log.end'; session_id: string; offset: number; reason: 'ended' | 'rotated' | 'deleted' | 'error'; message: string }
  | { type: 'log.rejected'; session_id: string | null; code: 'invalid' | 'limit' | 'not_found' | 'offline' | 'kernel'; message: string };
type LogParseResult<T> = { ok: true; value: T }
  | { ok: false; code: 'invalid'; message: string; session_id: string | null };

function parseLogMessage(message: unknown, type: LogClientMessage['type']): LogParseResult<LogClientMessage> {
  const value = message && typeof message === 'object' && !Array.isArray(message)
    ? message as Record<string, unknown> : {};
  const session_id = typeof value.session_id === 'string' && /^[A-Za-z0-9][A-Za-z0-9_-]{0,127}(?![\s\S])/.test(value.session_id)
    ? value.session_id : null;
  const invalid = (message: string): LogParseResult<never> => ({ ok: false, code: 'invalid', message, session_id });
  const keys = type === 'log.subscribe' ? ['type', 'session_id', 'offset'] : ['type', 'session_id'];
  if (Object.keys(value).some(key => !keys.includes(key))) {
    return invalid(`${type} 只接受 session_id${type === 'log.subscribe' ? ' 与 offset' : ''}；日志路径只能来自内核`);
  }
  if (value.type !== type) return invalid(`消息类型必须是 ${type}`);
  if (session_id === null) return invalid('session_id 不合法');
  if (type === 'log.subscribe' && Object.hasOwn(value, 'offset')) {
    if (!Number.isSafeInteger(value.offset) || (value.offset as number) < 0) return invalid('offset 必须是非负整数');
    return { ok: true, value: { type, session_id, offset: value.offset as number } };
  }
  return { ok: true, value: { type, session_id } };
}

export function parseLogSubscribe(message: unknown): LogParseResult<LogSubscribe> {
  return parseLogMessage(message, 'log.subscribe') as LogParseResult<LogSubscribe>;
}
export function parseLogUnsubscribe(message: unknown): LogParseResult<LogUnsubscribe> {
  return parseLogMessage(message, 'log.unsubscribe') as LogParseResult<LogUnsubscribe>;
}

export function isKernelLogPath(value: unknown): value is string {
  return typeof value === 'string' && isAbsolute(value) && normalize(value) === value
    && !value.includes('\0') && value.endsWith('.log');
}
