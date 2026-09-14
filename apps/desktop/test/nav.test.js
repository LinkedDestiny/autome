'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const nav = require('../renderer/nav');
const vocab = require('../renderer/vocabulary');

test('the four top-level tabs match plan §9 exactly, in order', () => {
  assert.deepEqual(
    nav.TABS.map((tab) => tab.id),
    ['project', 'environment', 'skills', 'settings']
  );
  assert.deepEqual(
    nav.TABS.map((tab) => tab.label),
    ['项目', '本地环境', '技能市场', '全局设置']
  );
});

test('the default tab is project, per §9 "默认入口"', () => {
  assert.equal(nav.initialState().activeTabId, 'project');
});

test('selectTab moves to any known tab', () => {
  const state = nav.selectTab(nav.initialState(), 'settings');
  assert.equal(state.activeTabId, 'settings');
});

test('selectTab rejects an unknown tab id instead of silently landing somewhere', () => {
  assert.throws(() => nav.selectTab(nav.initialState(), 'nonexistent'), /unknown tab id/);
});

test('selectTab does not mutate the state object it was given', () => {
  const before = nav.initialState();
  const snapshot = { ...before };
  nav.selectTab(before, 'settings');
  assert.deepEqual(before, snapshot);
});

test('the project tab with no project shows exactly the two §9.3 CTAs', () => {
  const state = nav.setProjects(nav.initialState(), []);
  const panel = nav.panelFor(state);
  assert.equal(panel.kind, 'no-project-empty-state');
  assert.deepEqual(panel.ctas, ['创建新产品项目', '导入已有仓库']);
});

test('the no-project empty state also surfaces the 17-step task rail and 40-gate checklist as reference content', () => {
  const state = nav.setProjects(nav.initialState(), []);
  const panel = nav.panelFor(state);
  assert.equal(panel.taskPhaseRail.length, 17);
  assert.equal(panel.completionGates.length, 40);
});

// Stronger than the two tests above: `projects: null` (not yet observed —
// the very first render, before any IPC has resolved) and `projects: []`
// (observed, confirmed empty) must share the same panel shape (CTAs/rail/
// gates all present either way) but must NOT be indistinguishable — the
// heading is the one field allowed to differ, and "尚无 Project。" is
// reserved for the confirmed-empty case (plan §5.1 manual check 5).
test('projects: null must not render the confirmed-empty heading "尚无 Project。"', () => {
  const unobserved = nav.panelFor(nav.initialState());
  assert.equal(unobserved.kind, 'no-project-empty-state');
  assert.equal(unobserved.observed, false);
  assert.notEqual(unobserved.heading, '尚无 Project。');
  assert.deepEqual(unobserved.ctas, ['创建新产品项目', '导入已有仓库']);
  assert.equal(unobserved.taskPhaseRail.length, 17);
  assert.equal(unobserved.completionGates.length, 40);

  const confirmedEmpty = nav.panelFor(nav.setProjects(nav.initialState(), []));
  assert.equal(confirmedEmpty.observed, true);
  assert.equal(confirmedEmpty.heading, '尚无 Project。');
});

// Semantic upgrade over the original "reports itself as not yet
// implemented" assertion: the original guarantee (this tab never claims
// to be functional) still holds — 'empty-state' is not 'vocabulary-display'
// and carries no fake data — and it is now strictly stronger, since every
// such panel must also explain *why* it is empty.
test('every other tab reports a designed empty state with a non-empty reason, never pretending to be functional', () => {
  for (const tabId of ['environment', 'skills', 'settings']) {
    const panel = nav.panelFor(nav.selectTab(nav.initialState(), tabId));
    assert.equal(panel.kind, 'empty-state');
    assert.equal(panel.tabId, tabId);
    assert.ok(panel.reason && panel.reason.length > 0, `${tabId} empty state has no reason`);
  }
});

test('the environment tab empty state carries the real 5-axis vocabulary, not placeholder text', () => {
  const panel = nav.panelFor(nav.selectTab(nav.initialState(), 'environment'));
  assert.equal(panel.axesVocabulary.length, 5);
  assert.equal(panel.requirementClasses.length, 5);
});

