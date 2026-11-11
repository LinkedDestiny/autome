import type { CoreStatus } from './core/supervisor.ts';

export type LedgerEvent = { seq: number; kind: string; subject: string; payload: unknown };
export type CoreMessage = { type: 'core'; status: 'connected' | 'offline'; reason?: string };
export type ServerMessage = { type: 'hello'; seq: number; core: CoreMessage['status'] } | ({ type: 'event' } & LedgerEvent) | CoreMessage | { type: 'reload' };
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
