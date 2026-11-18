import { useState } from 'react';
import { useFetcher } from 'react-router';
import type { BoardTask } from './board-data.ts';
import { useWriteGate } from '../lib/live.tsx';

export function QueueControls({ task, queue }: { task: BoardTask; queue: BoardTask[] }) {
  const peers = queue.filter(row => row.id !== task.id);
  const [reference, setReference] = useState('');
  const fetcher = useFetcher<{ error?: string }>();
  const gate = useWriteGate();
  const selected = peers.some(row => row.id === reference) ? reference : peers[0]?.id;
  return <details className="queue-controls"><summary>调整顺序</summary><fetcher.Form method="post">
    <input type="hidden" name="intent" value="move" /><input type="hidden" name="task_id" value={task.id} />
    <fieldset disabled={!gate.allowed || fetcher.state !== 'idle'}>
      <button className="btn btn-sm" name="top" value="yes">移到最前</button>
      <label>参照任务<select value={selected} onChange={event => setReference(event.target.value)}>{peers.map(row => <option key={row.id} value={row.id}>{row.id} · {row.title}</option>)}</select></label>
      <button className="btn btn-sm" name="before" value={selected}>移到它之前</button><button className="btn btn-sm" name="after" value={selected}>移到它之后</button>
    </fieldset>{!gate.allowed && <p>{gate.reason}</p>}{fetcher.data?.error && <p role="alert">{fetcher.data.error}</p>}
  </fetcher.Form></details>;
}
