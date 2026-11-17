import type { TaskEvent } from '../../server/task-history.ts';
import type { TaskState } from './labels.ts';
export const WAITING_NODES = ['await_design_approval', 'await_merge', 'await_info', 'not_a_bug'] as const;
export function waitingKind(state: TaskState): string | null {
  if (state.state === 'failed') return 'failed';
  return state.state === 'active' && WAITING_NODES.some(node => node === state.node) ? state.node! : null;
}
export function isCountedGate(kind: string, node: unknown): boolean {
  return kind === 'gate.result' && ['gate', 'fix_gate', 'merge_gate'].includes(String(node));
}
export type WaitingInterval = { start: number; end: number; kind: string };
export type OpenWaiting = { start: number; kind: string };
export function waitingTransition(current: OpenWaiting | undefined, state: TaskState, at: number) {
  const kind = waitingKind(state);
  if (kind === current?.kind) return { open: current };
  return { closed: current ? { ...current, end: at } : undefined, open: kind ? { kind, start: at } : undefined };
}

export function taskWaits(events: readonly TaskEvent[]) {
  let open: OpenWaiting | undefined;
  const closed: (WaitingInterval & { seq: number })[] = [];
  for (const event of events.toSorted((a, b) => a.seq - b.seq)) {
    if (event.kind !== 'task.updated' || !event.payload.state) continue;
    const next = waitingTransition(open, event.payload.state as TaskState, Date.parse(event.at));
    if (next.closed) closed.push({ ...next.closed, seq: event.seq });
    open = next.open;
  }
  return { closed, open };
}
