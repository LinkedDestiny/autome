import { TEST_IDS } from '../lib/testids.ts';
import { useEffect, useLayoutEffect, useRef } from 'react';
import { Link, useSearchParams } from 'react-router';
import { pinTimeline, withParams } from './task-url.ts';
import { SessionLog } from './session-log.tsx';
import { TIMELINE_TEXT as t, logName, type TimelineEntry } from './task-timeline-model.ts';
export function TaskTimeline({ entries, window, truncated, error }: { entries: TimelineEntry[]; window: { from: string | null; earlier: number }; truncated?: boolean; error?: string }) {
  const [search, setSearchParams] = useSearchParams();
  const container = useRef<HTMLElement>(null);
  const savedScroll = useRef<number | null>(null);
  useEffect(() => {
    if (pinTimeline(search, window.from) !== search) setSearchParams(prev => pinTimeline(prev, window.from), { replace: true, preventScrollReset: true });
  }, [search, setSearchParams, window.from]);
  useLayoutEffect(() => {
    if (savedScroll.current !== null && container.current) { container.current.scrollTop = savedScroll.current; savedScroll.current = null; }
  }, [entries]);
  const text = (value: string, log?: string) => {
    const suffix = log ? '日志 ' + logName(log) : '';
    return suffix && value.endsWith(suffix) ? <>{value.slice(0, -suffix.length)}<span className="timeline-log" title={log}>{suffix}</span></> : value;
  };
  return <section ref={container} className="task-timeline" data-testid={TEST_IDS.timeline} data-scroll-region aria-label={t.title}>
    <h2>{t.title}</h2>{truncated && <p>{t.truncated}</p>}{error && <p role="status">{error}</p>}
    {window.earlier > 0 && <Link preventScrollReset to={`?${withParams(search, { set: { timeline: 'all' }, remove: ['from'] })}`} onClick={() => { savedScroll.current = container.current?.scrollTop ?? null; }}>显示更早的 {window.earlier} 个会话</Link>}
    {!entries.length && <p>{t.empty}</p>}
    <ol>{entries.map(entry => <li data-testid={TEST_IDS.timelineEntry} data-entry-key={entry.key} data-node={entry.node} key={entry.key} className={entry.session ? 'timeline-session' : 'timeline-step'}>
      <h3>{text(entry.title, entry.log)}</h3><p className={entry.unverified ? 'timeline-unverified' : undefined}>{entry.summary}</p>
      <p className="timeline-meta">{text(entry.details.join(' · '), entry.log)}</p>
      {entry.failures?.filter(Boolean).map((failure, index) => <details className="timeline-failure" key={index}><summary>{t.failure}</summary><p>{failure}</p></details>)}
      {entry.session && <SessionLog session={entry.session} defaultOpen={entry.session.running} />}
    </li>)}</ol>
  </section>;
}
