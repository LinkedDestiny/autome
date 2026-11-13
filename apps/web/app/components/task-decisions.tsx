import { Form, useNavigation } from 'react-router';
import { useWriteGate } from '../lib/live.tsx';
import { MAX_PROSE_LENGTH } from '../../server/gates/write-gate.ts';
import type { TaskPayload } from './task-detail-data.ts';
import { DECISION_COPY, DISPOSITION_LABELS } from './task-decisions-model.ts';
export function TaskDecisions({ decisions = [] }: { decisions?: TaskPayload['decisions'] }) {
  const gate = useWriteGate();
  const pending = useNavigation().state !== 'idle';
  const items = decisions.filter(item => !item.consumed);
  return <section className="task-decisions" aria-label="待你决定">
    <p>{DECISION_COPY.next}</p>
    {!gate.allowed && <p>{gate.reason}</p>}
    {items.length === 0 && <p>暂无待表态的条目</p>}
    {items.map(item => <article key={`${item.kind}:${item.item_id}`}>
      <h3>{item.item_id}</h3><p>{item.text}</p>
      <p>{DISPOSITION_LABELS[item.disposition] ?? item.disposition}{item.disposition === 'ruled' && `：${item.ruling ?? ''}`}</p>
      {item.carried && <p>{DECISION_COPY.carried}</p>}
      <Form method="post"><input type="hidden" name="intent" value="decide" /><input type="hidden" name="kind" value={item.kind} /><input type="hidden" name="item_id" value={item.item_id} />
        <fieldset disabled={!gate.allowed || pending}>
          {item.kind === 'backlog' ? <><button name="disposition" value="include">纳入</button><button name="disposition" value="ignore">忽略</button></> : <>
            <label>裁定文字<textarea name="ruling" defaultValue={item.ruling ?? ''} maxLength={MAX_PROSE_LENGTH} required /></label><button name="disposition" value="ruled">裁定</button>
          </>}
          <button name="disposition" value="none" formNoValidate>撤销表态</button>
        </fieldset>
      </Form>
    </article>)}
  </section>;
}