test('the skills tab empty state carries the real 6-rung evidence ladder', () => {
  const panel = nav.panelFor(nav.selectTab(nav.initialState(), 'skills'));
  assert.equal(panel.evidenceLadder.length, 6);
});

test('PROJECT_SUBTABS: the five subtabs from §9, in order, defaulting to overview', () => {
  assert.deepEqual(
    nav.PROJECT_SUBTABS.map((tab) => tab.id),
    ['overview', 'tasks', 'config', 'environment-skills', 'history-evidence']
  );
  assert.equal(nav.DEFAULT_PROJECT_SUBTAB_ID, 'overview');
});

test('projectSubtabPanelFor rejects an unknown subtab id', () => {
  assert.throws(() => nav.projectSubtabPanelFor('nonexistent'), /unknown project subtab id/);
});

test('overview subtab renders Project phase/hold as a 2-cell axis strip, all unobserved', () => {
  const panel = nav.projectSubtabPanelFor('overview');
  assert.equal(panel.axes.length, 2);
  assert.ok(panel.axes.every((cell) => cell.observed === false));
});

test('tasks subtab renders the Run 3-axis strip plus the 17-step linear phase rail', () => {
  const panel = nav.projectSubtabPanelFor('tasks');
  assert.equal(panel.runAxes.length, 3);
  assert.equal(panel.linearPhaseRail.length, 17);
  assert.ok(panel.linearPhaseRail.every((step) => step.chip.tone === 'unobserved'));
});

test('config subtab exposes the §9.1 nine-column matrix and fixed non-overridable rows, identity column never collapsible', () => {
  const panel = nav.projectSubtabPanelFor('config');
  assert.equal(panel.matrixColumns.length, 9);
  assert.deepEqual(panel.fixedRows.map((r) => r.id), ['verifier', 'delivery']);
  assert.ok(panel.fixedRows.every((r) => r.overridable === false));
  assert.equal(panel.identityColumnCollapsible, false);
});

test('history-evidence subtab renders all 40 CompletionGate fields, each unobserved', () => {
  const panel = nav.projectSubtabPanelFor('history-evidence');
  assert.equal(panel.completionGates.length, 40);
  assert.ok(panel.completionGates.every((gate) => gate.chip.tone === 'unobserved'));
  assert.equal(panel.evidenceTableColumns.length, 5);
});

test('formatVerifiedRequirementsRatio never produces a percentage, only an integer ratio', () => {
  assert.equal(nav.formatVerifiedRequirementsRatio(3, 12), '3/12');
  assert.throws(() => nav.formatVerifiedRequirementsRatio(3.5, 12), TypeError);
});

test('vocabulary counts referenced by nav panels stay aligned with the Rust-derived tables', () => {
  assert.equal(vocab.COMPLETION_GATES.length, 40);
  assert.equal(vocab.RUN_LINEAR_NOMINAL_PATH.length, 17);
});

test('setProjects replaces the project list without mutating the state object given', () => {
  const before = nav.initialState();
  const snapshot = JSON.parse(JSON.stringify(before));
  const after = nav.setProjects(before, [{ id: 'p1', display_name: 'Alpha' }]);
  assert.deepEqual(before, snapshot);
  assert.deepEqual(after.projects, [{ id: 'p1', display_name: 'Alpha' }]);
  assert.equal(after.selectedProjectId, null);
});

test('setProjects rejects a non-array', () => {
  assert.throws(() => nav.setProjects(nav.initialState(), null), TypeError);
});

test('setProjects drops a selection that no longer exists in the new list', () => {
  let state = nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]);
  state = nav.selectProject(state, 'p1');
  assert.equal(state.selectedProjectId, 'p1');
  state = nav.setProjects(state, [{ id: 'p2', display_name: 'Beta' }]);
  assert.equal(state.selectedProjectId, null);
  assert.equal(state.activeProjectSubtabId, null);
});

test('setProjects keeps a selection that still exists in the new list', () => {
  let state = nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]);
  state = nav.selectProject(state, 'p1');
  state = nav.setProjects(state, [
    { id: 'p1', display_name: 'Alpha Renamed' },
    { id: 'p2', display_name: 'Beta' },
  ]);
  assert.equal(state.selectedProjectId, 'p1');
  assert.equal(state.activeProjectSubtabId, 'overview');
});

