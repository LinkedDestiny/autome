'use strict';

// Representative payloads, shaped exactly as `crates/automed/src/dispatch.rs`
// serialises them. Hand-written rather than recorded, because the point of
// them is to pin the *contract*: if a field is renamed in the core, these stop
// matching and the harness's "renders without throwing" checks start passing
// for the wrong reason (a screen that renders "—" everywhere throws nothing).
//
// Each payload is deliberately busy — a failed task, a merge stop, a config
// violation, an expired login — because the empty case is the one a renderer
// gets right by accident.

const TASK_RUNNING = {
  id: 'T-015',
  slug: 'checkout-flow',
  title: '购物车结算流程',
  request: '给岛屿商店加购物车结算：支持优惠码，库存不足时阻止下单，结算成功后用现有 mailer 发确认邮件。不要动支付网关。',
  state: { state: 'active', node: 'implement' },
  budget_n: 25,
  branch: 'autome/checkout-flow',
  created_at: '2026-09-15T12:25:00Z',
  completed_at: null,
  merge_commit: null,
  archived: false,
};

const TASK_APPROVE = {
  id: 'T-016',
  slug: 'member-points',
  title: '会员积分',
  request: '给会员加积分体系，下单返积分，积分可抵扣。',
  state: { state: 'active', node: 'await_design_approval' },
  budget_n: null,
  branch: 'autome/member-points',
  created_at: '2026-09-15T11:02:00Z',
  completed_at: null,
  merge_commit: null,
  archived: false,
};

const TASK_MERGE = {
  id: 'T-013',
  slug: 'search-suggest',
  title: '商品搜索联想',
  request: '搜索框加联想。',
  state: { state: 'active', node: 'await_merge' },
  budget_n: 20,
  branch: 'autome/search-suggest',
  created_at: '2026-09-15T09:40:00Z',
  completed_at: null,
  merge_commit: null,
  archived: false,
};

const TASK_FAILED = {
  id: 'T-009',
  slug: 'config-hot-reload',
  title: '配置文件热加载',
  request: '配置文件改动后热加载，不重启进程。',
  state: {
    state: 'failed',
    at: 'implement',
    reason: { kind: 'impl_budget', limit: 20 },
  },
  budget_n: 20,
  branch: 'autome/config-hot-reload',
  created_at: '2026-09-14T15:00:00Z',
  completed_at: null,
  merge_commit: null,
  archived: false,
};

const TASK_QUEUED = {
  id: 'T-017',
  slug: 'refund-flow',
  title: '退款流程',
  request: '退款流程支持部分退款。',
  state: { state: 'queued' },
  budget_n: null,
  branch: 'autome/refund-flow',
  created_at: '2026-09-15T14:20:00Z',
  completed_at: null,
  merge_commit: null,
  archived: false,
};

const TASK_DONE = {
  id: 'T-012',
  slug: 'order-export',
  title: '订单导出 CSV',
  request: '订单页加导出 CSV。',
  state: { state: 'done' },
  budget_n: 15,
  branch: 'autome/order-export',
  created_at: '2026-09-13T10:00:00Z',
  completed_at: '2026-09-14T16:20:00Z',
  merge_commit: 'a1b2c3d',
  archived: false,
};

const TASK_ARCHIVED = {
  id: 'T-010',
  slug: 'coupon-center',
  title: '优惠券中心',
  request: '做一个优惠券中心。',
  state: { state: 'done' },
  budget_n: 15,
  branch: 'autome/coupon-center',
  created_at: '2026-09-07T10:00:00Z',
  completed_at: '2026-09-08T18:00:00Z',
  merge_commit: '5c6d7e8',
  archived: true,
};

const SESSION_RUNNING = {
  id: 'ses_01',
  kind: { kind: 'role', role: 'impl' },
  label: '实现',
  runtime: 'claude',
  model: 'claude-opus-5',
  effort: 'high',
  skills: ['island-shop-conventions'],
  round: 4,
  started_at: '2026-09-15T14:02:00Z',
  ended_at: null,
  lifecycle: { state: 'running' },
  running: true,
};

