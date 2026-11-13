import { TaskHeader } from '../components/task-header.tsx';
import { headerView } from '../components/task-header-model.ts';
import { TaskStage } from '../components/task-stage.tsx';
import { stageBarView, type TaskConfig } from '../components/task-stage-model.ts';
import { TaskNotice } from '../components/task-notice.tsx';
import { noticeView } from '../components/task-notice-model.ts';
import type { CardContext } from '../components/task-card-model.ts';
import { loadTaskDetail } from '../components/task-detail-data.ts';
import { useLoaderData, type LoaderFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { bySubjectParam, liveShouldRevalidate } from '../lib/live-routes.ts';
import { TaskTimeline } from '../components/task-timeline.tsx';

export async function loader({ context, params, request }: LoaderFunctionArgs) {
  return loadTaskDetail(context.get(servicesContext), params.id!, new URL(request.url));
}
export const shouldRevalidate = liveShouldRevalidate(bySubjectParam(['task.', 'session.'], 'id'));
export default function TaskPage() {
  const { data, error, config, history, dashboard } = useLoaderData<typeof loader>();
  const settings = config && 'payload' in config ? config.payload as TaskConfig : {};
  const context: CardContext = { now: new Date().toISOString(), fixRounds: data?.task.budget_n ?? settings.resolved?.bug?.fix_rounds ?? 0,
    cooldowns: dashboard && 'payload' in dashboard ? (dashboard.payload as { cooldowns?: CardContext['cooldowns'] }).cooldowns ?? [] : [], unread: {} };
  return <div className="task-page">{data ? <>
    <TaskHeader view={headerView(data, context)} />
    <TaskStage view={stageBarView(data, history?.events ?? [], settings)} />
    <TaskNotice view={noticeView(data, context)} />
    <TaskTimeline sessions={data.sessions} events={history?.events ?? []} now={context.now} truncated={history?.truncated} error={history && 'error' in history ? history.error : undefined} />
  </> : <p>{error}</p>}</div>;
}
