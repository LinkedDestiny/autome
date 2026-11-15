export type TaskEvent = { seq: number; kind: string; subject: string; payload: Record<string, unknown>; at: string };
type Page = { events: TaskEvent[]; latest_seq: number };
type Call = (method: string, params: Record<string, unknown>) => Promise<unknown>;
const noise = new Set(['task.head', 'integrity_warning', 'task.lessons']);
export async function readTaskEvents(call: Call, taskId: string) {
  let cursor = 0;
  let events: TaskEvent[] = [];
  let truncated = false;
  const limit = 1000;
  for (;;) {
    const page = await call('events.since', { subject: taskId, after_seq: cursor, limit }) as Page;
    if (page.events.length) {
      const next = page.events.at(-1)!.seq;
      if (next <= cursor) throw new Error('任务事件暂不可用：账本分页未前进');
      cursor = next;
    }
    events.push(...page.events.filter(event => !noise.has(event.kind)));
    if (events.length > 5000) { events = events.slice(-5000); truncated = true; }
    // latest_seq is ledger-wide, not the last event belonging to this task.
    if (page.events.length < limit) return { events, truncated };
  }
}
