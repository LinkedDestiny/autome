import { toOverviewDetail } from '../app/lib/overview-model.ts';
import type { WebServices } from '../app/load-context.ts';
import type { ShellData } from '../app/components/shell-data.ts';
import type { TaskPayload } from '../app/components/task-detail-data.ts';
import { columnOf } from '../app/components/task-columns.ts';
import type { Clock } from '../app/lib/clock.tsx';
import { recentMerged, type TodaySummary } from './overview-summary.ts';
export type OverviewDetail = Pick<TaskPayload, 'status_block' | 'gate'> & {
  reviewName?: string; adjudicationName?: string;
  session?: Pick<TaskPayload['sessions'][number], 'label' | 'kind' | 'round' | 'started_at'>;
  reviews: number; adjudications: number; pending: number; designBytes?: number; designName?: string; error?: string;
};
export type OverviewData = { today?: TodaySummary; error?: string; listError?: string; waiting: Record<string, number>; details: Record<string, OverviewDetail>; recent: ShellData['tasks']; allDoneHref?: string; doneCount: number };
export async function loadOverview(services: WebServices, shell: ShellData, clock: Clock): Promise<OverviewData> {
  const data: OverviewData = { waiting: {}, details: {}, recent: recentMerged(shell.tasks), doneCount: shell.tasks.filter(t => t.state.state === 'done').length,
    allDoneHref: shell.projects.length === 1 ? `/projects/${encodeURIComponent(shell.projects[0].id)}?done=all` : undefined };
  if (!shell.kernel.ok || shell.projects.some(p => p.error)) {
    data.error = data.listError = shell.kernel.error ?? shell.projects.find(p => p.error)?.error ?? '内核离线';
    data.allDoneHref = undefined; return data;
  }
  await Promise.all([
    (async () => {
      try {
        if (!services.overviewSummary) throw Error('总览事件服务不可用');
        Object.assign(data, await services.overviewSummary(shell.tasks, clock));
      } catch (error) { data.error = `今天的数据暂不可用：${String(error)}`; }
    })(),
    ...shell.tasks.filter(t => ['needs_you', 'running'].includes(columnOf(t.state) ?? '')).map(async task => {
      try {
        const result = await services.gateway!.read({ method: 'task.get', params: { task_id: task.id } });
        if (!result.body.ok) throw Error(result.body.error ?? '任务读取失败');
        const detail = result.body.payload as TaskPayload;
        data.details[task.id] = toOverviewDetail(detail);
      } catch (error) { data.details[task.id] = { reviews: 0, adjudications: 0, pending: 0, error: `详情暂不可用：${String(error)}` }; }
    }),
  ]);
  return data;
}
