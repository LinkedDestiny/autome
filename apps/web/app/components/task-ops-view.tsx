import { TEST_IDS } from '../lib/testids.ts';
import { Form, Link, useSearchParams, useNavigation } from 'react-router';
import { useWriteGate } from '../lib/live.tsx';
import { MAX_PROSE_LENGTH } from '../../server/gates/write-gate.ts';
import { NODE_LABELS } from '../lib/labels.ts';
import { hasVisibleOps, OP_LABELS, type OpView } from './task-ops.ts';
export function TaskOps({ ops, result }: { ops: OpView[]; result?: { error?: string; notice?: string } }) {
  const gate = useWriteGate();
  const [search] = useSearchParams();
  const pending = useNavigation().state !== 'idle';
  const confirm = new URLSearchParams(search); confirm.set('confirm', 'cancel');
  const visible = hasVisibleOps(ops);
  if (!visible && !result?.error && !result?.notice) return null;
  return <section className="task-ops" aria-label="任务操作">
    {result?.error && <p role="alert">{result.error}</p>}{result?.notice && <p role="status">{result.notice}</p>}
    {visible && !gate.allowed && <p>{gate.reason}</p>}
    <div className="task-ops-list">{ops.filter(o => o.op !== 'decide').map(entry => {
      const { op } = entry;
      const disabled = !gate.allowed || !entry.enabled || pending;
      const button = <button className={['cancel', 'stop'].includes(op) ? 'btn btn-sm btn-danger' : 'btn btn-sm'} data-testid={TEST_IDS.taskOp} data-op={op} type="submit" disabled={disabled}>{entry.label ?? OP_LABELS[op]}</button>;
      if (op === 'cancel' && search.get('confirm') !== 'cancel') return <div key={op}>{disabled ? <button className="btn btn-sm btn-danger" data-testid={TEST_IDS.taskOp} data-op={op} disabled>{OP_LABELS.cancel}</button> : <Link className="btn btn-sm btn-danger" data-testid={TEST_IDS.taskOp} data-op={op} to={`?${confirm}`}>{OP_LABELS.cancel}</Link>}</div>;
      const inputs = <>
        {op === 'reject' && <label>驳回意见<textarea name="feedback" maxLength={MAX_PROSE_LENGTH} required /></label>}
        {op === 'reply' && <label>补充信息<textarea name="info" maxLength={MAX_PROSE_LENGTH} required /></label>}
        {op === 'rerun' && <label>从哪个节点重跑<select name="node" required>{entry.nodes?.map(node => <option key={node} value={node}>{NODE_LABELS[node as keyof typeof NODE_LABELS] ?? node}</option>)}</select></label>}
        {op === 'extend' && <label>追加轮数<input type="number" name="extra_rounds" min="1" max="4294967295" step="1" defaultValue="1" required /></label>}
        {op === 'convert' && <label><input type="checkbox" name="start" value="yes" />改建后立即开始</label>}
      </>;
      const form = <Form method="post"><input type="hidden" name="intent" value={op} /><fieldset disabled={disabled}>{inputs}
        {op === 'cancel' ? <><input type="hidden" name="confirm" value="yes" /><button className="btn btn-sm btn-danger" data-testid={TEST_IDS.taskOp} data-op={op} type="submit">确认取消这个任务</button><Link className="btn btn-sm" to={`?${new URLSearchParams([...search].filter(([key]) => key !== 'confirm'))}`}>返回</Link></> : button}
      </fieldset></Form>;
      return <div key={op}>{['reject', 'reply', 'rerun', 'extend', 'convert'].includes(op) ? <details><summary className="btn btn-sm">{OP_LABELS[op]}</summary>{form}</details> : form}{entry.reason && <p>{entry.reason}</p>}</div>;
    })}</div>
  </section>;
}
