import type { LedgerEvent } from '../../server/eventbus-protocol.ts';
import { matchEvent } from '../../server/notifier-rules.ts';

export function dashboardDouble(events: LedgerEvent[]) {
  const tasks = new Map<string, LedgerEvent>();
  for (const event of [...events].sort((a, b) => a.seq - b.seq)) {
    if (event.kind === 'task.updated') tasks.set(event.subject, event);
  }
  return {
    waiting: [...tasks.values()].filter(event => matchEvent(event)?.level === 'need_you').map(event => ({
      project_id: 'p1', project_name: '项目',
      task: { id: event.subject, title: '任务正文', state: (event.payload as { state: unknown }).state },
      gate_queue_position: null, pending_decisions: [], session: null, retry: null,
    })),
    running: [], alerts: [], cooldowns: [], config_warnings: [], environment: { probed: false, problems: [] },
  };
}
