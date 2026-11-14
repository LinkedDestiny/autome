import { AWAIT_PHRASES, BUG_STAGES, CARD_LABELS, COLUMN_LABELS, FEATURE_STAGES, KIND_LABELS, cooldownPhrase, fixRoundProgress, gatePhrase, mergedPhrase, milestoneProgress, queuePhrase, retryPhrase, sessionActivity, stageOf, taskStatusLine } from '../lib/labels.ts';
import { renderReason } from '../../server/notifier-rules.ts';
import type { BoardTask } from './board-data.ts';
import { columnOf } from './task-columns.ts';

export interface TaskDetail {
  gate?: { ok: boolean; node: string; exit_code: number } | null;
  gate_queue_position?: number | null;
  queue_position?: number | null;
  retry?: { attempts: number; not_before: string } | null;
  session?: { label: string; round: number; started_at: string };
  sessions?: { label: string; round: number; started_at: string; running: boolean }[];
  status_block?: { milestones_done: number; milestones_total: number; current_milestone?: string | null } | null;
}
export interface CardContext { now: string; fixRounds: number; cooldowns: { runtime: string; until: string }[]; unread: Record<string, number> }
export function needsTaskDetail(task: BoardTask) { return columnOf(task.state) !== null && task.state.state !== 'draft' && columnOf(task.state) !== 'done'; }
export function cardView(task: BoardTask, detail: TaskDetail | undefined, ctx: CardContext) {
  const stages = task.kind === 'bug' ? BUG_STAGES : FEATURE_STAGES;
  const stage = stageOf(task.kind, task.state);
  const block = detail?.status_block;
  const progress = task.kind === 'bug' && stage?.index === 1 ? fixRoundProgress(task.fix_round ?? 0, ctx.fixRounds)
    : task.kind === 'feature' && stage?.index === 3 && block && block.milestones_total > 0 ? milestoneProgress(block.milestones_done, block.milestones_total) : null;
  const retry = detail?.retry;
  const cooldown = retry && ctx.cooldowns.find(item => item.until === retry.not_before);
  const session = detail?.session ?? detail?.sessions?.find(item => item.running);
  const node = task.state.node;
  let activity = taskStatusLine(task.state);
  if (task.state.state === 'active' && node && Object.hasOwn(AWAIT_PHRASES, node)) activity = AWAIT_PHRASES[node as keyof typeof AWAIT_PHRASES];
  else if (retry) activity = cooldown ? cooldownPhrase(cooldown.runtime, cooldown.until) : retryPhrase(retry.attempts, retry.not_before);
  else if (session) activity = sessionActivity(session.label, session.round, task.kind === 'feature' ? block?.current_milestone : null, Math.max(0, Math.floor((Date.parse(ctx.now) - Date.parse(session.started_at)) / 60_000)));
  else if (task.state.state === 'queued') activity = queuePhrase(detail?.queue_position);
  else if (task.state.state === 'failed') activity = CARD_LABELS.failed + renderReason(task.state.reason);
  return {
    kind: Object.hasOwn(KIND_LABELS, task.kind) ? KIND_LABELS[task.kind as keyof typeof KIND_LABELS] : task.kind,
    stages: stages.map((item, index) => ({ label: item.label, current: stage?.index === index, passed: stage != null && stage.index > index })),
    unknownStage: stage && stage.index < 0 ? stage.label : null, progress, activity,
    gate: gatePhrase(detail?.gate, detail?.gate_queue_position),
    merged: task.state.state === 'done' && task.merge_commit ? mergedPhrase(task.merge_commit) : null,
    badge: columnOf(task.state) === 'needs_you' ? `${COLUMN_LABELS.needs_you}${ctx.unread[task.id] > 0 ? ` ${ctx.unread[task.id]}` : ''}` : null,
  };
}

// The board never serializes task documents, logs, or historical sessions.
export function toCardDetail(detail: TaskDetail): TaskDetail {
  const { gate, gate_queue_position, queue_position, retry, status_block } = detail;
  const session = detail.sessions?.find(item => item.running);
  return {
    gate: gate && { ok: gate.ok, node: gate.node, exit_code: gate.exit_code },
    gate_queue_position, queue_position,
    retry: retry && { attempts: retry.attempts, not_before: retry.not_before },
    session: session && { label: session.label, round: session.round, started_at: session.started_at },
    status_block: status_block && { milestones_done: status_block.milestones_done, milestones_total: status_block.milestones_total, current_milestone: status_block.current_milestone },
  };
}
