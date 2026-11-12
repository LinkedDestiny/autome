import { useRouteLoaderData } from 'react-router';
import type { loader } from '../root.tsx';
import { COLUMN_LABELS } from '../lib/labels.ts';
import { overviewTasks } from '../components/shell-data.ts';
import { OverviewCard } from '../components/overview-card.tsx';
export default function Overview() {
  const { kernel, projects, tasks } = useRouteLoaderData<typeof loader>('root')!;
  return <div className="overview"><h1>总览</h1><div className="overview-columns">{(['needs_you', 'running'] as const).map(column => {
    const cards = overviewTasks(tasks, column);
    return <section className="overview-column" key={column} aria-labelledby={`heading-${column}`}>
      <h2 id={`heading-${column}`}>{`${COLUMN_LABELS[column]}（${cards.length}）`}</h2>
      <div className="overview-column-body">{!kernel.ok ? <p>内核离线，暂时拿不到任务</p> : cards.length === 0 ? <p>{column === 'needs_you' ? '没有需要你处理的任务' : '没有正在进行的任务'}</p> : cards.map(task => <OverviewCard key={task.id} task={task} projectName={projects.find(project => project.id === task.project_id)?.name ?? task.project_id} />)}</div>
    </section>;
  })}</div></div>;
}
