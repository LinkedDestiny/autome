import { SUMMARY_TEXT as t } from './task-summary-labels.ts';
import type { ReactNode } from 'react';
import { Pill } from './ui/pill.tsx';
import { formatClock, useClock } from '../lib/clock.tsx';
import { Link } from 'react-router';
import { headerView } from './task-header-model.ts';
export function TaskHeader({ view, children }: { view: ReturnType<typeof headerView>; children?: ReactNode }) {
  const clock = useClock();
  return <header className="task-detail-header">
    <div className="task-detail-meta"><Pill color={view.merged ? 'green' : 'teal'} filled>{view.merged ? `${t.merged}${view.completedAt ? ' · ' + formatClock(view.completedAt, clock) : ''}` : view.activity}</Pill><span>{view.kind}</span><span>{view.id}</span>{view.project && <Link className="link" to={view.project.href}>{view.project.label}</Link>}<code>{view.branch}</code>{view.mergeCommit && <code>{view.mergeCommit}</code>}</div>{children}
    <h1>{view.title}</h1>

  </header>;
}
