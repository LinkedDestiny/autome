export type TaskEvent = { seq: number; kind: string; subject: string; payload: Record<string, unknown>; at: string };
type Page = { events: TaskEvent[]; latest_seq: number };
const noise = new Set(['task.head', 'integrity_warning', 'task.lessons']);
export function createTaskEvents({ call }: { call: (method: string, params: Record<string, unknown>) => Promise<unknown> }) {
  let cursor = 0;
  let closed = false;
  let subjects = new Map<string, { events: TaskEvent[]; truncated: boolean }>();
  let pending: Promise<void> | undefined;
  async function refresh() {
    let end = cursor;
    let reset = false;
    const collected: TaskEvent[] = [];
    while (!closed) {
      const page = await call('events.since', { after_seq: end, limit: 1000 }) as Page;
      if (closed) return;
      if (page.latest_seq < end) {
        end = 0; reset = true; collected.length = 0;
        continue;
      }
      const batch = page.events.filter(event => event.seq > end && event.seq <= page.latest_seq);
      collected.push(...batch);
      if (batch.length) end = batch.at(-1)!.seq;
      if (end >= page.latest_seq) break;
      if (!batch.length) throw new Error('任务事件暂不可用：账本分页未前进');
    }
    if (closed) return;
    // Commit only after every page succeeds; retries cannot lose partial pages.
    if (reset) subjects = new Map();
    for (const event of collected) {
      if (noise.has(event.kind)) continue;
      let group = subjects.get(event.subject);
      if (!group) { group = { events: [], truncated: false }; subjects.set(event.subject, group); }
      group.events.push(event);
      if (group.events.length > 5000) { group.events.shift(); group.truncated = true; }
    }
    cursor = end;
  }
  return {
    async forTask(taskId: string) {
      if (closed) throw new Error('任务事件服务已关闭');
      pending ??= refresh().finally(() => { pending = undefined; });
      await pending;
      const group = subjects.get(taskId);
      return { events: structuredClone(group?.events ?? []), truncated: group?.truncated ?? false };
    },
    close() { closed = true; subjects.clear(); },
  };
}
