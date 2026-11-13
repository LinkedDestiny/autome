import type { TaskPayload } from './task-detail-data.ts';
export function docsView(data: TaskPayload) {
  const documents = data.documents ?? [];
  const tab = (doc: NonNullable<TaskPayload['documents']>[number]) => ({ name: doc.name, label: doc.label === '证据' ? doc.name : doc.label ?? doc.name, href: `?doc=${encodeURIComponent(doc.name)}` });
  return {
    tabs: [{ name: null, label: '任务卡', href: '?' }, ...documents.filter(doc => doc.label !== '证据').map(tab)],
    evidence: documents.filter(doc => doc.label === '证据').map(tab),
    note: documents.length ? null : data.task.state?.state === 'done' ? '任务已完成，工作目录已清理，文档暂不可读。' : data.task.kind === 'bug' ? '内核尚未提供 Bug 文档，当前只显示任务卡。' : null,
    designName: data.task.slug ? `${data.task.slug}.md` : null,
  };
}
