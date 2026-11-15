import { BUG_STAGES, FEATURE_STAGES, STATE_LABELS, fixRoundProgress, stageOf, type TaskState } from '../lib/labels.ts';
import type { TaskEvent } from '../../server/task-history.ts';
import type { TaskPayload } from './task-detail-data.ts';
export interface TaskConfig { resolved?: { bug?: { fix_rounds?: number } } }
const milestoneLabels: Record<string, string> = { open: '开放', pending: '待审', done: '已完成' };
export function stageBarView(payload: TaskPayload, events: readonly TaskEvent[], config: TaskConfig = {}) {
  const task = payload.task;
  const kind = task.kind ?? 'feature';
  const state = task.state ?? { state: 'draft' };
  const history = events.filter(event => event.subject === task.id).toSorted((a, b) => b.seq - a.seq);
  let position: TaskState = state;
  if (state.state === 'cancelled') {
    const cancellation = history.find(event => event.kind === 'task.updated' && (event.payload.state as TaskState | undefined)?.state === 'cancelled');
    position = { state: 'cancelled' };
    for (const event of history) {
      if (event.kind !== 'task.updated' || (cancellation && event.seq >= cancellation.seq)) continue;
      const prior = event.payload.state as TaskState | undefined;
      if (prior && stageOf(kind, prior) && prior.state !== 'done') { position = prior; break; }
    }
  }
  const current = stageOf(kind, position);
  const stateLabel = Object.hasOwn(STATE_LABELS, state.state) ? STATE_LABELS[state.state as keyof typeof STATE_LABELS] : state.state;
  const status = ['failed', 'stopped', 'paused', 'queued', 'cancelled'].includes(state.state) ? stateLabel : null;
  const stages = (kind === 'bug' ? BUG_STAGES : FEATURE_STAGES).map((stage, index) => ({
    label: stage.label, current: current?.index === index, passed: state.state === 'done' || (current != null && current.index > index),
    status: current?.index === index && state.state !== 'cancelled' ? status : null,
  }));
  const block = payload.status_block;
  const snapshot = history.find(event => event.kind === 'task.snapshot')?.payload;
  const metrics = history.find(event => event.kind === 'task.metrics')?.payload;
  const milestones = (block?.milestones ?? ((snapshot?.milestones ?? []) as [string, string][]).map(([id, state]) => ({ id, state })))
    .map(item => ({ ...item, label: milestoneLabels[item.state] ?? item.state, current: item.id === block?.current_milestone }));
  const rounds = kind === 'bug' ? fixRoundProgress(task.fix_round ?? 0, task.budget_n ?? config.resolved?.bug?.fix_rounds ?? 0)
    : block ? `设计 ${block.design_round} / ${block.design_round_limit} · 实现 ${block.impl_round} / ${task.budget_n ?? block.impl_round_limit}`
    : metrics ? `设计 ${metrics.design_rounds_used} / ${metrics.design_rounds_limit} · 实现 ${metrics.impl_rounds_used} / ${task.budget_n ?? metrics.budget_n}` : null;
  const repro = history.find(event => event.kind === 'bug.reproduced' || event.kind === 'bug.repro_missed');
  let reproduction = '复现检查未跑';
  if (repro?.kind === 'bug.reproduced') reproduction = `已复现：复现命令失败（符合预期）· 退出码 ${repro.payload.exit_code}`;
  else if (repro) {
    switch (repro.payload.why) {
      case 'exited_zero': reproduction = '未复现：复现命令通过了'; break;
      case 'not_runnable': reproduction = '复现命令无法执行（退出码 126/127）'; break;
      case 'timed_out': reproduction = '复现检查超时，不能据此断言未复现'; break;
      default: reproduction = `复现检查：${repro.payload.why}`;
    }
  }
  return { stages, status: state.state === 'cancelled' ? stateLabel : null, unknownStage: current?.index === -1 ? current.label : null,
    milestones: kind === 'bug' ? [] : milestones, rounds, reproduction: kind === 'bug' ? reproduction : null,
    message: kind === 'bug' ? null : payload.status_error ? `设计文档读不出状态块：${payload.status_error_detail ?? ''}`
      : !milestones.length ? '设计文档还没写出里程碑' : null };
}
