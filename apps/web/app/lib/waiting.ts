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
