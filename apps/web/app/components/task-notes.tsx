import { Form, useNavigation } from 'react-router';
import { useWriteGate } from '../lib/live.tsx';
import { formatClock, useClock } from '../lib/clock.tsx';
import { nodeLabel, roleLabel } from '../lib/labels.ts';
import { MAX_PROSE_LENGTH } from '../../server/gates/write-gate.ts';
import { MarkdownView } from './markdown-view.tsx';
import { parseMarkdown } from '../lib/markdown.ts';
import type { TaskPayload } from './task-detail-data.ts';

export function TaskNotes({ data, result }: { data: TaskPayload; result?: { error?: string; warning?: string } }) {
  const gate = useWriteGate();
  const pending = useNavigation().state !== 'idle';
  const clock = useClock();
  const state = data.task.state;
  const ended = state?.state === 'done' || state?.state === 'cancelled';
  return <section className="task-notes" aria-label="备注">
    {(data.notes ?? []).map(note => <article key={note.n}>
      <div>#{note.n} · {formatClock(note.at, clock)}</div><div className="task-note-text"><MarkdownView nodes={parseMarkdown(note.text)} /></div>
      <div>{note.delivered ? `已送达 · ${roleLabel(note.delivered.role)} 第 ${note.delivered.round} 轮 · ${formatClock(note.delivered.at, clock)}` : '等待下一轮'}</div>
    </article>)}
    {ended ? <p>任务已结束，备注不会再被读到</p> : <Form method="post">
      <input type="hidden" name="intent" value="note" />
      <fieldset disabled={!gate.allowed || pending}>
        <label>备注正文<textarea name="text" maxLength={MAX_PROSE_LENGTH} required /></label>
        {state?.state === 'active' && state.node === 'await_merge' && <label><input type="checkbox" name="reopen" value="yes" />{`同时送回${nodeLabel('implement')}轮处理（不${nodeLabel('merging')}）`}</label>}
        <button type="submit">提交备注</button>
      </fieldset>
      {!gate.allowed && <p>{gate.reason}</p>}
      {result?.error && <p role="alert">{result.error}</p>}
      {result?.warning && <p role="status">{result.warning}</p>}
    </Form>}
  </section>;
}
