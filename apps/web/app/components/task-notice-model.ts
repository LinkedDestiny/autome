import { cooldownPhrase, failureText, gatePhrase, nodeLabel, queuePhrase, retryPhrase, taskStatusLine } from '../lib/labels.ts';
import type { TaskPayload } from './task-detail-data.ts';
import type { CardContext } from './task-card-model.ts';
export function noticeView(payload: TaskPayload, ctx: CardContext) {
  const state = payload.task.state ?? { state: 'draft' };
  const messages: string[] = [];
  let next: string | null = null;
  if (state.state === 'failed') {
    messages.push(failureText(state.reason));
    const reason = state.reason?.kind;
    next = reason === 'fix_budget' ? '追加轮次' : reason === 'gate_same_failure' ? `从 ${nodeLabel(state.at ?? '?')} 重跑或追加轮次`
      : ['design_budget', 'impl_budget'].includes(String(reason)) ? '重跑 / 追加轮次 / 取消' : '重跑 / 取消';
  } else if (state.state === 'stopped' || state.state === 'paused') {
    messages.push(taskStatusLine(state)); next = '恢复';
  }
  const retry = payload.retry;
  if (retry) {
    const cooldown = ctx.cooldowns.find(item => item.until === retry.not_before);
    messages.push(cooldown ? cooldownPhrase(cooldown.runtime, cooldown.until) : retryPhrase(retry.attempts, retry.not_before));
  }
  if (payload.gate_queue_position != null) messages.push(gatePhrase(null, payload.gate_queue_position));
  if (state.state === 'queued') messages.push(queuePhrase(payload.queue_position));
  const merge = payload.task.merge;
  if (merge?.stale) messages.push(`等你合并，但默认分支已前进 ${merge.stale.behind} 个提交，按合并会先 rebase 并重跑合并前门禁`);
  if (merge?.waiting_for) messages.push(`合并前门禁在等 ${merge.waiting_for} 合并完`);
  return { messages, next, failed: state.state === 'failed' };
}
