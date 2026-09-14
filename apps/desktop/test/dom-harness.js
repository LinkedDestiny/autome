'use strict';

// Electron main-process harness for real-DOM assertions (plan §"测试":
// "不引入 Playwright...Electron 自带 Chromium，测试就跑在应用真正使用的
// 那个内核上"). Opens a hidden BrowserWindow, registers the real
// `autome://` protocol via src/app-protocol.js, loads the real
// index.html/app.js/nav.js, drives it with webContents.executeJavaScript()
// against the real DOM, and prints one JSON array of {name, pass, detail}
// to stdout before exiting. No sidecar is started — these assertions only
// touch the renderer, never automed or the user's real DB.
//
// Not a *.test.js file itself: it must run under the `electron` binary
// (it needs app/BrowserWindow), not under plain `node --test`. See
// dom.test.js, which spawns this file as a child process and asserts on
// its JSON output.

const { app, BrowserWindow } = require('electron');
const appProtocol = require('../src/app-protocol');

appProtocol.registerSchemeAsPrivileged();

const results = [];
const consoleMessages = [];

function check(name, pass, detail) {
  results.push({ name, pass: Boolean(pass), detail: detail === undefined ? null : detail });
}

async function run() {
  appProtocol.registerAppProtocol();

  const win = new BrowserWindow({
    show: false,
    width: 1100,
    height: 720,
    webPreferences: {
      preload: require('node:path').join(__dirname, '..', 'preload.js'),
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      webSecurity: true,
      webviewTag: false,
    },
  });

  win.webContents.on('console-message', (_event, _level, message) => {
    consoleMessages.push(message);
  });

  await win.loadURL('autome://app/index.html');

  const evaluate = (fn) => win.webContents.executeJavaScript(`(${fn.toString()})()`);

  const execBarVisible = await evaluate(() => {
    const bar = document.getElementById('exec-bar');
    return Boolean(bar && bar.querySelector('.ac-exec-bar'));
  });
  check('exec bar renders with no data (never hidden)', execBarVisible);

  const writeDisabled = await evaluate(() => {
    const bar = document.getElementById('exec-bar');
    return Boolean(bar && bar.querySelector('.ac-exec-bar__write-disabled'));
  });
  check('exec bar shows write-disabled when disconnected', writeDisabled);

  const tabCount = await evaluate(() => document.querySelectorAll('#top-nav .ac-tab').length);
  check('all 4 top-level tabs render', tabCount === 4, tabCount);

  const ctaState = await evaluate(() => {
    const buttons = Array.from(document.querySelectorAll('.ac-cta-row .ac-btn'));
    return { count: buttons.length, allDisabled: buttons.every((b) => b.disabled) };
  });
  check('the two no-project CTAs render and are both disabled', ctaState.count === 2 && ctaState.allDisabled, ctaState);

  // §8.2: contextBridge exposes only named functions, never a generic
  // invoke -- checked directly on the bridge shape, not inferred from
  // behavior, so a future accidental widening (e.g. adding `invoke`
  // alongside the two named functions) fails loudly here.
  const automeWriteShape = await evaluate(() => {
    const aw = window.automeWrite;
    if (!aw || typeof aw !== 'object') return null;
    return {
      keys: Object.keys(aw).sort(),
      pickIsFn: typeof aw.pickProjectTarget === 'function',
      createIsFn: typeof aw.createProject === 'function',
      hasInvoke: typeof aw.invoke === 'function',
    };
  });
  check(
    'automeWrite bridge exposes exactly pickProjectTarget and createProject, never a generic invoke',
    automeWriteShape !== null &&
      automeWriteShape.keys.length === 2 &&
      automeWriteShape.keys.join(',') === 'createProject,pickProjectTarget' &&
      automeWriteShape.pickIsFn &&
      automeWriteShape.createIsFn &&
      !automeWriteShape.hasInvoke,
    automeWriteShape
  );

  // §9.3: a disconnected write affordance must say so, not fail silently.
  // This harness never connects (no sidecar, no ipcMain.handle('autome:write',
  // ...)), so the no-project CTAs above stay disabled forever -- the reason
  // for that must be visibly rendered text, not merely an invisible title
  // attribute a user would have to hover to find.
  const ctaDisabledReason = await evaluate(() => {
    const reason = document.querySelector('.ac-cta-row__reason');
    return reason ? reason.textContent : null;
  });
  check(
    'disconnected no-project CTAs render an explicit disabled reason, not silently',
    ctaDisabledReason === 'Core 不可达，写操作已禁用',
    ctaDisabledReason
  );

  const railCount = await evaluate(() => document.querySelectorAll('.ac-phase-rail__step').length);
  check('the 17-step task phase rail renders in full', railCount === 17, railCount);

  const gateCount = await evaluate(() => document.querySelectorAll('.ac-gate-list__item').length);
  check('the 40-gate completion checklist renders in full', gateCount === 40, gateCount);

  const allUnobservedOnFirstLoad = await evaluate(() => {
    const chips = Array.from(document.querySelectorAll('.ac-chip'));
    return chips.length > 0 && chips.every((c) => !c.classList.contains('ac-chip--ok'));
  });
  check('no chip on the zero-data no-project screen claims tone "ok"', allUnobservedOnFirstLoad);

  const unobservedIsDashed = await evaluate(() => {
    const chips = Array.from(document.querySelectorAll('.ac-chip--unobserved'));
    return chips.length > 0 && chips.every((c) => c.classList.contains('ac-chip--dashed'));
  });
  check('every unobserved chip carries the dashed shape cue, not color alone', unobservedIsDashed);

  // This harness never starts a sidecar or registers `ipcMain.handle`
  // ('autome:read', ...) — unlike the real app, so the preload's
  // `automeRead` bridge is present (preload always runs) but every call
  // through it rejects with "No handler registered". app.js's async
  // refresh must treat "absent" and "present-but-rejecting" identically:
  // both fall back to the default fully-unobserved render. Waits one
  // macrotask so the rejected getQueue() promise chain has settled before
  // asserting nothing changed.
  const stillUnobservedDespiteBridgePresent = await evaluate(() => {
    return new Promise((resolve) => {
      setTimeout(() => {
        const hasBridge = typeof window.automeRead === 'object' && typeof window.automeRead.getQueue === 'function';
        const chips = Array.from(document.querySelectorAll('.ac-chip'));
        const noneOk = chips.length > 0 && chips.every((c) => !c.classList.contains('ac-chip--ok'));
        const barStillRenders = Boolean(document.getElementById('exec-bar').querySelector('.ac-exec-bar'));
        resolve(hasBridge && noneOk && barStillRenders);
      }, 50);
    });
  });
  check(
    'automeRead bridge is present but unanswered in this harness, so the screen still renders fully unobserved (absence and rejection fall back identically)',
    stillUnobservedDespiteBridgePresent
  );

  // §5.1: pins the precondition check #7 above depends on. If a future
  // change ever registers `ipcMain.handle('autome:read', ...)` in this
  // harness, `listProjects()` would resolve instead of reject, `projects`
  // would leave `null`, and the no-project screen (with its "no chip is
  // ok" assertion) could silently stop being what's on screen after the
  // async refresh runs. Asserting the rejection directly, rather than only
  // its downstream effect, makes that change loud instead of silent.
  const listProjectsRejectsInThisHarness = await evaluate(() => {
    return new Promise((resolve) => {
      window.automeRead
        .listProjects()
        .then(() => resolve(false))
        .catch(() => resolve(true));
    });
  });
  check(
    'listProjects() rejects in this harness (no ipcMain handler registered), which is why projects stays null and check #7 still holds',
    listProjectsRejectsInThisHarness
  );

  const environmentPanel = await evaluate(() => {
    const before = Array.from(document.querySelectorAll('#top-nav .ac-tab')).find((b) => b.textContent === '本地环境');
    before.click();
    // renderNav() replaces #top-nav's children on every render, so the
    // pre-click node reference is now detached — re-query after the click.
    const after = Array.from(document.querySelectorAll('#top-nav .ac-tab')).find((b) => b.textContent === '本地环境');
    return {
      active: after.classList.contains('ac-tab--active'),
      axisCells: document.querySelectorAll('.ac-axis-vocab__axis').length,
      reason: (document.querySelector('.ac-empty-reason') || {}).textContent || '',
    };
  });
  check(
    'clicking 本地环境 switches tabs and renders the 5-axis vocabulary with a non-empty reason',
    environmentPanel.active && environmentPanel.axisCells === 5 && environmentPanel.reason.length > 0,
    environmentPanel
  );

  const cspViolations = consoleMessages.filter((m) => /content security policy|refused to/i.test(m));
  check('no CSP violations logged to the console', cspViolations.length === 0, cspViolations);

  process.stdout.write(JSON.stringify(results));
  app.exit(0);
}

app.whenReady().then(run).catch((err) => {
  process.stderr.write(String((err && err.stack) || err));
  app.exit(1);
});
