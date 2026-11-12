import { isDeepStrictEqual } from 'node:util';
import { basename } from 'node:path';
import type { LedgerEvent, ExtensionMessage } from './eventbus-protocol.ts';
import type { openWebStore, NotificationInput, Notification } from './store.ts';
import { matchEvent, renderTitle, type EventPayload } from './notifier-rules.ts';

type Page = { events: LedgerEvent[]; latest_seq: number };
type TaskState = { task_id: string; state: unknown; seq: number };
type Dashboard = { waiting: { project_id: string; task: { id: string; title: string; state: unknown } }[] };
type Info = { project?: { id: string; default_branch?: string; display_name?: string; path?: string }; task?: { title: string }; default_branch?: string; display_name?: string; path?: string };
export function createNotifier({ store, call, publish, log, pageSize = 200 }: {
  store: ReturnType<typeof openWebStore>; call: (method: string, params: Record<string, unknown>) => Promise<unknown>;
  publish: (message: ExtensionMessage) => void; log: (line: string) => void; pageSize?: number;
}) {
  let closed = false, again = false;
  let pending: Promise<void> | undefined;
  const cache = new Map<string, Info>();
  const save = (seq: number, notification?: NotificationInput) => store.commit({ cursor: { name: 'notifier', seq }, notification });
  const push = (row: Notification) => publish({ type: 'notification', id: row.id, kind: row.kind, task_id: row.task_id, title: row.title });
  async function takeSnapshot() {
    for (let attempt = 0; attempt < 3; attempt++) {
      const before = await call('events.since', { after_seq: Number.MAX_SAFE_INTEGER }) as Page;
      if (closed) return;
      const snapshot = await call('dashboard.get', {}) as Dashboard;
      if (closed) return;
      const after = await call('events.since', { after_seq: Number.MAX_SAFE_INTEGER }) as Page;
      if (closed) return;
      if (before.latest_seq !== after.latest_seq) continue;
      const seq = before.latest_seq;
      const notifications: NotificationInput[] = [], taskStates: TaskState[] = [];
      for (const { project_id, task } of snapshot.waiting) {
        const event: LedgerEvent = { seq, kind: 'task.updated', subject: task.id, payload: { state: task.state } };
        const rule = matchEvent(event);
        if (!rule || rule.level !== 'need_you') continue;
        notifications.push({ kind: rule.kind, title: renderTitle(rule, event), body: task.title, project_id, task_id: task.id, event_seq: seq });
        taskStates.push({ task_id: task.id, state: task.state, seq });
      }
      return { seq, notifications, taskStates };
    }
    throw new Error('快照与账本序号不一致');
  }
  async function seed(until: number) {
    const states = new Map<string, TaskState>();
    let after = 0;
    while (after < until) {
      const page = await call('events.since', { after_seq: after, limit: pageSize }) as Page;
      if (closed || page.latest_seq < until) return;
      for (const event of [...page.events].sort((a, b) => a.seq - b.seq)) {
        if (event.seq > until) break;
        after = Math.max(after, event.seq);
        const state = (event.payload as EventPayload).state;
        if (event.kind === 'task.updated' && !isDeepStrictEqual(states.get(event.subject)?.state, state)) {
          states.set(event.subject, { task_id: event.subject, state, seq: event.seq });
        }
      }
      if (!page.events.length || page.events.at(-1)!.seq >= until) break;
    }
    store.seedTaskStates([...states.values()]);
  }
  async function drain() {
    do {
      again = false;
      let cursor = store.getCursor('notifier');
      if (cursor === undefined) {
        const snapshot = await takeSnapshot();
        if (!snapshot) return;
        store.backfill({ ...snapshot, cursor: { name: 'notifier', seq: snapshot.seq } }).forEach(push);
        cursor = snapshot.seq;
      }
      const seedCursor = store.getCursor('notifier_seed');
      if (seedCursor !== undefined) await seed(seedCursor);
      while (!closed) {
        const page = await call('events.since', { after_seq: cursor, limit: pageSize }) as Page;
        if (closed) return;
        if (page.latest_seq < cursor) {
          const snapshot = await takeSnapshot();
          if (!snapshot) return;
          const rows = store.resetLedger('notifier', snapshot.seq, snapshot);
          cache.clear();
          log(`[通知] 账本已更换（最新序号 ${snapshot.seq} 小于游标 ${cursor}），从 ${snapshot.seq} 继续`);
          rows.forEach(push);
          cursor = snapshot.seq;
          continue;
        }
        for (const event of [...page.events].sort((a, b) => a.seq - b.seq)) {
          if (event.seq <= cursor) continue;
          const payload = event.payload as EventPayload;
          const prev = event.kind === 'task.updated' ? store.getTaskState(event.subject) : undefined;
          const changed = event.kind === 'task.updated' && (!prev || !isDeepStrictEqual(prev.state, payload.state));
          const taskState = changed ? { task_id: event.subject, state: payload.state, seq: event.seq } : undefined;
          const rule = event.kind === 'task.updated' && !changed ? undefined : matchEvent(event);
          let notification: NotificationInput | undefined;
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
            notification = {
              kind: rule.kind, project_id: project ? (cache.has(key) ? event.subject : null) : info.project?.id ?? null,
              task_id: project ? payload.task_id ?? null : event.subject,
              title: renderTitle(rule, event, info.default_branch || 'main'),
              body: project ? (cache.has(key) ? `${info.display_name || basename(info.path ?? '')}：${payload.task_id ?? ''} 合并后验证未通过` : null) : info.task?.title ?? null,
              event_seq: event.seq,
            };
          }
          if (notification || taskState) {
            const row = store.commit({ cursor: { name: 'notifier', seq: event.seq }, notification, taskState });
            if (row) push(row);
          }
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
