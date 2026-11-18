import { isCountedGate, waitingKind, waitingTransition, type OpenWaiting, type WaitingInterval } from '../app/lib/waiting.ts';
import type { TaskState } from '../app/lib/labels.ts';
import type { Clock } from '../app/lib/clock.tsx';
import type { TodayWaiting } from '../app/lib/overview-model.ts';
import type { ShellTask } from '../app/components/shell-data.ts';
import type { TaskEvent } from './task-history.ts';
type Call = (method: string, params: Record<string, unknown>) => Promise<unknown>;
type Page = { events: TaskEvent[]; latest_seq: number };
export type TodaySummary = TodayWaiting & { cost: number; billed: number; unbilled: number; gates: number; passed: number; merged: number; ended: Record<string, number> };
// Find local midnight in the server's IANA zone, including 23/25-hour DST days.
export function dayBounds(clock: Clock) {
  const formatter = new Intl.DateTimeFormat('en-CA', { timeZone: clock.timeZone, year: 'numeric', month: '2-digit', day: '2-digit' });
  const now = Date.parse(clock.now), key = formatter.format(now);
  function boundary(lo: number, hi: number, before: (at: number) => boolean) {
    while (hi - lo > 1) { const mid = Math.floor((lo + hi) / 2); if (before(mid)) lo = mid; else hi = mid; }
    return hi;
  }
  const dayStart = boundary(now - 48 * 3600_000, now, at => formatter.format(at) !== key);
  const dayEnd = boundary(now, now + 48 * 3600_000, at => formatter.format(at) === key);
  return { dayStart, dayEnd };
}
export function recentMerged(tasks: ShellTask[]) {
  return tasks.filter(t => t.state.state === 'done').sort((a, b) => Date.parse(b.completed_at ?? '') - Date.parse(a.completed_at ?? '')).slice(0, 8);
}
export function createOverviewSummary(call: Call) {
  let cursor = 0;
  let open = new Map<string, OpenWaiting>();
  let closed: (WaitingInterval & { task: string })[] = [];
  let costs: { at: number; cost: number | null }[] = [];
  let gates: { at: number; ok: boolean }[] = [];
  let pending: Promise<void> = Promise.resolve();
  async function consume() {
    for (;;) {
      const page = await call('events.since', { after_seq: cursor, limit: 1000 }) as Page;
      if (page.latest_seq < cursor) { cursor = 0; open = new Map(); closed = []; costs = []; gates = []; continue; }
      for (const event of page.events) {
        if (event.seq <= cursor) throw Error('总览事件分页未前进');
        const at = Date.parse(event.at);
        if (event.kind === 'task.updated' && event.payload.state) {
          const transition = waitingTransition(open.get(event.subject), event.payload.state as TaskState, at);
          if (transition.closed) closed.push({ ...transition.closed, task: event.subject });
          if (transition.open) open.set(event.subject, transition.open); else open.delete(event.subject);
        }
        if (event.kind === 'session.ended') costs.push({ at, cost: typeof event.payload.cost_usd === 'number' ? event.payload.cost_usd : null });
        if (isCountedGate(event.kind, event.payload.node)) gates.push({ at, ok: event.payload.ok === true });
        cursor = event.seq;
      }
      if (cursor >= page.latest_seq) return;
      if (!page.events.length) throw Error('总览事件分页未前进');
    }
  }
  return async (tasks: ShellTask[], clock: Clock): Promise<{ today: TodaySummary; waiting: Record<string, number> }> => {
    // Serialize overlapping loaders so each ledger event is consumed exactly once.
    const job = pending.then(consume); pending = job.catch(() => {}); await job;
    const { dayStart, dayEnd } = dayBounds(clock);
    const yesterday = dayBounds({ ...clock, now: new Date(dayStart - 1).toISOString() }).dayStart;
    closed = closed.filter(item => item.end >= yesterday);
    costs = costs.filter(item => item.at >= yesterday); gates = gates.filter(item => item.at >= yesterday);
    const todayCosts = costs.filter(item => item.at >= dayStart && item.at < dayEnd);
    const todayGates = gates.filter(item => item.at >= dayStart && item.at < dayEnd);
    const present = new Map(tasks.map(task => [task.id, task]));
    const currentOpen = [...open].filter(([id]) => { const task = present.get(id); return task && waitingKind(task.state); });
    const currentClosed = closed.filter(item => present.has(item.task));
    const ended: Record<string, number> = {};
    for (const item of currentClosed) if (item.end >= dayStart && item.end < dayEnd) ended[item.kind] = (ended[item.kind] ?? 0) + 1;
    return { today: { dayStart, dayEnd,
      closedMs: currentClosed.reduce((sum, item) => sum + Math.max(0, Math.min(item.end, dayEnd) - Math.max(item.start, dayStart)), 0),
      openStarts: currentOpen.map(([, item]) => item.start), ended,
      cost: todayCosts.reduce((sum, item) => sum + (item.cost ?? 0), 0), billed: todayCosts.filter(item => item.cost !== null).length,
      unbilled: todayCosts.filter(item => item.cost === null).length,
      gates: todayGates.length, passed: todayGates.filter(item => item.ok).length,
      merged: tasks.filter(t => t.state.state === 'done' && Date.parse(t.completed_at ?? '') >= dayStart && Date.parse(t.completed_at ?? '') < dayEnd).length,
    }, waiting: Object.fromEntries(currentOpen.map(([id, item]) => [id, item.start])) };
  };
}
