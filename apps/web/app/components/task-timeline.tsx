import { TEST_IDS } from '../lib/testids.ts';
import { useClock } from '../lib/clock.tsx';
import { SessionLog } from './session-log.tsx';
import { TIMELINE_TEXT as t, logName, timelineEntries, type TimelineSession } from './task-timeline-model.ts';
import type { TaskEvent } from '../../server/task-events.ts';
export function TaskTimeline({ sessions, events, now, truncated, error }: { sessions: readonly TimelineSession[]; events: readonly TaskEvent[]; now: string; truncated?: boolean; error?: string }) {
  const entries = timelineEntries(sessions, events, now, useClock().timeZone);
  const text = (value: string, log?: string) => {
    const suffix = log ? '日志 ' + logName(log) : '';
    return suffix && value.endsWith(suffix) ? <>{value.slice(0, -suffix.length)}<span className="timeline-log" title={log}>{suffix}</span></> : value;
  };
  return <section className="task-timeline" data-testid={TEST_IDS.timeline} data-scroll-region aria-label={t.title}>
    <h2>{t.title}</h2>{truncated && <p>{t.truncated}</p>}{error && <p role="status">{error}</p>}
    {!entries.length && <p>{t.empty}</p>}
    <ol>{entries.map(entry => <li data-testid={TEST_IDS.timelineEntry} data-entry-key={entry.key} data-node={entry.node} key={entry.key} className={entry.session ? 'timeline-session' : 'timeline-step'}>
      <h3>{text(entry.title, entry.log)}</h3><p className={entry.unverified ? 'timeline-unverified' : undefined}>{entry.summary}</p>
      <p className="timeline-meta">{text(entry.details.join(' · '), entry.log)}</p>
      {entry.failures?.filter(Boolean).map((failure, index) => <details className="timeline-failure" key={index}><summary>{t.failure}</summary><p>{failure}</p></details>)}
      {entry.session && <SessionLog session={entry.session} defaultOpen={entry.session.running} />}
    </li>)}</ol>
  </section>;
}
