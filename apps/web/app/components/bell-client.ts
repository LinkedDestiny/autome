import type { LiveMessage, LiveMessageListener } from '../lib/live-client.ts';

export type NotificationItem = {
  id: string; kind: string; level: string; project_id: string | null; task_id: string | null;
  title: string; body: string | null; created_at: string; read_at: string | null;
};
export type BellSnapshot = { unread: number; items: NotificationItem[]; error?: string };
export const EMPTY_BELL: BellSnapshot = { unread: 0, items: [] };

export function createBellClient(options: {
  fetch: typeof fetch;
  onChange: (snapshot: BellSnapshot) => void;
  onNotification?: (message: LiveMessage, items: NotificationItem[] | null) => void;
}) {
  let snapshot = EMPTY_BELL;
  let stopped = false;
  let flight: Promise<void> | undefined;
  let again = false;
  let pending: LiveMessage[] = [];
  const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
  async function request(path: string, init?: RequestInit) {
    const response = await options.fetch(path, init);
    const result = await response.json();
    if (!response.ok || !result.ok) throw new Error(result.error ?? `请求失败（${response.status}）`);
    return result.payload;
  }
  function refresh(): Promise<void> {
    if (stopped) return Promise.resolve();
    if (flight) { again = true; return flight; }
    flight = (async () => {
      do {
        again = false;
        const messages = pending;
        pending = [];
        let items: NotificationItem[] | null = null;
        try {
          const data = await request('/api/notifications');
          if (stopped) return;
          snapshot = { unread: data.unread, items: data.items };
          items = snapshot.items;
        } catch (error) {
          if (stopped) return;
          snapshot = { ...snapshot, error: `通知加载失败：${errorText(error)}` };
        }
        options.onChange(snapshot);
        for (const message of messages) options.onNotification?.(message, items);
      } while (again && !stopped);
    })().finally(() => { flight = undefined; });
    return flight;
  }
  async function read(body: { ids: string[] } | { all: true }) {
    if (stopped) return;
    try {
      await request('/api/notifications/read', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
      await refresh();
    } catch (error) {
      if (!stopped) {
        snapshot = { ...snapshot, error: `标记已读失败：${errorText(error)}` };
        options.onChange(snapshot);
      }
    }
  }
  const handleMessage: LiveMessageListener = (type, message) => {
    if (stopped) return;
    if (type === 'notification') pending.push(message);
    if (type === 'hello' || type === 'notification' || type === 'notification-read') void refresh();
  };
  return {
    start: refresh, handleMessage,
    markRead: (id: string) => read({ ids: [id] }), markAll: () => read({ all: true }),
    stop() { stopped = true; pending = []; },
  };
}
