import { overviewCardView } from '../lib/overview-labels.ts';
import type { OverviewDetail } from '../../server/overview.ts';
import { formatClock } from '../lib/clock.tsx';
import { Progress } from './ui/progress.tsx';
import { Pill } from './ui/pill.tsx';
import { QueueControls } from './queue-controls.tsx';
import { isQueued } from './queue-deps.ts';
import { TEST_IDS } from '../lib/testids.ts';
import { Link, useFetcher } from 'react-router';
import { useWriteGate } from '../lib/live.tsx';
import { columnOf } from './task-columns.ts';
import { CARD_LABELS } from '../lib/labels.ts';
import type { BoardTask } from './board-data.ts';
import { taskHref } from './nav-links.ts';
import { cardView, type CardContext, type TaskDetail } from './task-card-model.ts';
export function TaskCard({ task, detail, context, overviewDetail, queue = [] }: { task: BoardTask; detail?: TaskDetail; overviewDetail?: OverviewDetail; context: CardContext; queue?: BoardTask[] }) {
  const view = cardView(task, detail, context);
  const shared = overviewCardView(task, overviewDetail, context.now), column = columnOf(task.state);
  return <article className={`overview-card task-card${column === 'running' ? ' card card-teal' : column === 'needs_you' ? ' card card-yellow' : ''}`} data-testid={TEST_IDS.taskCard} data-task-id={task.id}>
    <div className="overview-card-meta"><Pill color={column === 'needs_you' ? 'yellow' : column === 'running' ? 'teal' : 'brown'}>{view.kind}</Pill><span>{task.id}</span></div>
    <h3><Link to={taskHref(task.id)}>{task.title}</Link></h3>
    {view.badge && <Pill color="yellow" filled>{shared.badge}</Pill>}
    {column === 'running' && overviewDetail?.status_block && <Progress value={overviewDetail.status_block.milestones_total ? overviewDetail.status_block.milestones_done / overviewDetail.status_block.milestones_total * 100 : 0} label={CARD_LABELS.progress} />}
    {view.unknownStage && <p>{view.unknownStage}</p>}{view.progress && <p>{view.progress}</p>}
    <p className="task-status">{view.activity}</p>{column !== 'todo' && view.gate && <p className="task-card-gate">{view.gate}</p>}
    {column === 'todo' && <p>{shared.queue || CARD_LABELS.waitStart} · 创建 {task.created_at ? formatClock(task.created_at, { now: context.now, timeZone: context.timeZone ?? 'UTC' }) : '—'}</p>}
    {(column === 'running' || column === 'needs_you') && <><p>{shared.summary}</p><p>已花费 {shared.cost} · 已用时 {shared.elapsed}</p>{shared.sessionTime && <p>{shared.sessionTime}</p>}</>}
    {view.dependency && <p className={view.dependencyWarning ? 'dependency-warning' : undefined}>{view.dependency}</p>}
    {isQueued(task) && queue.length > 1 && <QueueControls task={task} queue={queue} />}
    {column === 'needs_you' && <Link className="btn btn-primary btn-sm" to={shared.href}>{shared.action}</Link>}
    {view.merged && <p>{view.merged}</p>}
    {task.state.state === 'draft' && <StartTask taskId={task.id} />}
  </article>;
}

function StartTask({ taskId }: { taskId: string }) {
  const fetcher = useFetcher<{ error?: string }>();
  const gate = useWriteGate();
  return <fetcher.Form method="post">
    <input type="hidden" name="intent" value="start" />
    <input type="hidden" name="task_id" value={taskId} />
    <button data-testid={TEST_IDS.taskOp} data-op="start" className="btn btn-sm btn-primary" disabled={!gate.allowed || fetcher.state !== 'idle'}>开始</button>
    {!gate.allowed && <p role="status">{gate.reason}</p>}
    {fetcher.data?.error && <p role="alert">{fetcher.data.error}</p>}
  </fetcher.Form>;
}
