import { useEffect, useState } from 'react';
import { Link, useLoaderData, useLocation, type LoaderFunctionArgs } from 'react-router';
import { boardColumns, boardHeading, filterBoard, loadBoard, type KindFilter } from '../components/board-data.ts';
import { newTaskHref } from '../components/nav-links.ts';
import { OverviewCard } from '../components/overview-card.tsx';
import { servicesContext } from '../load-context.ts';
import { KIND_FILTER_LABELS, doneFoldLabel } from '../lib/labels.ts';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';

export function loader({ context, params, request }: LoaderFunctionArgs) {
  return loadBoard(context.get(servicesContext), params.id ?? '', new URL(request.url).searchParams.get('done') === 'all');
}
export const shouldRevalidate = liveShouldRevalidate(byKind('task.', 'project.', 'bug.', 'merge.', 'gate.', 'session.', 'runtime.'));
function useNow(initial: string) {
  const [now, setNow] = useState(initial);
  useEffect(() => { const timer = setInterval(() => setNow(new Date().toISOString()), 60_000); return () => clearInterval(timer); }, []);
  return now > initial ? now : initial;
}
export default function ProjectBoard() {
  const data = useLoaderData<typeof loader>();
  const [kind, setKind] = useState<KindFilter>('all');
  const location = useLocation();
  const now = useNow('now' in data ? data.now : '');
  if ('error' in data) return <p role="alert">{data.error}</p>;
  const columns = filterBoard(boardColumns(data.tasks, now), kind);
  const query = new URLSearchParams(location.search);
  if (data.showAll) query.delete('done'); else query.set('done', 'all');
  const foldHref = `${location.pathname}${query.size ? `?${query}` : ''}`;
  return <div className="board"><header className="board-header"><h1>{data.project.display_name}</h1>
    <div className="board-filters" role="group" aria-label="任务类型">{(Object.keys(KIND_FILTER_LABELS) as KindFilter[]).map(value => <button key={value} aria-pressed={kind === value} onClick={() => setKind(value)}>{KIND_FILTER_LABELS[value]}</button>)}</div>
    <Link className="board-new" to={newTaskHref(data.project.id)}>新建任务卡</Link>
  </header><div className="board-columns">{columns.map(column => <section className="board-column" key={column.id} aria-labelledby={`board-${column.id}`}>
    <h2 id={`board-${column.id}`}>{boardHeading(column, kind)}</h2><div className="board-column-body">
      {(data.showAll ? column.tasks : column.recent).map(task => <OverviewCard key={task.id} task={task} projectName={data.project.display_name} />)}
      {column.tasks.length === 0 && <p>暂无任务</p>}
      {column.id === 'done' && (data.showAll || column.older.length > 0) && <Link className="board-fold" to={foldHref}>{doneFoldLabel(data.showAll, column.older.length)}</Link>}
    </div></section>)}</div></div>;
}
