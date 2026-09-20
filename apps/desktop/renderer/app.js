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
//
// What that left out is that "something changed" is frequently false for the
// screen you are looking at, and rebuilding it anyway is visible: every card
// carries a staggered fade-up, so a repaint makes the whole page re-assemble.
// So the render is still all-or-nothing and still has one code path, but it
// only runs when the data it would render actually differs from what is on
// screen. The comparison is on the payload the core returned, not on a guess
// about which event affects which screen.

import { h, clear, icon, text } from './lib/dom.js';
import {
  isConnected, onConnectionChange, setConnected, onEvent, onCoreStatus,
  readOr, resetWriteControls, attempt,
} from './lib/api.js';
import { notify } from './lib/notify.js';
import * as theme from './lib/theme.js';
import { installOverlayHandlers, closeForNavigation } from './lib/overlay.js';

import * as dashboard from './screens/dashboard.js';
import * as projects from './screens/projects.js';
import * as project from './screens/project.js';
import * as task from './screens/task.js';
import * as routing from './screens/routing.js';
import * as protocol from './screens/protocol.js';
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
  protocol,
  settings,
  env,
  skills,
};

let currentScreen = 'dash';
let currentParams = {};
let refreshQueued = false;
// What is currently painted, as the exact bytes that produced it. `null` means
// nothing has been painted yet.
let paintedSignature = null;
let chromeSignature = null;
let enterTimer = null;
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
  // Arriving at a screen is when its entry animation belongs. A refresh of the
  // screen you are already on is not an arrival, and replaying the stagger
  // there is the flicker.
  return show({ entering: true });
}

