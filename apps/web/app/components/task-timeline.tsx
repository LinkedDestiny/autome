import { SegmentedTabs } from './ui/segmented.tsx';
import { timelineGroups } from './task-timeline-model.ts';
import { Ribbon } from './ui/ribbon.tsx';
import { Pill } from './ui/pill.tsx';
import { formatClock, formatDuration, formatRange, useClock } from '../lib/clock.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { useEffect, useLayoutEffect, useRef } from 'react';
import { Link, useSearchParams } from 'react-router';
import { pinTimeline, withParams } from './task-url.ts';
import { SessionLog } from './session-log.tsx';
import { TIMELINE_TEXT as t, logName, type TimelineEntry } from './task-timeline-model.ts';
export function TaskTimeline({ entries, window, truncated, error }: { entries: TimelineEntry[]; window: { from: string | null; earlier: number }; truncated?: boolean; error?: string }) {
  const clock = useClock();
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
    <Ribbon small>执行过程</Ribbon><SegmentedTabs className="timeline-filters" label="执行过程筛选">{[['', '全部'], ['gates', '只看门禁'], ['reopens', '只看退回']].map(([value, label]) => <Link key={value} preventScrollReset aria-current={(search.get('show') ?? '') === value ? 'page' : undefined} to={`?${withParams(search, { set: { show: value } })}`}>{label}</Link>)}</SegmentedTabs>{truncated && <p>{t.truncated}</p>}{error && <p role="status">{error}</p>}
    {window.earlier > 0 && <Link className="btn btn-sm" preventScrollReset to={`?${withParams(search, { set: { timeline: 'all' }, remove: ['from'] })}`} onClick={() => { savedScroll.current = container.current?.scrollTop ?? null; }}>显示更早的 {window.earlier} 个会话</Link>}
    {!entries.length && <p>{t.empty}</p>}
    <div>{timelineGroups(entries, search.get('show')).map(group => <section key={group.label}><h2 className="timeline-group-title">{group.label}<small>{formatRange(group.start, group.end, clock)}</small></h2><ol>{group.entries.map(entry => <li data-testid={TEST_IDS.timelineEntry} data-entry-key={entry.key} data-node={entry.node} key={entry.key} className={`timeline-row ${entry.session ? 'timeline-session' : 'timeline-step'} timeline-${entry.kind ?? 'event'}`}>
      <time dateTime={entry.at}>{formatClock(entry.at, { ...clock, now: entry.at })}</time>
      <Pill color={entry.session ? 'purple' : entry.kind === 'user' ? 'yellow' : 'teal'}>{entry.session?.label ?? (entry.kind === 'gate' ? '门禁' : entry.kind === 'user' ? '你' : '系统')}</Pill>
      <details className="timeline-text"><summary><b>{text(entry.title, entry.log)}</b> <span className={entry.unverified ? 'timeline-unverified' : undefined}>{entry.summary}</span></summary>{entry.details.length > 0 && <div>{entry.details.map((detail, index) => <p key={index}>{text(detail, entry.log)}</p>)}</div>}</details>
      <span className="timeline-metrics">{entry.durationMs != null ? formatDuration(entry.durationMs) : ''}{entry.cost ? ' · ' + entry.cost : ''}</span>
      <div className="timeline-result">{entry.outcome && <Pill color={entry.outcome === 'fail' ? 'red' : entry.outcome === 'pass' ? 'green' : 'orange'}>{t.outcome[entry.outcome]}</Pill>}{entry.failures?.filter(Boolean).map((failure, index) => <details className="timeline-failure" key={index}><summary>{t.failure}</summary><p>{failure}</p></details>)}</div>
      {entry.session && <SessionLog session={entry.session} defaultOpen={entry.session.running} />}
    </li>)}</ol></section>)}</div>
  </section>;
}
