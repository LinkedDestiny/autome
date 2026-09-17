// The core's vocabulary in the interface's words.
//
// Every string here is a rendering of a value the core defines — a `Node`, a
// `TaskState`, a `Role`, a `Login`. Keeping them in one file rather than
// inline in eight screens is what stops the same node being "实现" on the
// dashboard and "实现中" in the task panel. The Chinese is the design
// document's; proper nouns (Claude Code, Codex, rebase, main, worktree,
// Backlog) stay as written, per requirement U-11.
//
// When the core sends a value this table does not know, the fallback is the
// raw value, not a guess. An unknown node rendering as `some_new_node` is a
// bug report; rendering as "运行中" is a lie.

/** `Node` (crates/autome-domain/src/task.rs), in lifecycle order (T-03). */
export const NODE_LABELS = {
  intake: '任务整理',
  design: '设计',
  review: '评审',
  adjudicate: '裁决',
  await_design_approval: '设计批准',
  implement: '实现',
  audit: '审计',
  retro: '复盘',
  rebase: 'rebase',
  await_merge: '待合并',
  merging: '合并',
  cleanup: '清理',
};

/**
 * The stones of requirement T-03's node flow. `received` and `done` are not
 * `Node` values — the first is the moment before intake, the last is
 * `TaskState::Done` — but the user sees one line, so the flow carries them
 * too, marked as synthetic.
 */
export const FLOW = [
  { key: 'received', label: '已接收', synthetic: true },
  { key: 'intake', label: '任务整理' },
  { key: 'design', label: '设计' },
  { key: 'review', label: '评审' },
  { key: 'adjudicate', label: '裁决' },
  { key: 'await_design_approval', label: '设计批准' },
  { key: 'implement', label: '实现' },
  { key: 'audit', label: '审计' },
  { key: 'retro', label: '复盘' },
  { key: 'rebase', label: 'rebase' },
  { key: 'await_merge', label: '待合并' },
  { key: 'merging', label: '合并' },
  { key: 'cleanup', label: '清理' },
  { key: 'done', label: '完成', synthetic: true },
];

export const ROLE_LABELS = {
  plan: '设计',
  review: '评审',
  adjudicate: '裁决',
  impl: '实现',
  audit: '审计',
  retro: '复盘',
};

/** The role a node runs, mirroring `Node::role()`. `null` = system or human. */
export const NODE_ROLE = {
  design: 'plan',
  review: 'review',
  adjudicate: 'adjudicate',
  implement: 'impl',
  audit: 'audit',
  retro: 'retro',
};

export const RUNTIME_LABELS = { claude: 'Claude Code', codex: 'Codex' };

export const COMPONENT_LABELS = {
  git: 'Git',
  claude: 'Claude Code',
  codex: 'Codex',
  iterm2: 'iTerm2',
};

/** Which roles each component serves — the environment page's 被谁用到 card. */
export const COMPONENT_NOTE = {
  git: '所有任务必需',
  claude: '设计 · 裁决 · 实现 · 任务整理 · Onboarding',
  codex: '评审 · 审计',
  iterm2: '一键安装与登录命令在其中执行',
};

export const MILESTONE_LABELS = { open: '开放', pending: '待审', done: '已完成' };

export const DOC_STATUS_LABELS = {
  designing: '设计中',
  implementing: '实现中',
  done: '已完成',
  infeasible: '不可实现',
  protocol_failure: '协议失败',
};

export function nodeLabel(node) {
  if (!node) return '—';
  return NODE_LABELS[node] || node;
}

export function roleLabel(role) {
  return ROLE_LABELS[role] || role;
}

export function runtimeLabel(runtime) {
  return RUNTIME_LABELS[runtime] || runtime;
}

/** The node a `TaskState` sits at, or `null` for the terminal states. */
export function stateNode(state) {
  if (!state) return null;
  if (state.state === 'active') return state.node;
  if (state.state === 'paused') return state.resume;
  if (state.state === 'stopped' || state.state === 'failed') return state.at;
  return null;
}

