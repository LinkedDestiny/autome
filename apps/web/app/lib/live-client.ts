import type { CoreMessage, LedgerEvent, ServerMessage } from '../../server/eventbus-protocol.ts';

export type LiveMessage = { type: string } & Record<string, unknown>;
export type LiveMessageListener = (type: string, message: LiveMessage) => void;
const listeners = new Set<LiveMessageListener>();
export function subscribeLiveMessages(listener: LiveMessageListener) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
export function publishLiveMessage(type: string, message: LiveMessage) {
  for (const listener of listeners) {
    try { listener(type, message); } catch { /* 订阅者之间互不影响。 */ }
  }
}

export type LiveSender = (message: LiveMessage) => boolean;
let sender: LiveSender | undefined;
export function setLiveSender(value: LiveSender | undefined) { sender = value; }
export function sendLiveMessage(message: LiveMessage) { return sender?.(message) ?? false; }

export type ConnectionState = 'connecting' | 'open' | 'reconnecting';
export type LiveSocket = Pick<WebSocket, 'send' | 'close' | 'onmessage' | 'onclose' | 'onerror'>;
export const reconnectDelay = (attempt: number) => Math.min(500 * 2 ** attempt, 10000);

export function createLiveClient(options: {
  onMessage?: LiveMessageListener;
  url: string;
  createSocket: (url: string) => LiveSocket;
  onEvents: (events: LedgerEvent[]) => void;
  onReload: () => void;
  onCore: (core: CoreMessage) => void;
  onConnection: (state: ConnectionState) => void;
}) {
  let socket: LiveSocket;
  let stopped = false;
  let ready = false;
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
    ready = false;
    try { socket = options.createSocket(options.url); } catch { reconnect(); return; }
    const current = socket;
    current.onclose = () => { if (current === socket) { ready = false; reconnect(); } };
    current.onerror = () => current.close();
    current.onmessage = event => {
      if (stopped || current !== socket) return;
      let message: ServerMessage;
      try { message = JSON.parse(String(event.data)); } catch { return; }
      if (!message || typeof message !== 'object' || Array.isArray(message) || typeof message.type !== 'string') return;
      switch (message.type) {
        case 'hello':
          ready = true;
          if (lastSeq !== null) current.send(JSON.stringify({ type: 'resume', after: lastSeq }));
          else {
            lastSeq = message.seq;
            if (connectedBefore) options.onReload();
          }
          connectedBefore = true;
          attempt = 0;
          options.onCore({ type: 'core', status: message.core, ...(message.reason === undefined ? {} : { reason: message.reason }) });
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
      options.onMessage?.(message.type, message as LiveMessage);
    };
  }
  options.onConnection('connecting');
  connect();
  return { send(message: LiveMessage): boolean {
    if (stopped || !ready) return false;
    try { socket.send(JSON.stringify(message)); return true; } catch { return false; }
  }, stop() {
    stopped = true;
    ready = false;
    clearTimeout(retry);
    clearTimeout(flush);
    batch = [];
    socket?.close();
  } };
}