test('selectProject rejects an unknown project id', () => {
  const state = nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]);
  assert.throws(() => nav.selectProject(state, 'nonexistent'), /unknown project id/);
});

test('selectProject does not mutate the state object it was given, and defaults the subtab to overview', () => {
  const before = nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]);
  const snapshot = JSON.parse(JSON.stringify(before));
  const after = nav.selectProject(before, 'p1');
  assert.deepEqual(before, snapshot);
  assert.equal(after.selectedProjectId, 'p1');
  assert.equal(after.activeProjectSubtabId, 'overview');
});

test('deselectProject clears the selection and subtab without mutating the state given', () => {
  const before = nav.selectProject(nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]), 'p1');
  const snapshot = JSON.parse(JSON.stringify(before));
  const after = nav.deselectProject(before);
  assert.deepEqual(before, snapshot);
  assert.equal(after.selectedProjectId, null);
  assert.equal(after.activeProjectSubtabId, null);
});

test('selectProjectSubtab rejects an unknown subtab id', () => {
  const state = nav.selectProject(nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]), 'p1');
  assert.throws(() => nav.selectProjectSubtab(state, 'nonexistent'), /unknown project subtab id/);
});

test('selectProjectSubtab rejects selecting a subtab with no project selected', () => {
  assert.throws(() => nav.selectProjectSubtab(nav.initialState(), 'tasks'), /no project selected/);
});

test('all five project subtabs become reachable through panelFor once a project is selected', () => {
  let state = nav.selectProject(nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]), 'p1');
  for (const tab of nav.PROJECT_SUBTABS) {
    state = nav.selectProjectSubtab(state, tab.id);
    const panel = nav.panelFor(state);
    assert.equal(panel.kind, 'project-detail');
    assert.equal(panel.subtabId, tab.id);
    assert.equal(panel.subtabPanel.subtabId, tab.id);
  }
});

test('panelFor returns a project-list panel with no selection and multiple projects', () => {
  const state = nav.setProjects(nav.initialState(), [
    { id: 'p1', display_name: 'Alpha' },
    { id: 'p2', display_name: 'Beta' },
  ]);
  const panel = nav.panelFor(state);
  assert.equal(panel.kind, 'project-list');
  assert.deepEqual(panel.items.map((i) => i.id), ['p1', 'p2']);
});

// ---------------------------------------------------------------------
// §8.2 project draft: two-phase target-registration write flow reducers.
// ---------------------------------------------------------------------

test('setConnectionStatus flips the connected flag without mutating the state given', () => {
  const before = nav.initialState();
  assert.equal(before.connected, false);
  const snapshot = { ...before };
  const after = nav.setConnectionStatus(before, true);
  assert.equal(after.connected, true);
  assert.deepEqual(before, snapshot);
});

test('setConnectionStatus rejects a non-boolean', () => {
  assert.throws(() => nav.setConnectionStatus(nav.initialState(), 'yes'), /must be a boolean/);
});

test('the no-project panel exposes connected and ctaKinds paired 1:1 with ctas', () => {
  const disconnected = nav.panelFor(nav.setProjects(nav.initialState(), []));
  assert.equal(disconnected.connected, false);
  assert.deepEqual(disconnected.ctaKinds, ['new_product', 'existing_repository']);
  assert.equal(disconnected.ctaKinds.length, disconnected.ctas.length);

  const connectedState = nav.setConnectionStatus(nav.setProjects(nav.initialState(), []), true);
  assert.equal(nav.panelFor(connectedState).connected, true);
});

test('beginProjectDraft rejects an unknown kind instead of silently starting one', () => {
  assert.throws(() => nav.beginProjectDraft(nav.initialState(), 'renovation'), /unknown project draft kind/);
});

test('beginProjectDraft starts a draft with the given kind and all-empty defaults, without mutating the state given', () => {
  const before = nav.initialState();
  const snapshot = { ...before };
  const after = nav.beginProjectDraft(before, 'existing_repository');
  assert.deepEqual(before, snapshot);
  assert.deepEqual(after.projectDraft, {
    kind: 'existing_repository',
    target: null,
    displayName: '',
    destinationName: '',
    trustConfirmed: false,
    submitting: false,
    error: null,
  });
});

