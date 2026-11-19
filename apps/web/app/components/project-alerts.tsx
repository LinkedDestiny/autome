import { formatClock, useClock } from '../lib/clock.tsx';
import { Form, Link } from 'react-router';
import { ALERTS, ALERT_KINDS } from '../lib/admin-labels.ts';
import { useWriteGate } from '../lib/live.tsx';
import type { ProjectAlert } from './alerts-data.ts';

export function ProjectAlerts({ alerts, error }: { alerts: ProjectAlert[]; error?: string }) {
  const clock = useClock();
  const gate = useWriteGate();
  if (!alerts.length && !error) return null;
  return <section className="board-alerts" aria-label={ALERTS.title}>
    {error && <p role="alert">{ALERTS.readError}{error}</p>}
    {alerts.map(alert => <article className="project-alert" key={alert.id}>
      <strong>{Object.hasOwn(ALERT_KINDS, alert.kind) ? ALERT_KINDS[alert.kind] : alert.kind}</strong>
      <time dateTime={alert.created_at}>{formatClock(alert.created_at, clock)}</time>
      <p className="project-alert-detail">{alert.detail}</p>
      {alert.task_id && <Link className="link" to={`/tasks/${encodeURIComponent(alert.task_id)}`}>{alert.task_id}</Link>}
      <Form method="post"><input type="hidden" name="intent" value="dismiss" /><input type="hidden" name="alert_id" value={alert.id} />
        <button className="btn btn-sm" disabled={!gate.allowed} title={gate.reason}>{ALERTS.dismiss}</button>
      </Form>
    </article>)}
  </section>;
}
