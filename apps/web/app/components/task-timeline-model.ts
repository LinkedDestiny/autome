import { formatClock, formatDuration, formatRange, type Clock } from '../lib/clock.tsx';
import { guardLabel } from '../lib/guard-labels.ts';
import type { LogSession } from './session-log.tsx';
import type { TaskEvent } from '../../server/task-events.ts';
import { failureText, nodeLabel, RUNTIME_LABELS, taskStatusLine, type TaskState } from '../lib/labels.ts';
export type TimelineSession = LogSession & { runtime?: string; model?: string; ended_at?: string | null; metrics?: { cost_usd?: number } | null; lifecycle?: { state: string } };
export type TimelineEntry = { key: string; at: string; title: string; summary: string; unverified?: boolean; details: string[]; session?: TimelineSession };
const milestone: Record<string, string> = { open: '开放', pending: '待审', done: '已完成', '开放': '开放', '待审': '待审', '已完成': '已完成' };
const operations: Record<string, string> = { approve: '你批准了设计', reject: '你驳回了设计', reply: '你补充了信息', merge: '你提交了合并', pause: '你暂停了任务', resume: '你恢复了任务', stop: '你停止了任务', cancel: '你取消了任务', rerun_from: '你重跑了任务', extend_budget: '你追加了预算' };
const str = (value: unknown) => value == null ? '' : String(value);
function reproduction(event: TaskEvent) {
  if (event.kind === 'bug.reproduced') return `已复现：复现命令失败（符合预期）· 退出码 ${event.payload.exit_code}`;
  return ({ exited_zero: '未复现：复现命令通过了', not_runnable: '复现命令无法执行', timed_out: '复现检查超时' } as Record<string, string>)[str(event.payload.why)] ?? str(event.payload.why);
}
function gateName(p: Record<string, unknown>) {
  if (p.node === 'fix_gate') return p.kind === 'quick' ? '修复门禁（复现 + 快检）' : '修复后回归（修复门禁）';
  if (p.node === 'gate' && p.kind === 'quick') return '快检';
  return nodeLabel(str(p.node));
}
function light(event: TaskEvent, clock: Clock): string | undefined {
  const p = event.payload, s = (key: string) => str(p[key]);
  switch (event.kind) {
    case 'gate.queued': return p.job === 'setup' ? `准备命令排队第 ${p.position} 位` : undefined;
    case 'gate.reused': return '沿用已通过的结果';
    case 'gate.skipped': return `跳过：${s('reason')}`;
    case 'rebase.conflicts': return p.count === 0 ? 'rebase 无冲突' : `rebase 冲突：${Array.isArray(p.files) ? p.files.join('、') : s('count')}`;
    case 'merge.auto': return p.active === true ? '默认分支已前进，自动 rebase 后再合并' : undefined;
    case 'merge.rebased': return `已自动 rebase：${(Array.isArray(p.repos) ? p.repos as Record<string, unknown>[] : []).map(repo => `${str(repo.repo)} → ${str(repo.onto)} @ ${str(repo.head).slice(0, 8)}`).join('；')}`;
    case 'merge.repo': return p.blocked ? `合并受阻：${s('blocked')}` : `已合并 · ${s('commit').slice(0, 8)}`;
    case 'merge.locked': return '已取得合并锁';
    case 'merge.direct': return '直接进入合并';
    case 'merge.unlocked': return '已释放合并锁';
    case 'merge.after_started': return `合并后命令开始：${s('command')} · 日志：${s('log')}`;
    case 'merge.after': return `合并后命令${p.ok ? '通过' : '未通过'}（退出码 ${s('exit_code')}）· 日志：${s('log')}`;
    case 'merge.after_skipped': return `合并后命令跳过：${s('reason')}`;
    case 'setup.started': return `准备命令开始（${s('checkout')}）：${s('command')} · 日志：${s('log')}`;
    case 'setup.result': return `准备命令${p.ok ? '通过' : '未通过'}（${s('checkout')}，退出码 ${s('exit_code')}）· 日志：${s('log')}`;
    case 'setup.skipped': return `准备命令跳过：${s('reason')}`;
    case 'task.updated': return operations[s('trigger')];
    case 'session.retry': return p.rate_limited ? `会话撞到${s('reason')}` : `会话崩溃，${formatClock(s('not_before'), clock)} 重试（第 ${s('attempt')}/${s('of')} 次）：${s('reason')}`;
    case 'session.deferred': return `${nodeLabel(s('node'))} 等 ${s('runtime')} 冷却到 ${formatClock(s('until'), clock)}`;
    case 'runtime.cooldown': return `${s('runtime')} 限流，冷却到 ${formatClock(s('until'), clock)}：${s('reason')}`;
    case 'task.protocol_failure': return `协议失败：${s('detail') || s('reason')}`;
    case 'guard.warning': return `守卫提醒：${guardLabel(s('code'))}${p.message || p.detail ? `：${s('message') || s('detail')}` : ''}`;
  }
  if (event.kind.startsWith('guard.')) return `守卫提醒：${guardLabel(s('code'))}${p.detail ? `：${s('detail')}` : ''}`;
}
function sessionSummary(session: TimelineSession, window: TaskEvent[], previous: [string, string][]) {
  if (session.running) return '运行中';
  if (session.lifecycle?.state === 'killed') return '已被停止';
  let before = new Map(previous);
  const result: string[] = [];
  for (const event of window) {
    const p = event.payload;
    if (event.kind === 'task.snapshot') {
      const current = (p.milestones ?? []) as [string, string][];
      for (const [id, state] of current) {
        if (!before.has(id)) result.push(`新增 ${id}`);
        else if (before.get(id) !== state) result.push(`${id} ${milestone[before.get(id)!] ?? before.get(id)} → ${milestone[state] ?? state}`);
      }
      before = new Map(current);
    }
    if (event.kind === 'bug.located') result.push(p.status === 'Located' ? `已定位：${str(p.root_cause)}` : p.status === 'NotABug' ? '判定不是 Bug' : '无法复现');
    if (event.kind === 'bug.reviewed') result.push(p.verdict === 'Passed' ? '复核通过' : '复核打回');
    if (event.kind === 'task.updated' && ['session_ended', 'bug_session_ended'].includes(str(p.trigger))) {
      const state = p.state as TaskState & { reason?: Record<string, unknown> };
      if (state.state === 'failed') result.push(failureText(state.reason));
      else if (!result.length) result.push(`→ ${taskStatusLine(state)}`);
    }
  }
  return result.join('；') || '已结束';
}
export function timelineEntries(sessions: readonly TimelineSession[], input: readonly TaskEvent[], now: string, timeZone = 'UTC'): TimelineEntry[] {
  const clock = { now, timeZone };
  const events = input.toSorted((a, b) => a.seq - b.seq);
  const entries: TimelineEntry[] = [];
  const anchored = new Set<string>();
  const active = new Map<string, { entry: TimelineEntry; running: boolean }>();
  let repro: TimelineEntry | undefined;
  const sessionEntry = (session: TimelineSession, index: number): TimelineEntry => {
    const next = events.findIndex((e, i) => i > index && e.kind === 'session.started');
    const previous = events.slice(0, Math.max(0, index)).findLast(e => e.kind === 'task.snapshot');
    const elapsed = Math.max(0, Date.parse(session.ended_at ?? now) - Date.parse(session.started_at));
    const runtime = session.runtime ?? '';
    return { key: session.id, at: session.started_at, session, title: `${session.label} · 第 ${session.round} 轮`,
      summary: index < 0 ? (session.running ? '运行中' : '已结束') : sessionSummary(session, events.slice(index + 1, next < 0 ? undefined : next), (previous?.payload.milestones ?? []) as [string, string][]),
      details: [Object.hasOwn(RUNTIME_LABELS, runtime) ? RUNTIME_LABELS[runtime as keyof typeof RUNTIME_LABELS] : runtime, session.model ?? '',
        formatRange(session.started_at, session.ended_at, clock), `${session.running ? '已运行' : '用时'} ${formatDuration(elapsed)}`,
        session.metrics?.cost_usd != null ? `$${session.metrics.cost_usd.toFixed(2)}` : '无计费数据'].filter(Boolean) };
  };
  for (const [index, event] of events.entries()) {
    const p = event.payload, node = str(p.node);
    if (event.kind === 'session.started') {
      const session = sessions.find(s => s.id === p.session_id);
      if (session && !anchored.has(session.id)) { entries.push(sessionEntry(session, index)); anchored.add(session.id); }
    } else if ((event.kind === 'gate.queued' && p.job === 'gate') || event.kind === 'gate.started' || event.kind === 'gate.result') {
      if (event.kind === 'gate.started') repro = undefined;
      let run = active.get(node);
      const bind = event.kind === 'gate.started' ? run && !run.running : event.kind === 'gate.result' ? run?.running : false;
      if (!bind) {
        if (run) run.entry.summary = '已中断';
        run = { entry: { key: `gate:${node}:${event.seq}`, at: event.at, title: gateName(p), summary: '', details: [] }, running: false };
        entries.push(run.entry); active.set(node, run);
      }
      if (!run) continue;
      run.entry.title = gateName(p);
      if (event.kind === 'gate.queued') run.entry.summary = `排队第 ${p.position} 位`;
      else {
        run.entry.details = [str(p.command), str(p.sha).slice(0, 8), str(p.log)].filter(Boolean);
        run.running = true;
        run.entry.summary = event.kind === 'gate.started' ? '运行中' : p.exit_code == null ? '超时' : node === 'repro_check' ? `退出码 ${p.exit_code}` : p.ok ? '通过' : `未通过 · 退出码 ${p.exit_code}`;
      }
      if (event.kind === 'gate.result' && p.steps && typeof p.steps === 'object') {
        const steps = p.steps as Record<string, number>;
        run.entry.summary += ` · 执行 ${steps.ran} 步 · 沿用 ${steps.reused} 步`;
        if (steps.not_run > 0) run.entry.summary += ` · 未跑 ${steps.not_run} 项`;
        if (steps.unverified > 0) {
          run.entry.summary += ` · 沿用来源未核实 ${steps.unverified} 步`;
          run.entry.unverified = true;
        }
      }
      if (event.kind === 'gate.result') { active.delete(node); if (node === 'repro_check') repro = run.entry; }
    } else if (event.kind === 'bug.reproduced' || event.kind === 'bug.repro_missed') {
      if (repro) { repro.summary = reproduction(event); repro = undefined; }
    } else {
      const title = light(event, clock);
      if (title) entries.push({ key: `ev:${event.seq}`, at: event.at, title, summary: '', details: [] });
    }
  }
  for (const session of sessions.toSorted((a, b) => a.started_at.localeCompare(b.started_at))) {
    if (anchored.has(session.id)) continue;
    const entry = sessionEntry(session, -1), index = entries.findIndex(e => e.at > entry.at);
    entries.splice(index < 0 ? entries.length : index, 0, entry);
  }
  return entries;
}
export const TIMELINE_TEXT = { title: '执行时间线', empty: '还没有执行记录', truncated: '更早的内核事件已省略' };
