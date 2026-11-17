import { TEST_IDS } from '../lib/testids.ts';
import { environmentNotice } from '../components/env-model.ts';
import { ENV } from '../lib/admin-labels.ts';
import { Link, useRouteLoaderData } from 'react-router';
import type { loader } from '../root.tsx';
import { COLUMN_LABELS } from '../lib/labels.ts';
import { overviewTasks } from '../components/shell-data.ts';
import { OverviewCard } from '../components/overview-card.tsx';
export default function Overview() {
  const { kernel, projects, tasks, environment } = useRouteLoaderData<typeof loader>('root')!;
  const notice = environmentNotice(environment);
  return <div className="overview" data-testid={TEST_IDS.pageOverview}><h1>总览</h1>{notice && <p role="status">{notice} <Link to="/env">{ENV.go}</Link></p>}<div className="overview-columns">{(['needs_you', 'running'] as const).map(column => {
    const cards = overviewTasks(tasks, column);
    return <section className="overview-column" key={column} aria-labelledby={`heading-${column}`}>
      <h2 id={`heading-${column}`}>{`${COLUMN_LABELS[column]}（${cards.length}）`}</h2>
      <div className="overview-column-body">{!kernel.ok ? <p>内核离线，暂时拿不到任务</p> : cards.length === 0 ? <p>{column === 'needs_you' ? '没有需要你处理的任务' : '没有正在进行的任务'}</p> : cards.map(task => <OverviewCard key={task.id} task={task} configError={projects.find(project => project.id === task.project_id)?.configError} projectName={projects.find(project => project.id === task.project_id)?.name ?? task.project_id} />)}</div>
    </section>;
  })}</div></div>;
}

export const handle = { crumb: '总览' };
