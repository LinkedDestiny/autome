import { changesView, changeLabel, CHANGES_COPY, type ChangedFile, type TaskChanges } from './task-changes-model.ts';
import type { TaskPayload } from './task-detail-data.ts';
export function TaskChangesPanel({ data, changes }: { data: TaskPayload; changes?: { payload: unknown } | { error?: string } }) {
  const view = changesView(changes && 'payload' in changes ? changes.payload as TaskChanges : undefined, data);
  const fileList = (files: ChangedFile[]) => <ul>{files.map((file, index) => <li key={`${index}:${file.path}`}><span>{changeLabel(file)}</span> <code>{file.status === 'renamed' ? `${file.from} → ${file.path}` : file.path}</code> +{file.added} −{file.deleted}</li>)}</ul>;
  return <section className="task-changes" aria-label={CHANGES_COPY.title}>
    {view.merged && <p>{CHANGES_COPY.merged}</p>}
    <ol className="task-merge-progress">{view.progress.map(step => <li key={step.label} aria-current={step.current ? 'step' : undefined} className={step.complete ? 'is-complete' : undefined}>{step.label}</li>)}</ol>
    {view.failure && <p role="alert">{view.failure}</p>}
    {changes && 'error' in changes && <p role="alert">{changes.error}</p>}
    {view.available ? <>
      <p>{view.commits} 个提交 · {view.files.length} 个文件 · +{view.added} −{view.deleted} 行</p>
      <p className={view.warning ? 'task-delete-warning' : undefined} role={view.warning ? 'alert' : undefined}>删除文件数：{view.deletedCount}</p>
      {view.rebase && <p>{view.rebase}</p>}{view.included && <p>{view.included}</p>}
      {view.blocked.length > 0 && <div role="alert"><p>{CHANGES_COPY.blocked}</p><ul>{view.blocked.map(path => <li key={path}>{path}</li>)}</ul></div>}
      {view.groups.length ? view.groups.map((repo, index) => <section key={repo.name ?? index} aria-label={repo.name}><h3>{repo.name}</h3><p>删除文件数：{repo.deleted_count ?? 0}</p>{fileList(repo.files ?? [])}</section>) : fileList(view.files)}
      <details><summary>提交标题</summary><ul>{view.subjects.map((subject, index) => <li key={index}>{subject}</li>)}</ul></details>
    </> : <p>当前没有可读取的分支改动</p>}
  </section>;
}
