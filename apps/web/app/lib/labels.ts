// Chinese vocabulary and stages mirror crates/autome-domain/src/task.rs.
export const KIND_LABELS = { feature: '需求', bug: 'Bug' } as const;
export const STATE_LABELS = { draft: '未开始', queued: '排队中', active: '进行中', paused: '已暂停', stopped: '已停止', failed: '失败', done: '已完成', cancelled: '已取消' } as const;
export const BOARD_READ_ERROR = '项目读取失败';
export const COLUMN_LABELS = { todo: '待开始', running: '进行中', needs_you: '等你处理', done: '已完成' } as const;
export const NODE_LABELS = {
  intake: '任务整理', design: '设计', review: '评审', adjudicate: '裁决', await_design_approval: '设计批准',
  implement: '实现', gate: '门禁', audit: '审计', retro: '复盘', rebase: 'rebase', merge_gate: '合并前门禁',
  await_merge: '待合并', merging: '合并', main_check: '验证 main', cleanup: '清理',
  locate: '定位', repro_check: '复现检查', fix: '修复', fix_gate: '修复门禁', fix_review: '复核',
  await_info: '等你补充信息', not_a_bug: '判定不是 Bug',
} as const;
export const AWAIT_PHRASES = { await_design_approval: '等你批准设计', await_merge: '等你合并', await_info: '无法复现，等你补充', not_a_bug: '判定不是 Bug，等你确认' } as const;
export interface Stage { label: string; nodes: readonly string[] }
const merge: Stage = { label: '合并', nodes: ['rebase', 'merge_gate', 'await_merge', 'merging', 'main_check', 'cleanup'] };
const done: Stage = { label: '完成', nodes: [] };
export const FEATURE_STAGES: readonly Stage[] = [
  { label: '整理', nodes: ['intake'] },
  { label: '设计', nodes: ['design', 'review', 'adjudicate'] },
  { label: '批准设计', nodes: ['await_design_approval'] },
  { label: '实现', nodes: ['implement', 'gate'] },
  { label: '审计', nodes: ['audit', 'retro'] }, merge, done,
];
export const BUG_STAGES: readonly Stage[] = [
  { label: '定位', nodes: ['locate', 'repro_check', 'await_info', 'not_a_bug'] },
  { label: '修复', nodes: ['fix', 'fix_gate'] },
  { label: '复核', nodes: ['fix_review'] }, merge, done,
];
// Unknown wire values remain representable, so a newer core is not mislabelled.
export interface TaskState { state: string; node?: string; resume?: string; at?: string }
export function nodeLabel(node: string): string { return Object.hasOwn(NODE_LABELS, node) ? NODE_LABELS[node as keyof typeof NODE_LABELS] : node; }
export function stageOf(kind: string, state: TaskState): { label: string; index: number; total: number } | null {
  const stages = kind === 'bug' ? BUG_STAGES : FEATURE_STAGES;
  const node = state.state === 'active' ? state.node : state.state === 'paused' || state.state === 'queued' ? state.resume : state.state === 'stopped' || state.state === 'failed' ? state.at : undefined;
  if (state.state !== 'done' && node === undefined) return null;
  const index = state.state === 'done' ? stages.length - 1 : stages.findIndex(stage => stage.nodes.includes(node!));
  return { label: index < 0 ? node! : stages[index].label, index, total: stages.length };
}
export function taskStatusLine(state: TaskState): string {
  const node = state.state === 'active' ? state.node : state.state === 'paused' ? state.resume : state.at;
  if (state.state === 'active') return node && Object.hasOwn(AWAIT_PHRASES, node) ? AWAIT_PHRASES[node as keyof typeof AWAIT_PHRASES] : nodeLabel(node ?? '');
  const label = Object.hasOwn(STATE_LABELS, state.state) ? STATE_LABELS[state.state as keyof typeof STATE_LABELS] : state.state;
  return node ? `${label} · ${nodeLabel(node)}` : label;
}
export const KIND_FILTER_LABELS = { all: '全部', feature: '需求', bug: 'Bug' } as const;
export function doneFoldLabel(expanded: boolean, count: number): string { return expanded ? '收起' : `查看全部（${count}）`; }
export const RUNTIME_LABELS = { claude: 'Claude Code', codex: 'Codex' } as const;
export const CARD_LABELS = { progress: '阶段进度', failed: '失败：', justStarted: '刚开始' } as const;
export function milestoneProgress(done: number, total: number) { return `已完成 ${done} / ${total}`; }
export function fixRoundProgress(round: number, total: number) { return `修复第 ${round} / ${total} 轮`; }
export function formatClock(iso: string) { const date = new Date(iso); return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`; }
export function sessionActivity(label: string, round: number, milestone: string | null | undefined, minutes: number) {
  return [`${label}第 ${round} 轮`, milestone, minutes > 0 ? `运行 ${minutes} 分钟` : CARD_LABELS.justStarted].filter(Boolean).join(' · ');
}
export function cooldownPhrase(runtime: string, until: string) { return `等 ${Object.hasOwn(RUNTIME_LABELS, runtime) ? RUNTIME_LABELS[runtime as keyof typeof RUNTIME_LABELS] : runtime} 冷却到 ${formatClock(until)}`; }
export function retryPhrase(attempt: number, until: string) { return `等待重试（第 ${attempt} 次）· ${formatClock(until)}`; }
export function queuePhrase(position?: number | null) { return position == null ? '排队中' : `排队第 ${position} 位`; }
export function gatePhrase(gate?: { ok: boolean; node: string; exit_code: number } | null, position?: number | null) {
  if (position != null) return `门禁排队第 ${position} 位`;
  return !gate ? '门禁未跑过' : gate.ok === true ? `门禁通过 · ${nodeLabel(gate.node)}` : `门禁未通过 · 退出码 ${gate.exit_code}`;
}
export function mergedPhrase(sha: string) { return `已合并 · ${sha.slice(0, 7)}`; }
