'use strict';

// Electron main-process harness for real-DOM assertions.
//
// The renderer is plain ES modules served from the packaged `autome://app`
// origin under a real CSP, and it is laid out against a real font and a real
// stylesheet. None of that survives a simulated DOM, and two of the things
// this suite has to prove are specifically about the real one: that every
// screen fits 1512x944 without vertical scroll (requirement U-11 and the
// non-functional section 5), and that nothing in the renderer trips the CSP.
// So the assertions run under the actual Electron binary, in a hidden window,
// driven through `webContents.executeJavaScript`.
//
// No sidecar is started, so `window.autome.read.*` exists (the preload always
// runs) but every call through it rejects with "No handler registered". That
// is the disconnected case, and the harness leans on it: the first checks
// below are what the app does when the core never came up.
//
// Screens are exercised by importing the module and calling `render()` with a
// fixture, rather than by letting the router read — there is nothing to read
// from. `import()` inside `executeJavaScript` returns the *same* module
// instance the page already loaded, so these are the real modules with their
// real state, not fresh copies.
//
// Not a *.test.js file: it needs `app`/`BrowserWindow`, so it runs under
// `electron`, not `node --test`. See dom.test.js, which spawns it.

const fs = require('node:fs');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const appProtocol = require('../src/app-protocol');
const {
  FIXTURES,
  TASK_AT_MERGE,
  TASK_FAILED_PANEL,
  TASK_APPROVE_PANEL,
  TASK_UNREADABLE_DOC_PANEL,
  TASK_MEASURED_PANEL,
  PROTOCOL_SCREEN,
  SCREEN_IDS,
} = require('./fixtures');

appProtocol.registerSchemeAsPrivileged();

// The design's reference viewport (requirement U-11). `useContentSize` makes
// these the dimensions of the web page, not of the window plus its chrome —
// otherwise the measurement would be of a viewport nobody ships.
const VIEWPORT = { width: 1512, height: 944 };