const SESSION_DONE = {
  id: 'ses_02',
  kind: { kind: 'role', role: 'audit' },
  label: '审计',
  runtime: 'codex',
  model: 'gpt-5.4',
  effort: 'high',
  skills: ['vitest-runner'],
  round: 3,
  started_at: '2026-09-15T13:48:00Z',
  ended_at: '2026-09-15T13:59:00Z',
  lifecycle: { state: 'exited', exit_code: 0 },
  running: false,
};

const RESOLVED_CONFIG = {
  loop_defaults: { parallel: 3, design_rounds: 15, budget_factor: 5 },
  loop_provenance: { parallel: 'global', design_rounds: 'global', budget_factor: 'global' },
  roles: [
    {
      role: 'plan',
      config: { enabled: true, runtime: 'claude', model: 'claude-opus-5', effort: 'high', skills: ['island-shop-conventions'] },
      provenance: 'global',
    },
    {
      role: 'review',
      config: { enabled: true, runtime: 'codex', model: 'gpt-5.4', effort: 'high', skills: [] },
      provenance: 'global',
    },
    {
      role: 'adjudicate',
      config: { enabled: true, runtime: 'claude', model: 'claude-opus-5', effort: null, skills: [] },
      provenance: 'global',
    },
    {
      role: 'impl',
      config: { enabled: true, runtime: 'claude', model: 'claude-opus-5', effort: 'high', skills: ['island-shop-conventions'] },
      provenance: 'project',
    },
    {
      // Deliberately identical to `impl`: this is the C-06 collision, so every
      // screen that renders the routing has to survive a violation being present.
      role: 'audit',
      config: { enabled: true, runtime: 'claude', model: 'claude-opus-5', effort: 'high', skills: ['vitest-runner'] },
      provenance: 'global',
    },
  ],
};

const SAME_MODEL_VIOLATION = {
  kind: 'same_model',
  evaluator: 'audit',
  generator: 'impl',
  identity: 'claude:claude-opus-5',
};

const SKILLS = [
  {
    name: 'island-shop-conventions',
    sources: [
      { runtime: 'claude', scope: 'project', path: '/Users/x/code/island-shop/.claude/skills/island-shop-conventions' },
      { runtime: 'codex', scope: 'project', path: '/Users/x/code/island-shop/.agents/skills/island-shop-conventions' },
    ],
    visible_to: ['claude', 'codex'],
    project_scoped: true,
    bound_roles: ['plan', 'impl'],
    conflicts: [],
  },
  {
    name: 'vitest-runner',
    sources: [{ runtime: 'codex', scope: 'global', path: '/Users/x/.agents/skills/vitest-runner' }],
    visible_to: ['codex'],
    project_scoped: false,
    // Bound to 审计, which the fixture配置 runs on Claude Code — the S-04 conflict.
    bound_roles: ['audit'],
    conflicts: ['audit'],
  },
];

const PROJECT = {
  id: 'prj_island',
  path: '/Users/x/code/island-shop',
  display_name: '岛屿商店',
  default_branch: 'main',
  parallel_limit: 3,
  onboarding: { onboarding: 'completed' },
  disposition: 'existing_repo',
  added_at: '2026-09-01T09:00:00Z',
};

const PROJECT_ONBOARDING = {
  id: 'prj_coral',
  path: '/Users/x/code/coral-notes',
  display_name: '珊瑚笔记',
  default_branch: 'main',
  parallel_limit: 3,
  onboarding: { onboarding: 'in_progress', step: 3 },
  disposition: 'created',
  added_at: '2026-09-15T08:00:00Z',
};