/** Why a task failed (T-10), in the words the stopping panel uses. */
export function failureReason(reason) {
  if (!reason) return '未知原因';
  switch (reason.kind) {
    case 'design_budget':
      return `设计轮次耗尽 · 上限 ${reason.limit}`;
    case 'impl_budget':
      return `实现预算耗尽 · 上限 ${reason.limit}`;
    case 'infeasible':
      return '设计判定这件事做不了';
    case 'protocol':
      return `协议失败 · ${reason.detail}`;
    case 'session_crashed':
      return `会话崩溃 · ${reason.detail}`;
    case 'rebase_conflict':
      return `rebase 冲突无法自动解决 · ${reason.detail}`;
    case 'config':
      return `配置阻止了启动 · ${reason.detail}`;
    case 'core_step':
      return `${nodeLabel(reason.node)} 步骤失败 · ${reason.detail}`;
    default:
      return reason.kind;
  }
}

/**
 * The short status line for a task card: what it is doing right now.
 * Returns `{ label, variant, spinning }` so the caller can pick the pill.
 */
export function taskStatus(task) {
  const state = task && task.state;
  if (!state) return { label: '未知', variant: 'outlined', spinning: false };
  switch (state.state) {
    case 'queued':
      return { label: '排队', variant: 'outlined', spinning: false };
    case 'active': {
      const node = state.node;
      if (node === 'await_design_approval') {
        return { label: '等待批准', variant: 'solid-yellow', spinning: false, icon: 'hand' };
      }
      if (node === 'await_merge') {
        return { label: '待合并', variant: 'solid-green', spinning: false, icon: 'branch' };
      }
      if (node === 'intake') return { label: '任务整理', variant: 'solid-brown', spinning: true };
      if (node === 'design' || node === 'review' || node === 'adjudicate') {
        return { label: nodeLabel(node), variant: 'solid-blue', spinning: true };
      }
      if (node === 'audit') return { label: '审计', variant: 'solid-purple', spinning: true };
      if (node === 'retro') return { label: '复盘', variant: 'solid-purple', spinning: true };
      if (node === 'implement') return { label: '实现', variant: 'solid-teal', spinning: true };
      return { label: nodeLabel(node), variant: 'solid-teal', spinning: true };
    }
    case 'paused':
      return { label: `已暂停 · ${nodeLabel(state.resume)}`, variant: 'soft-yellow', spinning: false };
    case 'stopped':
      return { label: `已停止 · ${nodeLabel(state.at)}`, variant: 'soft-brown', spinning: false };
    case 'failed':
      return { label: '失败', variant: 'solid-red', spinning: false, icon: 'warn' };
    case 'done':
      return { label: '已完成', variant: 'soft-green', spinning: false, icon: 'check' };
    case 'cancelled':
      return { label: '已取消', variant: 'outlined', spinning: false };
    default:
      return { label: state.state, variant: 'outlined', spinning: false };
  }
}

/** The four environment components, in the order the design shows them. */
export const COMPONENT_ORDER = ['git', 'claude', 'codex', 'iterm2'];

export function loginLabel(login) {
  if (!login) return null;
  switch (login.login) {
    case 'not_applicable':
      return null;
    case 'ok':
      return login.account_hint ? `已登录 · 账号 ${login.account_hint}` : '已登录';
    case 'expired':
      return '认证已过期';
    case 'unknown':
      return `登录状态未知 · ${login.detail}`;
    default:
      return login.login;
  }
}

/** A config violation (C-06, S-04) as the sentence the routing graph shows. */
export function violationText(violation) {
  switch (violation.kind) {
    case 'same_model':
      return `${roleLabel(violation.evaluator)}与${roleLabel(violation.generator)}都配成了 ${violation.identity}。评测侧必须与生成侧不同模型，保存已被阻止。`;
    case 'skill_not_visible':
      return `${roleLabel(violation.role)}用 ${runtimeLabel(violation.runtime)}，看不到技能 ${violation.skill}。`;
    case 'skill_not_found':
      return `${roleLabel(violation.role)}绑定的技能 ${violation.skill} 没有安装。`;
    case 'parallel_out_of_range':
      return `并行 ${violation.value} 超出 ${violation.min}–${violation.max}。`;
    case 'design_rounds_out_of_range':
      return `设计轮次上限 ${violation.value} 不在允许范围内。`;
    case 'budget_factor_out_of_range':
      return `实现预算系数 ${violation.value} 不在允许范围内。`;
    default:
      return violation.detail || violation.kind;
  }
}

