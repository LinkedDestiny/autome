import { isDeepStrictEqual } from 'node:util';
import { basename } from 'node:path';
import type { LedgerEvent, ExtensionMessage } from './eventbus-protocol.ts';
import type { openWebStore, NotificationInput } from './store.ts';
import { matchEvent, renderTitle, type EventPayload } from './notifier-rules.ts';

type Page = { events: LedgerEvent[]; latest_seq: number };
type Info = { project?: { id: string; default_branch?: string; display_name?: string; path?: string }; task?: { title: string }; default_branch?: string; display_name?: string; path?: string };
export function createNotifier({ store, call, publish, log, pageSize = 200 }: {
  store: ReturnType<typeof openWebStore>; call: (method: string, params: Record<string, unknown>) => Promise<unknown>;
  publish: (message: ExtensionMessage) => void; log: (line: string) => void; pageSize?: number;
}) {
  let closed = false, again = false;
  let pending: Promise<void> | undefined;
  const states = new Map<string, unknown>();
  const cache = new Map<string, Info>();
  const save = (seq: number, notification?: NotificationInput) => store.commit({ cursor: { name: 'notifier', seq }, notification });
  async function drain() {
    do {
      again = false;
      let cursor = store.getCursor('notifier');
      if (cursor === undefined) {
        const page = await call('events.since', { after_seq: Number.MAX_SAFE_INTEGER }) as Page;
        if (closed) return;
        save(page.latest_seq);
        return;
      }
      while (!closed) {
        const page = await call('events.since', { after_seq: cursor, limit: pageSize }) as Page;
        if (closed) return;
        if (page.latest_seq < cursor) {
          store.resetLedger('notifier', page.latest_seq); states.clear(); cache.clear();
          log(`[通知] 账本已更换（最新序号 ${page.latest_seq} 小于游标 ${cursor}），从 ${page.latest_seq} 继续`);
          return;
        }
        for (const event of [...page.events].sort((a, b) => a.seq - b.seq)) {
          if (event.seq <= cursor) continue;
          const payload = event.payload as EventPayload;
          const rule = event.kind === 'task.updated' && states.has(event.subject) && isDeepStrictEqual(states.get(event.subject), payload.state) ? undefined : matchEvent(event);
          if (rule) {
            let info: Info = {};
            const key = `${rule.enrich}:${event.subject}`;
            try {
              if (!cache.has(key)) {
                const result = await call(`${rule.enrich}.get`, { [`${rule.enrich}_id`]: event.subject }) as Info;
                if (closed) return;
                cache.set(key, rule.enrich === 'project' ? result.project ?? {} : result);
              }
              info = cache.get(key)!;
            } catch (error) {
              if (closed) return;
              if (!error || typeof error !== 'object' || !('code' in error)) {
                save(cursor);
                log(`[通知] 补齐 ${event.subject} 的信息失败，稍后重试：${error instanceof Error ? error.message : String(error)}`);
                return;
              }
            }
            const project = rule.enrich === 'project';
            const row = save(event.seq, {
              kind: rule.kind, project_id: project ? (cache.has(key) ? event.subject : null) : info.project?.id ?? null,
              task_id: project ? payload.task_id ?? null : event.subject,
              title: renderTitle(rule, event, info.default_branch || 'main'),
              body: project ? (cache.has(key) ? `${info.display_name || basename(info.path ?? '')}：${payload.task_id ?? ''} 合并后验证未通过` : null) : info.task?.title ?? null,
              event_seq: event.seq,
            });
            if (row) publish({ type: 'notification', id: row.id, kind: row.kind, task_id: row.task_id, title: row.title });
          }
          if (event.kind === 'task.updated') states.set(event.subject, payload.state);
          cursor = event.seq;
        }
        save(cursor);
        if (!page.events.length || cursor >= page.latest_seq) break;
      }
    } while (again && !closed);
  }
  return {
    pull(): Promise<void> {
      if (closed) return Promise.resolve();
      if (pending) { again = true; return pending; }
      pending = drain().catch(error => log(`[通知] 拉取失败：${error instanceof Error ? error.message : String(error)}`)).finally(() => { pending = undefined; });
      return pending;
    },
    close() { closed = true; },
  };
}
