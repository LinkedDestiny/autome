import { Link } from 'react-router';
import { KIND_LABELS, stageOf, taskStatusLine } from '../lib/labels.ts';
import type { ShellTask } from './shell-data.ts';
import { taskHref } from './nav-links.ts';
export function OverviewCard({ task, projectName }: { task: ShellTask; projectName: string }) {
  return <Link className="overview-card" to={taskHref(task.id)}>
    <div className="overview-card-meta"><span>{KIND_LABELS[task.kind as keyof typeof KIND_LABELS] ?? task.kind}</span><span>{task.id}</span></div>
    <h3>{task.title}</h3><p>{projectName}</p><span className="task-stage">{stageOf(task.kind, task.state)?.label}</span>
    <p className="task-status">{taskStatusLine(task.state)}</p>
  </Link>;
}