const ENVIRONMENT = {
  components: [
    {
      component: 'git',
      present: true,
      version: '2.47.1',
      path: '/usr/bin/git',
      login: { login: 'not_applicable' },
      checked_at: '2026-09-15T14:31:00Z',
    },
    {
      component: 'claude',
      present: true,
      version: '2.1.14',
      path: '/opt/homebrew/bin/claude',
      login: { login: 'ok', account_hint: '4f…9a' },
      checked_at: '2026-09-15T14:31:00Z',
    },
    {
      component: 'codex',
      present: true,
      version: '0.58.0',
      path: '/opt/homebrew/bin/codex',
      login: { login: 'expired' },
      checked_at: '2026-09-15T14:31:00Z',
    },
    {
      component: 'iterm2',
      present: false,
      version: null,
      path: null,
      login: { login: 'not_applicable' },
      checked_at: '2026-09-15T14:31:00Z',
    },
  ],
  checked_at: '2026-09-15T14:31:00Z',
};

const FIXTURES = {
  dash: {
    waiting: [
      {
        project_id: PROJECT.id,
        project_name: PROJECT.display_name,
        task: TASK_APPROVE,
        pending_decisions: { included: 0, ruled: 1 },
        session: null,
      },
      {
        project_id: PROJECT.id,
        project_name: PROJECT.display_name,
        task: TASK_MERGE,
        pending_decisions: { included: 0, ruled: 0 },
        session: null,
      },
      {
        project_id: 'prj_seabreeze',
        project_name: '海风 CLI',
        task: TASK_FAILED,
        pending_decisions: { included: 0, ruled: 0 },
        session: null,
      },
    ],
    running: [
      {
        project_id: PROJECT.id,
        project_name: PROJECT.display_name,
        task: TASK_RUNNING,
        pending_decisions: { included: 1, ruled: 0 },
        session: SESSION_RUNNING,
      },
      {
        project_id: PROJECT.id,
        project_name: PROJECT.display_name,
        task: { ...TASK_RUNNING, id: 'T-018', title: '订单页骨架屏', state: { state: 'active', node: 'review' } },
        pending_decisions: { included: 0, ruled: 0 },
        session: { ...SESSION_DONE, running: true, lifecycle: { state: 'running' }, ended_at: null },
      },
    ],
    environment: {
      severity: 'blocking',
      problems: [
        { component: 'codex', name: 'Codex', present: true, login: { login: 'expired' } },
        { component: 'iterm2', name: 'iTerm2', present: false, login: { login: 'not_applicable' } },
      ],
    },
  },

  projects: {
    projects: [
      {
        project: PROJECT,
        counts: { running: 2, awaiting_user: 2, queued: 1, done: 4 },
        slots_in_use: 3,
      },
      {
        project: PROJECT_ONBOARDING,
        counts: {},
        slots_in_use: 0,
      },
    ],
  },

  project: {
    project: PROJECT,
    tasks: [TASK_RUNNING, TASK_APPROVE, TASK_MERGE, TASK_QUEUED, TASK_FAILED, TASK_DONE, TASK_ARCHIVED],
    counts: { running: 2, awaiting_user: 2, queued: 1, failed: 1, done: 2 },
    config: RESOLVED_CONFIG,
    violations: [SAME_MODEL_VIOLATION],
    rules: ['AGENTS.md', '.autome/rules/commits.md'],
    skills: SKILLS,
  },

  task: {
    task: TASK_RUNNING,
    project: { id: PROJECT.id, name: PROJECT.display_name, default_branch: 'main' },
    queue_position: null,
    status_block: {
      status: 'implementing',
      design_round: 3,
      design_round_limit: 15,
      impl_round: 4,
      impl_round_limit: 25,
      current_milestone: 'M-03',
      current_milestone_reopens: 1,
      convergence_mode: 'normal',
      next_action: '补 promo.spec.ts 大小写用例，跑 vitest src/checkout/promo',
      milestones: [
        { id: 'M-01', state: 'done', title: '购物车数据模型', reopen_count: 0, reopen_domains: [] },
        { id: 'M-02', state: 'done', title: '结算接口', reopen_count: 0, reopen_domains: [] },
        { id: 'M-03', state: 'pending', title: '优惠码校验', reopen_count: 1, reopen_domains: ['promo-case'] },
        { id: 'M-04', state: 'open', title: '库存不足阻止下单', reopen_count: 0, reopen_domains: [] },
        { id: 'M-05', state: 'open', title: '确认邮件', reopen_count: 0, reopen_domains: [] },
      ],
      milestones_done: 2,
      milestones_total: 5,
    },
    status_error: false,
    sessions: [SESSION_RUNNING, SESSION_DONE],
    decisions: [
      {
        kind: 'dispute',
        item_id: 'D3-P02',
        text: '优惠码是否大小写敏感',
        disposition: 'none',
        ruling: null,
        consumed: false,
      },
      {
        kind: 'backlog',
        item_id: 'B-01',
        text: '优惠码使用次数上限',
        disposition: 'include',
        ruling: null,
        consumed: false,
      },
    ],
    pending_decisions: { included: 1, ruled: 0 },
    documents: [
      { name: 'checkout-flow.md', label: '设计文档', path: 'docs/checkout-flow/checkout-flow.md', absolute: '/tmp/checkout-flow.md', size: 8123 },
      { name: 'retro.md', label: '运行记录', path: 'docs/checkout-flow/retro.md', absolute: '/tmp/retro.md', size: 921 },
    ],
    worktree: '/Users/x/code/island-shop/.worktree/checkout-flow',
    next_role: 'audit',
    changes: null,
  },

  settings: {
    global: {
      loop_defaults: { parallel: 3, design_rounds: 15, budget_factor: 5 },
      roles: {},
    },
    resolved: RESOLVED_CONFIG,
    violations: [SAME_MODEL_VIOLATION],
    scope: null,
  },

  routing: {
    global: { loop_defaults: { parallel: 3, design_rounds: 15, budget_factor: 5 }, roles: {} },
    resolved: RESOLVED_CONFIG,
    violations: [SAME_MODEL_VIOLATION],
    scope: null,
    skills: SKILLS,
    projectId: null,
    projectName: null,
  },

  env: {
    environment: ENVIRONMENT,
    severity: 'blocking',
    problems: ['codex', 'iterm2'],
    can_run_anything: false,
    prerequisites: { brew: true, npm: true },
    config: { resolved: RESOLVED_CONFIG, violations: [SAME_MODEL_VIOLATION] },
  },

  skills: {
    skills: SKILLS,
    scope: null,
    config: { resolved: RESOLVED_CONFIG, violations: [SAME_MODEL_VIOLATION] },
    projectId: null,
    projectName: null,
  },
};

