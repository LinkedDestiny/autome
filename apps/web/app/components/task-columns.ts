import type { TaskState } from '../lib/labels.ts';
export const AWAITING_NODES: readonly string[] = ['await_design_approval', 'await_merge', 'await_info', 'not_a_bug'];
export type TaskColumn = 'todo' | 'running' | 'needs_you' | 'done';
export function columnOf(state: TaskState): TaskColumn | null {
  switch (state.state) {
    case 'draft': case 'queued': return 'todo';
    case 'active': return AWAITING_NODES.includes(state.node ?? '') ? 'needs_you' : 'running';
    case 'paused': case 'stopped': case 'failed': return 'needs_you';
    case 'done': return 'done';
    default: return null;
  }
}
export function countByColumn(tasks: readonly { state: TaskState }[]): Record<TaskColumn, number> {
  const counts = { todo: 0, running: 0, needs_you: 0, done: 0 };
  for (const task of tasks) {
    const column = columnOf(task.state);
    if (column !== null) counts[column]++;
  }
  return counts;
}