// The node flow's length, read from the renderer's own table rather than
// written down here. A hard-coded 13 meant that adding the retro round made
// four unrelated assertions fail while saying nothing about what broke.
// Likewise for the role count: the graph draws one box per role, and writing
// the number here again means adding a role fails this test rather than the
// one that would have said what broke.
const ROLE_COUNT = (() => {
  const src = fs.readFileSync(path.join(__dirname, '..', 'renderer/screens/routing.js'), 'utf8');
  const block = src.slice(src.indexOf('const ROLE_NODES'));
  return block.slice(0, block.indexOf('};')).match(/^\s+\w+: \[/gm).length;
})();

const FLOW_STONES = (() => {
  const src = fs.readFileSync(path.join(__dirname, '..', 'renderer/lib/labels.js'), 'utf8');
  const block = src.slice(src.indexOf('export const FLOW'));
  return block.slice(0, block.indexOf('];')).match(/\{ key:/g).length;
})();

const results = [];
const consoleMessages = [];

function check(name, pass, detail) {
  results.push({ name, pass: Boolean(pass), detail: detail === undefined ? null : detail });
}

async function run() {
  appProtocol.registerAppProtocol();

  const win = new BrowserWindow({
    show: false,
    useContentSize: true,
    width: VIEWPORT.width,
    height: VIEWPORT.height,
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload.js'),
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      webSecurity: true,
      webviewTag: false,
      // A hidden window is throttled, and a throttled one hands back stale
      // *used* values: after switching the theme, `getComputedStyle(el)
      // .getPropertyValue('--token')` reported the new colour while
      // `.backgroundColor` on the same element still reported the old one.
      // The measurements below are of used values, so they would be measuring
      // the previous theme.
      backgroundThrottling: false,
    },
  });

  // Electron has moved this signature around; accept either shape.
  win.webContents.on('console-message', (...args) => {
    const first = args[0];
    if (first && typeof first === 'object' && typeof first.message === 'string') {
      consoleMessages.push(first.message);
    } else if (typeof args[2] === 'string') {
      consoleMessages.push(args[2]);
    }
  });

  await win.loadURL('autome://app/index.html');
  await win.webContents.executeJavaScript(fixtureInjector());

  // `executeJavaScript` evaluates a classic script; an async IIFE gives the
  // body `await`, and dynamic `import()` reaches the page's module registry.
  const evaluate = (fn, ...args) =>
    win.webContents.executeJavaScript(
      `(async () => { const ARGS = ${JSON.stringify(args)}; return await (${fn.toString()})(...ARGS); })()`
    );

  // ---- the shell renders at all -----------------------------------------
  const shell = await evaluate(() => ({
    nav: document.querySelectorAll('#nav .nav__item').length,
    main: Boolean(document.getElementById('main')),
    sprite: document.querySelectorAll('.sprite symbol').length,
  }));
  check(
    'the shell renders: five sidebar items, a screen host and the icon sprite',
    shell.nav === 5 && shell.main && shell.sprite > 10,
    shell
  );

  // ---- dark mode actually goes dark --------------------------------------
  // The hazard with a retrofitted theme is a half-dark window: the tokens flip
  // but colours written as literals in individual rules do not, leaving cream
  // cards on a dark page.
  //
  // The discriminator is chroma, not saturation. A cream like #fffbe7 has an
  // HSL saturation near 1.0 because it is a pure tint at high lightness, so a
  // saturation threshold calls it "vivid" and lets it through — the first
  // version of this check reported zero offenders on a stylesheet with
  // twenty-three of them. `max - min` separates a cream (0.09) from the brand
  // teal (0.69) cleanly.
  //
  // Brand accents are supposed to stay bright in the dark: a teal button is
  // still teal at night.
  const darkness = await evaluate(async () => {
    const t = await import('autome://app/lib/theme.js');
    // `color-mix()` computes to `color(srgb 0.22 0.21 0.26)`, whose components
    // are 0..1 — reading those as 0..255 makes every mixed colour look
    // near-black, which is the safe direction and therefore the direction a
    // check silently stops testing in.
    const rgb = (value) => {
      const parts = value.match(/[\d.]+/g).map(Number);
      return value.startsWith('color(')
        ? parts.slice(0, 3).map((c) => c * 255)
        : parts.slice(0, 3);
    };
    const luminance = (c) => (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) / 255;
    const chroma = (c) => (Math.max(...c) - Math.min(...c)) / 255;
    // Transitions are switched off for the measurement rather than waited
    // out. Every attempt to wait was wrong in a different way: the window is
    // hidden so its timers are throttled and `transition: all .25s` takes far
    // longer than 250ms of wall clock; a fixed 450ms read rgb(173,168,153), a
    // colour in neither theme because it is 38% of the way between them; and
    // settling on "two equal reads" returned before the transition had even
    // started. Inserting a rule into the page's own same-origin stylesheet is
    // allowed by the CSP, unlike an injected <style>.
    const sheet = document.styleSheets[0];
    const frozenRule = sheet.insertRule(
      '*, *::before, *::after { transition: none !important; animation: none !important; }',
      sheet.cssRules.length
    );
    const settle = async () => {
      void document.documentElement.offsetHeight;
      await new Promise((r) => setTimeout(r, 60));
    };
    const measure = () => {
      const out = { lightSurfaces: [], sampled: 0 };
      for (const el of document.querySelectorAll('#window *')) {
        const bg = getComputedStyle(el).backgroundColor;
        if (!bg || bg.startsWith('rgba(0, 0, 0, 0)')) continue;
        // A translucent fill takes its colour from what is under it: 3% white
        // over a dark page is dark, however white the declared value looks.
        const parts = bg.match(/[\d.]+/g).map(Number);
        const alpha = bg.startsWith('rgba') || bg.startsWith('color(') ? (parts[3] ?? 1) : 1;
        if (alpha < 0.5) continue;
        const box = el.getBoundingClientRect();
        if (box.width < 24 || box.height < 16) continue;
        out.sampled++;
        const c = rgb(bg);
        if (luminance(c) > 0.6 && chroma(c) < 0.3) {
          out.lightSurfaces.push(`${el.className || el.tagName}:${bg}`);
        }
      }
      return out;
    };
    t.apply('dark');
    await settle();
    const dark = measure();
    const attr = document.documentElement.dataset.theme;
    t.apply('light');
    await settle();
    const light = measure();
    t.apply('system');
    // Put motion back: the next check measures whether an entry animation
    // runs, and a leftover freeze here would answer it for them.
    sheet.deleteRule(frozenRule);
    return { dark, light, attr, systemAttr: document.documentElement.dataset.theme };
  });
  check(
    'dark mode leaves no cream surface behind',
    darkness.attr === 'dark' && darkness.dark.lightSurfaces.length === 0,
    { sampled: darkness.dark.sampled, offenders: darkness.dark.lightSurfaces.slice(0, 8) }
  );
  check(
    'light mode is still cream, so the switch goes both ways',
    darkness.light.lightSurfaces.length > 0,
    { creamSurfaces: darkness.light.lightSurfaces.length }
  );
  check(
    'following the system resolves to a real theme, never to the word "system"',
    darkness.systemAttr === 'light' || darkness.systemAttr === 'dark',
    darkness.systemAttr
  );

  // ---- entry animations do not replay on a refresh -----------------------
  // The screen is rebuilt whenever its data changes, and every card carries a
  // staggered fade-up with `both` fill — so an unscoped rule made the whole
  // page re-assemble from opacity 0 on every repaint. Measured in a real
  // browser because this is a computed-style question, not a source-text one.
  const revealAnimation = await evaluate(() => {
    const main = document.getElementById('main');
    const probe = document.createElement('div');
    probe.className = 'reveal';
    main.appendChild(probe);
    const wasEntering = main.classList.contains('entering');
    main.classList.remove('entering');
    const idle = getComputedStyle(probe).animationName;
    main.classList.add('entering');
    const entering = getComputedStyle(probe).animationName;
    main.classList.toggle('entering', wasEntering);
    probe.remove();
    return { idle, entering };
  });
  check(
    'a card animates when you arrive at a screen and not when it refreshes under you',
    revealAnimation.idle === 'none' && revealAnimation.entering === 'ac-fade-up',
    revealAnimation
  );

  // ---- disconnected: no read ever answered, so no write is offered ------
  await evaluate(() => new Promise((resolve) => setTimeout(resolve, 200)));
  const disconnected = await evaluate(async () => {
    const api = await import('autome://app/lib/api.js');
    return {
      connected: api.isConnected(),
      bodyOffline: document.body.classList.contains('offline'),
      bannerVisible: getComputedStyle(document.querySelector('.offline-banner')).display !== 'none',
    };
  });
  check(
    'with no core answering, the renderer reports disconnected and shows the banner rather than an empty-looking screen',
    disconnected.connected === false && disconnected.bodyOffline && disconnected.bannerVisible,
    disconnected
  );

  // Requirement: never render unverified state as verified — every write
  // control must be genuinely disabled, not merely dimmed by CSS.
  const writesDisabled = await evaluate(async () => {
    const api = await import('autome://app/lib/api.js');
    const projects = await import('autome://app/screens/projects.js');
    const host = document.getElementById('main');
    api.setConnected(false);
    api.resetWriteControls();
    host.replaceChildren();
    projects.render(host, JSON.parse(document.getElementById('fx-projects').textContent), {
      params: {},
      navigate() {},
      refresh() {},
      connected: false,
    });
    const controls = Array.from(host.querySelectorAll('.write'));
    return {
      count: controls.length,
      allDisabled: controls.every(
        (el) => el.disabled === true || el.getAttribute('aria-disabled') === 'true'
      ),
      reasons: controls.every((el) => Boolean(el.title)),
    };
  });
  check(
    'while disconnected every registered write control is actually disabled and says why',
    writesDisabled.count > 0 && writesDisabled.allDisabled && writesDisabled.reasons,
    writesDisabled
  );

  const reconnectEnables = await evaluate(async () => {
    const api = await import('autome://app/lib/api.js');
    api.setConnected(true);
    const controls = Array.from(document.querySelectorAll('#main .write'));
    const allEnabled = controls.every((el) => el.disabled !== true);
    api.setConnected(false);
    api.setConnected(true);
    return { count: controls.length, allEnabled };
  });
  check(
    'a reconnect re-enables the same controls, so the gate is state, not a one-way render',
    reconnectEnables.count > 0 && reconnectEnables.allEnabled,
    reconnectEnables
  );

  // ---- a failed write shows the core's own sentence ----------------------
  const writeFailure = await evaluate(async () => {
    const api = await import('autome://app/lib/api.js');
    document.getElementById('notif-stack').replaceChildren();
    // Exactly how Electron wraps a rejected ipcRenderer.invoke.
    const wrapped = new Error(
      "Error invoking remote method 'autome:write': Error: 主工作树有未提交改动，合并已取消"
    );
    const outcome = await api.attempt({
      label: '合并到 main',
      run: () => Promise.reject(wrapped),
    });
    const notif = document.querySelector('.notif--error');
    return {
      ok: outcome.ok,
      title: notif ? notif.querySelector('.notif__title').textContent : null,
      description: notif ? notif.querySelector('.notif__desc').textContent : null,
    };
  });
  check(
    "a rejected write surfaces a notification carrying the core's message verbatim, and reports failure to the caller",
    writeFailure.ok === false &&
      writeFailure.title === '合并到 main' &&
      writeFailure.description === '主工作树有未提交改动，合并已取消',
    writeFailure
  );

  const writeFailureNotSwallowed = await evaluate(async () => {
    const api = await import('autome://app/lib/api.js');
    document.getElementById('notif-stack').replaceChildren();
    let ran = false;
    const outcome = await api.attempt({
      label: '归档 T-012',
      run: () => Promise.reject(new Error('任务还没有合并，不能归档')),
      onDone: () => {
        ran = true;
      },
    });
    return {
      ok: outcome.ok,
      onDoneRan: ran,
      description: (document.querySelector('.notif--error .notif__desc') || {}).textContent || null,
      errorsPersist: document.querySelectorAll('.notif--error').length,
    };
  });
  check(
    'a failed write does not run its success continuation, and the error notification stays on screen',
    writeFailureNotSwallowed.ok === false &&
      writeFailureNotSwallowed.onDoneRan === false &&
      writeFailureNotSwallowed.description === '任务还没有合并，不能归档' &&
      writeFailureNotSwallowed.errorsPersist === 1,
    writeFailureNotSwallowed
  );

  // ---- every screen renders from a representative payload ---------------
  for (const id of SCREEN_IDS) {
    const outcome = await evaluate(async (screenId) => {
      const api = await import('autome://app/lib/api.js');
      api.setConnected(true);
      api.resetWriteControls();
      const module = await import(`autome://app/screens/${screenId === 'dash' ? 'dashboard' : screenId}.js`);
      const host = document.getElementById('main');
      host.replaceChildren();
      const data = JSON.parse(document.getElementById(`fx-${screenId}`).textContent);
      try {
        module.render(host, data, {
          params: { projectId: 'prj_island', taskId: 'T-015' },
          navigate() {},
          refresh() {},
          connected: true,
        });
      } catch (err) {
        return { threw: String((err && err.stack) || err) };
      }
      const screen = host.querySelector('.screen');
      return {
        threw: null,
        screen: screen ? screen.dataset.screen : null,
        cards: host.querySelectorAll('.card, .lnode, .empty').length,
        // A screen that renders nothing but placeholders would pass a
        // "did not throw" check; require it to have put real text on screen.
        textLength: host.textContent.replace(/\s/g, '').length,
      };
    }, id);
    check(
      `screen ${id} renders from a representative payload without throwing`,
      outcome.threw === null && outcome.screen === id && outcome.cards > 0 && outcome.textLength > 80,
      outcome
    );
  }

  // ---- the task panel's other three stopping faces ----------------------
  for (const [name, fixtureId, expected] of [
    ['approve', 'fx-task-approve', '批准设计'],
    ['merge', 'fx-task-merge', '合并到 main'],
    ['failed', 'fx-task-failed', '停在失败上，怎么办'],
  ]) {
    const outcome = await evaluate(async (elementId) => {
      const api = await import('autome://app/lib/api.js');
      api.setConnected(true);
      api.resetWriteControls();
      const module = await import('autome://app/screens/task.js');
      const host = document.getElementById('main');
      host.replaceChildren();
      try {
        module.render(host, JSON.parse(document.getElementById(elementId).textContent), {
          params: { taskId: 'T-013' },
          navigate() {},
          refresh() {},
          connected: true,
        });
      } catch (err) {
        return { threw: String((err && err.stack) || err) };
      }
      const titles = Array.from(host.querySelectorAll('.card__title')).map((t) => t.textContent);
      const mergeButton = Array.from(host.querySelectorAll('button')).find((b) =>
        b.textContent.startsWith('合并到')
      );
      return {
        threw: null,
        titles,
        mergeBlocked: mergeButton ? mergeButton.disabled : null,
        stones: host.querySelectorAll('.stone').length,
      };
    }, fixtureId);
    check(
      `the stopping panel shows the ${name} face, and the whole node flow renders`,
      outcome.threw === null && outcome.titles.includes(expected) && outcome.stones === FLOW_STONES,
      outcome
    );
  }

  // An unparseable design document must say which line, not just that it is
  // unparseable: the document is hundreds of KB and the user has to go edit
  // one row of it.
  const unreadable = await evaluate(async () => {
    const module = await import('autome://app/screens/task.js');
    const host = document.getElementById('main');
    host.replaceChildren();
    module.render(host, JSON.parse(document.getElementById('fx-task-unreadable').textContent), {
      params: {},
      navigate() {},
      refresh() {},
      connected: true,
    });
    return {
      texts: Array.from(host.querySelectorAll('.empty')).map((e) => e.textContent),
      stones: host.querySelectorAll('.stone').length,
    };
  });
  check(
    'an unreadable status block names the offending line in the milestone card',
    unreadable.stones === FLOW_STONES &&
      unreadable.texts.some((t) => t.includes('第 2124 行') && t.includes('读不出来')),
    unreadable
  );

  // T-07: the merge fixture's main worktree is dirty, so the button that
  // cannot succeed must not be offered as if it could.
  const mergeBlocked = await evaluate(async () => {
    const module = await import('autome://app/screens/task.js');
    const host = document.getElementById('main');
    host.replaceChildren();
    module.render(host, JSON.parse(document.getElementById('fx-task-merge').textContent), {
      params: {},
      navigate() {},
      refresh() {},
      connected: true,
    });
    const button = Array.from(host.querySelectorAll('button')).find((b) =>
      b.textContent.startsWith('合并到')
    );
    return { found: Boolean(button), disabled: button ? button.disabled : null, title: button ? button.title : null };
  });
  check(
    'with the main worktree dirty, the merge button is present but disabled and states the blocker',
    mergeBlocked.found && mergeBlocked.disabled === true && /未提交改动/.test(mergeBlocked.title || ''),
    mergeBlocked
  );

  // ---- C-06: SAME-MODEL paints the node red and disables save ------------
  const sameModel = await evaluate(async () => {
    const api = await import('autome://app/lib/api.js');
    api.setConnected(true);
    api.resetWriteControls();
    const routing = await import('autome://app/screens/routing.js');
    const host = document.getElementById('main');
    host.replaceChildren();
    routing.render(host, JSON.parse(document.getElementById('fx-routing').textContent), {
      params: {},
      navigate() {},
      refresh() {},
      connected: true,
    });
    const audit = host.querySelector('[data-node="audit"]');
    const impl = host.querySelector('[data-node="impl"]');
    const save = Array.from(host.querySelectorAll('button')).find((b) => b.textContent.startsWith('保存'));
    return {
      auditRed: audit ? audit.classList.contains('lnode--err') : null,
      implRed: impl ? impl.classList.contains('lnode--err') : null,
      saveDisabled: save ? save.disabled : null,
      saveReason: save ? save.title : null,
      banner: Boolean(host.querySelector('.alertbar')),
      roleNodes: host.querySelectorAll('.lnode--ai').length,
      fixedNodes: host.querySelectorAll('.lnode--rust').length,
      humanNodes: host.querySelectorAll('.lnode--human').length,
    };
  });
  check(
    'a SAME-MODEL collision paints both role nodes red, raises the banner and disables 保存 (C-06)',
    sameModel.auditRed === true &&
      sameModel.implRed === true &&
      sameModel.saveDisabled === true &&
      sameModel.banner === true,
    sameModel
  );
  check(
    'every role in the config gets a configurable node, and the fixed steps and human stops stay inert (C-04, U-08)',
    sameModel.roleNodes === ROLE_COUNT && sameModel.fixedNodes === 4 && sameModel.humanNodes === 2,
    { ...sameModel, expectedRoleNodes: ROLE_COUNT }
  );

  // ---- the usage card, and what the two gates have to disclose ----------
  //
  // Every number is absent rather than zero when nothing measured it. A task
  // run entirely on Codex has an unknown cost — Codex reports no price — and a
  // dash says that where `$0.00` would be a claim.
  const usage = await evaluate(async () => {
    const module = await import('autome://app/screens/task.js');
    const host = document.getElementById('main');
    host.replaceChildren();
    module.render(host, JSON.parse(document.getElementById('fx-task-measured').textContent), {
      params: {},
      navigate() {},
      refresh() {},
      connected: true,
    });
    const titles = Array.from(host.querySelectorAll('.card__title')).map((t) => t.textContent);
    return {
      titles,
      text: host.textContent,
      alerts: Array.from(host.querySelectorAll('.alertbar')).map((a) => a.textContent),
    };
  });
  check(
    'the usage card shows tokens and rounds, and says a cost it does not know is unknown',
    usage.titles.includes('用量') &&
      usage.text.includes('1.8M') &&
      usage.text.includes('9/35') &&
      usage.text.includes('protocol/v2') &&
      usage.text.includes('Codex 不报价'),
    usage
  );
  check(
    'a change the review round could not judge is disclosed at the gate (plan §6.3)',
    usage.alerts.some((a) => a.includes('需人工特批') && a.includes('prompts/review.md')),
    usage
  );

  // ---- the version page states its own limits ---------------------------
  const version = await evaluate(async () => {
    const module = await import('autome://app/screens/protocol.js');
    const host = document.getElementById('main');
    host.replaceChildren();
    module.render(host, JSON.parse(document.getElementById('fx-protocol').textContent), {
      params: {},
      navigate() {},
      refresh() {},
      connected: true,
    });
    return {
      text: host.textContent,
      rows: host.querySelectorAll('.metrics tbody tr').length,
      tags: Array.from(host.querySelectorAll('.tag')).map((t) => t.textContent),
    };
  });
  check(
    'the version page says 样本不足 rather than drawing a line between two points',
    version.rows === 2 && version.text.includes('样本不足'),
    version
  );
  check(
    'a prediction the numbers went against is labelled as such, not quietly dropped',
    version.tags.some((t) => t.includes('与预测相反')),
    version
  );
  check(
    'rolling back is offered as a forward commit, not as moving a tag',
    version.text.includes('回到这一版的内容'),
    version
  );

  // ---- the router maps each nav item to its screen -----------------------
  const routerMap = await evaluate(async () => {
    const appModule = await import('autome://app/app.js');
    const out = [];
    for (const button of Array.from(document.querySelectorAll('#nav .nav__item'))) {
      const label = button.textContent;
      // Reads reject in this harness, so the router renders the screen's
      // error face — which still carries `data-screen`, by design.
      // eslint-disable-next-line no-await-in-loop
      await appModule.navigate(button.dataset.nav === 'dash' ? 'dash' : button.dataset.nav);
      const screen = document.querySelector('#main .screen');
      const active = document.querySelector('#nav .nav__item.active');
      out.push({
        label,
        nav: button.dataset.nav,
        screen: screen ? screen.dataset.screen : null,
        activeNav: active ? active.dataset.nav : null,
      });
    }
    // The three screens without a sidebar entry are reached by navigation.
    for (const id of ['project', 'task', 'routing']) {
      // eslint-disable-next-line no-await-in-loop
      await appModule.navigate(id, { projectId: 'prj_island', taskId: 'T-015' });
      const screen = document.querySelector('#main .screen');
      const active = document.querySelector('#nav .nav__item.active');
      out.push({
        label: id,
        nav: appModule.SCREENS[id].nav,
        screen: screen ? screen.dataset.screen : null,
        activeNav: active ? active.dataset.nav : null,
      });
    }
    return out;
  });
  check(
    'every nav item and every deep screen routes to its own screen and lights the right sidebar entry',
    routerMap.length === 8 &&
      routerMap.map((r) => r.screen).join(',') === 'dash,projects,env,skills,settings,project,task,routing' &&
      routerMap.every((row) => row.activeNav === row.nav),
    routerMap
  );

  // ---- U-11: one screen, no vertical scroll -----------------------------
  for (const id of SCREEN_IDS) {
    const fit = await evaluate(async (screenId) => {
      const api = await import('autome://app/lib/api.js');
      api.setConnected(true);
      api.resetWriteControls();
      const module = await import(`autome://app/screens/${screenId === 'dash' ? 'dashboard' : screenId}.js`);
      const host = document.getElementById('main');
      host.replaceChildren();
      module.render(host, JSON.parse(document.getElementById(`fx-${screenId}`).textContent), {
        params: { projectId: 'prj_island', taskId: 'T-015' },
        navigate() {},
        refresh() {},
        connected: true,
      });
      await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      return {
        scrollHeight: host.scrollHeight,
        clientHeight: host.clientHeight,
        viewport: { w: window.innerWidth, h: window.innerHeight },
      };
    }, id);
    check(
      `screen ${id} fits 1512x944 with no vertical scroll (U-11)`,
      fit.viewport.w === VIEWPORT.width &&
        fit.viewport.h === VIEWPORT.height &&
        fit.scrollHeight <= fit.clientHeight,
      fit
    );
  }

  // ---- the security boundary --------------------------------------------
  const noInlineStyle = await evaluate(async () => {
    const dom = await import('autome://app/lib/dom.js');
    try {
      dom.h('div', { style: 'color:red' });
      return { rejected: false };
    } catch (err) {
      return { rejected: true, message: err.message };
    }
  });
  check(
    'h() refuses an inline style attribute, which the CSP would block anyway',
    noInlineStyle.rejected === true,
    noInlineStyle
  );

  const textNotMarkup = await evaluate(async () => {
    const dom = await import('autome://app/lib/dom.js');
    const host = document.getElementById('main');
    host.replaceChildren();
    // A title an agent could have written into a repository document.
    const hostile = '<img src=x onerror="window.__pwned=true">';
    host.appendChild(dom.h('div.taskcard__t', { text: hostile }));
    host.appendChild(dom.tag(hostile, 'outlined'));
    await new Promise((resolve) => setTimeout(resolve, 50));
    return {
      pwned: Boolean(window.__pwned),
      images: host.querySelectorAll('img').length,
      renderedAsText: host.textContent.includes('onerror'),
    };
  });
  check(
    'hostile text from a payload is rendered as text, never parsed as markup',
    textNotMarkup.pwned === false && textNotMarkup.images === 0 && textNotMarkup.renderedAsText,
    textNotMarkup
  );

  // ---- the Onboarding wizard (requirement C-02) --------------------------
  //
  // Steps 3, 4 and 5 each need something different from the user. A wizard
  // that showed the same "next" button at every step would leave the user with
  // no way to run the drafting session or to confirm what it produced, which
  // is what shipped before this.
  const wizard = await evaluate(async () => {
    const projects = await import('autome://app/screens/projects.js');
    const overlay = await import('autome://app/lib/overlay.js');
    const out = {};
    for (const step of [3, 4, 5]) {
      overlay.close();
      const project = {
        id: 'prj-onboarding',
        path: '/tmp/p',
        display_name: '珊瑚笔记',
        default_branch: 'main',
        onboarding: { onboarding: 'in_progress', step },
      };
      projects.__testOpenOnboarding(project, {
        refresh: async () => {},
        navigate: () => {
          out.navigated = true;
        },
      });
      const modal = document.getElementById('modal');
      out[step] = {
        open: document.getElementById('modal-mask').classList.contains('open'),
        buttons: Array.from(modal.querySelectorAll('button')).map((b) => b.textContent.trim()),
        current: modal.querySelectorAll('.wstep.now').length,
        done: modal.querySelectorAll('.wstep.done').length,
      };
    }
    overlay.close();
    return out;
  });
  check(
    'the wizard marks the step it is on, and the ones before it as done',
    wizard[3].current === 1 && wizard[3].done === 2 && wizard[5].done === 4,
    wizard
  );
  check(
    'step 3 offers to run the drafting session',
    wizard[3].buttons.some((b) => b.includes('运行起草会话')),
    wizard[3].buttons
  );
  check(
    'step 4 offers to view and edit the artefacts',
    wizard[4].buttons.some((b) => b.includes('查看并编辑产物')),
    wizard[4].buttons
  );
  check(
    'step 5 sends the user to the routing graph rather than duplicating it',
    wizard[5].buttons.some((b) => b.includes('去路由图')),
    wizard[5].buttons
  );
  check(
    'every step can be skipped, because the whole wizard is optional',
    [3, 4, 5].every((s) => wizard[s].buttons.some((b) => b.includes('跳过剩下的'))),
    wizard
  );

  // The editor reads through the core, so with no sidecar the read rejects.
  // What matters is that the failure is reported rather than swallowed into
  // an empty editor the user could save over their profile.
  const editorOffline = await evaluate(async () => {
    const projects = await import('autome://app/screens/projects.js');
    const before = document.querySelectorAll('.notif--error').length;
    await projects.__testOpenArtefactEditor(
      { id: 'prj-onboarding', display_name: 'x' },
      { refresh: async () => {} }
    );
    await new Promise((resolve) => setTimeout(resolve, 60));
    return {
      errors: document.querySelectorAll('.notif--error').length - before,
      drawerOpen: document.getElementById('drawer').classList.contains('open'),
      editors: document.querySelectorAll('.artefact__text').length,
    };
  });
  check(
    'a failed artefact read is reported and opens no editor to save over',
    editorOffline.errors === 1 && editorOffline.drawerOpen === false && editorOffline.editors === 0,
    editorOffline
  );

  const cspViolations = consoleMessages.filter((m) =>
    /content security policy|refused to/i.test(String(m))
  );
  check('no CSP violations logged to the console', cspViolations.length === 0, cspViolations);

  process.stdout.write(JSON.stringify(results));
  app.exit(0);
}

/**
 * The fixtures reach the page as `<script type="application/json">` blobs.
 * Passing them through `executeJavaScript` arguments would work too, but the
 * payloads are large and would be re-serialised into every evaluated snippet;
 * parking them in the document once keeps each snippet readable.
 */
function fixtureInjector() {
  const blobs = Object.assign({}, FIXTURES, {
    'task-merge': TASK_AT_MERGE,
    'task-failed': TASK_FAILED_PANEL,
    'task-approve': TASK_APPROVE_PANEL,
    'task-unreadable': TASK_UNREADABLE_DOC_PANEL,
    'task-measured': TASK_MEASURED_PANEL,
    protocol: PROTOCOL_SCREEN,
  });
  return `(() => {
    const blobs = ${JSON.stringify(blobs)};
    for (const [key, value] of Object.entries(blobs)) {
      const el = document.createElement('script');
      el.type = 'application/json';
      el.id = 'fx-' + key;
      el.textContent = JSON.stringify(value);
      document.body.appendChild(el);
    }
    return true;
  })()`;
}

app.whenReady()
  .then(run)
  .catch((err) => {
    process.stderr.write(String((err && err.stack) || err));
    app.exit(1);
  });
