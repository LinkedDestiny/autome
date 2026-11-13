import { Link } from 'react-router';
import { CARD_LABELS } from '../lib/labels.ts';
import type { BoardTask } from './board-data.ts';
import { taskHref } from './nav-links.ts';
import { cardView, type CardContext, type TaskDetail } from './task-card-model.ts';
export function TaskCard({ task, detail, context }: { task: BoardTask; detail?: TaskDetail; context: CardContext }) {
  const view = cardView(task, detail, context);
  return <article className="overview-card task-card">
    <div className="overview-card-meta"><span>{view.kind}</span><span>{task.id}</span></div>
    <h3><Link to={taskHref(task.id)}>{task.title}</Link></h3>
    {view.badge && <span className="task-card-badge">{view.badge}</span>}
    <ol className="task-card-stages" aria-label={CARD_LABELS.progress}>{view.stages.map(stage => <li key={stage.label} aria-current={stage.current ? 'step' : undefined} data-passed={stage.passed}>{stage.label}</li>)}</ol>
    {view.unknownStage && <p>{view.unknownStage}</p>}{view.progress && <p>{view.progress}</p>}
    <p className="task-status">{view.activity}</p><p className="task-card-gate">{view.gate}</p>
    {view.merged && <p>{view.merged}</p>}
  </article>;
}
