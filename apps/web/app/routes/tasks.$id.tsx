import { withParams } from '../components/task-url.ts';
import { TaskDependencies } from '../components/task-dependencies.tsx';
import { TaskNotes } from '../components/task-notes.tsx';
import { TEST_IDS } from '../lib/testids.ts';
import { TaskChangesPanel } from '../components/task-changes.tsx';
import { TaskDecisions } from '../components/task-decisions.tsx';
import { DETAIL_TABS, detailTab, TASK_PAGE_EVENT_PREFIXES } from '../components/task-changes-model.ts';
import { TaskOps } from '../components/task-ops-view.tsx';
import { handleTaskAction } from '../components/task-action.ts';
import { TaskDocs } from '../components/task-docs.tsx';
import { TaskHeader } from '../components/task-header.tsx';
import { TaskStage } from '../components/task-stage.tsx';
import { TaskNotice } from '../components/task-notice.tsx';
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
  const { data, error, header, stage, notice, ops, timeline, window, docs, dependencyReason, document, changes } = useLoaderData<typeof loader>();
  const [search] = useSearchParams();
  const selected = data ? detailTab(data, search) : 'docs';
  const tabLink = (id: string) => `?${withParams(search, { set: { tab: id } })}`;
  const result = useActionData<typeof action>();
  return <div className="task-page" data-testid={TEST_IDS.pageTask}>{data ? <>
    <TaskHeader view={header!} />
    <TaskStage view={stage!} />
    <div className="task-controls"><TaskNotice view={notice!} />
    <TaskOps ops={ops!} result={result} /></div>
    <div className="task-detail-body">
    <TaskTimeline entries={timeline!.entries} window={window!} truncated={timeline!.truncated} error={timeline!.error} />
    <div className="task-detail-panel"><nav className="task-detail-tabs" aria-label="任务详情标签">{DETAIL_TABS.map(tab => <Link key={tab.id} to={tabLink(tab.id)} preventScrollReset aria-current={selected === tab.id ? 'page' : undefined}>{tab.label}</Link>)}</nav>
      {selected === 'docs' ? <TaskDocs view={docs} data={data} document={document} /> : selected === 'changes' ? <TaskChangesPanel data={data} changes={changes} /> : selected === 'notes' ? <div className="task-notes-panel" data-scroll-region><TaskNotes data={data} result={result} /><TaskDependencies reason={dependencyReason} data={data} result={result} /></div> : <TaskDecisions decisions={data.decisions} />}
    </div></div>
  </> : <p>{error}</p>}</div>;
}
