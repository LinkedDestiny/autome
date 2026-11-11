import type { CoreStatus } from './core/supervisor.ts';
import { encodeMessage, mapCoreStatus, parseClientMessage } from './eventbus-protocol.ts';
import type { CoreMessage, LedgerEvent, ExtensionMessage } from './eventbus-protocol.ts';

type Transport = { send: (message: string) => void; close: (code: number, reason: string) => void };
type Page = { events: LedgerEvent[]; latest_seq: number };
type Connection = { send: (message: ExtensionMessage) => void };
type Client = { transport: Transport; connection: Connection; sentSeq: number; helloSeq: number; after?: number; queue: LedgerEvent[] };
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
  const connections = new Map<Transport, Client>();
  const listeners = new Set<(event: LedgerEvent) => void>();
  const handlers = new Map<string, (message: ExtensionMessage, connection: Connection) => void>();
  const disconnectHandlers = new Set<(connection: Connection) => void>();
  function sendEvent(client: Client, event: LedgerEvent) {
    if (event.seq <= client.sentSeq) return;
    client.transport.send(encodeMessage({ type: 'event', ...event }));
    client.sentSeq = event.seq;
  }
  function reload(client: Client, baseline: number) {
    client.after = undefined;
    client.queue = [];
    client.sentSeq = baseline;
    client.helloSeq = baseline;
    client.transport.send(encodeMessage({ type: 'reload' }));
  }
  function reset(latest: number) {
    cursor = latest;
    for (const client of connections.values()) reload(client, latest);
  }
  async function resume(client: Client) {
    const after = client.after!;
    const page = await call('events.since', { after_seq: after, limit: 1000 }) as Page;
    if (closed || !connections.has(client.transport) || client.after !== after) return;
    const latest = page.latest_seq;
    if (latest < cursor) { reset(latest); return; }
    if (after > latest) { reload(client, latest); return; }
    if (after > client.sentSeq) {
      client.sentSeq = after;
      for (const event of client.queue) sendEvent(client, event);
      client.queue = [];
      client.after = undefined;
      return;
    }
    if (latest - after > 1000) { reload(client, latest); return; }
    const events = page.events.filter(event => event.seq <= latest);
    let end = events.at(-1)?.seq ?? after;
    while (end < latest) {
      const next = await call('events.since', { after_seq: end, limit: 1000 }) as Page;
      if (closed || !connections.has(client.transport) || client.after !== after) return;
      if (next.latest_seq < latest) { reset(next.latest_seq); return; }
      const batch = next.events.filter(event => event.seq > end && event.seq <= latest);
      if (!batch.length) throw new Error('续传事件暂不可用');
      events.push(...batch);
      end = batch.at(-1)!.seq;
    }
    client.sentSeq = after;
    for (const event of events) sendEvent(client, event);
    for (const event of client.queue) sendEvent(client, event);
    client.queue = [];
    client.after = undefined;
  }
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
        if (page.latest_seq < cursor) { reset(page.latest_seq); break; }
        for (const event of page.events) {
          if (event.seq <= cursor) continue;
          for (const client of connections.values()) {
            if (client.after !== undefined) client.queue.push(event);
            else sendEvent(client, event);
          }
          cursor = event.seq;
          for (const listener of listeners) listener(event);
        }
        if (!page.events.length || cursor >= page.latest_seq) break;
      }
      for (const client of connections.values()) {
        if (!closed && client.after !== undefined) {
          try { await resume(client); }
          catch (error) { log(`[事件] 续传失败：${error instanceof Error ? error.message : String(error)}`); }
        }
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
  function disconnect(transport: Transport) {
    const client = connections.get(transport);
    if (!client) return;
    connections.delete(transport);
    for (const handler of disconnectHandlers) handler(client.connection);
  }
  return {
    pull,
    setCoreStatus(status: CoreStatus) {
      const next = mapCoreStatus(status);
      if (encodeMessage(next) !== encodeMessage(core)) {
        core = next;
        for (const client of connections.values()) client.transport.send(encodeMessage(core));
      }
      if (status.state === 'connected') void pull();
    },
    connect(transport: Transport) {
      if (closed) { transport.close(1001, '服务正在关闭'); return () => {}; }
      const connection = { send: (message: ExtensionMessage) => transport.send(encodeMessage(message)) };
      connections.set(transport, { transport, connection, sentSeq: cursor, helloSeq: cursor, queue: [] });
      transport.send(encodeMessage({ type: 'hello', seq: cursor, core: core.status }));
      return () => disconnect(transport);
    },
    receive(transport: Transport, text: string) {
      const client = connections.get(transport);
      const message = parseClientMessage(text);
      if (!client || !message) return;
      if (message.type !== 'resume') { handlers.get(message.type)?.(message, client.connection); return; }
      const after = message.after as number;
      if (client.after !== undefined || after === client.sentSeq) return;
      if (after < client.sentSeq && client.sentSeq > client.helloSeq) { reload(client, client.sentSeq); return; }
      client.after = after;
      void pull();
    },
    broadcast(message: ExtensionMessage) {
      for (const client of connections.values()) client.connection.send(message);
    },
    onClientMessage(type: string, handler: (message: ExtensionMessage, connection: Connection) => void) {
      handlers.set(type, handler);
      return () => { handlers.delete(type); };
    },
    onDisconnect(handler: (connection: Connection) => void) {
      disconnectHandlers.add(handler);
      return () => { disconnectHandlers.delete(handler); };
    },
    subscribe(listener: (event: LedgerEvent) => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    close() {
      closed = true;
      for (const transport of connections.keys()) { disconnect(transport); transport.close(1001, '服务正在关闭'); }
      listeners.clear();
      handlers.clear();
      disconnectHandlers.clear();
    },
  };
}
