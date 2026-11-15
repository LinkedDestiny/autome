import { Form, useNavigation, useRouteLoaderData } from 'react-router';
import { dependencyCandidates, dependencyReadOnly } from './queue-deps.ts';
import type { ShellData, ShellTask } from './shell-data.ts';
import type { TaskPayload } from './task-detail-data.ts';
import { useWriteGate } from '../lib/live.tsx';
import { nodeLabel, taskStatusLine } from '../lib/labels.ts';

export function DependencyChoices({ tasks, selected = [], disabled = false }: { tasks: ShellTask[]; selected?: string[]; disabled?: boolean }) {
  return <fieldset className="dependency-choices" disabled={disabled}><legend>{`在这些任务${nodeLabel('merging')}之后开始（可选）`}</legend>
    <div className="dependency-choice-list" data-scroll-region>{tasks.map(task => <label key={task.id}><input type="checkbox" name="after" value={task.id} defaultChecked={selected.includes(task.id)} />{task.id} · {task.title}</label>)}</div>
    <small>最多选择 8 个任务</small>
  </fieldset>;
}
export function TaskDependencies({ data, result, reason = dependencyReadOnly(data) }: { reason?: string | null; data: TaskPayload; result?: { error?: string } }) {
  const shell = useRouteLoaderData<ShellData>('root');
  const gate = useWriteGate();
  const busy = useNavigation().state !== 'idle';
  const after = data.task.after ?? [];
  const tasks = shell?.tasks ?? [];
  return <section aria-label="依赖"><h2>依赖</h2>
    {after.length ? <ul>{after.map(id => { const state = tasks.find(t => t.id === id)?.state ?? data.task.blocked_by?.find(t => t.id === id)?.state; return <li key={id}>{id} · {state ? taskStatusLine(state) : '状态未知'}</li>; })}</ul> : <p>无依赖</p>}
    {reason ? <p>{reason}</p> : <Form method="post" key={after.join(',')}>
      <input type="hidden" name="intent" value="set_after" />
      <DependencyChoices tasks={dependencyCandidates(tasks, data.project?.id ?? '', data.task.id)} selected={after} disabled={!gate.allowed || busy} />
      <button disabled={!gate.allowed || busy}>保存依赖</button><button name="clear" value="yes" disabled={!gate.allowed || busy}>清空依赖</button>
      {!gate.allowed && <p>{gate.reason}</p>}{result?.error && <p role="alert">{result.error}</p>}
    </Form>}
  </section>;
}
