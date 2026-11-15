import type { TaskPayload } from './task-detail-data.ts';
export function docsView(data: TaskPayload) {
  const documents = data.documents ?? [];
  const tab = (doc: NonNullable<TaskPayload['documents']>[number]) => ({ name: doc.name, label: doc.label === '证据' ? doc.name : doc.label ?? doc.name });
  return {
    tabs: [{ name: null, label: '任务卡' }, ...documents.filter(doc => doc.label !== '证据' && doc.label !== '任务卡').map(tab)],
    evidence: documents.filter(doc => doc.label === '证据').map(tab),
    note: !documents.length && data.task.state?.state === 'cancelled' ? '任务已取消，文档没有合并到默认分支。' : null,
    source: documents.some(doc => doc.source === 'default_branch') ? '来自默认分支（主检出）' : null,
    designName: data.task.slug ? `${data.task.slug}.md` : null,
  };
}
