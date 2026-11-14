import { TEST_IDS } from '../lib/testids.ts';
import { Link } from 'react-router';
import { KIND_LABELS, stageStatusLine } from '../lib/labels.ts';
import type { ShellTask } from './shell-data.ts';
import { taskHref } from './nav-links.ts';
export function OverviewCard({ task, projectName }: { task: ShellTask; projectName: string }) {
  return <Link className="overview-card" data-testid={TEST_IDS.taskCard} data-task-id={task.id} to={taskHref(task.id)}>
    <div className="overview-card-meta"><span>{KIND_LABELS[task.kind as keyof typeof KIND_LABELS] ?? task.kind}</span><span>{task.id}</span></div>
    <h3>{task.title}</h3><p>{projectName}</p><span className="task-stage">{stageStatusLine(task.kind, task.state)}</span>
  </Link>;
}
