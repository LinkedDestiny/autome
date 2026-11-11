import type { CoreMessage, LedgerEvent, ServerMessage } from '../../server/eventbus-protocol.ts';

export type ConnectionState = 'connecting' | 'open' | 'reconnecting';
export type LiveSocket = Pick<WebSocket, 'send' | 'close' | 'onmessage' | 'onclose' | 'onerror'>;
export const reconnectDelay = (attempt: number) => Math.min(500 * 2 ** attempt, 10000);

export function createLiveClient(options: {
  url: string;
  createSocket: (url: string) => LiveSocket;
  onEvents: (events: LedgerEvent[]) => void;
  onReload: () => void;
  onCore: (core: CoreMessage) => void;
  onConnection: (state: ConnectionState) => void;
}) {
  let socket: LiveSocket;
  let stopped = false;
  let connectedBefore = false;
  let lastSeq: number | null = null;
  let attempt = 0;
  let retry: ReturnType<typeof setTimeout> | undefined;
  let flush: ReturnType<typeof setTimeout> | undefined;
  let batch: LedgerEvent[] = [];
  function reconnect() {
    if (stopped || retry !== undefined) return;
    options.onConnection('reconnecting');
    retry = setTimeout(() => { retry = undefined; connect(); }, reconnectDelay(attempt++));
  }
  function connect() {
    if (stopped) return;
    try { socket = options.createSocket(options.url); } catch { reconnect(); return; }
    const current = socket;
    current.onclose = () => { if (current === socket) reconnect(); };
    current.onerror = () => current.close();
    current.onmessage = event => {
      if (stopped || current !== socket) return;
      let message: ServerMessage;
      try { message = JSON.parse(String(event.data)); } catch { return; }
      if (!message) return;
      switch (message.type) {
        case 'hello':
          if (lastSeq !== null) current.send(JSON.stringify({ type: 'resume', after: lastSeq }));
          else {
            lastSeq = message.seq;
            if (connectedBefore) options.onReload();
          }
          connectedBefore = true;
          attempt = 0;
          options.onCore({ type: 'core', status: message.core });
          options.onConnection('open');
          break;
        case 'event':
          if (lastSeq !== null && message.seq <= lastSeq) break;
          lastSeq = message.seq;
          batch.push(message);
          flush ??= setTimeout(() => {
            flush = undefined;
            const events = batch;
            batch = [];
            options.onEvents(events);
          }, 0);
          break;
        case 'reload':
          lastSeq = null;
          batch = [];
          clearTimeout(flush);
          flush = undefined;
          options.onReload();
          break;
        case 'core': options.onCore(message); break;
      }
    };
  }
  options.onConnection('connecting');
  connect();
  return { stop() {
    stopped = true;
    clearTimeout(retry);
    clearTimeout(flush);
    batch = [];
    socket?.close();
  } };
}
