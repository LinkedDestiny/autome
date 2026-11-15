import { TaskNotes } from '../components/task-notes.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { useClock } from '../lib/clock.tsx';
import { TaskChangesPanel } from '../components/task-changes.tsx';
import { TaskDecisions } from '../components/task-decisions.tsx';
import { DETAIL_TABS, detailTab, TASK_PAGE_EVENT_PREFIXES } from '../components/task-changes-model.ts';
import { TaskOps } from '../components/task-ops-view.tsx';
import { availableOps, opsInput } from '../components/task-ops.ts';
import { handleTaskAction } from '../components/task-action.ts';
import { TaskDocs } from '../components/task-docs.tsx';
import { TaskHeader } from '../components/task-header.tsx';
import { headerView } from '../components/task-header-model.ts';
import { TaskStage } from '../components/task-stage.tsx';
import { stageBarView, type TaskConfig } from '../components/task-stage-model.ts';
import { TaskNotice } from '../components/task-notice.tsx';
import { noticeView } from '../components/task-notice-model.ts';
import type { CardContext } from '../components/task-card-model.ts';
import { loadTaskDetail } from '../components/task-detail-data.ts';
import { Link, useSearchParams, useLoaderData, useActionData, type ActionFunctionArgs, type LoaderFunctionArgs } from 'react-router';
import { servicesContext } from '../load-context.ts';
import { bySubjectParam, liveShouldRevalidate } from '../lib/live-routes.ts';
import { TaskTimeline } from '../components/task-timeline.tsx';

export async function loader({ context, params, request }: LoaderFunctionArgs) {
  return loadTaskDetail(context.get(servicesContext), params.id!, new URL(request.url));
}
export async function action({ context, params, request }: ActionFunctionArgs) {
  return handleTaskAction(await request.formData(), params.id!, context.get(servicesContext));
}
export const shouldRevalidate = liveShouldRevalidate(bySubjectParam(TASK_PAGE_EVENT_PREFIXES, 'id'));
export default function TaskPage() {
  const { data, error, config, history, dashboard, document, changes } = useLoaderData<typeof loader>();
  const [search] = useSearchParams();
  const selected = data ? detailTab(data, search) : 'docs';
  const tabLink = (id: string) => { const next = new URLSearchParams(search); next.set('tab', id); return `?${next}`; };
  const result = useActionData<typeof action>();
  const settings = config && 'payload' in config ? config.payload as TaskConfig : {};
  const clock = useClock();
  const context: CardContext = { ...clock, fixRounds: data?.task.budget_n ?? settings.resolved?.bug?.fix_rounds ?? 0,
    cooldowns: dashboard && 'payload' in dashboard ? (dashboard.payload as { cooldowns?: CardContext['cooldowns'] }).cooldowns ?? [] : [], unread: {} };
  return <div className="task-page" data-testid={TEST_IDS.pageTask}>{data ? <>
    <TaskHeader view={headerView(data, context)} />
    <TaskStage view={stageBarView(data, history?.events ?? [], settings)} />
    <div className="task-controls"><TaskNotice view={noticeView(data, context)} />
    <TaskOps ops={availableOps(opsInput(data, settings))} result={result} /></div>
    <div className="task-detail-body">
    <TaskTimeline sessions={data.sessions} events={history?.events ?? []} now={context.now} truncated={history?.truncated} error={history && 'error' in history ? history.error : undefined} />
    <div className="task-detail-panel"><nav className="task-detail-tabs" aria-label="任务详情标签">{DETAIL_TABS.map(tab => <Link key={tab.id} to={tabLink(tab.id)} preventScrollReset aria-current={selected === tab.id ? 'page' : undefined}>{tab.label}</Link>)}</nav>
      {selected === 'docs' ? <TaskDocs data={data} document={document} /> : selected === 'changes' ? <TaskChangesPanel data={data} changes={changes} /> : selected === 'notes' ? <TaskNotes data={data} result={result} /> : <TaskDecisions decisions={data.decisions} />}
    </div></div>
  </> : <p>{error}</p>}</div>;
}