export function refresh() {
  return show({ entering: false });
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

async function show({ entering = false } = {}) {
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

  // Everything the render is a function of. If none of it moved, the DOM we
  // would build is the DOM already there, and replacing it only costs the user
  // their scroll position, their hover, and a screen-wide re-animation.
  const signature = renderSignature(data, failure);
  if (!entering && signature === paintedSignature && host.firstChild) {
    await refreshChrome();
    return;
  }
  // A failed load is never memoised. The error block it paints carries a 重试
  // button that calls `refresh()`; if the second attempt failed the same way,
  // the signature would match and the button would do nothing at all.
  paintedSignature = failure ? null : signature;

  markEntering(host, entering);
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

/**
 * A stable string for everything the current render depends on. The payload
 * goes in verbatim: the core serialises structs, so equal state produces equal
 * bytes — verified against the live core, whose `dashboard.get` and
 * `project.list` answers are byte-identical between ticks when nothing moved.
 */
function renderSignature(data, failure) {
  return JSON.stringify({
    screen: currentScreen,
    params: currentParams,
    connected: isConnected(),
    failure: failure ? String((failure && failure.message) || failure) : null,
    data,
  });
}

/**
 * Entry animations are scoped to `#main.entering` in the stylesheet, so they
 * run when you arrive at a screen and not when it refreshes underneath you.
 * The class is removed on a timer because the elements it applies to are built
 * fresh on the next render, and a class left behind would animate them.
 */
function markEntering(host, entering) {
  if (enterTimer) {
    clearTimeout(enterTimer);
    enterTimer = null;
  }
  host.classList.toggle('entering', Boolean(entering));
  if (entering) {
    enterTimer = setTimeout(() => {
      enterTimer = null;
      host.classList.remove('entering');
    }, ENTER_ANIMATION_WINDOW_MS);
  }
}

// Long enough for the last staggered card (`--i` × 60ms + .35s) on a full
// screen, short enough that a refresh arriving afterwards does not animate.
const ENTER_ANIMATION_WINDOW_MS = 1400;

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

  const list = await readOr(null, 'listProjects');
  const projectEntries = list ? list.projects || [] : [];

  // Same rule as the screen: rebuilding the sidebar and the exec bar every
  // three seconds throws away whatever the pointer was hovering, for a result
  // that is usually identical.
  const signature = JSON.stringify({
    counts: lastCounts,
    observed: Boolean(dashboardData),
    connected: isConnected(),
    recent: projectEntries
      .slice(0, 4)
      .map((e) => [(e.project || {}).id, (e.project || {}).display_name]),
  });
  if (signature === chromeSignature) return;
  chromeSignature = signature;

  renderExecBar(Boolean(dashboardData));
  applyBadges();
  renderCorePill();
  renderRecent(projectEntries);
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
 * Why the core is not running, once Main has stopped restarting it, or `null`
 * while a restart is still coming. The distinction is the whole point: a
 * core that is being restarted is a blink the user can wait out, and one that
 * has failed to start three times is a fact they have to act on.
 */
let coreFailure = null;
let bannerMessage = null;
let bannerButton = null;

/**
 * The offline banner. `body.offline` also drives the design's own dimming of
 * write affordances, but the real gate is `registerWrite` setting `disabled`
 * — CSS `pointer-events: none` is a look, not a lock.
 */
function connectionBanner() {
  bannerMessage = h('span');
  bannerButton = h('button.btn.btn--sm.ml-auto', {
    type: 'button',
    onClick: () => retryCore(),
  });
  const banner = h('div.offline-banner', [icon('warn'), bannerMessage, bannerButton]);
  paintConnectionBanner();
  return banner;
}

/**
 * Writes the current failure into the banner already on screen. The banner is
 * built once per render, but the core can die between renders — and the
 * moment it does is exactly when its reason is worth reading.
 */
function paintConnectionBanner() {
  if (!bannerMessage || !bannerButton) return;
  if (coreFailure) {
    bannerMessage.textContent = `内核起不来：${coreFailure}`;
    bannerButton.textContent = '重启内核';
  } else {
    bannerMessage.textContent = '内核不可达：显示的是最后一次读到的内容，所有写操作已禁用。';
    bannerButton.textContent = '重试';
  }
}

/**
 * The banner's button. While a restart is still expected it retries the read;
 * once Main has given up, only a new core will help, so it asks for one — and
 * the answer is the core's own sentence either way.
 */
async function retryCore() {
  if (!coreFailure) return refresh();
  const outcome = await attempt({
    label: '重启内核',
    run: (write) => write.restartCore(),
    success: '内核已经起来了',
  });
  if (!outcome.ok) return;
  coreFailure = null;
  paintConnectionBanner();
  await refresh();
}

function applyConnection(connected) {
  document.body.classList.toggle('offline', !connected);
  paintConnectionBanner();
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
  theme.watchSystem();
  // Painted from the core's answer as soon as one arrives. Until then the
  // stylesheet's own default (light) stands: guessing dark and correcting a
  // moment later is a flash, and the core is one read away.
  readOr(null, 'getConfig').then((config) => {
    if (config) theme.apply(((config.global || {}).ui || {}).theme);
  });
  applyConnection(isConnected());
  onConnectionChange(applyConnection);

  onEvent(() => queueRefresh());
  onCoreStatus((status) => {
    const connected = Boolean(status && status.connected);
    setConnected(connected);
    if (connected) {
      coreFailure = null;
      paintConnectionBanner();
      notify('info', '内核已恢复', status && status.restarted ? '自动重启完成，正在重新读取。' : undefined);
      queueRefresh();
      return;
    }
    // `fatal` means Main has stopped restarting it: retrying on a timer would
    // only repeat a failure that is not going to resolve itself, so the
    // reason goes on screen and the next attempt is the user's.
    if (status && status.fatal) {
      coreFailure = status.reason || `内核连续 ${status.attempts || 0} 次启动失败，没有留下原因`;
      paintConnectionBanner();
      notify('error', '内核起不来', `${coreFailure} 处理之后点「重启内核」。`);
      return;
    }
    coreFailure = null;
    paintConnectionBanner();
    notify('warning', '内核已断开', '写操作已禁用，正在等待自动重启。');
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
