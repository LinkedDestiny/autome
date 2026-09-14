'use strict';

// Thin DOM glue over nav.js's pure state machine. Not unit-tested itself
// (there is no DOM in node:test); verified by launching the real Electron
// window (`npm start`) and by apps/desktop/test/nav.test.js covering every
// state this file renders.
(function () {
  const nav = globalThis.AutomeNav;
  let state = nav.initialState();

  function renderNav(navEl) {
    navEl.innerHTML = '';
    for (const tab of nav.TABS) {
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = tab.label;
      button.dataset.tabId = tab.id;
      button.setAttribute('aria-pressed', String(tab.id === state.activeTabId));
      button.addEventListener('click', () => {
        state = nav.selectTab(state, tab.id);
        render();
      });
      navEl.appendChild(button);
    }
  }

  function renderPanel(panelEl) {
    panelEl.innerHTML = '';
    const panel = nav.panelFor(state);
    if (panel.kind === 'no-project-empty-state') {
      const heading = document.createElement('p');
      heading.textContent = '尚无 Project。';
      panelEl.appendChild(heading);
      for (const label of panel.ctas) {
        const button = document.createElement('button');
        button.type = 'button';
        button.textContent = label;
        button.disabled = true;
        button.title = '尚未接入 automed 的项目创建 IPC';
        panelEl.appendChild(button);
      }
      return;
    }
    const message = document.createElement('p');
    message.textContent = `${panel.tabId} 尚未实现`;
    panelEl.appendChild(message);
  }

  function render() {
    renderNav(document.getElementById('top-nav'));
    renderPanel(document.getElementById('panel'));
  }

  render();
})();