/** The roles a violation makes unsaveable, so the graph can paint them red. */
export function violationRoles(violation) {
  if (violation.kind === 'same_model') return [violation.evaluator, violation.generator];
  if (violation.role) return [violation.role];
  return [];
}

/** `2026-09-15T13:48:02Z` → `13:48`. Returns '' rather than "Invalid Date". */
export function clock(iso) {
  if (!iso) return '';
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return String(iso);
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
}

/** `2026-09-15T...` → `09-15`. */
export function day(iso) {
  if (!iso) return '';
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return String(iso);
  return `${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

/** Elapsed time between two ISO instants, as `1h 42m` / `12m`. */
export function duration(fromIso, toIso) {
  if (!fromIso) return '';
  const from = new Date(fromIso).getTime();
  const to = toIso ? new Date(toIso).getTime() : Date.now();
  if (!Number.isFinite(from) || !Number.isFinite(to) || to < from) return '';
  const minutes = Math.floor((to - from) / 60000);
  if (minutes < 60) return `${minutes}m`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}

/** `claude · claude-opus-5 · high`, with the effort omitted when defaulted. */
export function runtimeSummary(config) {
  if (!config) return '';
  const parts = [runtimeLabel(config.runtime), config.model];
  parts.push(config.effort ? config.effort : '默认 Effort');
  return parts.join(' · ');
}

/** The three appearance choices, mirroring `Theme::display_name` in the core. */
const THEME_LABELS = { system: '跟随系统', light: '浅色', dark: '深色' };

export function themeLabel(value) {
  return THEME_LABELS[value] || value || '跟随系统';
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------
//
// One rule across all of these: **absent reads as absent.** A task run
// entirely on Codex has an unknown cost, not a zero one — Codex reports no
// price — and a dash says that where `$0.00` would be a claim.

/** `$1.23`, or `—` when nothing reported a price. */
export function cost(usd) {
  if (typeof usd !== 'number' || !Number.isFinite(usd)) return '—';
  return usd >= 10 ? `$${usd.toFixed(1)}` : `$${usd.toFixed(2)}`;
}

/** `12.3k`, `1.2M`. Token counts are large and their last digits say nothing. */
export function tokens(n) {
  if (typeof n !== 'number' || !Number.isFinite(n)) return '—';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}

export function count(n) {
  return typeof n === 'number' && Number.isFinite(n) ? String(n) : '—';
}

/** `9/35`, the two numbers a round budget is. */
export function ratio(used, limit) {
  if (typeof used !== 'number') return '—';
  return typeof limit === 'number' && limit > 0 ? `${used}/${limit}` : String(used);
}

/** `protocol/v7 (3f9a12cd)` — the tag is the name, the hash is the identity. */
export function protocolRef(wire) {
  if (typeof wire !== 'string' || !wire) return '—';
  const at = wire.lastIndexOf('@');
  if (at < 0) return wire;
  return `${wire.slice(0, at)} (${wire.slice(at + 1, at + 9)})`;
}

/** The metric names the core records, in the order the version page shows. */
export const METRIC_LABELS = {
  design_rounds_used: '设计轮',
  impl_rounds_used: '实现轮',
  reopen_total: 'reopen',
  impl_defects: '实现缺陷',
  verification_gaps: '验证缺口',
  protocol_failures: '协议失败',
  closed_then_contradicted: '关闭后被推翻',
  manual_items_open: '人工未确认',
  total_tokens: 'tokens',
  total_turns: 'turns',
  total_cost_usd: '费用',
  mean_request_input: '每次请求输入',
};

export function metricLabel(key) {
  return METRIC_LABELS[key] || key;
}

/** Formats a metric's mean for the version page, by what kind of number it is. */
export function metricValue(key, value) {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '—';
  if (key === 'total_cost_usd') return cost(value);
  if (key === 'total_tokens' || key === 'mean_request_input') return tokens(Math.round(value));
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}
