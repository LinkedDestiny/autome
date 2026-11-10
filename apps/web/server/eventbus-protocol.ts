import type { CoreStatus } from './core/supervisor.ts';

export type LedgerEvent = { seq: number; kind: string; subject: string; payload: unknown };
export type CoreMessage = { type: 'core'; status: 'connected' | 'offline'; reason?: string };
export type ServerMessage = { type: 'hello'; seq: number; core: CoreMessage['status'] } | ({ type: 'event' } & LedgerEvent) | CoreMessage;
export function encodeMessage(message: ServerMessage) { return JSON.stringify(message); }
export function mapCoreStatus(status: CoreStatus): CoreMessage {
  switch (status.state) {
    case 'connected': return { type: 'core', status: 'connected' };
    case 'starting': return { type: 'core', status: 'offline', reason: '内核正在启动' };
    case 'restarting': return { type: 'core', status: 'offline', reason: `内核已退出，正在重启：${status.reason ?? '内核已退出'}` };
    case 'offline': return { type: 'core', status: 'offline', reason: status.reason ?? '内核已离线' };
    case 'stopped': return { type: 'core', status: 'offline', reason: '服务正在关闭' };
  }
}
