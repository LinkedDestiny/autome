import { useEffect, useRef } from 'react';
import { SummarySteps } from './task-summary.tsx';
import type { TaskSummary } from './task-summary-model.ts';
import { Pill } from './ui/pill.tsx';
import { stageBarView } from './task-stage-model.ts';
import { CARD_LABELS, STATE_LABELS } from '../lib/labels.ts';
export function revealCurrentMilestone(list: HTMLUListElement) {
  const current = list.querySelector<HTMLElement>('.is-current');
  if (!current) return;
  const viewport = list.getBoundingClientRect(), item = current.getBoundingClientRect();
  if (item.left < viewport.left) list.scrollLeft += item.left - viewport.left;
  else if (item.right > viewport.right) list.scrollLeft += Math.min(item.right - viewport.right, item.left - viewport.left);
}
export function TaskStage({ view, summary }: { view: ReturnType<typeof stageBarView>; summary?: TaskSummary }) {
  const milestonesRef = useRef<HTMLUListElement>(null);
  const currentId = view.milestones.find(item => item.current)?.id;
  useEffect(() => {
    if (milestonesRef.current) revealCurrentMilestone(milestonesRef.current);
  }, [currentId]);
  return <section className="task-stage" aria-label={CARD_LABELS.progress}>
    {view.status && <p>{view.status}</p>}
    <>{summary ? <SummarySteps summary={summary} /> : <ol className="task-stage-list">{view.stages.map(stage => <li key={stage.label} aria-current={stage.current ? 'step' : undefined} className={stage.passed ? 'is-passed' : undefined}>
      {stage.label}{stage.passed && <span className="task-stage-mark" aria-label={STATE_LABELS.done}> ✓</span>}{stage.status && <small> · {stage.status}</small>}
    </li>)}</ol>}</>
    {view.unknownStage && <p>{view.unknownStage}</p>}
    <div className="task-detail-meta"><span>{view.rounds}</span><span>{view.reproduction}</span></div>
    {view.milestones.length > 0 && <div className="task-milestone-bar"><Pill>里程碑 {STATE_LABELS.done} {view.milestones.filter(item => item.state === 'done').length} / {view.milestones.length}</Pill><ul ref={milestonesRef} className="task-milestones">{view.milestones.map(item => <li key={item.id} className={item.current ? 'is-current' : undefined}><Pill filled={item.current} color={item.current ? 'yellow' : item.state === 'done' ? 'green' : item.state === 'pending' ? 'yellow' : 'blue'}>{item.id} · {item.label}</Pill> {item.title}</li>)}</ul></div>}
    {view.message && <p>{view.message}</p>}
  </section>;
}