test('setProjectDraftTarget, setProjectDraftField, setProjectDraftSubmitting and setProjectDraftError all reject when no draft is in progress', () => {
  const state = nav.initialState();
  assert.throws(() => nav.setProjectDraftTarget(state, { target_id: 't1', summary: {} }), /no project draft/);
  assert.throws(() => nav.setProjectDraftField(state, 'displayName', 'x'), /no project draft/);
  assert.throws(() => nav.setProjectDraftSubmitting(state, true), /no project draft/);
  assert.throws(() => nav.setProjectDraftError(state, 'boom'), /no project draft/);
});

test('setProjectDraftTarget requires a { target_id, summary } shape', () => {
  const state = nav.beginProjectDraft(nav.initialState(), 'new_product');
  assert.throws(() => nav.setProjectDraftTarget(state, null), /target_id, summary/);
  assert.throws(() => nav.setProjectDraftTarget(state, { target_id: 't1' }), /target_id, summary/);
  assert.throws(() => nav.setProjectDraftTarget(state, { summary: {} }), /target_id, summary/);
});

test('setProjectDraftTarget records the target and clears any prior error, without mutating the state given', () => {
  let state = nav.beginProjectDraft(nav.initialState(), 'new_product');
  state = nav.setProjectDraftError(state, 'stale failure');
  const before = state;
  const snapshot = JSON.parse(JSON.stringify(before));
  const target = { target_id: 't1', summary: { kind: 'NewProduct', canonical_path: '/tmp/parent', is_git_repo: false, head_resolvable: false, worktree_clean: false } };
  const after = nav.setProjectDraftTarget(before, target);
  assert.deepEqual(before, snapshot);
  assert.deepEqual(after.projectDraft.target, target);
  assert.equal(after.projectDraft.error, null);
});

test('setProjectDraftField rejects an unknown field', () => {
  const state = nav.beginProjectDraft(nav.initialState(), 'new_product');
  assert.throws(() => nav.setProjectDraftField(state, 'favoriteColor', 'blue'), /unknown project draft field/);
});

test('setProjectDraftField type-checks each known field', () => {
  const state = nav.beginProjectDraft(nav.initialState(), 'new_product');
  assert.throws(() => nav.setProjectDraftField(state, 'displayName', 42), /must be a string/);
  assert.throws(() => nav.setProjectDraftField(state, 'destinationName', 42), /must be a string/);
  assert.throws(() => nav.setProjectDraftField(state, 'trustConfirmed', 'true'), /must be a boolean/);
});

test('setProjectDraftField updates displayName/destinationName/trustConfirmed without mutating the state given', () => {
  const before = nav.beginProjectDraft(nav.initialState(), 'existing_repository');
  const snapshot = JSON.parse(JSON.stringify(before));
  const afterName = nav.setProjectDraftField(before, 'displayName', 'My Repo');
  assert.deepEqual(before, snapshot);
  assert.equal(afterName.projectDraft.displayName, 'My Repo');
  const afterTrust = nav.setProjectDraftField(afterName, 'trustConfirmed', true);
  assert.equal(afterTrust.projectDraft.trustConfirmed, true);
  assert.equal(afterName.projectDraft.trustConfirmed, false, 'earlier snapshot must stay untouched');
});

test('setProjectDraftSubmitting toggles submitting without mutating the state given', () => {
  const before = nav.beginProjectDraft(nav.initialState(), 'new_product');
  const snapshot = JSON.parse(JSON.stringify(before));
  const after = nav.setProjectDraftSubmitting(before, true);
  assert.deepEqual(before, snapshot);
  assert.equal(after.projectDraft.submitting, true);
});

test('setProjectDraftError records a message, clears submitting, and rejects an empty message', () => {
  let state = nav.beginProjectDraft(nav.initialState(), 'new_product');
  state = nav.setProjectDraftSubmitting(state, true);
  const after = nav.setProjectDraftError(state, 'INVALID_PARAMS: destination already exists');
  assert.equal(after.projectDraft.error, 'INVALID_PARAMS: destination already exists');
  assert.equal(after.projectDraft.submitting, false);
  assert.throws(() => nav.setProjectDraftError(state, ''), /non-empty string/);
});

