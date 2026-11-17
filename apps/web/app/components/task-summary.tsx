import { SUMMARY_TEXT as t } from './task-summary-labels.ts';
import { summaryElapsed, type TaskSummary } from './task-summary-model.ts';
import { formatClock, formatSpan, useClock } from '../lib/clock.tsx';
import { Tile } from './ui/tile.tsx';
export function SummarySteps({ summary }: { summary: TaskSummary }) {
  const clock = useClock(), elapsed = summaryElapsed(summary, clock.now);
  return <ol className="task-stage-list summary-steps">{summary.steps.map((step, i) => <li key={step.label} aria-current={step.current ? 'step' : undefined} className={[step.passed && 'is-passed', step.label === '你批准' && 'step-approval'].filter(Boolean).join(' ')}>
    <b>{step.label}{[t.implementation, t.audit].includes(step.label) ? ` × ${step.sessions}` : ''}</b>
    <small>{step.label === t.merge ? summary.time.end ? formatClock(summary.time.end, clock) : t.unmerged : `${step.label === '你批准' ? '等了 ' : ''}${formatSpan(elapsed.steps[i])}`}</small>
    {step.sessions > 0 && <small>{step.unbilled === step.sessions ? `${step.tools.join(' · ')} · 无计费` : `$${step.cost.toFixed(2)}${step.unbilled ? ` + ${step.unbilled} 个无计费` : ''}`}</small>}
    {step.attempts > 0 && <small>{step.attempts === 1 && step.gatePassed ? '一次过' : `${step.attempts} 次`}</small>}
  </li>)}</ol>;
}
export function TaskSummaryTiles({ summary }: { summary: TaskSummary }) {
  const { now } = useClock(), elapsed = summaryElapsed(summary, now);
  const rounds = (used?: number, limit?: number | null) => used == null ? null : `${used} / ${limit ?? '—'}`;
  return <section className="task-summary-tiles" aria-label="任务统计">
    <Tile color="yellow" label="花费" value={`$${summary.cost.usd_total.toFixed(2)}`}>{summary.cost.sessions_without_cost} 个会话无计费</Tile>
    <Tile color="teal" label="总用时" value={summary.time.start ? formatSpan(elapsed.totalMs) : null}>其中等你 {formatSpan(elapsed.waitingMs)}{summary.time.incomplete ? '（事件不完整）' : ''}</Tile>
    <Tile color="blue" label="会话数" value={summary.sessions.total}>{Object.entries(summary.sessions.byRuntime).map(([runtime, n]) => `${runtime} ${n}`).join(' · ')}</Tile>
    <Tile color="purple" label={t.designRounds} value={rounds(summary.design.used, summary.design.limit)}>{t.reviews} {summary.design.reviews} 轮</Tile>
    <Tile color="orange" label={t.implementationRounds} value={rounds(summary.implementation.used, summary.implementation.limit)}>退回 {summary.implementation.reopens ?? '—'} 次</Tile>
    <Tile color="green" label="门禁" value={summary.gates ? `${summary.gates.passed} / ${summary.gates.total}` : null}>{summary.gates ? `快检 ${summary.gates.quick} · ${t.mergeGate} ${summary.gates.merge}` : '事件不完整，门禁数据缺失'}</Tile>
  </section>;
}
