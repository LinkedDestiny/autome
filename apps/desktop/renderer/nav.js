'use strict';

// Plan §9 top-level navigation: pure state/logic, framework-agnostic so it
// can be unit-tested with node:test (see apps/desktop/test/nav.test.js)
// without a browser, and loaded as a plain <script> in the renderer without
// a bundler. Depends on vocabulary.js/axis.js/status.js for the panel
// content this file builds — loaded via require() under node:test, and via
// the global namespace when loaded as a <script> (index.html must load
// vocabulary.js, axis.js and status.js before this file).
(function (factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.AutomeNav = api;
  }
})(function () {
  const vocab = typeof require === 'function' ? require('./vocabulary') : globalThis.AutomeVocabulary;
  const axisLib = typeof require === 'function' ? require('./axis') : globalThis.AutomeAxis;
  const statusLib = typeof require === 'function' ? require('./status') : globalThis.AutomeStatus;

  // §9: "顶层主导航固定为四项：项目 / 本地环境 / 技能市场 / 全局设置".
  const TABS = Object.freeze([
    { id: 'project', label: '项目' },
    { id: 'environment', label: '本地环境' },
    { id: 'skills', label: '技能市场' },
    { id: 'settings', label: '全局设置' },
  ]);

  // §9: "项目...默认入口".
  const DEFAULT_TAB_ID = 'project';

  // §9.3: "无 Project 时主 CTA 是"创建新产品项目"或"导入已有仓库"，不是直接
  // 输入一句话。"
  const NO_PROJECT_CTAS = Object.freeze(['创建新产品项目', '导入已有仓库']);

  // §8.2: kinds paired 1:1 by index with NO_PROJECT_CTAS above -- kept as
  // its own constant rather than folded into NO_PROJECT_CTAS's shape so
  // the existing `deepEqual(panel.ctas, ['创建新产品项目', '导入已有仓库'])`
  // assertion (nav.test.js, and the plan's own "断言口径不得稀释" rule)
  // keeps holding unchanged.
  const NO_PROJECT_CTA_KINDS = Object.freeze(['new_product', 'existing_repository']);

  // §9: 项目五子页签. Only reachable once a Project exists; the top-level
  // "project" tab currently always shows the no-project empty state (see
  // panelFor below), so nothing in this renderer navigates into these yet.
  // They exist as pure, independently-tested structure + vocabulary so the
  // shape is right the day project.* read IPC lands.
  const PROJECT_SUBTABS = Object.freeze([
    { id: 'overview', label: '概览' },
    { id: 'tasks', label: '任务' },
    { id: 'config', label: '项目配置' },
    { id: 'environment-skills', label: '环境与技能' },
    { id: 'history-evidence', label: '历史与证据' },
  ]);
  const DEFAULT_PROJECT_SUBTAB_ID = 'overview';

  // §9.1: nine-column identity/route matrix. Identity column must never
  // collapse or show only "Codex/Claude" when multiple installations or
  // accounts exist (plan Context: 反折叠硬规则).
  const PROJECT_CONFIG_MATRIX_COLUMNS = Object.freeze([
    '步骤', 'CLI 安装身份', 'Provider/账号', '模型', 'Effort', '人工审计', 'Skills', '配置来源', '状态',
  ]);
  // §9.1: the verifier/delivery rows are fixed and not overridable by a
  // Project even though every other row is project-configurable.
  const PROJECT_CONFIG_FIXED_ROWS = Object.freeze([
    { id: 'verifier', zh: 'verifier', overridable: false },
    { id: 'delivery', zh: 'delivery', overridable: false },
  ]);

  function initialState() {
    return {
      activeTabId: DEFAULT_TAB_ID,
      // Three-state, not boolean: `null` = not yet observed (no read IPC
      // has resolved), `[]` = observed and confirmed empty, non-empty =
      // real projects. Collapsing the first two would make "still
      // connecting" indistinguishable from "confirmed no Project" (plan
      // §5.1 manual check 5: "两者必须能一眼区分").
      projects: null,
      selectedProjectId: null,
      activeProjectSubtabId: null,
      // §8.2: whether the last-known read from Core succeeded. Gates the
      // no-project CTAs and projectDraftSubmittable -- §9.3's "任何断线
      // 缓存……写操作一律禁用" -- and defaults false so the very first
      // synchronous render (before any IPC has resolved) shows disabled
      // CTAs, same as a confirmed disconnect.
      connected: false,
      // §8.2: null when no create/import flow is in progress. Shape once
      // started: { kind, target, displayName, destinationName,
      // trustConfirmed, submitting, error }.
      projectDraft: null,
    };
  }

  function isKnownTab(tabId) {
    return TABS.some((tab) => tab.id === tabId);
  }

  function isKnownProjectSubtab(subtabId) {
    return PROJECT_SUBTABS.some((tab) => tab.id === subtabId);
  }

  function selectTab(state, tabId) {
    if (!isKnownTab(tabId)) {
      throw new Error(`unknown tab id: ${tabId}`);
    }
    return Object.assign({}, state, { activeTabId: tabId });
  }

  // Replaces the project list wholesale (a fresh `project.list` read).
  // Drops a stale selection if the previously-selected id no longer
  // appears in the new list, rather than leaving `panelFor` to find a
  // dangling reference.
  function setProjects(state, projects) {
    if (!Array.isArray(projects)) {
      throw new TypeError('projects must be an array');
    }
    const stillSelected =
      state.selectedProjectId != null && projects.some((p) => p.id === state.selectedProjectId);
    return Object.assign({}, state, {
      projects,
      selectedProjectId: stillSelected ? state.selectedProjectId : null,
      activeProjectSubtabId: stillSelected ? state.activeProjectSubtabId : null,
    });
  }

  function selectProject(state, projectId) {
    if (!Array.isArray(state.projects) || !state.projects.some((p) => p.id === projectId)) {
      throw new Error(`unknown project id: ${projectId}`);
    }
    return Object.assign({}, state, {
      selectedProjectId: projectId,
      activeProjectSubtabId: DEFAULT_PROJECT_SUBTAB_ID,
    });
  }

  function deselectProject(state) {
    return Object.assign({}, state, { selectedProjectId: null, activeProjectSubtabId: null });
  }

  function selectProjectSubtab(state, subtabId) {
    if (!isKnownProjectSubtab(subtabId)) {
      throw new Error(`unknown project subtab id: ${subtabId}`);
    }
    if (state.selectedProjectId == null) {
      throw new Error('cannot select a project subtab with no project selected');
    }
    return Object.assign({}, state, { activeProjectSubtabId: subtabId });
  }

  // ---------------------------------------------------------------------
  // §8.2 project draft: the two-phase target-registration write flow.
  // Pure reducers, zero DOM -- see preload.js's `automeWrite` bridge and
  // app.js's projectDraftPanel rendering for the IPC/DOM sides.
  // ---------------------------------------------------------------------

  function isKnownProjectDraftKind(kind) {
    return NO_PROJECT_CTA_KINDS.includes(kind);
  }

  function setConnectionStatus(state, connected) {
    if (typeof connected !== 'boolean') {
      throw new TypeError('connected must be a boolean');
    }
    return Object.assign({}, state, { connected });
  }

  function beginProjectDraft(state, kind) {
    if (!isKnownProjectDraftKind(kind)) {
      throw new Error(`unknown project draft kind: ${kind}`);
    }
    return Object.assign({}, state, {
      projectDraft: {
        kind,
        target: null,
        displayName: '',
        destinationName: '',
        trustConfirmed: false,
        submitting: false,
        error: null,
      },
    });
  }

  function requireActiveDraft(state) {
    if (!state.projectDraft) {
      throw new Error('no project draft is in progress');
    }
    return state.projectDraft;
  }

  // `result` is exactly Main's `pickProjectTarget` resolution once the
  // user has actually chosen a directory: `{ target_id, summary }`. A
  // `{ cancelled: true }` result must never reach this reducer -- app.js
  // checks `result.cancelled` itself and leaves the draft untouched.
  function setProjectDraftTarget(state, result) {
    const draft = requireActiveDraft(state);
    if (!result || typeof result !== 'object' || typeof result.target_id !== 'string' || !result.summary) {
      throw new TypeError('setProjectDraftTarget requires a { target_id, summary } result');
    }
    return Object.assign({}, state, {
      projectDraft: Object.assign({}, draft, { target: result, error: null }),
    });
  }

  const PROJECT_DRAFT_TEXT_FIELDS = Object.freeze(['displayName', 'destinationName']);

  function setProjectDraftField(state, field, value) {
    const draft = requireActiveDraft(state);
    if (field === 'trustConfirmed') {
      if (typeof value !== 'boolean') {
        throw new TypeError('trustConfirmed must be a boolean');
      }
    } else if (PROJECT_DRAFT_TEXT_FIELDS.includes(field)) {
      if (typeof value !== 'string') {
        throw new TypeError(`${field} must be a string`);
      }
    } else {
      throw new Error(`unknown project draft field: ${field}`);
    }
    return Object.assign({}, state, {
      projectDraft: Object.assign({}, draft, { [field]: value, error: null }),
    });
  }

  function setProjectDraftSubmitting(state, submitting) {
    const draft = requireActiveDraft(state);
    if (typeof submitting !== 'boolean') {
      throw new TypeError('submitting must be a boolean');
    }
    return Object.assign({}, state, {
      projectDraft: Object.assign({}, draft, { submitting }),
    });
  }

  function setProjectDraftError(state, message) {
    const draft = requireActiveDraft(state);
    if (typeof message !== 'string' || message.length === 0) {
      throw new TypeError('message must be a non-empty string');
    }
    return Object.assign({}, state, {
      projectDraft: Object.assign({}, draft, { submitting: false, error: message }),
    });
  }

  function cancelProjectDraft(state) {
    return Object.assign({}, state, { projectDraft: null });
  }

  // §8.2 client-side mirror of write-gate.js's `looksLikeAPath`-style rule.
  // UX-only fast feedback -- write-gate.js and Core's `locator_for` remain
  // the authoritative last line of defense and are not relaxed by this
  // duplication existing.
  function isValidDestinationName(name) {
    if (typeof name !== 'string') return false;
    const trimmed = name.trim();
    if (trimmed.length === 0 || trimmed.length > 255) return false;
    if (trimmed === '.' || trimmed === '..') return false;
    if (/[/\\]/.test(trimmed)) return false;
    if (trimmed.startsWith('~')) return false;
    if (trimmed.includes('\u0000')) return false;
    return true;
  }

  // Only `existing_repository`'s three probed facts are creation blockers
  // -- project.rs's `locator_for` only raises NotAGitRepository /
  // UnbornOrUnresolvableHead / DirtyWorktree on that path. A `new_product`
  // target's parent directory can be anything at all; a non-git parent is
  // the ordinary case, never a rejection.
  function targetHasKnownProblem(kind, target) {
    if (kind !== 'existing_repository' || !target || !target.summary) {
      return false;
    }
    const s = target.summary;
    return s.is_git_repo !== true || s.head_resolvable !== true || s.worktree_clean !== true;
  }

  // The single source of truth for "can this draft be submitted right
  // now" -- app.js only ever reads this, never re-derives it.
  function projectDraftSubmittable(state) {
    const blockers = [];
    if (state.connected !== true) {
      blockers.push('Core 未连接，写操作已禁用');
    }
    const draft = state.projectDraft;
    if (!draft) {
      blockers.push('尚未开始创建流程');
      return { ok: false, blockers };
    }
    if (!draft.target) {
      blockers.push('尚未选择目标目录');
    } else if (targetHasKnownProblem(draft.kind, draft.target)) {
      blockers.push('所选目标不满足创建条件，已被拒绝');
    }
    if (draft.displayName.trim().length === 0) {
      blockers.push('请输入项目名称');
    }
    if (draft.kind === 'existing_repository' && draft.trustConfirmed !== true) {
      blockers.push('需要勾选“信任此仓库”');
    }
    if (draft.kind === 'new_product' && !isValidDestinationName(draft.destinationName)) {
      blockers.push('文件夹名称不合法');
    }
    if (draft.submitting) {
      blockers.push('正在创建中');
    }
    return { ok: blockers.length === 0, blockers };
  }

  // §8.2: canonical_path is plain information, never a pass/fail chip.
  // The three probed booleans only matter -- and are only shown -- for
  // `existing_repository`; see targetHasKnownProblem's doc comment for why
  // a `new_product` parent directory never renders them at all.
  function targetSummaryCells(kind, summary) {
    const cells = [{ shape: 'text', zh: '路径', text: summary.canonical_path }];
    if (kind === 'existing_repository') {
      cells.push(
        {
          shape: 'chip',
          zh: '是否 Git 仓库',
          chip: statusLib.gateChip(true, summary.is_git_repo === true, summary.is_git_repo ? '是' : '否'),
        },
        {
          shape: 'chip',
          zh: 'HEAD 可解析',
          chip: statusLib.gateChip(true, summary.head_resolvable === true, summary.head_resolvable ? '是' : '否'),
        },
        {
          shape: 'chip',
          zh: '工作树干净',
          chip: statusLib.gateChip(true, summary.worktree_clean === true, summary.worktree_clean ? '是' : '否'),
        }
      );
    }
    return cells;
  }

  function projectDraftPanel(state) {
    const draft = requireActiveDraft(state);
    const submit = projectDraftSubmittable(state);
    return {
      kind: 'project-draft',
      draftKind: draft.kind,
      heading: draft.kind === 'new_product' ? '创建新产品项目' : '导入已有仓库',
      pickLabel: draft.kind === 'new_product' ? '选择父目录' : '选择仓库目录',
      target: draft.target,
      targetSummaryCells: draft.target ? targetSummaryCells(draft.kind, draft.target.summary) : null,
      displayName: draft.displayName,
      destinationName: draft.destinationName,
      trustConfirmed: draft.trustConfirmed,
      submitting: draft.submitting,
      error: draft.error,
      submittable: submit.ok,
      blockers: submit.blockers,
    };
  }

  // §5.1: `project.list`'s phase/hold strings are Rust's `format!("{:?}")`
  // output, matched against vocabulary.js by exact id. A value this
  // renderer's vocabulary does not (yet) know about must degrade to
  // unobserved, never reach axis.js:buildAxisStrip — that throws
  // RangeError on an unknown id, which would take the whole screen down
  // over a single stale/drifted field.
  function knownAxisValues(axes, rawValues) {
    const result = {};
    if (!rawValues) return result;
    for (const axis of axes) {
      const value = rawValues[axis.id];
      if (value != null && (axis.values || []).some((candidate) => candidate.id === value)) {
        result[axis.id] = value;
      }
    }
    return result;
  }

  function projectSummaryAxes(project) {
    const axes = [
      { id: 'phase', zh: 'Project 阶段', values: vocab.PROJECT_PHASES },
      { id: 'hold', zh: 'Project 等待', values: vocab.PROJECT_HOLDS },
    ];
    return axisLib.buildAxisStrip(axes, knownAxisValues(axes, { phase: project.phase, hold: project.hold }));
  }

  function projectListPanel(projects) {
    return {
      kind: 'project-list',
      items: projects.map((p) => ({
        id: p.id,
        displayName: p.display_name || '未登记身份（历史数据）',
        kind: p.kind || null,
        axes: projectSummaryAxes(p),
      })),
    };
  }

  // §9: only Core-computed counts may be shown, never an Agent-reported
  // percentage — this formatter structurally cannot produce a percentage.
  function formatVerifiedRequirementsRatio(verifiedCount, mustTotalCount) {
    if (!Number.isInteger(verifiedCount) || !Number.isInteger(mustTotalCount)) {
      throw new TypeError('verifiedCount and mustTotalCount must be integers');
    }
    return `${verifiedCount}/${mustTotalCount}`;
  }

  // 概览: ProjectPhase 8-state / ProjectHold 7-state axis strip. No project
  // read IPC exists, so every axis renders as not-yet-observed.
  function projectOverviewPanel() {
    return {
      kind: 'vocabulary-display',
      subtabId: 'overview',
      title: '概览',
      axes: axisLib.buildAxisStrip(
        [
          { id: 'phase', zh: 'Project 阶段', values: vocab.PROJECT_PHASES },
          { id: 'hold', zh: 'Project 等待', values: vocab.PROJECT_HOLDS },
        ],
        {}
      ),
    };
  }

  // 任务: Run 三轴轴带 (phase x hold x terminal, 从不压成一个徽章) + 17 步
  // 线性阶段轨 (RUN_LINEAR_NOMINAL_PATH，跳过 Repairing/Replanning 分支).
  function projectTasksPanel() {
    const phaseById = new Map(vocab.RUN_PHASES.map((p) => [p.id, p]));
    return {
      kind: 'vocabulary-display',
      subtabId: 'tasks',
      title: '任务',
      runAxes: axisLib.buildAxisStrip(
        [
          { id: 'phase', zh: 'Run 阶段', values: vocab.RUN_PHASES },
          { id: 'hold', zh: 'Run 等待', values: vocab.RUN_HOLDS },
          { id: 'terminal', zh: 'Run 终态', values: vocab.RUN_TERMINALS },
        ],
        {}
      ),
      linearPhaseRail: vocab.RUN_LINEAR_NOMINAL_PATH.map((phaseId) => ({
        id: phaseId,
        zh: phaseById.get(phaseId).zh,
        chip: statusLib.statusChip('unobserved'),
      })),
    };
  }

  // 项目配置: §9.1 九列矩阵表头 + 固定不可覆盖的 verifier/delivery 行.
  function projectConfigPanel() {
    return {
      kind: 'vocabulary-display',
      subtabId: 'config',
      title: '项目配置',
      matrixColumns: PROJECT_CONFIG_MATRIX_COLUMNS,
      fixedRows: PROJECT_CONFIG_FIXED_ROWS,
      identityColumnCollapsible: false,
    };
  }

  // 环境与技能 (per-project): not one of this increment's four built-out
  // subtabs — kept as an honest placeholder rather than a fake table.
  function projectEnvironmentSkillsPanel() {
    return {
      kind: 'empty-state',
      subtabId: 'environment-skills',
      title: '环境与技能',
      reason: '尚未接入按项目的环境/技能读取 IPC，本增量暂不填充此子页签内容。',
    };
  }

  // 历史与证据: CompletionGate 40 门清单（全部未观测）+ 证据表结构.
  function projectHistoryEvidencePanel() {
    return {
      kind: 'vocabulary-display',
      subtabId: 'history-evidence',
      title: '历史与证据',
      completionGates: vocab.COMPLETION_GATES.map((gate) => ({
        id: gate.id,
        zh: gate.zh,
        chip: statusLib.gateChip(false, false),
      })),
      evidenceTableColumns: Object.freeze(['实现位置', '环境', '时间', '结果', '制品']),
    };
  }

  function projectSubtabPanelFor(subtabId) {
    switch (subtabId) {
      case 'overview':
        return projectOverviewPanel();
      case 'tasks':
        return projectTasksPanel();
      case 'config':
        return projectConfigPanel();
      case 'environment-skills':
        return projectEnvironmentSkillsPanel();
      case 'history-evidence':
        return projectHistoryEvidencePanel();
      default:
        throw new Error(`unknown project subtab id: ${subtabId}`);
    }
  }

  // 本地环境 Tab: no environment read IPC exists yet, so this is a designed
  // empty state carrying the real 5-axis vocabulary and requirement-class
  // severity table rather than a bare "not yet implemented" placeholder.
  function environmentTabPanel() {
    return {
      kind: 'empty-state',
      tabId: 'environment',
      title: '本地环境',
      reason: '尚未接入 SupportedEnvironmentProfile 的读取 IPC，暂时无法展示真实组件状态。',
      axesVocabulary: vocab.ENVIRONMENT_AXES,
      readinessVocabulary: vocab.READINESS_VALUES,
      requirementClasses: vocab.REQUIREMENT_CLASSES,
    };
  }

  // 技能市场 Tab: same treatment, carrying the 6-rung evidence ladder and
  // the four sub-tab names named in §9.2 even though none has content yet.
  function skillsTabPanel() {
    return {
      kind: 'empty-state',
      tabId: 'skills',
      title: '技能市场',
      reason: '尚未接入 Skill 的读取 IPC，暂时无法展示已安装或可用的技能列表。',
      evidenceLadder: vocab.SKILL_EVIDENCE_LEVELS,
      subtabs: Object.freeze(['市场', '已安装', '更新', '隔离区']),
    };
  }

  // 全局设置 Tab: no global config read/write IPC exists yet.
  function settingsTabPanel() {
    return {
      kind: 'empty-state',
      tabId: 'settings',
      title: '全局设置',
      reason: '尚未接入全局配置的读取或写入 IPC。',
    };
  }

  // §5.1: `projects` is three-state (`null` unobserved / `[]` confirmed
  // empty / non-empty). `null` and `[]` share this exact panel shape — the
  // dom-harness's very first synchronous render happens before any IPC has
  // resolved, so `observed`/`heading` are the only fields allowed to
  // differ between "still connecting" and "confirmed no Project" (plan
  // manual check 5: "两者必须能一眼区分", not "两者渲染不同结构").
  function noProjectEmptyStatePanel(observed, connected) {
    return {
      kind: 'no-project-empty-state',
      observed,
      connected: connected === true,
      heading: observed ? '尚无 Project。' : '尚未从 Core 取得项目列表。',
      ctas: NO_PROJECT_CTAS,
      // §8.2: paired 1:1 by index with `ctas` -- app.js zips them to know
      // which kind each CTA button starts a draft with.
      ctaKinds: NO_PROJECT_CTA_KINDS,
      taskPhaseRail: projectTasksPanel().linearPhaseRail,
      completionGates: projectHistoryEvidencePanel().completionGates,
    };
  }

  function panelFor(state) {
    if (state.activeTabId === 'project') {
      // A draft in progress takes over the project tab regardless of the
      // projects list's own state -- it can be mid-flow while a concurrent
      // list refresh lands.
      if (state.projectDraft) {
        return projectDraftPanel(state);
      }
      if (!Array.isArray(state.projects)) {
        return noProjectEmptyStatePanel(false, state.connected);
      }
      if (state.projects.length === 0) {
        return noProjectEmptyStatePanel(true, state.connected);
      }
      if (state.selectedProjectId == null) {
        return projectListPanel(state.projects);
      }
      const project = state.projects.find((p) => p.id === state.selectedProjectId);
      if (!project) {
        return projectListPanel(state.projects);
      }
      const subtabId = isKnownProjectSubtab(state.activeProjectSubtabId)
        ? state.activeProjectSubtabId
        : DEFAULT_PROJECT_SUBTAB_ID;
      return {
        kind: 'project-detail',
        project: {
          id: project.id,
          displayName: project.display_name || '未登记身份（历史数据）',
          axes: projectSummaryAxes(project),
        },
        subtabId,
        subtabPanel: projectSubtabPanelFor(subtabId),
      };
    }
    if (state.activeTabId === 'environment') {
      return environmentTabPanel();
    }
    if (state.activeTabId === 'skills') {
      return skillsTabPanel();
    }
    if (state.activeTabId === 'settings') {
      return settingsTabPanel();
    }
    throw new Error(`unknown tab id: ${state.activeTabId}`);
  }

  return {
    TABS,
    DEFAULT_TAB_ID,
    NO_PROJECT_CTAS,
    NO_PROJECT_CTA_KINDS,
    PROJECT_SUBTABS,
    DEFAULT_PROJECT_SUBTAB_ID,
    PROJECT_CONFIG_MATRIX_COLUMNS,
    PROJECT_CONFIG_FIXED_ROWS,
    initialState,
    selectTab,
    isKnownTab,
    isKnownProjectSubtab,
    setProjects,
    selectProject,
    deselectProject,
    selectProjectSubtab,
    panelFor,
    projectSubtabPanelFor,
    projectListPanel,
    formatVerifiedRequirementsRatio,
    setConnectionStatus,
    isKnownProjectDraftKind,
    beginProjectDraft,
    setProjectDraftTarget,
    setProjectDraftField,
    setProjectDraftSubmitting,
    setProjectDraftError,
    cancelProjectDraft,
    isValidDestinationName,
    projectDraftSubmittable,
    projectDraftPanel,
  };
});