test('cancelProjectDraft clears the draft, and is a no-op on a state with no draft', () => {
  const withDraft = nav.beginProjectDraft(nav.initialState(), 'new_product');
  assert.equal(nav.cancelProjectDraft(withDraft).projectDraft, null);
  assert.equal(nav.cancelProjectDraft(nav.initialState()).projectDraft, null);
});

test('isValidDestinationName accepts an ordinary folder name and rejects every path shape', () => {
  assert.equal(nav.isValidDestinationName('my-new-app'), true);
  assert.equal(nav.isValidDestinationName(''), false);
  assert.equal(nav.isValidDestinationName('   '), false);
  assert.equal(nav.isValidDestinationName('.'), false);
  assert.equal(nav.isValidDestinationName('..'), false);
  assert.equal(nav.isValidDestinationName('a/b'), false);
  assert.equal(nav.isValidDestinationName('a\\b'), false);
  assert.equal(nav.isValidDestinationName('~secret'), false);
  assert.equal(nav.isValidDestinationName('a\u0000b'), false);
  assert.equal(nav.isValidDestinationName('a'.repeat(256)), false);
  assert.equal(nav.isValidDestinationName(42), false);
});

test('projectDraftSubmittable blocks on disconnection even with an otherwise-complete draft', () => {
  let state = nav.beginProjectDraft(nav.initialState(), 'existing_repository');
  state = nav.setProjectDraftTarget(state, {
    target_id: 't1',
    summary: { kind: 'ExistingRepository', canonical_path: '/repo', is_git_repo: true, head_resolvable: true, worktree_clean: true },
  });
  state = nav.setProjectDraftField(state, 'displayName', 'My Repo');
  state = nav.setProjectDraftField(state, 'trustConfirmed', true);
  const result = nav.projectDraftSubmittable(state);
  assert.equal(result.ok, false);
  assert.ok(result.blockers.includes('Core 未连接，写操作已禁用'));
});

test('projectDraftSubmittable reports "no draft in progress" when there is none, even while connected', () => {
  const state = nav.setConnectionStatus(nav.initialState(), true);
  const result = nav.projectDraftSubmittable(state);
  assert.equal(result.ok, false);
  assert.ok(result.blockers.includes('尚未开始创建流程'));
});

test('projectDraftSubmittable: existing_repository blockers accumulate one by one and clear as the draft is completed', () => {
  let state = nav.setConnectionStatus(nav.beginProjectDraft(nav.initialState(), 'existing_repository'), true);

  let result = nav.projectDraftSubmittable(state);
  assert.equal(result.ok, false);
  assert.deepEqual(result.blockers, ['尚未选择目标目录', '请输入项目名称', '需要勾选“信任此仓库”']);

  // A rejected target (not a git repo) blocks even once "selected".
  state = nav.setProjectDraftTarget(state, {
    target_id: 't1',
    summary: { kind: 'ExistingRepository', canonical_path: '/tmp', is_git_repo: false, head_resolvable: false, worktree_clean: false },
  });
  result = nav.projectDraftSubmittable(state);
  assert.ok(result.blockers.includes('所选目标不满足创建条件，已被拒绝'));

  // Swap in a clean target, fill in the rest -- blockers clear one at a time.
  state = nav.setProjectDraftTarget(state, {
    target_id: 't2',
    summary: { kind: 'ExistingRepository', canonical_path: '/repo', is_git_repo: true, head_resolvable: true, worktree_clean: true },
  });
  result = nav.projectDraftSubmittable(state);
  assert.deepEqual(result.blockers, ['请输入项目名称', '需要勾选“信任此仓库”']);

  state = nav.setProjectDraftField(state, 'displayName', 'My Repo');
  result = nav.projectDraftSubmittable(state);
  assert.deepEqual(result.blockers, ['需要勾选“信任此仓库”']);

  state = nav.setProjectDraftField(state, 'trustConfirmed', true);
  result = nav.projectDraftSubmittable(state);
  assert.deepEqual(result, { ok: true, blockers: [] });
});

