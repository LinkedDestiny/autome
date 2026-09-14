'use strict';

// Plan §9 top-level navigation: pure state/logic, framework-agnostic so it
// can be unit-tested with node:test (see apps/desktop/test/nav.test.js)
// without a browser, and loaded as a plain <script> in the renderer without
// a bundler.
(function (factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  if (typeof globalThis !== 'undefined') {
    globalThis.AutomeNav = api;
  }
})(function () {
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

  function initialState() {
    return { activeTabId: DEFAULT_TAB_ID };
  }

  function isKnownTab(tabId) {
    return TABS.some((tab) => tab.id === tabId);
  }

  function selectTab(state, tabId) {
    if (!isKnownTab(tabId)) {
      throw new Error(`unknown tab id: ${tabId}`);
    }
    return Object.assign({}, state, { activeTabId: tabId });
  }

  // There is no `project.list`-style read IPC method yet (automed's
  // project.* surface only advances an already-identified Project through
  // its phases — see crates/automed/src/dispatch.rs), so the only state
  // this renderer can honestly represent right now is "no project". Real
  // project data plugs in here once that read path exists.
  function panelFor(state) {
    if (state.activeTabId === 'project') {
      return { kind: 'no-project-empty-state', ctas: NO_PROJECT_CTAS };
    }
    return { kind: 'not-yet-implemented', tabId: state.activeTabId };
  }

  return {
    TABS,
    DEFAULT_TAB_ID,
    NO_PROJECT_CTAS,
    initialState,
    selectTab,
    isKnownTab,
    panelFor,
  };
});
