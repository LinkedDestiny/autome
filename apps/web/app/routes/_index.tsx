import { useEffect, useRef } from 'react';
import { TEST_IDS } from '../lib/testids.ts';
import { environmentNotice } from '../components/env-model.ts';
import { ENV } from '../lib/admin-labels.ts';
import { Link, useRevalidator, useRouteLoaderData } from 'react-router';
import type { loader } from '../root.tsx';
import { COLUMN_LABELS } from '../lib/labels.ts';
import { overviewTasks } from '../components/shell-data.ts';
import { OverviewCard } from '../components/overview-card.tsx';
import { OVERVIEW, endedWaiting } from '../lib/overview-labels.ts';
import { mergeResult, needsNewDay, overviewConclusion, waitingMilliseconds } from '../lib/overview-model.ts';
import { formatRelative, formatSpan, useClock } from '../lib/clock.tsx';
import { Card } from '../components/ui/card.tsx';
import { Ribbon } from '../components/ui/ribbon.tsx';
import { Tile } from '../components/ui/tile.tsx';
import { Pill } from '../components/ui/pill.tsx';
export default function Overview() {
  const { kernel, projects, tasks, environment, drivers, overview } = useRouteLoaderData<typeof loader>('root')!;
  const notice = environmentNotice(environment), { now } = useClock();
  const revalidator = useRevalidator(), requestedDay = useRef<number | null>(null);
  const today = overview?.today;
  const waitMs = today ? waitingMilliseconds(today, now) : 0;
  useEffect(() => {
    if (today && needsNewDay(today, now) && requestedDay.current !== today.dayEnd) {
      requestedDay.current = today.dayEnd; void revalidator.revalidate();
    }
  }, [today, now, revalidator]);
  const waiting = overviewTasks(tasks, 'needs_you'), running = overviewTasks(tasks, 'running');
  const queued = tasks.filter(t => t.state.state === 'queued').sort((a, b) => (a.queue_position ?? Infinity) - (b.queue_position ?? Infinity));
  return <div className="overview" data-testid={TEST_IDS.pageOverview}>
    <header className="overview-heading"><Ribbon><h1>总览</h1></Ribbon><p className="overview-conclusion">{kernel.ok ? overviewConclusion(waiting.length, running.length, queued.length, drivers?.list.some(d => d.driving)) : '内核离线，任务统计暂不可用'}</p></header>
    {notice && <p role="status">{notice} <Link className="btn btn-text btn-sm" to="/env">{ENV.go}</Link></p>}
    <div className="overview-columns"><div className="overview-column-body" data-scroll-region>
      {(['needs_you', 'running'] as const).map(column => {
        const cards = column === 'needs_you' ? waiting : [...running, ...queued];
        return <section className="overview-column" key={column} aria-labelledby={`heading-${column}`}>
          <h2 id={`heading-${column}`}>{`${COLUMN_LABELS[column]}（${column === 'needs_you' ? waiting.length : running.length}）`}{column === 'running' && queued.length > 0 && <small> · {queued.length} 个排队</small>}</h2>
          {!kernel.ok ? <p>内核离线，暂时拿不到任务</p> : cards.length === 0 ? <Card dashed className="overview-empty"><b>{column === 'needs_you' ? OVERVIEW.emptyWait : OVERVIEW.emptyRun}</b><p>{column === 'needs_you' ? OVERVIEW.emptyWaitBody : OVERVIEW.emptyRunBody}</p>{column === 'running' && <Link className="btn btn-primary" to={projects.length ? `/projects/${encodeURIComponent(projects[0].id)}/tasks/new` : '/projects/new'}>新建任务</Link>}</Card> : cards.map(task => <OverviewCard key={task.id} task={task} detail={overview?.details[task.id]} waitingSince={overview?.waiting[task.id]} configError={projects.find(p => p.id === task.project_id)?.configError} projectName={projects.find(p => p.id === task.project_id)?.name ?? task.project_id} />)}
        </section>;
      })}
    </div><aside className="overview-side">
      <h2>{OVERVIEW.today}<small>所有项目</small></h2>
      {overview?.error && <p role="status">{overview.error}</p>}
      <div className="overview-tiles">
        <Tile color="teal" label={OVERVIEW.merged} value={today?.merged}>个任务</Tile>
        <Tile color="yellow" label="花费" value={today ? today.billed === 0 && today.unbilled > 0 ? '无计费' : `$${today.cost.toFixed(2)}` : null}>{today ? `${today.unbilled} 个会话无计费` : '数据暂不可用'}</Tile>
        <Tile color="blue" label="门禁" value={today ? `${today.passed} / ${today.gates}` : null}>{today ? today.gates ? today.passed === today.gates ? '全部通过' : '有未通过项' : '今天暂无门禁' : '数据暂不可用'}</Tile>
        <Tile color="pink" label="等你的时间" value={today ? waitMs === 0 ? OVERVIEW.zeroWait : formatSpan(waitMs) : null}>{today ? waitMs === 0 ? OVERVIEW.noWait : endedWaiting(today.ended) || OVERVIEW.ongoingWait : '数据暂不可用'}</Tile>
      </div>
      <Card className="overview-recent"><h2>{OVERVIEW.recent}</h2>
        <div className="overview-merged-list" data-scroll-region>{overview?.listError ? <p>{overview.listError}</p> : overview?.recent.length ? overview.recent.map(task => {
          const result = mergeResult(task.metrics);
          return <Link className="overview-merged-row" key={task.id} to={`/tasks/${encodeURIComponent(task.id)}`}><span>{task.id}</span><strong>{task.title}</strong><Pill color={result.color} filled={result.filled}>{result.text}</Pill><small>{task.completed_at && <time dateTime={task.completed_at} title={task.completed_at}>{formatRelative(task.completed_at, now)}</time>} · {task.cost_usd == null ? '无计费' : `$${task.cost_usd.toFixed(2)}`}</small></Link>;
        }) : <p>{OVERVIEW.emptyMerged}</p>}</div>
        {overview?.allDoneHref && <Link className="btn btn-text btn-sm" to={overview.allDoneHref}>全部 {overview.doneCount} 个 →</Link>}
      </Card>
    </aside></div>
  </div>;
}
export const handle = { crumb: '总览' };
