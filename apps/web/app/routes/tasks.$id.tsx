import { useLoaderData, type LoaderFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { bySubjectParam, liveShouldRevalidate } from '../lib/live-routes.ts';
import { SessionLog, type LogSession } from '../components/session-log.tsx';

type TaskData = { task: { id: string; title: string }; sessions: LogSession[] };
export async function loader({ context, params }: LoaderFunctionArgs): Promise<{ data?: TaskData; error?: string }> {
  const { gateway } = context.get(servicesContext);
  const offline = '内核离线，暂时拿不到任务';
  if (!gateway) return { error: offline };
  const result = await gateway.read({ method: 'task.get', params: { task_id: params.id } });
  if (!result.body.ok) return { error: result.status === 503 || result.status === 504 ? offline
    : result.status === 404 || result.body.code === 'not_found' ? `找不到任务 ${params.id}` : `读取任务失败：${result.body.error}` };
  return { data: result.body.payload as TaskData };
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
