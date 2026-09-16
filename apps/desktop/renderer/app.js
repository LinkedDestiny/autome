// Router and bootstrap.
//
// Eight screens, one host element, no history stack: navigation is
// `navigate(id, params)` and every screen re-reads from the core when it is
// shown. There is no client-side cache of business state, and that is the
// point — the core is the only place it lives (plan D3), so the shortest path
// to a wrong screen would be a projection here that drifted from it.
//
// Refresh is event-driven but not event-carried: `onEvent` only ever says
// *something changed*, and the response is to re-run the current screen's
// `load()`. Patching the current render from an event payload would mean two
// code paths producing a screen, one of them exercised only in production.

import { h, clear, icon, text } from './lib/dom.js';
import {
  isConnected, onConnectionChange, setConnected, onEvent, onCoreStatus,
  readOr, resetWriteControls,
} from './lib/api.js';
import { notify } from './lib/notify.js';
import { installOverlayHandlers, closeForNavigation } from './lib/overlay.js';

import * as dashboard from './screens/dashboard.js';
import * as projects from './screens/projects.js';
import * as project from './screens/project.js';
import * as task from './screens/task.js';
import * as routing from './screens/routing.js';
import * as settings from './screens/settings.js';
import * as env from './screens/env.js';
import * as skills from './screens/skills.js';

/** U-01's sidebar, in the design document's order. */
const NAV_ITEMS = [
  { nav: 'dash', label: '仪表盘', icon: 'island', screen: 'dash' },
  { nav: 'projects', label: '项目', icon: 'git', screen: 'projects' },
  { nav: 'env', label: '本地环境', icon: 'plug', screen: 'env' },
  { nav: 'skills', label: '技能', icon: 'bag', screen: 'skills' },
  { nav: 'settings', label: '全局设置', icon: 'gear', screen: 'settings' },
];

export const SCREENS = {
  dash: dashboard,
  projects,
  project,
  task,
  routing,
  settings,
  env,
  skills,
};

let currentScreen = 'dash';
let currentParams = {};
let refreshQueued = false;
// Guards against two refreshes racing: the later render must win, and the
// earlier one must not paint over it after its read finally returns.
let renderToken = 0;

function mainHost() {
  return document.getElementById('main');
}

export function navigate(screenId, params) {
  if (!SCREENS[screenId]) {
    notify('error', '打不开这个界面', `未知的界面 ${screenId}`);
    return Promise.resolve();
  }
  closeForNavigation();
  currentScreen = screenId;
  currentParams = params || {};
  renderNav();
  return show();
}

export function refresh() {
  return show();
}

/** The object every screen's `load`/`render` receives. Deliberately small. */
function context() {
  return {
    params: currentParams,
    screen: currentScreen,
    navigate,
    refresh,
    connected: isConnected(),
  };
}

async function show() {
  const host = mainHost();
  if (!host) return;
  const screen = SCREENS[currentScreen];
  const token = ++renderToken;
  const ctx = context();

  let data = null;
  let failure = null;
  try {
    data = await screen.load(ctx);
  } catch (err) {
    failure = err;
  }
  if (token !== renderToken) return;

  resetWriteControls();
  clear(host);
  host.appendChild(connectionBanner());

  if (failure) {
    // A screen that cannot read cannot honestly render. Saying so, with the
    // reason, beats an empty layout that looks like "there is nothing here".
    host.appendChild(loadFailure(screen, failure));
  } else {
    try {
      screen.render(host, data, ctx);
    } catch (err) {
      host.appendChild(loadFailure(screen, err));
      // A render bug is ours, not the core's; make it loud rather than silent.
      notify('error', '这个界面渲染失败', String((err && err.message) || err));
    }
  }

  await refreshChrome();
}

function loadFailure(screen, err) {
  // Keeps `data-screen` equal to the screen id even when the read failed, so
  // "which screen is up" is one question with one answer.
  const wrap = h('div.screen.active', { 'data-screen': screen.id, 'data-state': 'error' });
  wrap.appendChild(
    h('div.pagehead', [
      h('h1.ribbon.ribbon--brown', [h('span.ribbon__front', { text: '读不到' })]),
      h('span.pagehead__sub', { text: '这一屏需要的数据没有拿到，所以什么都不显示，而不是显示旧的。' }),
    ])
  );
  wrap.appendChild(h('div.empty', { text: String((err && err.message) || err) }));
  wrap.appendChild(
    h('div.row.mt-12', [
      h('button.btn', { type: 'button', onClick: () => refresh() }, [text('重试')]),
    ])
  );
  return wrap;
}

// ---------------------------------------------------------------------------
// Window chrome: the sidebar, the exec bar and the core pill
// ---------------------------------------------------------------------------

function renderNav() {
  const nav = document.getElementById('nav');
  if (!nav) return;
  const active = SCREENS[currentScreen] ? SCREENS[currentScreen].nav : currentScreen;
  clear(nav);
  for (const item of NAV_ITEMS) {
    const button = h('button.nav__item', {
      type: 'button',
      dataset: { nav: item.nav },
      class: item.nav === active ? 'active' : '',
      onClick: () => navigate(item.screen),
    });
    button.appendChild(icon(item.icon));
    button.appendChild(text(item.label));
    nav.appendChild(button);
  }
  applyBadges();
}