// The merge stop, used on its own to exercise the stopping panel's third face.
const TASK_AT_MERGE = {
  ...FIXTURES.task,
  task: TASK_MERGE,
  status_block: { ...FIXTURES.task.status_block, milestones_done: 5, status: 'done' },
  changes: {
    available: true,
    branch: 'autome/search-suggest',
    into: 'main',
    commits: 3,
    files: [
      { path: 'src/search/suggest.ts', added: 120, deleted: 4 },
      { path: 'src/search/suggest.spec.ts', added: 83, deleted: 0 },
    ],
    total_added: 203,
    total_deleted: 18,
    subjects: ['feat(search): suggest endpoint', 'test(search): suggest cases'],
    mergeable: false,
    blocked_by: { kind: 'dirty_worktree', paths: ['src/app.ts'] },
  },
};

const TASK_FAILED_PANEL = { ...FIXTURES.task, task: TASK_FAILED, changes: null };
const TASK_APPROVE_PANEL = { ...FIXTURES.task, task: TASK_APPROVE, changes: null };

module.exports = {
  FIXTURES,
  TASK_AT_MERGE,
  TASK_FAILED_PANEL,
  TASK_APPROVE_PANEL,
  SCREEN_IDS: ['dash', 'projects', 'project', 'task', 'routing', 'settings', 'env', 'skills'],
};