test('projectDraftSubmittable: new_product never blocks on trust, but does block on an invalid destination name', () => {
  let state = nav.setConnectionStatus(nav.beginProjectDraft(nav.initialState(), 'new_product'), true);
  state = nav.setProjectDraftTarget(state, {
    target_id: 't1',
    // A non-git parent directory is the ordinary case for new_product --
    // must never appear as a blocker.
    summary: { kind: 'NewProduct', canonical_path: '/tmp/parent', is_git_repo: false, head_resolvable: false, worktree_clean: false },
  });
  state = nav.setProjectDraftField(state, 'displayName', 'My New App');

  let result = nav.projectDraftSubmittable(state);
  assert.deepEqual(result.blockers, ['文件夹名称不合法']);

  state = nav.setProjectDraftField(state, 'destinationName', 'my-new-app');
  result = nav.projectDraftSubmittable(state);
  assert.deepEqual(result, { ok: true, blockers: [] });

  state = nav.setProjectDraftField(state, 'destinationName', '../escape');
  result = nav.projectDraftSubmittable(state);
  assert.deepEqual(result.blockers, ['文件夹名称不合法']);
});

test('projectDraftSubmittable blocks while a submission is already in flight', () => {
  let state = nav.setConnectionStatus(nav.beginProjectDraft(nav.initialState(), 'new_product'), true);
  state = nav.setProjectDraftTarget(state, {
    target_id: 't1',
    summary: { kind: 'NewProduct', canonical_path: '/tmp/parent', is_git_repo: false, head_resolvable: false, worktree_clean: false },
  });
  state = nav.setProjectDraftField(state, 'displayName', 'My New App');
  state = nav.setProjectDraftField(state, 'destinationName', 'my-new-app');
  state = nav.setProjectDraftSubmitting(state, true);
  const result = nav.projectDraftSubmittable(state);
  assert.equal(result.ok, false);
  assert.ok(result.blockers.includes('正在创建中'));
});

test('panelFor routes to the project-draft panel whenever a draft is in progress, even with an existing non-empty project list', () => {
  let state = nav.setProjects(nav.initialState(), [{ id: 'p1', display_name: 'Alpha' }]);
  state = nav.beginProjectDraft(state, 'new_product');
  const panel = nav.panelFor(state);
  assert.equal(panel.kind, 'project-draft');
  assert.equal(panel.draftKind, 'new_product');
  assert.equal(panel.heading, '创建新产品项目');
  assert.equal(panel.target, null);
  assert.equal(panel.targetSummaryCells, null);
});

test('projectDraftPanel renders targetSummaryCells for existing_repository (path + three pass/fail chips) but only the path for new_product', () => {
  let existingState = nav.setConnectionStatus(nav.beginProjectDraft(nav.initialState(), 'existing_repository'), true);
  existingState = nav.setProjectDraftTarget(existingState, {
    target_id: 't1',
    summary: { kind: 'ExistingRepository', canonical_path: '/repo', is_git_repo: true, head_resolvable: true, worktree_clean: false },
  });
  const existingPanel = nav.panelFor(existingState);
  assert.equal(existingPanel.targetSummaryCells.length, 4);
  assert.equal(existingPanel.targetSummaryCells[0].shape, 'text');
  assert.equal(existingPanel.targetSummaryCells[0].text, '/repo');
  const worktreeCell = existingPanel.targetSummaryCells.find((c) => c.zh === '工作树干净');
  assert.equal(worktreeCell.chip.tone, 'critical');

  let newProductState = nav.setConnectionStatus(nav.beginProjectDraft(nav.initialState(), 'new_product'), true);
  newProductState = nav.setProjectDraftTarget(newProductState, {
    target_id: 't2',
    summary: { kind: 'NewProduct', canonical_path: '/tmp/parent', is_git_repo: false, head_resolvable: false, worktree_clean: false },
  });
  const newProductPanel = nav.panelFor(newProductState);
  assert.equal(newProductPanel.targetSummaryCells.length, 1);
  assert.equal(newProductPanel.targetSummaryCells[0].shape, 'text');
});
