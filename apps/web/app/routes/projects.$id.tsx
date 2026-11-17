import { Ribbon } from '../components/ui/ribbon.tsx';
import { isQueued, moveTask } from '../components/queue-deps.ts';
import { TEST_IDS } from '../lib/testids.ts';
import { useClock } from '../lib/clock.tsx';
import { ProjectAlerts } from '../components/project-alerts.tsx';
import { projectAlerts, type AlertDashboard } from '../components/alerts-data.ts';
import { ADMIN } from '../lib/admin-labels.ts';
import { useState } from 'react';
import { Link, useActionData, useLoaderData, useLocation, data as responseData, type ActionFunctionArgs, type LoaderFunctionArgs } from 'react-router';
import { boardColumns, boardHeading, DONE_GROUPS, doneGroup, doneRow, doneTotals, filterBoard, loadBoard, type KindFilter } from '../components/board-data.ts';
import { newTaskHref, settingsHref, taskHref } from '../components/nav-links.ts';
import { useNotificationRevalidate } from '../components/board-live.ts';
import { TaskCard } from '../components/task-card.tsx';
import { servicesContext } from '../load-context.ts';
import { KIND_FILTER_LABELS, KIND_LABELS, STATE_LABELS, BOARD_QUEUE_HINT, BOARD_TOTAL_HINT, BOARD_IMPL } from '../lib/labels.ts';
import { byKind, liveShouldRevalidate } from '../lib/live-routes.ts';

export async function loader({ context, params, request }: LoaderFunctionArgs) {
  const services = context.get(servicesContext), id = params.id ?? '';
  const [board, dashboard] = await Promise.all([
    loadBoard(services, id, new URL(request.url).searchParams.get('done') === 'all', new URL(request.url).searchParams.get('cancelled') === 'all'),
    services.gateway?.read({ method: 'dashboard.get', params: {} }),
  ]);
  return { ...board, alerts: dashboard?.body.ok ? projectAlerts(dashboard.body.payload as AlertDashboard, id) : [],
    alertsError: dashboard?.body.ok ? undefined : dashboard?.body.error ?? ADMIN.offline };
}
export async function action({ request, context }: ActionFunctionArgs) {
  const form = await request.formData();
  const intent = form.get('intent');
  if (intent === 'move') return moveTask(form, context.get(servicesContext));
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
  if ('error' in data) return <p data-testid={TEST_IDS.pageBoard} role="alert">{data.error}</p>;
  const columns = filterBoard(boardColumns(data.tasks, now, timeZone), kind);
  const totals = doneTotals(data.tasks);
  const query = new URLSearchParams(location.search);
  if (data.showAll) query.delete('done'); else query.set('done', 'all');
  const foldHref = `${location.pathname}${query.size ? `?${query}` : ''}`;
  const cancelledQuery = new URLSearchParams(location.search);
  if (data.showCancelled) cancelledQuery.delete('cancelled'); else cancelledQuery.set('cancelled', 'all');
  const cancelledHref = location.pathname + (cancelledQuery.size ? '?' + cancelledQuery : '');
  const cancelledCount = data.cancelledCounts?.[kind] ?? 0;
  return <div className="board" data-testid={TEST_IDS.pageBoard}><header className="board-header"><h1>{data.project.display_name}</h1>
    <div className="board-filters" role="group" aria-label="任务类型">{(Object.keys(KIND_FILTER_LABELS) as KindFilter[]).map(value => <button key={value} aria-pressed={kind === value} onClick={() => setKind(value)}>{KIND_FILTER_LABELS[value]}</button>)}</div>
    <Link to={settingsHref(data.project.id)}>{ADMIN.settings}</Link>
    <Link className="board-new" to={newTaskHref(data.project.id)}>新建任务卡</Link>
  </header>{result && 'error' in result && <p role="alert">{result.error}</p>}
  {data.configError && <section role="alert"><p>项目配置有误，以下是内核的校验原文</p><pre className="board-config-error" data-scroll-region>{data.configError}</pre></section>}<ProjectAlerts alerts={data.alerts} error={data.alertsError} /><div className="board-columns">{columns.map(column => <section className="board-column" data-testid={TEST_IDS.boardColumn} data-column={column.id} key={column.id} aria-labelledby={`board-${column.id}`}>
    <Ribbon small color={column.id === 'todo' ? 'brown' : column.id === 'needs_you' ? 'yellow' : column.id === 'done' ? 'green' : 'teal'}><h2 id={`board-${column.id}`}>{boardHeading(column, kind)}</h2></Ribbon>{column.id === 'todo' && <p className="board-hint">{BOARD_QUEUE_HINT}</p>}{column.id === 'done' && <p className="board-hint" title={BOARD_TOTAL_HINT}>累计 {totals.cost} · 平均{BOARD_IMPL} {totals.average} 轮</p>}<div className="board-column-body" data-scroll-region>
      {column.id === 'done' ? DONE_GROUPS.filter(group => data.showAll || group !== '更早').map(group => {
        const rows = column.tasks.filter(task => doneGroup(task.completed_at, now, timeZone) === group).map(doneRow);
        return rows.length > 0 && <section className="board-done-group" key={group}><h3>{group} · {rows.length} 个</h3>{rows.map(row => <Link className="board-done-row" data-testid={TEST_IDS.taskCard} data-task-id={row.id} key={row.id} to={taskHref(row.id)}><span>{row.id}</span><strong>{row.title}</strong><small>{BOARD_IMPL} {row.rounds} 轮 · 退回 {row.reopens} · {row.cost}</small></Link>)}</section>;
      }) : column.tasks.map(task => <TaskCard key={task.id} task={task} queue={data.tasks.filter(isQueued)} detail={data.details?.[task.id]} overviewDetail={data.overviewDetails?.[task.id]} context={{ fixRounds: 0, cooldowns: [], unread: {}, ...data.cardContext, now, timeZone }} />)}
      {column.tasks.length === 0 && <p>暂无任务</p>}
      {column.id === 'done' && (data.showAll || column.older.length > 0) && <Link className="board-fold" to={foldHref}>{data.showAll ? '收起' : `更早 · ${column.older.length} 个 →`}</Link>}
      {column.id === 'done' && cancelledCount > 0 && <>
        <Link className="board-fold" data-testid={TEST_IDS.boardCancelledToggle} to={cancelledHref}>{data.showCancelled ? '收起' + STATE_LABELS.cancelled : STATE_LABELS.cancelled + '（' + cancelledCount + '）'}</Link>
        {data.showCancelled && <div data-testid={TEST_IDS.boardCancelled}>{data.cancelled?.filter(task => kind === 'all' || task.kind === kind).map(task => <Link key={task.id} className="overview-card task-card" data-testid={TEST_IDS.taskCard} data-task-id={task.id} to={taskHref(task.id)}>
          <div className="overview-card-meta"><span>{KIND_LABELS[task.kind as keyof typeof KIND_LABELS] ?? task.kind}</span><span>{task.id}</span></div><h3>{task.title}</h3><p>{STATE_LABELS.cancelled}</p>
        </Link>)}</div>}
      </>}
    </div></section>)}</div></div>;
}

export const handle = { crumb: '项目看板' };
