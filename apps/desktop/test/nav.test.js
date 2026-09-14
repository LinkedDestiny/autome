'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const nav = require('../renderer/nav');

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
  const panel = nav.panelFor(nav.initialState());
  assert.equal(panel.kind, 'no-project-empty-state');
  assert.deepEqual(panel.ctas, ['创建新产品项目', '导入已有仓库']);
});

test('every other tab reports itself as not yet implemented', () => {
  for (const tabId of ['environment', 'skills', 'settings']) {
    const panel = nav.panelFor(nav.selectTab(nav.initialState(), tabId));
    assert.equal(panel.kind, 'not-yet-implemented');
    assert.equal(panel.tabId, tabId);
  }
});
