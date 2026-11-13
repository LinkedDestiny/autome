import { Link } from 'react-router';
import type { loadTaskDetail, TaskPayload } from './task-detail-data.ts';
import { docsView } from './task-docs-model.ts';
import { MarkdownView } from './markdown-view.tsx';
export function TaskDocs({ data, document }: { data: TaskPayload; document: Awaited<ReturnType<typeof loadTaskDetail>>['document'] }) {
  const view = docsView(data);
  const link = (tab: (typeof view.tabs)[number]) => <Link key={tab.name ?? 'brief'} to={tab.href} preventScrollReset aria-current={document?.name === tab.name ? 'page' : undefined}>{tab.label}</Link>;
  return <section className="task-docs" aria-label="任务文档"><h2>文档</h2>
    <nav aria-label="文档选择">{view.tabs.map(link)}{view.evidence.length > 0 && <details open={view.evidence.some(tab => tab.name === document?.name)}><summary>证据（{view.evidence.length}）</summary>{view.evidence.map(link)}</details>}</nav>
    {view.note && <p>{view.note}</p>}
    <article className="task-docs-body">
      {document?.error ? <p role="alert">{document.error}</p> : <>
        {document?.missingBrief && <p>这个任务没有任务卡正文</p>}
        {document?.truncated && <p role="status">文档共 {Math.ceil((document.size ?? 0) / 1024)} KB，只显示前 200 KB</p>}
        <MarkdownView nodes={document?.nodes ?? []} currentMilestone={document?.name === view.designName && view.designName !== null ? data.status_block?.current_milestone : null} />
      </>}
    </article>
  </section>;
}
