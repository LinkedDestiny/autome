import type { openWebStore } from './store.ts';
import type { createEventBus } from './eventbus.ts';
import type { createNotifier } from './notifier.ts';
import type { CoreStatus } from './core/supervisor.ts';
import type { ExtensionMessage } from './eventbus-protocol.ts';
import { levelOf } from './notifier-rules.ts';

export function attachNotifier({ bus, notifier }: {
  bus: Pick<ReturnType<typeof createEventBus>, 'pull' | 'setCoreStatus'>;
  notifier: Pick<ReturnType<typeof createNotifier>, 'pull'>;
}) {
  return {
    onTick() { void bus.pull().then(() => notifier.pull()); },
    onStatus(status: CoreStatus) {
      bus.setCoreStatus(status);
      if (status.state === 'connected') void notifier.pull();
    },
  };
}

export function createNotificationApi({ store, publish }: {
  store: ReturnType<typeof openWebStore>; publish: (message: ExtensionMessage) => void;
}) {
  const reject = (error: string) => ({ status: 400, body: { ok: false as const, error: `已拒绝：${error}` } });
  return {
    list(rawLimit: unknown) {
      const limit = rawLimit === undefined ? 50 : typeof rawLimit === 'string' && /^\d+$/.test(rawLimit) ? Number(rawLimit) : NaN;
      if (!Number.isInteger(limit) || limit < 1 || limit > 200) return reject('limit 必须是 1 到 200 的整数');
      const items = store.list({ limit }).map(({ event_seq, ...item }) => { void event_seq; return { ...item, level: levelOf(item.kind) }; });
      return { status: 200, body: { ok: true as const, payload: { unread: store.unreadCount(), items } } };
    },
    read(body: unknown) {
      if (!body || typeof body !== 'object' || Array.isArray(body)) return reject('请提供 ids 或 all');
      const value = body as Record<string, unknown>;
      const keys = Object.keys(value);
      const all = keys.length === 1 && value.all === true;
      const ids = keys.length === 1 && Array.isArray(value.ids) && value.ids.length >= 1 && value.ids.length <= 200
        && value.ids.every(id => typeof id === 'string' && id.length <= 64) ? value.ids as string[] : undefined;
      if (!all && !ids) return reject('请提供 1 到 200 个不超过 64 字的 ids，或 all: true');
      const changed = all ? store.markAllRead() : store.markRead(ids!);
      if (changed.length) publish({ type: 'notification-read', ids: changed });
      return { status: 200, body: { ok: true as const, payload: { unread: store.unreadCount(), ids: changed } } };
    },
  };
}
