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
export function TaskCard({ task, detail, context, queue = [] }: { task: BoardTask; detail?: TaskDetail; context: CardContext; queue?: BoardTask[] }) {
  const view = cardView(task, detail, context);
  return <article className="overview-card task-card" data-testid={TEST_IDS.taskCard} data-task-id={task.id}>
    <div className="overview-card-meta"><span>{view.kind}</span><span>{task.id}</span></div>
    <h3><Link to={taskHref(task.id)}>{task.title}</Link></h3>
    {view.badge && <span className="task-card-badge">{view.badge}</span>}
    <ol className="task-card-stages" aria-label={CARD_LABELS.progress}>{view.stages.map(stage => <li key={stage.label} aria-current={stage.current ? 'step' : undefined} data-passed={stage.passed}>{stage.label}</li>)}</ol>
    {view.unknownStage && <p>{view.unknownStage}</p>}{view.progress && <p>{view.progress}</p>}
    <p className="task-status">{view.activity}</p>{columnOf(task.state) !== 'todo' && <p className="task-card-gate">{view.gate}</p>}
    {view.dependency && <p className={view.dependencyWarning ? 'dependency-warning' : undefined}>{view.dependency}</p>}
    {isQueued(task) && queue.length > 1 && <QueueControls task={task} queue={queue} />}
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
    <button data-testid={TEST_IDS.taskOp} data-op="start" className="btn-primary" disabled={!gate.allowed || fetcher.state !== 'idle'}>开始</button>
    {!gate.allowed && <p role="status">{gate.reason}</p>}
    {fetcher.data?.error && <p role="alert">{fetcher.data.error}</p>}
  </fetcher.Form>;
}
