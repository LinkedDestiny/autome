import { useClock } from '../lib/clock.tsx';
import { ProjectAlerts } from '../components/project-alerts.tsx';
import { projectAlerts, type AlertDashboard } from '../components/alerts-data.ts';
import { ADMIN } from '../lib/admin-labels.ts';
import { useState } from 'react';
import { Link, useActionData, useLoaderData, useLocation, data as responseData, type ActionFunctionArgs, type LoaderFunctionArgs } from 'react-router';
import { boardColumns, boardHeading, filterBoard, loadBoard, type KindFilter } from '../components/board-data.ts';
import { newTaskHref, settingsHref } from '../components/nav-links.ts';
import { useNotificationRevalidate } from '../components/board-live.ts';
import { TaskCard } from '../components/task-card.tsx';
import { servicesContext } from '../load-context.ts';
import { KIND_FILTER_LABELS, doneFoldLabel } from '../lib/labels.ts';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';

export async function loader({ context, params, request }: LoaderFunctionArgs) {
  const services = context.get(servicesContext), id = params.id ?? '';
  const [board, dashboard] = await Promise.all([
    loadBoard(services, id, new URL(request.url).searchParams.get('done') === 'all'),
    services.gateway?.read({ method: 'dashboard.get', params: {} }),
  ]);
  return { ...board, alerts: dashboard?.body.ok ? projectAlerts(dashboard.body.payload as AlertDashboard, id) : [],
    alertsError: dashboard?.body.ok ? undefined : dashboard?.body.error ?? ADMIN.offline };
}
export async function action({ request, context }: ActionFunctionArgs) {
  const form = await request.formData();
  const intent = form.get('intent');
  if (intent !== 'start' && intent !== 'dismiss') return responseData({ error: '未知操作' }, { status: 400 });
  const { gateway } = context.get(servicesContext);
  if (!gateway) return { error: '内核未连接：服务未接入网关' };
  const result = await gateway.write(intent === 'dismiss'
    ? { op: 'project.alert.dismiss', params: { alert_id: form.get('alert_id') } }
    : { op: 'task.start', params: { task_id: form.get('task_id') } });
  return result.body;
}
export const shouldRevalidate = liveShouldRevalidate(byKind('task.', 'project.', 'bug.', 'merge.', 'gate.', 'session.', 'runtime.'));
export default function ProjectBoard() {
  useNotificationRevalidate();
  const data = useLoaderData<typeof loader>();
  const result = useActionData<typeof action>();
  const [kind, setKind] = useState<KindFilter>('all');
  const location = useLocation();
  const { now, timeZone } = useClock();
  if ('error' in data) return <p role="alert">{data.error}</p>;
  const columns = filterBoard(boardColumns(data.tasks, now), kind);
  const query = new URLSearchParams(location.search);
  if (data.showAll) query.delete('done'); else query.set('done', 'all');
  const foldHref = `${location.pathname}${query.size ? `?${query}` : ''}`;
  return <div className="board"><header className="board-header"><h1>{data.project.display_name}</h1>
    <div className="board-filters" role="group" aria-label="任务类型">{(Object.keys(KIND_FILTER_LABELS) as KindFilter[]).map(value => <button key={value} aria-pressed={kind === value} onClick={() => setKind(value)}>{KIND_FILTER_LABELS[value]}</button>)}</div>
    <Link to={settingsHref(data.project.id)}>{ADMIN.settings}</Link>
    <Link className="board-new" to={newTaskHref(data.project.id)}>新建任务卡</Link>
  </header>{result && 'error' in result && <p role="alert">{result.error}</p>}
  <ProjectAlerts alerts={data.alerts} error={data.alertsError} /><div className="board-columns">{columns.map(column => <section className="board-column" key={column.id} aria-labelledby={`board-${column.id}`}>
    <h2 id={`board-${column.id}`}>{boardHeading(column, kind)}</h2><div className="board-column-body">
      {(data.showAll ? column.tasks : column.recent).map(task => <TaskCard key={task.id} task={task} detail={data.details?.[task.id]} context={{ fixRounds: 0, cooldowns: [], unread: {}, ...data.cardContext, now, timeZone }} />)}
      {column.tasks.length === 0 && <p>暂无任务</p>}
      {column.id === 'done' && (data.showAll || column.older.length > 0) && <Link className="board-fold" to={foldHref}>{doneFoldLabel(data.showAll, column.tasks.length)}</Link>}
    </div></section>)}</div></div>;
}
