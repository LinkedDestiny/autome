import { changesView, CHANGES_COPY, type TaskChanges } from './task-changes-model.ts';
import type { TaskPayload } from './task-detail-data.ts';
export function TaskChangesPanel({ data, changes }: { data: TaskPayload; changes?: { payload: unknown } | { error?: string } }) {
  const view = changesView(changes && 'payload' in changes ? changes.payload as TaskChanges : undefined, data);
  return <section className="task-changes" aria-label={CHANGES_COPY.title}>
    <ol className="task-merge-progress">{view.progress.map(step => <li key={step.label} aria-current={step.current ? 'step' : undefined} className={step.complete ? 'is-complete' : undefined}>{step.label}</li>)}</ol>
    {view.failure && <p role="alert">{view.failure}</p>}
    {changes && 'error' in changes && <p role="alert">{changes.error}</p>}
    {view.available ? <>
      <p>{view.commits} 个提交 · {view.files.length} 个文件 · +{view.added} −{view.deleted} 行</p>
      <p className={view.warning ? 'task-delete-warning' : undefined} role={view.warning ? 'alert' : undefined}>疑似删除的文件：{view.suspected}（只删不增，即新增 0 行、删除大于 0 行；也可能是清空文件）</p>
      {view.rebase && <p>{view.rebase}</p>}{view.included && <p>{view.included}</p>}
      {view.blocked.length > 0 && <div role="alert"><p>{CHANGES_COPY.blocked}</p><ul>{view.blocked.map(path => <li key={path}>{path}</li>)}</ul></div>}
      <ul>{view.files.map((file, index) => <li key={`${index}:${file.path}`}><code>{file.path}</code> +{file.added} −{file.deleted}</li>)}</ul>
      <details><summary>提交标题</summary><ul>{view.subjects.map((subject, index) => <li key={index}>{subject}</li>)}</ul></details>
    </> : <p>当前没有可读取的分支改动</p>}
  </section>;
}
