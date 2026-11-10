import type { CoreStatus } from './core/supervisor.ts';
import { encodeMessage, mapCoreStatus } from './eventbus-protocol.ts';
import type { CoreMessage, LedgerEvent } from './eventbus-protocol.ts';

type Transport = { send: (message: string) => void; close: (code: number, reason: string) => void };
type Page = { events: LedgerEvent[]; latest_seq: number };
export function createEventBus({ call, log }: {
  call: (method: string, params: Record<string, unknown>) => Promise<unknown>;
  log: (line: string) => void;
}) {
  let cursor = 0;
  let initialized = false;
  let closed = false;
  let again = false;
  let pending: Promise<void> | undefined;
  let core: CoreMessage = mapCoreStatus({ state: 'stopped', attempts: 0 });
  const connections = new Map<Transport, number>();
  const listeners = new Set<(event: LedgerEvent) => void>();
  async function drain() {
    do {
      again = false;
      if (!initialized) {
        const page = await call('events.since', { after_seq: Number.MAX_SAFE_INTEGER }) as Page;
        if (closed) return;
        cursor = page.latest_seq;
        initialized = true;
      }
      while (!closed) {
        const page = await call('events.since', { after_seq: cursor, limit: 1000 }) as Page;
        if (closed) return;
        for (const event of page.events) {
          if (event.seq <= cursor) continue;
          for (const [connection, sentSeq] of connections) {
            if (event.seq > sentSeq) {
              connection.send(encodeMessage({ type: 'event', ...event }));
              connections.set(connection, event.seq);
            }
          }
          cursor = event.seq;
          for (const listener of listeners) listener(event);
        }
        if (!page.events.length || cursor >= page.latest_seq) break;
      }
    } while (again && !closed);
  }
  function pull(): Promise<void> {
    if (closed) return Promise.resolve();
    if (pending) { again = true; return pending; }
    pending = drain().catch(error => {
      log(`[事件] 拉取失败：${error instanceof Error ? error.message : String(error)}`);
    }).finally(() => { pending = undefined; });
    return pending;
  }
  return {
    pull,
    setCoreStatus(status: CoreStatus) {
      core = mapCoreStatus(status);
      if (status.state === 'connected') void pull();
    },
    connect(connection: Transport) {
      if (closed) { connection.close(1001, '服务正在关闭'); return () => {}; }
      connections.set(connection, cursor);
      connection.send(encodeMessage({ type: 'hello', seq: cursor, core: core.status }));
      return () => { connections.delete(connection); };
    },
    subscribe(listener: (event: LedgerEvent) => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    close() {
      closed = true;
      for (const connection of connections.keys()) connection.close(1001, '服务正在关闭');
      connections.clear();
      listeners.clear();
    },
  };
}
