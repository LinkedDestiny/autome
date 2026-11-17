import { TEST_IDS } from '../lib/testids.ts';
import { Link } from 'react-router';
import { KIND_LABELS, stageStatusLine } from '../lib/labels.ts';
import { OVERVIEW, overviewCardView } from '../lib/overview-labels.ts';
import { formatSpan, useClock } from '../lib/clock.tsx';
import { columnOf } from './task-columns.ts';
import type { ShellTask } from './shell-data.ts';
import type { OverviewDetail } from '../../server/overview.ts';
import { Card } from './ui/card.tsx';
import { Pill } from './ui/pill.tsx';
import { Progress } from './ui/progress.tsx';
export function OverviewCard({ task, projectName, configError, detail, waitingSince }: { task: ShellTask; projectName: string; configError?: string; detail?: OverviewDetail; waitingSince?: number }) {
  const { now } = useClock(), waiting = columnOf(task.state) === 'needs_you', running = columnOf(task.state) === 'running';
  const view = overviewCardView(task, detail, now), block = detail?.status_block;
  return <Card className="overview-task-card" color={waiting ? 'yellow' : running ? 'teal' : undefined} patterned={waiting || running} data-testid={TEST_IDS.taskCard} data-task-id={task.id}>
    <div className="overview-card-meta"><Pill filled color={waiting ? 'yellow' : running ? 'teal' : 'brown'}>{view.badge}</Pill><Pill color="brown">{task.id} · {projectName}</Pill><span>{KIND_LABELS[task.kind as keyof typeof KIND_LABELS] ?? task.kind}</span>
      {waitingSince != null && <span>已等 {formatSpan(Date.parse(now) - waitingSince)}</span>}
      {configError && <span className="count-pill count-alert" title={configError}>配置有误</span>}
    </div>
    <h3><Link to={view.href}>{task.title}</Link></h3>
    <span className="task-stage">{stageStatusLine(task.kind, task.state)}</span>
    {view.summary && <p>{view.summary}</p>}
    {view.queue && <p>{view.queue}</p>}
    {(waiting || running) && <div className="overview-facts">
      {waiting && <span>{OVERVIEW.designRound}<b>{view.round}</b></span>}<span>已花费<b>{view.cost}</b></span><span>已用时<b>{view.elapsed}</b></span>
      {detail?.designBytes != null && <span>{OVERVIEW.designDoc}<b>{Math.ceil(detail.designBytes / 1024)} KB</b></span>}
    </div>}
    {running && block && <><Progress value={block.milestones_total ? block.milestones_done / block.milestones_total * 100 : 0} label={OVERVIEW.progress} /><p>{view.progress}</p></>}
    {running && <p>{[view.current, view.sessionTime, view.gate].filter(Boolean).join(' · ')}</p>}
    {waiting && <Link className="btn btn-primary btn-sm" to={view.href}>{view.action}</Link>}
  </Card>;
}
