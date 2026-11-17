import { detailStageNode } from './task-stage-model.ts';
import type { TaskPayload } from './task-detail-data.ts';
import type { TaskEvent } from '../../server/task-history.ts';
import { sessionNode } from './task-timeline-model.ts';
import { taskWaits, isCountedGate } from '../lib/waiting.ts';

const feature = [ ['整理', 'intake'], ['设计', 'design'], ['评审·裁决', 'review', 'adjudicate'], ['你批准', 'await_design_approval'], ['实现', 'implement', 'gate'], ['审计', 'audit'], ['复盘', 'retro'], ['合并前门禁', 'merge_gate'], ['合并', 'await_merge', 'rebase', 'merging', 'main_check', 'cleanup', 'done'] ];
const bug = [['定位', 'locate', 'await_info', 'not_a_bug'], ['复现检查', 'repro_check'], ['修复', 'fix'], ['修复门禁', 'fix_gate'], ['复核', 'fix_review'], ['合并', 'await_merge', 'rebase', 'merge_gate', 'merging', 'main_check', 'cleanup', 'done']];
const span = (start: string | undefined, end: string) => start ? Math.max(0, Date.parse(end) - Date.parse(start)) : 0;
export function taskSummary(data: TaskPayload, input: readonly TaskEvent[], truncated = false) {
  const events = input.filter(e => e.subject === data.task.id).toSorted((a, b) => a.seq - b.seq);
  const waits = taskWaits(events);
  const gates = events.filter(e => isCountedGate(e.kind, e.payload.node));
  const node = detailStageNode(data, events);
  const definitions = data.task.kind === 'bug' ? bug : feature;
  const current = definitions.findIndex(([, ...nodes]) => nodes.includes(node ?? ''));
  const steps = definitions.map(([label, ...nodes], index) => {
    const sessions = data.sessions.filter(s => nodes.includes(sessionNode(s.kind) ?? ''));
    let durationMs = sessions.reduce((sum, s) => sum + (s.ended_at ? span(s.started_at, s.ended_at) : 0), 0);
    const runningSince = sessions.filter(s => s.running && !s.ended_at).map(s => Date.parse(s.started_at));
    if (nodes.includes('await_design_approval')) {
      durationMs = waits.closed.filter(w => w.kind === 'await_design_approval').reduce((n, w) => n + w.end - w.start, 0);
      if (waits.open?.kind === 'await_design_approval') runningSince.push(waits.open.start);
    }
    const results = events.filter(e => ['复现检查', '修复门禁', '合并前门禁'].includes(label) && e.kind === 'gate.result' && nodes.includes(String(e.payload.node)));
    for (const result of results) {
      const start = events.findLast(e => e.seq < result.seq && e.kind === 'gate.started' && e.payload.node === result.payload.node && e.payload.log === result.payload.log);
      if (start) durationMs += span(start.at, result.at);
    }
    return { label, current: current === index, passed: data.task.state?.state === 'done' || current > index, durationMs, runningSince,
      cost: sessions.reduce((n, s) => n + (s.metrics?.cost_usd ?? 0), 0), unbilled: sessions.filter(s => s.metrics?.cost_usd == null).length,
      tools: [...new Set(sessions.map(s => s.runtime).filter((s): s is string => !!s))], sessions: sessions.length, gatePassed: results.at(-1)?.payload.ok === true, attempts: results.length };
  });
  const byRuntime: Record<string, number> = {};
  for (const s of data.sessions) byRuntime[s.runtime ?? '未知'] = (byRuntime[s.runtime ?? '未知'] ?? 0) + 1;
  const block = data.status_block, metrics = data.task.metrics;
  return { steps, cost: data.cost ?? { usd_total: 0, sessions: data.sessions.length, sessions_without_cost: data.sessions.filter(s => s.metrics?.cost_usd == null).length },
    time: { start: data.task.created_at, end: data.task.completed_at, waitingMs: waits.closed.reduce((n, w) => n + w.end - w.start, 0), openWaiting: waits.open, incomplete: truncated },
    sessions: { total: data.sessions.length, byRuntime },
    design: { used: block?.design_round ?? metrics?.design_rounds_used, limit: block?.design_round_limit ?? metrics?.design_rounds_limit, reviews: data.sessions.filter(s => sessionNode(s.kind) === 'review').length },
    implementation: { used: block?.impl_round ?? metrics?.impl_rounds_used, limit: data.task.budget_n ?? block?.impl_round_limit, reopens: data.task.completed_at ? metrics?.reopen_total ?? null : block?.milestones?.reduce((n, m) => n + (m.reopen_count ?? 0), 0) ?? null },
    gates: truncated ? null : { total: gates.length, passed: gates.filter(e => e.payload.ok === true).length, quick: gates.filter(e => e.payload.node !== 'merge_gate').length, merge: gates.filter(e => e.payload.node === 'merge_gate').length } };
}
export type TaskSummary = ReturnType<typeof taskSummary>;
export function summaryElapsed(summary: TaskSummary, now: string) {
  return { totalMs: span(summary.time.start, summary.time.end ?? now), waitingMs: summary.time.waitingMs + (summary.time.openWaiting ? Math.max(0, Date.parse(summary.time.end ?? now) - summary.time.openWaiting.start) : 0),
    steps: summary.steps.map(step => step.durationMs + step.runningSince.reduce((n, start) => n + Math.max(0, Date.parse(now) - start), 0)) };
}
