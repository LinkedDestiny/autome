import { loadTaskDetail } from '../components/task-detail-data.ts';
import { useLoaderData, type LoaderFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { bySubjectParam, liveShouldRevalidate } from '../lib/live-routes.ts';
import { SessionLog } from '../components/session-log.tsx';

export async function loader({ context, params, request }: LoaderFunctionArgs) {
  return loadTaskDetail(context.get(servicesContext), params.id!, new URL(request.url));
}
export const shouldRevalidate = liveShouldRevalidate(bySubjectParam(['task.', 'session.'], 'id'));
export default function TaskPage() {
  const { data, error } = useLoaderData<typeof loader>();
  return <div className="task-page">{data ? <>
    <h1>{data.task.title} <small>{data.task.id}</small></h1>
    <p>任务详情页还在建设中，这里先列出各轮会话的日志</p>
    {data.sessions.map(session => <SessionLog key={session.id} session={session} />)}
  </> : <p>{error}</p>}</div>;
}