let lastCounts = { running: 0, waiting: 0, env: 0 };

function applyBadges() {
  const nav = document.getElementById('nav');
  if (!nav) return;
  for (const button of nav.querySelectorAll('.nav__item')) {
    const existing = button.querySelector('.nav__badge');
    if (existing) existing.remove();
    const key = button.dataset.nav;
    const count = key === 'dash' ? lastCounts.waiting : key === 'env' ? lastCounts.env : 0;
    if (count > 0) {
      button.appendChild(h('span.nav__badge', { text: String(count) }));
    }
  }
}

/**
 * U-01's top bar counts and the recent-project list. Read separately from the
 * screen so the chrome is right regardless of which screen is up; when the
 * read fails the counts are cleared rather than frozen, because a stale "4
 * 等待我" is a claim we can no longer support.
 */
async function refreshChrome() {
  const dashboardData = await readOr(null, 'dashboard');
  if (dashboardData) {
    lastCounts = {
      running: (dashboardData.running || []).length,
      waiting: (dashboardData.waiting || []).length,
      env: ((dashboardData.environment || {}).problems || []).length,
    };
  } else {
    lastCounts = { running: 0, waiting: 0, env: 0 };
  }
  renderExecBar(Boolean(dashboardData));
  applyBadges();
  renderCorePill();

  const list = await readOr(null, 'listProjects');
  renderRecent(list ? list.projects || [] : []);
}

function renderExecBar(observed) {
  const bar = document.getElementById('exec-bar');
  if (!bar) return;
  clear(bar);
  if (!observed) {
    bar.appendChild(h('span.corepill', { text: '还没有读到任何状态' }));
    return;
  }

  const running = h('button.exec.exec--run', {
    type: 'button',
    title: '正在运行的任务',
    onClick: () => navigate('dash'),
  });
  running.appendChild(h('span.exec__k', [h('span.pulse'), text('运行中')]));
  running.appendChild(h('span.exec__v.num', { text: String(lastCounts.running) }));

  const waiting = h('button.exec.exec--wait', {
    type: 'button',
    title: '需要我操作的事项',
    onClick: () => navigate('dash'),
  });
  waiting.appendChild(h('span.exec__k', { text: '等待我' }));
  waiting.appendChild(h('span.exec__v.num', { text: String(lastCounts.waiting) }));

  bar.appendChild(running);
  bar.appendChild(waiting);
}

function renderCorePill() {
  const label = document.getElementById('core-pill-text');
  if (!label) return;
  label.textContent = isConnected()
    ? `${lastCounts.running} 个任务运行中 · 后台`
    : '内核不可达';
}

function renderRecent(projectEntries) {
  const host = document.getElementById('recent');
  if (!host) return;
  clear(host);
  const palette = ['var(--tile-blue)', 'var(--tile-teal)', 'var(--tile-orange)', 'var(--tile-pink)'];
  const recent = projectEntries.slice(0, 4);
  if (!recent.length) {
    host.appendChild(h('div.small.muted', { text: '还没有项目' }));
    return;
  }
  recent.forEach((entry, index) => {
    const project0 = entry.project || {};
    const button = h('button.recent__item', {
      type: 'button',
      onClick: () => navigate('project', { projectId: project0.id }),
    });
    const swatch = h('span.sw');
    swatch.style.setProperty('background', palette[index % palette.length]);
    button.appendChild(swatch);
    button.appendChild(text(project0.display_name || project0.id));
    host.appendChild(button);
  });
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

/**
 * The offline banner. `body.offline` also drives the design's own dimming of
 * write affordances, but the real gate is `registerWrite` setting `disabled`
 * — CSS `pointer-events: none` is a look, not a lock.
 */
function connectionBanner() {
  const banner = h('div.offline-banner', [
    icon('warn'),
    text('内核不可达：显示的是最后一次读到的内容，所有写操作已禁用。'),
  ]);
  banner.appendChild(
    h('button.btn.btn--sm.ml-auto', { type: 'button', onClick: () => refresh() }, [text('重试')])
  );
  return banner;
}

function applyConnection(connected) {
  document.body.classList.toggle('offline', !connected);
  renderCorePill();
}

// ---------------------------------------------------------------------------
// Bootstrap
// ---------------------------------------------------------------------------

/** Coalesces a burst of events into one refresh (the core ticks every 3s). */
function queueRefresh() {
  if (refreshQueued) return;
  refreshQueued = true;
  setTimeout(() => {
    refreshQueued = false;
    refresh();
  }, 120);
}

export function start() {
  installOverlayHandlers();
  applyConnection(isConnected());
  onConnectionChange(applyConnection);

  onEvent(() => queueRefresh());
  onCoreStatus((status) => {
    const connected = Boolean(status && status.connected);
    setConnected(connected);
    if (connected) {
      notify('info', '内核已恢复', status && status.restarted ? '自动重启完成，正在重新读取。' : undefined);
      queueRefresh();
    } else {
      notify('warning', '内核已断开', '写操作已禁用，正在等待自动重启。');
    }
  });

  renderNav();
  return navigate('dash');
}

if (typeof document !== 'undefined' && !window.__AUTOME_NO_AUTOSTART__) {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
}
