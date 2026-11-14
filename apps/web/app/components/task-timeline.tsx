import { useClock } from '../lib/clock.tsx';
import { SessionLog } from './session-log.tsx';
import { TIMELINE_TEXT as t, timelineEntries, type TimelineSession } from './task-timeline-model.ts';
import type { TaskEvent } from '../../server/task-events.ts';
export function TaskTimeline({ sessions, events, now, truncated, error }: { sessions: readonly TimelineSession[]; events: readonly TaskEvent[]; now: string; truncated?: boolean; error?: string }) {
  const entries = timelineEntries(sessions, events, now, useClock().timeZone);
  return <section className="task-timeline" aria-label={t.title}>
    <h2>{t.title}</h2>{truncated && <p>{t.truncated}</p>}{error && <p role="status">{error}</p>}
    {!entries.length && <p>{t.empty}</p>}
    <ol>{entries.map(entry => <li key={entry.key} className={entry.session ? 'timeline-session' : 'timeline-step'}>
      <h3>{entry.title}</h3><p className={entry.unverified ? 'timeline-unverified' : undefined}>{entry.summary}</p>
      <p className="timeline-meta">{entry.details.join(' · ')}</p>
      {entry.session && <SessionLog session={entry.session} defaultOpen={entry.session.running} />}
    </li>)}</ol>
  </section>;
}
